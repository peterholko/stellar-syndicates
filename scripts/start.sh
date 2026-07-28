#!/usr/bin/env bash
# One command to get a clean game running: rebuild the client, rebuild the
# server, throw away the old galaxy, and start fresh.
#
#   scripts/start.sh
#
# What it does, in order:
#   1. Builds the client (tsc --noEmit + vite build) into client/dist — the
#      server serves that directory, so this is what "rebuild the ux" means.
#   2. Builds target/release/server.
#   3. Stops whatever is already listening on $PORT (TERM, then KILL).
#   4. Wipes the persisted galaxy so the server boots a NEW one. With no
#      DATABASE_URL the server is already in-memory, so a restart IS a fresh
#      galaxy; with one set, the snapshot is truncated (that's the destructive
#      step — pass --keep-galaxy to resume the old one instead).
#   5. Starts the server and waits for /healthz, then reports which galaxy it got.
#
# Env: PORT (8080), GALAXY_SEED, MAX_PLAYERS, DATABASE_URL, RUST_LOG — all read
# by the server itself; this script only passes them through.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT="${PORT:-8080}"
# Per-port so two servers on different ports never clobber each other's log
# (gitignored as server*.log).
LOG="$ROOT/server-$PORT.log"
BIN="$ROOT/target/release/server"
KEEP_GALAXY=0

for arg in "$@"; do
  case "$arg" in
    --keep-galaxy) KEEP_GALAXY=1 ;;
    # Print this file's header comment (everything after the shebang) as the help.
    -h|--help) awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown option: $arg (try --help)" >&2; exit 1 ;;
  esac
done

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

step "Building the client → client/dist"
npm --prefix client run build

step "Building the server → target/release/server"
cargo build --release

step "Stopping anything on port $PORT"
PIDS="$(lsof -ti "tcp:$PORT" -sTCP:LISTEN 2>/dev/null || true)"
if [ -n "$PIDS" ]; then
  # shellcheck disable=SC2086
  kill $PIDS 2>/dev/null || true
  for _ in $(seq 1 40); do
    lsof -ti "tcp:$PORT" -sTCP:LISTEN >/dev/null 2>&1 || break
    sleep 0.25
  done
  # Still holding the port after 10s — take it.
  REMAIN="$(lsof -ti "tcp:$PORT" -sTCP:LISTEN 2>/dev/null || true)"
  # shellcheck disable=SC2086
  [ -n "$REMAIN" ] && kill -9 $REMAIN 2>/dev/null || true
  echo "  stopped: $(echo "$PIDS" | tr '\n' ' ')"
else
  echo "  nothing was listening"
fi

step "Resetting the galaxy"
if [ "$KEEP_GALAXY" = 1 ]; then
  echo "  --keep-galaxy: leaving any snapshot in place (the server will resume it)"
elif [ -n "${DATABASE_URL:-}" ]; then
  if command -v psql >/dev/null 2>&1; then
    psql "$DATABASE_URL" -q -c "truncate snapshots, events;" \
      && echo "  truncated snapshots + events — the server will generate a new galaxy"
  else
    echo "  WARNING: DATABASE_URL is set but psql is not installed; the old galaxy" >&2
    echo "           will be restored from its snapshot. Install psql or unset" >&2
    echo "           DATABASE_URL for an in-memory galaxy." >&2
  fi
else
  echo "  no DATABASE_URL — the server runs in-memory, so this start is a new galaxy"
fi

step "Starting the server on :$PORT"
: >"$LOG"
# nohup + disown so the server outlives this script (and the shell that ran it).
nohup "$BIN" >>"$LOG" 2>&1 &
SRV=$!
disown "$SRV" 2>/dev/null || true
for _ in $(seq 1 80); do
  if curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1; then
    break
  fi
  # Died during startup — surface the log rather than spinning for 20s.
  if ! kill -0 "$SRV" 2>/dev/null; then
    echo "  server exited during startup:" >&2
    tail -20 "$LOG" >&2
    exit 1
  fi
  sleep 0.25
done
if ! curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1; then
  echo "  server never became healthy:" >&2
  tail -20 "$LOG" >&2
  exit 1
fi

# The server's log lines carry ANSI colour even when redirected, so strip it
# before matching (otherwise `seed=` never matches: the escapes sit in between).
CLEAN="$(perl -pe 's/\e\[[0-9;]*m//g' "$LOG")"
if printf '%s\n' "$CLEAN" | grep -q "initialising fresh galaxy"; then
  echo "  new galaxy ($(printf '%s\n' "$CLEAN" | grep -o 'seed=[0-9]*' | head -1))"
elif printf '%s\n' "$CLEAN" | grep -q "resuming galaxy from snapshot"; then
  echo "  RESUMED the existing galaxy from its snapshot — not a new one"
fi

printf '\n\033[1mReady →\033[0m http://localhost:%s   (pid %s, log %s)\n' "$PORT" "$SRV" "$(basename "$LOG")"
echo "Stop it with: kill $SRV"

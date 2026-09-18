#!/usr/bin/env bash
# Rebuild the client/server and RESUME the saved galaxy by default.
#
#   scripts/start.sh                  # ordinary safe restart
#   scripts/start.sh --reset-galaxy   # explicit new galaxy; old files archived
#   BOT_PLAYERS=8 MAX_PLAYERS=8 scripts/start.sh --reset-galaxy  # eight bots
#
# Full galaxy checkpoints run every 15 wall minutes and on clean shutdown.
# A hard crash resumes the last successful save; intervening progress is lost.
# Saves live in GALAXY_DATA_DIR (default saves/galaxy-PORT); back up that WHOLE
# directory, not just its newest checkpoint. Accounts remain in PostgreSQL and
# are never reset here. --keep-galaxy is retained as a compatibility no-op.
#
# Accounts: ACCOUNTS_DATABASE_URL (falls back to DATABASE_URL), APP_ORIGIN.
# Local development starts scripts/devdb.sh if neither DB URL is set.
# Env: PORT (8080), SIM_PACING (1), GALAXY_SEED, MAX_PLAYERS, BOT_PLAYERS, RUST_LOG.
# Bots are saved with the galaxy; ordinary restarts need no BOT_PLAYERS override.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT="${PORT:-8080}"
# Per-port so two servers on different ports never clobber each other's log
# (gitignored as server*.log).
LOG="$ROOT/server-$PORT.log"
BIN="$ROOT/target/release/server"
RESET_GALAXY=0

for arg in "$@"; do
  case "$arg" in
    --keep-galaxy) ;; # resume is now the default
    --reset-galaxy) RESET_GALAXY=1 ;;
    # Print this file's header comment (everything after the shebang) as the help.
    -h|--help) awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "unknown option: $arg (try --help)" >&2; exit 1 ;;
  esac
done

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

if [ -z "${ACCOUNTS_DATABASE_URL:-}" ] && [ -z "${DATABASE_URL:-}" ]; then
  case "${APP_ORIGIN:-http://localhost:$PORT}" in
    http://localhost:*|http://127.0.0.1:*)
      step "Starting local PostgreSQL for accounts"
      scripts/devdb.sh init
      ACCOUNTS_DATABASE_URL="$(scripts/devdb.sh url)"
      export ACCOUNTS_DATABASE_URL
      ;;
    *)
      echo "Set ACCOUNTS_DATABASE_URL before starting a public server." >&2
      exit 1
      ;;
  esac
fi

step "Building the client → client/dist"
npm --prefix client run build

step "Building the server → target/release/server"
cargo build --release

step "Stopping anything on port $PORT"
PIDS="$(lsof -ti "tcp:$PORT" -sTCP:LISTEN 2>/dev/null || true)"
if [ -n "$PIDS" ]; then
  # shellcheck disable=SC2086
  kill $PIDS 2>/dev/null || true
  # The listener can close before the final checkpoint finishes. Wait for the
  # actual process, and never SIGKILL a server that may be saving the galaxy.
  for _ in $(seq 1 240); do
    ALIVE=0
    for pid in $PIDS; do kill -0 "$pid" 2>/dev/null && ALIVE=1; done
    [ "$ALIVE" = 0 ] && break
    sleep 0.25
  done
  if [ "$ALIVE" != 0 ]; then
    echo "Server is still shutting down/saving after 60s; restart aborted. Check its log." >&2
    exit 1
  fi
  echo "  stopped: $(echo "$PIDS" | tr '\n' ' ')"
else
  echo "  nothing was listening"
fi

if [ "$RESET_GALAXY" = 1 ]; then
  step "Explicit galaxy reset (server will archive the previous save files)"
else
  step "Resuming the galaxy from its last saved checkpoint"
fi

step "Starting the server on :$PORT"
: >"$LOG"
# nohup + disown so the server outlives this script (and the shell that ran it).
if [ "$RESET_GALAXY" = 1 ]; then
  nohup "$BIN" --reset-galaxy >>"$LOG" 2>&1 &
else
  nohup "$BIN" >>"$LOG" 2>&1 &
fi
SRV=$!
disown "$SRV" 2>/dev/null || true
for _ in $(seq 1 480); do
  if curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1; then
    break
  fi
  # Died during startup — surface the log rather than waiting through recovery.
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
elif printf '%s\n' "$CLEAN" | grep -q "resuming galaxy from last saved checkpoint"; then
  echo "  RESUMED the existing galaxy from its last saved checkpoint"
fi

printf '\n\033[1mReady →\033[0m http://localhost:%s   (pid %s, log %s)\n' "$PORT" "$SRV" "$(basename "$LOG")"
echo "Stop it with: kill $SRV"

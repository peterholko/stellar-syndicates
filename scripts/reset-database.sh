#!/usr/bin/env bash
# Reset local registration data AND start a fresh galaxy for onboarding tests.
#
#   scripts/reset-database.sh            # asks you to type RESET
#   scripts/reset-database.sh --dry-run  # read-only target checks; no reset
#   scripts/reset-database.sh --yes      # explicitly skip confirmation
#
# Permanently deletes ALL accounts and login sessions in this project's .devdb.
# Keeps the database/schema and other tables. The old galaxy is archived by the
# ordinary restart script, not deleted. Starts without bots; MAX_PLAYERS still
# controls map capacity. Refresh the browser afterward and choose Create account.
#
# Local development ONLY. Refuses other databases, custom galaxy directories,
# non-local APP_ORIGIN values and unrelated processes on the game port.
# PORT (8080) and DEVDB_PORT (5433) may select other local ports.
# Does not touch earlier simulation runs or other saved galaxy archives.
set -euo pipefail

RESET_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$RESET_ROOT"
RESET_YES=0
RESET_DRY_RUN=0

fail() { printf 'Reset aborted: %s\n' "$*" >&2; exit 1; }
for arg in "$@"; do
  case "$arg" in
    --yes) RESET_YES=1 ;;
    --dry-run) RESET_DRY_RUN=1 ;;
    -h|--help)
      awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' "${BASH_SOURCE[0]}"
      exit 0 ;;
    *) fail "unknown option: $arg (try --help)" ;;
  esac
done

valid_port() { [[ "$1" =~ ^[0-9]{1,5}$ ]] && (( 10#$1 >= 1 && 10#$1 <= 65535 )); }
RESET_PORT="${PORT:-8080}"
RESET_DB_PORT="${DEVDB_PORT:-5433}"
valid_port "$RESET_PORT" || fail "PORT must be between 1 and 65535."
valid_port "$RESET_DB_PORT" || fail "DEVDB_PORT must be between 1 and 65535."
RESET_DB_URL="postgres://postgres@127.0.0.1:$RESET_DB_PORT/stellar_syndicates"
RESET_DB_DIR="$RESET_ROOT/.devdb"
RESET_GALAXY_DIR="$RESET_ROOT/saves/galaxy-$RESET_PORT"
RESET_BIN="$RESET_ROOT/target/release/server"
RESET_ORIGIN="${APP_ORIGIN:-http://localhost:$RESET_PORT}"

for configured_url in "${ACCOUNTS_DATABASE_URL:-}" "${DATABASE_URL:-}"; do
  [[ -z "$configured_url" || "$configured_url" == "$RESET_DB_URL" ]] ||
    fail "a database URL override targets a different database. This helper only resets this project's .devdb."
done
case "$RESET_ORIGIN" in
  "http://localhost:$RESET_PORT"|"http://127.0.0.1:$RESET_PORT") ;;
  *) fail "APP_ORIGIN must be the local game URL." ;;
esac
case "${GALAXY_DATA_DIR:-}" in
  ""|"$RESET_GALAXY_DIR"|"saves/galaxy-$RESET_PORT") ;;
  *) fail "custom GALAXY_DATA_DIR is not supported; only saves/galaxy-$RESET_PORT may be reset." ;;
esac
[[ ! -L "$RESET_DB_DIR" && ! -L "$RESET_ROOT/saves" && ! -L "$RESET_GALAXY_DIR" ]] ||
  fail "reset targets must not be symlinks."
[[ -d "$RESET_DB_DIR" ]] || fail "local .devdb is missing. Initialize it with scripts/devdb.sh init first."
[[ "${MAX_PLAYERS:-4}" =~ ^[0-9]{1,10}$ ]] &&
  (( 10#${MAX_PLAYERS:-4} >= 1 && 10#${MAX_PLAYERS:-4} <= 4294967295 )) || fail "MAX_PLAYERS must be a positive u32."
for program in psql lsof sort grep npm cargo; do
  command -v "$program" >/dev/null || fail "$program is required."
done

# No psql startup file, password prompt, remote URL, unqualified table or CASCADE.
db() { PGCONNECT_TIMEOUT=5 psql -X -w --dbname="$RESET_DB_URL" --set=ON_ERROR_STOP=1 "$@"; }
verify_database() {
  local database directory
  database="$(db -At --command='SELECT current_database();')" || fail "cannot connect to the local database."
  directory="$(db -At --command='SHOW data_directory;')" || fail "cannot verify PostgreSQL's data directory."
  [[ "$database" == "stellar_syndicates" && "$directory" == "$RESET_DB_DIR" ]] ||
    fail "PostgreSQL does not match this project's .devdb; nothing was deleted."
}
listener_pids() {
  local pids code=0
  pids="$(lsof -ti "tcp:$RESET_PORT" -sTCP:LISTEN)" || code=$?
  [[ "$code" -le 1 ]] || fail "cannot inspect the game port."
  printf '%s\n' "$pids" | sort -nu
}
verify_server() {
  local executable
  [[ "$1" =~ ^[1-9][0-9]*$ ]] || fail "invalid listener PID."
  executable="$(lsof -a -p "$1" -d txt -Fn)" || fail "cannot verify server PID $1."
  # A rebuild can replace the executable while the previous process still runs.
  if ! printf '%s\n' "$executable" | grep -Fx -e "n$RESET_BIN" -e "n$RESET_BIN (deleted)" >/dev/null; then
    fail "PID $1 on port $RESET_PORT is not this project's release server."
  fi
}

verify_database
RESET_COUNT="$(db -At --command='SELECT count(*) FROM public.accounts;')" || fail "accounts table is unavailable."
RESET_PIDS="$(listener_pids)"
for pid in $RESET_PIDS; do verify_server "$pid"; done
printf '\nLocal registration reset\n  Database: %s\n  Cluster: %s\n  Accounts to permanently delete: %s\n  Galaxy to archive: %s\n  New galaxy: no bots\n\n' \
  "$RESET_DB_URL" "$RESET_DB_DIR" "$RESET_COUNT" "$RESET_GALAXY_DIR"
if [[ "$RESET_DRY_RUN" == 1 ]]; then
  printf 'Dry run only. No builds, process stops, account deletions or galaxy changes.\n'
  exit 0
fi
if [[ "$RESET_YES" != 1 ]]; then
  printf 'There is no account backup. Type RESET to delete all local accounts and sessions: '
  IFS= read -r confirmation || confirmation=""
  [[ "$confirmation" == RESET ]] || { printf 'Cancelled. Nothing changed.\n'; exit 0; }
fi

# Serialize destructive resets; never remove a pre-existing lock on failure.
RESET_LOCK="$RESET_DB_DIR/registration-reset.lock"
mkdir "$RESET_LOCK" 2>/dev/null || fail "another reset may be running ($RESET_LOCK)."
trap 'rmdir "$RESET_LOCK" 2>/dev/null || true' EXIT

# Catch compilation failures before deleting accounts or stopping the old game.
# restart.sh repeats these builds, but they are then incremental/up-to-date.
npm --prefix "$RESET_ROOT/client" run build
cargo build --release
[[ "$(listener_pids)" == "$RESET_PIDS" ]] || fail "the game listener changed during the build. Retry after checking it."
for pid in $RESET_PIDS; do
  verify_server "$pid"
  kill -TERM "$pid"
done
RESET_ALIVE=0
for ((attempt=0; attempt<240; attempt++)); do
  RESET_ALIVE=0
  for pid in $RESET_PIDS; do kill -0 "$pid" 2>/dev/null && RESET_ALIVE=1; done
  [[ "$RESET_ALIVE" == 0 ]] && break
  sleep 0.25
done
[[ "$RESET_ALIVE" == 0 ]] || fail "server did not finish its shutdown/save within 60s; no accounts were deleted."
[[ -z "$(listener_pids)" ]] || fail "a new listener appeared; no accounts were deleted."
verify_database
db --single-transaction --command="SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '10s'; TRUNCATE TABLE public.account_sessions, public.accounts;"
printf 'Deleted all local accounts and login sessions. Starting a fresh galaxy…\n'

if ! ACCOUNTS_DATABASE_URL="$RESET_DB_URL" PORT="$RESET_PORT" APP_ORIGIN="$RESET_ORIGIN" \
  GALAXY_DATA_DIR="$RESET_GALAXY_DIR" BOT_PLAYERS=0 "$RESET_ROOT/scripts/restart.sh" --reset-galaxy; then
  printf 'Accounts were deleted, but galaxy startup failed. Fix the error and retry this script; do not resume the old galaxy with the erased accounts.\n' >&2
  exit 1
fi
printf '\nReset complete. Refresh %s and choose Create account.\n' "$RESET_ORIGIN"

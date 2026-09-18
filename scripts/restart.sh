#!/usr/bin/env bash
# Rebuild the client/server and restart the game, preserving the saved galaxy.
#
#   scripts/restart.sh                  # restart and resume the current galaxy
#   scripts/restart.sh --reset-galaxy   # fresh galaxy; previous saves archived
#
# Uses start.sh's graceful shutdown, final checkpoint and startup health check.
# PostgreSQL accounts are preserved, including when resetting the galaxy.
# Accepts the same options and environment variables as start.sh.
set -euo pipefail

RESTART_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

for arg in "$@"; do
  case "$arg" in
    -h|--help)
      awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' "${BASH_SOURCE[0]}"
      exit 0
      ;;
  esac
done

exec "$RESTART_DIR/start.sh" "$@"

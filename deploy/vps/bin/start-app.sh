#!/usr/bin/env bash
# Starts the game server (systemd user unit) and waits for /healthz.
# `start-app.sh --reset-galaxy` passes its arguments to this one start only.
set -euo pipefail

ROOT=/home/peter/stellar-syndicates
UNIT=stellar-syndicates.service
LOG_FILE="$ROOT/logs/app.log"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
mkdir -p "$ROOT/run" "$ROOT/logs"

if (($#)); then
  if systemctl --user is-active --quiet "$UNIT"; then
    echo "already running; stop it first to start with: $*" >&2
    exit 1
  fi
  printf '%s\n' "$@" >"$ROOT/run/next-start-args"
fi

systemctl --user start "$UNIT"
pid="$(systemctl --user show --property=MainPID --value "$UNIT")"

for _ in $(seq 1 240); do
  if curl -fsS http://127.0.0.1:18080/healthz >/dev/null 2>&1; then
    exit 0
  fi
  # A different pid means it died and systemd is restarting it.
  [[ "$(systemctl --user show --property=MainPID --value "$UNIT")" == "$pid" ]] || break
  sleep 0.25
done

tail -40 "$LOG_FILE" >&2
exit 1

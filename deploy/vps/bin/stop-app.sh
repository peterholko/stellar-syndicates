#!/usr/bin/env bash
# Stops the game server and returns once its shutdown save is written.
set -euo pipefail
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
exec systemctl --user stop stellar-syndicates.service

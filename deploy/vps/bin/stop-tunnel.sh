#!/usr/bin/env bash
# Stops both tunnel replicas until start-tunnel.sh or the next boot.
set -euo pipefail
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
exec systemctl --user stop stellar-syndicates-tunnel@18081.service stellar-syndicates-tunnel@18083.service

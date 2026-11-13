#!/usr/bin/env bash
# The tunnel runs as two systemd user services (replicas) that start at boot
# and restart cloudflared if it dies; update-tunnel.sh keeps it current.
set -euo pipefail
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
exec systemctl --user start stellar-syndicates-tunnel@18081.service stellar-syndicates-tunnel@18083.service

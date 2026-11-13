#!/usr/bin/env bash
# Peter's half of the Debian setup (docs/vps-debian-setup.md): the Stellar
# Syndicates folders, scripts and user services, cloudflared and Rust. Run as
# peter after debian-setup-root.sh. Safe to run again.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SS="$HOME/stellar-syndicates"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

[[ $(id -un) == peter ]] || { echo "run as peter" >&2; exit 1; }
[[ -s $SS/secrets/app.env ]] || { echo "run debian-setup-root.sh first" >&2; exit 1; }

step() { printf '\n== %s\n' "$*"; }

step "Stellar Syndicates folders and scripts"
mkdir -p "$SS"/{bin,releases,data,logs,run,secrets}
install -m 755 "$HERE"/bin/*.sh "$SS/bin/"

step "cloudflared"
mkdir -p "$HOME/.local/bin"
if [[ ! -x $HOME/.local/bin/cloudflared ]]; then
  curl -fL -o "$HOME/.local/bin/cloudflared.new" \
    https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64
  chmod 755 "$HOME/.local/bin/cloudflared.new"
  mv "$HOME/.local/bin/cloudflared.new" "$HOME/.local/bin/cloudflared"
fi
"$HOME/.local/bin/cloudflared" --version

step "systemd user services"
mkdir -p "$HOME/.config/systemd/user"
install -m 644 "$HERE"/systemd-user/* "$HOME/.config/systemd/user/"
systemctl --user daemon-reload
# The game server is enabled but has nothing to run until the first deploy.
systemctl --user enable stellar-syndicates.service \
  stellar-syndicates-tunnel@18081.service stellar-syndicates-tunnel@18083.service
systemctl --user enable --now stellar-syndicates-tunnel-update.timer
if [[ -s $SS/secrets/tunnel-token ]]; then
  chmod 600 "$SS/secrets/tunnel-token"
  "$SS/bin/start-tunnel.sh"
  echo "tunnel replicas: $(systemctl --user is-active stellar-syndicates-tunnel@18081.service stellar-syndicates-tunnel@18083.service | tr '\n' ' ')"
else
  echo "No tunnel token yet. Put it in $SS/secrets/tunnel-token, then run:"
  echo "  $SS/bin/start-tunnel.sh"
fi

step "Rust"
if [[ ! -x $HOME/.cargo/bin/cargo ]]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
"$HOME/.cargo/bin/cargo" --version

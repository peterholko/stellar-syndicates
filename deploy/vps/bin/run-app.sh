#!/usr/bin/env bash
# The game server in the foreground; stellar-syndicates.service runs this.
set -euo pipefail

ROOT=/home/peter/stellar-syndicates
cd "$ROOT/current"

# DATABASE_URL: Stellar Syndicates' own role and database
# (written by debian-setup-root.sh).
set -a
. "$ROOT/secrets/app.env"
set +a

export PORT=18080
export BIND_ADDR=127.0.0.1
export MAX_PLAYERS=5
export SIM_PACING=1
export RUST_LOG=info
export APP_ORIGIN=https://stellarsyndicates.com
# Front door (docs/front-door.md): the game socket, /status and /healthz on
# their own host, and the sign-in cookie shared with it.
export GAME_ORIGIN=https://play.stellarsyndicates.com
export COOKIE_DOMAIN=stellarsyndicates.com
# The operator dashboard (/admin.html). Leave empty until Peter has registered
# his login on this server, or a stranger could register that name first.
export ADMIN_LOGINS=
# Saves live outside the release folders so a redeploy keeps the galaxy.
export GALAXY_DATA_DIR="$ROOT/data/galaxy-18080"

# One-shot arguments left by `start-app.sh --reset-galaxy`.
args=()
if [[ -f "$ROOT/run/next-start-args" ]]; then
  mapfile -t args <"$ROOT/run/next-start-args"
  rm -f "$ROOT/run/next-start-args"
fi

exec "$ROOT/current/target/release/server" "${args[@]}" "$@"

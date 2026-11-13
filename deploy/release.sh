#!/usr/bin/env bash
# One production release (docs/client-updates.md): build the client, build the
# server on the box, switch and restart it, then upload the client straight
# after. A protocol bump leaves server and client apart for the seconds the
# upload takes, and open tabs on the old client are told to reload.
#
#   deploy/release.sh                  the server, then the client
#   deploy/release.sh --client-only    the client alone (no restart)
#   deploy/release.sh --server-only    the server alone
#   deploy/release.sh --reset-galaxy   restart the server on a fresh galaxy
#   deploy/release.sh --dry-run        print every step, run none
#
# The box is SS_VPS (default peter@172.245.142.11; peter@vps.stellarsyndicates.com
# once that DNS record exists), reached with the key SS_KEY (default
# ~/.ssh/id_ed25519). wrangler must be logged in for the client upload.
# This changes production: only on Peter's "deploy".
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VPS="${SS_VPS:-peter@172.245.142.11}"
KEY="${SS_KEY:-$HOME/.ssh/id_ed25519}"
BOX=stellar-syndicates            # under the box's home
APEX=https://stellarsyndicates.com
PLAY=https://play.stellarsyndicates.com

server=1 client=1 dry=0 reset=""
for arg in "$@"; do
  case "$arg" in
    --client-only) server=0 ;;
    --server-only) client=0 ;;
    --reset-galaxy) reset="--reset-galaxy" ;;
    --dry-run) dry=1 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done
if (( !server && !client )); then echo "nothing to release" >&2; exit 2; fi
if [[ -n $reset ]] && (( !server )); then echo "--reset-galaxy needs the server" >&2; exit 2; fi

say() { printf '\n== %s\n' "$*"; }
# A step here, printed first.
run() {
  printf '+ %s\n' "$*"
  if (( !dry )); then "$@"; fi
}
# A step on the box, as one remote command line.
box() {
  printf '+ [box] %s\n' "$1"
  if (( !dry )); then ssh -i "$KEY" -o BatchMode=yes "$VPS" "$1"; fi
}

cd "$ROOT"
rev="$(git rev-parse --short HEAD)"
git diff --quiet HEAD -- Cargo.toml Cargo.lock crates client || rev="$rev-wip"
release="$rev-$(date -u +%Y%m%d-%H%M%S)"
say "Release $release"

if (( client )); then
  say "Client build"
  run npm --prefix client run build
  if (( !dry )); then
    [[ -s client/dist/version.json ]] || { echo "client/dist/version.json missing: the build stamp didn't run" >&2; exit 1; }
    echo "client $(cat client/dist/version.json)"
  fi
fi

if (( server )); then
  say "Server source to the box"
  box "mkdir -p $BOX/releases/$release"
  run rsync -az --delete -e "ssh -i $KEY -o BatchMode=yes" Cargo.toml Cargo.lock crates "$VPS:$BOX/releases/$release/"
  box "printf '%s\n' '$release $(git rev-parse HEAD)' > $BOX/releases/$release/REVISION"

  say "Server build on the box (about 8 min warm, 13 cold)"
  box "cd $BOX/releases/$release && CARGO_TARGET_DIR=\$HOME/$BOX/cargo-target nice -n 15 \$HOME/.cargo/bin/cargo build --release -p server -j 4 && install -D \$HOME/$BOX/cargo-target/release/server target/release/server"

  say "Switch and restart${reset:+ (fresh galaxy)}"
  previous=""
  if (( !dry )); then previous="$(ssh -i "$KEY" -o BatchMode=yes "$VPS" "readlink $BOX/current" || true)"; fi
  # start-app.sh waits for /healthz and fails loudly; the client then stays as it was.
  box "cd $BOX && ln -sfn releases/$release current.new && mv -T current.new current && bin/stop-app.sh && bin/start-app.sh $reset"
  if [[ -n $previous ]]; then
    echo "rollback: ssh -i $KEY $VPS 'cd $BOX && ln -sfn $previous current.new && mv -T current.new current && bin/stop-app.sh && bin/start-app.sh'"
  fi
fi

if (( client )); then
  # Straight after the server: the gap is this upload. Only changed files go.
  say "Client upload (Cloudflare)"
  run npm --prefix deploy/cloudflare-proxy run deploy
fi

say "Check"
if (( dry )); then
  echo "(dry run: nothing checked)"
  exit 0
fi
if (( server )); then
  printf 'healthz: '
  curl -fsS "$PLAY/healthz"
  echo
fi
if (( client )); then
  want="$(cat client/dist/version.json)"
  live=""
  for _ in 1 2 3 4 5 6; do
    live="$(curl -fsS -H 'Cache-Control: no-cache' "$APEX/version.json" || true)"
    [[ $live == "$want" ]] && break
    sleep 5
  done
  if [[ $live != "$want" ]]; then
    echo "live version.json is '$live', expected '$want'" >&2
    exit 1
  fi
  echo "live client $live"
fi
echo "released $release"

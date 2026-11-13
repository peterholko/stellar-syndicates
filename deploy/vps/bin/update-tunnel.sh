#!/usr/bin/env bash
# Updates cloudflared with its own updater, then restarts each tunnel replica
# still running an older binary, one at a time, so the other replica carries
# the tunnel meanwhile. Run daily by stellar-syndicates-tunnel-update.timer.
set -euo pipefail

BIN=/home/peter/.local/bin/cloudflared
UNITS=(stellar-syndicates-tunnel@18081.service stellar-syndicates-tunnel@18083.service)
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
cd "${BIN%/*}"

# Keep the binary in use as cloudflared.prev for rollback. A hard link is
# enough: cloudflared update renames a new file into place.
[[ "$BIN.prev" -ef "$BIN" ]] || ln -f "$BIN" "$BIN.prev"

rc=0
"$BIN" update || rc=$?
case "$rc" in
  0 | 11) ;; # 11 means it updated
  *)
    echo "cloudflared update failed with exit code $rc" >&2
    exit "$rc"
    ;;
esac
echo "On disk: $("$BIN" --version)"

ready_connections() {
  curl -fsS --max-time 2 "http://127.0.0.1:$1/ready" 2>/dev/null |
    sed -n 's/.*"readyConnections":\([0-9]*\).*/\1/p'
}

restarted=0
for unit in "${UNITS[@]}"; do
  pid="$(systemctl --user show --property=MainPID --value "$unit")"
  # Leave stopped replicas alone, and skip ones already on this binary.
  [[ "$pid" != 0 ]] || continue
  [[ /proc/$pid/exe -ef "$BIN" ]] && continue

  # Give Cloudflare time to route to the replica restarted before this one.
  ((restarted)) && sleep 10
  echo "Restarting $unit"
  # Type=notify, so this returns once the new process has an edge connection.
  if ! systemctl --user restart "$unit"; then
    echo "$unit did not start on $("$BIN" --version); rolling back" >&2
    [[ "$BIN.prev" -ef "$BIN" ]] || mv -f "$BIN.prev" "$BIN"
    systemctl --user restart "$unit" || true
    exit 1
  fi
  restarted=1
  port="${unit#*@}"
  port="${port%.service}"
  for _ in $(seq 30); do
    [[ "$(ready_connections "$port")" -ge 4 ]] && break
    sleep 1
  done
  echo "$unit is up with $(ready_connections "$port") edge connections"
done

#!/usr/bin/env bash
# M6 restart-survival smoke test: a player trades, the world snapshots, the
# server is killed and restarted, and the galaxy is restored from the snapshot
# (the rejoining player's state survives). Requires the dev Postgres cluster
# (scripts/devdb.sh init) and a built server (cargo build -p server).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export DATABASE_URL="${DATABASE_URL:-$(bash "$ROOT/scripts/devdb.sh" url)}"
export PORT=8081 RUST_LOG=info SNAPSHOT_EVERY_TICKS=150   # snapshot every ~5s
BIN="$ROOT/target/debug/server"
WS="ws://127.0.0.1:$PORT/ws"
LOG1=$(mktemp); LOG2=$(mktemp)
fail() { echo "FAIL: $*"; kill "${PID:-0}" 2>/dev/null; exit 1; }

psql "$DATABASE_URL" -c "truncate snapshots, events;" >/dev/null 2>&1

# --- Server #1: join, trade (change state), let it snapshot ---
"$BIN" >"$LOG1" 2>&1 & PID=$!
for i in $(seq 1 40); do curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1 && break; sleep 0.25; done

CREDITS_BEFORE=$(SERVER_WS="$WS" node -e '
const ws=new WebSocket(process.env.SERVER_WS);let v=null;
ws.onopen=()=>ws.send(JSON.stringify({type:"Join",name:"Persist Test"}));
ws.onmessage=(e)=>{const m=JSON.parse(e.data);if(m.type==="View")v=m;};
setTimeout(()=>ws.send(JSON.stringify({type:"MarketBuy",commodity:"fuel",units:200})),1200);
setTimeout(()=>{console.log(Math.round(v.wallet.credits));process.exit(0)},2500);
')
echo "  before restart: credits = $CREDITS_BEFORE (bought fuel; started at 10000)"
[ "$CREDITS_BEFORE" = "10000" ] && fail "trade did not change credits"

echo "  waiting ~7s for a snapshot to capture the post-trade state…"
sleep 7
kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null
echo "  server #1 stopped."

# --- Server #2: restart, should restore from snapshot ---
"$BIN" >"$LOG2" 2>&1 & PID=$!
for i in $(seq 1 40); do curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1 && break; sleep 0.25; done
grep -q "restored world from snapshot" "$LOG2" || fail "server did not restore from snapshot (see log)"
echo "  server #2 restored world from snapshot ✓"

CREDITS_AFTER=$(SERVER_WS="$WS" node -e '
const ws=new WebSocket(process.env.SERVER_WS);let v=null;
ws.onopen=()=>ws.send(JSON.stringify({type:"Join",name:"Persist Test"}));
ws.onmessage=(e)=>{const m=JSON.parse(e.data);if(m.type==="View")v=m;};
setTimeout(()=>{console.log(Math.round(v.wallet.credits));process.exit(0)},1500);
')
echo "  after restart: rejoined corp has credits = $CREDITS_AFTER"
kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null
rm -f "$LOG1" "$LOG2"

[ "$CREDITS_AFTER" = "10000" ] && fail "credits reset to 10000 — state was NOT restored"
DIFF=$(( CREDITS_BEFORE > CREDITS_AFTER ? CREDITS_BEFORE - CREDITS_AFTER : CREDITS_AFTER - CREDITS_BEFORE ))
[ "$DIFF" -le 60 ] || fail "restored credits ($CREDITS_AFTER) differ too much from before ($CREDITS_BEFORE)"
echo ""
echo "PASS — galaxy survived a restart: the rejoining corporation's state was restored from the snapshot."

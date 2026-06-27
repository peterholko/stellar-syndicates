// M1 checkpoint smoke test (no dependencies — uses Node 18+ global WebSocket).
//
// Verifies: TWO clients connect simultaneously, each gets its OWN per-player
// stream (distinct player ids) and a live tick from the authoritative loop, and
// joins/leaves are handled (online count rises to 2, then falls to 1 when one
// client leaves).
//
// Usage: node scripts/m1_smoke.mjs   (assumes server on ws://127.0.0.1:8080/ws,
//        override with SERVER_WS=...)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const STATUS_URL = URL.replace(/^ws/, "http").replace(/\/ws$/, "/status");
const fail = (m) => {
  console.error("FAIL:", m);
  process.exit(1);
};
// Session presence is meta/ops state on the /status endpoint — deliberately NOT
// in the per-player game view (that would leak join/leave faster than light).
const onlinePlayers = async () => (await (await fetch(STATUS_URL)).json()).online_players;

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, ticks: [], lastOnline: 0, errors: [] };
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") got.ticks.push(m.tick);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const main = async () => {
  console.log(`connecting two clients to ${URL}`);
  const a = client("Alpha Freight");
  const b = client("Bravo Mining");

  // Let both join and accumulate ticks.
  await sleep(1500);

  if (!a.got.welcome) fail("client A never received Welcome");
  if (!b.got.welcome) fail("client B never received Welcome");
  if (a.got.errors.length) fail("client A errors: " + a.got.errors.join(", "));
  if (b.got.errors.length) fail("client B errors: " + b.got.errors.join(", "));

  // Distinct per-player identity.
  if (a.got.welcome.player_id === b.got.welcome.player_id)
    fail("both clients got the SAME player id (no per-player identity)");
  console.log(`  A id=${a.got.welcome.player_id}  B id=${b.got.welcome.player_id}  (distinct ✓)`);

  // Live ticks on each independent stream.
  if (a.got.ticks.length < 3) fail(`client A got too few ticks (${a.got.ticks.length})`);
  if (b.got.ticks.length < 3) fail(`client B got too few ticks (${b.got.ticks.length})`);
  const increasing = (arr) => arr.every((v, i) => i === 0 || v > arr[i - 1]);
  if (!increasing(a.got.ticks)) fail("client A ticks not strictly increasing");
  if (!increasing(b.got.ticks)) fail("client B ticks not strictly increasing");
  console.log(
    `  A ticks ${a.got.ticks.at(0)}→${a.got.ticks.at(-1)} (${a.got.ticks.length})  ` +
    `B ticks ${b.got.ticks.at(0)}→${b.got.ticks.at(-1)} (${b.got.ticks.length})  (live ✓)`
  );

  // Session layer registered both (via the /status meta endpoint).
  const online2 = await onlinePlayers();
  if (online2 !== 2) fail(`expected /status online_players=2, got ${online2}`);
  console.log(`  session layer reports 2 online (via /status meta endpoint) ✓`);

  // Leave handling: close A; the server should deregister it.
  a.ws.close();
  await sleep(1000);
  const online1 = await onlinePlayers();
  if (online1 !== 1) fail(`after A left, expected online_players=1, got ${online1}`);
  console.log(`  after A disconnected, session layer reports 1 online ✓`);

  b.ws.close();
  console.log("\nPASS — M1 checkpoint: simultaneous per-player streams, live ticks, join/leave handled.");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

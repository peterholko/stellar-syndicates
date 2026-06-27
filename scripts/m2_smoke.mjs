// M2 checkpoint smoke test (no deps — Node 18+ global WebSocket).
//
// Verifies: the galaxy generates (hub + systems), ships exist and MOVE with
// flip-and-burn, and multiple connected clients see the SAME shared world
// advancing (identical ship sets, identical positions).
//
// Usage: node scripts/m2_smoke.mjs   (server on ws://127.0.0.1:8080/ws,
//        override with SERVER_WS=...)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, views: [], errors: [] };
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") got.views.push(m);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got };
}

const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

const main = async () => {
  console.log(`connecting to ${URL}`);
  const a = client("Alpha Freight");
  await sleep(800);

  // --- Galaxy generated ---
  if (!a.got.welcome) fail("no Welcome");
  const g = a.got.welcome.galaxy;
  if (!g || !Array.isArray(g.systems) || g.systems.length === 0) fail("galaxy has no systems");
  if (typeof g.c !== "number" || typeof g.radius !== "number") fail("galaxy missing c/radius");
  console.log(`  galaxy: ${g.systems.length} systems, radius=${g.radius}, c=${g.c} ✓`);

  // --- Ships exist ---
  await sleep(400);
  const firstView = a.got.views.at(-1);
  if (!firstView || firstView.ships.length < 2) fail(`expected >=2 ships, got ${firstView?.ships.length}`);
  console.log(`  ${firstView.ships.length} ships present (convoy + raider) ✓`);

  // --- Ships MOVE (flip-and-burn) ---
  const startPos = new Map(firstView.ships.map((s) => [s.id, { ...s.pos }]));
  await sleep(6000);
  const laterView = a.got.views.at(-1);
  let maxMove = 0;
  let anyVel = false;
  for (const s of laterView.ships) {
    const p0 = startPos.get(s.id);
    if (p0) maxMove = Math.max(maxMove, dist(p0, s.pos));
    if (Math.hypot(s.vel.x, s.vel.y) > 1) anyVel = true;
  }
  if (maxMove < 50) fail(`ships barely moved (max ${maxMove.toFixed(1)} su in 6s)`);
  if (!anyVel) fail("no ship reported a non-zero velocity");
  console.log(`  ships moved up to ${maxMove.toFixed(0)} su in 6s, non-zero velocities ✓`);

  // --- Shared world: a second client sees the SAME ships ---
  const b = client("Bravo Mining");
  await sleep(1500);
  const av = a.got.views.at(-1);
  const bv = b.got.views.at(-1);
  if (av.ships.length !== 4 || bv.ships.length !== 4)
    fail(`after B joins, expected 4 ships each; A=${av.ships.length} B=${bv.ships.length}`);
  const aIds = new Set(av.ships.map((s) => s.id));
  const bIds = new Set(bv.ships.map((s) => s.id));
  for (const id of aIds) if (!bIds.has(id)) fail(`ship ${id} seen by A but not B (not a shared world)`);
  console.log(`  both clients see the same 4 ships (shared world) ✓`);

  // Positions agree (same tick → identical true positions in M2).
  const aByTick = a.got.views.reduce((m, v) => (m.set(v.tick, v), m), new Map());
  let matched = 0;
  for (const v of b.got.views) {
    const av2 = aByTick.get(v.tick);
    if (!av2) continue;
    for (const s of v.ships) {
      const as = av2.ships.find((x) => x.id === s.id);
      if (as && dist(as.pos, s.pos) > 0.001) fail(`tick ${v.tick} ship ${s.id} differs between clients`);
    }
    matched++;
  }
  if (matched === 0) fail("no overlapping ticks to compare positions");
  console.log(`  positions identical across ${matched} shared ticks ✓`);

  a.ws.close();
  b.ws.close();
  console.log("\nPASS — M2 checkpoint: galaxy generated, ships flip-and-burn, shared world across clients.");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

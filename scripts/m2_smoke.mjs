// M2 checkpoint smoke test (no deps — Node 18+ global WebSocket).
//
// Verifies the galaxy generates (hub + systems) and ships MOVE under
// flip-and-burn. NOTE: as of M3 every player sees a per-player DELAYED view, so
// ships arrive as "ghosts" (not raw true positions) and two clients no longer
// see identical positions — that per-player divergence is the whole point and
// is verified by scripts/m3_smoke.mjs. Here we just confirm the world is alive.
//
// Usage: node scripts/m2_smoke.mjs   (server on ws://127.0.0.1:8080/ws)

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
  await sleep(900);

  // --- Galaxy generated ---
  if (!a.got.welcome) fail("no Welcome");
  const g = a.got.welcome.galaxy;
  if (!g || !Array.isArray(g.systems) || g.systems.length === 0) fail("galaxy has no systems");
  if (typeof g.c !== "number" || typeof g.radius !== "number") fail("galaxy missing c/radius");
  console.log(`  galaxy: ${g.systems.length} systems, radius=${g.radius}, c=${g.c} ✓`);

  // --- Own ships exist (as delayed ghosts) ---
  await sleep(400);
  const firstView = a.got.views.at(-1);
  const own = firstView.ghosts.filter((s) => s.own);
  if (own.length < 2) fail(`expected >=2 own ships, got ${own.length}`);
  console.log(`  ${own.length} own ships present (convoy + raider) ✓`);

  // --- Ships MOVE (flip-and-burn) ---
  const startPos = new Map(own.map((s) => [s.id, { ...s.pos }]));
  await sleep(6000);
  const later = a.got.views.at(-1).ghosts.filter((s) => s.own);
  let maxMove = 0;
  let anyVel = false;
  for (const s of later) {
    const p0 = startPos.get(s.id);
    if (p0) maxMove = Math.max(maxMove, dist(p0, s.pos));
    if (Math.hypot(s.vel.x, s.vel.y) > 1) anyVel = true;
  }
  if (maxMove < 50) fail(`ships barely moved (max ${maxMove.toFixed(1)} su in 6s)`);
  if (!anyVel) fail("no ship reported a non-zero velocity");
  console.log(`  own ships moved up to ${maxMove.toFixed(0)} su in 6s, non-zero velocities ✓`);

  a.ws.close();
  console.log("\nPASS — M2 checkpoint: galaxy generated, ships flip-and-burn. (Per-player divergence: see m3_smoke.)");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

// Two-tier information model smoke test (no deps — Node 18+ global WebSocket).
//
// Verifies, from wire data:
//   * Tier 1 broadcast: all convoys are visible galaxy-wide as light-delayed
//     ghosts (you see a rival convoy even far away);
//   * Tier 2 cargo gating: a convoy's cargo is present IFF it is within the
//     viewer's sensor coverage (your own convoy: yes; a far rival convoy: no);
//   * raiders are DARK: a rival raider outside your sensor coverage is OMITTED
//     entirely from your view (no leak); any rival raider you do see is within
//     your coverage;
//   * convoys carry a broadcast route.
//
// Usage: node scripts/sensor_smoke.mjs   (server on ws://127.0.0.1:8080/ws)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

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

// The viewer's sensor coverage: union of `range` circles around their command
// center + their own ships, at the positions the view reports (same as server).
function coverageCenters(view) {
  const centers = [view.command_center];
  for (const g of view.ghosts) if (g.own) centers.push(g.pos);
  return centers;
}
const inCoverage = (centers, range, p) => centers.some((c) => dist(c, p) <= range);

const main = async () => {
  console.log(`connecting two players to ${URL}`);
  const a = client("Alpha Freight");
  const b = client("Bravo Mining");
  console.log("  waiting ~32s for light to cross the galaxy…");
  await sleep(32000);

  const sr = a.got.welcome.galaxy.sensor_range;
  const av = a.got.views.at(-1);
  const bv = b.got.views.at(-1);
  if (a.got.errors.length || b.got.errors.length) fail("errors: " + [...a.got.errors, ...b.got.errors]);
  console.log(`  sensor_range = ${sr}`);

  const aCenters = coverageCenters(av);

  // Tier 1: a rival convoy is visible galaxy-wide (broadcast).
  const rivalConvoys = av.ghosts.filter((g) => !g.own && g.kind === "convoy");
  if (rivalConvoys.length === 0) fail("A sees no rival convoy — broadcast (Tier 1) failed");
  console.log(`  A sees ${rivalConvoys.length} rival convoy(s) galaxy-wide (broadcast) ✓`);

  // Own convoy reveals cargo (it's its own sensor center).
  const ownConvoy = av.ghosts.find((g) => g.own && g.kind === "convoy");
  if (!ownConvoy || !ownConvoy.cargo) fail("A's own convoy should reveal its cargo");
  console.log(`  A's own convoy cargo known: ${ownConvoy.cargo.commodity} ×${ownConvoy.cargo.units} ✓`);

  // Tier 2 cargo gating: cargo present IFF the convoy is within A's coverage.
  for (const g of av.ghosts) {
    if (g.kind !== "convoy") continue;
    const covered = inCoverage(aCenters, sr, g.pos);
    if (!!g.cargo !== covered)
      fail(`convoy ${g.id}: cargo=${!!g.cargo} but in-coverage=${covered} (cargo not gated by sensor range)`);
  }
  const hiddenCargo = rivalConvoys.filter((g) => !g.cargo).length;
  console.log(`  cargo present IFF in sensor coverage (${hiddenCargo} rival convoy cargo hidden out of range) ✓`);

  // Raiders: every rival raider A sees is within coverage (no out-of-range leak).
  const rivalRaiders = av.ghosts.filter((g) => !g.own && g.kind === "raider");
  for (const g of rivalRaiders) {
    if (!inCoverage(aCenters, sr, g.pos)) fail(`rival raider ${g.id} visible OUTSIDE sensor coverage (LEAK)`);
  }

  // Omission: B's own raider, if clearly outside A's coverage, must be absent
  // from A's view entirely.
  const bRaider = bv.ghosts.find((g) => g.own && g.kind === "raider");
  if (bRaider) {
    const minDist = Math.min(...aCenters.map((c) => dist(c, bRaider.pos)));
    if (minDist > sr * 1.3) {
      if (av.ghosts.some((g) => g.id === bRaider.id))
        fail(`B's dark raider ${bRaider.id} (well outside A's coverage) leaked into A's view`);
      console.log(`  B's raider is dark to A (${Math.round(minDist)} su from A's nearest sensor) — omitted ✓`);
    } else {
      console.log(`  (B's raider is within A's coverage margin this run; omission check skipped)`);
    }
  }
  console.log(`  rival raiders A sees: ${rivalRaiders.length} (all within coverage) ✓`);

  // Route: a convoy broadcasts a route.
  const routed = av.ghosts.find((g) => g.kind === "convoy" && Array.isArray(g.route) && g.route.length > 0);
  if (!routed) fail("no convoy broadcast a route");
  console.log(`  convoys broadcast routes (${routed.route.length} waypoints) ✓`);

  a.ws.close();
  b.ws.close();
  console.log("\nPASS — two-tier model: convoys broadcast galaxy-wide; cargo gated by sensor range; dark raiders omitted out of coverage (no leak).");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

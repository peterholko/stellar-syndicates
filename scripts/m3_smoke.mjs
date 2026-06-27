// M3 checkpoint smoke test (no deps — Node 18+ global WebSocket).
//
// Verifies THE CORE — the lightspeed information model — end to end, from wire
// data alone:
//   * each player gets their OWN command center and a DIFFERENT delayed view;
//   * every ghost's staleness equals its light-distance from that player's
//     command center (age == |pos − cc| / c) — i.e. you only ever see light
//     that has actually arrived (the fairness guarantee, observable on the wire);
//   * own ships are coherent (uncertainty 0); rivals are fogged
//     (uncertainty == age · max_speed > 0);
//   * the two players see the same ship at DIFFERENT ages (no shared/true view).
//
// Usage: node scripts/m3_smoke.mjs   (server on ws://127.0.0.1:8080/ws)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const MAXSPEED = { convoy: 36, raider: 90 };

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
  console.log(`connecting two players to ${URL}`);
  const a = client("Alpha Freight");
  const b = client("Bravo Mining");

  // --- EARLY (before light crosses): presence must NOT leak ---
  await sleep(2500);
  const aEarly = a.got.views.at(-1);
  if (!aEarly) fail("no early view for A");
  // The view must not carry a global player count (that would leak join/leave).
  if ("players_online" in aEarly) fail("View leaked players_online (faster-than-light presence)");
  // A should see exactly ONE owned anchor — its own. The rival's claim light
  // hasn't arrived, so the rival's anchor must appear UNOWNED.
  const aId = a.got.welcome.player_id;
  const owned = aEarly.anchors.filter((an) => an.owner !== null);
  if (owned.length !== 1 || owned[0].owner !== aId)
    fail(`early: A should see only its own anchor owned, saw ${JSON.stringify(owned.map((o) => o.owner))} (rival presence leaked)`);
  console.log(`  before light crosses: no players_online field; rival anchors appear UNOWNED ✓`);

  // Wait long enough that each player's light has crossed the galaxy and
  // reached the other (cross-home light delay is ~25-30s here).
  console.log("  waiting ~32s for light to cross the galaxy between the two homes…");
  await sleep(30000);

  const c = a.got.welcome.galaxy.c;
  const av = a.got.views.at(-1);
  const bv = b.got.views.at(-1);
  if (!av || !bv) fail("missing views");
  if (a.got.errors.length || b.got.errors.length) fail("errors: " + [...a.got.errors, ...b.got.errors]);

  // --- Each player has their OWN command center ---
  if (dist(av.command_center, bv.command_center) < 1)
    fail("both players share a command center — not per-player vantages");
  console.log(`  distinct command centers ✓`);

  // --- The wire never carries true positions or a shared view ---
  if ("ships" in av || "ships" in bv) fail("view leaked raw true-position 'ships' field");

  // --- Fairness invariant: age == |pos − cc| / c for EVERY ghost ---
  // (You only ever see light that has actually arrived; its staleness is exactly
  // its light-distance.)
  let checked = 0;
  for (const [who, v] of [["A", av], ["B", bv]]) {
    for (const g of v.ghosts) {
      const d = dist(g.pos, v.command_center);
      const expectedAge = d / c;
      // age must be >= the light delay (can't see fresher than light allows),
      // and within a small sampling slack of it.
      if (g.age < expectedAge - 1e-6)
        fail(`${who} ghost ${g.id} age ${g.age.toFixed(3)}s < light delay ${expectedAge.toFixed(3)}s (LEAK — saw fresher than light)`);
      if (g.age - expectedAge > 0.4)
        fail(`${who} ghost ${g.id} age ${g.age.toFixed(3)}s far exceeds light delay ${expectedAge.toFixed(3)}s`);
      checked++;
    }
  }
  console.log(`  every ghost's staleness == its light-distance (${checked} ghosts, both players) ✓`);

  // --- Own coherent (uncertainty 0), rivals fogged (uncertainty=age*max_speed) ---
  let ownSeen = 0, rivalSeen = 0;
  for (const g of av.ghosts) {
    if (g.own) {
      if (g.uncertainty !== 0) fail(`own ghost ${g.id} has non-zero uncertainty`);
      ownSeen++;
    } else {
      const expected = g.age * MAXSPEED[g.kind];
      if (Math.abs(g.uncertainty - expected) > 1e-3)
        fail(`rival ghost ${g.id} uncertainty ${g.uncertainty} != age*max_speed ${expected}`);
      if (g.uncertainty <= 0) fail(`rival ghost ${g.id} should be fogged (uncertainty>0)`);
      rivalSeen++;
    }
  }
  if (ownSeen === 0 || rivalSeen === 0) fail(`A should see own (${ownSeen}) and rival (${rivalSeen}) ships`);
  console.log(`  own ships coherent (${ownSeen}), rivals fogged with growing cones (${rivalSeen}) ✓`);

  // --- Two players see the SAME ship at DIFFERENT ages (different vantages) ---
  let differing = 0;
  for (const ga of av.ghosts) {
    const gb = bv.ghosts.find((x) => x.id === ga.id);
    if (gb && Math.abs(ga.age - gb.age) > 0.5) differing++;
  }
  if (differing === 0) fail("no ship is seen at a meaningfully different age by the two players");
  console.log(`  ${differing} ship(s) seen at different staleness by A vs B (no shared truth) ✓`);

  // --- Anchor ownership DOES arrive once light has crossed ---
  const ownedNow = av.anchors.filter((an) => an.owner !== null && an.owner !== a.got.welcome.player_id);
  if (ownedNow.length === 0) fail("after light crossed, A still cannot see the rival's claimed anchor");
  console.log(`  after light crossed, A now sees the rival's anchor owner (gated, not leaked) ✓`);

  a.ws.close();
  b.ws.close();
  console.log("\nPASS — M3 checkpoint: per-player lightspeed views, fairness invariant holds on the wire, no leaks.");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

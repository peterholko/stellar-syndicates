// M4 checkpoint smoke test (no deps — Node 18+ global WebSocket).
//
// Verifies the raiding loop end to end: player A commits a raider to intercept
// player B's convoy; the raid resolves in true space; and BOTH players receive a
// delayed report — each only when the event's light reaches THEIR command center
// (so the report's staleness equals its light-distance, and the two players
// generally learn it on different clocks). Recall-can-miss is covered by the sim
// unit tests (deterministic); here we drive the natural intercept.
//
// Usage: node scripts/m4_smoke.mjs   (server on ws://127.0.0.1:8080/ws)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, views: [], reports: [], errors: [] };
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") got.views.push(m);
    else if (m.type === "Report") got.reports.push(m.report);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got, send: (o) => ws.send(JSON.stringify(o)) };
}

const main = async () => {
  console.log(`connecting two players to ${URL}`);
  const a = client("Alpha Freight");
  const b = client("Bravo Mining");
  await sleep(2500);

  const c = a.got.welcome.galaxy.c;
  const aView = a.got.views.at(-1);
  const bView = b.got.views.at(-1);
  // A's own raider; B's own convoy (read from each player's own coherent view).
  const raider = aView.ghosts.find((g) => g.own && g.kind === "raider");
  const convoy = bView.ghosts.find((g) => g.own && g.kind === "convoy");
  if (!raider) fail("A has no raider");
  if (!convoy) fail("B has no convoy");
  const aCC = aView.command_center;
  const bCC = bView.command_center;

  console.log(`  A commits raider ${raider.id} to intercept B's convoy ${convoy.id}…`);
  a.send({ type: "CommitRaid", raider_id: raider.id, target_id: convoy.id });

  // Wait for BOTH players to receive a report (or timeout). The raider is faster
  // than the convoy, so it intercepts within ~a minute; reports then lag by each
  // player's light-distance to the resolution point.
  const deadline = Date.now() + 110_000;
  while (Date.now() < deadline) {
    if (a.got.reports.length && b.got.reports.length) break;
    await sleep(1000);
  }
  if (a.got.errors.length || b.got.errors.length) fail("errors: " + [...a.got.errors, ...b.got.errors]);
  if (!a.got.reports.length) fail("attacker A never received a report");
  if (!b.got.reports.length) fail("defender B never received a report");

  const ra = a.got.reports[0];
  const rb = b.got.reports[0];
  console.log(`  outcome: ${ra.outcome} · A(attacker) learned it ${ra.age.toFixed(1)}s stale · B(defender) ${rb.age.toFixed(1)}s stale`);

  // Roles correct.
  if (ra.you !== "attacker") fail(`A's report role should be attacker, got ${ra.you}`);
  if (rb.you !== "defender") fail(`B's report role should be defender, got ${rb.you}`);
  // Same raid.
  if (ra.convoy !== rb.convoy || ra.raider !== rb.raider || Math.abs(ra.at_time - rb.at_time) > 1e-6)
    fail("A and B received reports about different raids");
  console.log(`  both learned of the SAME raid (convoy ${ra.convoy}), correct roles ✓`);

  // Report fairness: each report's staleness == its light-distance from THAT
  // player's command center to the resolution point.
  const eA = dist(ra.pos, aCC) / c;
  const eB = dist(rb.pos, bCC) / c;
  if (Math.abs(ra.age - eA) > 0.6) fail(`A report age ${ra.age.toFixed(2)} != light delay ${eA.toFixed(2)}`);
  if (Math.abs(rb.age - eB) > 0.6) fail(`B report age ${rb.age.toFixed(2)} != light delay ${eB.toFixed(2)}`);
  console.log(`  each report's staleness == its light-distance (delayed news on own clocks) ✓`);

  a.ws.close();
  b.ws.close();
  console.log("\nPASS — M4 checkpoint: A raided B's convoy under honest delay; both learned the outcome as delayed news on their own clocks.");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

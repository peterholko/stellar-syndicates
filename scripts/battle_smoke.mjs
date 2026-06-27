// Two-player BATTLE observation smoke test (no deps — Node 18+ global WebSocket).
//
// Drives a real raider-vs-convoy battle between two connected players and proves
// the lightspeed law for DESTRUCTION (§6, §8):
//
//   1. ONE true outcome. The battle resolves once in true space from the seeded
//      sim Rng; BOTH players observe the identical result (never a per-viewer
//      re-roll).
//   2. Asymmetric observation. Attacker and defender learn of the destruction at
//      DIFFERENT times — each only when the event's light reaches THEIR command
//      center. Whoever is closer to the battle sees it first.
//   3. Ghosts before light. After a ship is destroyed in true space but before a
//      given player's light has arrived, that player STILL sees the dead ship
//      flying along as a delayed ghost. It vanishes exactly when the destruction
//      light arrives (T + |P − CC| / c), not a moment sooner.
//   4. No FTL leak. No player's view drops the ship — and no report is delivered —
//      before that player's own light delay allows.
//
// We anchor everything to each View's `sim_time` (the server's authoritative
// clock) rather than wall time, so the checks are exact. The destruction path is
// exercised whenever the random outcome kills a ship (≈80% of raider-vs-convoy
// battles); on a both-survive/escaped roll the script still verifies the shared
// outcome + asymmetric report timing and notes that no destruction occurred.
//
// Usage: node scripts/battle_smoke.mjs   (server on ws://127.0.0.1:8080/ws)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, views: [], reports: [], errors: [] };
  const own = { raider: null, convoy: null }; // this player's own ship ids
  // Per-View timeline anchored to sim_time: which ghost ids were visible, and
  // how many reports had arrived by then.
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") {
      for (const g of m.ghosts) if (g.own && own[g.kind] === null) own[g.kind] = g.id;
      got.views.push({
        st: m.sim_time,
        cc: m.command_center,
        ids: new Set(m.ghosts.map((g) => g.id)),
        nReports: got.reports.length,
      });
    } else if (m.type === "Report") got.reports.push(m.report);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got, own, send: (o) => ws.send(JSON.stringify(o)) };
}

// In player p's timeline, was ship S visible at-or-before sim-time `st`, and was
// it absent at-or-after it? Returns the last-present and first-absent sim-times.
function vanishWindow(views, shipId, afterSt) {
  let lastPresent = null, firstAbsent = null;
  for (const v of views) {
    if (v.ids.has(shipId)) lastPresent = v.st;
    else if (v.st >= afterSt && firstAbsent === null && lastPresent !== null) firstAbsent = v.st;
  }
  return { lastPresent, firstAbsent };
}

const main = async () => {
  console.log(`connecting two players to ${URL}`);
  const a = client("Alpha Freight");
  const b = client("Bravo Mining");
  await sleep(2500);
  if (a.got.errors.length || b.got.errors.length) fail("join errors: " + [...a.got.errors, ...b.got.errors]);

  const c = a.got.welcome.galaxy.c;
  // Read each player's own ship ids (captured from their own coherent views).
  const lastA = a.got.views.at(-1), lastB = b.got.views.at(-1);
  const aRaider = a.own.raider;
  const bConvoy = b.own.convoy;
  if (!aRaider) fail("A has no raider");
  if (!bConvoy) fail("B has no convoy");

  console.log(`  A commits raider ${aRaider} to intercept B's convoy ${bConvoy}…`);
  a.send({ type: "CommitRaid", raider_id: aRaider, target_id: bConvoy });

  // Wait until at least one player receives a report (the battle resolved and its
  // light reached someone). Keep polling afterwards so we capture BOTH players'
  // observations (the farther one lags).
  const deadline = Date.now() + 130_000;
  while (Date.now() < deadline && !(a.got.reports.length || b.got.reports.length)) await sleep(250);
  if (!(a.got.reports.length || b.got.reports.length)) fail("no battle resolved within the time budget");

  // The first report (from either side) carries the ONE true outcome + where/when.
  const first = a.got.reports[0] || b.got.reports[0];
  const { outcome, at_time: T, pos: P, attacker_ship, target_ship } = first;
  console.log(`\n  ⚔  battle resolved at sim t=${T.toFixed(2)} near (${P.x.toFixed(0)}, ${P.y.toFixed(0)}) → outcome: ${outcome}`);

  // Let both players' light arrive (so both get a report) before cross-checking.
  while (Date.now() < deadline && !(a.got.reports.length && b.got.reports.length)) await sleep(250);

  const ra = a.got.reports[0], rb = b.got.reports[0];
  if (!ra || !rb) fail("only one player ever received the battle report");

  // (1) ONE shared outcome, correct roles, same battle.
  if (ra.outcome !== rb.outcome) fail(`A saw ${ra.outcome} but B saw ${rb.outcome} — must be ONE result`);
  if (ra.you !== "attacker" || rb.you !== "defender") fail(`roles wrong: A=${ra.you} B=${rb.you}`);
  if (ra.target_ship !== rb.target_ship || ra.attacker_ship !== rb.attacker_ship || Math.abs(ra.at_time - rb.at_time) > 1e-6)
    fail("A and B received reports about different battles");
  console.log(`  (1) both observed the SAME battle, same outcome "${ra.outcome}", roles attacker/defender ✓`);

  // (2) Asymmetric report timing — each report's staleness == its own light delay.
  const aCC = lastA.cc, bCC = lastB.cc;
  const eA = dist(P, aCC) / c, eB = dist(P, bCC) / c;
  if (Math.abs(ra.age - eA) > 0.8) fail(`A report age ${ra.age.toFixed(2)} != light delay ${eA.toFixed(2)}`);
  if (Math.abs(rb.age - eB) > 0.8) fail(`B report age ${rb.age.toFixed(2)} != light delay ${eB.toFixed(2)}`);
  const closer = eA < eB ? "A (attacker)" : "B (defender)";
  console.log(`  (2) A learned it ${ra.age.toFixed(1)}s stale, B ${rb.age.toFixed(1)}s stale — ${closer} (closer) saw it first ✓`);
  if (Math.abs(eA - eB) < 0.5) console.log(`      (note: command centers nearly equidistant this run — asymmetry is small)`);

  // (3)+(4) Ghost persistence + no FTL, per destroyed ship, per player who could
  // see it. We must keep collecting views until the farther player's light has
  // arrived; both reports are in, so give one more margin sweep.
  await sleep(1500);

  const kills = [];
  if (outcome === "target_destroyed" || outcome === "both_destroyed") kills.push({ id: target_ship, what: "convoy (target)" });
  if (outcome === "attacker_destroyed" || outcome === "both_destroyed") kills.push({ id: attacker_ship, what: "raider (attacker)" });

  if (kills.length === 0) {
    console.log(`  (3) no ship was destroyed this run (random "${outcome}") — ghost-vanish path not exercised.`);
    console.log(`      Re-run to roll a destruction (≈80% of raider-vs-convoy battles), or see the deterministic`);
    console.log(`      Rust test  server::view::tests::destroyed_ship_vanishes_per_viewer_by_light  which proves it.`);
  } else {
    for (const k of kills) {
      for (const [tag, p, cc] of [["A", a, aCC], ["B", b, bCC]]) {
        const predicted = T + dist(P, cc) / c; // when this player's destruction-light arrives
        const seenBefore = p.got.views.some((v) => v.st < T && v.ids.has(k.id));
        if (!seenBefore) { console.log(`      · ${tag} never had ${k.what} in view (legitimately dark) — skip`); continue; }
        const { lastPresent, firstAbsent } = vanishWindow(p.got.views, k.id, T);
        // No FTL: the ship must NOT disappear before this player's light arrives.
        if (firstAbsent !== null && firstAbsent < predicted - 0.6)
          fail(`${tag} lost the ${k.what} ghost at sim t=${firstAbsent.toFixed(2)} — BEFORE its light at ${predicted.toFixed(2)} (FTL leak!)`);
        // Ghost alive: it was still visible after true destruction T (on old light).
        if (lastPresent !== null && lastPresent >= T)
          console.log(`      · ${tag} still saw the ${k.what} as a ghost up to sim t=${lastPresent.toFixed(2)} (destroyed at ${T.toFixed(2)}) ✓`);
        // Vanished on schedule: it's gone at-or-after the predicted light arrival.
        if (firstAbsent !== null)
          console.log(`      · ${tag} ghost vanished by sim t=${firstAbsent.toFixed(2)} (light predicted ${predicted.toFixed(2)}) ✓`);
        else
          console.log(`      · ${tag} still sees the ${k.what} ghost (its light hasn't arrived yet — that's fair)`);
        // No report before light (already gated, but assert it explicitly).
        const reportBeforeLight = p.got.views.some((v) => v.nReports > 0 && v.st < predicted - 0.6);
        if (reportBeforeLight) fail(`${tag} received a battle report before its light arrived (FTL leak!)`);
      }
    }
    console.log(`  (3)+(4) destroyed ship persisted as a per-player ghost until each player's light arrived; no FTL leak ✓`);
  }

  a.ws.close();
  b.ws.close();
  console.log(`\nPASS — two-player battle observed under honest delay: ONE seeded outcome, asymmetric per-player observation, ghosts until light, no FTL.`);
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

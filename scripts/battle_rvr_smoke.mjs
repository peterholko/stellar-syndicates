// Raider-vs-raider BATTLE leak probe (no deps — Node 18+ global WebSocket).
//
// Reproduces the reported bug: when two raiders fight, the destroyed raider is
// supposed to linger as a per-viewer light-delayed ghost (like the convoy case),
// but a raider is only ever shown inside the viewer's sensor coverage — so if the
// destruction collapses that coverage, the dead raider can blink out instantly,
// leaking the result faster than light.
//
// Both players commit their raiders at each other (mutual intercept) so they
// converge and contact quickly. We then watch, per player and anchored to
// sim_time, exactly when each destroyed raider drops out of that player's view,
// and compare against the honest light-arrival time T + |P - CC| / c.
//
// Usage: node scripts/battle_rvr_smoke.mjs   (fresh server on :8080)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, views: [], reports: [], errors: [] };
  const own = { raider: null, convoy: null };
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") {
      for (const g of m.ghosts) if (g.own && own[g.kind] === null) own[g.kind] = g.id;
      got.views.push({ st: m.sim_time, cc: m.command_center, ids: new Set(m.ghosts.map((g) => g.id)) });
    } else if (m.type === "Report") got.reports.push(m.report);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got, own, send: (o) => ws.send(JSON.stringify(o)) };
}

const main = async () => {
  console.log(`connecting two players to ${URL}`);
  const a = client("Alpha Raiders");
  const b = client("Bravo Raiders");
  await sleep(2500);
  if (a.got.errors.length || b.got.errors.length) fail("join errors: " + [...a.got.errors, ...b.got.errors]);

  const c = a.got.welcome.galaxy.c;
  const aRaider = a.own.raider, bRaider = b.own.raider;
  if (!aRaider || !bRaider) fail("both players need a raider");

  console.log(`  mutual intercept: A's raider ${aRaider} ⇄ B's raider ${bRaider}`);
  a.send({ type: "CommitRaid", raider_id: aRaider, target_id: bRaider });
  b.send({ type: "CommitRaid", raider_id: bRaider, target_id: aRaider });

  const deadline = Date.now() + 170_000;
  while (Date.now() < deadline && !(a.got.reports.length || b.got.reports.length)) await sleep(250);
  if (!(a.got.reports.length || b.got.reports.length)) fail("no raider-vs-raider battle resolved in the time budget");

  const first = a.got.reports[0] || b.got.reports[0];
  const { outcome, at_time: T, pos: P, attacker_ship, target_ship } = first;
  console.log(`\n  ⚔  raider battle at sim t=${T.toFixed(2)} near (${P.x.toFixed(0)}, ${P.y.toFixed(0)}) → ${outcome}`);

  // Keep collecting so both players' light arrives and we capture the vanish.
  await sleep(25_000);

  const kills = [];
  if (outcome === "target_destroyed" || outcome === "both_destroyed") kills.push({ id: target_ship, what: "target raider" });
  if (outcome === "attacker_destroyed" || outcome === "both_destroyed") kills.push({ id: attacker_ship, what: "attacker raider" });
  if (kills.length === 0) {
    console.log(`  both raiders survived ("${outcome}") — re-run to roll a destruction (≈88% of RVR battles kill ≥1).`);
    a.ws.close(); b.ws.close(); process.exit(0);
  }

  let leaked = false;
  for (const k of kills) {
    for (const [tag, p] of [["A", a], ["B", b]]) {
      const cc = p.got.views.at(-1).cc;
      const predicted = T + dist(P, cc) / c; // honest light-arrival of the destruction
      const seenBefore = p.got.views.some((v) => v.st < T && v.ids.has(k.id));
      let lastPresent = null, firstAbsent = null;
      for (const v of p.got.views) {
        if (v.ids.has(k.id)) lastPresent = v.st;
        else if (v.st >= T && firstAbsent === null && lastPresent !== null) firstAbsent = v.st;
      }
      const early = firstAbsent !== null ? (predicted - firstAbsent) : null;
      const verdict = firstAbsent === null ? "still ghosting"
        : early > 0.6 ? `⚠ LEAK: vanished ${early.toFixed(1)}s BEFORE its light (predicted ${predicted.toFixed(1)})`
        : `ok (vanished ${firstAbsent.toFixed(1)}, light ${predicted.toFixed(1)})`;
      if (early > 0.6) leaked = true;
      console.log(`  ${tag} · ${k.what} ${k.id}: seenBefore=${seenBefore} lastPresent=${lastPresent} firstAbsent=${firstAbsent} → ${verdict}`);
    }
  }

  a.ws.close(); b.ws.close();
  if (leaked) fail("raider destruction leaked faster than light to at least one viewer");
  console.log(`\nPASS — destroyed raider(s) lingered as per-viewer ghosts until each player's light arrived; no FTL leak.`);
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

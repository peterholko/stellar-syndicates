// Autonomous defensive interception — OFFLINE smoke test (Node 18+ WebSocket).
//
// Proves the core async principle (§5.1, Pillar 1): a player's standing patrol
// defends their convoy WITHOUT the player being present, and the player learns
// what happened as DELAYED NEWS when they next connect.
//
//   1. Defender joins, notes its escort raider + convoy, then GOES OFFLINE.
//   2. Attacker joins and raids the (now-unattended) convoy.
//   3. The server keeps simulating: the defender's escort raider, on its own,
//      detects the inbound hostile and intercepts it — a raider-vs-raider battle
//      (seeded RVR). The attacker (online) observes this via its own report.
//   4. The defender RECONNECTS and receives the delayed report — learning what its
//      patrol did while it was away.
//
// Usage: SERVER_WS=ws://127.0.0.1:8090/ws node scripts/patrol_defense_smoke.mjs

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8090/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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
const isRvr = (r) => r.attacker_kind === "raider" && r.target_kind === "raider";

const main = async () => {
  console.log(`connecting to ${URL}`);
  // 1. Defender joins, records its escort + convoy, then goes offline.
  const d = client("Defender Inc");
  await sleep(3000);
  if (d.got.errors.length) fail("defender join errors: " + d.got.errors);
  const dv = d.got.views.at(-1);
  const escort = dv.ghosts.find((g) => g.own && g.kind === "raider");
  const convoy = dv.ghosts.find((g) => g.own && g.kind === "convoy");
  if (!escort || !convoy) fail("defender has no escort raider / convoy");
  console.log(`  defender online: escort raider ${escort.id} escorting convoy ${convoy.id}`);
  d.ws.close();
  console.log("  → DEFENDER WENT OFFLINE");

  // 2. Attacker joins and raids the unattended convoy.
  const a = client("Raider Co");
  await sleep(3000);
  if (!a.got.welcome) fail("attacker never connected");
  const aRaider = a.got.views.at(-1).ghosts.find((g) => g.own && g.kind === "raider");
  if (!aRaider) fail("attacker has no raider");

  // 3. Keep committing the raid until the attacker observes a raider-vs-raider
  //    battle — i.e. the escort autonomously intervened.
  console.log("  attacker hunting the defender's convoy; waiting for the escort to react…");
  let aRvr = null;
  const deadline = Date.now() + 170_000;
  while (Date.now() < deadline && !aRvr) {
    const v = a.got.views.at(-1);
    const rivalConvoy = v && v.ghosts.find((g) => !g.own && g.kind === "convoy" && g.id === convoy.id);
    const myRaider = v && v.ghosts.find((g) => g.own && g.kind === "raider");
    if (rivalConvoy && myRaider) a.send({ type: "CommitRaid", raider_id: myRaider.id, target_id: rivalConvoy.id });
    aRvr = a.got.reports.find(isRvr);
    await sleep(2000);
  }
  if (!aRvr) fail("the escort never autonomously engaged the attacker (positioning/timing — re-run)");
  console.log(`  ✓ AUTONOMOUS DEFENSE: attacker observed a raider-vs-raider battle — outcome "${aRvr.outcome}", attacker saw it as ${aRvr.you}`);

  // 4. The defender reconnects (same name resumes the corp) and learns the news.
  console.log("  → DEFENDER RECONNECTS…");
  const d2 = client("Defender Inc");
  let dReport = null;
  const dl2 = Date.now() + 90_000;
  while (Date.now() < dl2 && !dReport) {
    dReport = d2.got.reports.find(isRvr);
    await sleep(1500);
  }
  if (!dReport) fail("the offline defender never received the delayed report on reconnect");
  console.log(`  ✓ DELAYED NEWS: defender reconnected and learned its patrol's action — "${dReport.outcome}", role ${dReport.you}, ${dReport.age.toFixed(0)}s old`);
  if (dReport.you !== "attacker") console.log("    (note: the patrol was the interceptor → defender is the 'attacker' of that engagement)");

  a.ws.close();
  d2.ws.close();
  console.log("\nPASS — a standing patrol defended an OFFLINE player's convoy autonomously; the player learned of it as delayed news on reconnect (§5.1, Pillar 1).");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

// Own-ship lightspeed-law check (no deps — Node 18+ global WebSocket).
//
// Proves the corrected law (§6): certainty tracks PROXIMITY to the command
// center, NOT ownership. A player's OWN ship carries honest uncertainty
// `age × max_speed` — never the old `0`. We connect one player, watch their own
// ships patrol (their distance from the command center changes), and assert:
//   * every own ship with age>0 has uncertainty>0 (the FTL tether is gone);
//   * uncertainty / age is a positive CONSTANT per kind (== max_speed), i.e. it
//     scales purely with staleness/distance;
//   * a fresher (nearer) sighting is less uncertain than a staler (farther) one.
//
// Usage: SERVER_WS=ws://127.0.0.1:8090/ws node scripts/own_fog_check.mjs

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const ws = new WebSocket(URL);
const views = [];
ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name: "Fog Probe" })));
ws.addEventListener("message", (ev) => {
  const m = JSON.parse(ev.data);
  if (m.type === "View") views.push(m);
});

const main = async () => {
  console.log(`connecting to ${URL}`);
  // Sample for a while so own ships move to a range of distances from the CC.
  await sleep(20000);
  if (views.length < 3) fail("no views received");

  // Gather every (own ship) sample across all views, by id.
  const byShip = new Map();
  for (const v of views) {
    for (const g of v.ghosts) {
      if (!g.own) continue;
      if (!byShip.has(g.id)) byShip.set(g.id, []);
      byShip.get(g.id).push({ age: g.age, unc: g.uncertainty, kind: g.kind });
    }
  }
  if (byShip.size === 0) fail("player received no OWN ghosts at all");

  let checked = 0;
  for (const [id, samples] of byShip) {
    const moving = samples.filter((s) => s.age > 0.05);
    if (moving.length < 2) continue;
    const kind = moving[0].kind;

    // (1) No FTL tether: own + age>0 ⇒ uncertainty>0.
    const zero = moving.find((s) => s.unc <= 0);
    if (zero) fail(`own ${kind} ${id} had uncertainty 0 at age ${zero.age.toFixed(2)} — the FTL cheat is still present`);

    // (2) uncertainty == age × max_speed ⇒ ratio is a positive constant.
    const ratios = moving.map((s) => s.unc / s.age);
    const min = Math.min(...ratios), max = Math.max(...ratios);
    if (min <= 0) fail(`own ${kind} ${id} non-positive uncertainty/age ratio ${min}`);
    if ((max - min) / max > 0.02) fail(`own ${kind} ${id} uncertainty/age not constant (${min.toFixed(2)}..${max.toFixed(2)}) — not age×max_speed`);

    // (3) Fresher ⇒ less uncertain.
    const fresh = moving.reduce((a, b) => (a.age < b.age ? a : b));
    const stale = moving.reduce((a, b) => (a.age > b.age ? a : b));
    if (!(fresh.unc < stale.unc)) fail(`own ${kind} ${id}: fresher sighting not less uncertain`);

    console.log(`  own ${kind} ${id}: age ${fresh.age.toFixed(1)}–${stale.age.toFixed(1)}s · uncertainty ${fresh.unc.toFixed(0)}–${stale.unc.toFixed(0)} su · max_speed≈${min.toFixed(0)} su/s (constant) ✓`);
    checked++;
  }
  if (checked === 0) fail("no own ship moved enough to range over distances — inconclusive");

  ws.close();
  console.log(`\nPASS — own ships obey the lightspeed law: honest uncertainty = age × max_speed, certainty tracks proximity to the command center, no ownership exemption.`);
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

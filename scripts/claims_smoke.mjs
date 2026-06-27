// System-claims + production smoke test (no deps — Node 18+ global WebSocket).
//
// Verifies the new economic engine end to end, and that it obeys the lightspeed
// law (§4, §6, §9):
//   1. Frontier-richer geology: outer systems out-produce the core (on the wire).
//   2. Claiming charges credits and transfers ownership to the claimer instantly.
//   3. LIGHT-GATED OWNERSHIP: a rival does NOT see the claim until its light
//      arrives (claimed_at + |sys − their_cc|/c), and a rival NEVER sees the
//      owner's stockpile.
//   4. Production accrues over time at the claimed system (owner-only stockpile).
//   5. Shipping production empties the whole-unit stockpile into hub convoys
//      (the full sell payout is covered by the sim unit tests).
//
// Usage: SERVER_WS=ws://127.0.0.1:8090/ws node scripts/claims_smoke.mjs

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
const VALUE = { provisions: 6, ore: 8, fuel: 10, volatiles: 18, alloys: 26 };
const valueRate = (sys) => sys.deposits.reduce((s, d) => s + d.richness * VALUE[d.resource], 0);

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, views: [], trades: [], errors: [] };
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") {
      const sys = new Map(m.systems.map((s) => [s.id, s]));
      got.views.push({ st: m.sim_time, cc: m.command_center, credits: m.wallet.credits, sys });
    } else if (m.type === "Trade") got.trades.push(m.trade);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got, send: (o) => ws.send(JSON.stringify(o)) };
}

const latest = (c) => c.got.views.at(-1);
const sysState = (view, id) => view.sys.get(id);

const main = async () => {
  console.log(`connecting two players to ${URL}`);
  const a = client(" Astra Mining");
  const b = client("Bdolon Holdings");
  await sleep(2500);
  if (a.got.errors.length || b.got.errors.length) fail("join errors: " + [...a.got.errors, ...b.got.errors]);

  const galaxy = a.got.welcome.galaxy;
  const c = galaxy.c;

  // (1) Frontier-richer: outer third out-produces the inner third (on the wire).
  const byDist = [...galaxy.systems].sort((x, y) => Math.hypot(x.pos.x, x.pos.y) - Math.hypot(y.pos.x, y.pos.y));
  const third = Math.floor(byDist.length / 3);
  const mean = (arr) => arr.reduce((s, x) => s + valueRate(x), 0) / arr.length;
  const inner = mean(byDist.slice(0, third));
  const outer = mean(byDist.slice(byDist.length - third));
  if (!galaxy.systems.every((s) => s.deposits.length > 0)) fail("some systems have no deposits");
  if (!(outer > inner * 1.5)) fail(`frontier not richer: inner ${inner.toFixed(1)} vs outer ${outer.toFixed(1)}`);
  console.log(`  (1) frontier richer than core: inner value-rate ${inner.toFixed(1)} vs outer ${outer.toFixed(1)} ✓`);

  // Pick the affordable system FARTHEST from B's command center, to make the
  // light-gating of B's reveal clearly measurable.
  const bcc = latest(b).cc;
  const credits0 = latest(a).credits;
  const affordable = galaxy.systems.filter((s) => s.claim_cost <= credits0);
  if (!affordable.length) fail("no affordable system to claim");
  const target = affordable.reduce((best, s) => (dist(s.pos, bcc) > dist(best.pos, bcc) ? s : best));
  const lightDelayB = dist(target.pos, bcc) / c;
  console.log(`  target ${target.name}: claim_cost ${Math.round(target.claim_cost)} cr · ${lightDelayB.toFixed(1)}s of light from B`);

  // (2) A claims it.
  a.send({ type: "ClaimSystem", system_id: target.id });
  // Wait until A sees it owned by itself.
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline && sysState(latest(a), target.id)?.owner !== a.got.welcome.player_id) await sleep(120);
  const aOwn = sysState(latest(a), target.id);
  if (aOwn?.owner !== a.got.welcome.player_id) fail("A never saw its own claim");
  const tClaim = latest(a).st;
  const charged = credits0 - latest(a).credits;
  if (Math.abs(charged - target.claim_cost) > 1.0) fail(`claim should charge ${target.claim_cost}, charged ${charged.toFixed(0)}`);
  console.log(`  (2) A claimed ${target.name} at sim t=${tClaim.toFixed(1)} — charged ${charged.toFixed(0)} cr, owns it instantly ✓`);

  // (3) LIGHT-GATING: right now (just after A's claim) B must NOT yet see the
  // owner — and must NEVER see the stockpile.
  const bNow = sysState(latest(b), target.id);
  if (bNow?.owner != null) fail(`B saw the claim ${bNow.owner} before its light arrived (FTL leak!)`);
  if (bNow?.stockpile != null) fail("B can see the owner's stockpile (private-data leak!)");
  console.log(`  (3a) immediately after the claim, B still sees ${target.name} as unowned (no FTL leak) ✓`);

  // Wait out B's light delay, then B must see ownership = A (but still no stockpile).
  const waitMs = (lightDelayB + 2.0) * 1000;
  await sleep(Math.min(waitMs, 40_000));
  const bLater = sysState(latest(b), target.id);
  // Find the first B-view where ownership appeared, and check its timing.
  const firstOwned = b.got.views.find((v) => sysState(v, target.id)?.owner === a.got.welcome.player_id);
  if (!firstOwned) fail("B never learned of the claim even after the light delay");
  const observedDelay = firstOwned.st - tClaim;
  if (observedDelay < lightDelayB - 1.0) fail(`B learned the claim ${observedDelay.toFixed(1)}s after it — faster than its ${lightDelayB.toFixed(1)}s light!`);
  if (bLater?.stockpile != null) fail("B can see the owner's stockpile after learning ownership (leak!)");
  console.log(`  (3b) B learned the claim ${observedDelay.toFixed(1)}s later (light delay ${lightDelayB.toFixed(1)}s), never its stockpile ✓`);

  // (4) Production accrues over time (owner-only stockpile grows).
  const stockOf = (v) => (sysState(v, target.id)?.stockpile ?? []).reduce((s, x) => s + x.units, 0);
  const s1 = stockOf(latest(a));
  await sleep(6000);
  const s2 = stockOf(latest(a));
  if (!(s2 > s1)) fail(`production did not accrue: ${s1} → ${s2}`);
  console.log(`  (4) ${target.name} accrued production: ${s1} → ${s2} units (owner-only) ✓`);

  // (5) Ship production: the whole-unit stockpile empties into hub convoy(s).
  const before = stockOf(latest(a));
  a.send({ type: "ShipProduction", system_id: target.id });
  await sleep(1500);
  const after = stockOf(latest(a));
  if (!(after < before)) fail(`shipping did not empty the stockpile: ${before} → ${after}`);
  const dispatched = a.got.trades.some((t) => t.event === "SellDispatched");
  if (!dispatched) fail("no SellDispatched trade event after shipping production");
  console.log(`  (5) shipped production: stockpile ${before} → ${after}, sell convoy(s) dispatched to the hub ✓`);

  a.ws.close();
  b.ws.close();
  console.log(`\nPASS — claim → produce → ship loop works; ownership is light-gated to rivals and stockpiles stay private (§4, §6, §9).`);
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

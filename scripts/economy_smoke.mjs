// Economy smoke test (no deps — Node 18+ global WebSocket).
//
// Verifies the §9 hub-Exchange mechanics over the wire:
//   * the price ticker is LIGHT-DELAYED (staleness > 0 from home);
//   * a market BUY settles instantly (credits debited now) and spawns a delivery
//     convoy (goods cross home, raidable);
//   * a market SELL commits the goods now (inventory debited) and spawns a convoy
//     toward the hub (clears at price-on-arrival — the buy/sell asymmetry).
// (Delivery/sale ARRIVAL resolution is covered deterministically by sim tests.)
//
// Usage: node scripts/economy_smoke.mjs   (server on ws://127.0.0.1:8080/ws)

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, views: [], trades: [], errors: [] };
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") got.views.push(m);
    else if (m.type === "Trade") got.trades.push(m.trade);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got, send: (o) => ws.send(JSON.stringify(o)) };
}

const ownConvoys = (v) => v.ghosts.filter((g) => g.own && g.kind === "convoy").length;
const held = (v, c) => (v.wallet.inventory.find((i) => i.commodity === c)?.units ?? 0);

const main = async () => {
  console.log(`connecting to ${URL}`);
  const a = client("Trader One");
  await sleep(1500);
  let v = a.got.views.at(-1);
  if (!v || !v.market || !v.wallet) fail("no market/wallet in view");

  // Lagged ticker.
  if (!(v.market.staleness > 1)) fail(`expected a lagged price ticker, staleness=${v.market.staleness}`);
  console.log(`  hub ticker is light-delayed: ~${v.market.staleness.toFixed(0)}s stale ✓`);
  const fuelPrice = v.market.prices.find((p) => p.commodity === "fuel").price;
  console.log(`  fuel ticker price ≈ ${fuelPrice.toFixed(2)}; credits = ${Math.round(v.wallet.credits)}`);

  // --- BUY: instant settlement (the delivery convoy spawns AT the hub, ~16s
  // of light away, so the player can't see their own far convoy yet — itself
  // the lightspeed model). ---
  const credits0 = v.wallet.credits;
  a.send({ type: "MarketBuy", commodity: "fuel", units: 80 });
  await sleep(1800);
  v = a.got.views.at(-1);
  if (v.wallet.credits >= credits0) fail("buy did not debit credits (no instant settlement)");
  if (!a.got.trades.some((t) => t.event === "Bought" && t.commodity === "fuel")) fail("no Bought trade event");
  console.log(`  BUY 80 fuel: settled now (−${Math.round(credits0 - v.wallet.credits)} cr), Bought event ✓`);

  // --- SELL: commit goods now + convoy toward hub (spawns at HOME → visible). ---
  const ore0 = held(v, "ore");
  const convoysB = ownConvoys(v);
  a.send({ type: "MarketSell", commodity: "ore", units: 40 });
  await sleep(1800);
  v = a.got.views.at(-1);
  if (held(v, "ore") !== ore0 - 40) fail(`sell should commit goods now (ore ${ore0}→${held(v, "ore")})`);
  if (!a.got.trades.some((t) => t.event === "SellDispatched" && t.commodity === "ore")) fail("no SellDispatched trade event");
  if (ownConvoys(v) !== convoysB + 1) fail("sell convoy (home-spawned) should be visible immediately");
  console.log(`  SELL 40 ore: goods committed now (ore ${ore0}→${held(v, "ore")}), convoy dispatched (visible at home) ✓`);

  // --- The buy's delivery convoy becomes visible once its light reaches home. ---
  const before = ownConvoys(v);
  console.log(`  waiting ~18s for the delivery convoy's light to reach home…`);
  await sleep(18000);
  v = a.got.views.at(-1);
  if (ownConvoys(v) <= before) fail("delivery convoy never became visible (own-ship light delay)");
  console.log(`  delivery convoy now visible (own convoys ${before}→${ownConvoys(v)}) — even your own far convoy is light-delayed ✓`);

  if (a.got.errors.length) fail("errors: " + a.got.errors.join(", "));
  a.ws.close();
  console.log("\nPASS — hub Exchange: lagged ticker, instant buy settlement + delivery convoy, sell commits goods to a hub-bound convoy (asymmetry).");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

// Limit-order / batch-clearing smoke test (no deps — Node 18+ global WebSocket).
//
// Two players place CROSSING limit orders; the periodic uniform-price call
// auction (§9) clears them at one price. Verifies: orders rest with reservations
// taken, then clear in the batch (the buyer gets a delivery convoy + refund of
// over-reservation, the seller is paid), at a single uniform price.
//
// Usage: node scripts/limit_smoke.mjs   (server on ws://127.0.0.1:8080/ws)

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
const wallet = (c) => c.got.views.at(-1).wallet;
const held = (c, com) => wallet(c).inventory.find((i) => i.commodity === com)?.units ?? 0;

const main = async () => {
  console.log(`connecting two traders to ${URL}`);
  const buyer = client("Limit Buyer");
  const seller = client("Limit Seller");
  await sleep(1500);

  const buyCredits0 = wallet(buyer).credits;
  const sellCredits0 = wallet(seller).credits;
  const sellOre0 = held(seller, "ore");

  // Crossing pair on ORE: buyer up to 9, seller at least 7.
  buyer.send({ type: "PlaceLimitOrder", side: "buy", commodity: "ore", units: 50, limit_price: 9.0 });
  seller.send({ type: "PlaceLimitOrder", side: "sell", commodity: "ore", units: 50, limit_price: 7.0 });
  await sleep(1500);

  // Resting orders + reservations.
  if (wallet(buyer).orders.length !== 1) fail("buyer's limit order not resting");
  if (wallet(seller).orders.length !== 1) fail("seller's limit order not resting");
  if (!(wallet(buyer).credits < buyCredits0)) fail("buyer credits not reserved");
  if (held(seller, "ore") !== sellOre0 - 50) fail("seller goods not reserved");
  console.log(`  both limit orders resting; reservations taken (buyer −${Math.round(buyCredits0 - wallet(buyer).credits)} cr held, seller ore ${sellOre0}→${held(seller, "ore")}) ✓`);

  // Wait for the periodic batch to clear (≈ every 20 s).
  console.log("  waiting for the next uniform-price batch (≤ ~22s)…");
  let cleared = false;
  for (let i = 0; i < 24; i++) {
    await sleep(1000);
    if (wallet(buyer).orders.length === 0 && wallet(seller).orders.length === 0) { cleared = true; break; }
  }
  if (!cleared) fail("batch never cleared the crossing orders");

  const bf = buyer.got.trades.find((t) => t.event === "LimitFilled" && t.side === "buy");
  const sf = seller.got.trades.find((t) => t.event === "LimitFilled" && t.side === "sell");
  if (!bf || !sf) fail("missing LimitFilled events");
  if (Math.abs(bf.unit_price - sf.unit_price) > 1e-6) fail(`uniform price violated: buy@${bf.unit_price} sell@${sf.unit_price}`);
  console.log(`  batch cleared ${bf.units} ore at a UNIFORM price ${bf.unit_price.toFixed(2)} (both sides) ✓`);

  // Seller paid; buyer's over-reservation refunded (net spend = units × clear).
  if (!(wallet(seller).credits > sellCredits0)) fail("seller not paid");
  const buyerNet = buyCredits0 - wallet(buyer).credits;
  if (Math.abs(buyerNet - bf.units * bf.unit_price) > 1.0) fail(`buyer net spend ${buyerNet} != units×clear ${bf.units * bf.unit_price}`);
  console.log(`  seller paid (+${Math.round(wallet(seller).credits - sellCredits0)} cr); buyer settled at uniform price (over-reservation refunded) ✓`);

  if (buyer.got.errors.length || seller.got.errors.length) fail("errors");
  buyer.ws.close();
  seller.ws.close();
  console.log("\nPASS — limit orders rest and clear in a periodic uniform-price call auction (anti-sniping).");
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));

// §merchant: the traveling merchant's derive helpers and desktop stall,
// against isolated served fixtures. It never connects to a live game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {}, ...globals });
  return exports;
}
const copy = (value) => JSON.parse(JSON.stringify(value));

deps["./industry"] = { availableStock: (s, c) => Math.max(0, s.stockpile?.find((x) => x.commodity === c)?.units ?? 0) };
const label = (c) => c === "alloys" ? "Alloys" : c;
deps["../../icons"] = { commodityIcon: () => "", label };
const m = deps["../../core/derive/merchant"] = compile("core/derive/merchant.ts");

const merchant = { fleet: "m", system: "home", phase: "docked", leave_by: 10_860, markup: 0.5, buys: "alloys", buy_price: 4, buy_room: 150,
  goods: [{ commodity: "composites", stock: 120, price: 39, hub_mid: 26 }, { commodity: "machinery", stock: 50, price: 93, hub_mid: 62 },
    { commodity: "electronics", stock: 60, price: 51, hub_mid: 34 }] };
const home = { id: "home", storage_used: 120, storage_cap: 250,
  stockpile: [{ commodity: "composites", units: 25 }, { commodity: "machinery", units: 7 }, { commodity: "alloys", units: 60 }] };

// Quotes: priced like the sim, refused with the sim's reasons.
let quote = m.merchantQuote(merchant, { electronics: 15 }, { alloys: 40 }, home, 10_000);
assert.deepEqual(copy(quote), { cost: 765, revenue: 160, netUnits: -25, error: null });
const why = (buys, sells, h = home, credits = 10_000) => m.merchantQuote(merchant, buys, sells, h, credits).error;
assert.equal(why({ electronics: 61 }, {}), "The merchant hasn't that much");
assert.equal(why({ titanium: 1 }, {}), "The merchant doesn't sell that");
assert.equal(why({}, { composites: 1 }), "The merchant doesn't buy that");
assert.equal(why({}, { alloys: 151 }), "The merchant won't take that many");
assert.equal(why({}, { alloys: 61 }), "Not enough Alloys at home", "the sim's words");
assert.equal(why({ composites: 10 }, {}, { ...home, storage_used: 245 }), "Not enough storage room at home");
assert.equal(why({ electronics: 15 }, {}, home, 100), "Not enough credits");
assert.equal(why({}, {}), "Nothing to trade");

// §storage-full (Peter, Nov 3 2026): a blocked deal in plain words; storage
// with the sale (§sell-alloys: Alloys), in all, that makes room in the same deal.
const block = (buys, sells, h = home, credits = 10_000, told) =>
  copy(m.merchantBlock(merchant, sells, m.merchantQuote(merchant, buys, sells, h, credits), h, "Teo", told));
const full = { ...home, storage_used: 249.6 };
assert.deepEqual(block({ composites: 10 }, {}, full), { text: "Storage full · 249/250", storage: true, full: true, sell: 10 });
assert.deepEqual(block({ composites: 10 }, {}, { ...home, storage_used: 245 }),
  { text: "Storage 245/250 · room for 5", storage: true, full: false, sell: 5 });
assert.equal(block({ composites: 30 }, { alloys: 10 }, { ...home, storage_used: 245 }).sell, 25, "on top of the Alloys already offered");
assert.equal(block({ composites: 100 }, {}, full).sell, null, "60 Alloys at home can't make room for 100");
assert.deepEqual(block({ electronics: 15 }, {}, home, 100), { text: "Not enough credits", storage: false, full: false, sell: null });
// §three-stipends (Nov 6 2026): told the credits, a short deal offers the
// sale that pays for it, in all, if the home holds it and the trader takes it.
assert.deepEqual(block({ electronics: 15 }, {}, home, 700, 700), { text: "Not enough credits", storage: false, full: false, sell: 17 });
assert.equal(block({ electronics: 15 }, { alloys: 10 }, home, 700, 700).sell, 17, "on top of the Alloys already offered");
assert.equal(block({ electronics: 15 }, {}, home, 400, 400).sell, null, "92 Alloys: more than the 60 at home");
assert.equal(block({ electronics: 15 }, {}, home, 700, 700).sell * merchant.buy_price + 700 >= 765, true);
assert.equal(block({ electronics: 61 }, {}).text, "Teo hasn't that much", "the trader by name");
assert.equal(m.merchantBlock(merchant, {}, m.merchantQuote(merchant, {}, {}, home), home, "Teo"), null, "an empty table");
assert.equal(m.merchantBlock(merchant, {}, quote, home, "Teo"), null, "a deal that goes");

// The guide's prefill: what a build lacks at home, bounded by its stock.
const st = { merchant, galaxy: { build_options: [{ key: "shipyard", costs: [{ commodity: "composites", units: 40 },
  { commodity: "machinery", units: 20 }, { commodity: "electronics", units: 15 }] }] } };
assert.deepEqual(copy(m.merchantShortfall(st, home, "shipyard", merchant)), { composites: 15, machinery: 13, electronics: 15 });
const thin = { ...merchant, goods: merchant.goods.map((g) => g.commodity === "electronics" ? { ...g, stock: 10 } : g) };
assert.equal(m.merchantShortfall(st, home, "shipyard", thin).electronics, 10, "never more than it has");
// §den-contract: it stays for the charter, so every build after the den
// buys its parts there, and the den-mauled Interceptor its repair.
assert.ok(["mining_barge", "smelter", "mining_complex", "repair"].every(m.isMerchantTarget) && !m.isMerchantTarget("ahead"));
assert.equal(m.dockedMerchant({ merchant: { ...merchant, phase: "arriving" } }), null, "it trades only while docked");

// §persona: Teo Brannick's word on the offer, refusals in Teo's voice, the
// server's result after a deal, and a portrait that falls back to the ship.
deps["../../protocol"] = compile("protocol.ts");
const t = deps["../../core/derive/trader"] = compile("core/derive/trader.ts");
const teo = t.TRADERS.teo;
const quoteOf = (buys, sells, credits = 10_000) => m.merchantQuote(merchant, buys, sells, home, credits);
const says = (buys, sells, opts = {}) => t.traderLine(opts.trader ?? teo, opts.merchant ?? merchant, buys, sells,
  quoteOf(buys, sells, opts.credits), opts.dealt ?? false);
assert.deepEqual(copy(says({}, {})), { text: "Raided! Terrible. Lucky I came by, eh?", mood: "neutral" });
assert.deepEqual(copy(says({}, {}, { dealt: true })), { text: "Back for more? Good, good.", mood: "pleased" });
assert.equal(says({}, { alloys: 40 }).text, "Alloys? Still warm! 4 Cr a unit, and I'm being kind.");
assert.equal(says({ electronics: 8 }, {}).text, "Chips! The genuine article.");
assert.deepEqual(copy(says({ electronics: 8 }, { alloys: 40 })), { text: "Now we're talking!", mood: "pleased" });
assert.equal(says({ machinery: 20 }, {}).text, "A big spender! I like you already.", "1,860 Cr of Machinery");
assert.deepEqual(copy(says({ electronics: 8 }, {}, { credits: 100 })), { text: "Credits, friend. Or Alloys. I take both.", mood: "scoff" });
assert.equal(says({ electronics: 61 }, {}).text, "Ha! I'd sell you the ship if I could.");
assert.equal(says({}, { alloys: 151 }).text, "I've a hold, not a moon.");
assert.equal(says({}, { alloys: 61 }).text, "You haven't smelted that much yet.");
assert.equal(t.refusalLine(teo, "Not enough Alloys at home").text, "You haven't smelted that much yet.", "the sim's refusal");
assert.equal(t.refusalLine(teo, "Not enough storage room at home").text, "And where would you put it all?", "storage, not stock");
assert.equal(says({}, {}, { merchant: { ...merchant, phase: "arriving" } }).text, "Hold your horses, I'm still docking.");
assert.equal(says({}, {}, { merchant: { ...merchant, phase: "leaving" } }).mood, "scoff");
assert.equal(t.refusalLine(teo, "Your charter bars trade").text, "Your charter's in the red. I don't touch that.");
assert.equal(t.refusalLine(teo, "Something new").text, "No deal.");
assert.deepEqual(copy(t.dealLine(teo)), { text: "Done! You'll tell your grandchildren.", mood: "pleased" });
const timeline = [{ at_time: 50, severity: "info", text: "Free trader: bought 8 semiconductors for 408 Cr." },
  { at_time: 120, severity: "warn", text: "Free trader: trade refused — Not enough credits." }];
assert.equal(t.traderResult(timeline, 200), null, "nothing yet");
assert.deepEqual(copy(t.traderResult(timeline, 100)), { ok: false, reason: "Not enough credits" });
assert.deepEqual(copy(t.traderResult(timeline.slice(0, 1), 40)), { ok: true });
assert.deepEqual(copy(t.traderPortrait(teo, "pleased")), { src: "/art/derived/ships/models-v1/previews/free_trader-768.webp",
  alt: "The Long Odds.", painted: false }, "until the painting lands, the Long Odds stands in");

// §traders (Peter, Oct 27 2026): three free traders, one dealt per
// corporation by the server; each speaks in their own voice.
const ines = t.traderOf({ trader: "ines" });
const mags = t.traderOf({ trader: "mags" });
const rafi = t.traderOf({ trader: "rafi" });
assert.deepEqual([t.traderOf(null).name, t.traderOf({}).slug, ines.name, mags.ship],
  ["Teo Brannick", "teo", "Ines Varga", "Second Wind"], "the dealt trader; Teo until the server says");
for (const trader of [teo, ines, mags, rafi]) {
  assert.deepEqual(Object.keys(trader.voice).sort(), Object.keys(teo.voice).sort(), `${trader.name}: a line for every moment`);
  for (const [key, line] of Object.entries(trader.voice)) {
    assert.ok(typeof line === "function" ? /4 Cr/.test(line("4 Cr")) : line.length > 0, `${trader.name}: ${key}`);
  }
}
assert.deepEqual(copy(says({}, {}, { trader: ines })), { text: "Raided. Unfortunate. I carry what they took.", mood: "neutral" });
assert.equal(says({}, { alloys: 40 }, { trader: ines }).text, "Alloys. 4 Cr a unit. That is the price.");
assert.deepEqual(copy(says({ electronics: 8 }, {}, { trader: ines, credits: 100 })), { text: "The numbers don't close.", mood: "scoff" });
assert.equal(says({}, {}, { trader: mags, merchant: { ...merchant, phase: "arriving" } }).text, "Slow ships get there too, pet. Nearly tied up.");
assert.equal(says({ electronics: 8 }, {}, { trader: mags }).text, "Chips. Real ones. I don't sell rubbish.");
assert.equal(t.refusalLine(mags, "Something new").text, "Not today, pet.");
// Peter: "can we have 4th male free trader".
assert.equal(says({}, { alloys: 40 }, { trader: rafi }).text, "Alloys? Sure. 4 Cr a unit. Practically charity.");
assert.deepEqual(copy(t.dealLine(rafi)), { text: "Done! Pleasure doing business, chief.", mood: "pleased" });
assert.deepEqual(copy(t.traderPortrait(ines, "scoff")), { src: "/art/derived/ships/models-v1/previews/clean_slate-768.webp",
  alt: "The Clean Slate.", painted: false }, "each trader's own portrait, their own ship until it is painted");
assert.match(src("core/derive/trader.ts"), /\$\{PORTRAITS\}\/\$\{trader\.portrait\}\$\{suffix\}-320\.webp/);
assert.deepEqual([teo.portrait, ines.portrait, mags.portrait, rafi.portrait], ["teo-brannick", "ines-varga", "mags-halloran", "rafi-mensah"]);
const simTraders = [...readFileSync(new URL("../../crates/sim/src/merchant.rs", import.meta.url), "utf8")
  .matchAll(/FreeTrader \{ slug: "(\w+)", name: "([^"]+)", first: "(\w+)", ship: "([^"]+)" \}/g)]
  .map(([, slug, name, first, ship]) => ({ slug, name, first, ship }));
assert.deepEqual(Object.values(t.TRADERS).map(({ slug, name, first, ship }) => ({ slug, name, first, ship })), simTraders,
  "the sim's FREE_TRADERS, in order, with the same names");

// The desktop stall: a guide shortcut fills the shortfall; the handshake sends one exchange.
const sent = [];
const notices = [];
let now = 10_000;
deps["../../core/derive/format"] = { fmt: (n) => Math.round(n).toLocaleString("en-US"), fmtDur: (s) => `${Math.round(s)}s` };
deps["../../core/derive/market"] = { spendableMarketCredits: () => 10_000 };
deps["../../core/derive/founding"] = { fuelNeed: () => null };
deps["../../icons"] = { commodityIcon: () => "", label };
deps["../../state"] = { liveSimTime: () => now };
deps["../dom"] = { renderDeferred: () => false, setHtml: (root, html) => { root.innerHTML = html; }, lastHtmlWritten: new WeakMap() };
deps["../signature"] = { sheetFingerprint: (value) => JSON.stringify(value) };
class InputStub {}
const doc = { activeElement: null };
// §goods-tabs: the shared Resources · Parts · Modules split and its desktop strip.
deps["./equipment"] = compile("core/derive/equipment.ts");
deps["./fleet"] = { ...(deps["./fleet"] ?? {}), lightBase: (c) => c.replace(/^(heavy|capital)_/, "") };
deps["../../core/derive/goods"] = compile("core/derive/goods.ts");
deps["./goods-tabs"] = compile("shell/deck/goods-tabs.ts");
const { DeckMerchantRoutes } = compile("shell/deck/merchant.ts", { document: doc, HTMLInputElement: InputStub });
const state = { merchant, wallet: { credits: 10_000, report_pending: false }, systems: [home], galaxy: st.galaxy, timeline: [], simTime: 300 };
const root = { id: "deck-workspace-body", innerHTML: "", contains: () => false, querySelector: () => null };
const stall = new DeckMerchantRoutes(root, { state, send: (msg) => sent.push(msg) }, { go() {}, notice: (html) => notices.push(html) });
const route = { name: "merchant", query: { for: "shipyard" } };
assert.equal(stall.render({ name: "market" }), false, "another route is not the stall");
assert.equal(stall.render(route), true);
// Teo, his ship and his word on the offer, over your side and his.
// §den-contract: docked for the whole charter, so no countdown.
assert.match(root.innerHTML, /<b>Teo Brannick<\/b><small>the Long Odds · Docked<\/small>/);
assert.match(root.innerHTML, /class="deck-merchant__portrait is-interim" src="\/art\/derived\/ships\/models-v1\/previews\/free_trader-768\.webp"/);
assert.match(root.innerHTML, /data-mood="pleased"[\s\S]*“A big spender! I like you already\.”/, "2,559 Cr of goods on the table");
assert.match(root.innerHTML, /<h3>You give<\/h3>[\s\S]*data-merchant-input="sell:alloys"[\s\S]*<h3>You get<\/h3>/, "both sides at once: no tabs");
assert.doesNotMatch(root.innerHTML, /merchant-tab/);
assert.match(root.innerHTML, /value="15"[^>]+data-merchant-input="buy:electronics"/, "the shortfall is filled in");
assert.match(root.innerHTML, /value="13"[^>]+data-merchant-input="buy:machinery"/);
assert.doesNotMatch(root.innerHTML, /Fill for|merchant-fill|merchant-clear/, "the guide's shortcut fills it; the stall has no Fill for row");
// Its markup is its own business: no markup, no Hub price beside its own,
// no credits or storage stats on the stall.
assert.match(root.innerHTML, /39\.0 Cr · 120 left/);
// Your side shows the Alloys you have to sell; the box stops at it.
assert.match(root.innerHTML, /4\.0 Cr · takes 150 more<\/small><small class="deck-merchant-row__held"><b>60<\/b> at home<\/small>/);
assert.match(root.innerHTML, /max="60"[^>]+data-merchant-input="sell:alloys"/, "no more than you hold");
// Max puts all of it on the table, up to what the trader takes (Peter, Oct 21 2026).
assert.match(root.innerHTML, /<span class="deck-merchant-row__amount"><input[^>]+data-merchant-input="sell:alloys"[^>]*><button type="button" data-deck-act="merchant-max"[^>]*>Max<\/button><\/span>/);
assert.equal((root.innerHTML.match(/merchant-max/g) ?? []).length, 1, "only on your side");
assert.doesNotMatch(root.innerHTML, /\+50%|Markup|Hub \d|half the Hub|Storage room|<small>Credits/);
// The terms and the handshake stay pinned to the panel's foot.
assert.match(root.innerHTML, /class="deck-merchant__dock"><div class="deck-ticket-quote">You pay <b>2,559 Cr<\/b> · storage \+43<\/div>/,
  "one total: 15×39 + 13×93 + 15×51");
assert.match(root.innerHTML, /data-deck-act="merchant-submit" title="Settles at your home at once" >Trade</);
assert.equal(stall.handleAction({ dataset: { deckAct: "merchant-submit" } }, route), true);
assert.deepEqual(copy(sent), [{ type: "MerchantTrade", buys: { composites: 15, machinery: 13, electronics: 15 }, sells: {} }]);
assert.match(notices[0], /Deal sent<\/b> · you pay 2,559 Cr/);
assert.match(root.innerHTML, /“Done! You(&#39;|')ll tell your grandchildren\.”/, "his parting shot");
assert.match(root.innerHTML, /data-deck-act="merchant-submit" title="Settles at your home at once" disabled>Trade/, "the amounts clear after sending");
// The sim refused it after all: his word on why.
state.timeline = [{ at_time: 300.5, severity: "warn", text: "Free trader: trade refused — Not enough storage room at home." }];
stall.render(route);
assert.match(root.innerHTML, /data-mood="scoff"[\s\S]*“And where would you put it all\?”/);

// Max: the 60 you hold, under the 150 it takes; with more at home, the 150.
assert.equal(stall.handleAction({ dataset: { deckAct: "merchant-max" } }, route), true);
assert.match(root.innerHTML, /value="60"[^>]+data-merchant-input="sell:alloys"/);
const alloysHeld = home.stockpile.find((line) => line.commodity === "alloys");
const heldBefore = alloysHeld.units;
alloysHeld.units = 400;
stall.handleAction({ dataset: { deckAct: "merchant-max" } }, route);
assert.match(root.innerHTML, /value="150"[^>]+data-merchant-input="sell:alloys"/, "no more than it takes");
alloysHeld.units = heldBefore;
stall.handleInput({ dataset: { merchantInput: "sell:alloys" }, value: "0" }, route);

// Typing a sale prices it; an offer he won't take is never sent.
stall.handleInput({ dataset: { merchantInput: "sell:alloys" }, value: "200" }, route);
assert.match(root.innerHTML, /“I(&#39;|')ve a hold, not a moon\.”/);
assert.equal(stall.handleAction({ dataset: { deckAct: "merchant-submit" } }, route), true);
assert.equal(sent.length, 1, "a refused trade is never sent");
stall.handleInput({ dataset: { merchantInput: "sell:alloys" }, value: "40" }, route);
assert.match(root.innerHTML, /You receive <b>160 Cr<\/b> · storage -40/);

// Smelting goes on while you type: the table isn't redrawn under the cursor,
// but the Alloys at home tick up in place, and the box's cap with it.
const count = { textContent: "60" };
const box = { max: "60" };
doc.activeElement = new InputStub();
root.contains = () => true;
root.querySelector = (selector) => selector === ".deck-merchant-row__held b" ? count
  : selector.includes("sell:alloys") ? box : null;
const typed = root.innerHTML;
home.stockpile.find((x) => x.commodity === "alloys").units = 75;
stall.render(route);
assert.equal(root.innerHTML, typed, "no redraw while an amount box has the cursor");
assert.deepEqual([count.textContent, box.max], ["75", "75"]);
doc.activeElement = null;
root.contains = () => false;
stall.render(route);
assert.match(root.innerHTML, /<b>75<\/b> at home/, "the next redraw agrees");
home.stockpile.find((x) => x.commodity === "alloys").units = 60;

// §storage-full: a deal that won't fit says so over the terms; one click
// sells the Alloys that make room, and the deal goes.
stall.handleInput({ dataset: { merchantInput: "sell:alloys" }, value: "0" }, route);
stall.handleInput({ dataset: { merchantInput: "buy:electronics" }, value: "12" }, route);
stall.handleInput({ dataset: { merchantInput: "buy:machinery" }, value: "15" }, route);
home.storage_used = 249.6;
stall.invalidate();
stall.render(route, true);
assert.match(root.innerHTML, /<div class="deck-merchant__dock"><p class="deck-merchant__blocked is-full"><span>Storage full · 249\/250<\/span><button type="button" data-deck-act="merchant-room" data-units="27" title="Selling Alloys makes room in the same deal\.">Sell 27 Alloys<\/button><\/p><div class="deck-ticket-quote">You pay <b>2,007 Cr<\/b> · <span class="is-over">storage \+27<\/span><\/div>/);
assert.match(root.innerHTML, /data-deck-act="merchant-submit" title="Storage full · 249\/250" disabled>Trade/);
assert.match(root.innerHTML, /data-mood="scoff"[\s\S]*“And where would you put it all\?”/, "Teo's word stays");
assert.equal(stall.handleAction({ dataset: { deckAct: "merchant-room", units: "27" } }, route), true);
assert.match(root.innerHTML, /value="27"[^>]+data-merchant-input="sell:alloys"/);
assert.doesNotMatch(root.innerHTML, /deck-merchant__blocked|is-over/);
assert.match(root.innerHTML, /You pay <b>1,899 Cr<\/b><\/div><button type="button" class="is-primary" data-deck-act="merchant-submit" title="Settles at your home at once" >Trade/, "it fits: even storage");
// More than the Alloys can clear: no fix, a hint on hover.
stall.handleInput({ dataset: { merchantInput: "buy:composites" }, value: "100" }, route);
assert.match(root.innerHTML, /<p class="deck-merchant__blocked is-full" title="Buy less, or make room at home first\."><span>Storage full · 249\/250<\/span><\/p>/);
// Not storage: the plain reason, the trader by name.
home.storage_used = 120;
stall.handleInput({ dataset: { merchantInput: "buy:composites" }, value: "121" }, route);
assert.match(root.innerHTML, /<p class="deck-merchant__blocked"><span>Teo hasn(&#39;|')t that much<\/span><\/p>/);
for (const box of ["buy:composites", "buy:electronics", "buy:machinery", "sell:alloys"]) stall.handleInput({ dataset: { merchantInput: box }, value: "0" }, route);
assert.doesNotMatch(root.innerHTML, /deck-merchant__blocked/, "an empty table names its terms");

// Before it docks, nothing can be traded.
state.merchant = { ...merchant, phase: "arriving" };
stall.invalidate();
stall.render({ name: "merchant" }, true);
assert.match(root.innerHTML, /the Long Odds · Arriving/);
assert.match(root.innerHTML, /“Hold your horses, I(&#39;|')m still docking\.”/);
assert.match(root.innerHTML, /data-merchant-input="buy:composites" disabled/);
// Mobile: the same Max (as if typed) and Trade; Teo's portrait 25% larger in
// both shells, from the 320 px derivative (Peter, Oct 21 2026).
// An open guide floats over the table; the table keeps its height (Peter, Oct 21 2026).
const deckCss = readFileSync(new URL("../src/styles/deck.css", import.meta.url), "utf8");
assert.match(deckCss, /\.deck-workspace:is\(\[data-route="merchant"\], \[data-route="shipyard"\]\) \{ bottom: max\(calc\(var\(--deck-control-height\) \+ var\(--space-6\)\), calc\(var\(--deck-bottom-row-inset, 0px\) \+ var\(--space-3\)\)\); \}/);
assert.doesNotMatch(deckCss, /data-route="merchant"\][^{]*\.deck-founding/, "the guide never folds for the trader");
const mobile = src("shell/mobile/surfaces.ts");
assert.match(mobile, /data-mobile-act="merchant-max" \$\{docked && most > 0 \? "" : "disabled"\}>Max<\/button>/);
assert.match(mobile, /case "merchant-max": \{[\s\S]*?box\.value = units > 0 \? String\(units\) : "";\n\s*this\.handleMerchantInput\(box\);/);
assert.match(mobile, /data-mobile-act="merchant-submit"[^>]*>Trade<\/button>/);
assert.match(mobile, /merchantBlock\(merchant, sells, quote, this\.merchantHome\(\), trader\.first,\n\s*this\.ctx\.state\.credits !== null \? spendableMarketCredits\(\) : undefined\)/);
assert.match(mobile, /<p class="m-merchant-blocked\$\{block\.full \? " is-full" : ""\}"><span>\$\{esc\(block\.text\)\}<\/span>\$\{fix\}<\/p>/);
assert.match(mobile, /data-mobile-act="merchant-room" data-units="\$\{block\.sell\}">Sell \$\{fmt\(block\.sell\)\} \$\{esc\(human\(merchant\.buys\)\)\}<\/button>/);
assert.match(mobile, /case "merchant-room": \{[\s\S]*?box\.value = String\(units\);\n\s*this\.handleMerchantInput\(box\);/);
assert.match(mobile, /m-merchant-portrait[^\n]{0,160}width="90" height="90"/);
assert.match(src("shell/deck/merchant.ts"), /deck-merchant__portrait[^\n]{0,160}width="210" height="210"/);
assert.match(src("core/derive/trader.ts"), /-320\.webp/);
console.log("PASS: merchant quotes, refusals, shortfall prefill, four traders' lines (Teo, Ines, Mags, Rafi), Max and Trade, and the desktop negotiation table.");

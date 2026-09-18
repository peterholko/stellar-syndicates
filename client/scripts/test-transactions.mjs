// Real reducers, pagination and both shells, without connecting to a player's game.
// --serve exposes only this isolated visual fixture at :8094.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const dependencies = {};
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => dependencies[name] ?? {}, performance, ...globals });
  return exports;
}
const protocol = dependencies["./protocol"] = compile("protocol.ts");
dependencies["./equipment"] = dependencies["../../core/derive/equipment"] = compile("core/derive/equipment.ts");
const history = dependencies["./core/derive/transactions"] = dependencies["./derive/transactions"] =
  dependencies["../../core/derive/transactions"] = compile("core/derive/transactions.ts");
const stateModule = dependencies["../state"] = dependencies["../../state"] = compile("state.ts");
const { state } = stateModule;
const icons = dependencies["../icons"] = dependencies["../../icons"] = compile("icons.ts");
dependencies["../../protocol"] = protocol;
dependencies["../core/derive/format"] = dependencies["../../core/derive/format"] = compile("core/derive/format.ts");
dependencies["../battlehistory"] = { loadBattleMarks() {} };
dependencies["../wire.mjs"] = { PROTOCOL_VERSION: 32 };
const market = dependencies["./derive/market"] = dependencies["../../core/derive/market"] = compile("core/derive/market.ts");
const { applyServerMessage, applyLinkStatus } = compile("core/session.ts");
const ui = dependencies["../transactions"] = compile("shell/transactions.ts");
const { emptyTransactions, requestTransactions } = history;
state.playerId = "1";
state.link = "online";
state.galaxy = { systems: [{ id: "home", name: "Freya" }], hub: { x: 70_000, y: 0 } };
const sent = [];
const ctx = { state, send: (message) => sent.push(message) };
const row = (id, trade = {}) => ({ id, occurred_at: id * 10, reported_at: id * 10 + 40,
  details: { kind: "trade", trade: { event: "Sold", player: "1", commodity: "metallic_ore",
    units: 150, unit_price: 8.22, penalty: 3, ...trade } } });
const page = (max, next, player = "1", request = sent.at(-1).request_id) => applyServerMessage({
  type: "Transactions", player_id: player, request_id: request, before: state.transactions.before,
  entries: Array.from({ length: Math.min(25, max) }, (_, i) => row(max - i)), next_before: next, since: 200,
}, state);
const live = (id, player = "1") => applyServerMessage({ type: "TransactionRecorded", player_id: player, entry: row(id) }, state);

requestTransactions(ctx);
requestTransactions(ctx);
assert.equal(sent.length, 1, "no request per View while a page is pending");
assert.equal(page(60, 36, "2").length, 0, "another owner's page is ignored");
assert.equal(page(60, 36, "1", 0).length, 0, "stale request cannot overwrite current page");
assert.equal(page(60, 36)[0].kind, "TransactionsApplied");
requestTransactions(ctx);
assert.equal(sent.length, 1, "loaded history is not fetched per View");
assert.equal(live(61)[0].kind, "TransactionsApplied");
assert.equal(state.transactions.entries.length, 25);
assert.equal(state.transactions.entries[0].id, 61);
assert.equal(state.transactions.nextBefore, 37);
assert.equal(live(61).length, 0, "duplicate notifications do not duplicate records");
assert.equal(live(62, "2").length, 0);

requestTransactions(ctx, "older");
assert.equal(sent.at(-1).before, 37);
page(36, 12);
live(62);
assert.equal(state.transactions.entries[0].id, 36, "new receipts do not pull an older page away");
assert.equal(state.transactions.newer, true);
requestTransactions(ctx, "newer");
assert.equal(sent.at(-1).before, null);
page(62, 38);
assert.equal(state.transactions.newer, false);
live(64);
assert.equal(state.transactions.loaded, false, "a missed notification repairs from stored history");
requestTransactions(ctx);
page(64, 40);
requestTransactions(ctx, "latest");
state.transactions.requestedWallMs = Date.now() - 13_000;
const previousRequest = sent.at(-1).request_id;
requestTransactions(ctx);
assert.notEqual(sent.at(-1).request_id, previousRequest, "lost reply is retried without a permanent spinner");
page(64, 40);
applyLinkStatus("offline", state);
const count = sent.length;
requestTransactions(ctx, "older");
assert.equal(sent.length, count);
assert.match(ui.transactionsHtml(state), /Offline · saved reports/);
applyLinkStatus("online", state);

const sale = ui.transactionSummary(row(1), state);
assert.equal(sale.net, 1230);
assert.equal(sale.fees, 3);
assert.equal(sale.item, "Ferrite Ore");
assert.equal(ui.transactionSummary(row(2, { event: "Bought" }), state).net, -1236);
assert.equal(ui.transactionSummary(row(3, { event: "LimitFilled", side: "sell" }), state).net, 1230);
assert.equal(ui.transactionSummary(row(4, { event: "LimitPlaced", side: "sell", limit_price: 8 }), state).net, null);
assert.equal(ui.transactionSummary(row(5, { event: "Unloaded", system: null }), state).net, null);
assert.equal(ui.transactionSummary(row(6, { event: "Loaded", system: "home" }), state).note, "From Freya");
assert.equal(ui.transactionSummary({ ...row(7), details: { kind: "purchase", item: "fuel", units: 10,
  unit_price: 4, fees: 2, fleet: "ship", system: null } }, state).net, -42);
assert.equal(ui.transactionTime(90061), "T+25:01:01", "long games never wrap the timestamp to yesterday");
state.transactions.entries = [{ ...row(8), occurred_at: null,
  details: { kind: "earlier_report", text: '<img src=x onerror="bad"> Old receipt' } }];
const escaped = ui.transactionsHtml(state);
assert.match(escaped, /Earlier report/);
assert.match(escaped, /&lt;img/);
assert.doesNotMatch(escaped, /<img/);
assert.equal(ui.transactionSummary(state.transactions.entries[0], state).net, null, "no fabricated finances from old prose");

applyServerMessage({ type: "Welcome", player_id: "2", name: "Other", tick_hz: 30, pacing_scale: 1,
  tick: 0, sim_time: 0, galaxy: { instance_id: "new", systems: [] } }, state);
assert.equal(state.transactions.loaded, false);
assert.equal(state.transactions.entries.length, 0, "a new session must load only its own checkpoint's ledger");
assert.equal(live(65).length, 0, "old owner's in-flight receipt cannot refill the cleared history");

// Both real shell controllers display the shared ledger, even without a fresh wallet.
state.playerId = "1";
state.transactions = emptyTransactions();
state.galaxy = { systems: [{ id: "home", name: "Freya" }] };
requestTransactions(ctx);
page(12, null);
state.transactions.entries = [row(12), row(11, { event: "LimitFilled", side: "buy", commodity: "machinery", units: 10, unit_price: 58.4, penalty: 0 }),
  row(10, { event: "Unloaded", commodity: "metallic_ore", system: null }),
  { ...row(9), details: { kind: "purchase", item: "fuel", units: 40, unit_price: 18.35, fees: 0, system: null, fleet: "ship" } },
  row(8, { event: "Loaded", commodity: "provisions", units: 30, system: "home" }),
  { ...row(7), occurred_at: null, details: { kind: "earlier_report", text: "Sold 150 ferrite ore at the hub for 8.22 ea." } }];
dependencies["../../core/derive/fleet"] = { hubDockedFleets: () => [] };
dependencies["../signature"] = { sheetFingerprint: JSON.stringify };
dependencies["../dom"] = { renderDeferred: () => false, setHtml: (root, html) => root.innerHTML = html };
const { DeckMarketRoutes } = compile("shell/deck/market.ts", { document: { activeElement: null } });
const root = { id: "fixture", innerHTML: "" };
const deck = new DeckMarketRoutes(root, ctx, {});
deck.render({ name: "market", query: { tab: "transactions" } });
assert.match(root.innerHTML, /data-tab="transactions" aria-selected="true"/);
assert.match(root.innerHTML, /data-transaction-id="12"/);
assert.doesNotMatch(root.innerHTML, /Market report unavailable/);
const desktopHtml = root.innerHTML;
const receiptRoute = { name: "market", query: { tab: "warehouse" } };
deck.handleAction({ dataset: { deckAct: "market-tab", tab: "transactions" } }, receiptRoute);
assert.match(root.innerHTML, /data-tab="transactions" aria-selected="true"/, "a receipt's old route tab must not override the player's tab click");

class FakeButton { constructor(dataset) { this.dataset = dataset; } closest() { return this; } }
const { MobileSurfaces } = compile("shell/mobile/surfaces.ts", { HTMLElement: FakeButton, HTMLButtonElement: FakeButton });
const mobile = new MobileSurfaces(ctx, { refresh() {} }, {});
mobile.handleClick({ target: new FakeButton({ mobileAct: "market-tab", tab: "transactions" }) });
const mobileHtml = mobile.render({ id: "market" }).html;
assert.match(mobileHtml, /data-tab="transactions" aria-selected="true"/);
assert.match(mobileHtml, /data-transaction-id="12"/);
assert.ok(desktopHtml.includes(ui.transactionsHtml(state)) && mobileHtml.includes(ui.transactionsHtml(state)), "both shells share the exact ledger rows");
console.log("PASS: delayed-receipt history, owner/request isolation, paging, reconnects, duplicates, lost-message recovery, exact values, legacy evidence and both Market tabs.");

if (process.argv.includes("--serve")) {
  const css = ["tokens", "deck", "mobile", "transactions"].map((name) => src(`styles/${name}.css`)).join("\n");
  const pageHtml = `<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><title>Transaction history fixture</title><style>${css}
    body{overflow:auto;padding:24px} .fixture{max-width:1000px;margin:auto;padding:20px;background:var(--panel)} .mobile-fixture{display:none}
    @media(max-width:620px){body{padding:8px}.fixture{padding:12px}.desktop-fixture{display:none}.mobile-fixture{display:block}}
  </style><main class="fixture"><div class="desktop-fixture">${desktopHtml}</div><div class="mobile-fixture">${mobileHtml}</div></main>`;
  createServer((_req, res) => { res.setHeader("Content-Type", "text/html"); res.end(pageHtml); })
    .listen(8094, "127.0.0.1", () => console.log("Isolated fixture: http://127.0.0.1:8094"));
}

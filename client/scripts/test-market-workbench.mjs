// Actual desktop controllers with isolated served data. --serve previews the
// resulting layouts on loopback; it never connects to or changes a live game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, performance, ...globals });
  return exports;
}

deps["./router"] = compile("shell/deck/router.ts");
const node = () => ({ dataset: {}, attributes: new Map(), hidden: false,
  setAttribute(key, value) { this.attributes.set(key, value); },
  getAttribute(key) { return this.attributes.get(key); },
  removeAttribute(key) { this.attributes.delete(key); if (key.startsWith("data-")) delete this.dataset[key.slice(5)]; }, replaceChildren() {}, append() {} });
const controls = new Map(["[data-deck-act=back]", "[data-deck-act=width]", "#deck-breadcrumb", "#deck-workspace-title"].map(key => [key, node()]));
const rail = { ...node(), querySelector: key => controls.get(key), getBoundingClientRect: () => ({ left: 1_368 }) };
const properties = new Map(), storage = new Map([["stellarSyndicates.deck.width.market", "standard"]]);
const chromeNodes = new Map();
const fakeWindow = { innerWidth: 1_920, innerHeight: 1_080, addEventListener() {} };
const renderer = { setCameraRect(rect) { this.cameraRect = rect; } };
const { DeckWorkspace } = compile("shell/deck/workspace.ts", {
  window: fakeWindow, document: { getElementById: id => chromeNodes.get(id) ?? null,
    documentElement: { style: { setProperty: (k, v) => properties.set(k, v), removeProperty: k => properties.delete(k) } } },
  localStorage: { getItem: k => storage.get(k), setItem: (k, v) => storage.set(k, v) },
});
const workspace = new DeckWorkspace(rail, renderer, new AbortController().signal);
for (const query of [undefined, { inspect: "map" }, { tab: "warehouse" }, { tab: "transactions" }]) {
  workspace.show({ name: "market", query }, [], true);
  assert.equal(rail.dataset.width, "wide", "all entry points ignore the old narrow Market preference");
  assert.equal(controls.get("#deck-workspace-title").textContent, "Market Hub");
  assert.equal(controls.get("[data-deck-act=width]").hidden, true);
  assert.equal(renderer.cameraRect.w, 1_920, "floating Market never reserves a right rail");
  assert.equal(properties.get("--deck-workspace-inset"), "0px");
  workspace.toggleWidth();
  assert.equal(rail.dataset.width, "wide");
  assert.equal(storage.get("stellarSyndicates.deck.width.market"), "standard", "no preference mutation");
}
workspace.show({ name: "system" }, [], false);
assert.equal(renderer.cameraRect.w, 1_368, "returning to System restores its camera inset");
assert.equal(controls.get("[data-deck-act=width]").hidden, false, "ordinary rail width control returns");
workspace.close();
assert.equal(rail.getAttribute("aria-hidden"), "true");
assert.equal(rail.dataset.route, undefined, "closing removes the centered presentation");
assert.equal(renderer.cameraRect.w, 1_920);
workspace.show({ name: "market" }, [], true);
chromeNodes.set("deck-command-strip", { hidden: false, offsetWidth: 700, offsetHeight: 100,
  getBoundingClientRect: () => ({ top: 900 }) });
workspace.publishCameraRect(true);
assert.equal(properties.get("--deck-workbench-bottom-inset"), "180px", "floating panels clear the live bottom stack");
fakeWindow.innerWidth = 1_366;
workspace.publishCameraRect();
assert.equal(properties.get("--deck-workspace-inset"), "0px", "resizing a centered Market cannot create a feedback inset");
assert.equal(renderer.cameraRect.w, 1_366);
workspace.teardown();
assert.equal(properties.has("--deck-workspace-inset"), false);
assert.equal(properties.has("--deck-workbench-bottom-inset"), false);

deps["./protocol"] = deps["../../protocol"] = compile("protocol.ts");
deps["./core/derive/transactions"] = deps["../../core/derive/transactions"] = compile("core/derive/transactions.ts");
const stateModule = deps["../state"] = deps["../../state"] = compile("state.ts");
const { state } = stateModule;
stateModule.liveSimTime = () => 100;
deps["../icons"] = deps["../../icons"] = compile("icons.ts");
deps["./equipment"] = deps["../../core/derive/equipment"] = compile("core/derive/equipment.ts");
deps["../core/derive/format"] = deps["../../core/derive/format"] = compile("core/derive/format.ts");
deps["../../core/derive/geo"] = { systemName: () => "Freya" };
const market = deps["../../core/derive/market"] = compile("core/derive/market.ts");
deps["../transactions"] = compile("shell/transactions.ts");
deps["../signature"] = { sheetFingerprint: JSON.stringify };
deps["../dom"] = { renderDeferred: () => false, setHtml: (root, html) => root.innerHTML = html };
const berthed = [{ id: "freighter", kind: "convoy", own: true, composition: [{ kind: "convoy", count: 1 }],
  cargo_manifest: [{ commodity: "alloys", units: 65 }], count_class: "single" }];
deps["../../core/derive/fleet"] = { hubDockedFleets: () => berthed, shipKindLabel: () => "Freighter" };
Object.assign(state, {
  playerId: "1", link: "online", simTime: 100,
  galaxy: { specialist_hire_cost: 800, systems: [{ id: "home", name: "Freya" }],
    build_options: deps["./equipment"].MODULES.map(m => ({ key: `module:${m.kind}`, costs: [{ commodity: "alloys", units: 10 }] })) },
  systems: [{ id: "home", owner: "1", stockpile: [], storage_cap: 1_000, storage_used: 400, modules: {} }],
  market: { staleness: 39, prices: market.COMMODITIES.map((commodity, i) => ({ commodity,
    price: 5 + i * 4, available_buy: 500, available_sell: 400 })) },
  wallet: { credits: 5_000, valuation: 12_500, orders: [{ id: 1, side: "buy", commodity: "alloys", units: 25, limit_price: 20 }],
    warehouse: market.COMMODITIES.map((commodity, i) => ({ commodity, units: 20 + i * 10 })) },
  freight: { terms: [{ system: "home", cap: 400, distance: 80_000, secs_out: 400, secs_round: 800 }],
    fee_frac: .02, fee_per_unit_dist: .000001, next_departure: 130, period: 120, shipments: [] },
  priceHistory: Object.fromEntries(market.COMMODITIES.map((c, i) => [c, [5 + i * 4, 6 + i * 4, 4 + i * 4, 5 + i * 4]])),
});
class Input { constructor(dataset, value, checked = false) { Object.assign(this, { dataset, value, checked }); } }
const marketDocument = { activeElement: null };
const { DeckMarketRoutes } = compile("shell/deck/market.ts", { document: marketDocument, HTMLInputElement: Input, HTMLSelectElement: Input });
const root = { id: "fixture", innerHTML: "", contains: element => element === marketDocument.activeElement }, sent = [], intents = [], opened = [];
const ctx = { state, send: message => sent.push(message), intent: { beginFleetCommand: command => intents.push(command) } };
const controller = new DeckMarketRoutes(root, ctx, { go: route => opened.push(route), notice() {} });
const route = { name: "market" };
const click = (action, data = {}) => controller.handleAction({ dataset: { deckAct: action, ...data } }, route);
controller.render(route);
assert.match(root.innerHTML, /deck-market-layout/);
assert.match(root.innerHTML, /<\/div><div class="deck-market-orders">/);
assert.doesNotMatch(root.innerHTML, /deck-market-side|the shared commons/);
click("market-commodity", { commodity: "metallic_ore" });
assert.match(root.innerHTML, /is-selected" data-deck-act="market-commodity" data-commodity="metallic_ore"/);
controller.handleInput({ dataset: { marketInput: "trade-quantity" }, value: "12" }, route);
click("market-submit");
assert.equal(sent.at(-1).units, 12);
assert.equal(sent.at(-1).commodity, "metallic_ore");
assert.equal(market.marketReservations.length, 1, "dispatch remains incoming, not an immediate execution");
assert.equal(market.recentMarketOrders.length, 0);
assert.match(root.innerHTML, /sent · awaiting Market Hub/);
assert.doesNotMatch(root.innerHTML, /data-market-input="trade-limit"/, "the inactive limit-price field does not crowd the ticket");
controller.handleInput(new Input({ marketInput: "trade-limit-on" }, "", true), route);
assert.match(root.innerHTML, /data-market-input="trade-limit"/);
assert.match(root.innerHTML, /data-deck-act="market-submit" disabled/, "limit orders still need a price");
controller.handleInput(new Input({ marketInput: "trade-limit" }, "5"), route);
assert.doesNotMatch(root.innerHTML, /data-deck-act="market-submit" disabled/);
controller.handleInput(new Input({ marketInput: "trade-limit-on" }, "", false), route);
click("market-tab", { tab: "warehouse" });
assert.match(root.innerHTML, /deck-market-warehouse/);
assert.match(root.innerHTML, /Docked fleets/);
click("market-hub-unload", { fleet: "freighter" });
assert.equal(intents.at(-1).type, "HubUnload", "unloading still arms an explicit fleet-order confirmation");
click("market-hub-fleet", { fleet: "freighter" });
assert.equal(opened.at(-1).name, "fleet");
for (const tab of ["specialists", "modules", "transactions", "exchange"]) {
  click("market-tab", { tab });
  assert.match(root.innerHTML, new RegExp(`data-tab="${tab}" aria-selected="true"`));
  if (tab === "specialists" || tab === "modules") assert.match(root.innerHTML, /deck-market-services/);
}
assert.equal(controller.handleAction({ dataset: { deckAct: "close" } }, route), false, "Close remains owned by the router");
assert.equal(controller.handleAction({ dataset: { deckAct: "back" } }, route), false);

// A new corporation/legacy-save warm-up has no arrived account yet, but the
// ticker, catalogues, local command ledger and fleet reports remain independent.
const arrivedWallet = state.wallet, arrivedMarket = state.market;
for (const pendingWallet of [null, { ...arrivedWallet, report_pending: true }]) {
  state.wallet = pendingWallet;
  controller.render(route, true);
  assert.match(root.innerHTML, /Observed prices/, "pending account must not hide the arrived ticker");
  assert.match(root.innerHTML, /Account report pending/);
  assert.match(root.innerHTML, /warehouse —/, "unknown holdings must not read as zero or leak placeholder truth");
  assert.match(root.innerHTML, /data-deck-act="market-submit" disabled/);
  assert.match(root.innerHTML, /Incoming orders/);
  assert.doesNotMatch(root.innerHTML, /No resting limit orders\./, "unknown open orders are not an empty book");
  const before = sent.length;
  click("market-submit");
  click("market-cancel", { order: "1" });
  click("market-tab", { tab: "warehouse" });
  assert.match(root.innerHTML, /Docked fleets/);
  assert.match(root.innerHTML, /data-deck-act="market-hub-unload"/);
  assert.doesNotMatch(root.innerHTML, /<b>empty<\/b>|Alloys<\/small><b>\d+/, "unknown warehouse is not empty or current truth");
  click("freight-submit");
  click("market-tab", { tab: "specialists" });
  assert.match(root.innerHTML, /Available specialists/);
  assert.match(root.innerHTML, /data-specialist="geologist" disabled/);
  click("market-hire", { specialist: "geologist" });
  click("market-tab", { tab: "modules" });
  assert.match(root.innerHTML, /Combat modules/);
  click("market-module-buy", { module: "mass_driver" });
  assert.equal(sent.length, before, "UI guards and action handlers both wait for the account report");
  click("market-tab", { tab: "exchange" });
}
state.market = null;
controller.render(route);
assert.match(root.innerHTML, /Observed prices/);
assert.match(root.innerHTML, /No observed quote yet/);
assert.match(root.innerHTML, /data-deck-act="market-submit" disabled/);
// Preparing a quantity while waiting must not hold the report behind the usual
// edit-focus guard. Reconciliation keeps the edit; readiness refreshes in-place.
marketDocument.activeElement = new Input({ marketInput: "trade-quantity" }, "12");
state.market = arrivedMarket;
state.wallet = arrivedWallet;
controller.render(route);
assert.doesNotMatch(root.innerHTML, /Account report pending|data-deck-act="market-submit" disabled/);
assert.match(root.innerHTML, /data-market-input="trade-quantity"[^>]+value="12"/);
marketDocument.activeElement = null;
const css = src("styles/deck.css");
assert.match(css, /\.deck-build-workbench, \.deck-workspace\[data-route="market"\].*var\(--deck-build-workbench-max\)/,
  "Market and Build share the same floating size rule");
console.log("PASS: centered Market layout, tabs, incoming trades, pending-account isolation, action guards and automatic report arrival while editing.");

if (process.argv.includes("--serve")) {
  const publicRoot = fileURLToPath(new URL("../public/", import.meta.url));
  const tabs = ["exchange", "warehouse", "specialists", "modules", "transactions"];
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      const tab = tabs.includes(url.searchParams.get("tab")) ? url.searchParams.get("tab") : "exchange";
      state.wallet = url.searchParams.has("pending") ? null : arrivedWallet;
      click("market-tab", { tab });
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1">
        ${["tokens", "deck", "transactions"].map(name => `<link rel="stylesheet" href="/${name}.css">`).join("")}</head><body><div class="deck">
        <header class="deck-topbar"><b>Isolated Market layout preview</b><a href="/?pending">Pending account</a><a href="/">Arrived account</a></header>
        <aside id="deck-workspace" class="deck-workspace" data-route="market" data-width="wide" aria-hidden="false">
        <header class="deck-workspace__header"><button data-deck-act="back" aria-label="Back">←</button><nav class="deck-breadcrumb"></nav><h1>Market Hub</h1><button data-deck-act="width" hidden>↔</button><button data-deck-act="close" aria-label="Close">✕</button></header>
        <div class="deck-workspace__body">${root.innerHTML}</div></aside></div>
        <script>document.addEventListener('click', e => { const b = e.target.closest('[data-tab]'); if(b) { const q = new URLSearchParams(location.search); q.set('tab', b.dataset.tab); location.search = q; } });</script>
        </body></html>`);
      return;
    }
    let path;
    if (["/tokens.css", "/deck.css", "/transactions.css"].includes(url.pathname)) {
      path = fileURLToPath(new URL(`../src/styles${url.pathname}`, import.meta.url));
      response.setHeader("Content-Type", "text/css");
    } else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(resolve(publicRoot) + sep)) { response.writeHead(403).end(); return; }
    } else { response.writeHead(404).end(); return; }
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  server.listen(0, "127.0.0.1", () => console.log(`Market layout fixture: http://127.0.0.1:${server.address().port}/`));
}

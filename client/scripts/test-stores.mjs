// §stores: the stores bar and Stores pop-out (docs/stores.md): measured rates,
// groups, storage, focus and pins, the desktop module's markup and actions,
// and its wiring into the Deck. It never connects to a live game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
const storage = new Map();
const localStorage = { getItem: (key) => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, String(value)) };
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {}, localStorage, ...globals });
  return exports;
}
const s = deps["../../core/derive/stores"] = compile("core/derive/stores.ts");
const copy = (value) => JSON.parse(JSON.stringify(value));

// Every good is listed once, in one group.
const commodities = [...src("core/derive/market.ts").match(/export const COMMODITIES: Commodity\[\] = \[([\s\S]*?)\];/)[1].matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]);
const grouped = s.STORE_GROUPS.flatMap((group) => group.goods);
assert.equal(commodities.length, 34);
assert.deepEqual([...grouped].sort(), [...commodities].sort(), "the groups hold every good exactly once");

// Reports every 2 sim seconds over five minutes, whole units like the View.
const system = (units, extra = {}) => ({ id: "freya", owner: "me", storage_used: 100, storage_cap: 1_000,
  stockpile: Object.entries(units).map(([commodity, n]) => ({ commodity, units: Math.floor(n) })), ...extra });
function feed(unitsAt, from = 0, to = 300, extra = {}) {
  for (let t = from; t <= to; t += 2) s.noteStockSamples({ playerId: "me", systems: [system(unitsAt(t), extra)] }, t);
}
const near = (value, want, tolerance, message) => assert.ok(Math.abs(value - want) <= tolerance, `${message}: ${value} vs ${want}`);

// A steady flow reads through whole-unit rounding; a one-off transfer doesn't count.
s.clearStockSamples();
feed((t) => ({ alloys: 100 + 0.2 * t + (t >= 240 ? 40 : 0), electronics: 80 - 0.05 * t, provisions: 500 }));
near(s.stockRate("freya", "alloys"), 0.2, 0.01, "+12/min Alloys despite a 40-unit unload");
near(s.stockRate("freya", "electronics"), -0.05, 0.005, "−3/min Semiconductors");
assert.equal(s.stockRate("freya", "provisions"), 0, "a flat good measures zero");
const drain = s.storeLine(system({ electronics: 65 }), "electronics");
near(drain.emptyIn, 1_300, 140, "65 units at −3/min");
assert.equal(drain.low, true, "empties inside half an hour");
assert.equal(s.storeLine(system({ alloys: 160 }), "alloys").emptyIn, null, "a rising good never empties");

// §vault: the served reserve marks what raiders can't reach; the header shows the Vault's room.
const vaulted = system({ electronics: 120, alloys: 150 }, { vault: [{ commodity: "electronics", units: 120 },
  { commodity: "alloys", units: 80 }], bodies: [{ structures: { vault: 1 } }, { structures: {} }] });
assert.equal(s.storeLine(vaulted, "electronics").safe, 120, "all of the dearest good");
assert.equal(s.storeLine(vaulted, "alloys").safe, 80, "the Vault's share of the next");
assert.equal(s.storeLine(system({ alloys: 30 }, { vault: [{ commodity: "alloys", units: 80 }] }), "alloys").safe, 30,
  "never more than is stored");
assert.equal(s.storeLine(system({ alloys: 30 }), "alloys").safe, 0, "no report, nothing claimed");
assert.equal(s.vaultCapacity(vaulted), 200, "200 a level");
assert.equal(s.vaultCapacity(system({})), 0);

// Too little history shows no rate; time running backwards or a new player resets.
s.clearStockSamples();
feed((t) => ({ alloys: 100 + t }), 0, 40);
assert.equal(s.stockRate("freya", "alloys"), null, "40 s of history is too little");
feed((t) => ({ alloys: 100 + t }), 42, 120);
assert.ok(s.stockRate("freya", "alloys") > 0.9);
feed((t) => ({ alloys: 5 }), 10, 20);
assert.equal(s.stockRate("freya", "alloys"), null, "an earlier time starts the history over");
feed((t) => ({ alloys: 100 + t }), 0, 120);
s.noteStockSamples({ playerId: "someone-else", systems: [] }, 122);
assert.equal(s.stockRate("freya", "alloys"), null, "another corporation's history is gone");
s.noteStockSamples({ playerId: "me", systems: [{ ...system({ alloys: 1 }), owner: "rival" }] }, 124);
assert.equal(s.stockRate("freya", "alloys"), null, "a rival's system is never sampled");

// Groups list held, pinned or moving goods; reservations show as held.
s.clearStockSamples();
feed((t) => ({ metallic_ore: 300 + 0.5 * t, alloys: 80, mystery_good: 4 }));
const freya = system({ metallic_ore: 450, alloys: 80, mystery_good: 4 }, {
  industry: { reservations: [{ target: { kind: "orbital_assembly" }, goods: { alloys: 30 } }, { target: { kind: "deep_extraction" }, goods: { alloys: 10 } }], outpost: null, projects: [] },
});
const groups = s.storeGroups(freya, ["deuterium"]);
assert.deepEqual(copy(groups.map((group) => group.title)), ["Raw", "Refined", "Fuel", "Other"]);
assert.deepEqual(copy(groups.map((group) => group.lines.map((line) => line.commodity))), [["metallic_ore"], ["alloys"], ["deuterium"], ["mystery_good"]]);
assert.equal(groups[1].lines[0].held, 40, "both reservations count");
assert.equal(groups[2].lines[0].units, 0, "a pinned good shows even when empty");

// Storage: the fill, the warnings and when it fills.
s.clearStockSamples();
for (let t = 0; t <= 300; t += 2) s.noteStockSamples({ playerId: "me", systems: [system({}, { storage_used: 400 + Math.floor(t) })] }, t);
let store = s.storageState(system({}, { storage_used: 700 }));
near(store.fullIn, 300, 10, "300 units of room at +1/s");
assert.equal(store.warn, false);
store = s.storageState(system({}, { storage_used: 920 }));
assert.deepEqual([store.warn, store.full], [true, false]);
store = s.storageState(system({}, { storage_used: 1_000 }));
assert.deepEqual([store.full, store.fullIn], [true, null]);

// Focus: your open system, else the selected one, else home; never a rival's.
const owned = new Set(["freya", "dabih"]);
assert.equal(s.storesFocus(owned, "dabih", "freya", "freya"), "dabih");
assert.equal(s.storesFocus(owned, "rival", "dabih", "freya"), "dabih");
assert.equal(s.storesFocus(owned, null, "rival", "freya"), "freya");
assert.equal(s.storesFocus(owned, null, null, null), "freya");
assert.equal(s.storesFocus(new Set(), "freya", null, null), null);

// Formats: terse, signed, compact.
assert.deepEqual([0.3, -2 / 60, 0.0004, null].map(s.rateText), ["+18", "−2", "", ""]);
assert.equal(s.rateText(0.4 / 60), "+0.4");
assert.deepEqual([860, 1_240, 12_400, 128_000].map(s.stockText), ["860", "1,240", "12.4k", "128k"]);

// Pins: the defaults until you pin; unknown goods are dropped.
storage.clear();
assert.deepEqual(copy(s.storePins()), ["provisions", "metallic_ore", "alloys", "machinery", "electronics", "uranium", "deuterium"]);
assert.deepEqual(copy(s.togglePin("crystalline_ore").slice(-1)), ["crystalline_ore"]);
assert.ok(!s.togglePin("uranium").includes("uranium"));
storage.set("stellar.stores.pins.v1", JSON.stringify(["alloys", "not_a_good", 7]));
assert.deepEqual(copy(s.storePins()), ["alloys"]);
storage.clear();

// The desktop module: bar and workbench markup, pins, switcher and shipping.
deps["../../core/derive/format"] = { fmt: (n) => Math.round(n).toLocaleString(), fmtBuildDur: (secs) => `${Math.round(secs / 60)}m` };
deps["../../core/derive/geo"] = { foundingHomeSystemId: () => "freya" };
deps["../../core/derive/market"] = { shippableStock: (sys) => sys.stockpile.filter((slot) => !["uranium", "deuterium", "antimatter"].includes(slot.commodity) && slot.units >= 1) };
deps["../../icons"] = { commodityIcon: (c) => `<img class="icon icon--resource" alt="${c}" title="${c}">`, label: (c) => c === "electronics" ? "Semiconductors" : c };
deps["../../state"] = { liveSimTime: () => 300 };
deps["../dom"] = { renderDeferred: () => false, setHtml: (el, html) => { el.html = html; } };
deps["../signature"] = { sheetFingerprint: (value) => JSON.stringify(value) };
class FakeSelect {}
const { DeckStores } = compile("shell/deck/stores.ts", { HTMLSelectElement: FakeSelect });
s.clearStockSamples();
feed((t) => ({ provisions: 1_000 + 0.3 * t, electronics: 60 - 0.04 * t, uranium: 120 }));
const dabih = { ...system({ alloys: 5 }), id: "dabih" };
const st = { playerId: "me", selectedSystemId: null, systems: [system({ provisions: 1_090, electronics: 48, uranium: 120 }), dabih],
  galaxy: { systems: [{ id: "freya", name: "Freya" }, { id: "dabih", name: "Dabih" }] } };
const sent = [], notices = [];
const deck = { toggled: [], classList: { toggle: (name, on) => deck.toggled.push([name, on]) } };
const element = (id) => ({ id, hidden: true, html: "", textContent: "", addEventListener() {}, closest: (selector) => selector === ".deck" ? deck : null });
const bar = { ...element("deck-stores"), querySelector: () => null }, body = element("deck-stores-workbench-body"), title = element("deck-stores-workbench-title");
const shell = { ...element("deck-stores-workbench"), querySelector: (selector) => selector === "#deck-stores-workbench-body" ? body : selector === "#deck-stores-workbench-title" ? title : null };
let resized = 0;
const stores = new DeckStores(bar, shell, { state: st, send: (message) => sent.push(message) },
  { notice: (html) => notices.push(html), resized: () => resized++ }, { aborted: false });
stores.renderBar(null);
assert.equal(bar.hidden, false);
assert.deepEqual(deck.toggled, [["has-stores", true]], "the chrome grows by the bar");
assert.equal(resized, 1, "the map's free area is republished");
assert.match(bar.html, /<select class="deck-stores__system" data-stores-system/, "two systems: a switcher");
assert.match(bar.html, /<b>1,090<\/b><small class="is-up">\+18<\/small>/, "Provisions with its rate");
assert.match(bar.html, /class="deck-stores__good is-low"[^>]*title="Semiconductors 48 · −2\.4\/min · empty in 20m"/, "draining Semiconductors warn");
assert.doesNotMatch(bar.html, /<img[^>]* title=/, "the button's tooltip, not the icon's");
assert.match(bar.html, /deck-stores__storage[^>]*title="Storage 100 \/ 1,000"/);
assert.equal(bar.html.match(/class="deck-stores__good[ "]/g).length, 7, "seven default goods");

// Peter (Oct 28 2026): Stores opens over the map like Construction; the
// route (here the system's Production tab) and the camera stay.
const route = { name: "system", params: { id: "freya" }, query: { tab: "production" } };
stores.render(route, true);
assert.equal(shell.hidden, true, "closed until asked");
stores.handleAction({ dataset: { deckAct: "stores-open", system: "freya" } }, route);
assert.equal(shell.hidden, false, "the workbench opens");
assert.equal(title.textContent, "Freya");
assert.match(body.html, /<h3>Refined<\/h3>.*provisions/s);
assert.match(body.html, /deck-stores-row is-low.*empty 20m/s);
assert.match(body.html, /data-deck-act="stores-ship" >Ship 1,138 goods to Market/, "fuels stay out of the shipment");
stores.handleAction({ dataset: { deckAct: "stores-pin", good: "uranium" } }, route);
assert.ok(!s.storePins().includes("uranium"), "unpinned from the bar");
assert.equal(bar.html.match(/class="deck-stores__good[ "]/g).length, 6);
stores.handleAction({ dataset: { deckAct: "stores-ship" } }, route);
assert.deepEqual(copy(sent), [{ type: "ShipProduction", system_id: "freya" }]);
assert.match(notices[0], /Authority pickup requested/);
stores.handleAction({ dataset: { deckAct: "stores-open", system: "dabih" } }, route);
assert.equal(title.textContent, "Dabih", "a system's own Open Stores shows it");
assert.equal(stores.handleInput(Object.assign(new FakeSelect(), { value: "dabih", dataset: { storesSystem: "" } }), route), true);
assert.match(bar.html, /<option value="dabih" selected>Dabih/, "a pick switches the bar");
assert.match(body.html, /<option value="dabih" selected>Dabih/, "and the open workbench");
stores.renderBar(route);
assert.match(bar.html, /<option value="dabih" selected>Dabih/, "the pick holds while you stay");
stores.renderBar({ name: "world", params: { systemId: "freya" } });
assert.match(bar.html, /<option value="freya" selected>Freya/, "opening a system shows that system");
st.selectedSystemId = "dabih";
stores.renderBar(null);
assert.match(bar.html, /<option value="dabih" selected>Dabih/, "with nothing open, the map's selection");
st.selectedSystemId = null;
assert.equal(stores.close(), true, "Esc closes it");
assert.equal(shell.hidden, true);
assert.equal(stores.close(), false, "and then passes Esc on");
stores.toggle(null);
assert.equal(shell.hidden, false, "I opens it");
stores.toggle(null);
assert.equal(shell.hidden, true, "and I closes it");
storage.clear();

// Wiring: workbench, bar, chrome height, shortcut, session sampling. Stores
// is no route: opening it never closes the workspace or moves the camera.
assert.doesNotMatch(src("shell/deck/router.ts"), /"stores"/);
assert.doesNotMatch(src("shell/deck/workspace.ts"), /"stores"/);
assert.match(src("shell/deck/markup.ts"), /<section id="deck-stores-workbench" class="deck-build-workbench deck-stores-workbench"[^>]*hidden>/, "a centred workbench like Construction");
assert.match(src("shell/deck/workspace.ts"), /stores && !stores\.hidden \? stores\.getBoundingClientRect\(\)\.bottom/, "the map camera clears the bar");
assert.match(src("shell/deck/markup.ts"), /<\/header>\n\s*<div id="deck-stores" class="deck-stores" role="region" aria-label="Stores" hidden><div id="deck-stores-main" class="deck-stores__main"><\/div>/);
assert.match(src("shell/deck/markup.ts"), /<kbd>I<\/kbd><span>Stores: every good in the system<\/span>/);
const index = src("shell/deck/index.ts");
assert.match(index, /key\.toLowerCase\(\) === "i"\) \{\n\s*event\.preventDefault\(\);\n\s*this\.stores\?\.toggle/, "I opens and closes Stores");
assert.match(index, /else if \(this\.stores\?\.close\(\)\) \{/, "Esc closes it first");
assert.match(index, /this\.renderChrome\(\);\n\s*this\.stores\?\.renderBar/, "the bar refreshes with the chrome");
assert.match(src("styles/tokens.css"), /--deck-topbar-row: 40px;[\s\S]*--deck-topbar-height: var\(--deck-topbar-row\);/);
const css = src("styles/deck.css");
assert.match(css, /\.deck\.has-stores \{ --deck-topbar-height: calc\(var\(--deck-topbar-row\) \+ var\(--deck-stores-height\)\); \}/);
assert.match(css, /\.deck-topbar \{[^}]*height: var\(--deck-topbar-row\);/, "the top bar's own row keeps its height");
assert.match(src("core/session.ts"), /st\.systems = msg\.systems;\n\s*\/\/ §stores[^\n]*\n\s*noteStockSamples\(st, msg\.sim_time\);/);
const empire = src("shell/deck/empire.ts");
assert.match(empire, /<h3>Stores<\/h3>.*data-deck-act="stores-open" data-system=/s, "Production links to Stores");
assert.doesNotMatch(empire, /ship-production/, "shipping moved to Stores");

console.log("PASS: stores rates (steady flow, transfers ignored, warm-up, resets), groups, storage, focus, formats, pins, desktop bar and pop-out, wiring.");

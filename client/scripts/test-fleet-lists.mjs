// Exercise the actual desktop row/controller code with served-only fixtures.
// --serve exposes an isolated art/CSS comparison; it never connects to a game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const dependencies = {};
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => dependencies[name] ?? {}, ...globals });
  return exports;
}
const system = { id: "8", name: "Freya", pos: { x: 0, y: 0 } };
const ship = (id, kind, extra = {}) => ({ id, kind, own: true, owner: "1", age: 12,
  count_class: "one", composition: [{ kind, count: 1 }],
  pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 }, docked: null, ...extra });
const home = ship("home", "raider", { docked: "8" });
const berth = ship("berth", "convoy", { docked: "E8", age: 0,
  cargo_manifest: [{ commodity: "alloys", units: 40 }, { commodity: "fuel", units: 20 }] });
const initialGhosts = [home, berth, ship("nearby", "raider"), ship("hub", "convoy", { docked: "hub" }),
  ship("elsewhere", "scout", { docked: "9" }), ship("rival", "raider", { own: false, docked: "8" }),
  ship("guard", "raider", { guard_target: "hub", vel: { x: 20, y: 0 } })];
const state = { playerId: "1", galaxy: { systems: [system, { id: "9", name: "Vega" }] },
  systems: [], ghosts: structuredClone(initialGhosts), selectedShipIds: new Set(),
  battles: [], commandSignals: [], orders: {}, raids: {}, syndicate: { flagship_name: "North Star" } };
dependencies["../../state"] = { state, liveSimTime: () => 100 };
dependencies["../../protocol"] = compile("protocol.ts");
dependencies["./equipment"] = dependencies["../../core/derive/equipment"] = compile("core/derive/equipment.ts");
dependencies["../../icons"] = compile("icons.ts");
dependencies["../../core/derive/fleet"] = compile("core/derive/fleet.ts");
dependencies["../../core/derive/format"] = compile("core/derive/format.ts");
dependencies["./fleet-row"] = compile("shell/deck/fleet-row.ts");
const { fleetListRow } = dependencies["./fleet-row"];
const { DeckRosterRoutes } = compile("shell/deck/roster.ts");
const { DeckEmpireRoutes } = compile("shell/deck/empire.ts");
const opened = [], centered = [], sent = [];
const hooks = { go: (route) => opened.push(route), notice() {} };
const ctx = { state, renderer: { stateVersion: 0, centerOnWorld: (pos) => centered.push(pos) }, send: (msg) => sent.push(msg) };
const roster = new DeckRosterRoutes({}, ctx, hooks);
const empire = new DeckEmpireRoutes({}, {}, {}, ctx, hooks);
roster.render = () => true; // Selection tests do not need a DOM repaint.
const fleetIds = (html) => [...html.matchAll(/data-fleet-row="([^"]+)"/g)].map((match) => match[1]).sort();

assert.deepEqual(fleetIds(roster.fleetsHtml()), ["berth", "elsewhere", "guard", "home", "hub", "nearby"]);
assert.deepEqual(fleetIds(empire.systemFleets(system)), ["berth", "home"], "system list remains own docked fleets only");
state.ghosts.find((g) => g.id === "home").pos = { x: 99_999, y: 0 };
assert.ok(fleetIds(empire.systemFleets(system)).includes("home"), "served berth, not position estimates, controls membership");
state.ghosts.find((g) => g.id === "home").docked = null;
assert.deepEqual(fleetIds(empire.systemFleets(system)), ["berth"], "fresh departure report retires the row");
state.ghosts = structuredClone(initialGhosts);
assert.match(empire.systemFleets(system), /Information delay 12s/);
assert.match(empire.systemFleets(system), /Live report/);
assert.match(empire.systemFleets(system), /60 cargo/);
assert.doesNotMatch(empire.systemFleets(system), /roster-group|roster-center/);
assert.match(roster.fleetsHtml(), /Market Hub/);
assert.match(roster.fleetsHtml(), />Guarding</);

const ownOpts = { openAction: "system-fleet-open", status: "Docked", flagshipName: "North Star" };
assert.ok(empire.systemFleets(system).includes(fleetListRow(home, ownOpts)));
assert.ok(roster.fleetsHtml().includes(fleetListRow(home, {
  ...ownOpts, openAction: "roster-open", location: "Freya", controls: true, grouped: false,
})), "both controllers use the same row renderer");
const { icon } = dependencies["../../icons"];
for (const kind of ["scout", "raider", "corvette", "convoy", "colony", "destroyer", "cruiser", "battleship", "dreadnought", "titan", "freighter"]) {
  assert.ok(fleetListRow(ship(kind, kind), ownOpts).includes(icon(kind === "freighter" ? "authorityFreighter" : kind, "md")), `${kind} uses hull-specific art`);
}
assert.ok(fleetListRow(ship("mixed", "raider", { composition: [{ kind: "raider", count: 2 }] }), ownOpts).includes(icon("fleet", "md")));
assert.match(fleetListRow(ship("unknown", "raider", { composition: null }), ownOpts), /estimated 1 ships/);
assert.match(fleetListRow(ship("titan", "titan"), ownOpts), /North Star/);
assert.doesNotMatch(fleetListRow(ship('id"<', "titan"), { ...ownOpts, flagshipName: '<img src=x onerror="bad">' }), /<img src=x|data-fleet="id"</);

const click = (controller, action, id, route) => controller.handleAction({ dataset: { deckAct: action, fleet: id } }, route);
click(roster, "roster-open", "home", { name: "fleets" });
assert.equal(opened.at(-1).params.id, "home");
assert.equal(state.selectedShipId, "home");
assert.deepEqual([...state.selectedShipIds], ["home"]);
click(roster, "roster-group", "berth", { name: "fleets" });
assert.deepEqual([...state.selectedShipIds].sort(), ["berth", "home"]);
assert.match(roster.fleetRow(berth), /aria-pressed="true"/);
const openCount = opened.length;
click(roster, "roster-center", "berth", { name: "fleets" });
assert.equal(centered.at(-1), state.ghosts.find((g) => g.id === "berth").pos);
assert.equal(opened.length, openCount, "Center does not open another panel");
click(empire, "system-fleet-open", "berth", { name: "system", params: { id: "8" } });
assert.equal(opened.at(-1).params.id, "berth");
assert.deepEqual([...state.selectedShipIds], ["berth"]);
assert.ok(ctx.renderer.stateVersion > 0, "row selection invalidates the map highlight");
const validOpenCount = opened.length;
for (const id of ["rival", "nearby", "hub", "missing"]) click(empire, "system-fleet-open", id, { name: "system", params: { id: "8" } });
click(roster, "roster-open", "rival", { name: "fleets" });
assert.equal(opened.length, validOpenCount, "forged/stale action targets cannot bypass membership");
assert.equal(sent.length, 0, "list interactions are local selection, not fleet orders");

dependencies["./router"] = compile("shell/deck/router.ts");
const storage = new Map();
const { DeckWorkspace } = compile("shell/deck/workspace.ts", { window: { addEventListener() {} },
  localStorage: { getItem: (key) => storage.get(key), setItem: (key, value) => storage.set(key, value) } });
const workspace = new DeckWorkspace({ dataset: {} }, {}, new AbortController().signal);
for (const width of ["standard", "wide"]) {
  storage.set("stellarSyndicates.deck.width.system", width);
  storage.set("stellarSyndicates.deck.width.fleets", width === "standard" ? "wide" : "standard");
  assert.equal(workspace.storedWidth({ name: "fleets" }), workspace.storedWidth({ name: "system" }), "old independent roster preference cannot cause mismatched widths");
}
const css = src("styles/deck.css");
assert.match(css, /\.deck-workspace:is\([^\n]*\[data-route="fleets"\][^\n]*var\(--deck-workspace-system\)/);
assert.match(css, /\.deck-fleet-list-row__art > \.icon[^\n]*width: calc\(var\(--space-6\) \* 2\); height: calc\(var\(--space-6\) \* 2\); object-fit: contain/);
assert.match(src("styles/tokens.css"), /--space-6: 24px/);
assert.match(src("styles/tokens.css"), /--deck-workspace-system: 552px/);
console.log("PASS: shared fleet rows, hull art, served membership, local selection/group/center, shared width preference and 48px thumbnails.");

if (process.argv.includes("--serve")) {
  state.ghosts = structuredClone(initialGhosts);
  state.selectedShipIds = new Set();
  const publicRoot = fileURLToPath(new URL("../public/", import.meta.url));
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      const panel = (route, title, html, left) => `<aside class="deck-workspace" data-route="${route}" data-width="standard" style="top:0;${left ? "left:0;right:auto" : ""}"><header class="deck-workspace__header"><h1>${title}</h1></header><div class="deck-workspace__body">${html}</div></aside>`;
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css"></head><body><div class="deck">${panel("fleets", "Galaxy Fleets", roster.fleetsHtml(), true)}${panel("system", "Freya · Fleets", empire.systemFleets(system), false)}</div></body></html>`);
      return;
    }
    let path;
    if (url.pathname === "/tokens.css" || url.pathname === "/deck.css") {
      path = fileURLToPath(new URL(`../src/styles${url.pathname}`, import.meta.url));
      response.setHeader("Content-Type", "text/css");
    } else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(resolve(publicRoot) + sep)) { response.writeHead(403).end(); return; }
    } else { response.writeHead(404).end(); return; }
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  server.listen(0, "127.0.0.1", () => console.log(`Fleet lists fixture: http://127.0.0.1:${server.address().port}/`));
}

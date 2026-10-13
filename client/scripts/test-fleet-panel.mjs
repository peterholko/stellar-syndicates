// The everyday fleet panel and its details workbench. The panel keeps the
// verbs players reach for; the workbench (the construction workbench's place
// and size) shows the hull in 3D and the fleet's full record.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = file => readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
class HTMLElementStub {}
const load = (file, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(file), { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText, { exports, require: name => deps[name] ?? {}, document: { activeElement: null }, HTMLElement: HTMLElementStub });
  return exports;
};

const st = {
  ghosts: [], pendingOrders: new Map(), orders: {}, raids: {}, battles: [], systems: [], captains: [],
  selectedShipIds: new Set(), emplacements: [], jumpDepartures: [], commandSignals: [], wallet: { credits: 5000 },
  market: null, syndicate: null, research: { programmes: [] }, commandCenter: { x: 0, y: 0 },
  galaxy: { c: 400, sensor_range: 80000, jump_spool_s: 10, build_options: [], systems: [] },
};
const clock = { state: st, liveSimTime: () => 0 };
const protocol = load("protocol.ts");
const eq = load("core/derive/equipment.ts", { "../../protocol": protocol });
const fleets = load("core/derive/fleet.ts", { "../../state": clock, "../../protocol": protocol, "./equipment": eq });
const icons = { icon: key => `<i data-icon="${key}"></i>`, label: slug => slug, commodityIcon: () => "" };
const market = load("core/derive/market.ts", { "../../state": clock, "../../protocol": protocol, "./fleet": fleets, "./equipment": eq, "../../icons": icons });
const format = load("core/derive/format.ts", { "../../state": clock });
const geo = { gravityWellAt: () => false, nearestKnownDock: () => ({ name: "Freya", distance: 1200, pos: { x: 1, y: 1 } }),
  systemName: () => "Freya", emplacementLabel: () => "Sensor" };
const orders = load("core/derive/orders.ts", { "../../protocol": protocol, "../../icons": icons, "../../render": { renderer: {} },
  "../../state": clock, "./geo": geo, "./research": {}, "./fleet": fleets, "./format": format, "./blockade": {}, "../fleetorders": {} });
const readiness = load("core/derive/readiness.ts", { "../../protocol": protocol, "./fleet": fleets });
const { DeckFleetRoutes } = load("shell/deck/fleet.ts", {
  "../../protocol": protocol,
  "../../core/derive/blockade": { blockadeStatus: () => "Blockading", heldOutsideStatus: () => "Held", runBlockadesCopy: () => ({ label: "Run", hint: "" }) },
  "../../core/derive/interdiction": { FIELD_FUEL: 20, FIELD_RADIUS_SU: 8000, FIELD_SECONDS: 300, interdictionReason: () => "", interdictorCapable: g => g.composition.some(c => c.kind === "destroyer") },
  "../../core/derive/captains": { captainTitle: t => t, captainXpFloor: () => 0, fleetCommandLoad: () => 1 },
  "../mission": { missionHtml: () => `<section class="deck-section"><h3>Mission</h3></section>`, missionCommand: () => null, pirateFactionHtml: () => "" },
  "../industry": { fleetIndustryHtml: () => "" },
  "../../core/derive/belts": { isStation: g => g.kind === "mining_belt_station", anchorStatus: () => "Anchored" },
  "../../core/derive/readiness": readiness,
  "../../core/derive/fleet": fleets,
  "../../core/derive/format": format,
  "../../core/derive/geo": geo,
  "../../core/derive/market": market,
  "../../core/derive/equipment": eq,
  "../../core/derive/tenders": { tenderStatus: () => "Tending" },
  "../fueltransfer": { fuelTransferHtml: () => "" },
  "../../core/derive/orders": orders,
  "../../core/derive/nebula": { jumpRangeAt: () => 50000 },
  "../../core/session": { jumpDepartureKey: () => "" },
  "../../icons": icons,
  "../../state": clock,
  "../art": { captainPortrait: () => "" },
  "../dom": { renderDeferred: () => false, setHtml: (node, html) => { node.innerHTML = html; } },
  "../signature": { sheetFingerprint: JSON.stringify },
});

const ghost = (id, composition, extra = {}) => ({
  id, kind: composition[0].kind, own: true, pos: { x: 5000, y: 0 }, vel: { x: 0, y: 0 }, age: 0, composition,
  fuel: 120, docked: null, path: [], loadouts: [], cargo_manifest: [], posture: "passive", ...extra,
});
const interceptor = ghost("101", [{ kind: "raider", count: 1 }]);
const freighter = ghost("102", [{ kind: "tiny_freighter", count: 1 }], { docked: "E7", cargo_manifest: [{ commodity: "metallic_ore", units: 30 }] });
const squadron = ghost("103", [{ kind: "corvette", count: 2 }, { kind: "destroyer", count: 1 }]);
st.ghosts.push(interceptor, freighter, squadron, ghost("104", [{ kind: "mining_barge", count: 1 }]), ghost("105", [{ kind: "mining_belt_station", count: 1 }]));

const root = { innerHTML: "", contains: () => false, querySelector: () => null };
const title = { textContent: "" };
const shell = { hidden: true, querySelector: selector => selector === "#deck-fleet-workbench-title" ? title : null };
const details = { innerHTML: "", closest: selector => selector === "#deck-fleet-workbench" ? shell : null, querySelector: () => null };
const ctx = { state: st, renderer: { selectedJumpDepartureKey: null, siteError: () => "", centerOnWorld() {} },
  intent: { beginFleetCommand() {}, armJumpAiming() {}, armGuardAiming() {}, beginPendingIntent() {} }, send() {} };
const panel = new DeckFleetRoutes(root, ctx, { go() {}, notice() {} }, details);
const route = id => ({ name: "fleet", params: { id } });
const click = (act, id, data = {}) => panel.handleAction({ dataset: { deckAct: act, ...data } }, route(id));

// The everyday panel: assignment, the common verbs, fuel, cargo and a way in.
panel.render(route("101"), true);
let html = root.innerHTML;
for (const expected of ["Assignment", "Holding station", "data-deck-act=\"fleet-move\"", "Set jump destination", "Guard a fleet",
  "Initiate docking", "Transit", "Fuel", "data-deck-act=\"fleet-details\"", "Fleet details"]) {
  assert.ok(html.includes(expected), `the panel shows ${expected}`);
}
for (const absent of ["Hold position", "Recall raid", "Lift blockade", "Withdraw from battle", "Standing policy", ">Officer<",
  "Composition", "Readiness", "Equipment", "ship-model-preview", "Raise interdiction"]) {
  assert.ok(!html.includes(absent), `the panel leaves ${absent} to the details`);
}
assert.equal(shell.hidden, true, "the details stay closed until asked for");
// §fuel-overhaul: the fusion tank reads as space, never energy over space: a
// full tank of Helium-3 (52.5 units, 315 of energy) is 52.5 / 52.5.
Object.assign(freighter, { fuel: 315, fuel_capacity: 52.5, helium3: 52.5, uranium: 10, uranium_capacity: 52.5 });
panel.render(route("102"), true);
assert.ok(root.innerHTML.includes("0 + 53 He-3 / 53"), "Helium-3 counts its units of tank space");
assert.ok(!root.innerHTML.includes("315 / 53"), "never energy over physical capacity");
Object.assign(freighter, { fuel: 120, fuel_capacity: undefined, helium3: undefined, uranium: undefined, uranium_capacity: undefined });

// Urgent verbs appear only while they apply.
st.orders["101"] = { x: 9000, y: 0 };
interceptor.vel = { x: 300, y: 0 };
st.raids["101"] = { target: "999" };
st.battles.push({ id: "b1", participants: ["101"] });
panel.render(route("101"), true);
html = root.innerHTML;
for (const expected of ["Hold position", "Recall raid", "Withdraw from battle"]) assert.ok(html.includes(expected), expected);
delete st.orders["101"]; interceptor.vel = { x: 0, y: 0 }; delete st.raids["101"]; st.battles.length = 0;

// A freighter keeps its cargo; a hull without a jump drive offers no jump.
panel.render(route("102"), true);
html = root.innerHTML;
assert.ok(html.includes("Cargo") && html.includes("metallic_ore"), "cargo stays on the everyday panel");
assert.ok(!html.includes("Set jump destination"), "no jump verb without a jump drive");
assert.ok(!html.includes("Guard a fleet"), "a civilian hull cannot guard");

// The details workbench: the hull on show in 3D, then the full record.
click("fleet-details", "101");
assert.equal(shell.hidden, false, "Fleet details opens the workbench");
assert.equal(title.textContent, "Interceptor fleet");
html = details.innerHTML;
assert.match(html, /<ship-model-preview class="is-hero" kind="raider" turntable>/, "the hull is shown in 3D, slowly turning");
for (const expected of ["Readiness", "Composition", "Equipment", "Standing policy", "Officer", "Mission", "Impulse", "Attack"]) {
  assert.ok(html.includes(expected), `the details carry ${expected}`);
}
assert.match(root.innerHTML, /aria-expanded="true"/, "the panel's button shows the details are open");
assert.equal(panel.closeDetails(), true, "Esc closes the details first");
assert.equal(shell.hidden, true);
assert.equal(panel.closeDetails(), false, "and then has nothing more to close");

// A mixed fleet picks which hull is on show; the flagship leads.
click("fleet-details", "103");
assert.match(details.innerHTML, /kind="destroyer"/, "the flagship leads");
assert.match(details.innerHTML, /data-deck-act="fleet-hull" data-kind="corvette" aria-pressed="false"/);
assert.ok(details.innerHTML.includes("Raise interdiction field"), "rarer commands live in the details");
click("fleet-hull", "103", { kind: "corvette" });
assert.match(details.innerHTML, /kind="corvette"/, "choosing a hull puts it on show");
click("fleet-hull", "103", { kind: "titan" });
assert.match(details.innerHTML, /kind="corvette"/, "a hull the fleet lacks is ignored");

// Leaving the fleet closes its details; the guide can open them directly.
panel.render(route("101"), true);
assert.equal(shell.hidden, true, "another fleet's panel closes the details");
panel.render({ name: "command" }, true);
panel.openDetails("101");
assert.equal(shell.hidden, true, "opening waits for that fleet's panel");
panel.render(route("101"), true);
assert.equal(shell.hidden, false, "then shows on its panel (the Recon fit step)");
assert.match(source("shell/deck/command.ts"), /openFleetDetails\?\.\(fleet\.id\)/, "the guide opens the details for the Recon fit");
assert.match(source("shell/deck/index.ts"), /else if \(this\.fleet\?\.closeDetails\(\)\)/, "Esc closes the fleet details");
assert.match(source("ship-model-preview.ts"), /height:var\(--ship-preview-height,250px\)/, "the stage can grow for the hero view");

panel.openDetails("104");
panel.render(route("104"), true);
assert.match(details.innerHTML, /<ship-model-preview\b[^>]*\bkind="mining_barge" turntable>/, "Mining Barge opens its own 3D hero");
panel.closeDetails();
panel.openDetails("105");
panel.render(route("105"), true);
assert.match(details.innerHTML, /<ship-model-preview\b[^>]*\bkind="mining_belt_station" turntable>/, "Mining Belt Station opens its own 3D hero");
panel.closeDetails();
console.log("Fleet panel: everyday verbs, urgent verbs on demand, cargo, details workbench with 3D hull, Mining Barge and Belt Station heroes, hull choice, Esc and guide entry passed.");

// --serve: the real panel, deck CSS and 3D models over canned fleets. It never
// contacts a game server; orders only print in the fixture's status line.
if (process.argv.includes("--serve")) {
  const { createServer } = await import("node:http");
  const { resolve, sep } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const { build } = await import("vite");
  const clientRoot = fileURLToPath(new URL("../", import.meta.url));
  const markup = source("shell/deck/markup.ts");
  const workspaceShell = markup.match(/<aside id="deck-workspace"[\s\S]*?<\/aside>/)[0]
    .replace('aria-hidden="true"', 'aria-hidden="false" data-route="fleet" data-width="standard"');
  const workbenchShell = markup.match(/<section id="deck-fleet-workbench"[\s\S]*?<\/section>/)[0];
  const module = path => JSON.stringify(resolve(clientRoot, `src/${path}`));
  const result = await build({ root: clientRoot, configFile: false, publicDir: false, logLevel: "error",
    plugins: [{ name: "fleet-fixture", resolveId: id => id.endsWith("virtual:fleet-fixture") ? "\0fleet-fixture" : undefined,
      load: id => id === "\0fleet-fixture" ? `
        import { DeckFleetRoutes } from ${module("shell/deck/fleet.ts")};
        import { state } from ${module("state.ts")};
        import { installPressGuard } from ${module("shell/dom.ts")};
        import ${module("ship-model-preview.ts")};
        state.playerId = "1"; state.simTime = 100; state.wallet = { credits: 48000, reserved: 0 };
        state.commandCenter = { x: 0, y: 0 };
        state.galaxy = { c: 400, hub: { x: -60000, y: 20000 }, hyperlimit: 900, sensor_range: 80000, jump_spool_s: 10,
          build_options: [], nebulas: [], systems: [{ id: "7", name: "Freya", pos: { x: 0, y: 0 } }, { id: "9", name: "Vega", pos: { x: 42000, y: -18000 } }] };
        const captain = { id: 4, name: "Mara Venn", portrait: "mara_venn", level: 3, title: "commander", portrait_age: "middle",
          command_capacity: 3, xp: 220, next_level_xp: 400, unspent: 0, attributes: { command: 3, navigation: 2, fieldcraft: 1, logistics: 1 } };
        state.captains = [{ ...captain, assigned_fleet: "101", stationed_system: null, recovering_until: null }];
        const ghost = (id, composition, extra = {}) => ({ id, owner: "1", kind: composition[0].kind, own: true, jumped: false,
          pos: { x: 18000, y: 6000 }, vel: { x: 0, y: 0 }, age: 0, composition, count_class: "one", route: null, passengers: {}, cargo: null,
          fuel: 140, fuel_capacity: 200, docked: null, path: [], loadouts: composition.map(c => ({ kind: c.kind, n: c.count, modules: [] })),
          cargo_manifest: [], posture: "defensive", signature: 1, ...extra });
        state.ghosts = [
          ghost("101", [{ kind: "raider", count: 1 }], { captain, speed: 520, vel: { x: 380, y: -160 },
            path: [{ pos: { x: 42000, y: -18000 }, t: 160 }], loadouts: [{ kind: "raider", n: 1, modules: ["mass_driver", "extended_tanks"] }] }),
          ghost("102", [{ kind: "small_freighter", count: 2 }], { docked: "E7", pos: { x: 0, y: 0 }, posture: "passive",
            cargo_manifest: [{ commodity: "metallic_ore", units: 60 }, { commodity: "provisions", units: 24 }] }),
          ghost("103", [{ kind: "destroyer", count: 1 }, { kind: "corvette", count: 3 }], { pos: { x: 26000, y: 9000 }, fuel: 90, fuel_capacity: 260,
            loadouts: [{ kind: "destroyer", n: 1, modules: ["interdiction_field", "mass_driver"] }, { kind: "corvette", n: 3, modules: ["point_defense_screen"] }] }),
          ghost("104", [{ kind: "mining_barge", count: 1 }], { posture: "passive", speed: 30,
            cargo_manifest: [{ commodity: "metallic_ore", units: 45 }] }),
          ghost("105", [{ kind: "mining_belt_station", count: 1 }], { posture: "passive", speed: 0,
            cargo_manifest: [{ commodity: "metallic_ore", units: 240 }] }),
        ];
        state.orders = { "101": { x: 42000, y: -18000 } };
        const status = text => { document.getElementById("fixture-status").textContent = text; };
        const ctx = { state, send: command => status(JSON.stringify(command)),
          renderer: { selectedJumpDepartureKey: null, siteError: () => "", centerOnWorld() {} },
          intent: { beginFleetCommand: command => status("Order: " + JSON.stringify(command)), armJumpAiming: () => status("Aiming a jump"),
            armGuardAiming: () => status("Pick a fleet to guard"), beginPendingIntent: () => status("Pending intent") } };
        const root = document.getElementById("deck-workspace-body");
        const panel = new DeckFleetRoutes(root, ctx, { go: route => status("Opened " + route.name), notice: html => status(html.replace(/<[^>]+>/g, "")) },
          document.getElementById("deck-fleet-workbench-body"));
        let current = { name: "fleet", params: { id: new URLSearchParams(location.search).get("fleet") ?? "101" } };
        const show = id => { current = { name: "fleet", params: { id } }; document.getElementById("deck-workspace-title").textContent = "Fleet " + id; panel.render(current, true); };
        for (const id of ["deck-workspace", "deck-fleet-workbench"]) document.getElementById(id).addEventListener("click", event => {
          const button = event.target.closest("button[data-deck-act]");
          if (button && !button.disabled) panel.handleAction(button, current);
        });
        document.addEventListener("keydown", event => { if (event.key === "Escape") panel.closeDetails(); });
        for (const button of document.querySelectorAll("[data-fixture-fleet]")) button.onclick = () => show(button.dataset.fixtureFleet);
        // The deck measures the workspace column the same way (workspace.ts).
        const inset = () => document.documentElement.style.setProperty("--deck-workspace-inset", (innerWidth - document.getElementById("deck-workspace").getBoundingClientRect().left) + "px");
        addEventListener("resize", inset); inset();
        installPressGuard();
        show(current.params.id);
        if (new URLSearchParams(location.search).has("details")) panel.openDetails(current.params.id);
        setInterval(() => panel.render(current, false), 250);
      ` : undefined }], build: { write: false, minify: false, lib: { entry: "virtual:fleet-fixture", formats: ["es"] } } });
  const output = (Array.isArray(result) ? result[0] : result).output;
  const bundle = output.find(file => file.type === "chunk" && file.isEntry).code;
  const files = new Map(output.map(file => [`/${file.fileName}`, file.type === "chunk" ? file.code : file.source]));
  const publicRoot = resolve(clientRoot, "public");
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Fleet panel fixture</title><link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css"><style>body{margin:0;background:#050a12}</style></head><body><div class="deck"><header class="deck-topbar" style="pointer-events:auto"><b>Fleet fixture</b><button data-fixture-fleet="101">Interceptor</button><button data-fixture-fleet="102">Freighters</button><button data-fixture-fleet="103">Squadron</button><button data-fixture-fleet="104">Mining Barge</button><button data-fixture-fleet="105">Mining Belt Station</button><span id="fixture-status"></span></header>${workspaceShell}${workbenchShell}</div><script type="module" src="/fixture.js"></script></body></html>`);
      return;
    }
    if (url.pathname === "/fixture.js") { response.setHeader("Content-Type", "text/javascript"); response.end(bundle); return; }
    if (files.has(url.pathname)) { response.setHeader("Content-Type", url.pathname.endsWith(".css") ? "text/css" : "text/javascript"); response.end(files.get(url.pathname)); return; }
    let path;
    if (["/tokens.css", "/deck.css"].includes(url.pathname)) { path = resolve(clientRoot, `src/styles${url.pathname}`); response.setHeader("Content-Type", "text/css"); }
    else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
      const type = { glb: "model/gltf-binary", svg: "image/svg+xml", webp: "image/webp", png: "image/png", jpg: "image/jpeg", json: "application/json" }[path.split(".").pop()];
      if (type) response.setHeader("Content-Type", type);
    } else { response.writeHead(404).end(); return; }
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  const port = Number(process.argv.find(arg => arg.startsWith("--port="))?.slice("--port=".length)) || 0;
  server.listen(port, "127.0.0.1", () => console.log(`Fleet panel fixture: http://127.0.0.1:${server.address().port}/`));
}

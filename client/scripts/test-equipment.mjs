import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = file => readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
const load = (file, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(file), { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText, { exports, require: name => deps[name] ?? {} });
  return exports;
};
const eq = load("core/derive/equipment.ts", { "../../protocol": load("protocol.ts") });
const st = { systems: [], research: { programmes: [] }, pendingOrders: new Map(), galaxy: { build_options: [
  { key: "module:extended_tanks", costs: [{ commodity: "alloys", units: 12 }, { commodity: "polymers", units: 8 }, { commodity: "machinery", units: 4 }] },
  { key: "module:recon_suite", costs: [{ commodity: "electronics", units: 12 }] },
] } };
const clock = { state: st };
const protocol = load("protocol.ts");
const fleets = load("core/derive/fleet.ts", { "../../state": clock, "../../protocol": protocol, "./equipment": eq });
const icons = { label: slug => eq.MODULES.find(m => m.kind === slug)?.name ?? slug, icon: () => "" };
const market = load("core/derive/market.ts", { "../../state": clock, "../../protocol": protocol, "./fleet": fleets, "./equipment": eq, "../../icons": icons });

assert.equal(eq.MODULES.length, 13);
for (const m of eq.MODULES) assert.equal(m.fit, ["torpedo_rack", "whipple_armor", "prismatic_lance"].includes(m.kind) ? 3 : 2);
for (const kind of ["scout", "raider", "corvette", "convoy"]) assert.equal(market.fitLegal(kind, ["extended_tanks"]), true);
for (const kind of ["freighter", "builder", "colony", "cruiser", "titan"]) assert.equal(market.fitLegal(kind, ["extended_tanks"]), false);
for (const m of eq.MODULES) assert.equal(market.fitLegal("convoy", [m.kind]), ["extended_tanks", "cargo_pods", "fuel_transfer_rig"].includes(m.kind));
assert.equal(market.fitLegal("raider", ["recon_suite", "extended_tanks"]), true);
assert.equal(market.fitLegal("raider", ["extended_tanks", "extended_tanks"]), false);
assert.equal(market.fitLegal("scout", ["recon_suite", "extended_tanks"]), false);
assert.equal(market.fitLegal("raider", ["not_a_module"]), false);
assert.equal(eq.utilityChange(["mass_driver"], ["mass_driver", "extended_tanks"]), true);
assert.equal(eq.utilityChange(["mass_driver"], ["extended_tanks"]), false);
assert.equal(market.fitLegal("corvette", ["escort_datalink"]), false);
const escortFit = ["point_defense_screen", "escort_datalink"];
assert.equal(market.fitLegal("corvette", escortFit), true);
assert.equal(market.fitLegal("corvette", [...escortFit, "whipple_armor"]), false);
for (const hull of ["raider", "scout", "convoy", "destroyer", "cruiser", "titan"]) {
  assert.equal(market.fitLegal(hull, escortFit), false);
}
assert.equal(eq.utilityProgramme("escort_datalink"), "hull_line_iii_escort_datalink");
assert.equal(eq.utilityChange(["point_defense_screen"], escortFit), true);
assert.equal(eq.utilityChange([], escortFit), false);
assert.equal(eq.utilityProgramme("fuel_transfer_rig"), "prop_expedition_iv_fleet_tenders");
for (const kind of ["scout", "raider", "corvette", "freighter", "titan"]) {
  assert.equal(market.fitLegal(kind, ["fuel_transfer_rig"]), false);
}
for (const extra of ["fuel_transfer_rig", "cargo_pods", "extended_tanks"]) {
  assert.equal(market.fitLegal("convoy", ["fuel_transfer_rig", extra]), false);
}
assert.match(eq.hullUtilitySummary("corvette", ["point_defense_screen"], escortFit), /one extra intercept\/step/);

const g = { id: "fleet", own: true, kind: "convoy", composition: [{ kind: "convoy", count: 1 }],
  loadouts: [], fuel: 20, fuel_capacity: 157.5, docked: "E29", vel: { x: 0, y: 0 } };
const dock = { id: "29", owner: "me", structures: { shipyard: 1 }, bodies: [{ id: 0, structures: { shipyard: 1 } }],
  assignments: [{ body_id: 0, structure: "shipyard", staffing: 1, skill: 1 }], modules: { extended_tanks: 1, recon_suite: 1 },
  stockpile: [{ commodity: "alloys", units: 12 }, { commodity: "polymers", units: 8 }, { commodity: "machinery", units: 4 }] };
st.systems = [dock]; st.ghosts = [g]; st.playerId = "me";
assert.equal(market.moduleBuildReason(dock, "extended_tanks"), "Research required");
st.research.programmes.push({ id: eq.utilityProgramme("extended_tanks"), state: "completed" });
assert.equal(market.moduleBuildReason(dock, "extended_tanks"), "");
dock.assignments[0].staffing = 0;
assert.equal(market.moduleBuildReason(dock, "extended_tanks"), "Assign Shipyard workforce");
dock.assignments[0].staffing = 1;
dock.stockpile[0].units = 11;
assert.equal(market.moduleBuildReason(dock, "extended_tanks"), "Insufficient stock");
dock.stockpile[0].units = 12;
st.research.programmes = []; // recovered stock is usable without the blueprint
const choices = market.utilityRefitChoices(g, dock);
assert.equal(choices.length, 1);
assert.equal(choices[0].module, "extended_tanks");
assert.match(choices[0].preview, /157.5 → 275.6 Fuel/);
assert.match(choices[0].preview, /aboard 20.0 unchanged/);
assert.match(choices[0].preview, /Refit 3s after receipt/);
assert.equal(g.fuel, 20);
assert.equal(g.loadouts.length, 0, "preview does not fit the served ship");
const fullTanks = { ...g, fuel: 275.625, fuel_capacity: 275.625, loadouts: [{ kind: "convoy", n: 1, modules: ["extended_tanks"] }] };
assert.equal(market.refitFuelBlocked(fullTanks, ["extended_tanks"], []), true);
assert.equal(market.utilityRefitChoices(fullTanks, dock)[0].blocked, true);
const scout = { ...g, kind: "scout", composition: [{ kind: "scout", count: 1 }] };
assert.equal(eq.sensorMultiplier(scout), 0);
const arrived = { ...scout, loadouts: [{ kind: "scout", n: 1, modules: ["recon_suite"] }] };
assert.equal(eq.sensorMultiplier(arrived), .5);
assert.equal(eq.sensorMultiplier(scout), 0, "new fitting never changes an older report");
const interceptor = { ...g, kind: "raider", composition: [{ kind: "raider", count: 1 }], loadouts: [{ kind: "raider", n: 1, modules: ["recon_suite"] }] };
assert.equal(eq.sensorMultiplier(interceptor), 1.5);
assert.match(eq.hullUtilitySummary("scout", [], ["recon_suite"]), /Site contacts 30k → 60k su/);

for (const module of ["survey_drive", "nebula_spectrometer", "prismatic_lance"]) {
  assert.equal(eq.isBlueprintOnly(module), true);
  assert.equal(market.moduleBuildReason(dock, module), "Recover blueprint through exploration");
}
st.galaxy.build_options.push({ key: "module:survey_drive", costs: [{ commodity: "alloys", units: 12 }] });
st.research.blueprints = ["survey_drive"];
assert.equal(market.moduleBuildReason(dock, "survey_drive"), "");
assert.equal(market.moduleBuildReason(dock, "nebula_spectrometer"), "Recover blueprint through exploration");
const speedy = { ...scout, loadouts: [{ kind: "scout", n: 1, modules: ["survey_drive"] }] };
assert.equal(fleets.fleetBaseSpeed(speedy), fleets.fleetBaseSpeed(scout) * 1.25);
assert.equal(fleets.fleetBaseSpeed({ ...speedy, composition: [{ kind: "scout", count: 2 }] }), fleets.fleetBaseSpeed(scout));
assert.equal(eq.tankMultiplier(["survey_drive"]), .6);
assert.equal(eq.sensorMultiplier({ ...scout, loadouts: [{ kind: "scout", n: 1, modules: ["nebula_spectrometer"] }] }), 0);
assert.equal(market.fitLegal("scout", ["survey_drive", "nebula_spectrometer"]), false);
assert.equal(market.fitLegal("raider", ["prismatic_lance", "reflective_plating"]), false);

const controller = load("shell/deck/fleet.ts", { "../../core/derive/market": market, "../../core/derive/equipment": eq,
  "../../core/derive/fleet": fleets, "../../icons": icons, "../../core/derive/geo": { systemName: () => "Home" } });
const staged = [];
const ctx = { state: st, intent: { beginFleetCommand: command => staged.push(command) } };
const panel = new controller.DeckFleetRoutes({}, ctx, {});
let html = panel.refitHtml(g);
assert.match(html, /Equipment/);
assert.match(html, /Extended Tanks/);
assert.match(html, /data-refit-preview/);
assert.match(html, /157.5 → 275.6/);
assert.doesNotMatch(html, /value="mass_driver"/);
const row = { dataset: { ship: "convoy", from: "" }, querySelector: selector => selector === "[data-refit-target]"
  ? { value: "extended_tanks" } : { max: "1", value: "1" } };
panel.refit({ closest: () => row }, g);
assert.equal(staged.length, 1);
assert.equal(staged[0].type, "RefitShips", "uses the standard confirmation intent, not a direct network send");
assert.equal(staged[0].to[0], "extended_tanks");
assert.equal(g.loadouts.length, 0);
assert.match(source("shell/mobile/surfaces.ts"), /beginFleetCommand\(\{ type: "RefitShips"/);
for (const file of ["shell/deck/empire.ts", "shell/deck/market.ts", "shell/mobile/parity.ts", "shell/mobile/surfaces.ts"]) {
  assert.match(source(file), /import \{ MODULES/);
}
for (const path of ["ui_icons/png/64/concept-sensor-range.png", "ui_icons/resource/fuel.png"]) {
  assert.equal(existsSync(new URL(`../public/art/${path}`, import.meta.url)), true, path);
}
assert.equal(market.fitLegal("convoy", ["cargo_pods", "extended_tanks"]), false);
assert.equal(market.fitLegal("convoy", ["cargo_pods", "cargo_pods"]), false);
for (const hull of ["scout", "raider", "corvette", "freighter", "builder", "colony", "titan"]) {
  assert.equal(market.fitLegal(hull, ["cargo_pods"]), false);
}
assert.equal(eq.utilityProgramme("cargo_pods"), "hull_cargo_pods");
const podded = { ...g, loadouts: [{ kind: "convoy", n: 1, modules: ["cargo_pods"] }],
  cargo_manifest: [{ commodity: "alloys", units: 100 }, { commodity: "polymers", units: 301 }] };
assert.equal(fleets.fleetCargoCapacity(g), 400);
assert.equal(fleets.fleetCargoCapacity(podded), 800);
assert.equal(fleets.fleetFuelCapacity(podded), fleets.fleetFuelCapacity(g));
const mixed = { ...podded, composition: [{ kind: "convoy", count: 3 }, { kind: "raider", count: 1 }] };
assert.equal(fleets.fleetCargoCapacity(mixed), 1600, "only the fitted hull gains hold space");
assert.equal(market.refitCargoBlocked(podded, "convoy", ["cargo_pods"], [], 1), true);
assert.equal(market.refitCargoBlocked(podded, "convoy", ["cargo_pods"], ["extended_tanks"], 1), true);
const atLimit = { ...podded, cargo_manifest: [{ commodity: "alloys", units: 400 }] };
assert.equal(market.refitCargoBlocked(atLimit, "convoy", ["cargo_pods"], [], 1), false);
const partial = { ...mixed, loadouts: [{ kind: "convoy", n: 2, modules: ["cargo_pods"] }],
  cargo_manifest: [{ commodity: "alloys", units: 1201 }] };
assert.equal(market.maxCargoRefitCount(partial, "convoy", ["cargo_pods"], [], 2), 1);
assert.match(market.refitPreview(g, "convoy", [], ["cargo_pods"], 1), /Cargo 400 → 800/);
assert.match(market.refitPreview(podded, "convoy", ["cargo_pods"], [], 1), /Unload cargo before removing pods/);
dock.modules.cargo_pods = 2;
const cargoChoice = market.utilityRefitChoices(g, dock).find(c => c.module === "cargo_pods");
assert.ok(cargoChoice && !cargoChoice.blocked, "a recovered crate is usable without research");
assert.equal(market.utilityRefitChoices(podded, dock).find(c => c.module === "cargo_pods").blocked, true);
assert.match(panel.refitHtml(podded), /Unload cargo before removing pods/);
assert.match(panel.refitHtml(partial), /data-max="1"/);
const cargoRow = { dataset: { ship: "convoy", from: "" }, querySelector: selector => selector === "[data-refit-target]"
  ? { value: "cargo_pods" } : { max: "1", value: "1" } };
panel.refit({ closest: () => cargoRow }, g);
assert.equal(staged.at(-1).to[0], "cargo_pods");
assert.equal(fleets.fleetCargoCapacity(g), 400, "a confirmation preview never changes the reported capacity");

// Every freight size uses the same logistics and fitting paths, with its own
// hold rather than a flagship-based or hardcoded Medium capacity.
for (const [index, capacity] of [50, 150, 400, 1000, 2500, 6000].entries()) {
  const kind = protocol.PLAYER_FREIGHTERS[index];
  const hull = { ...g, kind, composition: [{ kind, count: 1 }], cargo_manifest: [], loadouts: [] };
  assert.equal(protocol.cargoUnitsPerHull(kind), capacity);
  assert.equal(fleets.fleetCargoCapacity(hull), capacity);
  assert.equal(fleets.hauls(hull), true);
  assert.equal(eq.sensorMultiplier(hull), .25);
  assert.equal(market.fitLegal(kind, ["cargo_pods"]), true);
  assert.equal(market.fitLegal(kind, ["fuel_transfer_rig"]), true);
  assert.equal(market.fitLegal(kind, ["mass_driver"]), false);
  const loaded = { ...hull, loadouts: [{ kind, n: 1, modules: ["cargo_pods"] }],
    cargo_manifest: [{ commodity: "alloys", units: capacity + 1 }] };
  assert.equal(fleets.fleetCargoCapacity(loaded), capacity * 2);
  assert.equal(market.refitCargoBlocked(loaded, kind, ["cargo_pods"], [], 1), true);
}
const mixedSizes = { ...g, kind: "raider", loadouts: [],
  composition: [{ kind: "raider", count: 1 }, ...protocol.PLAYER_FREIGHTERS.map(kind => ({ kind, count: 1 }))] };
assert.equal(fleets.fleetCargoCapacity(mixedSizes), 10100);
assert.equal(fleets.hauls(mixedSizes), true, "an escort flagship does not hide its mixed freight hulls");
assert.equal(protocol.isPlayerFreighter("freighter"), false, "Authority freight remains independent");

const pdCorvette = { ...g, kind: "corvette", composition: [{ kind: "corvette", count: 1 }],
  loadouts: [{ kind: "corvette", modules: ["point_defense_screen"], n: 1 }] };
dock.modules.escort_datalink = 1;
st.ghosts.push(pdCorvette);
const linkChoice = market.utilityRefitChoices(pdCorvette, dock).find(c => c.module === "escort_datalink");
assert.ok(linkChoice && !linkChoice.blocked, "existing PD can receive a physical link at a staffed yard");
assert.equal(linkChoice.to.join("+"), escortFit.join("+"));
assert.match(panel.refitHtml(pdCorvette), /Escort Datalink/);
const linkRow = { dataset: { ship: "corvette", from: "point_defense_screen" }, querySelector: selector => selector === "[data-refit-target]"
  ? { value: escortFit.join(",") } : { max: "1", value: "1" } };
panel.refit({ closest: () => linkRow }, pdCorvette);
assert.equal(staged.at(-1).type, "RefitShips");
assert.equal(staged.at(-1).to.join("+"), escortFit.join("+"));
assert.equal(pdCorvette.loadouts[0].modules.length, 1, "confirmation does not mutate served equipment");
assert.equal(eq.sensorMultiplier({ ...pdCorvette, loadouts: [{ kind: "corvette", modules: escortFit, n: 1 }] }), eq.sensorMultiplier(pdCorvette));
assert.equal(existsSync(new URL("../public/art/ui_icons/panel/action-escort.png", import.meta.url)), true);
const sent = staged.length;
const removalRow = { ...cargoRow, dataset: { ship: "convoy", from: "cargo_pods" },
  querySelector: selector => selector === "[data-refit-target]" ? { value: "" } : { max: "1", value: "1" } };
panel.refit({ closest: () => removalRow }, podded);
assert.equal(staged.length, sent, "a changed manifest is rechecked before staging removal");
assert.equal(existsSync(new URL("../public/art/ui_icons/panel/concept-manifest.png", import.meta.url)), true);
console.log("Equipment: fitting limits, workforce/research/stock, per-hull cargo/fuel, safe partial refits, delayed reports and confirmation controls passed.");

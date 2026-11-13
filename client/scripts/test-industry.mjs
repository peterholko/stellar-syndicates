// The real client helpers, using only served stock/research fixtures.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import { commandCenter } from "./command-center-fixture.mjs";

const src = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const state = { research: { programmes: [] }, ghosts: [], playerId: "1", systems: [] };
const deps = { "../../state": { state, liveSimTime: () => 0 },
  "./fleet": { constructionStock: sys => ({ available: new Map(sys.stockpile.map(s => [s.commodity, s.units])) }) } };
function compile(path) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {} });
  return exports;
}
const icons = deps["../../icons"] = compile("icons.ts");
deps["./equipment"] = deps["../../core/derive/equipment"] = compile("core/derive/equipment.ts");
deps["../../core/derive/fleet"] = deps["./fleet"];
deps["../../core/derive/format"] = { fmtBuildDur: seconds => `${seconds}s`, fmtEta: seconds => `${seconds}s` };
const market = deps["../../core/derive/market"] = compile("core/derive/market.ts");
const colony = compile("core/derive/colony.ts");
const { DeckEmpireRoutes } = compile("shell/deck/empire.ts");
deps["./market"] = market;
deps["./construction"] = deps["../../core/derive/construction"] = compile("core/derive/construction.ts");
deps["./format"] = deps["../../core/derive/format"];
deps["./station"] ??= compile("core/derive/station.ts");
deps["../../core/derive/worldbuild"] = deps["./worldbuild"] = compile("core/derive/worldbuild.ts");
deps["./fleet"].shipKindLabel ??= kind => kind;
deps["../../shipyard-scene"] = compile("shipyard-scene.ts");
deps["../../core/derive/shipyard"] = compile("core/derive/shipyard.ts");
const { MobileParitySurfaces } = compile("shell/mobile/parity.ts");
// The mobile builder reads the served line for an order's Cancel.
const mobileThis = () => Object.assign(Object.create(MobileParitySurfaces.prototype), { ctx: { state }, cancelSent: new Set(), cancelArmed: null });
const fixtures = [
  ["composite_works", "composites", "mat_prefab_construction", .3, [["alloys", 1], ["polymers", .75], ["silicates", .5]]],
  ["hull_fabricator", "hull_sections", "mat_prefab_construction", .18, [["composites", 1.2], ["machinery", .5], ["titanium", .6]]],
  ["precision_works", "precision_components", "mat_autoforges", .2, [["electronics", 1], ["machinery", .5], ["rare_elements", .5]]],
  ["drive_works", "fission_reactor", "mat_autoforges", .005, [["precision_components", .8], ["composites", .5], ["machinery", .3]]],
];
// §charter-gates: a structure the charter hasn't reached yet waits, its step named.
{
  const opt = { key: "shipyard", label: "Shipyard", build_secs: 20, costs: [{ commodity: "alloys", units: 40 }] };
  const body = { id: 0, structures: {}, industrial_slots: 3, construction_time_mult: 1 };
  const system = { id: "home", owner: "1", structures: {}, builds: [], bodies: [body], stockpile: [{ commodity: "alloys", units: 40 }] };
  const pools = market.bodyPoolUsage(body, system);
  state.founding = { stage: "build_bioharvester", charter_gates: { shipyard: 8, power_plant: 7 } };
  const gated = market.structOption(opt, system, body, pools);
  assert.equal(gated.buildable, false);
  assert.equal(gated.reason, "The charter opens this at step 8.");
  assert.equal(gated.gate, "Charter step 8");
  state.founding = { stage: "build_shipyard" };
  assert.equal(market.structOption(opt, system, body, pools).buildable, true, "its step reached, it opens");
  state.founding = { stage: "build_bioharvester", charter_gates: { shipyard: 8 } };
  assert.equal(market.structOption(opt, { ...system, owner: "2" }, body, pools).reason.includes("charter"), false, "only the founder's own worlds");
  delete state.founding;
}
for (const [key, output, research, rate, inputs] of fixtures) {
  const opt = { key, label: icons.label(key), build_secs: 30,
    costs: [{ commodity: "alloys", units: 30 }], research_prerequisite: research,
    conversion: { output, rate, inputs } };
  const body = { id: 0, structures: {}, industrial_slots: 3, construction_time_mult: 1 };
  const system = { id: "home", structures: {}, builds: [], bodies: [body], stockpile: [{ commodity: "alloys", units: 30 }] };
  const pools = market.bodyPoolUsage(body, system);
  state.research.programmes = [{ id: research, state: "available" }];
  assert.equal(market.structOption(opt, system, body, pools).buildable, false);
  assert.match(market.structOption(opt, system, body, pools).reason, /Requires .* research/);
  state.research.programmes[0].state = "completed";
  assert.equal(market.structOption(opt, system, body, pools).buildable, true, "corporate unlock, no syndicate");
  system.stockpile[0].units = 29;
  assert.equal(market.structOption(opt, system, body, pools).buildable, false);
  system.stockpile[0].units = 60;
  assert.equal(market.structOption(opt, system, body, pools).buildable, true, "new stock reports update affordability");
  const html = DeckEmpireRoutes.prototype.structureDetail.call({}, system, body, opt, pools);
  assert.ok(html.includes(`${key}/tier-1-128.webp`), "a new structure previews tier I");
  body.structures[key] = 2;
  system.builds.push({ body_id: body.id, key });
  for (const preview of [
    DeckEmpireRoutes.prototype.structureDetail.call({}, system, body, opt, pools),
    MobileParitySurfaces.prototype.structureDetail.call(mobileThis(), system.id, system, body, opt, pools),
  ]) assert.ok(preview.includes(`${key}/tier-4-128.webp`), "both builders preview the target after the queued upgrade");
  delete body.structures[key];
  system.builds.length = 0;
  assert.ok(html.includes(icons.label(output)));
  assert.ok(html.includes(market.conversionSummary(opt)), "builder actually shows the served conversion recipe");
  for (const [input] of inputs) assert.ok(html.includes(icons.label(input)));
  assert.equal(market.POOL_OF[key], "industrial");
  assert.ok(market.COMMODITIES.includes(output), "exchange and mixed freight catalog include the new output");
  assert.equal(colony.PRODUCTION_INPUTS[key].join(), inputs.map(([good]) => good).join());
  const art = icons.structureIcon(key);
  assert.notEqual(art, "build");
  assert.equal(icons.isPlaceholder(art), false);
  const outputArt = icons.GOODS_ICONS.has(output) ? `resource/goods-v1/${output}-64` : `resource/${output}`;
  assert.ok(icons.commodityIcon(output).includes(`/art/ui_icons/${outputArt}.png`));
  for (const [path, size] of [[`structures/${key}`, 128], [outputArt, 64]]) {
    const png = readFileSync(new URL(`../public/art/ui_icons/${path}.png`, import.meta.url));
    assert.equal(png.readUInt32BE(16), size);
    assert.equal(png.readUInt32BE(20), size);
  }
}
assert.equal(market.conversionSummary({ key: "habitat" }), "");

// §gaia: Living Terraces grow only on Gaia worlds and farm with no input basket.
{
  const opt = { key: "living_terraces", label: "Living Terraces", build_secs: 30, requires_environment: "gaia",
    costs: [{ commodity: "alloys", units: 30 }], conversion: { output: "provisions", rate: 1.2, inputs: [] } };
  const body = { id: 0, structures: {}, environment: "terran", infrastructure_slots: 3, construction_time_mult: 1 };
  const system = { id: "home", structures: {}, builds: [], bodies: [body], stockpile: [{ commodity: "alloys", units: 30 }] };
  const pools = market.bodyPoolUsage(body, system);
  assert.equal(market.structOption(opt, system, body, pools).buildable, false, "a Terran world is refused");
  assert.match(market.structOption(opt, system, body, pools).reason, /Grows only on Gaia worlds/);
  body.environment = "gaia";
  assert.equal(market.structOption(opt, system, body, pools).buildable, true, "a Gaia world takes it");
  assert.equal(market.POOL_OF.living_terraces, "infrastructure");
  assert.equal(colony.PRODUCTION_INPUTS.living_terraces.length, 0, "no input basket");
  assert.match(market.conversionSummary(opt), /^No inputs → 1 Provisions · base 72\/min/);
}
assert.equal(new Set(market.COMMODITIES).size, 82, "22 goods, the three belt ores, the three pocket ores and their materials, the fuel overhaul's Uranium and Antimatter, the four reactors, the four drives, the kit parts, the munitions, §hull-ladder the 18 heavy and capital parts and §modules-as-goods the 17 modules");

// Welcome supplies the gates and names; View supplies completion. Exercise the
// shared builder helper used by BOTH shells, not a parallel UI-only rule list.
const unlocks = [
  ["prop_bunkerage", "Bunkerage", ["volatile_harvester", "fuel_refinery"]],
  ["mat_enrichment", "Enrichment", ["chemical_works"]],
  ["comp_signal_libraries", "Signal Libraries", ["electronics_fabricator"]],
  ["mat_autoforges", "Autoforges", ["machine_works", "precision_works", "drive_works", "naval_drydock"]],
  ["mat_prefab_construction", "Prefab Construction", ["composite_works", "hull_fabricator"]],
  ["weap_munitions_lines", "Munitions Lines", ["armaments_complex"]],
  ["weap_fire_control", "Fire Control", ["orbital_defense", "garrison"]],
  ["comp_sensor_gain", "Sensor Gain", ["sensor_array"]],
  ["hull_drydock_efficiency", "Drydock Efficiency", ["ordnance_foundry"]],
  ["hull_line_vii_dreadnought", "Dreadnought", ["capital_slipway"]],
  ["mat_foundry_iv_orbital_yards", "Orbital Yards", ["orbital_warehouse"]],
];
state.researchCatalog = unlocks.map(([id, name]) => ({ id, name }));
const starter = ["mining_complex", "bioharvester", "agroplex", "shipyard", "habitat", "warehouse", "academy"];
const body = { id: 0, structures: {}, deposits: ["metallic_ore", "biomass", "volatiles"].map(resource => ({ resource })),
  resource_slots: 3, industrial_slots: 3, infrastructure_slots: 3 };
const system = { id: "home", structures: { shipyard: 2, naval_drydock: 3 }, builds: [], bodies: [body], stockpile: [] };
const pools = market.bodyPoolUsage(body, system);
for (const [id, name, structures] of unlocks) for (const key of structures) {
  const opt = { key, costs: [], research_prerequisite: id };
  for (const progress of [undefined, "locked", "available", "queued", "active"]) {
    state.research.programmes = progress ? [{ id, state: progress }] : [];
    const result = market.structOption(opt, system, body, pools);
    assert.equal(result.buildable, false, `${key}: ${progress} is not completed research`);
    assert.equal(result.reason, `Requires ${name} research.`, "title comes from Welcome even before the progress report");
  }
  state.research.programmes = [{ id, state: "completed" }];
  assert.equal(market.structOption(opt, system, body, pools).buildable, true, key);
}
state.research.programmes = [];
for (const key of starter) {
  assert.equal(market.structOption({ key, costs: [] }, system, body, pools).buildable, true, `${key}: founding stays accessible`);
}
const orbitalOption = { key: "naval_drydock", costs: [], research_prerequisite: "mat_autoforges",
  research_alternatives: ["hull_modular_berths"] };
state.researchCatalog.push({ id: "hull_modular_berths", name: "Modular Berths" });
assert.equal(market.structOption(orbitalOption, system, body, pools).reason, "Requires Autoforges or Modular Berths research.");
for (const id of [orbitalOption.research_prerequisite, ...orbitalOption.research_alternatives]) {
  state.research.programmes = [{ id, state: "active" }];
  assert.equal(market.structureResearched(orbitalOption), false, "no speculative unlock");
  state.research.programmes[0].state = "completed";
  assert.equal(market.structOption(orbitalOption, system, body, pools).buildable, true, `${id}: civilian and legacy unlocks agree with server`);
}
// §orbital-defense: the station ladder's gates mirror the sim — each level's
// research and yard, one station per system, one Star Fortress per corporation.
{
  const level = (tier, label, research, requires = null) => ({ tier, label, costs: [], research_prerequisite: research, requires });
  // §gun-platform: six levels, the first needing no research.
  const ladder = { key: "orbital_defense", costs: [], research_prerequisite: null, tiers: [
    level(1, "Laser Battery", null), level(2, "Defense Platform", "weap_fire_control"), level(3, "Orbital Battery", "weap_orbital_batteries"),
    level(4, "Defense Station", "weap_cm_iii_defense_stations", { yard: "ordnance_foundry", tier: 1 }),
    level(5, "Citadel", "weap_cm_v_grand_batteries", { yard: "naval_drydock", tier: 1 }),
    level(6, "Star Fortress", "weap_cm_v_star_fortress", { yard: "capital_slipway", tier: 1 })] };
  const world = { ...body, id: 0, structures: { orbital_defense: 3 } };
  const moon = { ...body, id: 7, structures: {} };
  const fort = { id: "fort", owner: "1", structures: { naval_drydock: 1 }, builds: [], bodies: [world, moon], stockpile: [] };
  const gate = (b = world) => market.structOption(ladder, fort, b, market.bodyPoolUsage(b, fort));
  state.research.programmes = [{ id: "weap_fire_control", state: "completed" }, { id: "weap_orbital_batteries", state: "completed" }];
  assert.equal(gate().reason, "Requires Weap Cm Iii Defense Stations research.", "each level names its own programme");
  state.research.programmes.push({ id: "weap_cm_iii_defense_stations", state: "completed" },
    { id: "weap_cm_v_grand_batteries", state: "completed" }, { id: "weap_cm_v_star_fortress", state: "completed" });
  assert.equal(gate().reason, "Needs Ordnance Foundry tier 1 somewhere in this system (have 0).");
  fort.structures.ordnance_foundry = 1;
  assert.equal(gate().buildable, true, "Defense Station: researched, with its foundry");
  assert.equal(gate(moon).reason, "The system's station orbits another world; raise that one.");
  fort.builds.push({ key: "orbital_defense", body_id: 0 });
  assert.equal(gate(moon).buildable, false, "a queued level holds the station's world too");
  fort.builds = [];
  world.structures.orbital_defense = 5;
  assert.match(gate().reason, /Capital Slipway tier 1/);
  fort.structures.capital_slipway = 1;
  assert.equal(gate().buildable, true);
  state.systems = [fort, { id: "far", owner: "1", station: { level: 5, name: "Citadel", mounts: [] },
    builds: [{ key: "orbital_defense", body_id: 3 }] }];
  assert.equal(gate().reason, "Your corporation already has its Star Fortress.", "one standing or rising");
  state.systems[1].builds = [];
  assert.equal(gate().buildable, true, "a Citadel elsewhere isn't a fortress");
  // §unlocks (v72): the served ceiling caps what is offered. Above it a level
  // is `capped` (the sheets leave the building out); with no level open the
  // building isn't catalogued.
  state.research.structure_tiers = { orbital_defense: 5 };
  assert.equal(gate().capped, true, "level VI is above a ceiling of V");
  assert.equal(gate().buildable, false);
  assert.equal(market.structureUnlocked(ladder), true);
  state.research.structure_tiers.orbital_defense = 0;
  assert.equal(market.structureUnlocked(ladder), false, "no level open: not catalogued");
  state.research.structure_tiers.orbital_defense = 6;
  assert.equal(gate().capped, false);
  assert.equal(gate().buildable, true);
  delete state.research.structure_tiers;
  assert.equal(market.structureCeiling("orbital_defense"), Infinity, "unknown until research arrives");
  // A module whose programme isn't done stays out of the forges.
  assert.equal(market.moduleResearched("extended_tanks"), false);
  state.research.programmes.push({ id: "prop_expedition_iii_extended_tanks", state: "completed" });
  assert.equal(market.moduleResearched("extended_tanks"), true);
  assert.equal(market.moduleResearched("mass_driver"), true, "no programme, no gate");
  state.systems[1] = { id: "rival", owner: "2", station: { level: 6, name: "Star Fortress", mounts: [] }, builds: [] };
  assert.equal(gate().buildable, true, "a rival's fortress doesn't count");
  world.structures.orbital_defense = 6;
  assert.equal(gate().reason, "The Star Fortress is the station's top level.");
  state.systems = [];
  state.research.programmes = [];
}
assert.equal(icons.label("naval_drydock"), "Orbital Shipyard");
for (const [key, tier] of [["large_freighter", 1], ["heavy_freighter", 2], ["bulk_freighter", 3]]) {
  assert.equal(market.SHIP_YARD[key].yard, "naval_drydock");
  assert.equal(market.SHIP_YARD[key].tier, tier);
}
const yardSystem = { ...system, structures: { shipyard: 1, naval_drydock: 1 },
  builds: [{ key: "large_freighter", yard: "shipyard", body_id: 0 }] };
state.research.programmes = [{ id: "prop_line_express_charters", state: "completed" }];
assert.equal(market.shipOption({ key: "large_freighter", costs: [] }, yardSystem).slipsFull, false);
assert.equal(market.shipOption({ key: "tiny_freighter", costs: [] }, yardSystem).slipsFull, true,
  "legacy work occupies its reported original yard in both builders");
yardSystem.builds.push({ key: "large_freighter", yard: "naval_drydock", body_id: 0 });
assert.equal(market.shipOption({ key: "large_freighter", costs: [] }, yardSystem).slipsFull, true);

// §power (Peter, Nov 8 2026): the Power Plant's line is flavour by level;
// the card's Supplies and Burns rows carry the numbers, and Helium-3 waits
// for the levels its research opens.
{
  const flavour = deps["./worldbuild"].powerPlantFlavour;
  assert.match(flavour(1), /sunlight/);
  assert.doesNotMatch(flavour(2), /Deuterium|Helium-3|MW/, "a Solar Array names no fuel and no number");
  assert.match(flavour(3), /Deuterium/);
  assert.doesNotMatch(flavour(4), /Helium-3/, "Helium-3 waits for its levels");
  assert.match(flavour(5), /Helium-3/);
  assert.doesNotMatch(readFileSync(new URL("../src/shell/deck/empire.ts", import.meta.url), "utf8"), /Solar I–II; Deuterium III–IV/);
}

// §licences (docs/blueprint-licences.md): research is the skill, the
// Authority's licence the right to build. An Orbital Shipyard hull's keel
// waits for its licence; the Buy row shows its price; at war, none is needed.
{
  const fresh = { ...system, structures: { shipyard: 1, naval_drydock: 1 }, builds: [] };
  const option = { key: "large_freighter", costs: [] };
  state.charter = { licences: [{ hull: "large_freighter", held: false, price: 16196.4 }] };
  let quote = market.shipOption(option, fresh);
  assert.equal(quote.unlicensed, true);
  assert.equal(quote.buildable, false, "researched but unlicensed: no keel");
  assert.equal(quote.reason, "Needs the Authority's licence.");
  assert.equal(market.licenceNote(quote.licence), "16,196 Cr");
  assert.equal(market.licenceForSale(quote.licence), true);
  state.charter.licences[0] = { hull: "large_freighter", held: true, price: 16196.4 };
  quote = market.shipOption(option, fresh);
  assert.equal(quote.unlicensed, false);
  assert.equal(quote.buildable, true, "licensed: the keel can be laid");
  assert.equal(market.licenceNote(quote.licence), "Licensed");
  assert.equal(market.licenceForSale(quote.licence), false);
  state.charter.licences[0] = { hull: "large_freighter", held: false, price: 32392.8, refusal: { reason: "charter_closed" } };
  quote = market.shipOption(option, fresh);
  assert.equal(quote.buildable, false);
  assert.match(quote.reason, /suspended while your charter is revoked/);
  assert.equal(market.licenceForSale(quote.licence), false);
  state.charter.licences[0] = { hull: "large_freighter", held: false, price: 16196.4, refusal: { reason: "at_war" } };
  assert.equal(market.shipOption(option, fresh).buildable, true, "at war with the Authority: none needed");
  assert.equal(market.licenceNote(state.charter.licences[0]), "None needed · at war");
  assert.equal(market.licenceNote({ hull: "titan", held: false, price: 13e6, refusal: { reason: "needs_research" } }), "",
    "research-locked stays hidden");
  assert.equal(market.licenceNote({ hull: "dreadnought", held: false, price: 1304244.5, refusal: { reason: "needs_commissioned" } }),
    "Commissioned only · 1,304,245 Cr");
  assert.equal(market.shipOption({ key: "tiny_freighter", costs: [] }, fresh).licence, undefined, "a ground-Shipyard hull needs none");
  delete state.charter;
  const shellSource = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
  assert.match(shellSource("shell/deck/empire.ts"), /data-deck-act="builder-buy-licence"/, "desk Shipyard: Buy licence");
  assert.match(shellSource("shell/deck/empire.ts"), /type: "BuyLicence", hull/);
  assert.match(shellSource("shell/deck/strategic.ts"), /\$\{this\.licencesHtml\(charter\)\}/, "desk Faction: the licences");
  assert.match(shellSource("shell/deck/strategic.ts"), /type: "BuyLicence", hull/);
  assert.match(shellSource("shell/mobile/parity.ts"), /data-mobile-act="buy-licence"/, "mobile: Buy licence");
  assert.match(shellSource("shell/mobile/parity.ts"), /\$\{licences\}\$\{claims\}\$\{pay\}/, "mobile Faction: the licences and claims");
  // §claims (v90): both shells sell the Authority's claim on an unowned system
  // and list the claims held on the Faction page.
  assert.match(shellSource("shell/deck/empire.ts"), /data-deck-act="buy-claim"/, "desk: Buy claim");
  assert.match(shellSource("shell/deck/empire.ts"), /type: "BuyClaim", system: system\.id/);
  assert.match(shellSource("shell/deck/strategic.ts"), /\$\{this\.claimsHtml\(charter\)\}/, "desk Faction: the claims");
  assert.match(shellSource("shell/mobile/parity.ts"), /data-mobile-act="buy-claim"/, "mobile: Buy claim");
  assert.match(shellSource("shell/mobile/parity.ts"), /type: "BuyClaim", system: button\.dataset\.system/);
}

// Render the real catalogues: missing/unfinished research hides a structure
// even if a stale route or previous selection still points at it. Unaffordable
// researched structures remain visible; only the research gate hides a row.
state.galaxy = { build_options: [
  ...starter.map(key => ({ key, label: icons.label(key), costs: [], build_secs: 30 })),
  ...unlocks.flatMap(([id, , structures]) => structures.map(key => ({
    key, label: icons.label(key), costs: [{ commodity: "alloys", units: 100 }], build_secs: 30, research_prerequisite: id,
  }))),
] };
const desktop = Object.assign(Object.create(DeckEmpireRoutes.prototype), { ctx: { state }, selectedBuild: "smelter" });
const mobile = Object.assign(Object.create(MobileParitySurfaces.prototype), { ctx: { state }, selectedBuild: "smelter",
  buildContext: () => ({ systemId: system.id, dynamic: system, body }) });
for (const researchState of [undefined, "locked", "available", "queued", "active", "completed"]) {
  state.research.programmes = researchState ? unlocks.map(([id]) => ({ id, state: researchState })) : [];
  desktop.selectedBuild = mobile.selectedBuild = "smelter";
  const desktopHtml = desktop.structureBuilder(system, body);
  const mobileHtml = mobile.renderBuild({}).html;
  for (const html of [desktopHtml, mobileHtml]) {
    for (const key of starter) assert.ok(html.includes(`data-key="${key}"`), `${key}: starter visible`);
    for (const [, , structures] of unlocks) for (const key of structures) {
      assert.equal(html.includes(`data-key="${key}"`), researchState === "completed", `${key}: ${researchState} catalogue visibility`);
      if (researchState !== "completed") assert.ok(!html.includes(icons.label(key)), `${key}: no locked detail remains`);
    }
  }
  if (researchState !== "completed") {
    assert.notEqual(desktop.selectedBuild, "smelter", "desktop clears a hidden selection");
    assert.equal(mobile.selectedBuild, "", "mobile clears a hidden selection");
  }
}
const hullUnlocks = [
  // §research-locked (Nov 11 2026): the Corvette is Escort Datalink's.
  ["corvette", "hull_line_iii_escort_datalink"],
  ["small_freighter", "prop_freight_frames"], ["convoy", "prop_heavy_lifters"],
  ["large_freighter", "prop_line_express_charters"], ["heavy_freighter", "prop_line_bulk_charters"],
  ["bulk_freighter", "prop_line_autonomous_freight"],
  ["destroyer", "hull_line_iv_destroyer"], ["cruiser", "hull_line_v_cruiser"],
  ["battleship", "hull_line_vi_battleship"], ["dreadnought", "hull_line_vii_dreadnought"],
  ["titan", "hull_line_viii_titan"],
];
const starterHulls = ["scout", "raider", "tiny_freighter", "colony"];
state.galaxy.build_options.push(...[...starterHulls, ...hullUnlocks.map(([key]) => key)].map(key => ({
  key, label: icons.label(key), costs: [{ commodity: "alloys", units: 100 }], build_secs: 30,
})));
// Detail/fitting content is tested separately; retain markers here to catch a
// hidden hull leaking through the previous selection after a catalogue refresh.
desktop.shipDetail = (_system, _body, option) => `<h3>Hull detail: ${option.key}</h3>`;
desktop.shipCommit = (_system, option) => `<button>Build hull: ${option.key}</button>`;
mobile.shipDetail = (_id, _system, _body, option) => `<h3>Hull detail: ${option.key}</h3>`;
for (const researchState of [undefined, "locked", "available", "queued", "active", "completed"]) {
  state.research.programmes = researchState ? hullUnlocks.map(([, id]) => ({ id, state: researchState })) : [];
  desktop.selectedHull = mobile.selectedHull = "cruiser";
  const htmls = [[desktop.shipBuilder(system, body), "data-hull"], [mobile.renderShipyard({}).html, "data-kind"]];
  for (const [html, attribute] of htmls) {
    for (const key of starterHulls) assert.ok(html.includes(`${attribute}="${key}"`), `${key}: starter hull visible`);
    for (const [key] of hullUnlocks) {
      assert.equal(html.includes(`${attribute}="${key}"`), researchState === "completed", `${key}: ${researchState} hull visibility`);
    }
    assert.equal(html.includes("Hull detail: cruiser"), researchState === "completed", "no hidden hull detail");
  }
  if (researchState !== "completed") {
    assert.notEqual(desktop.selectedHull, "cruiser");
    assert.equal(mobile.selectedHull, "");
  }
}
state.research.programmes = [{ id: "hull_line_v_cruiser", state: "completed" }];
for (const [html, attribute] of [[desktop.shipBuilder(system, body), "data-hull"], [mobile.renderShipyard({}).html, "data-kind"]]) {
  assert.ok(html.includes(`${attribute}="cruiser"`), "researched hull remains visible despite insufficient goods/yard");
  for (const [key] of hullUnlocks.filter(([key]) => key !== "cruiser")) {
    assert.ok(!html.includes(`${attribute}="${key}"`), "one blueprint does not reveal other capital hulls");
  }
}
console.log("Industry: structure/hull research visibility, starter access, live affordability, recipes, cargo and icons pass.");

const berthBody={id:9,structures:{warehouse:1},infrastructure_slots:1,industrial_slots:0,resource_slots:0,construction_time_mult:1};
const berthSystem={id:"home",owner:"1",structures:{warehouse:1},builds:[],bodies:[berthBody],stockpile:[{commodity:"alloys",units:5},{commodity:"machinery",units:3}]};
const berthOption={key:"service_berth",label:"Service Berth",build_secs:120,costs:[{commodity:"alloys",units:5},{commodity:"machinery",units:3}]};
const berthPools=market.bodyPoolUsage(berthBody,berthSystem);
assert.equal(berthPools.infrastructure.used,berthPools.infrastructure.total);
assert.equal(market.structOption(berthOption,berthSystem,berthBody,berthPools).buildable,true);
const detail=DeckEmpireRoutes.prototype.structureDetail.call({ctx:{state}},berthSystem,berthBody,berthOption,berthPools);
assert.match(detail,/Warehouse attachment/);assert.match(detail,/no extra slot/);assert.doesNotMatch(detail,/1 → 2/);
berthBody.structures.service_berth=1;
assert.equal(market.bodyPoolUsage(berthBody,berthSystem).infrastructure.used,1);
assert.match(market.structOption(berthOption,berthSystem,berthBody,berthPools).reason,/one tier/);
delete berthBody.structures.service_berth;delete berthBody.structures.warehouse;
assert.match(market.structOption(berthOption,berthSystem,berthBody,berthPools).reason,/Requires a Warehouse/);

const commandBody = { id: 1, kind: "terrestrial", population: .002, structures: { command_center: 1 },
  infrastructure_slots: 2, construction_time_mult: 1, structure_build_time_mult: .9 };
const commandSystem = { id: "home", structures: {}, builds: [], bodies: [commandBody], stockpile: [] };
const commandDetail = () => [
  DeckEmpireRoutes.prototype.structureDetail.call({}, commandSystem, commandBody, commandCenter, market.bodyPoolUsage(commandBody, commandSystem)),
  MobileParitySurfaces.prototype.structureDetail.call(mobileThis(), "home", commandSystem, commandBody, commandCenter, market.bodyPoolUsage(commandBody, commandSystem)),
];
for (const html of commandDetail()) {
  assert.match(html, /Colony Command Center/);
  assert.match(html, /no extra slot/i);
  assert.match(html, /270s/);
  assert.match(html, /tier-2-128.webp/);
}
commandBody.structures.command_center = 2;
state.research.programmes = [];
for (const html of commandDetail()) {
  assert.match(html, /Syndicate Headquarters/);
  assert.match(html, /Requires Prefab Construction research/);
  assert.match(html, /120/);
  assert.match(html, /tier-3-128.webp/);
}
commandBody.structures.command_center = 3;
for (const html of commandDetail()) {
  assert.match(html, /Maximum level built or queued/);
  assert.doesNotMatch(html, /[Tt]ier 4|Queue build|Construction requirements|need 120/);
}
console.log("Command Center desktop/mobile panels: exact next-level identity, duration, materials, research gate and no fictitious fourth upgrade pass.");

// §construction: one order, one bar, on mobile too. A world's list draws the
// bar on the order building now and lists the rest by place; the system sheet
// reads one line per world, its yard work beneath.
{
  const body = { id: 0, name: "Freya I", structures: { mining_complex: 1 }, construction_time_mult: 1,
    resource_slots: 2, industrial_slots: 2, infrastructure_slots: 2 };
  const system = { id: "home", owner: "1", structures: {}, bodies: [body], stockpile: [], assignments: [],
    builds: [{ id: 7, key: "mining_complex", body_id: 0, start_time: -20, complete_time: 40 },
      { id: 8, key: "habitat", body_id: 0, queued: true, complete_time: null },
      { id: 9, key: "tiny_freighter", body_id: 0, complete_time: null }] };
  const mobile = mobileThis();
  const world = mobile.planetInfrastructure("home", system, body);
  assert.equal(world.match(/class="m-progress"/g)?.length, 1, "one bar: the order building now");
  // §live-progress: the bar carries its line (from -20 s, 100% over 60 s), so
  // each frame redraws it.
  assert.match(world, /m-order--head"><span><b>Mining Complex II<\/b><div class="m-progress" data-live-progress="-20 0 1\.6666666666666667"><i style="width:33\.3%"><\/i><\/div><\/span><span>40s<\/span><button type="button" data-mobile-act="build-cancel" data-job="7">Cancel</);
  assert.match(world, /<b>#2 Habitat I<\/b><\/span><span>starts in 40s<\/span>/);
  assert.match(world, /<b>Tiny Freighter<\/b><\/span><span>paused · needs workforce<\/span>/);
  const sheet = mobile.systemConstruction("home", system);
  assert.match(sheet, /<b>Freya I<\/b><small>Mining Complex II · 40s · \+1 queued<\/small><button type="button" class="m-yard-line" data-mobile-act="open-yard" data-system="home" data-kind="tiny_freighter">Tiny Freighter · paused · needs workforce<\/button>/,
    "a hull line opens its yard (§shipyard-view)");
  assert.doesNotMatch(sheet, /m-progress/, "the system sheet draws no bar");
}
console.log("Mobile construction: one bar on the order building now, places for the rest, one line per world.");

// §first-session: while the charter runs, the opening buildings rise in ten
// seconds on the landing world (mirror of the sim's founding::OPENING_BUILDS).
{
  const land = { id: 0, structures: { command_center: 1 }, construction_time_mult: 1 };
  const moon = { id: 1, structures: {}, construction_time_mult: 1 };
  Object.assign(state, { founding: { stage: "build_bioharvester" }, commandCenter: { x: 10, y: 20 },
    galaxy: { ...(state.galaxy ?? {}), systems: [{ id: "home", pos: { x: 10, y: 20 } }, { id: "far", pos: { x: 900, y: 0 } }] } });
  const mine = { key: "mining_complex", build_secs: 18, costs: [] };
  for (const key of ["mining_complex", "bioharvester", "habitat", "agroplex", "power_plant"]) {
    assert.equal(market.openingBuild(key, 1, "home", land), true, key);
  }
  assert.equal(market.structureBuildSeconds(mine, land, market.openingBuild("mining_complex", 1, "home", land)), 10);
  assert.equal(market.structureBuildSeconds(mine, land), 18);
  // §habitat-first (Nov 8 2026): the raid waits for the Smelter, so the
  // mine is an ordinary opening build (it rose in the privateer's flight).
  state.founding.stage = "build_mine";
  assert.equal(market.structureBuildSeconds(mine, land, market.openingBuild("mining_complex", 1, "home", land)), 10);
  assert.equal(market.structureBuildSeconds({ key: "habitat", build_secs: 20, costs: [] }, land, true), 10, "the others keep ten");
  state.founding.stage = "build_bioharvester";
  assert.equal(market.openingBuild("mining_complex", 2, "home", land), false, "level II keeps its time");
  assert.equal(market.openingBuild("smelter", 1, "home", land), false, "only the five opening buildings");
  assert.equal(market.openingBuild("mining_complex", 1, "home", moon), false, "only the Command Center's world");
  assert.equal(market.openingBuild("mining_complex", 1, "far", land), false, "only at home");
  state.founding.stage = "complete";
  assert.equal(market.openingBuild("mining_complex", 1, "home", land), false, "after the charter, the normal time");
  delete state.founding;
  delete state.commandCenter;
}
console.log("Opening builds: ten seconds for level I of the five on the landing world while the charter runs.");

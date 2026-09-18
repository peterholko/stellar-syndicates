// The real client helpers, using only served stock/research fixtures.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

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
const { MobileParitySurfaces } = compile("shell/mobile/parity.ts");
const fixtures = [
  ["composite_works", "composites", "mat_prefab_construction", .3, [["alloys", 1], ["polymers", .75], ["silicates", .5]]],
  ["hull_fabricator", "hull_sections", "mat_prefab_construction", .18, [["composites", 1.2], ["machinery", .5], ["titanium", .6]]],
  ["precision_works", "precision_components", "mat_autoforges", .2, [["electronics", 1], ["machinery", .5], ["rare_elements", .5]]],
  ["drive_works", "drive_assemblies", "mat_autoforges", .12, [["precision_components", 1.25], ["composites", 1], ["fuel", 1]]],
];
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
    MobileParitySurfaces.prototype.structureDetail.call({}, system.id, system, body, opt, pools),
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
  assert.ok(icons.commodityIcon(output).includes(`/resource/${output}.png`));
  for (const [path, size] of [[`structures/${key}`, 128], [`resource/${output}`, 64]]) {
    const png = readFileSync(new URL(`../public/art/ui_icons/${path}.png`, import.meta.url));
    assert.equal(png.readUInt32BE(16), size);
    assert.equal(png.readUInt32BE(20), size);
  }
}
assert.equal(market.conversionSummary({ key: "habitat" }), "");
assert.equal(new Set(market.COMMODITIES).size, 22);

// Welcome supplies the gates and names; View supplies completion. Exercise the
// shared builder helper used by BOTH shells, not a parallel UI-only rule list.
const unlocks = [
  ["prop_bunkerage", "Bunkerage", ["volatile_harvester", "fuel_refinery"]],
  ["mat_enrichment", "Enrichment", ["smelter", "chemical_works"]],
  ["comp_signal_libraries", "Signal Libraries", ["electronics_fabricator"]],
  ["mat_autoforges", "Autoforges", ["machine_works", "precision_works", "drive_works"]],
  ["mat_prefab_construction", "Prefab Construction", ["composite_works", "hull_fabricator"]],
  ["weap_munitions_lines", "Munitions Lines", ["armaments_complex"]],
  ["weap_fire_control", "Fire Control", ["defense_platform", "garrison"]],
  ["comp_sensor_gain", "Sensor Gain", ["sensor_array"]],
  ["hull_drydock_efficiency", "Drydock Efficiency", ["ordnance_foundry"]],
  ["hull_modular_berths", "Modular Berths", ["naval_drydock"]],
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
  ["small_freighter", "prop_freight_frames"], ["convoy", "prop_heavy_lifters"],
  ["large_freighter", "prop_line_express_charters"], ["heavy_freighter", "prop_line_bulk_charters"],
  ["bulk_freighter", "prop_line_autonomous_freight"],
  ["destroyer", "hull_line_iv_destroyer"], ["cruiser", "hull_line_v_cruiser"],
  ["battleship", "hull_line_vi_battleship"], ["dreadnought", "hull_line_vii_dreadnought"],
  ["titan", "hull_line_viii_titan"],
];
const starterHulls = ["scout", "corvette", "raider", "tiny_freighter", "colony"];
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

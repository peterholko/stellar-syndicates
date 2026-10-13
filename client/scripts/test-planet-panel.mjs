// Served-only planet management on the world route: the scene over the map area,
// the management column in the workspace, and the actual confirmation controller.
// --serve opens an isolated browser fixture; it never contacts a game server.
// Add --terran to check structure contrast against the green landscape.
// --habitable presents all six approved habitable surface variants as worlds.
// --surface-structures adds all twelve biomes, all tiers and the real layered art.
// --port=N pins the fixture's port for a launch configuration (default: any free port).
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";
import { commandCenter } from "./command-center-fixture.mjs";

const src = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
function compile(path, source = src(path), globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, ...globals });
  return exports;
}
deps["../../icons"] = compile("icons.ts");
deps["./icons"] = deps["../../icons"];
deps["./planet-surface-structure-art.generated"] = compile("planet-surface-structure-art.generated.ts");
deps["../../planet-structure-art"] = compile("planet-structure-art.ts");
const marketDerive = compile("core/derive/market.ts");
deps["../../core/derive/market"] = marketDerive;
// §construction: the world's work-slot model, shared by the panel and the controller.
deps["./construction"] = compile("core/derive/construction.ts");
deps["./market"] = marketDerive;
const worldbuild = deps["../../core/derive/worldbuild"] = compile("core/derive/worldbuild.ts");
deps["../../core/derive/format"] = compile("core/derive/format.ts");
deps["./prng"] = compile("prng.ts");
deps["../../planet-surfaces"] = compile("planet-surfaces.ts");
deps["./planet-sites"] = compile("shell/deck/planet-sites.ts");
const { PlanetSites } = deps["./planet-sites"];
deps["../../planet-sites.generated"] = compile("planet-sites.generated.ts");
deps["../../ship-models.generated"] = compile("ship-models.generated.ts");
// §construction: one structure at a time per world, in receipt order.
{
  const plain = value => JSON.parse(JSON.stringify(value));
  const catalog = [{ key: "warehouse", label: "Warehouse", costs: [], build_secs: 15 }, { key: "academy", label: "Academy", costs: [], build_secs: 20 }];
  const body = { id: 7, structures: { warehouse: 1 }, construction_time_mult: 1, structure_build_time_mult: 2 };
  const report = { builds: [
    { id: 3, key: "academy", body_id: 7, queued: true, complete_time: null },
    { id: 1, key: "warehouse", body_id: 7, queued: false, start_time: 90, complete_time: 130 },
    { id: 4, key: "warehouse", body_id: 7, queued: true, complete_time: null },
    { id: 9, key: "tiny_freighter", body_id: 7, queued: false, complete_time: 140 },
    { id: 5, key: "camp_quarters", body_id: 7, queued: true, complete_time: null },
  ] };
  const line = worldbuild.structureQueue(report, body, catalog, 100);
  assert.deepEqual(plain(line.map(job => [job.key, job.tier, job.position, job.active])),
    [["warehouse", 2, 1, true], ["academy", 1, 2, false], ["warehouse", 3, 3, false], ["camp_quarters", 1, 4, false]],
    "the active job leads, waiting jobs keep receipt order, hulls never take the slot, and each order builds the next tier");
  assert.deepEqual(plain(line.map(job => [job.startsIn, job.readyIn])), [[0, 30], [30, 70], [70, 100], [100, 340]],
    "estimates chain through the line at this world's construction speed");
  assert.deepEqual(plain(line.map(job => job.cancellable)), [false, true, true, false],
    "only waiting structures cancel, never the active job or a camp project");
  report.builds.push({ id: 6, key: "academy", body_id: 7, queued: true, complete_time: null });
  assert.equal(worldbuild.structureQueue(report, body, catalog, 100).find(job => job.job.id === 3).cancellable, false,
    "an order a later one builds on cannot be cancelled");
  assert.deepEqual(plain(worldbuild.hullJobs(report, body).map(job => job.key)), ["tiny_freighter"]);
  assert.deepEqual(plain(worldbuild.nextOrderTiming([], 20, 17)), { startsIn: 17, readyIn: 37 }, "an empty slot starts when the order arrives");
  // §build-sheet: the figures the sheet quotes are the sim's own.
  const facts = (option, from, to, world) => plain(worldbuild.structureFacts({ costs: [], build_secs: 20, ...option }, from, to, { structures: {}, ...world }));
  assert.deepEqual(facts({ key: "habitat" }, 2, 3, { habitat_capacity_mult: 1.5625 }), [{ label: "Houses", from: "78,125", to: "117,188" }],
    "25,000 people a level × the world's habitat multiplier");
  assert.deepEqual(facts({ key: "warehouse" }, 1, 2, {}), [{ label: "Storage", from: "700", to: "1,100" }]);
  assert.deepEqual(facts({ key: "warehouse" }, 1, 2, { founding_camp: { profile: "temperate", completed: [] } }), [{ label: "Storage", from: "250", to: "1,100" }],
    "a camp's Warehouse holds 250 until its Storehouse is done");
  assert.deepEqual(facts({ key: "command_center", tiers: [{ tier: 1 }, { tier: 2 }, { tier: 3 }] }, 1, 2, {}), [{ label: "Gives", from: null, to: "+1 Infrastructure slot, builds 10% faster" }]);
  assert.deepEqual(facts({ key: "shipyard" }, 1, 2, {}).find(fact => fact.label === "Unlocks"), { label: "Unlocks", from: null, to: "Orbital Shipyard" });
  assert.deepEqual(facts({ key: "fuel_refinery", conversion: { output: "deuterium", rate: .5, inputs: [] } }, 0, 1, {}),
    [{ label: "Output", from: null, to: "30 Deuterium/min" }, { label: "Crew", from: null, to: "1" }, { label: "Power", from: null, to: "1 MW" }],
    "a new converter quotes its level I rate and draw");
  // §power: a plant supplies its tier's MW and switches to Helium-3 at V; the
  // grid previews this planet after the order.
  const grid = (demand, supply, power = 1) => ({ demand, supply, power, burn: 0, fuel: "deuterium", no_fuel: false });
  assert.deepEqual(facts({ key: "power_plant" }, 4, 5, { structures: { power_plant: 4 }, power: grid(40, 51) }),
    [{ label: "Supplies", from: "48 MW", to: "68.8 MW" }, { label: "Burns", from: "Deuterium past 20.6 MW", to: "Helium-3 past 20.6 MW" }, { label: "Grid", from: "40 / 51 MW", to: "40 / 71.8 MW" }]);
  // §fuel-overhaul: I–II are Solar Arrays and burn nothing; III starts fusion on Deuterium.
  // The fusion plant keeps the solar fields: it burns only past base + solar.
  assert.deepEqual(facts({ key: "power_plant" }, 2, 3, { structures: { power_plant: 2 } }).find(fact => fact.label === "Burns"),
    { label: "Burns", from: "Nothing (solar)", to: "Deuterium past 20.6 MW" });
  assert.deepEqual(facts({ key: "power_plant" }, 0, 1, {}).find(fact => fact.label === "Burns"), { label: "Burns", from: null, to: "Nothing (solar)" });
  // The heat-heavy works draw double (no furnace fuel); the Antimatter Plant ten times.
  assert.deepEqual(facts({ key: "smelter" }, 1, 2, { structures: { smelter: 1 }, power: grid(2.6, 3) }).filter(fact => ["Power", "Grid"].includes(fact.label)),
    [{ label: "Power", from: "2 MW", to: "4.4 MW" }, { label: "Grid", from: "2.6 / 3 MW", to: "5 / 3 MW · 60%" }], "an upgrade past the base warns of the brownout");
  assert.equal(worldbuild.drawMW("antimatter_plant", 1), 10);
  assert.equal(worldbuild.drawMW("uranium_mill", 1), 1);
  assert.equal(worldbuild.gridLine({ structures: {}, power: grid(4.4, 3, .68) }).text, "⚡ 4.4 / 3 MW · Power 68%");
  assert.equal(worldbuild.gridLine({ structures: {}, power: grid(0, 3) }), null, "nothing drawing, nothing to show");
  assert.deepEqual(facts({ key: "mining_complex" }, 0, 1, {}), [{ label: "Crew", from: null, to: "1" }, { label: "Power", from: null, to: "1 MW" }],
    "a new mine's rate depends on its deposit, so none is quoted; its draw is");
  assert.equal(worldbuild.maxLevel({ key: "service_berth" }), 1);
  assert.equal(worldbuild.maxLevel({ key: "smelter" }), 6);
}
// Both ship builders offer the hulls the founding charter asks for.
for (const file of ["shell/deck/empire.ts", "shell/mobile/parity.ts"]) {
  const source = src(file);
  assert.match(source, /const SHIP_ORDER: ShipKind\[\] = \[[^\]]*"survey_probe"[^\]]*"mining_barge"/, `${file} lists the Survey Probe and Mining Barge`);
}
const deck = compile("shell/deck/planet.ts");
const { PlanetPanel, planetStructures, planetSurface, footprintAddress } = deck;
const empireSource = src("shell/deck/empire.ts");
const markup = src("shell/deck/markup.ts");
const stageShell = markup.match(/<section id="deck-planet-stage"[\s\S]*?<\/section>/)?.[0];
const buildShell = markup.match(/<section id="deck-build-workbench"[\s\S]*?<\/section>/)?.[0];
assert.ok(buildShell);
assert.ok(stageShell, "the planet scene has its own stage over the map area");
assert.doesNotMatch(markup, /deck-world-workbench/, "the full-screen planet dialog is gone");
assert.doesNotMatch(stageShell, /role="dialog"|aria-modal|world-workbench-close/, "the stage is a map layer, not a dialog with its own close button");
assert.ok(markup.indexOf(stageShell) < markup.indexOf('<aside id="deck-workspace"'), "the stage precedes the workspace in DOM order, matching map → rail");
const planetCss = src("styles/planet-panel.css");
const tokensCss = src("styles/tokens.css");
const zPlanet = Number(tokensCss.match(/--z-planet:\s*(\d+)/)?.[1]);
const zChrome = Number(tokensCss.match(/--z-chrome:\s*(\d+)/)?.[1]);
const zCommand = Number(tokensCss.match(/--z-command:\s*(\d+)/)?.[1]);
assert.ok(zPlanet > 0 && zPlanet < zChrome && zPlanet < zCommand, "the stage sits above the map but beneath chrome, the command band and toasts");
assert.match(planetCss, /\.deck-planet-stage \{[^}]*z-index: var\(--z-planet\)/);
assert.match(planetCss, /\.deck-planet-stage \{[^}]*right: var\(--deck-workspace-inset, 0px\)/, "the stage stops at the workspace edge");
assert.match(planetCss, /\.deck-planet-stage \{[^}]*--deck-workbench-bottom-inset/, "the scene keeps clear of the command band");
assert.doesNotMatch(planetCss, /--z-overlay|inset: var\(--deck-topbar-height\) 0 0/, "nothing about the planet is full-screen any more");
assert.match(planetCss, /\.deck-planet__canvas \{[^}]*width: min\(100cqw, calc\(100cqh \* 16 \/ 9\)\); aspect-ratio: 16 \/ 9;/, "one fitted image plane preserves terrain anchors for any aspect ratio");
assert.match(planetCss, /--planet-building: clamp\(36px, 7\.5cqw, 130px\)/, "front-row footprints are 7.5% of the plane, capped at 130px");
assert.doesNotMatch(planetCss, /planet-narrow|planet-wide|object-fit: cover/, "neither responsive rearrangement nor independent image cropping may move structures");
assert.match(planetCss, /--planet-footprint: calc\(var\(--planet-building\) \* var\(--planet-scale, 1\)\)/, "sprites, shadows, rings and badges share the row-scaled footprint");
assert.match(planetCss, /\.deck-planet__building::before \{[^}]*radial-gradient/, "structures stand on a soft contact shadow");
assert.doesNotMatch(planetCss, /\.deck-planet__building[^{]*\{[^}]*url\(/, "terrain comes from an explicit biome image, not a hardcoded CSS pad");
assert.match(planetCss, /\.planet-structure-ground \{[^}]*pointer-events: none/, "biome transition layers cannot steal clicks");
assert.match(planetCss, /\.deck-planet__building:has\(> \.planet-structure-art\)::before \{ display: none;/, "generated shadows are not doubled with an artificial halo");
assert.match(planetCss, /\.deck \.deck-planet__building:hover \{ background: transparent; \}/, "hovering a structure never boxes the sprite with the generic button hover");
assert.match(planetCss, /\.deck \.deck-planet__building \{[^}]*width: calc\(var\(--planet-footprint\) \* \.8\); height: calc\(var\(--planet-footprint\) \* \.72\)/, "the hit target is the building's own box, the one the address generator keeps apart");
assert.match(planetCss, /\.deck \.deck-planet__site \{[^}]*width: max\(calc\(var\(--planet-footprint\) \* \.8\)/, "build-site markers use the same box, so neighbouring markers never overlap");
assert.doesNotMatch(planetCss, /--planet-tilt|scaleY/, "footprints are never squashed");
assert.match(planetCss, /--planet-haze: clamp\(0, \(1 - var\(--planet-scale, 1\)\) \/ \.3, 1\.5\)/, "haze grows with distance, derived from the row scale");
assert.match(planetCss, /\.deck-planet__building > \.icon \{[^}]*saturate\(calc\(1 - var\(--planet-haze, 0\)[^}]*contrast\(calc\(1 - var\(--planet-haze, 0\)/, "distant sprites lose saturation and contrast like the terrain around them");
assert.doesNotMatch(planetCss, /\.deck-planet__building(:hover|\[aria-pressed="true"\]) \{[^}]*(border-color|background: rgba)/, "hover and selection are a ground ring, not a box around the sprite");
assert.match(planetCss, /\.deck-planet__building:is\(\[aria-pressed="true"\], :focus-visible\)::after \{[^}]*border-color: var\(--accent\)/, "the selected or focused structure shows an accent ground ring");
assert.match(planetCss, /\.deck-planet__scene\.is-orbital \.deck-planet__building::before \{ display: none; \}/, "orbital infrastructure has no ground to shadow");
const workspaceSource = src("shell/deck/workspace.ts");
assert.match(workspaceSource, /route\.name === "world"\) return "standard"/, "the world route keeps the system rail width so the scene stays wide");
assert.match(workspaceSource, /isPopoutRoute\(this\.route\) \|\| this\.route\.name === "world"\) return;/, "no wide toggle on a world");
// The fixture follows the actual authored catalogue and staffed-kind list.
const descriptions = Object.fromEntries([...empireSource.slice(empireSource.indexOf("const STRUCTURE_DESCRIPTION"), empireSource.indexOf("import { MODULES"))
  .matchAll(/^  (\w+): "([^"]+)"/gm)].map(([, key, description]) => [key, description]));
assert.equal(Object.keys(descriptions).length, 31);
// §construction: the staffed set lives with the work-slot model now.
const workforceStructures = new Set(worldbuild.WORKFORCE_STRUCTURES);
const catalog = [
  { key: "mining_complex", label: "Mining Complex", costs: [], build_secs: 20 },
  { key: "smelter", label: "Smelter", costs: [], build_secs: 20, conversion: { output: "alloys", rate: 1, inputs: [["metallic_ore", 2]] } },
];
const body = { id: 3, name: "Freya III", kind: "rocky", parent: null, habitable: false, size: "large", environment: "hostile", geology: "rich",
  structures: { mining_complex: 4, smelter: 2, habitat: 1, shipyard: 1 },
  deposits: [{ resource: "metallic_ore", richness: 1.5, reserves: 5000 }, { resource: "silicates", richness: 1.2, reserves: 500 }],
  population: .008, inbound_migrants: 0, migration_policy: "managed", special: null, special_effect: null,
  construction_time_mult: 1, habitat_capacity_mult: 1, population_growth_mult: 1, provisions_mult: 1,
  resource_slots: 5, industrial_slots: 8, infrastructure_slots: 6,
};
const line = { body_id: 3, structure: "mining_complex", title: "Mining Complex", tier: 4, workers: 2, specialists: { engineer: 1 },
  suspended: null, throughput: 1, staffing: 1, skill: 1, food: 1, site: 1, outputs: [["metallic_ore", .4], ["silicates", .1]] };
const report = { id: "home", owner: "1", assignments: [line, { ...line, body_id: 4, workers: 4 }], bodies: [body],
  workforce: { units: 8, posted: 6 }, builds: [{ key: "fuel_refinery", body_id: 3 }], converters: [],
  habitat_fed: true, stockpile: [], blockade: null, opportunities: [] };
const model = { system: { id: "home", name: "Freya", pos: { x: 3400, y: 0 } }, report, body, mine: true,
  art: "/art/derived/planets/2026-09-08/desert-512.png", delay: 17, catalog, descriptions, workforceStructures,
  populationHtml: "<p>Population controls</p>", surveyHtml: "<p>Survey report</p>",
  constructionHtml: '<section class="deck-construction"><button data-deck-act="world-build" data-mode="structures">Build</button><p>Queue here</p></section>',
  builderHtml: "", construction: null };
const initialModel = structuredClone(model);
// Orbital yards are real scene objects, independent of ground-site allocation.
for (const surface of [...deps["../../planet-surfaces"].HABITABLE_SURFACES, "terrestrial", "desert", "ocean", "ice", "lava", "barren", "marginal", null]) {
  const orbitModel = structuredClone(initialModel);
  const habitable = deps["../../planet-surfaces"].HABITABLE_SURFACES.includes(surface);
  orbitModel.body.habitable = habitable;
  orbitModel.body.kind = surface === null ? "gas_giant" : surface === "archipelago-world" ? "ocean" : "terrestrial";
  orbitModel.art = `/art/${surface ?? "gas_giant"}-512.png`;
  if (habitable) {
    orbitModel.body.id = 0;
    while (deck.planetSurface(orbitModel.art, "home", orbitModel.body) !== surface) orbitModel.body.id++;
  }
  orbitModel.report.bodies = [orbitModel.body];
  orbitModel.report.builds = [];
  const panel = new deck.PlanetPanel();
  // §planet-orbit: a painting's orbit lives in the space added above its 16:9
  // ground frame (negative y); a gas giant's globe keeps it inside the frame.
  const painted = surface !== null;
  const yardY = painted ? -12 : 7;
  const stationY = (level) => (painted ? -16 : 6) + level;
  const oldGround = panel.renderScene(orbitModel).match(/id="planet-spot-shipyard"[^>]*style="([^"]+)"/)[1];
  for (let tier = 1; tier <= 6; tier++) {
    Object.assign(orbitModel.body.structures, { naval_drydock: tier, capital_slipway: tier });
    panel.focus("naval_drydock");
    const html = panel.renderScene(orbitModel);
    for (const [key, x] of [["naval_drydock", 26], ["capital_slipway", 74]]) {
      assert.match(html, new RegExp(`id="planet-spot-${key}"[^>]*class="deck-planet__building is-space"[^>]*--planet-x:${x}%;--planet-y:${yardY}%;--planet-scale:1`));
      assert.ok(html.includes(`/planet-shipyards/v1/${key}/tier-${tier}-256.webp`));
      assert.equal(panel.renderScene({ ...orbitModel, mine: false }).includes(`planet-spot-${key}`), false);
    }
    assert.equal(html.match(/id="planet-spot-shipyard"[^>]*style="([^"]+)"/)[1], oldGround, "adding orbital capacity never moves a ground building");
    assert.ok(html.includes('class="deck-planet__selection is-space"'));
  }
  // §orbital-defense: the station hangs between the yards, as its level's art.
  for (let level = 1; level <= 5; level++) {
    orbitModel.body.structures.orbital_defense = level;
    const html = new deck.PlanetPanel().renderScene(orbitModel);
    assert.match(html, new RegExp(`id="planet-spot-orbital_defense"[^>]*class="deck-planet__building is-space is-station"[^>]*--planet-x:50%;--planet-y:${stationY(level)}%;--planet-scale:1;--station-level:${level}"`),
      "between the yards, sized by its level");
    assert.ok(html.includes(`/art/derived/stations/v1/level-${level}-256.webp`));
    assert.match(new deck.PlanetPanel().render(orbitModel), new RegExp(`Level ${level} of 5`), "a station has levels, not tiers");
    assert.equal(html.match(/id="planet-spot-shipyard"[^>]*style="([^"]+)"/)[1], oldGround, "the station never moves a ground building");
  }
  delete orbitModel.body.structures.orbital_defense;
  delete orbitModel.body.structures.naval_drydock;
  orbitModel.report.builds = [{ key: "naval_drydock", body_id: orbitModel.body.id, queued: true, complete_time: null }];
  const queued = new deck.PlanetPanel().renderScene(orbitModel);
  assert.match(queued, new RegExp(`id="planet-construction-naval_drydock"[^>]*is-space[^>]*--planet-x:26%;--planet-y:${yardY}%`));
  // The scene shows the painting extended with space; the founding screen keeps the 16:9 original.
  if (painted) {
    const folder = habitable ? "habitable-orbit-v1" : "orbit-v1";
    assert.match(queued, new RegExp(`class="deck-planet__surface" src="/art/derived/planet-workbench/${folder}/${surface}-768\\.webp"`));
    assert.match(deps["../../planet-surfaces"].planetSurfaceArt(surface).full, new RegExp(`/planet-workbench/${habitable ? "habitable-v1" : "v1"}/`));
  }
  assert.doesNotMatch(queued, /id="planet-spot-naval_drydock"/, "an order is not a completed station");
}
// §planet-orbit: fleets docked at this world hang in its sky, three at most and
// then "+N in orbit". The selected fleet comes first, then your own, then the
// larger hulls; capital ships sit by the Capital Slipway, other warships by the
// Orbital Shipyard, everything else between. A click opens the fleet.
{
  const sky = structuredClone(initialModel);
  const fleet = (id, kind, extra = {}) => ({ id, kind, own: true, count: 1, name: kind, selected: false, ...extra });
  sky.docked = [fleet("c1", "convoy"), fleet("r1", "battleship", { own: false, count: null }), fleet("d1", "cruiser", { count: 3 }),
    fleet("s1", "scout"), fleet("t1", "titan", { own: false, count: null })];
  const ships = (html) => [...html.matchAll(/class="deck-planet__orbit-ship([^"]*)" style="--planet-x:([\d.-]+)%;--planet-y:([\d.-]+)%[^"]*" data-deck-act="world-fleet-open" data-fleet="([^"]+)"/g)]
    .map(m => [m[4], Number(m[2]), Number(m[3]), m[1].trim()]);
  let html = new deck.PlanetPanel().renderScene(sky);
  assert.deepEqual(JSON.parse(JSON.stringify(ships(html))), [["d1", 38, -4, "is-own"], ["c1", 50, 3, "is-own"], ["s1", 62, -4, "is-own"]],
    "your own first, larger hulls first; warships by the shipyard, the rest between, overflow to the free slot");
  assert.match(html, /<span class="deck-planet__orbit-more"[^>]*>\+2 in orbit<\/span>/, "the rest fold into one tag");
  assert.match(html, /data-fleet="d1"[^>]*aria-label="cruiser ×3 · docked in orbit"[\s\S]*?<small>×3<\/small>/, "a known hull count shows");
  assert.match(html, /data-fleet="d1"[^>]*><img src="\/art\/derived\/ships\/models-v1\/previews\/cruiser-768\.webp"/, "the model's three-quarter render");
  sky.docked[1].selected = true;
  html = new deck.PlanetPanel().renderScene(sky);
  assert.deepEqual(JSON.parse(JSON.stringify(ships(html)))[0], ["r1", 62, -4, ""], "the selected fleet is always drawn; a capital ship sits by the slipway");
  assert.match(html, /data-fleet="r1" aria-pressed="true"/);
  sky.docked = [];
  assert.doesNotMatch(new deck.PlanetPanel().renderScene(sky), /deck-planet__orbit/, "an empty sky draws nothing");
  const giant = { ...structuredClone(initialModel), art: "/art/gas_giant-512.png", docked: [fleet("g1", "convoy")] };
  giant.body.kind = "gas_giant";
  assert.deepEqual(JSON.parse(JSON.stringify(ships(new deck.PlanetPanel().renderScene(giant)))), [["g1", 88, 36, "is-own"]],
    "a gas giant's globe fills the middle, so its sky slots sit in the corners");
}
// Occupied artwork follows only the reported, body-local ship queue. It is a
// generic hull-in-berth indicator, not an estimate of hull class or progress.
const occupiedModel = structuredClone(initialModel), occupiedPanel = new PlanetPanel();
Object.assign(occupiedModel.body.structures, { naval_drydock: 1, capital_slipway: 1 });
occupiedModel.report.builds = [];
assert.doesNotMatch(occupiedPanel.renderScene(occupiedModel), /-occupied-/);
const orbitalJob = { key: "large_freighter", body_id: occupiedModel.body.id, yard: "naval_drydock",
  queued: false, start_time: 10, complete_time: 20, work: { fraction: .4, per_second: .1, at_time: 15 } };
for (let tier = 1; tier <= 6; tier++) {
  occupiedModel.body.structures.naval_drydock = tier;
  for (const state of [{}, { queued: true, start_time: null, complete_time: null },
    { complete_time: null, work: { fraction: .4, per_second: 0, at_time: 15 } }, { complete_time: -1 }]) {
    occupiedModel.report.builds = [{ ...orbitalJob, ...state }];
    const html = occupiedPanel.renderScene(occupiedModel);
    assert.match(html, new RegExp(`naval_drydock/tier-${tier}-occupied-256.webp`));
    assert.match(html, /aria-label="Inspect Orbital Shipyard · in orbit · ship under construction"/);
    assert.doesNotMatch(html, /capital_slipway\/tier-\d+-occupied/);
    assert.doesNotMatch(occupiedPanel.renderScene({ ...occupiedModel, mine: false }), /-occupied-/);
  }
}
for (const other of [{ ...orbitalJob, body_id: occupiedModel.body.id + 1 },
  { ...orbitalJob, yard: "shipyard" }, { ...orbitalJob, yard: undefined },
  { ...orbitalJob, key: "naval_drydock" }, { ...orbitalJob, key: "recon_suite" },
  { ...orbitalJob, key: "titan", yard: "capital_slipway" }]) {
  occupiedModel.report.builds = [other];
  assert.doesNotMatch(occupiedPanel.renderScene(occupiedModel), /-occupied-/, "other worlds, yards, legacy ground freight, upgrades and modules are not orbital shipbuilding");
}
occupiedModel.report.builds = [{ ...orbitalJob, key: "destroyer", yard: undefined }];
assert.match(occupiedPanel.renderScene(occupiedModel), /-occupied-/, "legacy line-warship reports still identify their orbital yard");
occupiedModel.report.builds = [orbitalJob, { ...orbitalJob, key: "heavy_freighter" }];
occupiedModel.report.builds.shift();
assert.match(occupiedPanel.renderScene(occupiedModel), /-occupied-/, "one remaining job keeps the berth occupied");
occupiedModel.report.builds = [];
assert.doesNotMatch(occupiedPanel.renderScene(occupiedModel), /-occupied-/, "a received empty queue restores idle artwork");
// A command center is a no-workforce physical building. Its sprite and perks
// advance only with the received tier, never when an upgrade is ordered.
const commandModel = structuredClone(initialModel);
commandModel.catalog.push(commandCenter);
commandModel.body.structures.command_center = 1;
const commandPanel = new PlanetPanel();
commandPanel.focus("command_center");
assert.match(commandPanel.render(commandModel), /Landing Command/);
assert.match(commandPanel.render(commandModel), /data-select="command_center"/);
assert.match(commandPanel.renderScene(commandModel), /buildings\/command_center\/tier-1-256.webp/);
commandModel.report.builds.push({ key: "command_center", body_id: commandModel.body.id, queued: true });
assert.match(commandPanel.renderScene(commandModel), /buildings\/command_center\/tier-1-256.webp/);
assert.doesNotMatch(commandPanel.renderScene(commandModel), /buildings\/command_center\/tier-2-256.webp/);
const commandLayout = new PlanetSites();
const commandSlot = commandLayout.snapshot(commandModel).occupied.find(s => s.key === "command_center").slot;
for (const tier of [2, 3]) {
  commandModel.body.structures.command_center = tier;
  commandModel.body.infrastructure_slots = 6 + tier - 1;
  commandModel.report.builds = [];
  const html = commandPanel.render(commandModel);
  assert.ok(html.includes(commandCenter.tiers[tier - 1].label));
  assert.ok(html.includes(`${tier === 2 ? 10 : 20}% shorter`));
  assert.equal(commandLayout.snapshot(commandModel).occupied.find(s => s.key === "command_center").slot, commandSlot);
  const usage = marketDerive.bodyPoolUsage(commandModel.body, commandModel.report);
  assert.equal(commandLayout.snapshot(commandModel).free.filter(s => s.pool === "infrastructure").length,
    usage.infrastructure.total - usage.infrastructure.used, "the command footprint does not spend a regular plot");
}
assert.doesNotMatch(commandPanel.render(commandModel), /data-select="command_center"/, "headquarters has no fourth-level upgrade button");
assert.equal(planetStructures(commandModel).find(s => s.key === "command_center").staffable, false);
// Slots are capacity, not a truth shortcut or a new city-placement mechanic.
// Reconcile only received jobs/buildings; hold the same address through growth,
// queued→built, upgrades, cancellation and reload. Preferences are cosmetic.
const storageValues = new Map();
const storage = { getItem: key => storageValues.get(key) ?? null, setItem: (key, value) => storageValues.set(key, value) };
const sites = new PlanetSites(storage), siteModel = structuredClone(initialModel);
let siteView = sites.snapshot(siteModel);
const initialPositions = Object.fromEntries(siteView.occupied.map(site => [site.key, site.slot]));
const usage = marketDerive.bodyPoolUsage(siteModel.body, siteModel.report);
for (const pool of ["resource", "industrial", "infrastructure"]) {
  assert.equal(siteView.free.filter(site => site.pool === pool).length, usage[pool].total - usage[pool].used);
}
assert.equal(initialPositions.mining_complex, 0, "existing terrain locations are preserved");
const oldFree = siteView.free.map(site => [site.pool, site.slot]);
siteModel.body.industrial_slots++;
siteView = sites.snapshot(siteModel);
assert.ok(oldFree.every(([pool, slot]) => siteView.free.some(s => s.pool === pool && s.slot === slot)), "growth only reveals more sites");
assert.deepEqual(Object.fromEntries(siteView.occupied.map(site => [site.key, site.slot])), initialPositions);
const academySite = siteView.free.find(site => site.pool === "infrastructure");
sites.prefer(siteModel, "academy", academySite.slot);
assert.ok(sites.snapshot(siteModel).free.some(site => site.slot === academySite.slot), "sending a build does not consume a slot before its report arrives");
assert.ok(!sites.snapshot(siteModel).occupied.some(site => site.key === "academy"));
siteModel.report.builds.push({ key: "academy", body_id: siteModel.body.id, queued: true, complete_time: null });
assert.equal(sites.snapshot(siteModel).occupied.find(site => site.key === "academy").slot, academySite.slot);
siteModel.body.structures.academy = 1;
siteModel.report.builds = siteModel.report.builds.filter(job => job.key !== "academy");
assert.equal(sites.snapshot(siteModel).occupied.find(site => site.key === "academy").slot, academySite.slot, "completion uses the outline's address");
siteModel.report.builds.push({ key: "academy", body_id: siteModel.body.id, queued: true, complete_time: null });
const upgraded = sites.snapshot(siteModel);
assert.equal(upgraded.occupied.filter(site => site.key === "academy").length, 1, "upgrades deepen in place");
assert.equal(new PlanetSites(storage).snapshot(siteModel).occupied.find(site => site.key === "academy").slot, academySite.slot, "reload preserves the chosen site");
const fullModel = structuredClone(siteModel);
// Stress every available address (24, more than the real per-world pool caps).
// Omit two optional military yards, the Gaia-only Living Terraces, the
// slot-free Power Plant and the fuel overhaul's two optional fuel works to
// include storage and local administration without constructing an impossible
// world that exceeds the 24 terrain sites.
for (const key of Object.keys(fullModel.descriptions).filter(key => !["capital_slipway", "ordnance_foundry", "living_terraces", "power_plant", "uranium_mill", "antimatter_plant"].includes(key))) fullModel.body.structures[key] = 1;
const allOccupied = new PlanetSites(storage).snapshot(fullModel).occupied;
assert.equal(allOccupied.length, 24, "even the all-buildings preview reuses empty reservations instead of overlapping built structures");
assert.equal(new Set(allOccupied.map(site => site.slot)).size, 24);
assert.ok(allOccupied.some(site => site.key === "warehouse"), "the founding store receives a terrain site, including when empty sites must be reused");
assert.ok(!allOccupied.some(site => site.key === "service_berth"), "the Warehouse attachment does not occupy a second planetary footprint");
assert.ok(allOccupied.some(site => site.key === "command_center"), "local command has a physical sprite even though it uses no development slot");
assert.ok(Object.entries(initialPositions).every(([key, slot]) => allOccupied.find(site => site.key === key)?.slot === slot), "reclaiming empty plots never moves an existing building");
// All 27 non-Gaia kinds fit, the Power Plant and the two fuel works aside: 24
// ground sites, two starfield yards, one Warehouse attachment. (A real world's
// slot pools stay far below 24 buildings.)
const everyStructure = structuredClone(initialModel);
everyStructure.body.structures = Object.fromEntries(Object.keys(descriptions).filter(key => !["living_terraces", "power_plant", "uranium_mill", "antimatter_plant"].includes(key)).map(key => [key, 1]));
everyStructure.report.builds = [];
const everySite = new PlanetSites().snapshot(everyStructure);
assert.equal(everySite.occupied.length, 26);
assert.equal(everySite.occupied.filter(site => site.slot < 24).length, 24);
assert.equal(new Set(everySite.occupied.map(site => site.slot)).size, 26);
const everyPanel = new PlanetPanel(), everyScene = everyPanel.renderScene(everyStructure);
const warehouseStyle = everyScene.match(/id="planet-spot-warehouse"[^>]*style="([^"]+)"/)[1];
assert.equal(everyScene.match(/id="planet-spot-service_berth"[^>]*style="([^"]+)"/)[1], warehouseStyle, "Service Berth shares the Warehouse address");
assert.match(everyScene, /id="planet-spot-service_berth"[^>]*is-attachment/);
assert.equal((everyScene.match(/class="planet-structure-ground"/g) ?? []).length, 24);
everyPanel.handleAction({ deckAct: "planet-select", structure: "service_berth" }, everyStructure, () => {});
assert.match(everyPanel.render(everyStructure), /id="planet-row-service_berth"[^>]*aria-pressed="true"/);
assert.match(everyPanel.render(everyStructure), /aria-label="Selected structure"[\s\S]*Service Berth/);
// Migrate a saved cosmetic layout which previously assigned orbital yards a
// terrain plot. Existing ground positions survive, and slot accounting stays
// tied to the actual report, not to the number of illustrated installations.
const migrationModel = structuredClone(initialModel);
migrationModel.body.structures = { warehouse: 1, naval_drydock: 1, capital_slipway: 1 };
migrationModel.report.builds = [];
const legacyLayout = { sites: [
  { slot: 3, pool: "infrastructure", key: "warehouse" },
  { slot: 4, pool: "industrial", key: "naval_drydock" },
  { slot: 5, pool: "industrial", key: "capital_slipway" },
], preferred: {} };
let savedLayout;
const migrated = new PlanetSites({ getItem: () => JSON.stringify(legacyLayout), setItem: (_, value) => { savedLayout = JSON.parse(value); } });
const migrationView = migrated.snapshot(migrationModel);
assert.equal(migrationView.occupied.find(s => s.key === "warehouse").slot, 3);
assert.equal(migrationView.occupied.find(s => s.key === "naval_drydock").slot, 24);
assert.equal(migrationView.occupied.find(s => s.key === "capital_slipway").slot, 25);
assert.ok(savedLayout.sites.every(s => !["naval_drydock", "capital_slipway"].includes(s.key)));
assert.equal(migrationView.free.filter(s => s.pool === "industrial").length,
  marketDerive.bodyPoolUsage(migrationModel.body, migrationModel.report).industrial.total - 2, "orbital visual relocation grants no free industrial capacity");
siteModel.report.builds = [];
assert.ok(!sites.snapshot(siteModel).occupied.some(site => site.key === "fuel_refinery"), "cancelled new construction releases its site");
assert.equal(sites.snapshot({ ...siteModel, mine: false }).free.length, 0);
const sitePanel = new PlanetPanel();
// §build-sheet: free plots show for their owner as quiet + marks that name their kind on hover.
assert.match(sitePanel.renderScene(initialModel), /class="deck-planet__site is-free is-industrial"[^>]*data-deck-act="world-build-site"[^>]*><b>\+<\/b><small>Free Industrial site<\/small>/,
  "a free plot is a + mark naming its kind");
assert.doesNotMatch(sitePanel.renderScene(initialModel), /is-match|is-dim/, "nothing is lit outside placing");
assert.match(sitePanel.renderScene(initialModel), /id="planet-construction-fuel_refinery"/, "reported new construction gets an outline");
const controller = new PlanetPanel(), sent = [];
const click = (action, data = {}) => controller.handleAction({ deckAct: action, ...data }, model, command => sent.push(structuredClone(command)));
// One controller, two surfaces: the scene over the map and the workspace column.
const render = () => controller.renderScene(model) + controller.render(model);
let html = render();
const worldsModel = structuredClone(initialModel);
worldsModel.report.bodies.push({ ...worldsModel.body, id: 4, name: "Freya IV <surveyed>" });
const worldsHtml = new PlanetPanel().renderScene(worldsModel);
assert.match(worldsHtml, /aria-label="Worlds in Freya"/);
assert.match(worldsHtml, /id="planet-world-3"[^>]*aria-current="page">Freya III/);
assert.match(worldsHtml, /id="planet-world-4"[^>]*>Freya IV &lt;surveyed&gt;/);
assert.equal((worldsHtml.match(/data-deck-act="world-switch"/g) ?? []).length, 2, "the strip lists the served worlds only");
assert.equal((worldsHtml.match(/aria-current="page"/g) ?? []).length, 1);
const visual = controller.renderScene(model), controls = controller.render(model);
assert.match(visual, /class="deck-planet-scene"[\s\S]*id="planet-worlds"[\s\S]*class="deck-planet__scene"/, "the scene carries the world strip and the artwork");
assert.match(visual, /class="deck-planet__canvas"><img class="deck-planet__surface"[\s\S]*class="deck-planet__building"[\s\S]*class="deck-planet__selection"/, "art, hit targets and selected badges share the same canvas");
const firstMine = footprintAddress("desert", 0);
assert.ok(visual.includes(`id="planet-spot-mining_complex" type="button" class="deck-planet__building" style="--planet-x:${firstMine.x}%;--planet-y:${firstMine.y}%;--planet-scale:${firstMine.scale}"`), "the opening mine takes the painting's first generated address at its depth scale");
// Every painting has its own 24 addresses on open land, generated from the master
// by scripts/build-planet-sites.mjs; a changed master needs `npm run assets:sites`.
const { PLANET_SITES } = deps["../../planet-sites.generated"];
const siteGenerator = await import("./build-planet-sites.mjs");
for (const [surface, { file }] of Object.entries(siteGenerator.PAINTINGS)) {
  const master = readFileSync(new URL(`../art-src/planet-view/${file}`, import.meta.url));
  assert.equal(PLANET_SITES[surface]?.source, createHash("sha256").update(master).digest("hex"), `${surface}: src/planet-sites.generated.ts is stale; run npm run assets:sites and review the overlays`);
}
assert.deepEqual(siteGenerator.HIT, { w: .8, h: .72 }, "the generator spaces the same hit box the CSS draws");
for (const y of [40, 47, 55, 63.4, 70, 85, 92]) assert.equal(deck.siteScale(y), Math.round(100 * siteGenerator.siteScale(y)) / 100, "the stage scales footprints exactly as the generator spaced them");
// Footprints stay inside the painting and below the limb haze (~19% at the centre,
// ~32% at the edges). A sprite is 7.5% of the plane's width (asserted above), 13.3%
// of its height, times its depth scale; hit boxes (80% × 72% of it) never overlap.
const planetSurfaces = ["desert", "terrestrial", "ice", "lava", "barren", "ocean", "marginal", ...deps["../../planet-surfaces"].HABITABLE_SURFACES];
for (const surface of [...planetSurfaces, null]) {
  if (surface !== null) assert.equal(PLANET_SITES[surface]?.sites.length, 24, `${surface} has 24 generated addresses`);
  const spots = Array.from({ length: 24 }, (_, slot) => footprintAddress(surface, slot));
  for (const { x, y, scale } of spots) {
    const limb = 19 + 13 * ((x - 50) / 50) ** 2, halfW = 7.5 * scale / 2, halfH = 7.5 * 16 / 9 * scale / 2;
    assert.equal(scale, surface === null ? 1 : deck.siteScale(y), `${surface} address ${x},${y} recedes with its depth`);
    assert.ok(y - halfH >= limb + 3, `${surface} address ${x},${y} pokes into the limb haze (${limb.toFixed(1)})`);
    assert.ok(x - halfW >= 0 && x + halfW <= 100 && y + halfH <= 100, `${surface} address ${x},${y} leaves the painting`);
  }
  spots.forEach((a, i) => spots.slice(i + 1).forEach((b) => {
    const s = (a.scale + b.scale) / 2;
    assert.ok(Math.abs(a.x - b.x) >= .8 * 7.5 * s || Math.abs(a.y - b.y) >= .72 * 7.5 * 16 / 9 * s, `${surface} addresses ${a.x},${a.y} and ${b.x},${b.y} overlap`);
  }));
}
for (const surface of planetSurfaces) {
  const { rough } = PLANET_SITES[surface];
  // Any painting with room for the opening colony keeps it on clean open land;
  // paintings short of room report the rest as rough (the ocean painting has room
  // for fewer than six buildings and needs an art edit).
  if (24 - rough.length >= siteGenerator.OPENING.length) assert.ok(siteGenerator.OPENING.every(slot => !rough.includes(slot)), `${surface} keeps its opening colony on open land`);
}
assert.notDeepEqual(footprintAddress("ocean", 7), footprintAddress("desert", 7), "each painting uses its own addresses");
// All six new landscapes reach the real scene, not just the art catalogue.
for (const surface of deps["../../planet-surfaces"].HABITABLE_SURFACES) {
  const habitat = structuredClone(initialModel);
  habitat.body.habitable = true;
  habitat.body.kind = surface === "archipelago-world" ? "ocean" : "terrestrial";
  habitat.art = "/art/terrestrial-512.png";
  let id = 0;
  while (planetSurface(habitat.art, habitat.system.id, { ...habitat.body, id }) !== surface) {
    assert.ok(id++ < 100, `reachable ${surface}`);
  }
  habitat.body.id = id;
  habitat.report.bodies = [habitat.body];
  habitat.report.builds = [];
  const panel = new PlanetPanel();
  const scene = panel.renderScene(habitat);
  assert.ok(scene.includes(`/habitable-orbit-v1/${surface}-1672.webp 1672w`), "the scene shows the painting with orbit space added");
  const positions = text => [...text.matchAll(/id="planet-spot-([^\"]+)"[^>]*style="([^\"]+)"/g)].map(m => [m[1], m[2]]);
  habitat.body.structures.mining_complex++;
  habitat.body.population += .001;
  habitat.body.name = "Renamed world";
  assert.deepEqual(positions(panel.renderScene(habitat)), positions(scene), "new art preserves footprints through upgrades and reports");
  assert.deepEqual(positions(new PlanetPanel().renderScene(habitat)), positions(scene), "reopening preserves the art coordinates");
  assert.doesNotMatch(panel.renderScene({ ...habitat, mine: false }), /planet-spot-|planet-construction-|world-build-site/, "new art cannot invent rival structures");
  assert.doesNotMatch(scene, /deck-planet__siege/, "no siege overlay without a served siege");
  const siege = { caption: "UNDER BLOCKADE · SIEGE 40%", detail: "GARRISON PINNED 37%", suppression: .37, landingId: "landing-1" };
  const besieged = panel.renderScene({ ...habitat, siege });
  assert.match(besieged, /class="deck-planet__siege"[\s\S]*UNDER BLOCKADE · SIEGE 40%[\s\S]*data-deck-act="watch-ground" data-ground="landing-1"/, "the besieged world shows its banner and follows the landing");
  assert.equal((besieged.match(/deck-planet__impact/g) ?? []).length, 4, "bombardment density follows the pinned garrison");
  assert.equal((besieged.match(/deck-planet__lander/g) ?? []).length, 3, "troop craft drop while marines are down");
  assert.equal(panel.renderScene({ ...habitat, siege }), besieged, "stable markup keeps the overlay's animations running across reports");
  assert.doesNotMatch(panel.renderScene({ ...habitat, siege: { ...siege, suppression: 0, landingId: null } }), /deck-planet__impact|deck-planet__lander|watch-ground/,
    "no guns, no landing → only the banner");
}
assert.doesNotMatch(visual, /workforce-summary|class="deck-tabs|id="planet-page"|deck-construction|planet-workers|planet-review|planet-narrow|planet-wide/, "no management control lives on the map scene");
assert.match(controls, /class="deck-page deck-planet"[\s\S]*deck-planet__lead[\s\S]*<h2>Freya III<\/h2>[\s\S]*Information delay <b>~17s<\/b>[\s\S]*class="deck-construction"[\s\S]*workforce-summary[\s\S]*class="deck-tabs[\s\S]*id="planet-page"/,
  "§construction: the construction bar sits right under the world's header, above workforce and the structure tabs");
assert.doesNotMatch(controls, /deck-planet__canvas|deck-planet__building|world-switch/, "the workspace column never repeats the scene or the world strip");
assert.doesNotMatch(controls, /deck-planet__development/, "nothing about construction is left at the bottom of the column");
assert.deepEqual([...controls.matchAll(/data-deck-act="planet-tab" data-tab="([^"]+)"/g)].map(match => match[1]), ["structures", "population", "survey"], "production and structures share one management tab");
assert.match(controls, /data-tab="structures" aria-selected="true"/, "Structures is the default planet tab");
assert.equal((controls.match(/class="deck-planet__row"/g) ?? []).length, 4, "the merged list includes production, unstaffed buildings and automatic infrastructure");
const miningRow = controls.match(/<button id="planet-row-mining_complex"[\s\S]*?<\/button>/)?.[0];
assert.match(miningRow, /aria-label="\+24 Ferrite Ore per minute"/);
assert.match(miningRow, /class="deck-planet__staffed-value">2 [\s\S]*alt="Workforce"/, "the same row shows output and assigned workforce");
const smelterRow = controls.match(/<button id="planet-row-smelter"[\s\S]*?<\/button>/)?.[0];
assert.match(smelterRow, /No workforce[\s\S]*aria-label="\+0 Alloys per minute"[\s\S]*class="deck-planet__staffed-value">0 /);
assert.match(controls, /id="planet-row-habitat"/, "non-production structures remain selectable");
assert.match(html, /aria-label="\+24 Ferrite Ore per minute"/);
assert.match(html, /aria-label="\+6 Silicates per minute"/);
assert.match(html, /2 available in system/);
assert.match(html, /Workforce <b>2<\/b> assigned here/);
assert.equal((html.match(/class="deck-planet__building"/g) ?? []).length, 4, "only reported built structures get footprints");
assert.doesNotMatch(html, /id="planet-spot-fuel_refinery"/, "queued buildings aren't already present");
assert.match(html, /title="Ferrite Ore"/);
assert.match(html, /title="Silicates"/);
assert.match(html, /alt="Workforce" title="Workforce"/);
assert.match(html, /orbit-v1\/desert-1536.webp 1536w/, "large art gets a high-DPI source");
assert.match(visual, /id="planet-spot-mining_complex"[^>]*><img class="planet-structure-ground"[^>]*grounds\/desert-256.webp[^>]*><img class="icon planet-structure-art"[^>]*srcset="[^"]*mining_complex\/tier-4-512\.webp 512w"/, "footprints pair this biome's terrain with the reported building tier and a high-DPI sprite");
assert.match(miningRow, /mining_complex\/tier-4-128\.webp/, "the list and planet scene use the same tier");
assert.match(controls.match(/<aside id="planet-structure-detail"[\s\S]*?<\/aside>/)[0], /mining_complex\/tier-4-128\.webp/, "selected detail uses the same reported tier");
const upgrading = structuredClone(initialModel), upgradePanel = new PlanetPanel();
upgrading.report.builds.push({ key: "mining_complex", body_id: 3, complete_time: 1, queued: false });
assert.match(upgradePanel.renderScene(upgrading), /mining_complex\/tier-4-256\.webp/, "an upgrade whose estimate expired cannot change the built artwork");
assert.doesNotMatch(upgradePanel.renderScene(upgrading), /mining_complex\/tier-5-/, "future artwork is confined to the builder preview");
upgrading.body.structures.mining_complex = 5;
assert.match(upgradePanel.renderScene(upgrading), /mining_complex\/tier-5-256\.webp/, "only the arrived tier report upgrades the footprint");
assert.doesNotMatch(controls, /planet-surface-structures\/v2|planet-structure-ground|9\.15vw/, "list/detail panels keep the established icons; new art is scene-only");
assert.deepEqual(JSON.parse(JSON.stringify(planetStructures(model).find(item => item.key === "smelter").outputs)), [["alloys", 0]], "unstaffed converters still appear at zero");
const positions = planetStructures(model).map(({ key, x, y }) => [key, x, y]);
body.structures.academy = 1;
assert.deepEqual(planetStructures(model).filter(item => item.key !== "academy").map(({ key, x, y }) => [key, x, y]), positions);
delete body.structures.academy;

click("planet-workers", { delta: "1" });
assert.equal(sent.length, 0, "stepper only edits a local draft");
html = render();
assert.match(html, /aria-label="Planned workforce">3</);
assert.match(html, /Mining Complex: 2 workforce assigned/);
assert.match(html, /aria-label="\+24 Ferrite Ore per minute"/);
// The change is reviewed in place, directly beneath the stepper, before
// anything is sent: no separate review step and nothing to scroll to.
const detail = html.match(/<aside id="planet-structure-detail"[\s\S]*?<\/aside>/)[0];
assert.match(detail, /aria-label="Planned workforce">3<\/b><button[^>]*>\+<\/button><\/div><\/div><div class="deck-planet__confirm"/,
  "the review sits directly beneath the stepper");
assert.match(detail, /2 → 3 workforce[\s\S]*Order travel ~17s[\s\S]*data-deck-act="planet-confirm">Confirm</);
assert.doesNotMatch(detail, /Review assignment|No changes/, "no separate review step");
assert.equal(sent.length, 0);
click("planet-confirm");
assert.deepEqual(sent[0], { type: "SetAssignment", system_id: "home", body_id: 3, structure: "mining_complex", workers: 3, specialists: { engineer: 1 } });
click("planet-confirm");
assert.equal(sent.length, 1, "double confirmation sends once");
html = render();
assert.match(html, /3 workforce requested · awaiting report/);
assert.match(html, /Mining Complex: 2 workforce assigned/);
assert.match(html, /aria-label="\+24 Ferrite Ore per minute"/);
line.workers = 3;
line.outputs = [["metallic_ore", .6], ["silicates", .15]];
html = render();
assert.doesNotMatch(html, /3 workforce requested/);
assert.match(html, /Mining Complex: 3 workforce assigned/);
assert.match(html, /aria-label="\+36 Ferrite Ore per minute"/);
assert.match(html, /aria-label="\+9 Silicates per minute"/);

click("planet-workers", { delta: "1" });
click("planet-review");
line.workers = 2; // A newly arrived assignment invalidates an older review.
click("planet-confirm");
assert.equal(sent.length, 1);
html = render();
assert.match(html, /2 → 4 workforce[\s\S]*A newer report arrived[\s\S]*data-deck-act="planet-review">Update</,
  "a review overtaken by a newer report says so and offers Update, not Confirm");
assert.doesNotMatch(html, /data-deck-act="planet-confirm"/);
click("planet-review");
html = render();
assert.match(html, /2 → 4 workforce[\s\S]*data-deck-act="planet-confirm">Confirm</, "updating re-reviews the change against the new report");
assert.doesNotMatch(html, /A newer report arrived/);
click("planet-workers", { delta: "-1" });
click("planet-workers", { delta: "-1" });
assert.doesNotMatch(render(), /deck-planet__confirm/, "stepping back to the reported number closes the review");
click("planet-select", { structure: "smelter" });
assert.match(render(), /aria-label="\+0 Alloys per minute"/);
click("planet-workers", { delta: "999" });
assert.match(render(), /aria-label="Planned workforce">0</);
click("planet-select", { structure: "habitat" });
assert.doesNotMatch(render().match(/<aside id="planet-structure-detail"[\s\S]*?<\/aside>/)[0], /planet-workers|planet-review/);
click("planet-tab", { tab: "population" });
assert.match(render(), /Population controls/);
click("planet-confirm");
assert.equal(sent.length, 1, "no hidden confirmation under a different tab");
click("planet-tab", { tab: "survey" });
assert.match(render(), /Survey report/);

const focused = new PlanetPanel();
focused.focus("smelter");
assert.match(focused.render(model), /id="planet-row-smelter"[^>]*aria-pressed="true"/, "a production-line deep link opens the world on its structure");
focused.handleAction({ deckAct: "planet-tab", tab: "survey" }, model, () => {});
focused.focus("habitat");
assert.match(focused.render(model), /data-tab="structures" aria-selected="true"[\s\S]*id="planet-row-habitat"[^>]*aria-pressed="true"/, "a deep link from a non-management tab lands on Structures");

line.suspended = "no_inputs";
assert.ok(planetStructures(model).find(item => item.key === "mining_complex").outputs.every(([, rate]) => rate === 0), "a stopped line doesn't advertise its rated output as actual output");
model.mine = false;
html = render();
assert.doesNotMatch(html, /planet-spot-|planet-row-|planet-workers|planet-review|assigned here|Population controls|deck-construction|Queue here/);
assert.match(html, /Survey report/);
click("planet-tab", { tab: "structures" });
click("planet-select", { structure: "mining_complex" });
click("planet-workers", { delta: "1" });
click("planet-review");
click("planet-confirm");
assert.equal(sent.length, 1, "rivals never expose management even if stale buttons survive");
model.mine = true;
model.body = { ...body, id: 5, name: '<img src=x onerror="bad">', structures: {} };
html = render();
assert.doesNotMatch(html, /<img src=x|planet-spot-/);
assert.match(html, /No structures built/);
assert.match(html, /class="deck-construction"/, "an undeveloped world still offers construction");
assert.equal(planetSurface("/art/derived/planets/2026-09-08/gas_giant-512.png"), null);
for (const kind of ["terrestrial", "ocean", "ice", "lava", "desert"]) assert.equal(planetSurface(`/art/${kind}-512.png`), kind);
assert.equal(planetSurface("/art/moon-512.png"), "barren");
assert.match(empireSource, /setHtml\(this\.stageRoot, this\.planet\.renderScene\(model\)\)/);
assert.match(empireSource, /setHtml\(this\.root, this\.planet\.render\(model\)\)/);
assert.match(empireSource, /this\.planet\.handleAction\(button\.dataset/);
assert.doesNotMatch(empireSource, /set-workers|type: "SetAssignment"/, "the system rail no longer sends workforce directives; the world's review → confirm flow is the only editor");
assert.match(empireSource, /data-deck-act="open-world" data-body="\$\{line\.body_id\}" data-structure="\$\{esc\(line\.structure\)\}"/, "production lines link down to their world and structure");
assert.doesNotMatch(empireSource, /\/s \$\{esc\(label\(commodity\)\)\}|\$\{label\(commodity\)\}\/s/, "rates use one per-minute unit everywhere");
assert.match(empireSource, /this\.worldBuildRoute\s*\?\s*`<b>\$\{esc\(body\.name\)\}<\/b>`/, "a child builder shows its fixed site instead of a second world strip");

// Exercise the real empire controller on the world route. Only the catalogue
// markup is stubbed; routing, selection, dispatch and refresh are real.
function checkWorldRoute(source) {
  const doc = { activeElement: null };
  class Element {
    constructor(id, parent = null) { this.id = id; this.parent = parent; this.hidden = false; this.isConnected = true; this.html = ""; this.scrollTop = 0; this.dataset = {}; this.queries = new Map(); this.attributes = new Map(); }
    closest(selector) { return selector === `#${this.id}` ? this : this.parent?.closest(selector) ?? null; }
    contains(node) { return !!node && (node === this || this.contains(node.parent)); }
    querySelector(selector) { return this.queries.get(selector) ?? null; }
    setAttribute(name, value) { this.attributes.set(name, value); }
    focus() { doc.activeElement = this; }
  }
  const originRoot = new Element("deck-workspace-body");
  const stagePanel = new Element("deck-planet-stage"), stageRoot = new Element("deck-planet-stage-body", stagePanel);
  stagePanel.hidden = true;
  const buildPanel = new Element("deck-build-workbench"), buildRoot = new Element("deck-build-workbench-body", buildPanel);
  buildPanel.queries.set("#deck-build-workbench-title", new Element("title"));
  const fixture = structuredClone(initialModel), navigation = [], dispatched = [], deferred = [];
  fixture.report.bodies = [fixture.body, { ...fixture.body, id: 4, name: "Freya IV" }];
  const state = { playerId: "1", simTime: 100, commandCenter: { x: 0, y: 0 }, galaxy: { systems: [fixture.system], build_options: fixture.catalog }, systems: [fixture.report], timeline: [], ghosts: [], pendingOrders: new Map(), battles: [], research: { programmes: [] } };
  let deferBuild = false;
  deps["../dom"] = { setHtml(node, html) { node.html = html; }, renderDeferred(id, callback) { if (deferBuild && id === buildRoot.id) { deferred.push(callback); return true; } return false; } };
  deps["../signature"] = { sheetFingerprint: JSON.stringify };
  deps["../../core/derive/fleet"] = { dockedAtSystem: (g, id) => g.docked === id || g.docked === `E${id}`, shipKindLabel: kind => kind };
  let simNow = 100;
  deps["../../state"] = { liveSimTime: () => simNow };
  deps["./planet"] = { PlanetPanel };
  deps["../../core/derive/market"] = { ...marketDerive,
    structureResearched: entry => !entry.research_prerequisite,
    // The sim's tier rule (built + orders + 1) without stock or research gates.
    structOption: (o, dyn, b) => {
      const current = b.structures[o.key] ?? 0, orders = dyn.builds.filter(j => j.body_id === b.id && j.key === o.key).length;
      return { o, pool: marketDerive.POOL_OF[o.key], currentTier: current, targetTier: current + orders + 1, foundsNew: !current && !orders,
        maxed: false, poolFull: false, noDeposit: false, yardPrereq: null, afford: true, buildable: true, reason: "" };
    },
    dispatchBuildKey: (...args) => dispatched.push(args) };
  deps["../../core/derive/fleet"].constructionStock = () => ({ available: new Map() });
  deps["../../core/derive/geo"] = { commandDelayTo: () => 17 };
  const coached = [], commands = [];
  deps["../coach"] = { coach: (...args) => coached.push(args) };
  const microtasks = [];
  const { DeckEmpireRoutes } = compile("", source, { document: doc, HTMLElement: Element, requestAnimationFrame: callback => callback(),
    queueMicrotask: callback => microtasks.push(callback) });
  const ui = new DeckEmpireRoutes(originRoot, buildRoot, stageRoot, { state, renderer: { pulseSystemBody() {} },
    send(command) { if (command.type !== "CancelBuild") throw Error("unexpected directive"); commands.push(command); } },
    { go: route => navigation.push(["go", route]), replace: route => navigation.push(["replace", route]),
      openWorldBuild: (systemId, bodyId, query) => navigation.push(["openWorldBuild", systemId, bodyId, query]), notice() {}, toast() {} });
  ui.planetModel = (system, report, body) => ({ ...fixture, system, report, body, mine: report.owner === state.playerId,
    ...ui.constructionModel(system, report, body) });
  fixture.catalog.push({ key: "academy", label: "Academy", costs: [], build_secs: 20 });
  ui.systemHtml = () => "Originating Worlds tab";
  ui.buildHtml = (route, system, report) => { ui.builderMode = route.query.mode; return JSON.stringify([route, report.stockpile, state.research]); };
  const origin = { name: "system", params: { id: "home" }, query: { tab: "worlds" } };
  const world = { name: "world", params: { systemId: "home", systemLabel: "Freya", bodyId: "3", worldLabel: "Freya III" } };
  const world4 = { name: "world", params: { systemId: "home", systemLabel: "Freya", bodyId: "4", worldLabel: "Freya IV" } };
  const action = (root, deckAct, extra = {}) => { const button = new Element("", root); button.dataset = { deckAct, ...extra }; return button; };
  ui.render(origin, true);
  assert.equal(stagePanel.hidden, true, "no stage without a world route");
  ui.handleAction(action(originRoot, "open-world", { body: "3", structure: "smelter" }), origin);
  // Routes are built inside the compiled module's realm: compare shape, not prototype.
  const plain = value => JSON.parse(JSON.stringify(value));
  assert.deepEqual(plain(navigation.at(-1)), ["go", world], "a world is a route beneath its system");
  ui.render(world, true);
  assert.equal(stagePanel.hidden, false, "the world route raises the scene over the map");
  assert.equal(stagePanel.attributes.get("aria-label"), "Freya III planet view");
  assert.match(stageRoot.html, /class="deck-planet__canvas"/);
  assert.match(originRoot.html, /<h2>Freya III<\/h2>/);
  assert.match(originRoot.html, /id="planet-row-smelter"[^>]*aria-pressed="true"/, "the production-line deep link lands on its structure");
  ui.handleAction(action(stageRoot, "planet-select", { structure: "mining_complex" }), world);
  assert.match(stageRoot.html, /id="planet-spot-mining_complex"[^>]*aria-pressed="true"/, "a footprint click on the scene selects in the column too");
  assert.match(originRoot.html, /id="planet-row-mining_complex"[^>]*aria-pressed="true"/);
  state.ghosts = [{ id: "f9", kind: "cruiser", own: true, docked: "Ehome" }, { id: "x1", kind: "raider", own: true, docked: "E7" }];
  ui.handleAction(action(stageRoot, "world-fleet-open", { fleet: "f9" }), world);
  assert.deepEqual(plain(navigation.at(-1)), ["go", { name: "fleet", params: { id: "f9", fleetLabel: "cruiser fleet" } }],
    "a docked hull in the sky opens its fleet");
  assert.equal(state.selectedShipId, "f9", "and selects your own");
  const opened = navigation.length;
  ui.handleAction(action(stageRoot, "world-fleet-open", { fleet: "x1" }), world);
  assert.equal(navigation.length, opened, "a fleet docked at another system is not this world's");
  originRoot.scrollTop = 190;
  ui.planet.handleAction({ deckAct: "planet-workers", delta: "1" }, fixture, () => {});
  const settled = navigation.length;
  // §build-sheet: Build opens a sheet across this world's planet view; the
  // column keeps the world panel, the hull workbench stays shut and nothing navigates.
  const opener = action(originRoot, "world-build", { mode: "structures" });
  ui.handleAction(opener, world);
  assert.equal(stagePanel.hidden, false, "Build must not remove the planet scene");
  assert.equal(buildPanel.hidden, true, "structures build on the planet, not in the hull workbench");
  assert.match(stageRoot.html, /class="deck-planet-scene is-building"[\s\S]*class="deck-bsheet"[\s\S]*Build on Freya III[\s\S]*class="deck-bsheet__detail"/, "the sheet opens across the planet view");
  assert.match(originRoot.html, /<h2>Freya III<\/h2>[\s\S]*class="deck-construction is-building"[\s\S]*Fuel Refinery/, "the column keeps the world panel and its work slot in view");
  assert.doesNotMatch(originRoot.html, /deck-bsheet/);
  assert.equal(navigation.length, settled, "opening the builder never navigates the workspace");
  assert.match(stageRoot.html, /data-deck-act="world-builder-exit">Done</, "the sheet has its own Done");
  assert.doesNotMatch(stageRoot.html, /deck-planet__sites-toggle/, "the strip's Build waits until the sheet closes");
  ui.handleAction(action(stageRoot, "world-builder-tab", { pool: "industrial" }), world);
  assert.match(stageRoot.html, /aria-selected="true" data-deck-act="world-builder-tab" data-pool="industrial"/);
  assert.equal(marketDerive.POOL_OF[ui.selectedBuild], "industrial", "a tab picks one of its own buildings");
  ui.handleAction(action(stageRoot, "world-builder-pick", { key: "mining_complex" }), world);
  assert.match(stageRoot.html, /aria-selected="true" data-deck-act="world-builder-tab" data-pool="resource"/, "a pick opens its own slot type");
  assert.match(stageRoot.html, /class="deck-bsheet__level">IV → V</, "an upgrade card names the step");
  assert.match(stageRoot.html, /data-deck-act="world-builder-queue" data-key="mining_complex"[^>]*>Queue level V</, "the button names the exact level it orders");
  assert.match(stageRoot.html, /<dt>Output<\/dt><dd>×6 → <b>×8\.6<\/b><\/dd><dt>Crew<\/dt><dd>4 → <b>5<\/b><\/dd>/, "the sim's own figures for the next level");
  assert.match(stageRoot.html, /class="deck-bsheet__rung is-research" title="Levels V and VI need research"/, "levels V and VI are marked as research");
  ui.handleAction(action(stageRoot, "world-builder-queue", { key: "mining_complex" }), world);
  assert.deepEqual(plain(dispatched.at(-1)), ["mining_complex", "home", 3], "the order targets this world");
  assert.match(stageRoot.html, /disabled>Order sent · confirming</, "a sent order is acknowledged at once");
  assert.match(stageRoot.html, /class="deck-bsheet__status is-quiet">Order sent</, "on its card too");
  assert.match(stageRoot.html, /↑ V · order sent/, "and rides on its building in the scene");
  assert.match(originRoot.html, /Order sent · confirms in/, "the construction bar shows it in flight");
  const sentOnce = dispatched.length;
  ui.handleAction(action(stageRoot, "world-builder-queue", { key: "mining_complex" }), world);
  assert.equal(dispatched.length, sentOnce, "a second click while the order is in flight sends nothing");
  // The served report confirms it: the order joins the line, and the next
  // level is a separate, explicit order on the same card.
  fixture.report.builds.push({ id: 41, key: "mining_complex", body_id: 3, queued: true, complete_time: null });
  ui.render(world, true);
  assert.doesNotMatch(stageRoot.html, /Order sent · confirming/);
  assert.match(stageRoot.html, /class="deck-bsheet__status is-quiet">V queued #2</);
  assert.match(stageRoot.html, /data-deck-act="world-builder-queue" data-key="mining_complex"[^>]*>Queue level VI</);
  assert.equal(originRoot.scrollTop, 190);
  assert.equal(navigation.length, settled);
  ui.handleAction(action(stageRoot, "world-builder-exit"), world);
  assert.doesNotMatch(stageRoot.html, /class="deck-bsheet"/);
  assert.doesNotMatch(stageRoot.html, /is-building/);
  assert.match(originRoot.html, /aria-label="Planned workforce">3</, "planet workforce drafts survive construction");
  assert.match(originRoot.html, /<section class="deck-construction"[\s\S]*Fuel Refinery[\s\S]*1 queued · next <b>Mining Complex V<\/b>/,
    "the pinned bar shows the job holding the slot and what waits");
  assert.match(originRoot.html, /upgrading to V|upgrade to V queued #2/, "the building's own row says it is being upgraded");
  // The line opens; only a waiting order can be cancelled, and only after a confirm.
  ui.handleAction(action(originRoot, "world-queue-toggle"), world);
  assert.match(originRoot.html, /class="deck-construction__queue"[\s\S]*#2[\s\S]*Mining Complex V[\s\S]*data-deck-act="world-cancel" data-job="41"/);
  ui.handleAction(action(originRoot, "world-cancel", { job: "41" }), world);
  assert.equal(commands.length, 0, "Cancel asks first");
  ui.handleAction(action(originRoot, "world-cancel-confirm", { job: "41" }), world);
  assert.deepEqual(plain(commands), [{ type: "CancelBuild", system_id: "home", job_id: 41 }]);
  assert.match(originRoot.html, /Cancel sent/);
  fixture.report.builds.pop();
  ui.render(world, true);
  assert.doesNotMatch(originRoot.html, /Cancel sent|Mining Complex V/, "the report without the job settles the cancel");
  // An order that never comes back says so instead of waiting forever.
  ui.handleAction(action(originRoot, "world-build", { mode: "structures" }), world);
  ui.handleAction(action(stageRoot, "world-builder-pick", { key: "smelter" }), world);
  ui.handleAction(action(stageRoot, "world-builder-queue", { key: "smelter" }), world);
  ui.handleAction(action(stageRoot, "world-builder-exit"), world);
  assert.match(originRoot.html, /Order sent · confirms in/);
  simNow += 120;
  ui.render(world, true);
  assert.match(originRoot.html, /No report yet · it may have been refused[\s\S]*data-deck-act="world-pending-dismiss" data-key="smelter"/);
  ui.handleAction(action(originRoot, "world-pending-dismiss", { key: "smelter" }), world);
  assert.doesNotMatch(originRoot.html, /No report yet/);
  simNow = 100;
  // Escape closes the sheet before anything else.
  ui.handleAction(action(originRoot, "world-build", { mode: "structures" }), world);
  assert.equal(ui.closeBuildWorkbench(world), true);
  assert.doesNotMatch(stageRoot.html, /class="deck-bsheet"/);
  // Hulls keep the central workbench over the planet.
  const shipOpener = action(originRoot, "world-build", { mode: "ships" });
  ui.handleAction(shipOpener, world);
  assert.equal(stagePanel.hidden, false);
  assert.equal(buildPanel.hidden, false, "hulls keep the central workbench");
  assert.equal(ui.worldBuildRoute.query.mode, "ships");
  assert.equal(ui.worldBuildRoute.query.body, "3");
  assert.equal(navigation.length, settled, "opening a child builder never navigates the workspace");
  ui.handleAction(action(buildRoot, "builder-mode", { mode: "modules" }), world);
  assert.equal(ui.worldBuildRoute.query.mode, "modules");
  fixture.report.stockpile = [{ commodity: "metallic_ore", units: 123 }];
  ui.render(world);
  assert.match(buildRoot.html, /123/, "received inventory refreshes the builder without a route change");
  ui.handleAction(action(buildRoot, "build-workbench-close"), world);
  assert.equal(buildPanel.hidden, true);
  assert.equal(doc.activeElement, shipOpener);
  ui.render(world);
  assert.equal(buildPanel.hidden, true, "a later report must not reopen the closed builder");
  // A free site on the planet opens the sheet on new buildings of its kind.
  const siteButton = stageRoot.html.match(/data-deck-act="world-build-site" data-slot="(\d+)" data-pool="infrastructure"/);
  assert.ok(siteButton, "the planet shows this world's free sites");
  const beforeSite = dispatched.length;
  ui.handleAction(action(stageRoot, "world-build-site", { slot: siteButton[1], pool: "infrastructure" }), world);
  assert.equal(ui.worldBuilder.site.slot, Number(siteButton[1]));
  assert.equal(navigation.length, settled, "site shortcuts also keep the planet mounted");
  assert.equal(dispatched.length, beforeSite, "choosing a site is not an order");
  assert.match(stageRoot.html, /class="deck-bsheet__site"><span>Free Infrastructure site<\/span>/);
  assert.match(stageRoot.html, /data-deck-act="world-builder-pick" data-key="academy"/);
  assert.doesNotMatch(stageRoot.html, /data-deck-act="world-builder-pick" data-key="(?:mining_complex|smelter)"/, "a site offers only new buildings of its kind");
  assert.match(stageRoot.html, /data-pool="industrial" disabled>/, "and only its own slot type");
  // Choose on planet folds the sheet away and lights the sites the pick can take.
  ui.handleAction(action(stageRoot, "world-builder-place"), world);
  assert.doesNotMatch(stageRoot.html, /class="deck-bsheet"/, "placing folds the sheet away");
  assert.match(stageRoot.html, /class="deck-bsheet-place"[\s\S]*Place <b>Academy I<\/b> on a lit site/);
  const address = footprintAddress(planetSurface(fixture.art), Number(siteButton[1]));
  const at = `style="--planet-x:${address.x}%;--planet-y:${address.y}%;--planet-scale:${address.scale}"`;
  assert.ok(stageRoot.html.includes(`class="deck-planet__site is-construction is-ghost" ${at}`), "the pick previews on the chosen site");
  const lit = stageRoot.html.match(/class="deck-planet__site is-free is-infrastructure is-match"[^>]*data-slot="(\d+)"/);
  assert.ok(lit, "the other free sites of its kind light up");
  assert.match(stageRoot.html, /is-free is-industrial is-dim/, "sites of other kinds dim");
  assert.equal(ui.closeBuildWorkbench(world), true);
  assert.equal(ui.worldBuilder.placing, false, "Escape steps back from placing to the sheet");
  assert.match(stageRoot.html, /class="deck-bsheet"/);
  ui.handleAction(action(stageRoot, "world-builder-place"), world);
  ui.handleAction(action(stageRoot, "world-build-site", { slot: lit[1], pool: "infrastructure" }), world);
  assert.equal(ui.worldBuilder.site.slot, Number(lit[1]), "a lit site becomes the pick's site");
  assert.equal(ui.worldBuilder.placing, false, "and the sheet comes back");
  assert.match(stageRoot.html, /class="deck-bsheet__where"><span>Your site<\/span>/);
  ui.handleAction(action(stageRoot, "world-builder-queue", { key: "academy" }), world);
  assert.deepEqual(plain(dispatched.at(-1)), ["academy", "home", 3]);
  const placed = footprintAddress(planetSurface(fixture.art), Number(lit[1]));
  const placedAt = `style="--planet-x:${placed.x}%;--planet-y:${placed.y}%;--planet-scale:${placed.scale}"`;
  assert.ok(stageRoot.html.includes(`class="deck-planet__site is-construction is-sent" ${placedAt}`), "the order in flight holds the chosen site");
  assert.doesNotMatch(stageRoot.html, /id="planet-construction-academy"/, "sending a directive is not a received construction report");
  fixture.report.builds.push({ id: 42, key: "academy", body_id: 3, queued: true, complete_time: null });
  ui.render(world, true);
  assert.ok(stageRoot.html.includes(`id="planet-construction-academy" class="deck-planet__site is-construction is-waiting" ${placedAt}`), "the received job uses the chosen address");
  assert.match(stageRoot.html, /id="planet-construction-academy"[\s\S]*?deck-planet__pill is-waiting"><i>2<\/i>Academy</, "a waiting job shows its place in line on the planet");
  fixture.report.builds.pop();
  ui.handleAction(action(stageRoot, "world-builder-exit"), world);
  assert.equal(ui.worldBuilder, null);
  // Staffing happens in the world's management view: a guide link to a
  // building, or a click on it, closes the sheet so its workforce shows.
  ui.handleAction(action(originRoot, "world-build", { mode: "structures" }), world);
  assert.match(stageRoot.html, /class="deck-bsheet"/);
  ui.focusWorldStructure("smelter");
  ui.render(world, true);
  assert.doesNotMatch(stageRoot.html, /class="deck-bsheet"/, "a guide link to a building closes the sheet");
  assert.match(originRoot.html, /id="planet-row-smelter"[^>]*aria-pressed="true"[\s\S]*data-coach="staff-plus"/, "and shows that building's workforce control");
  ui.handleAction(action(originRoot, "world-build", { mode: "structures" }), world);
  ui.handleAction(action(originRoot, "planet-select", { structure: "mining_complex" }), world);
  assert.doesNotMatch(stageRoot.html, /class="deck-bsheet"/, "clicking a building shows its management");
  assert.match(originRoot.html, /id="planet-row-mining_complex"[^>]*aria-pressed="true"/);
  // The building's own Upgrade opens the sheet on its next level.
  assert.match(originRoot.html, /class="deck-planet__upgrade"><span>Level V<small>[^<]*<\/small><\/span><button type="button" class="is-primary" data-deck-act="world-build" data-mode="structures" data-select="mining_complex">Upgrade to V</);
  ui.handleAction(action(originRoot, "world-build", { mode: "structures", select: "mining_complex" }), world);
  assert.match(stageRoot.html, /data-deck-act="world-builder-pick" data-key="mining_complex" aria-pressed="true"/, "the sheet opens on it");
  assert.match(stageRoot.html, /aria-selected="true" data-deck-act="world-builder-tab" data-pool="resource"/, "in its slot type");
  ui.handleAction(action(stageRoot, "world-builder-exit"), world);

  // Slot filtering follows actual mechanics, not the Shipyard's display-only
  // placement in the normal Infrastructure catalogue. Research stays gated.
  const priorCatalog = state.galaxy.build_options;
  state.galaxy.build_options = ["shipyard", "fuel_refinery", "academy", "mining_complex", "smelter", "electronics_fabricator"].map(key => ({ key, label: key, costs: [], build_secs: 20,
    ...(key === "electronics_fabricator" ? { research_prerequisite: "locked" } : {}) }));
  const buildBody = { ...fixture.body, structures: { ...fixture.body.structures, shipyard: 0 } };
  ui.structureDetail = () => "Detail";
  ui.structureCommit = () => "Queue";
  const filtered = ui.structureBuilder(fixture.report, buildBody, "industrial");
  assert.match(filtered, /data-key="shipyard"/);
  assert.doesNotMatch(filtered, /data-key="(?:academy|mining_complex|smelter|fuel_refinery|electronics_fabricator)"/, "only researched, new, unqueued structures for this pool appear");
  state.galaxy.build_options = priorCatalog;
  ui.handleAction(action(originRoot, "world-build", { mode: "ships" }), world);
  ui.handleAction(action(stageRoot, "world-switch", { body: "999" }), world);
  ui.handleAction(action(stageRoot, "world-switch", { body: "3" }), world);
  assert.equal(navigation.length, settled, "invalid and current-world clicks are no-ops");
  assert.equal(buildPanel.hidden, false);
  assert.match(originRoot.html, /aria-label="Planned workforce">3</, "a no-op never discards a workforce draft");
  ui.handleAction(action(stageRoot, "world-switch", { body: "4" }), world);
  assert.deepEqual(plain(navigation.at(-1)), ["replace", world4], "a sibling world replaces the route entry, so Back still returns to the system");
  assert.equal(originRoot.scrollTop, 0, "the column returns to the top for the new world");
  ui.render(world4, true);
  assert.equal(stagePanel.hidden, false);
  assert.equal(buildPanel.hidden, true, "switching worlds closes the old world's child builder");
  assert.match(originRoot.html, /<h2>Freya IV<\/h2>/);
  assert.match(stageRoot.html, /id="planet-world-4"[^>]*aria-current="page"/);
  assert.match(originRoot.html, /aria-label="Planned workforce">4</, "the new world uses its own served workforce, not the old world's draft");
  ui.handleAction(action(originRoot, "world-build", { mode: "structures" }), world4);
  assert.equal(ui.worldBuilder.world, "home:4", "new construction targets the selected world");
  ui.render(world, true);
  assert.match(stageRoot.html, /id="planet-world-3"[^>]*aria-current="page"/);
  assert.equal(buildPanel.hidden, true);
  assert.equal(ui.worldBuilder, null, "another world closes this one's builder");
  ui.handleAction(action(originRoot, "world-build", { mode: "ships" }), world);
  deferBuild = true;
  ui.renderWorldBuildWorkbench(true);
  ui.render({ name: "market" });
  deferBuild = false;
  deferred.forEach(callback => callback());
  assert.equal(stagePanel.hidden, true, "main navigation lowers the stage");
  assert.equal(buildPanel.hidden, true, "deferred work cannot resurrect a closed child");
  ui.render(world, true);
  ui.handleAction(action(originRoot, "world-build", { mode: "ships" }), world);
  ui.render(null);
  assert.equal(stagePanel.hidden, true, "closing the workspace lowers the stage");
  assert.equal(buildPanel.hidden, true);
  // §planet-build: a structure builder opened from anywhere (tutorial, links,
  // the system's Build tab) lands on its world with the builder over the planet.
  ui.render({ name: "build", params: { systemId: "home" }, query: { mode: "structures", body: "4", select: "academy" } });
  assert.equal(microtasks.length, 1, "the redirect waits for the router to finish announcing the build route");
  microtasks.splice(0).forEach(callback => callback());
  assert.deepEqual(plain(navigation.at(-1)), ["openWorldBuild", "home", 4, { select: "academy" }],
    "the structure builder opens over its requested world, keeping the selection");
  ui.render({ name: "build", params: { systemId: "home" }, query: { select: "mining_complex" } });
  microtasks.splice(0).forEach(callback => callback());
  assert.equal(navigation.at(-1)[0], "openWorldBuild", "no mode means structures");
  const requested = { ...world4, query: { build: "structures", select: "academy" } };
  ui.render(requested, true);
  assert.match(stageRoot.html, /class="deck-bsheet"[\s\S]*Build on Freya IV/, "a world route carrying the request opens the world's sheet");
  assert.equal(stagePanel.hidden, false, "beside the planet");
  assert.equal(buildPanel.hidden, true);
  assert.equal(ui.worldBuilder.world, "home:4");
  assert.match(stageRoot.html, /data-deck-act="world-builder-pick" data-key="academy" aria-pressed="true"/, "keeping the requested selection");
  assert.equal(microtasks.length, 1);
  microtasks.splice(0).forEach(callback => callback());
  assert.deepEqual(plain(navigation.at(-1)), ["replace", world4], "the request is consumed, so closing the builder keeps it closed");
  ui.render(world4, true);
  assert.match(stageRoot.html, /class="deck-bsheet"/, "the consumed route keeps its sheet");
  ui.handleAction(action(stageRoot, "world-builder-exit"), world4);
  ui.render(world4, true);
  assert.doesNotMatch(stageRoot.html, /class="deck-bsheet"/);
  // A completion toast lands on the new building and points at its workforce.
  ui.render({ ...world4, query: { structure: "mining_complex", staff: "1" } }, true);
  microtasks.splice(0).forEach(callback => callback());
  assert.match(originRoot.html, /id="planet-row-mining_complex"[^>]*aria-pressed="true"/, "the toast selects the finished building");
  assert.equal(coached.length, 0, "a staffed building needs no pointer");
  ui.render({ ...world4, query: { structure: "smelter", staff: "1" } }, true);
  microtasks.splice(0).forEach(callback => callback());
  assert.deepEqual(plain(coached.at(-1)), ["staff-plus", "Assign workforce so the Smelter starts work"], "an unstaffed one points at its workforce +");
  ui.render(origin, true);
  ui.builderMode = "structures";
  ui.handleAction(action(originRoot, "system-tab", { tab: "build" }), origin);
  assert.equal(navigation.at(-1)[0], "openWorldBuild", "the system's Build tab opens the planet builder too");
  // Ships keep the system builder.
  const shipsBuild = { name: "build", params: { systemId: "home" }, query: { mode: "ships" } };
  ui.render(shipsBuild);
  assert.equal(microtasks.length, 0, "a ship builder is not redirected");
  assert.equal(buildPanel.hidden, false, "the system ship builder still opens");
  assert.equal(stagePanel.hidden, true);
  const before = navigation.length;
  ui.handleAction(action(buildRoot, "builder-mode", { mode: "modules" }), shipsBuild);
  assert.equal(navigation.length, before + 1, "the system builder keeps its existing routing");
  assert.equal(navigation.at(-1)[0], "go");
}
checkWorldRoute(empireSource);
assert.throws(() => checkWorldRoute(empireSource.replace("this.worldBuildFor = worldKey(route);", "this.hooks.go(this.worldBuildRoute); this.worldBuildFor = worldKey(route);")), /never navigates the workspace/, "restoring navigation-based construction proves the regression catches it");

// Exercise the actual shell keyboard and zoom methods: a planet is a map rung,
// never an overlay, so map accelerators stay live and Esc/zoom-out climb one rung.
const shellAst = ts.createSourceFile("index.ts", src("shell/deck/index.ts"), ts.ScriptTarget.Latest, true);
const shellClass = shellAst.statements.find(node => ts.isClassDeclaration(node) && node.members.some(member => member.name?.getText(shellAst) === "keyDown"));
const methods = shellClass.members.filter(member => ["keyDown", "overlayOpen", "escapeOneLayer", "zoomIn", "zoomOut", "zoomFit"].includes(member.name?.getText(shellAst))).map(member => member.getText(shellAst)).join("\n");
assert.doesNotMatch(methods, /deck-world-workbench|closeWorldWorkbench/);
const nodes = { "deck-help": { hidden: true }, "deck-join": { hidden: true }, "deck-nav-overflow": { hidden: true } };
const document = { getElementById: id => nodes[id] ?? null, activeElement: null };
const { Keyboard } = compile("", `export class Keyboard { ${methods} }`, { document, byId: id => nodes[id] });
const keyboard = new Keyboard();
let backs = 0, exits = 0, routes = 0, zoomOuts = 0, zoomIns = 0, confirms = 0;
keyboard.ctx = {
  state: { pendingIntent: null, selectedShipId: null, selectedShipIds: new Set(), ghosts: [] },
  intent: { intentAiming: {}, confirmPendingIntent() { confirms++; } },
  renderer: { viewMode: { type: "system", systemId: "home" }, isSystemScrubbing: () => false, exitSystemView() { exits++; }, setSystemDynamic() {} },
};
keyboard.router = { current: { name: "world" }, back() { backs++; } };
keyboard.empire = { closeBuildWorkbench: () => false };
keyboard.theaters = { closeTop: () => false, isOpen: false };
keyboard.map = { zoomIn() { zoomIns++; }, zoomOut() { zoomOuts++; }, fit() {} };
keyboard.strip = { render() {} };
keyboard.editableTarget = () => false;
keyboard.openRoute = () => routes++;
keyboard.setNavOverflow = open => { nodes["deck-nav-overflow"].hidden = !open; };
const key = (name, options = {}) => keyboard.keyDown({ key: name, preventDefault() {}, ...options });
assert.equal(keyboard.overlayOpen(), false, "a planet is a map rung, never an overlay");
key("Escape");
assert.equal(backs, 1, "Esc on a planet climbs one rung");
assert.equal(exits, 0, "without leaving the orrery beneath it");
key("-");
assert.equal(backs, 2, "zooming out on a planet climbs to the orrery");
assert.equal(zoomOuts, 0);
key("+");
assert.equal(zoomIns, 0, "a planet has nowhere deeper to zoom");
key("m");
assert.equal(routes, 1, "route accelerators stay live on a planet");
keyboard.router.current = { name: "system" };
key("Escape");
assert.equal(exits, 1, "on the system rung Esc leaves the orrery first");
assert.equal(backs, 2);
key("-");
assert.equal(zoomOuts, 1, "the orrery keeps its own scrub-out");
keyboard.ctx.state.pendingIntent = {};
key("Enter");
assert.equal(confirms, 1, "map confirmation is live beside a planet");
console.log("PASS: world route beneath its system, map-area scene with workspace column, child construction, world switch by replace, deep links, Esc/zoom ladder; served reports, workforce confirmation, stable footprints, orbit space and docked fleets, and rival privacy.");

if (process.argv.includes("--serve")) {
  initialModel.catalog.push(commandCenter);
  initialModel.body.structures.command_center = 1;
  initialModel.body.structure_build_time_mult = 1;
  initialModel.catalog.push({ key: "convoy", label: "Freighter", costs: [{ commodity: "alloys", units: 10 }], build_secs: 12 });
  initialModel.catalog.push({ key: "academy", label: "Academy", costs: [{ commodity: "alloys", units: 10 }], build_secs: 20 });
  initialModel.body.resource_slots = 2;
  initialModel.body.industrial_slots = 4;
  initialModel.body.infrastructure_slots = 3;
  initialModel.report.structures = { ...initialModel.body.structures };
  initialModel.report.stockpile = [{ commodity: "alloys", units: 40 }];
  // §belts: --belt opens a charted Metallic belt with a Mining Complex.
  if (process.argv.includes("--belt")) {
    initialModel.body = { ...initialModel.body, id: 5, name: "Freya Belt", kind: "asteroid_belt",
      belt: { kind: "metallic", radius: .55, width: .04 }, environment: "uninhabitable", habitable: false,
      structures: { mining_complex: 1 }, resource_slots: 2, industrial_slots: 0, infrastructure_slots: 0,
      deposits: [{ resource: "bornite_ore", richness: .24, reserves: null }, { resource: "metallic_ore", richness: .21, reserves: null }],
      // §pockets: a found Titanium pocket after two prospecting passes.
      pockets: [{ resource: "titanium_ore", richness: .86, reserves: 1820 }], prospected: 2 };
    initialModel.report.assignments = [{ ...line, body_id: 5, workers: 1, tier: 1, outputs: [["bornite_ore", .2], ["metallic_ore", .21]] }];
    const ship = (id, kind) => ({ id, kind, owner: "1", own: true, pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 }, age: 0, jumped: false, route: null,
      passengers: {}, cargo: null, count_class: "one", composition: [{ kind, count: 1 }] });
    initialModel.fixtureGhosts = [ship("fx-barge", "mining_barge"), ship("fx-scout", "scout"),
      // §belts: a station anchored at this belt, half full.
      { ...ship("fx-station", "mining_belt_station"), anchor: { system: "home", body: 5, phase: "anchored", since: 0 },
        cargo_manifest: [{ commodity: "bornite_ore", units: 1240 }, { commodity: "metallic_ore", units: 310 }] }];
  }
  // §power: --power opens a world browning out under a tier I Power Plant.
  if (process.argv.includes("--power")) {
    Object.assign(initialModel.body.structures, { mining_complex: 3, smelter: 2, shipyard: 2, power_plant: 1 });
    initialModel.body.power = { demand: 14.4, supply: 11, power: .76, burn: .04, fuel: "fuel", no_fuel: false };
    const level = (tier) => ({ tier, label: tier >= 5 ? "Fusion Plant" : "Power Plant", build_secs: tier >= 5 ? 90 : 25,
      research_prerequisite: tier >= 5 ? "mat_foundry_iv_fusion_reactors" : null, description: `+${[0, 8, 17.6, 30.4, 48, 68.8, 92][tier]} MW for this planet.`,
      costs: tier >= 5 ? [{ commodity: "alloys", units: 120 }, { commodity: "iridium", units: 12 }] : [{ commodity: "alloys", units: 30 }, { commodity: "machinery", units: 12 }] });
    initialModel.catalog.push({ key: "power_plant", label: "Power Plant", costs: level(1).costs, build_secs: 25, consumes_slot: false,
      tiers: [1, 2, 3, 4, 5, 6].map(level) });
    initialModel.report.structures = { ...initialModel.body.structures };
  }
  // §gaia: --gaia opens a Gaia world with Living Terraces beside a Habitat.
  if (process.argv.includes("--gaia")) {
    Object.assign(initialModel.body, { kind: "terrestrial", environment: "gaia", habitable: true });
    Object.assign(initialModel.body.structures, { living_terraces: 3, habitat: 2 });
    initialModel.art = "/art/terrestrial-512.png";
  }
  // §marginal: --marginal opens a thin-aired terrestrial world colonists can't settle.
  if (process.argv.includes("--marginal")) {
    Object.assign(initialModel.body, { kind: "terrestrial", environment: "marginal", habitable: false });
    initialModel.art = "/art/derived/planets/2026-09-08/marginal-512.png";
  }
  if (process.argv.includes("--terran")) {
    initialModel.body.kind = "terrestrial";
    initialModel.body.environment = "terran";
    initialModel.body.habitable = true;
  }
  let surfaceWorlds = null;
  if (process.argv.includes("--habitable") || process.argv.includes("--orbital-yards") || process.argv.includes("--surface-structures")) {
    const used = new Set();
    surfaceWorlds = deps["../../planet-surfaces"].HABITABLE_SURFACES.map(surface => {
      const world = { ...structuredClone(initialModel.body), id: 0,
        kind: surface === "archipelago-world" ? "ocean" : "terrestrial", habitable: true, environment: "terran",
        name: surface.replace(/-/g, " ").replace(/\b\w/g, s => s.toUpperCase()),
        structures: { bioharvester: 1, agroplex: 1, habitat: 1, warehouse: 1, mining_complex: 1, command_center: 1 } };
      while (used.has(world.id) || planetSurface("/art/terrestrial-512.png", "home", world) !== surface) world.id++;
      used.add(world.id);
      return world;
    });
    initialModel.body = surfaceWorlds[0];
    initialModel.report.builds = [];
    initialModel.report.assignments = surfaceWorlds.flatMap(world => [
      { ...line, body_id: world.id, workers: 1, tier: 1 },
      { ...line, body_id: world.id, structure: "bioharvester", title: "Bioharvester", workers: 1, tier: 1, outputs: [["biomass", .5]] },
      { ...line, body_id: world.id, structure: "agroplex", title: "Agroplex", workers: 1, tier: 1, outputs: [["provisions", 1.2]] },
    ]);
  }
  if (process.argv.includes("--orbital-yards") || process.argv.includes("--surface-structures")) {
    const worlds = surfaceWorlds;
    for (const [index, surface] of ["terrestrial", "desert", "ocean", "ice", "lava", "barren", "gas_giant"].entries()) {
      worlds.push({ ...structuredClone(initialModel.body), id: 1000 + index, name: surface,
        kind: surface === "gas_giant" ? "gas_giant" : "rocky", habitable: false,
        fixtureArt: `/art/derived/planets/2026-09-08/${surface}-512.png` });
    }
    for (const key of ["shipyard", "naval_drydock", "capital_slipway"]) {
      initialModel.catalog.push({ key, label: deps["../../icons"].label(key), costs: [], build_secs: 60 });
      for (const world of worlds) {
        world.structures[key] = 1;
        world.industrial_slots = 8;
        initialModel.report.assignments.push({ ...line, body_id: world.id, structure: key,
          title: deps["../../icons"].label(key), workers: 1, tier: 1, outputs: [] });
      }
    }
    // §orbital-defense: a station over every world, its level cycling 1–5.
    initialModel.catalog.push({ key: "orbital_defense", label: "Orbital Defense", costs: [], build_secs: 60,
      tiers: ["Defense Platform", "Orbital Battery", "Defense Station", "Citadel", "Star Fortress"]
        .map((label, i) => ({ tier: i + 1, label, costs: [], build_secs: 60, description: "" })) });
    worlds.forEach((world, index) => { world.structures.orbital_defense = index % 5 + 1; });
  }
  if (process.argv.includes("--surface-structures")) {
    for (const world of surfaceWorlds) world.structures.service_berth = 1;
    for (const key of Object.keys(descriptions)) if (!initialModel.catalog.some(option => option.key === key)) {
      initialModel.catalog.push({ key, label: deps["../../icons"].label(key), costs: [], build_secs: 60 });
    }
  }
  const { build } = await import("vite");
  const clientRoot = fileURLToPath(new URL("../", import.meta.url));
  const result = await build({ root: clientRoot, configFile: false, publicDir: false, logLevel: "error",
    plugins: [{ name: "planet-fixture", resolveId: id => id.endsWith("virtual:planet-fixture") ? "\0planet-fixture" : undefined,
      load: id => id === "\0planet-fixture" ? `
        import { DeckEmpireRoutes } from ${JSON.stringify(resolve(clientRoot, "src/shell/deck/empire.ts"))};
        import { state } from ${JSON.stringify(resolve(clientRoot, "src/state.ts"))};
        import { installPressGuard } from ${JSON.stringify(resolve(clientRoot, "src/shell/dom.ts"))};
        import { bindMarketDerive } from ${JSON.stringify(resolve(clientRoot, "src/core/derive/market.ts"))};
        import { SURFACE_STRUCTURE_TIERS } from ${JSON.stringify(resolve(clientRoot, "src/planet-surface-structure-art.generated.ts"))};
        const model = ${JSON.stringify(initialModel)};
        state.playerId = '1'; state.simTime = 100;
        state.researchCatalog = [{id:'mat_prefab_construction',name:'Prefab Construction'}];
        state.commandCenter = {x:0,y:0}; state.galaxy = {systems:[model.system],c:200,build_options:model.catalog};
        state.systems = [model.report]; state.systems[0].bodies = ${JSON.stringify(surfaceWorlds)} ?? [
          {...model.body,id:2,name:'Freya II',kind:'rocky',environment:'hostile',habitable:false,structures:{mining_complex:1}},
          model.body,
          {...model.body,id:4,name:'Freya IV',kind:'gas_giant',environment:'hostile',habitable:false,structures:{},deposits:[]}
        ];
        model.body = state.systems[0].bodies.find(b=>b.id===model.body.id);
        if (model.fixtureGhosts) state.ghosts = model.fixtureGhosts;
        const root = document.getElementById('deck-workspace-body'), stageRoot = document.getElementById('deck-planet-stage-body');
        root.className = 'deck-workspace__body';
        document.querySelector('.deck').style.setProperty('--deck-workspace-inset', '552px');
        document.querySelector('.deck-workspace').style.width = '552px';
        let current = {name:'world',params:{systemId:'home',systemLabel:'Freya',bodyId:String(model.body.id),worldLabel:model.body.name}}; let sent = null;
        const controller = new DeckEmpireRoutes(root,document.getElementById('deck-build-workbench-body'),stageRoot,{state,renderer:{pulseSystemBody(){}},send(command){sent=command;document.getElementById('fixture-status').textContent='Directive in flight';}}, {go(route){document.getElementById('fixture-status').textContent='Opened '+route.name;},replace(route){current=route;controller.render(current,true);},notice(){},toast(){}});
        const originalModel = controller.planetModel.bind(controller);
        controller.planetModel = (system, report, body) => {const view=originalModel(system,report,body);return body.fixtureArt ? {...view,art:body.fixtureArt} : view;};
        bindMarketDerive(()=>({send(command){sent=command;document.getElementById('fixture-status').textContent=JSON.stringify(command);}}),()=>controller.composedFit);
        installPressGuard();
        for (const id of ['deck-planet-stage','deck-workspace-body','deck-build-workbench']) document.getElementById(id).addEventListener('click',event=>{const b=event.target.closest('button[data-deck-act]');if(b&&!b.disabled)controller.handleAction(b,current);});
        document.getElementById('fixture-open').onclick=()=>controller.render(current,true);
        document.getElementById('fixture-deliver').onclick=()=>{if(sent?.type==='SetAssignment'){let line=state.systems[0].assignments.find(a=>a.body_id===sent.body_id&&a.structure===sent.structure);if(!line){line={...model.report.assignments[0],body_id:sent.body_id,structure:sent.structure,title:sent.structure,outputs:[]};state.systems[0].assignments.push(line);}line.workers=sent.workers;line.specialists=sent.specialists;line.outputs=line.structure==='mining_complex'?[['metallic_ore',line.workers*.2],['silicates',line.workers*.05]]:[];sent=null;document.getElementById('fixture-status').textContent='Assignment report received';controller.invalidate();}};
        document.getElementById('fixture-build-report').onclick=()=>{if(sent?.type==='DevelopSystem'){state.systems[0].builds.push({key:sent.upgrade,body_id:sent.body_id,queued:true,start_time:null,complete_time:null});sent=null;document.getElementById('fixture-status').textContent='Construction report received';controller.invalidate();}};
        document.getElementById('fixture-all').onclick=()=>{for(const body of state.systems[0].bodies)for(const key of Object.keys(model.descriptions))body.structures[key]=Math.min(1,SURFACE_STRUCTURE_TIERS[key]??6);controller.invalidate();};
        document.getElementById('fixture-command').onclick=()=>{const tier=model.body.structures.command_center%3+1;model.body.structures.command_center=tier;model.body.infrastructure_slots=3+tier-1;model.body.structure_build_time_mult=[1,1,.9,.8][tier];controller.invalidate();};
        const yardButton = document.createElement('button');yardButton.id='fixture-yard';yardButton.textContent='Next yard tier';
        yardButton.onclick=()=>{for(const body of state.systems[0].bodies)for(const key of ['shipyard','naval_drydock','capital_slipway'])body.structures[key]=(body.structures[key]??0)%6+1;controller.invalidate();};
        document.querySelector('.deck-topbar').append(yardButton);
        const queueButton = document.createElement('button');queueButton.id='fixture-yard-queue';queueButton.textContent='Toggle ship queue';queueButton.setAttribute('aria-pressed','false');
        queueButton.onclick=()=>{const busy=queueButton.getAttribute('aria-pressed')!=='true';queueButton.setAttribute('aria-pressed',String(busy));state.systems[0].builds=busy?state.systems[0].bodies.filter(body=>body.structures.naval_drydock>0).map(body=>({key:'large_freighter',body_id:body.id,yard:'naval_drydock',queued:false,start_time:90,complete_time:190,work:{fraction:.4,per_second:.01,at_time:100}})):[];controller.invalidate();};
        document.querySelector('.deck-topbar').append(queueButton);
        const tierSelect=document.createElement('select');tierSelect.id='fixture-tier';tierSelect.setAttribute('aria-label','All structure tiers');
        for(let tier=1;tier<=6;tier++){const option=document.createElement('option');option.value=String(tier);option.textContent='Tier '+tier;tierSelect.append(option);}
        tierSelect.onchange=()=>{for(const body of state.systems[0].bodies)for(const key of Object.keys(body.structures))body.structures[key]=Math.min(Number(tierSelect.value),SURFACE_STRUCTURE_TIERS[key]??6);controller.invalidate();};
        document.querySelector('.deck-topbar').append(tierSelect);
        const siegeButton=document.createElement('button');siegeButton.id='fixture-siege';siegeButton.textContent='Toggle siege';siegeButton.setAttribute('aria-pressed','false');
        siegeButton.onclick=()=>{const on=siegeButton.getAttribute('aria-pressed')!=='true';siegeButton.setAttribute('aria-pressed',String(on));const sys=state.systems[0];state.galaxy.siege_secs=600;sys.blockade=on?{by:'2',by_name:'Vesper Combine',since:-300,by_me:false,held_since:-200,siege_since:-160,siege_secs:600,capturable:true,catch:.657,catch_stabilized:.51}:null;sys.ground=on?{garrison_tier:2,garrison_fed:true,suppression:.42,marines_needed:30}:null;state.groundRecords=on?[{id:'fixture-landing',system:sys.id,pos:{x:0,y:0},started_at:60,fidelity:'participant',attacker:'2',defender:'1',attacking:false,marines_landed:40,defenders_initial:30,garrison_tiers:2,suppression_at_drop:.42,rounds:[{tick:1,marines:26,defenders:19,marines_frac:.65,defenders_frac:.63,suppression:.42,marine_losses:2,defender_losses:2}],light_frontier_tick:1,outcome:null}]:[];controller.invalidate();};
        document.querySelector('.deck-topbar').append(siegeButton);
        const dockButton=document.createElement('button');dockButton.id='fixture-docked';dockButton.textContent='Toggle docked ships';dockButton.setAttribute('aria-pressed','false');
        dockButton.onclick=()=>{const on=dockButton.getAttribute('aria-pressed')!=='true';dockButton.setAttribute('aria-pressed',String(on));const at='E'+model.system.id;const g=(id,kind,n)=>({id,kind,owner:'1',own:true,pos:{x:0,y:0},vel:{x:0,y:0},age:0,jumped:false,route:null,passengers:{},cargo:null,count_class:n===1?'one':'two_to_three',composition:[{kind,count:n}],docked:at});state.ghosts=on?[g('fx-cruiser','cruiser',3),g('fx-destroyer','destroyer',1),g('fx-battleship','battleship',1),g('fx-convoy','convoy',2),g('fx-scout','scout',1)]:[];controller.invalidate();};
        document.querySelector('.deck-topbar').append(dockButton);
        controller.render(current,true);
        setInterval(()=>controller.render(current,false),100);
      ` : undefined }], build: { write: false, minify: false, lib: { entry: "virtual:planet-fixture", formats: ["es"] } } });
  const output = (Array.isArray(result) ? result[0] : result).output;
  const bundle = output.find(file => file.type === "chunk" && file.isEntry).code;
  const files = new Map(output.map(file => [`/${file.fileName}`, file.type === "chunk" ? file.code : file.source]));
  const publicRoot = resolve(clientRoot, "public");
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css"><link rel="stylesheet" href="/planet-panel.css"><div class="deck" style="--deck-workspace-inset:552px"><header class="deck-topbar"><b>Planet fixture</b><button id="fixture-open">Render world</button><button id="fixture-deliver">Deliver assignment report</button><button id="fixture-build-report">Deliver construction report</button><button id="fixture-all">All buildings</button><button id="fixture-command">Next Command level</button><span id="fixture-status"></span></header>${stageShell}<aside class="deck-workspace" style="width:552px"><header class="deck-workspace__header"><h1>World</h1></header><div id="deck-workspace-body"></div></aside>${buildShell}</div><script type="module" src="/fixture.js"></script>`);
      return;
    }
    if (url.pathname === "/fixture.js") { response.setHeader("Content-Type", "text/javascript"); response.end(bundle); return; }
    if (files.has(url.pathname)) { response.setHeader("Content-Type", url.pathname.endsWith(".css") ? "text/css" : "text/javascript"); response.end(files.get(url.pathname)); return; }
    let path;
    if (["/tokens.css", "/deck.css", "/planet-panel.css"].includes(url.pathname)) { path = resolve(clientRoot, `src/styles${url.pathname}`); response.setHeader("Content-Type", "text/css"); }
    else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
    } else { response.writeHead(404).end(); return; }
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  const port = Number(process.argv.find(arg => arg.startsWith("--port="))?.slice("--port=".length)) || 0;
  server.listen(port, "127.0.0.1", () => console.log(`Planet fixture: http://127.0.0.1:${server.address().port}/`));
}

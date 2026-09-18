// Served-only planet management on the world route: the scene over the map area,
// the management column in the workspace, and the actual confirmation controller.
// --serve opens an isolated browser fixture; it never contacts a game server.
// Add --terran to check structure contrast against the green landscape.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

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
const marketDerive = compile("core/derive/market.ts");
deps["../../core/derive/market"] = marketDerive;
deps["./planet-sites"] = compile("shell/deck/planet-sites.ts");
const { PlanetSites } = deps["./planet-sites"];
const { PlanetPanel, planetStructures, planetSurface, footprintAddress } = compile("shell/deck/planet.ts");
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
assert.match(planetCss, /--planet-building: clamp\(36px, 8\.625cqw, 150px\)/, "footprints are 25% smaller at the minimum, responsive size and maximum");
assert.doesNotMatch(planetCss, /planet-narrow|planet-wide|object-fit: cover/, "neither responsive rearrangement nor independent image cropping may move structures");
assert.doesNotMatch(planetCss, /\.deck-planet__building[^{}]*::before/, "structures have no added foundation pads, including when hovered or selected");
const workspaceSource = src("shell/deck/workspace.ts");
assert.match(workspaceSource, /route\.name === "world"\) return "standard"/, "the world route keeps the system rail width so the scene stays wide");
assert.match(workspaceSource, /this\.route\.name === "market" \|\| this\.route\.name === "world"\) return;/, "no wide toggle on a world");
// The fixture follows the actual authored catalogue and staffed-kind list.
const descriptions = Object.fromEntries([...empireSource.slice(empireSource.indexOf("const STRUCTURE_DESCRIPTION"), empireSource.indexOf("import { MODULES"))
  .matchAll(/^  (\w+): "([^"]+)"/gm)].map(([, key, description]) => [key, description]));
assert.equal(Object.keys(descriptions).length, 25);
const workforceStructures = new Set([...empireSource.slice(empireSource.indexOf("const WORKFORCE_STRUCTURES"), empireSource.indexOf("const STRUCTURE_DESCRIPTION"))
  .matchAll(/"([a-z_]+)"/g)].map(match => match[1]));
const catalog = [
  { key: "mining_complex", label: "Mining Complex", costs: [], build_secs: 20 },
  { key: "smelter", label: "Smelter", costs: [], build_secs: 20, conversion: { output: "alloys", rate: 1, inputs: [["metallic_ore", 2], ["fuel", 1]] } },
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
  populationHtml: "<p>Population controls</p>", surveyHtml: "<p>Survey report</p>", developmentHtml: "<button>Build structure</button>", queueHtml: "<p>Queue here</p>" };
const initialModel = structuredClone(model);
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
// Include the new Warehouse in place of the optional Capital Slipway, rather
// than constructing an impossible 25-building world to exceed the artwork.
for (const key of Object.keys(fullModel.descriptions).filter(key => key !== "capital_slipway")) fullModel.body.structures[key] = 1;
const allOccupied = new PlanetSites(storage).snapshot(fullModel).occupied;
assert.equal(allOccupied.length, 24, "even the all-buildings preview reuses empty reservations instead of overlapping built structures");
assert.equal(new Set(allOccupied.map(site => site.slot)).size, 24);
assert.ok(allOccupied.some(site => site.key === "warehouse"), "the founding store receives a terrain site, including when empty sites must be reused");
assert.ok(Object.entries(initialPositions).every(([key, slot]) => allOccupied.find(site => site.key === key)?.slot === slot), "reclaiming empty plots never moves an existing building");
siteModel.report.builds = [];
assert.ok(!sites.snapshot(siteModel).occupied.some(site => site.key === "fuel_refinery"), "cancelled new construction releases its site");
assert.equal(sites.snapshot({ ...siteModel, mine: false }).free.length, 0);
const sitePanel = new PlanetPanel();
assert.doesNotMatch(sitePanel.renderScene(initialModel), /data-deck-act="world-build-site"/, "empty plots stay hidden outside Build mode");
assert.match(sitePanel.renderScene(initialModel), /id="planet-construction-fuel_refinery"/, "reported new construction gets an outline");
sitePanel.handleAction({ deckAct: "planet-build-sites" }, initialModel, () => { throw Error("a visual toggle must not send an order"); });
assert.match(sitePanel.renderScene(initialModel), /data-deck-act="world-build-site"/);
sitePanel.handleAction({ deckAct: "planet-build-sites" }, initialModel, () => {});
assert.doesNotMatch(sitePanel.renderScene(initialModel), /data-deck-act="world-build-site"/);
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
assert.match(visual, /id="planet-spot-mining_complex"[^>]*style="--planet-x:7%;--planet-y:46%"/);
// Footprints must sit on the painting's terrain: below the planetary limb (whose haze
// curves from ~19% at the centre down to ~32% at the edges) and never overlapping,
// even across the full reserved footprint (11.5% wide, 20.4% tall on 16:9);
// the rendered sprites are now smaller, but their terrain addresses stay fixed.
for (const surface of ["desert", "terrestrial", "ice", "lava", "barren", "ocean", null]) {
  const spots = Array.from({ length: 24 }, (_, slot) => footprintAddress(surface, slot));
  for (const { x, y } of spots) {
    const limb = 19 + 13 * ((x - 50) / 50) ** 2;
    assert.ok(y - 10.2 >= limb + 3, `${surface} slot at ${x},${y} pokes into the limb haze (${limb.toFixed(1)})`);
    assert.ok(x - 5.75 >= 0 && x + 5.75 <= 100 && y + 10.2 <= 100, `${surface} slot at ${x},${y} leaves the painting`);
  }
  spots.forEach((a, i) => spots.slice(i + 1).forEach((b) => {
    assert.ok(Math.abs(a.x - b.x) >= 12 || Math.abs(a.y - b.y) >= 21, `${surface} slots ${a.x},${a.y} and ${b.x},${b.y} overlap`);
  }));
}
assert.notDeepEqual(footprintAddress("ocean", 7), footprintAddress("desert", 7), "ocean worlds use their own island addresses");
const oceanModel = { ...structuredClone(initialModel), art: "/art/derived/planets/2026-09-08/ocean-512.png" };
oceanModel.body.structures = { bioharvester: 1, agroplex: 1, habitat: 1, mining_complex: 1 };
const oceanSpots = Object.fromEntries(planetStructures(oceanModel).map(({ key, x, y }) => [key, [x, y]]));
assert.deepEqual(oceanSpots, { mining_complex: [67, 46], bioharvester: [31, 41], agroplex: [7, 46], habitat: [31, 89] }, "the common surface kinds sit on islands and shorelines, not in the sky or the open sea");
assert.doesNotMatch(visual, /workforce-summary|class="deck-tabs|id="planet-page"|deck-planet__development|planet-workers|planet-review|planet-narrow|planet-wide/, "no management control lives on the map scene");
assert.match(controls, /class="deck-page deck-planet"[\s\S]*deck-planet__lead[\s\S]*<h2>Freya III<\/h2>[\s\S]*Information delay <b>~17s<\/b>[\s\S]*workforce-summary[\s\S]*class="deck-tabs[\s\S]*id="planet-page"[\s\S]*deck-planet__development/, "identity, workforce, tabs, structure details and build actions all live in the workspace column");
assert.doesNotMatch(controls, /deck-planet__canvas|deck-planet__building|world-switch/, "the workspace column never repeats the scene or the world strip");
assert.match(controls, /Build structure<\/button><p>Queue here<\/p>/, "this world's own construction queue sits with its build actions");
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
assert.match(html, /desert-1536.webp 1536w/, "large art gets a high-DPI source");
assert.match(visual, /id="planet-spot-mining_complex"[^>]*>[^<]*<img class="icon icon--lg"[^>]*srcset="[^"]*mining_complex\/tier-4-512\.webp 512w"[^>]*sizes="calc\(11\.5vw - 64px\)"/, "footprints use the reported tier and offer a high-DPI sprite");
assert.match(miningRow, /mining_complex\/tier-4-128\.webp/, "the list and planet scene use the same tier");
assert.match(controls.match(/<aside id="planet-structure-detail"[\s\S]*?<\/aside>/)[0], /mining_complex\/tier-4-128\.webp/, "selected detail uses the same reported tier");
const upgrading = structuredClone(initialModel), upgradePanel = new PlanetPanel();
upgrading.report.builds.push({ key: "mining_complex", body_id: 3, complete_time: 1, queued: false });
assert.match(upgradePanel.renderScene(upgrading), /mining_complex\/tier-4-128\.webp/, "an upgrade whose estimate expired cannot change the built artwork");
assert.doesNotMatch(upgradePanel.renderScene(upgrading), /mining_complex\/tier-5-/, "future artwork is confined to the builder preview");
upgrading.body.structures.mining_complex = 5;
assert.match(upgradePanel.renderScene(upgrading), /mining_complex\/tier-5-128\.webp/, "only the arrived tier report upgrades the footprint");
assert.doesNotMatch(controls, /sizes="calc\(11\.5vw/, "only the scene's footprints name the plane-relative sprite width");
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
click("planet-confirm");
assert.equal(sent.length, 0, "cannot bypass review");
click("planet-review");
assert.match(render(), /Order travel ~17s/);
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
assert.doesNotMatch(html, /planet-spot-|planet-row-|planet-workers|planet-review|assigned here|Population controls|Build structure|Queue here/);
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
assert.match(html, /Build structure/, "an undeveloped world still offers construction");
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
  deps["../../state"] = { liveSimTime: () => 100 };
  deps["./planet"] = { PlanetPanel };
  deps["../../core/derive/market"] = { ...marketDerive,
    structureResearched: entry => !entry.research_prerequisite, structOption: () => ({ buildable: true }),
    dispatchBuildKey: (...args) => dispatched.push(args) };
  const { DeckEmpireRoutes } = compile("", source, { document: doc, HTMLElement: Element, requestAnimationFrame: callback => callback() });
  const ui = new DeckEmpireRoutes(originRoot, buildRoot, stageRoot, { state, renderer: { pulseSystemBody() {} }, send() { throw Error("unexpected directive"); } },
    { go: route => navigation.push(["go", route]), replace: route => navigation.push(["replace", route]), notice() {}, toast() {} });
  ui.planetModel = (system, report, body) => ({ ...fixture, system, report, body, mine: report.owner === state.playerId });
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
  originRoot.scrollTop = 190;
  ui.planet.handleAction({ deckAct: "planet-workers", delta: "1" }, fixture, () => {});
  const settled = navigation.length;
  for (const mode of ["structures", "ships"]) {
    const opener = action(originRoot, "world-build", { mode });
    ui.handleAction(opener, world);
    assert.equal(stagePanel.hidden, false, "Build must not remove the planet scene");
    assert.equal(buildPanel.hidden, false);
    assert.equal(ui.worldBuildRoute.query.mode, mode);
    assert.equal(ui.worldBuildRoute.query.body, "3");
    assert.equal(navigation.length, settled, "opening a child builder never navigates the workspace");
    ui.handleAction(action(buildRoot, "builder-mode", { mode: "modules" }), world);
    assert.equal(ui.worldBuildRoute.query.mode, "modules");
    ui.handleAction(action(buildRoot, "builder-mode", { mode: "structures" }), world);
    ui.handleAction(action(buildRoot, "builder-body", { body: "4" }), world);
    ui.handleAction(action(buildRoot, "builder-select", { key: "mining_complex" }), world);
    ui.handleAction(action(buildRoot, "builder-queue"), world);
    assert.deepEqual(plain(dispatched.at(-1)), ["mining_complex", "home", 4], "the existing build dispatcher uses the child workbench's site");
    assert.equal(stagePanel.hidden, false, "queueing does not dismiss the planet");
    fixture.report.stockpile = [{ commodity: "metallic_ore", units: 123 }];
    ui.render(world);
    assert.match(buildRoot.html, /123/, "received inventory refreshes the builder without a route change");
    assert.equal(originRoot.scrollTop, 190);
    assert.match(originRoot.html, /aria-label="Planned workforce">3</, "planet workforce drafts survive construction");
    ui.handleAction(action(buildRoot, "build-workbench-close"), world);
    assert.equal(buildPanel.hidden, true);
    assert.equal(stagePanel.hidden, false);
    assert.equal(doc.activeElement, opener);
    ui.render(world);
    assert.equal(buildPanel.hidden, true, "a later report must not reopen the closed builder");
    assert.equal(navigation.length, settled, "category and site changes stay local too");
  }
  ui.handleAction(action(stageRoot, "planet-build-sites"), world);
  const siteButton = stageRoot.html.match(/data-deck-act="world-build-site" data-slot="(\d+)" data-pool="infrastructure"/);
  assert.ok(siteButton);
  const siteOpener = action(stageRoot, "world-build-site", { slot: siteButton[1], pool: "infrastructure" });
  const beforeSite = dispatched.length;
  ui.handleAction(siteOpener, world);
  assert.equal(ui.worldBuildRoute.query.site_pool, "infrastructure");
  assert.equal(ui.worldBuildRoute.query.site_slot, siteButton[1]);
  assert.equal(ui.worldBuildRoute.query.body, "3");
  assert.equal(navigation.length, settled, "site shortcuts also keep the planet mounted");
  assert.equal(dispatched.length, beforeSite, "opening a site is not an order");
  ui.handleAction(action(buildRoot, "builder-select", { key: "mining_complex" }), world);
  ui.handleAction(action(buildRoot, "builder-queue"), world);
  assert.equal(dispatched.length, beforeSite, "a site cannot dispatch another pool or an upgrade");
  ui.handleAction(action(buildRoot, "builder-select", { key: "academy" }), world);
  ui.handleAction(action(buildRoot, "builder-queue"), world);
  assert.deepEqual(plain(dispatched.at(-1)), ["academy", "home", 3]);
  assert.doesNotMatch(stageRoot.html, /id="planet-construction-academy"/, "sending a directive is not a received construction report");
  fixture.report.builds.push({ key: "academy", body_id: 3, queued: true });
  ui.render(world, true);
  const address = footprintAddress(planetSurface(fixture.art), Number(siteButton[1]));
  assert.ok(stageRoot.html.includes(`id="planet-construction-academy" class="deck-planet__site is-construction" style="--planet-x:${address.x}%;--planet-y:${address.y}%"`), "the received job uses the clicked address");
  ui.handleAction(action(buildRoot, "builder-queue"), world);
  assert.equal(dispatched.length, beforeSite + 1, "a consumed site rejects a stale queue click");
  ui.handleAction(action(buildRoot, "builder-all-structures"), world);
  assert.equal(ui.worldBuildRoute.query.site_pool, undefined);
  assert.equal(ui.worldBuildRoute.query.site_slot, undefined);
  fixture.report.builds.pop();
  ui.handleAction(action(buildRoot, "build-workbench-close"), world);
  ui.handleAction(siteOpener, world);
  ui.handleAction(action(buildRoot, "builder-mode", { mode: "ships" }), world);
  assert.equal(ui.worldBuildRoute.query.site_pool, undefined, "leaving Structures clears the clicked-site constraint");
  ui.handleAction(action(buildRoot, "build-workbench-close"), world);

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
  ui.handleAction(action(stageRoot, "planet-build-sites"), world);
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
  assert.equal(ui.worldBuildRoute.query.body, "4", "new construction targets the selected world");
  ui.render(world, true);
  assert.match(stageRoot.html, /id="planet-world-3"[^>]*aria-current="page"/);
  assert.equal(buildPanel.hidden, true);
  ui.handleAction(action(originRoot, "world-build", { mode: "structures" }), world);
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
  const normalBuild = { name: "build", params: { systemId: "home" }, query: { mode: "structures" } };
  ui.render(normalBuild);
  assert.equal(buildPanel.hidden, false, "the normal system builder still opens");
  assert.equal(stagePanel.hidden, true);
  const before = navigation.length;
  ui.handleAction(action(buildRoot, "builder-mode", { mode: "ships" }), normalBuild);
  assert.equal(navigation.length, before + 1, "the normal builder keeps its existing routing");
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
console.log("PASS: world route beneath its system, map-area scene with workspace column, child construction, world switch by replace, deep links, Esc/zoom ladder; served reports, workforce confirmation, stable footprints and rival privacy.");

if (process.argv.includes("--serve")) {
  initialModel.catalog.push({ key: "convoy", label: "Freighter", costs: [{ commodity: "alloys", units: 10 }], build_secs: 12 });
  initialModel.catalog.push({ key: "academy", label: "Academy", costs: [{ commodity: "alloys", units: 10 }], build_secs: 20 });
  initialModel.body.resource_slots = 2;
  initialModel.body.industrial_slots = 4;
  initialModel.body.infrastructure_slots = 3;
  initialModel.report.structures = { ...initialModel.body.structures };
  initialModel.report.stockpile = [{ commodity: "alloys", units: 40 }];
  if (process.argv.includes("--terran")) {
    initialModel.body.kind = "terrestrial";
    initialModel.body.environment = "terran";
    initialModel.body.habitable = true;
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
        const model = ${JSON.stringify(initialModel)};
        state.playerId = '1'; state.simTime = 100;
        state.commandCenter = {x:0,y:0}; state.galaxy = {systems:[model.system],c:200,build_options:model.catalog};
        state.systems = [model.report]; state.systems[0].bodies = [
          {...model.body,id:2,name:'Freya II',kind:'rocky',environment:'hostile',habitable:false,structures:{mining_complex:1}},
          model.body,
          {...model.body,id:4,name:'Freya IV',kind:'gas_giant',environment:'hostile',habitable:false,structures:{},deposits:[]}
        ];
        const root = document.getElementById('deck-workspace-body'), stageRoot = document.getElementById('deck-planet-stage-body');
        document.querySelector('.deck').style.setProperty('--deck-workspace-inset', '552px');
        document.querySelector('.deck-workspace').style.width = '552px';
        let current = {name:'world',params:{systemId:'home',systemLabel:'Freya',bodyId:'3',worldLabel:'Freya III'}}; let sent = null;
        const controller = new DeckEmpireRoutes(root,document.getElementById('deck-build-workbench-body'),stageRoot,{state,renderer:{pulseSystemBody(){}},send(command){sent=command;document.getElementById('fixture-status').textContent='Directive in flight';}}, {go(route){document.getElementById('fixture-status').textContent='Opened '+route.name;},replace(route){current=route;controller.render(current,true);},notice(){},toast(){}});
        bindMarketDerive(()=>({send(command){sent=command;document.getElementById('fixture-status').textContent=JSON.stringify(command);}}),()=>controller.composedFit);
        installPressGuard();
        for (const id of ['deck-planet-stage','deck-workspace-body','deck-build-workbench']) document.getElementById(id).addEventListener('click',event=>{const b=event.target.closest('button[data-deck-act]');if(b&&!b.disabled)controller.handleAction(b,current);});
        document.getElementById('fixture-open').onclick=()=>controller.render(current,true);
        document.getElementById('fixture-deliver').onclick=()=>{if(sent?.type==='SetAssignment'){let line=state.systems[0].assignments.find(a=>a.body_id===sent.body_id&&a.structure===sent.structure);if(!line){line={...model.report.assignments[0],body_id:sent.body_id,structure:sent.structure,title:sent.structure,outputs:[]};state.systems[0].assignments.push(line);}line.workers=sent.workers;line.specialists=sent.specialists;line.outputs=line.structure==='mining_complex'?[['metallic_ore',line.workers*.2],['silicates',line.workers*.05]]:[];sent=null;document.getElementById('fixture-status').textContent='Assignment report received';controller.invalidate();}};
        document.getElementById('fixture-build-report').onclick=()=>{if(sent?.type==='DevelopSystem'){state.systems[0].builds.push({key:sent.upgrade,body_id:sent.body_id,queued:true,start_time:null,complete_time:null});sent=null;document.getElementById('fixture-status').textContent='Construction report received';controller.invalidate();}};
        document.getElementById('fixture-all').onclick=()=>{for(const key of Object.keys(model.descriptions))model.body.structures[key]=4;controller.invalidate();};
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
      response.end(`<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css"><link rel="stylesheet" href="/planet-panel.css"><div class="deck" style="--deck-workspace-inset:552px"><header class="deck-topbar"><b>Planet fixture</b><button id="fixture-open">Render world</button><button id="fixture-deliver">Deliver assignment report</button><button id="fixture-build-report">Deliver construction report</button><button id="fixture-all">All buildings</button><span id="fixture-status"></span></header>${stageShell}<aside class="deck-workspace" style="width:552px"><header class="deck-workspace__header"><h1>World</h1></header><div id="deck-workspace-body"></div></aside>${buildShell}</div><script type="module" src="/fixture.js"></script>`);
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
  server.listen(0, "127.0.0.1", () => console.log(`Planet fixture: http://127.0.0.1:${server.address().port}/`));
}

// Received-report fixtures exercising actual helpers and panel controllers.
// --serve is an isolated visual fixture: it never connects to the running game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

const src = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const state = { playerId: "1", simTime: 60, midgameStage: "home_development",
  selectedShipId: null, pendingOrders: new Map(), orders: {}, raids: {}, operations: [],
  explorationSites: [], selectedExplorationSiteId: null,
  battleViewed: new Set(), battleDismissed: new Set(),
  commandCenter: { x: 0, y: 0 }, galaxy: { hub: { x: 70_000, y: 0 },
    systems: [{ id: "home", pos: { x: 0, y: 0 } }] }, systems: [], ghosts: [] };
let renderNow = 60;
const deps = { "../../state": { state, liveSimTime: () => renderNow } };
function compile(path, extra = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, ...extra });
  return exports;
}
const protocol = deps["../../protocol"] = compile("protocol.ts");
deps["./vendetta"] = compile("core/derive/vendetta.ts");
const founding = deps["../../core/derive/founding"] = compile("core/derive/founding.ts");
assert.equal(founding.FOUNDING_TOTAL, 23);
assert.deepEqual(["build_bioharvester", "build_habitat", "build_agroplex", "build_mine", "pirate_raid", "build_gun_platform", "sell_ore",
  "power_shipyard", "build_shipyard", "build_interceptor", "defeat_privateer", "eliminate_den", "prepare_export", "first_research",
  "build_smelter", "expand_mining", "survey_candidates", "build_barge", "prospect_belt", "mine_belt", "haul_ore", "secure_stores",
  "vendetta"].map(stage => founding.FOUNDING_STEP[stage]),
  [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]);
assert.equal(founding.FOUNDING_STEP.complete, 23);
assert.deepEqual(["local_recovery", "export_production", "build_convoy", "complete_export", "grow_business", "build_second_freighter",
  "build_academy", "build_scout", "buy_semiconductors", "build_freighter"].map(stage => founding.FOUNDING_STEP[stage]),
  [13, 13, 10, 9, 9, 9, 14, 17, 8, 10], "a retired stage sits on the step that replaced it");
assert.deepEqual([...founding.FOUNDING_CHAPTERS.map(([name, steps]) => `${name} ${steps.length}`)],
  ["Landfall 4", "Raid 5", "Pirates 3", "Expansion 5", "Reach 4", "Vendetta 2"], "twenty-three steps in six chapters");
{
  const st = { research: { programmes: [] } };
  const home = { bodies: [{id:1,structures:{}}], assignments:[] };
  const next = () => [...founding.foundingBusinessGoals(st,home)].map(g=>g.action);
  assert.deepEqual(next(),["build-shipyard","build-academy"]);
  home.bodies[0].structures = {shipyard:1,academy:1};
  assert.deepEqual(next(),["shipyard","academy"]);
  home.assignments = ["shipyard","academy"].map(structure=>({body_id:1,structure,workers:1,specialists:{}}));
  assert.deepEqual(next(),["build-convoy","research-enrichment"]);
  st.research.programmes = [{id:"mat_enrichment",state:"queued"}];
  assert.equal(next()[1],"research-enrichment","a queued tech is not an arrived unlock");
  st.research.programmes[0].state="completed";
  assert.equal(next()[1],"build-smelter");
  home.bodies[0].structures.smelter=1;
  assert.equal(next()[1],"smelter");
  assert.equal(founding.foundingScoutGoal(home).action,"build-scout");
  home.assignments=[];
  assert.equal(founding.foundingScoutGoal(home).action,"shipyard");
  delete home.bodies[0].structures.shipyard;
  assert.equal(founding.foundingScoutGoal(home).action,"build-shipyard","refiners receive the missing Scout prerequisite");
}
deps["./equipment"] = deps["../../core/derive/equipment"] = compile("core/derive/equipment.ts");
deps["../../core/derive/construction"] = compile("core/derive/construction.ts");
const icons = deps["../../icons"] = compile("icons.ts");
deps["./format"] = { fmtDur: n => `${Math.round(n)}s` };
deps["./geo"] = { HYPERLIMIT_SU: 900, operationSystemName: () => "Freya" };
const fleets = deps["./fleet"] = deps["../../core/derive/fleet"] = compile("core/derive/fleet.ts");
// The founding plan reads fleet speeds and formats ETAs: re-link it in place,
// so every module already holding the founding exports sees the real helpers.
Object.assign(founding, compile("core/derive/founding.ts"));
const pirateSites = deps["../../core/derive/pirates"] = compile("core/derive/pirates.ts");
const readiness = deps["../../core/derive/readiness"] = compile("core/derive/readiness.ts");
const colony = deps["../../core/derive/colony"] = compile("core/derive/colony.ts");
deps["../../prng"] = { hashId: () => 0 };
deps["../../core/derive/format"] = compile("core/derive/format.ts");
deps["../../core/derive/captains"] = { captainTitle: () => "Lieutenant", officerFleetName: g => g.kind };
deps["../signature"] = { sheetFingerprint: x => JSON.stringify(x) };
deps["../dom"] = { renderDeferred: () => false, setHtml(root, html) { root.innerHTML = html; } };
deps["../battlewithdraw"] = compile("shell/battlewithdraw.ts");
deps["../core/derive/exploration"] = compile("core/derive/exploration.ts");
deps["../icons"] = icons;
deps["../core/derive/fleet"] = fleets;
deps["../exploration"] = compile("shell/exploration.ts");
deps["../industry"] = compile("shell/industry.ts");

const ghost = (id, kind, extra = {}) => ({ id, kind, own: true, owner: "1",
  pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 }, age: 8, damage: .04,
  composition: [{ kind, count: 1 }], fuel: 150, supplied: true, cargo: null,
  ...extra });
const freighter = ghost("102", "convoy", { fuel: 12, cargo_manifest: [
  { commodity: "alloys", units: 40 }, { commodity: "deuterium", units: 60 }] });
const guard = ghost("101", "raider", { damage: .74, fuel: 2, guard_target: "102" });
state.ghosts = [guard, freighter];
const r = readiness.fleetReadiness(freighter);
assert.equal(r.cargoFree, 300, "mixed manifests share one Medium hold");
assert.equal(r.fuel, 12);
assert.equal(readiness.fleetReadiness(ghost("3", "raider", { damage: undefined })).hull, null);
assert.match(readiness.dispatchWarnings(guard, state.galaxy.hub).join(" "), /26% hull.*Deuterium short/);
assert.equal(readiness.dispatchWarnings(ghost("3", "raider", { fuel: 0 }), state.galaxy.hub, true).length, 0,
  "jumps are not priced as warp cruises");
assert.match(readiness.dispatchWarnings(ghost("3", "raider", { fuel: 0, antimatter: 0, antimatter_capacity: 2 }), state.galaxy.hub, true).join(""),
  /Antimatter short.*jump cancels; refill at a dock or from a tender/, "a jump burns Antimatter from the bottle");
const slow = ghost("4", "convoy", { transit: "slow", fuel: 100, uranium: 1, uranium_capacity: 100 });
assert.equal(readiness.fleetReadiness(slow).fuelKind, "uranium", "Slow reads the fission store");
assert.match(readiness.dispatchWarnings(slow, state.galaxy.hub).join(""), /Uranium short.*Switches to Deuterium/);
assert.match(readiness.dispatchWarnings(ghost("3", "convoy", { fuel: undefined }), state.galaxy.hub).join(""), /unavailable/);
assert.equal(readiness.dispatchWarnings({ ...guard, own: false }, state.galaxy.hub).length, 0);
const multi = readiness.intentReadinessWarnings({ ...state, emplacements: [] },
  { verb: "move", shipId: "101", shipIds: ["101", "102"], dest: state.galaxy.hub });
assert.match(multi.join(" "), /Freighter 102: Deuterium short/, "all dispatched fleets, not only primary selection");
const refueled = { ...freighter, fuel: 900 };
assert.equal(readiness.dispatchWarnings(refueled, state.galaxy.hub).length, 0, "new telemetry clears the warning");

const home = { id: "home", owner: "1", bodies: [], food_state: "well_supplied",
  workforce: { units: 6, posted: 6 }, builds: [],
  stockpile: [{ commodity: "metallic_ore", units: 0 }, { commodity: "deuterium", units: 3 }],
  converters: [{ structure: "smelter", title: "Smelter", status: "no_inputs" }],
  opportunities: [{ role: "mining_world", score: 1.3 }, { role: "mining_world", score: 1.5 }] };
const prospect = { id: "prospect", owner: null,
  bodies: [{ id: 0, deposits: [{ resource: "metallic_ore", reserves: 10_000 }] }],
  opportunities: [{ body_id: 0, role: "mining_world", score: 2.2 }] };
state.systems = [home, prospect];
let purpose = colony.colonyPurpose(prospect, home);
assert.match(purpose.headline, /Excellent mining · poor agriculture/);
assert.match(purpose.homeNeed, /Smelter/);
assert.deepEqual([...purpose.imports], ["provisions"]);
assert.match(purpose.advantages, /home 1.50×/, "compare with best home role, not arbitrary first world");
assert.equal(colony.colonyPurpose({ ...prospect, bodies: [{ deposits: null }] }, home), null,
  "no role or import inference before a survey");
assert.doesNotMatch(colony.colonyPurpose(prospect, { ...home, converters: [] }).homeNeed, /shortage|missing/,
  "do not fabricate a home bottleneck");
const electronics = { ...prospect, bodies: [{ id: 0, deposits: [{ resource: "rare_elements", reserves: 100 }] }],
  opportunities: [{ role: "electronics_center", score: 2, body_id: 0 }] };
assert.deepEqual([...colony.colonyPurpose(electronics, home).imports], ["provisions", "conductive_metals", "silicates"]);
const depleted = { ...prospect, bodies: [{ id: 0, deposits: [{ resource: "metallic_ore", reserves: 0 }] }], opportunities: [] };
assert.equal(colony.colonyPurpose(depleted, home).exports.length, 0);
const mixed = { ...prospect, bodies: [...prospect.bodies, { id: 1, deposits: [{ resource: "biomass", reserves: 100 }] }] };
assert.equal(colony.colonyPurpose(mixed, home, mixed.bodies[0]).imports.length, 0,
  "another world in the system can feed a barren mining moon without interstellar imports");

deps["./geo"] = deps["../../core/derive/geo"] = compile("core/derive/geo.ts");
deps["./market"] = deps["../../core/derive/market"] = compile("core/derive/market.ts");
// The construction model formats with the real helpers; founding keeps its stub.
{
  const foundingFormat = deps["./format"];
  deps["./format"] = deps["../../core/derive/format"];
  deps["./construction"] = deps["../../core/derive/construction"];
  deps["./station"] ??= compile("core/derive/station.ts");
  deps["../../core/derive/worldbuild"] = compile("core/derive/worldbuild.ts");
  deps["./format"] = foundingFormat;
}
deps["./colony"] = colony;
deps["../../settlement"] = { SETTLEMENT_PROJECTS: ["prospect", "surface_workings", "storehouse", "quarters"].map(id => ({ id, title: id })) };
const handoff = deps["../../core/derive/handoff"] = compile("core/derive/handoff.ts");
const handoffUi = deps["./handoff"] = compile("shell/deck/handoff.ts");
// §medals and §contract-board: the real helpers, with the real formatter.
{
  const foundingFormat = deps["./format"];
  deps["./format"] = deps["../../core/derive/format"];
  deps["./medals"] = deps["../../core/derive/medals"] = deps["../core/derive/medals"] = compile("core/derive/medals.ts");
  deps["../../core/derive/board"] = compile("core/derive/board.ts");
  deps["../corpcard"] = compile("shell/corpcard.ts");
  deps["./format"] = foundingFormat;
}
let rendered = "";
const root = { id: "fixture", contains: () => true, set innerHTML(v) { rendered = v; } };
class Select {}
const document = { activeElement: null };
const { DeckStrategicRoutes } = compile("shell/deck/strategic.ts",
  { document, HTMLSelectElement: Select, HTMLInputElement: class {} });
const sent = [], previews = [], selected = [], located = [];
const ctx = { state, send: c => sent.push(c), intent: {
  beginPendingIntent: i => previews.push(i),
  beginFleetCommand: commands => previews.push({ verb: "command", commands }),
},
  renderer: { centerOnWorld: p => located.push(p) } };
const ui = new DeckStrategicRoutes(root, ctx, { notice() {}, go() {},
  selectFleets: ids => selected.push(ids) });
const operation = (id, kind, follow_up, title) => ({ id, kind, target_pos: kind.pos ?? state.galaxy.hub,
  state: "offered", joined: false, issuer: "authority", scope: { scope: "private", player: "1" },
  goal: kind.units ?? 1, progress: 0, reported_at: 50, expires_at: 900,
  reward: { credits: 900, captain_xp: 80, authority_standing: 2 },
  briefing: { follow_up, title, difficulty: follow_up === "escort" ? "Moderate" : "Low",
    suitable_fleets: follow_up === "escort" ? "1 healthy Interceptor + 1 Freighter" : "1 Freighter",
    summary: follow_up === "escort" ? "Guard from home to Market. Expect a slow two-ship pirate pack."
      : follow_up === "salvage" ? "Recover 24 Alloys; the nearby Corvette fight is optional."
        : "Ore + Fuel → Smelter → Alloys. Deliver 40 to Market; selling is separate." } });
const escort = operation("11", { kind: "freight_escort", origin: { x: 0, y: 0 }, destination: state.galaxy.hub }, "escort", "Guarded market run");
const salvage = operation("12", { kind: "rescue_salvage", units: 24, commodity: "alloys", pos: { x: 25_000, y: 16_000 } }, "salvage", "Quiet wreck, armed neighbours");
const production = operation("13", { kind: "market_delivery", commodity: "alloys", units: 40 }, "production", "First specialist export");
state.operations = [escort, salvage, production];
const offers = ui.operationsHtml();
assert.match(offers, /Moderate/);
assert.match(offers, /healthy Interceptor/);
assert.match(offers, /900 cr/);
assert.match(offers, /strategic-operation-accept/);
assert.doesNotMatch(offers, /strategic-operation-dispatch/, "dispatch only after acceptance arrives");
escort.state = "active"; escort.joined = true;
ui.operationFleets.set("11", "101"); ui.operationCharges.set("11", "102");
assert.match(ui.operationCard(escort), /Deuterium short/);
ui.operationAction("strategic-operation-dispatch", { dataset: { operation: "11" } });
assert.equal(sent.length, 0, "dispatch only opens confirmation; not even the assignment is sent early");
assert.deepEqual(JSON.parse(JSON.stringify(previews[0].commands)), [
  { type: "AssignOperationFleet", operation_id: "11", fleet_id: "101", protected_fleet: "102" },
  { type: "GuardFleet", interceptor_id: "101", target_id: "102" },
], "assignment and guard are confirmed together; no hidden movement");
salvage.state = "active"; salvage.joined = true;
ui.operationFleets.set("12", "102");
ui.operationAction("strategic-operation-dispatch", { dataset: { operation: "12" } });
assert.equal(previews[1].verb, "command", "salvage must preview before dispatch");
assert.equal(sent.length, 0);
assert.equal(previews[1].commands[0].type, "AssignOperationFleet");
assert.equal(previews[1].commands[1].dest.x, 25_000);
const select = new Select();
select.dataset = { operation: "11", deckInput: "operation-fleet" }; select.value = "101";
ui.handleInput(select, { name: "operations" });
document.activeElement = select;
const previous = rendered;
state.simTime++;
ui.render({ name: "operations" });
assert.equal(rendered, previous, "view refresh does not replace an open selector");
document.activeElement = null;
assert.equal(ui.operationFleets.get("11"), "101", "selection survives subsequent refreshes");

// Combat progression uses the same confirmation and served-data surface.
const corvette = ghost("103", "corvette");
const cruiser = ghost("104", "cruiser");
assert.equal(fleets.guardCapable(corvette), true);
assert.equal(fleets.guardCapable(cruiser), true);
assert.equal(fleets.guardCapable(freighter), false);
assert.equal(fleets.guardCapable({ ...cruiser, own: false }), false);
assert.equal(pirateSites.pirateSite(0), null, "no hidden-site inference");
assert.match(pirateSites.pirateSite(5).title, /stronghold/);
state.ghosts.push(corvette, cruiser);
for (const [id, kind, chapter] of [
  ["patrol", { kind: "privateer_patrol", pos: { x: 40000, y: 10000 } }, "patrol"],
  ["depot", { kind: "pirate_bounty", tier: 4, system: "site" }, "depot"],
  ["stronghold", { kind: "pirate_bounty", tier: 5, system: "site" }, "stronghold"],
]) {
  const o = { ...operation(id, kind, chapter, chapter), state: "active", joined: true };
  state.operations.push(o);
  ui.operationFleets.set(id, cruiser.id);
  const card = ui.operationCard(o);
  assert.match(card, /Cruiser/);
  assert.match(card, /Plot assault/);
  assert.doesNotMatch(card, /value="102"/, "cargo hull isn't an assault selection");
  if (id !== "patrol") assert.match(card, new RegExp(`/art/pirate-sites/${id}.png`));
  const sentBefore = sent.length;
  ui.operationAction("strategic-operation-dispatch", { dataset: { operation: id } });
  assert.equal(sent.length, sentBefore, "assault is a preview, never a surprise dispatch");
  assert.equal(previews.at(-1).commands[0].type, "AssignOperationFleet");
  assert.equal(previews.at(-1).commands[1].type, "MoveShip");
  state.operations.pop();
}
state.ghosts.splice(-2);

// The home-raid chain is a served offer, not a client-side truth detector.
{
const scout = ghost("scout", "scout");
state.ghosts.push(scout);
const trace = { ...operation("trace", { kind: "survey_expedition", system: "hideout" },
  "counter_raid_trace", "1/3 · Trace the home raiders"), state: "active", joined: true };
state.operations.push(trace);
ui.operationFleets.set(trace.id, scout.id);
const traceCard = ui.operationCard(trace);
assert.match(traceCard, /Plot survey/);
assert.match(traceCard, /value="scout"/);
assert.doesNotMatch(traceCard, /value="101"|value="102"/, "neither an Interceptor nor a Freighter can survey");
const ordersBeforeTrace = sent.length;
ui.operationAction("strategic-operation-dispatch", { dataset: { operation: trace.id } });
assert.equal(sent.length, ordersBeforeTrace, "survey must wait for the ordinary confirmation");
assert.deepEqual(JSON.parse(JSON.stringify(previews.at(-1).commands)), [
  { type: "AssignOperationFleet", operation_id: "trace", fleet_id: "scout" },
  { type: "SurveySystem", fleet_id: "scout", system_id: "hideout" },
]);
const recovery = { ...operation("recovery", { kind: "rescue_salvage", units: 11, commodity: "electronics", pos: { x: 80_000, y: 0 } },
  "counter_raid_recovery", "3/3 · Recover electronics"), state: "active", joined: true };
state.operations.push(recovery);
ui.operationFleets.set(recovery.id, freighter.id);
assert.match(ui.operationCard(recovery), /Plot recovery/);
assert.doesNotMatch(ui.operationCard(recovery), /value="scout"|value="101"/);
ui.operationAction("strategic-operation-dispatch", { dataset: { operation: recovery.id } });
assert.equal(sent.length, ordersBeforeTrace);
assert.equal(previews.at(-1).commands[1].type, "MoveShip");
assert.match(ui.operationsHtml(), /Defend home · strike back/);
state.operations.splice(-2);
state.ghosts.pop();
}

const { DeckFleetRoutes } = compile("shell/deck/fleet.ts");
const fleetUi = new DeckFleetRoutes(root, ctx, { go() {}, notice() {} });
{
  const prize = { ...operation("prize", { kind: "prize_recovery", prize: "breacher_cache", system: "hideout", pos: { x: 80_000, y: 0 } },
    "site_recovery", "Recover Breacher cache"), state: "active", joined: true };
  state.operations.push(prize);
  ui.operationFleets.set(prize.id, freighter.id);
  const html = ui.operationCard(prize);
  assert.match(html, /Plot recovery/);
  assert.match(html, /4 module crates · Cruiser Hull dossier/);
  assert.doesNotMatch(html, /value="101"/, "equipment requires a Freighter, not the Interceptor");
  const before = sent.length;
  ui.operationAction("strategic-operation-dispatch", { dataset: { operation: prize.id } });
  assert.equal(sent.length, before, "prize recovery dispatch still needs order confirmation");
  assert.deepEqual(JSON.parse(JSON.stringify(previews.at(-1).commands)), [
    { type: "AssignOperationFleet", operation_id: prize.id, fleet_id: freighter.id },
    { type: "MoveShip", ship_id: freighter.id, dest: prize.target_pos },
  ]);
  freighter.modules = { torpedo_rack: 12 };
  assert.match(ui.operationCard(prize), /Needs 4 free module berths/);
  delete freighter.modules;
  assert.match(fleetUi.cargoHtml({ ...freighter, modules: { mass_driver: 2, whipple_armor: 2 } }),
    /Module crates.*Stored on docking.*Mass Driver.*Whipple Armor/);
  state.operations.pop();
  const research = compile("core/derive/research.ts");
  const merged = research.mergeResearch({ active: null, queue: [], rate: 0, stalled: false, academies: [],
    programmes: [{ id: "cruiser", state: "locked", recovered_data: 300 }], structure_tiers: { habitat: 4 } },
    { researchCatalog: [{ id: "cruiser", name: "Cruiser Hull", cost: 1000 }] });
  assert.equal(merged.programmes[0].recovered_data, 300, "catalog merging must retain arrived, earmarked data");
  assert.deepEqual({ ...merged.structure_tiers }, { habitat: 4 }, "§unlocks: the served ceilings survive the merge");
}
assert.match(fleetUi.readinessHtml(guard), /Guarding Medium Freighter/);
assert.match(fleetUi.readinessHtml(guard), /26%/);
assert.match(readiness.intentReadinessWarnings(state, { verb: "command", shipId: "102",
  commands: [{ type: "HaulToMarketHub", fleet_id: "102", sell_on_arrival: false }] }).join(" "), /Deuterium short/,
  "the shared confirmation retains the haul fuel warning");
const { DeckEmpireRoutes } = compile("shell/deck/empire.ts");
const empire = Object.create(DeckEmpireRoutes.prototype); empire.ctx = ctx;
assert.match(empire.colonyPurposeHtml(prospect), /Excellent mining/);
assert.equal(empire.colonyPurposeHtml(home), "", "home is not pitched as the next colony");
assert.match(icons.commodityIcon("provisions"), /provisions.png/);

// §construction: progress is the job's own estimate, never its recipe time.
// Its bar is drawn once, in its world's Construction card; the system view
// says each world's slot in one line, with yard work beneath it.
state.galaxy.build_options = [{ key: "convoy", build_secs: 12, costs: [] }];
const { buildProgress } = deps["../../core/derive/construction"];
const pct = (job, now) => buildProgress(job, now)?.toFixed(1) ?? null;
const queue = (job, now) => {
  renderNow = now;
  return empire.queueHtml({ ...home, builds: [{ key: "convoy", body_id: 0, ...job }] });
};
for (const duration of [12, 9.6, 230 / 30, 18]) {
  const job = { start_time: 60, complete_time: 60 + duration };
  assert.equal(pct(job, 60), "0.0", `a fresh ${duration}s job starts at zero, regardless of its recipe`);
  assert.equal(pct(job, 60 + duration / 2), "50.0");
  assert.equal(pct(job, 60 + duration), "100.0");
  assert.equal(pct(job, 59), "0.0", "clock slew cannot make progress negative");
  assert.match(queue(job, 80), /System yard.*Medium Freighter · completing/,
    "clock expiry does not remove a job before its completion report arrives");
}
state.galaxy.build_options[0].build_secs = 999;
assert.equal(pct({ start_time: 60, complete_time: 69.6 }, 62.4), "25.0",
  "a late first report uses the job's start, not the report's arrival time");
assert.equal(pct({ start_time: 60, complete_time: 69.6 }, 64.8), "50.0",
  "new recipes or bonuses never retime a reported job");
for (const start_time of [undefined, null, NaN, Infinity, 70, 71]) {
  assert.equal(pct({ start_time, complete_time: 70 }, 60), null,
    "legacy or invalid timing retains the ETA, not a fabricated percentage");
  const html = queue({ start_time, complete_time: 70 }, 60);
  assert.match(html, /Medium Freighter · 10s/);
  assert.doesNotMatch(html, /NaN|Infinity/);
}
const pausedJob = { start_time: 60, complete_time: null,
  work: { fraction: .375, per_second: 0, at_time: 63.6 } };
for (const now of [63.6, 70, 600]) {
  assert.equal(pct(pausedJob, now), "37.5", "paused progress never creeps or vanishes when its former ETA expires");
  assert.match(queue(pausedJob, now), /Medium Freighter · paused · needs workforce/);
  assert.doesNotMatch(queue(pausedJob, now), /completing|NaN|Infinity/);
}
const resumedJob = { ...pausedJob, complete_time: 76,
  work: { fraction: .375, per_second: 1 / 9.6, at_time: 70 } };
assert.equal(pct(resumedJob, 70), "37.5");
assert.equal(pct(resumedJob, 71.2), "50.0",
  "resuming continues earned work instead of restarting or using original elapsed time");
assert.equal(pct(resumedJob, 100), "100.0");
assert.match(queue(resumedJob, 100), /completing/, "an estimated finish still waits for a completion report");
assert.equal(pct({ ...resumedJob, work: { fraction: .5, per_second: 1 / 12, at_time: 71.2 } }, 74.2), "75.0",
  "only remaining work follows the changed rate");
assert.equal(pct({ complete_time: null }, 100), null);
assert.doesNotMatch(queue({ complete_time: null }, 100), /completing/);
const waitingStructure = { key: "mining_complex", body_id: 0, queued: true,
  start_time: null, complete_time: null, work: { fraction: 0, per_second: 0, at_time: 60 } };
const slotWorld = job => ({ ...home, bodies: [{ id: 0, name: "Freya I", structures: {} }], builds: [job] });
for (const now of [60, 69.6, 6000]) {
  renderNow = now;
  const html = empire.queueHtml(slotWorld(waitingStructure));
  assert.match(html, /<b>Freya I<\/b><small[^>]*>Mining Complex I · starting<\/small>/);
  assert.doesNotMatch(html, /Building|completing|NaN|Infinity/,
    "queued is report state: neither clock expiry nor another job finishing can start it locally");
}
const startedStructure = { ...waitingStructure, queued: false, start_time: 6000,
  complete_time: 6020, work: { fraction: 0, per_second: .05, at_time: 6000 } };
renderNow = 6000;
assert.match(empire.queueHtml(slotWorld(startedStructure)), /Mining Complex I · 20s/);
renderNow = 6010;
assert.match(empire.queueHtml(slotWorld(startedStructure)), /Mining Complex I · 10s/);
assert.doesNotMatch(empire.queueHtml(slotWorld(startedStructure)), /progressbar|deck-queue-progress/,
  "the system view draws no bar: an order's one bar is its world's");
// §live-progress (Peter, Oct 27 2026: the bar looked "laggy/jittery"): a bar
// carries its job's line, and each frame redraws it from the render clock, so
// it glides between its panel's whole-second redraws instead of jumping on them.
{
  const construction = deps["../../core/derive/construction"];
  const attr = (line) => construction.liveProgressAttr(line).match(/^ data-live-progress="([^"]*)"$/)?.[1];
  for (const job of [{ start_time: 60, complete_time: 69.6 }, pausedJob, resumedJob, waitingStructure, startedStructure]) {
    const line = construction.progressLine(job);
    assert.deepEqual(construction.readLiveProgress(attr(line)), line, "a bar's line survives its attribute");
    for (const now of [0, 60, 64.8, 71.2, 6010]) assert.equal(construction.progressAt(line, now), buildProgress(job, now));
  }
  assert.deepEqual(JSON.parse(JSON.stringify(construction.progressLine(startedStructure))), { at: 6000, base: 0, slope: 5 },
    "5% a second from its report");
  assert.equal(construction.progressLine(pausedJob).slope, 0, "a paused job's bar holds");
  assert.equal(construction.progressLine(waitingStructure).slope, 0, "so does a waiting one's");
  assert.equal(construction.liveProgressAttr(construction.progressLine({ start_time: 70, complete_time: 70 })), "", "no clock, no live bar");
  for (const value of [undefined, "", "1 2", "a b c", "1 2 Infinity"]) assert.equal(construction.readLiveProgress(value), null);
  deps["../core/derive/construction"] = construction;
  const { tickLiveProgress } = compile("shell/liveprogress.ts");
  const bar = (value, fill = { style: { width: "12.0%" } }) => ({ dataset: { liveProgress: value }, firstElementChild: fill });
  const live = bar(attr(construction.progressLine(startedStructure)));
  const broken = bar("not a line");
  const empty = bar(attr(construction.progressLine(startedStructure)), null);
  const root = { querySelectorAll: (selector) => selector === "[data-live-progress]" ? [live, broken, empty] : [] };
  tickLiveProgress(root, 6004.5);
  assert.equal(live.firstElementChild.style.width, "22.50%", "each frame: its line's percent now");
  tickLiveProgress(root, 6004.6);
  assert.equal(live.firstElementChild.style.width, "23.00%", "a tenth of a second later, a little further");
  assert.equal(broken.firstElementChild.style.width, "12.0%", "a bar without a line keeps what its panel drew");
  for (const [file, call] of [["shell/deck/index.ts", "tickLiveProgress(document, liveSimTime());"],
    ["shell/mobile/index.ts", "tickLiveProgress(document, liveSimTime());"]]) {
    const tick = src(file).slice(src(file).indexOf("onViewTick(): void {"), src(file).indexOf("framePolicy() {"));
    assert.ok(tick.includes(call), `${file}: every frame`);
  }
  for (const [file, count] of [["shell/deck/empire.ts", 1], ["shell/deck/planet.ts", 1], ["shell/deck/shipyard.ts", 1], ["shell/mobile/parity.ts", 2]]) {
    assert.equal(src(file).match(/\$\{liveProgressAttr\(job\.line\)\}/g)?.length, count, `${file}: its construction bars are live`);
  }
}
const queueWorld = { ...home,
  bodies: [{ id: 0, name: "Freya I", structures: {} }, { id: 1, name: "Freya II", structures: {} }],
  builds: [waitingStructure,
    { key: "habitat", body_id: 1, start_time: 60, complete_time: 80 },
    { key: "shipyard", body_id: 0, start_time: 60, complete_time: 80 },
    { ...waitingStructure, key: "orbital_warehouse" }] };
const receivedOrder = JSON.stringify(queueWorld.builds);
renderNow = 60;
const planetsHtml = empire.queueHtml(queueWorld);
assert.equal(JSON.stringify(queueWorld.builds), receivedOrder, "rendering never mutates the served queue");
assert.match(planetsHtml, /<b>Freya I<\/b><small[^>]*>Shipyard I · 20s · \+2 queued<\/small>.*<b>Freya II<\/b><small[^>]*>Habitat I · 20s<\/small>/,
  "one line per world: the order holding its slot, then how many wait");
assert.match(planetsHtml, /data-deck-act="open-world" data-body="0">Open</, "and a way to the world");
const yardWorld = { ...home, structures: { shipyard: 2 }, builds: [],
  bodies: [{ id: 0, name: "First", structures: { shipyard: 1 } },
    { id: 1, name: "Second", structures: { shipyard: 2 } }],
  assignments: [{ body_id: 0, structure: "shipyard", staffing: 1, skill: 1 }] };
const hullOption = { key: "convoy", label: "Medium Freighter", build_secs: 50, costs: [] };
state.research = { programmes: [{ id: "prop_heavy_lifters", state: "completed" }] };
let hullQuote = deps["./market"].shipOption(hullOption, yardWorld);
assert.equal(hullQuote.buildRate, 0, "workers on another planet do not staff the actual yard");
assert.equal(hullQuote.buildable, true, "an unstaffed hull may queue but cannot progress");
assert.match(hullQuote.reason, /wait for workforce.*Second/);
yardWorld.assignments.push({ body_id: 1, structure: "shipyard", staffing: 1, skill: 1 });
hullQuote = deps["./market"].shipOption(hullOption, yardWorld);
assert.equal(hullQuote.buildRate, 1.25);
assert.equal(hullQuote.reason, "");
yardWorld.assignments[1].staffing = .5;
assert.equal(deps["./market"].shipOption(hullOption, yardWorld).buildRate, 1.125);
yardWorld.assignments[1].staffing = 0;
assert.equal(deps["./market"].shipOption(hullOption, yardWorld).buildRate, 0,
  "a nominal assignment with no effective workforce is still paused");
delete state.research;
renderNow = 60;

// All six freight choices use the same arrived research/recipe checks on both
// shells. Learning a larger hull never removes Tiny as a backup-route option.
{
  const market = deps["./market"];
  const hulls = ["tiny_freighter", "small_freighter", "convoy", "large_freighter", "heavy_freighter", "bulk_freighter"];
  const programmes = [null, "prop_freight_frames", "prop_heavy_lifters", "prop_line_express_charters", "prop_line_bulk_charters", "prop_line_autonomous_freight"];
  const names = [null, "Freight Frames", "Heavy Lifters", "Express Charters", "Bulk Charters", "Autonomous Freight"];
  const oldCatalog = state.researchCatalog;
  state.researchCatalog = programmes.slice(1).map((id, i) => ({ id, name: names[i + 1] }));
  state.research = { programmes: [] };
  // This block isolates research/affordability. The larger freight hulls use
  // the existing Orbital Shipyard gate, so provide both kinds of yard here.
  const freightYard = { ...yardWorld, structures: { shipyard: 5, naval_drydock: 3 },
    bodies: [{ id: 0, name: "Freight Yard", structures: { shipyard: 5, naval_drydock: 3 } }],
    assignments: ["shipyard", "naval_drydock"].map(structure => ({ body_id: 0, structure, staffing: 1, skill: 1 })),
    stockpile: [{ commodity: "alloys", units: 200 }] };
  assert.deepEqual(hulls.filter(market.hullResearched), ["tiny_freighter"]);
  for (let i = 1; i < hulls.length; i++) {
    const option = { key: hulls[i], build_secs: 24, costs: [{ commodity: "alloys", units: 45 }] };
    for (const status of ["locked", "available", "active"]) {
      state.research.programmes = [{ id: programmes[i], state: status }];
      assert.equal(market.hullResearched(hulls[i]), false, `${hulls[i]}: ${status} is not completed`);
      const quote = market.shipOption(option, freightYard);
      assert.equal(quote.buildable, false);
      assert.equal(quote.reason, `Requires ${names[i]} research.`);
    }
    state.research.programmes[0].state = "completed";
    assert.deepEqual(hulls.filter(market.hullResearched), ["tiny_freighter", hulls[i]],
      "only this hull unlocks; its predecessors and successors are not free");
    const quote = market.shipOption(option, freightYard);
    assert.equal(quote.buildable, true);
    assert.equal(quote.maxAff, 4, "affordability reads the served recipe, not a stale hull cost");
    const shortage = market.shipOption({ ...option, costs: [{ commodity: "alloys", units: 201 }] }, freightYard);
    assert.equal(shortage.buildable, false);
    assert.equal(shortage.maxAff, 0);
  }
  state.researchCatalog = oldCatalog;
  delete state.research;
}

// Post-victory advice is assembled from the same received picture as the panels.
// Exercise the real derivation + both routed consumers, not an imitation checklist.
deps["../../core/derive/orders"] = { nextDecisionLabel: () => "No urgent decisions" };
state.name = "Frontier Test"; state.timeline = []; state.battles = [];
state.galaxy.systems = [
  { id: "home", name: "Freya", pos: { x: 0, y: 0 } },
  { id: "prospect", name: "Ore Haven", pos: { x: 80_000, y: 0 }, band: "rich" },
  { id: "garden", name: "Garden", pos: { x: 0, y: 85_000 } },
];
state.galaxy.build_options = ["scout", "corvette", "colony"].map((key, i) => ({ key,
  costs: [{ commodity: "alloys", units: 30 + i * 10 }], build_secs: 20 }));
home.bodies = [{ id: 4, name: "Freya IV", structures: { shipyard: 1 }, infrastructure_slots: 4, industrial_slots: 3 }];
home.structures = { shipyard: 1 }; home.assignments = [];
home.stockpile.push({ commodity: "alloys", units: 2 });
prospect.bodies[0].geology = null;
const garden = { id: "garden", owner: null, bodies: [{ id: 0, geology: null, deposits: null }], opportunities: [] };
state.systems = [home, prospect, garden];
// §charter v3: the charter itself teaches the Shipyard, research, surveys and
// belt mining, so the handoff stays hidden until the charter is fulfilled.
state.founding = { stage: "build_mine", bounty_received: true, expansion_unlocked: true,
  protected: true, protection_min_until: 100, protection_max_until: 1000, survey_candidates: ["prospect"] };
for (const stage of ["build_bioharvester", "build_mine", "pirate_raid", "sell_ore", "defeat_privateer", "build_shipyard",
  "first_research", "expand_mining", "survey_candidates", "build_barge", "mine_belt", "haul_ore"]) {
  state.founding.stage = stage;
  assert.equal(handoff.postVictoryHandoff().length, 0, `${stage}: the charter still owns the guide`);
}
assert.equal(handoffUi.handoffHtml(), "");
state.founding.stage = "complete";
let goals = handoff.postVictoryHandoff();
const goal = id => handoff.postVictoryHandoff().find(g => g.id === id);
assert.deepEqual([...goals.map(g => g.id)], ["colony", "upgrade"]);
// Nothing habitable charted yet: a probe survey comes first, and no prospect is invented.
assert.equal(goal("colony").status, "Next goal");
assert.equal(goal("colony").action.select, "survey_probe");
assert.equal(goal("colony").actionLabel, "Build Survey Probe");
assert.equal(goal("colony").prospect, null);
assert.equal(goal("upgrade").action.select, "shipyard");
assert.equal(goal("upgrade").action.mode, "structures");
assert.doesNotMatch(handoffUi.handoffHtml(), /Excellent mining|colony kit|Found a specialist colony/,
  "unarrived surveys cannot advertise a hidden jackpot or promise a colony grant");
assert.equal(goal("upgrade").costs[0].stock, 2);
home.stockpile.at(-1).units = 45;
assert.equal(goal("upgrade").costs[0].stock, 45, "stock updates with the received inventory");
freighter.docked = "Ehome";
assert.equal(goal("upgrade").costs[0].stock, 85, "construction includes served docked Freighter goods");
freighter.docked = null;
// A surveyed habitable world nobody holds becomes the prospect.
garden.bodies[0].habitable = true;
assert.equal(goal("colony").status, "Habitable world charted");
assert.equal(goal("colony").prospect, "garden");
assert.equal(goal("colony").action.select, "colony");
assert.equal(goal("colony").costs[0].units, 50);
garden.owner = "2";
assert.equal(goal("colony").prospect, null, "a rival's world is not offered");
garden.owner = null;
home.builds.push({ key: "colony", body_id: 4, complete_time: 1 });
state.simTime = 100;
assert.equal(goal("colony").actionLabel, "View Colony Ship build", "an expired build estimate is not an arrived hull");
const colonyShip = ghost("105", "colony");
state.ghosts.push({ ...colonyShip, own: false });
assert.equal(goal("colony").action.kind, "build", "rival hulls do not satisfy own readiness");
state.ghosts.pop(); state.ghosts.push(colonyShip); home.builds = [];
assert.equal(goal("colony").action.kind, "fleet");
assert.equal(goal("colony").action.id, "105");
assert.equal(goal("colony").status, "Colony Ship ready");
assert.equal(goal("colony").done, false, "a ready hull is not a held system");
garden.owner = "1";
assert.equal(goal("colony").done, true);
assert.equal(goal("colony").action.kind, "system");
assert.equal(goal("colony").action.id, "home");
assert.equal(goal("colony").actionLabel, "Develop home");
garden.owner = null;
home.structures.shipyard = 2; home.bodies[0].structures.shipyard = 2;
assert.equal(goal("upgrade").action.select, "corvette");
state.ghosts.push(ghost("104", "corvette"));
assert.equal(goal("upgrade").done, true);
const scout = ghost("103", "scout");
state.ghosts = [guard, freighter, scout];
escort.state = "offered"; escort.joined = false;
escort.expires_at = state.simTime - 1;
assert.equal(goal("upgrade").funding, null, "expired offers are not offered as attainable funding");
escort.expires_at = 900;
assert.equal(goal("upgrade").funding.id, "11");

// §camp: after the charter, the Founding Camp's projects are a goal until done.
home.bodies[0].founding_camp = { completed: ["prospect"] };
assert.deepEqual([...handoff.postVictoryHandoff().map(g => g.id)], ["colony", "upgrade", "camp"]);
assert.equal(goal("camp").status, "1/4 projects");
assert.deepEqual([...goal("camp").requirements.map(r => r.met)], [true, false, false, false]);
assert.deepEqual(JSON.parse(JSON.stringify(goal("camp").action)), { kind: "world", system: "home", body: 4 });
home.bodies[0].founding_camp.completed = ["prospect", "surface_workings", "storehouse", "quarters"];
assert.equal(goal("camp").done, true);
delete home.bodies[0].founding_camp;

const routes = [], worlds = [], focused = [];
const hooks = { go: r => routes.push(r), selectFleet: id => focused.push(id), openWorld: (...args) => worlds.push(args) };
const navigate = (act, id) => handoffUi.handleHandoffAction({ dataset: { deckAct: act, goal: id } }, ctx, hooks);
const sentBeforeNavigation = sent.length;
navigate("handoff-open", "colony");
assert.equal(routes.at(-1).name, "build"); assert.equal(routes.at(-1).query.select, "colony");
navigate("handoff-open", "upgrade");
assert.equal(routes.at(-1).name, "build"); assert.equal(routes.at(-1).query.select, "corvette");
assert.equal(routes.at(-1).query.body, "4");
navigate("handoff-prospect", "colony");
assert.equal(routes.at(-1).name, "system"); assert.equal(routes.at(-1).params.id, "garden");
navigate("handoff-contract", "upgrade");
assert.equal(routes.at(-1).query.contract, "11");
assert.equal(sent.length, sentBeforeNavigation, "handoff navigation never sends an order, accepts a job or pays a reward");
const withHandoff = new DeckStrategicRoutes(root, ctx, { ...hooks, openWorld: hooks.openWorld, selectFleets() {}, notice() {} });
const chosenContract = withHandoff.operationsHtml("13");
assert.ok(chosenContract.indexOf("Funding contract") < chosenContract.indexOf("Your next chapter"), "a funding link surfaces the chosen offer first");
assert.match(chosenContract, /data-deck-act="strategic-operation-accept"/);
assert.match(chosenContract, /Hull costs · local stock/);
assert.match(chosenContract, /900 cr/);
deps["./ledger"] = compile("shell/deck/ledger.ts");
const coached = [];
deps["../coach"] = { coach: (target, caption) => coached.push([target, caption]), clearCoach() {} };
const { DeckCommandRoutes } = compile("shell/deck/command.ts");
const guide = { id: "guide", hidden: true, classList: { toggle() {} }, innerHTML: "" };
const command = new DeckCommandRoutes(root, guide, ctx, { ...hooks, focusFleet: hooks.selectFleet, inbox: () => [], notice() {} });
assert.equal(command.foundingContent({ ...state.founding, stage: "export_production" }).copy,
  "Load Ferrite Ore, then send the Freighter.");
assert.equal(command.foundingContent({ ...state.founding, stage: "complete_export" }).copy,
  "Sell Ferrite Ore at the Market Hub.", "the guide must match the Ore-only completion rule");
assert.equal(command.foundingContent({ ...state.founding, stage: "survey_candidates" }).copy,
  "Build a Survey Probe and send it to the assigned stop. Its report charts a belt for your first Mining Barge.");
// §belts: the barge lesson follows the surveys: Shipyard first, then the barge, then the belt.
assert.match(command.foundingContent({ ...state.founding, stage: "build_barge" }).title, /Shipyard|Mining Barge/);
assert.equal(command.foundingContent({ ...state.founding, stage: "mine_belt" }).action, "mine-belt");
assert.equal(command.foundingContent({ ...state.founding, stage: "haul_ore" }).action, "barge-home");
assert.match(command.foundingContent({ ...state.founding, stage: "build_shipyard" }).copy, /15 Semiconductors/);
// §founding copy (Peter, Oct 27 2026: "reduce the repetitive text in the
// tutorial steps"): one line per fact on the desktop card itself.
{
  const shield = { label: "shield", detail: "" };
  const parts = command.planHtml({ chapter: { name: "Raid", done: 2, total: 4 }, checklist: [], progress: [],
    primary: { title: "Buy the Solar Array parts", copy: "8 Semiconductors", action: "merchant-power_plant", label: "Buy from trader", hint: "+8 MW.",
      alternatives: [{ title: "Buy the Solar Array parts", copy: "8 Semiconductors", action: "merchant-power_plant", label: "Buy from trader" },
        { title: "Build a Solar Array", copy: "30 Alloys", action: "build-power", label: "Build a Solar Array" }] },
    waiting: [], goal: null, blocked: false }, shield, true, state.founding, "");
  assert.doesNotMatch(parts, />Next</, "no eyebrow: the card is always next");
  assert.match(parts, /<b title="\+8 MW\.">Buy the Solar Array parts<\/b>/, "why is on hover");
  assert.equal(parts.match(/8 Semiconductors/g).length, 1, "the copy once, not again in its choice");
  assert.match(parts, /class="is-primary"[^>]*>Buy from trader<\/button>/);
  assert.match(parts, />Build a Solar Array<\/button>/, "a choice's button says the whole step");
  const waiting = command.planHtml({ chapter: { name: "Landfall", done: 1, total: 5 }, checklist: [], progress: [],
    primary: { title: "Bioharvester under construction", status: "Bioharvester under construction · 9s", copy: "", action: "home-world", label: "Open Sirius I" },
    waiting: [], goal: null, blocked: false }, shield, true, state.founding, "");
  assert.match(waiting, /<b>Bioharvester under construction · 9s<\/b>/, "a waiting card's title is its progress line");
  assert.doesNotMatch(waiting, /<p>/, "no empty copy line");
}
// The raid and the trader are news only once told: the stages' own
// fallbacks never name them.
assert.equal(command.foundingContent({ ...state.founding, stage: "sell_ore" }).action, "market");
assert.doesNotMatch(JSON.stringify(command.foundingContent({ ...state.founding, stage: "sell_ore" })), /trader/i);
assert.doesNotMatch(JSON.stringify(command.foundingContent({ ...state.founding, stage: "pirate_raid" })), /pirate|raid|privateer/i);
assert.equal(command.foundingContent({ ...state.founding, stage: "build_interceptor" }).action, "build-interceptor");
assert.equal(command.foundingContent({ ...state.founding, stage: "build_second_freighter" }).title,
  "Build a second Tiny Freighter");
assert.equal(command.foundingContent({ ...state.founding, stage: "build_second_freighter" }).action, "build-convoy");
assert.match(command.commandHtml(), /Requirements &amp; contract rewards/);
state.founding.stage = "grow_business";
command.renderFounding(true);
assert.match(guide.innerHTML,/Expand exports/);
assert.match(guide.innerHTML,/Start refining/);
assert.equal((guide.innerHTML.match(/data-deck-act="founding-action"/g)??[]).length,2);
const beforeResearchNavigation=sent.length;
command.runFoundingAction("research-enrichment",state.founding);
assert.equal(routes.at(-1).name,"research");
assert.equal(routes.at(-1).query.programme,"mat_enrichment");
assert.equal(sent.length,beforeResearchNavigation,"business goals navigate; they never dispatch or lock a profession");
state.founding.stage = "survey_candidates";
command.renderFounding(true);
assert.doesNotMatch(guide.innerHTML, /Next objectives &amp; rewards/, "the charter owns the guide until it is fulfilled");
assert.match(guide.innerHTML, /Founding 17\/23/);
assert.match(guide.innerHTML, /Build Survey Probe/);
assert.doesNotMatch(guide.innerHTML, /colony kit|Colony Ship/);
state.founding.stage = "complete";
command.renderFounding(true);
assert.match(guide.innerHTML, /Next objectives &amp; rewards/);
state.founding.protected = false;
command.renderFounding(true);
assert.equal(guide.hidden, false, "unfinished optional goals survive tutorial completion");
garden.owner = "1"; state.ghosts.push(ghost("104", "corvette"));
command.renderFounding(true);
assert.equal(guide.hidden, true, "a finished handoff does not leave a permanent floating checklist");
garden.owner = null; state.ghosts = [guard, freighter, scout];
const handoffPage = handoffUi.handoffHtml();
const mobileInputs = new Map();
deps["../../core/derive/research"] = compile("core/derive/research.ts");
// §node: the map draws only a dim ◈ for a node you don't hold; the hover and
// the system panels say what it is in these words.
{
  const { nodeLabel } = deps["../../core/derive/research"];
  const node = { bonus: "deep_scan", title: "Deep Scan", awakened: true, fed: false, region_radius: 0 };
  assert.equal(nodeLabel(node, false), "◈ Deep Scan", "another holder's upkeep is not ours to report");
  assert.equal(nodeLabel(node, true), "◈ Deep Scan · unfed");
  assert.equal(nodeLabel({ ...node, awakened: false }, false), "◈ Deep Scan · dormant");
}
const { MobileParitySurfaces } = compile("shell/mobile/parity.ts", { document: { getElementById: id => mobileInputs.get(id) } });
const sheetsOpened = []; let refreshes = 0;
const mobile = new MobileParitySurfaces(ctx, { refresh() { refreshes++; } }, {
  openSheet: s => sheetsOpened.push(s), focusFleet: id => focused.push(id), focusSystem: id => located.push(id),
});
const mobileQueue = mobile.systemConstruction("home", queueWorld);
assert.match(mobileQueue, /<b>Freya I<\/b><small>Shipyard I · 20s · \+2 queued<\/small>.*<b>Freya II<\/b><small>Habitat I · 20s<\/small>/,
  "mobile reads each world's slot in the same one line");
assert.doesNotMatch(mobileQueue, /paused · needs workforce|m-progress/i,
  "a waiting structure is not an unstaffed hull, and the system sheet draws no bar");
assert.match(mobile.handoffHtml(), /Your next chapter/);
assert.match(mobile.handoffHtml(), /Build Colony Ship/);
assert.match(mobile.handoffHtml(), /Inspect Garden/);
assert.doesNotMatch(mobile.handoffHtml(), /colony kit|Found a specialist colony/);
mobile.openHandoffGoal("next-prospect", "colony");
assert.equal(located.at(-1), "garden");
mobile.openHandoffGoal("next-goal", "colony");
assert.equal(sheetsOpened.at(-1).id, "shipyard"); assert.equal(mobile.selectedHull, "colony");
mobile.openHandoffGoal("next-goal", "upgrade");
assert.equal(sheetsOpened.at(-1).id, "shipyard"); assert.equal(mobile.selectedHull, "corvette");
assert.equal(sheetsOpened.at(-1).props.bodyId, 4);
mobile.openHandoffGoal("next-funding", "upgrade");
assert.equal(mobile.handoffContract, "11"); assert.equal(refreshes, 1);
assert.equal(sent.length, sentBeforeNavigation, "mobile handoff is navigation-only too");
{
  const trace = { ...operation("countertrace", { kind: "survey_expedition", system: "hideout" },
    "counter_raid_trace", "1/3 · Trace the home raiders"), state: "active", joined: true };
  state.operations.push(trace);
  const card = mobile.operationCard(trace);
  assert.match(card, /Plot survey/);
  assert.match(card, /value="103"/);
  assert.doesNotMatch(card, /value="101"|value="102"/);
  mobileInputs.set("m-op-fleet-countertrace", { value: "103" });
  const sentBefore = sent.length;
  mobile.handleClick({ target: { closest: () => ({ dataset: { mobileAct: "operation-assign", id: trace.id } }) } });
  assert.equal(sent.length, sentBefore);
  assert.deepEqual(JSON.parse(JSON.stringify(previews.at(-1).commands)), [
    { type: "AssignOperationFleet", operation_id: trace.id, fleet_id: "103" },
    { type: "SurveySystem", fleet_id: "103", system_id: "hideout" },
  ]);
  assert.match(mobile.renderOperations().html, /Defend home · strike back/);
  state.operations.pop();
}
{
  const prize = { ...operation("prize", { kind: "prize_recovery", prize: "screen_cache", system: "hideout", pos: { x: 80_000, y: 0 } },
    "site_recovery", "Recover Fleet-screen cache"), state: "active", joined: true };
  state.operations.push(prize);
  const card = mobile.operationCard(prize);
  assert.match(card, /Plot recovery.*Recover now/);
  assert.match(card, /8 module crates · Battleship dossier/);
  assert.doesNotMatch(card, /value="101"|value="103"/, "only the Freighter can recover equipment");
  mobileInputs.set("m-op-fleet-prize", { value: freighter.id });
  const before = sent.length;
  mobile.handleClick({ target: { closest: () => ({ dataset: { mobileAct: "operation-assign", id: prize.id } }) } });
  assert.equal(sent.length, before);
  assert.equal(previews.at(-1).commands[1].type, "MoveShip");
  assert.match(mobile.programmeCard({ id: "cruiser", name: "Cruiser Hull", blurb: "", tier: 5,
    state: "locked", cost: 1000, recovered_data: 300 }), /30% research work banked/);
  // §rare-materials: a programme's basket shows per research-hour, a good
  // listed twice (Hulls' Machinery) merged into one figure.
  const supplied = mobile.programmeCard({ id: "hull_v", name: "Hull V", blurb: "", tier: 5, state: "available", cost: 1000,
    supplies: [["electronics", 0.05], ["exotic_matter", 0.001], ["machinery", 0.004], ["machinery", 0.006]] });
  assert.match(supplied, /Supplies \/h.*<b>180<\/b>.*Exotic Matter.*<b>3\.6<\/b>.*<b>36<\/b>/);
  assert.doesNotMatch(mobile.programmeCard({ id: "hull_v", name: "Hull V", blurb: "", tier: 5, state: "completed", cost: 1000,
    supplies: [["electronics", 0.05]] }), /Supplies/, "a finished programme draws nothing");
  state.operations.pop();
}
// §charter-v5: eighteen steps in five chapters. A founder lands with only its
// Command Center: no ships, no free power, and a land grant on the landing world.
{
  deps["./industry"] = { availableStock: (s, c) => Math.max(0, s.stockpile?.find(x => x.commodity === c)?.units ?? 0) };
  deps["./merchant"] = compile("core/derive/merchant.ts");
  Object.assign(founding, compile("core/derive/founding.ts"));
  const saved = { founding: state.founding, ghosts: state.ghosts, pending: state.pendingOrders, merchant: state.merchant, galaxy: state.galaxy,
    captains: state.captains };
  state.captains = [];
  const hull = (id, kind, extra = {}) => ({ id, kind, own: true, pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 }, age: 0,
    composition: [{ kind, count: 1 }], cargo_manifest: [], ...extra });
  const onRoute = id => hull(id, "tiny_freighter", { industry: { kind: "route", run: { route: { stops: [{ port: { kind: "hub" } }] },
    stop: 0, phase: "travelling", remaining_load: [], remaining_unload: [], visits: 0 } } });
  const landing = { id: 1, name: "Freya I", structures: { command_center: 1 }, resource_slots: 2, founding_camp: { completed: ["a", "b", "c", "d"] },
    deposits: [{ resource: "biomass", richness: 1 }, { resource: "metallic_ore", richness: 0.8 }] };
  const rock = { id: 3, name: "Freya III", structures: {}, resource_slots: 1, deposits: [{ resource: "metallic_ore", richness: 0.9 }] };
  const home = { id: "home", builds: [], modules: {}, storage_used: 120, storage_cap: 1000, stockpile: [], bodies: [landing, rock], assignments: [] };
  const crew = structure => ({ body_id: 1, structure, workers: 1, specialists: {} });
  state.galaxy = { ...state.galaxy, build_options: [
    ["bioharvester", [["alloys", 25], ["machinery", 12]]], ["mining_complex", [["alloys", 25], ["machinery", 12]]],
    ["power_plant", [["alloys", 30], ["machinery", 12], ["electronics", 8]]],
    ["shipyard", [["alloys", 40], ["machinery", 20], ["electronics", 15]]], ["raider", [["alloys", 20], ["electronics", 12], ["armaments", 15], ["warp_core", 1]]],
    ["tiny_freighter", [["alloys", 20], ["machinery", 8], ["polymers", 8], ["warp_core", 1]]],
  ].map(([key, costs]) => ({ key, costs: costs.map(([commodity, units]) => ({ commodity, units })) })) };
  state.merchant = null;
  state.founding = { stage: "build_bioharvester", local_opening: true, bounty_received: false, protected: true, land_grant_open: false,
    protection_min_until: 0, protection_max_until: 90_000, expansion_unlocked: false, opening_exports: [], survey_candidates: ["stop"] };
  state.ghosts = [];
  state.pendingOrders = new Map();
  const plan = () => founding.foundingPlan(state, home);
  const stage = s => { state.founding.stage = s; return plan(); };

  // Landfall: the colony ship is the Command Center; the settlers build around it.
  let p = plan();
  assert.equal(p.chapter.name, "Landfall");
  assert.deepEqual([...p.checklist.map(item => item.title)],
    ["Open your home system", "Open your home world", "Harvest biomass", "House the settlers", "Make provisions", "Bring the mine online"]);
  // §home-step: first, open the home system (a client step before the server's eighteen).
  state.galaxy.systems[0].name = "Freya";
  p = plan();
  assert.equal(p.primary.action, "open-home");
  assert.equal(p.primary.copy, "Click Freya on the map.");
  assert.equal(p.primary.label, "Open Freya");
  assert.equal(p.waiting.length, 0, "nothing else asks for attention yet");
  assert.equal(p.chapter.done, 0);
  delete state.galaxy.systems[0].name;
  state.systems = [home];
  assert.equal(founding.homeStepSystem(state, "home"), "home", "a click on home flies in");
  assert.equal(founding.homeStepSystem(state, null), null);
  assert.equal(founding.noteHomeOpened(state, "elsewhere", "home"), false, "another system's view doesn't count");
  assert.equal(founding.noteHomeOpened(state, "home", "home"), true, "home's System View does");
  assert.equal(founding.noteHomeOpened(state, "home", "home"), false, "once");
  assert.equal(founding.homeStepSystem(state, "home"), null, "done: clicks select again");
  p = plan();
  assert.equal(p.checklist[0].done, true);
  // §home-world-step (Peter, Oct 28 2026): then one of home's worlds, to see the planet view.
  assert.deepEqual([p.checklist[1].title, p.checklist[1].done], ["Open your home world", false]);
  assert.deepEqual([p.primary.action, p.primary.copy, p.primary.label], ["home-world", "Click Freya I in the system.", "Open Freya I"]);
  assert.equal(founding.noteHomeWorldOpened(state, "elsewhere", "home"), false, "another system's world doesn't count");
  assert.equal(founding.noteHomeWorldOpened(state, "home", "home"), true, "a world of home does");
  assert.equal(founding.noteHomeWorldOpened(state, "home", "home"), false, "once");
  p = plan();
  assert.equal(p.checklist[1].done, true);
  assert.equal(p.primary.action, "build-bioharvester");
  state.homeWorldOpened = false;
  state.homeOpened = false;
  home.builds = [{ key: "habitat", body_id: 1, complete_time: state.simTime + 30, queued: false }];
  assert.equal(plan().primary.action, "build-bioharvester", "anything built or queued at home passes it too (a reload)");
  home.builds = [];
  state.homeOpened = true;
  state.homeWorldOpened = true;
  state.systems = [];
  // §camp: the Founding Camp's projects wait for the charter's end.
  landing.founding_camp.completed = [];
  assert.ok(!plan().waiting.some(card => card.action === "home-development"), "no settlement card during the charter");
  assert.equal(founding.campOpen(state), false);
  assert.equal(founding.campOpen({ founding: { stage: "complete" } }), true);
  assert.equal(founding.campOpen({ founding: null }), true);
  landing.founding_camp.completed = ["a", "b", "c", "d"];
  // Both shells latch the world step: the desktop world route, the phone planet sheet.
  assert.match(src("shell/deck/index.ts"), /route\?\.name === "world" && !this\.ctx\.state\.homeWorldOpened/);
  assert.match(src("shell/mobile/index.ts"), /sheet\?\.id === "planet" && !this\.ctx\.state\.homeWorldOpened/);
  // Both shells fly in on that click (or tap) and on the guide's button.
  assert.match(src("shell/deck/map.ts"), /if \(this\.hooks\.diveOnSelect\(result\.target\.id\)\) this\.flyInto\(result\.target\.id\);/);
  assert.match(src("shell/mobile/map.ts"), /if \(this\.hooks\.diveOnSelect\(target\.id\) && this\.flyInto\(target\.id\)\) break;/);
  assert.match(src("shell/deck/command.ts"), /action === "open-home" && homeId\) \{\n\s*this\.hooks\.flyIntoSystem\(homeId\);/);
  assert.match(src("shell/mobile/surfaces.ts"), /if \(action === "open-home"\) \{\n\s*const id = this\.homeSystemId\(\);\n\s*if \(id\) this\.hooks\.flyIntoSystem\(id\);/);
  assert.match(src("render.ts"), /flyIntoSystem\(sys: SystemInfo, bodies: BodyView\[\]\): boolean/);
  p = plan();
  assert.equal(p.primary.action, "build-bioharvester");
  assert.match(p.primary.copy, /25 Alloys · 12 Machinery/, "the recipe comes from the served catalogue");
  landing.structures.bioharvester = 1;
  assert.equal(plan().primary.action, "bioharvester", "then staff it");
  assert.deepEqual([plan().primary.copy, plan().primary.label], ["", "Assign workforce"], "the button says it: no copy");
  // §workforce cap: with every worker posted, the + is off; the guide says where to find one.
  home.workforce = { units: 6, posted: 6 };
  assert.equal(plan().primary.copy, "No free workforce: take one off another building.");
  home.workforce = { units: 6, posted: 0 };
  home.assignments.push(crew("bioharvester"));
  home.builds = [{ key: "habitat", body_id: 1, complete_time: state.simTime + 30, queued: false }];
  p = stage("build_habitat");
  assert.equal(p.primary.title, "Habitat under construction");
  // One line per fact (Peter, Oct 27 2026): a waiting card's progress line is its title.
  assert.equal(p.primary.status, "Habitat under construction · 30s");
  assert.equal(p.progress.length, 0, p.progress.join(" | "));
  assert.equal(p.checklist[0].done, true);
  home.builds = [];
  p = stage("build_mine");
  assert.equal(p.primary.action, "build-mine");
  assert.equal(p.primary.copy, "25 Alloys · 12 Machinery", "figures only");
  assert.equal(p.primary.label, "Build", "the title names the thing");
  assert.match(p.primary.hint, /Freya I's Ferrite Ore\. The Command Center's 1 MW reactor runs it/);
  // §charter-gates: the privateer sets out as the mine starts. Once command
  // sees it, its approach shows here, timed to the well edge (900 su), where
  // the raid begins and the mine comes online: 2,100 su at its warp cruise
  // is 8 s (11 s to the star itself).
  Object.assign(state.founding, { raid_seen: true, privateer: "p" });
  state.ghosts = [{ ...hull("p", "raider", { pos: { x: 3_000, y: 0 }, vel: { x: -200, y: 0 } }), own: false }];
  assert.ok(plan().progress.includes("Raider closing on the colony · ETA 8s"), plan().progress.join(" | "));
  Object.assign(state.founding, { raid_seen: false, privateer: null });
  state.ghosts = [];

  // Raid: as the mine comes online, a privateer strikes. §no-spoilers: the
  // guide keeps it back until command sees it, then keeps to what happens.
  p = stage("pirate_raid");
  assert.equal(p.chapter.name, "Industry", "a plain chapter until the raid is seen");
  assert.deepEqual([...p.checklist.map(item => item.title)], ["Power the Shipyard", "Build a Shipyard"]);
  assert.equal(p.primary.title, "The mine is running");
  assert.doesNotMatch(JSON.stringify(p), /pirate|raid|privateer/i);
  Object.assign(state.founding, { raid_seen: true, privateer: "p" });
  state.ghosts = [{ ...hull("p", "raider", { pos: { x: 5_000, y: 0 }, vel: { x: -200, y: 0 } }), own: false }];
  p = plan();
  assert.equal(p.chapter.name, "Raid", "seen, the chapter is told");
  assert.deepEqual([...p.checklist.map(item => item.title)], ["Pirates raid the colony", "Build a Gun Platform", "Power the Shipyard", "Build a Shipyard"],
    "§gun-platform: told with the raid");
  assert.equal(p.primary.title, "Raider closing on the colony");
  assert.equal(p.blocked, true);
  assert.match(p.primary.status, /^Raider closing on the colony · ETA /);
  assert.equal(p.progress.length, 0, p.progress.join(" | "));
  state.ghosts[0].pos = { x: 600, y: 0 };
  assert.equal(plan().primary.title, "Raider over the colony");
  state.ghosts = [];
  // §gun-platform (Peter, Oct 30 2026): the report hands on to the home's
  // defense first, a Gun Platform over the landing world.
  p = stage("build_gun_platform");
  assert.deepEqual([p.primary.title, p.primary.action, p.primary.label], ["Build a Gun Platform", "build-gun-platform", "Build"]);
  assert.equal(p.chapter.name, "Raid");
  // The raid's report hands on to the ore sale; the trader stays unnamed
  // until its own pop-up has told it.
  state.founding.raided = true;
  p = stage("sell_ore");
  assert.deepEqual([...p.checklist.map(item => item.title)], ["Pirates raid the colony", "Build a Gun Platform", "Power the Shipyard", "Build a Shipyard"]);
  assert.equal(p.primary.title, "Find semiconductors");
  assert.match(p.primary.copy, /The Solar Array needs 8/);
  assert.doesNotMatch(JSON.stringify([p.primary, p.waiting, p.progress]), /trader|merchant/i);
  state.merchant = { fleet: "m", system: "home", phase: "docked", leave_by: 9_999, markup: 0.5, ore_price: 4, ore_room: 300,
    goods: [{ commodity: "electronics", stock: 100, price: 51, hub_mid: 34 }, { commodity: "armaments", stock: 15, price: 84, hub_mid: 56 },
      { commodity: "deuterium", stock: 30, price: 38, hub_mid: 25 }] };
  p = plan();
  assert.doesNotMatch(JSON.stringify([p.primary, p.waiting, p.progress, p.checklist]), /trader|merchant/i,
    "docked, but its pop-up has not told it yet: still no word of it");
  state.storyTold = new Set(["free_trader"]);
  p = plan();
  assert.equal(p.primary.title, "Sell ore to the trader");
  assert.equal(p.primary.copy, "We need semiconductors for our ships.");
  assert.equal(p.primary.action, "merchant-ore");
  assert.equal(p.primary.label, "Sell");
  assert.deepEqual([...p.checklist.map(item => item.title)],
    ["Pirates raid the colony", "Build a Gun Platform", "Sell ore to the trader", "Power the Shipyard", "Build a Shipyard"]);
  home.stockpile = [{ commodity: "metallic_ore", units: 412 }];
  assert.deepEqual(JSON.parse(JSON.stringify(deps["./merchant"].merchantErrand("ore", home, state.merchant))),
    { buys: {}, sells: { metallic_ore: 300 } }, "it fills in all the ore the trader will take");
  home.stockpile = [];
  // Then the Solar Array, its semiconductors from the trader.
  p = stage("power_shipyard");
  assert.equal(p.primary.action, "merchant-power_plant", "the docked trader sells the Solar Array's semiconductors");
  assert.match(p.primary.copy, /8 Semiconductors/);
  assert.doesNotMatch(p.primary.copy, /\+50%|Hub/, "its markup stays its own business");
  assert.deepEqual([...p.primary.alternatives.map(a => a.action)], ["merchant-power_plant", "build-power"]);
  home.stockpile = [{ commodity: "electronics", units: 8 }];
  assert.equal(plan().primary.action, "build-power", "bought: build it");
  home.stockpile = [];
  const trader = state.merchant;
  state.merchant = null;

  // Pirates: an Interceptor from Shipyard I and the hunt for the lurking privateer.
  landing.structures.shipyard = 1;
  home.assignments.push(crew("shipyard"));
  p = stage("build_interceptor");
  assert.equal(p.chapter.name, "Pirates");
  assert.equal(p.primary.action, "build-interceptor", "Shipyard I builds Interceptors");
  assert.equal(p.primary.hint, "The privateer lurks nearby.");
  // Peter (Oct 30 2026): the Interceptor's captain is not optional.
  state.ghosts.push(hull("i", "raider", { docked: "home", fuel: 7, fuel_capacity: 7 }));
  state.captains = [{ id: 1, name: "Ada Voss", assigned_fleet: null, stationed_system: "home", recovering_until: null,
    loss_fate: null, report: null }];
  p = plan();
  assert.deepEqual([p.primary.title, p.primary.copy, p.primary.action], ["Assign a captain", "Ada Voss → Interceptor", "assign-captain"]);
  assert.equal(p.waiting.length, 0, "no way round it");
  Object.assign(state.ghosts.find(g => g.id === "i"), { docked: null, pos: { x: 3_000, y: 0 } });
  p = plan();
  assert.deepEqual([p.primary.title, p.primary.action], ["Bring the Interceptor home", "interceptor-captain"], "the captain boards at the dock");
  assert.deepEqual(JSON.parse(JSON.stringify(founding.foundingCommands("interceptor-captain", state, home))),
    [{ type: "MoveShip", ship_id: "i", dest: { x: 0, y: 0 } }]);
  Object.assign(state.ghosts.find(g => g.id === "i"), { docked: "home", pos: { x: 0, y: 0 }, defend_system: { system: "home", station: { x: 0, y: 0 }, radius: 20_000 } });
  assert.equal(plan().primary.title, "Stand the Interceptor down", "a captain boards only a ship at rest");
  state.ghosts.find(g => g.id === "i").defend_system = null;
  state.captains[0].assigned_fleet = "i";
  assert.equal(plan().primary.title, "Captain boarding", "until the step's report");
  assert.equal(founding.foundingCommands("interceptor-captain", state, home), null);
  state.captains = [];
  assert.equal(plan().primary.action, "build-interceptor", "no captain free: the hull alone completes the step");
  state.ghosts = state.ghosts.filter(g => g.id !== "i");
  p = stage("defeat_privateer");
  assert.equal(p.primary.title, "Interceptor lost");
  assert.equal(p.blocked, true);
  state.ghosts.push(hull("i", "raider", { docked: "home", fuel: 7, fuel_capacity: 7 }));
  p = plan();
  assert.equal(p.primary.title, "Destroy the privateer");
  assert.equal(p.primary.action, "command", "out of sight, nothing to aim at");
  // §privateer-hunt (Peter, Oct 27 2026): after its raid it lies quiet where it
  // went to ground; the Interceptor is sent there and finds it on arrival.
  state.founding.privateer_last_seen = { x: 6_600, y: 0 };
  p = plan();
  assert.deepEqual([p.primary.title, p.primary.action, p.primary.label], ["Find the privateer", "hunt-privateer", "Hunt"]);
  assert.deepEqual(JSON.parse(JSON.stringify(founding.foundingCommands("hunt-privateer", state, home))),
    [{ type: "MoveShip", ship_id: "i", dest: { x: 6_600, y: 0 } }]);
  state.pendingOrders = new Map([["i", [{ kind: "move", dest: { x: 6_600, y: 0 } }]]]);
  p = plan();
  assert.equal(p.primary.title, "Interceptor hunting the privateer", "never offered twice while the order travels");
  assert.equal(p.primary.action, "raider", "watching it means watching the Interceptor");
  assert.match(p.primary.status, /^Interceptor hunting the privateer · ETA/);
  state.pendingOrders = new Map();
  const hunter = state.ghosts.find(g => g.id === "i");
  Object.assign(hunter, { pos: { x: 6_200, y: 0 }, docked: null });
  p = plan();
  assert.equal(p.primary.title, "Interceptor lying in wait", "there before it, or before its light");
  Object.assign(hunter, { pos: { x: 0, y: 0 }, docked: "home" });
  // In sight (tracked since the raid): a replacement's reserve captain first, then a one-click attack.
  state.ghosts.push({ ...hull("p", "raider", { pos: { x: 9_000, y: 0 } }), own: false });
  assert.equal(founding.foundingCommands("hunt-privateer", state, home), null, "in sight, it's an attack");
  state.captains = [{ id: 1, name: "Ada Voss", assigned_fleet: null, stationed_system: "home", recovering_until: null,
    loss_fate: null, report: null }];
  p = plan();
  assert.equal(p.primary.action, "assign-captain");
  assert.equal(p.primary.copy, "Ada Voss → Interceptor");
  assert.ok(!p.waiting.some(step => ["attack-privateer", "hunt-privateer"].includes(step.action)), "Peter: the captain is not optional");
  assert.deepEqual(JSON.parse(JSON.stringify(founding.foundingCommands("assign-captain", state, home))), [{ type: "AssignCaptain", captain_id: 1, fleet_id: "i" }]);
  state.captains[0].assigned_fleet = "i";
  p = plan();
  assert.equal(p.primary.action, "attack-privateer");
  assert.equal(p.primary.reason, "Bounty: 8,300 Cr.");
  assert.deepEqual(JSON.parse(JSON.stringify(founding.foundingCommands("attack-privateer", state, home))), [{ type: "AttackFleet", fleet_id: "i", target_id: "p" }]);
  // An empty tank at home: the trader's Deuterium fills it.
  state.ghosts.find(g => g.id === "i").fuel = 0;
  state.merchant = trader;
  p = plan();
  assert.equal(p.primary.action, "merchant-fuel");
  assert.equal(founding.fuelRoom(state), 7);
  assert.deepEqual(JSON.parse(JSON.stringify(deps["./merchant"].merchantErrand("fuel", home, state.merchant, founding.fuelRoom(state)))),
    { buys: { deuterium: 7 }, sells: {} });
  state.ghosts.find(g => g.id === "i").fuel = 7;
  state.merchant = null; state.storyTold = new Set();
  state.pendingOrders = new Map([["i", [{ kind: "attack", target_id: "p" }]]]);
  p = plan();
  assert.equal(p.primary.title, "Interceptor closing on the privateer", "never offered twice while the order travels");
  assert.match(p.primary.status, /^Interceptor closing on the privateer · ETA /);
  assert.equal(p.progress.length, 0, p.progress.join(" | "));
  state.pendingOrders = new Map();

  // The privateer's log names its den: the Interceptor goes there next.
  Object.assign(state.founding, { bounty_received: true, den: "den" });
  state.galaxy = { ...state.galaxy, systems: [...(state.galaxy.systems ?? []), { id: "den", name: "Tarn", pos: { x: 20_000, y: 0 } }] };
  state.ghosts = state.ghosts.filter(g => g.id !== "p");
  p = stage("eliminate_den");
  assert.equal(p.chapter.name, "Pirates");
  assert.deepEqual([...p.checklist.map(item => item.title)].slice(-2), ["Destroy the privateer", "Eliminate the privateer den"]);
  assert.equal(p.primary.title, "Destroy the den at Tarn");
  assert.equal(p.primary.action, "assault-den");
  assert.deepEqual(JSON.parse(JSON.stringify(founding.foundingCommands("assault-den", state, home))), [{ type: "MoveShip", ship_id: "i", dest: { x: 20_000, y: 0 } }]);
  // §charter-v5: the trader's last call. The bounty paid, it leaves within
  // minutes; the guide has the bounty buy what the builds ahead lack.
  state.merchant = { ...trader, leave_by: state.simTime + 150 };
  state.storyTold = new Set(["free_trader"]);
  p = plan();
  assert.equal(p.primary.title, "Stock up before the trader leaves");
  assert.equal(p.primary.copy, "52 Semiconductors, 15 Armaments for the builds and research ahead.",
    "the catalogue's builds ahead, the vendetta's second and third Interceptors and 20 for research");
  assert.equal(p.primary.action, "merchant-ahead");
  assert.deepEqual([...p.primary.alternatives.map(a => a.action)], ["merchant-ahead", "assault-den"]);
  assert.deepEqual([...p.primary.alternatives.map(a => a.label)], ["Buy from trader", "Destroy the den at Tarn"], "each choice says the whole step");
  assert.match(p.primary.status, /^Stock up · trader leaves in /, "the countdown is its headline");
  assert.equal(p.progress.length, 0, p.progress.join(" | "));
  assert.deepEqual(JSON.parse(JSON.stringify(deps["./merchant"].merchantShortfall(state, home, "ahead", state.merchant))), { electronics: 52, armaments: 15 });
  home.stockpile = [{ commodity: "electronics", units: 32 }, { commodity: "armaments", units: 30 }];
  assert.equal(plan().primary.copy, "20 Semiconductors for the builds and research ahead.", "the builds' covered: research's reserve still short");
  home.stockpile = [{ commodity: "electronics", units: 52 }, { commodity: "armaments", units: 30 }];
  p = plan();
  assert.equal(p.primary.action, "assault-den", "stocked up: back to the den");
  assert.ok(p.progress[0].startsWith("Trader leaves in "));
  home.stockpile = [];
  state.merchant = null; state.storyTold = new Set();
  const interceptor = state.ghosts.find(g => g.id === "i");
  Object.assign(interceptor, { path: [{ pos: { x: 20_000, y: 0 } }], vel: { x: 100, y: 0 }, docked: undefined });
  p = plan();
  assert.equal(p.primary.title, "Interceptor en route to Tarn");
  assert.match(p.primary.status, /^Interceptor en route to Tarn · ETA /);
  interceptor.pos = { x: 19_950, y: 0 };
  assert.equal(plan().primary.status, "Interceptor at the den · Tarn");
  state.ghosts = state.ghosts.filter(g => g.id !== "i");
  p = plan();
  assert.equal(p.primary.title, "Interceptor lost");

  // Past the den, nothing points at the belt it guarded before the survey
  // step (Peter, Oct 29 2026: no long-term goal card).
  const candidates = state.founding.survey_candidates;
  state.founding.survey_candidates = ["den"];

  // §charter-v5 (Oct 16 2026): Teo left with the bounty; the ore goes to the
  // Market Hub on a Tiny Freighter.
  state.galaxy = { ...state.galaxy, hub: { x: 125_000, y: 0 } };
  p = stage("prepare_export");
  assert.equal(p.chapter.name, "Expansion");
  assert.deepEqual([...p.checklist.map(item => item.title)],
    ["Export your ore", "Research Enrichment", "Build a Smelter", "Expand mining operations", "Chart a nearby system"]);
  assert.equal(p.primary.title, "Build a Tiny Freighter");
  assert.equal(p.primary.action, "build-convoy");
  assert.equal(p.primary.copy, "20 Alloys · 8 Machinery · 8 Polymers · 1 Warp Core");
  assert.match(p.primary.hint, /It fuels with Uranium/);
  assert.equal(p.primary.reason, "The Market Hub buys ore.");
  assert.ok(!("goal" in p), "no long-term goal card");
  state.market = { prices: [{ commodity: "metallic_ore", price: 8.04 }] };
  assert.equal(plan().primary.reason, "The Market Hub pays 8.0 Cr a unit.");
  home.builds = [{ key: "tiny_freighter", body_id: 1, complete_time: state.simTime + 12, queued: false }];
  assert.equal(plan().primary.title, "Tiny Freighter under construction");
  home.builds = [];
  state.ghosts.push(hull("f", "tiny_freighter", { docked: "home" }));
  p = plan();
  assert.equal(p.primary.title, "Export your ore");
  assert.equal(p.primary.action, "opening-imports");
  state.ghosts[state.ghosts.length - 1] = { ...onRoute("f"), pos: { x: 25_000, y: 0 }, vel: { x: 100, y: 0 } };
  p = plan();
  assert.equal(p.primary.title, "Shipment heading to the Market Hub");
  assert.equal(p.primary.action, "shipment-trip");
  assert.match(p.primary.status, /^Shipment heading to the Market Hub · ETA /);
  assert.ok(!p.progress.some(line => line.startsWith("Shipment")), p.progress.join(" | "));
  // Storage full while it's away: build a Warehouse (the camp's storehouse
  // waits for the charter's end).
  home.storage_used = 1_000;
  p = plan();
  assert.equal(p.primary.title, "Make room at home");
  assert.equal(p.primary.action, "build-warehouse");
  assert.equal(p.primary.label, "Build Warehouse");
  assert.equal(founding.foundingBuildTarget("build-warehouse", state, home).select, "warehouse");
  home.storage_used = 120;
  assert.ok(stage("first_research").progress[0].startsWith("Shipment heading to the Market Hub"), "its trip stays in view");
  state.ghosts.pop();
  state.market = null;
  p = stage("first_research");
  assert.ok(!("goal" in p), "no long-term goal card");
  assert.ok(!p.progress.some(line => /Survey|Tarn/.test(line)), p.progress.join(" | "));
  state.ghosts.push(hull("probe", "survey_probe", { docked: "home" }));
  assert.deepEqual(JSON.parse(JSON.stringify(founding.foundingCommands("survey-stop", state, home))),
    [{ type: "SurveySystem", fleet_id: "probe", system_id: "den" }], "the survey step's Send still targets the den");
  state.ghosts.pop();
  state.founding.survey_candidates = candidates;
  state.founding.den = null;

  // §refining (Peter, Oct 30 2026): Enrichment, then the Smelter it unlocks.
  landing.structures.academy = 1;
  home.assignments.push(crew("academy"));
  p = stage("first_research");
  assert.deepEqual([p.primary.title, p.primary.action, p.primary.hint], ["Research Enrichment", "research", "It unlocks the Smelter."]);
  const savedResearch = state.research;
  state.research = { active: { id: "mat_enrichment", name: "Enrichment", progress: 10, cost: 190, eta_secs: 180 }, programmes: [] };
  p = stage("build_smelter");
  assert.equal(p.chapter.name, "Expansion");
  assert.equal(p.primary.status, "Enrichment under way · 180s", "the Smelter waits on its research");
  state.research = { active: null, programmes: [{ id: "mat_enrichment", state: "completed" }] };
  p = plan();
  assert.deepEqual([p.primary.title, p.primary.action], ["Build a Smelter", "build-smelter"]);
  assert.equal(founding.foundingBuildTarget("build-smelter", state, home).body.id, 1, "the landing world's free industrial site");
  landing.structures.smelter = 1;
  assert.deepEqual([plan().primary.title, plan().primary.action], ["Staff the Smelter", "smelter"]);
  delete landing.structures.smelter;
  state.research = savedResearch;

  // Expansion: "Expand mining operations" opens the land grant.
  Object.assign(state.founding, { bounty_received: true, land_grant_open: true });
  p = stage("expand_mining");
  assert.equal(p.chapter.name, "Expansion");
  // The Smelter's want of ore is what the second mine answers.
  assert.equal(p.primary.reason, undefined);
  home.converters = [{ body_id: 1, structure: "smelter", title: "Smelter", tier: 1, status: "no_inputs" }];
  assert.equal(plan().primary.reason, "The Smelter is short of ore.");
  delete home.converters;
  assert.deepEqual([...p.checklist.map(item => item.title)], ["Export your ore", "Research Enrichment", "Build a Smelter", "Expand mining operations", "Chart a nearby system"]);
  assert.equal(founding.expansionBody(home).id, 3, "another world of the home system with ore");
  assert.equal(p.primary.action, "build-expansion-power");
  assert.equal(p.primary.title, "Power Freya III");
  rock.structures.power_plant = 1;
  assert.equal(plan().primary.action, "build-expansion-mine");
  rock.structures.mining_complex = 1;
  assert.equal(plan().primary.action, "expansion-mine");

  // Until then every guide build stands on the landing world.
  state.founding.land_grant_open = false;
  for (const action of ["build-shipyard", "build-power", "build-mine", "build-academy", "build-habitat"]) {
    assert.equal(founding.foundingBuildTarget(action, state, home).body.id, 1, `${action}: the landing world`);
  }
  assert.equal(founding.foundingBuildTarget("build-expansion-power", state, home).body.id, 3);
  assert.deepEqual({ ...founding.foundingBuildTarget("build-interceptor", state, home), body: undefined },
    { body: undefined, mode: "ships", select: "raider" });
  Object.assign(state, saved);
}
// §merchant: a docked free trader turns a short build step into "buy the
// parts", the ordinary build beside it; no docked merchant, the ordinary step.
{
  deps["./industry"] = { availableStock: (s, c) => Math.max(0, s.stockpile?.find(x => x.commodity === c)?.units ?? 0) };
  deps["./merchant"] = compile("core/derive/merchant.ts");
  Object.assign(founding, compile("core/derive/founding.ts"));
  const saved = { founding: state.founding, merchant: state.merchant, galaxy: state.galaxy };
  const home = { id: "home", builds: [], modules: {}, storage_used: 120, storage_cap: 250,
    stockpile: [{ commodity: "alloys", units: 25 }, { commodity: "machinery", units: 7 }],
    bodies: [{ id: 1, structures: { mining_complex: 1 } }], assignments: [] };
  state.galaxy = { ...state.galaxy, build_options: [{ key: "shipyard", costs: [{ commodity: "alloys", units: 40 },
    { commodity: "machinery", units: 20 }, { commodity: "electronics", units: 15 }] }] };
  // Before the bounty: the trader's last call (its stock-up) is not this card.
  state.founding = { stage: "build_shipyard", local_opening: true, bounty_received: false, protected: true,
    protection_min_until: 0, protection_max_until: 90_000, expansion_unlocked: false, opening_exports: [], survey_candidates: ["stop"] };
  state.merchant = null;
  assert.equal(founding.foundingPlan(state, home).primary.action, "build-shipyard");
  state.merchant = { fleet: "m", system: "home", phase: "docked", leave_by: 9_999, markup: 0.5, ore_price: 4, ore_room: 150,
    goods: [{ commodity: "alloys", stock: 120, price: 39, hub_mid: 26 }, { commodity: "machinery", stock: 50, price: 93, hub_mid: 62 },
      { commodity: "electronics", stock: 60, price: 51, hub_mid: 34 }] };
  assert.equal(founding.foundingPlan(state, home).primary.action, "build-shipyard", "not before its pop-up tells it");
  state.storyTold = new Set(["free_trader"]);
  const plan = founding.foundingPlan(state, home);
  assert.equal(plan.primary.action, "merchant-shipyard");
  assert.equal(plan.primary.copy, "15 Alloys · 13 Machinery · 15 Semiconductors");
  assert.deepEqual([...plan.primary.alternatives.map(a => a.label)], ["Buy from trader", "Build a Shipyard"]);
  assert.deepEqual([...plan.primary.alternatives.map(a => a.action)], ["merchant-shipyard", "build-shipyard"], "the Hub route stays beside it");
  state.merchant.phase = "arriving";
  assert.equal(founding.foundingPlan(state, home).primary.action, "build-shipyard", "not until it docks");
  Object.assign(state, saved, { storyTold: new Set() });
}
// §home-ore: the first mine goes to the command's own planet while it has ore
// and a free resource site, even when another body's ore is richer.
{
  const garden = { id: 2, structures: { command_center: 1, bioharvester: 1 }, resource_slots: 2,
    deposits: [{ resource: "biomass", richness: 1 }, { resource: "metallic_ore", richness: 0.85 }] };
  const rock = { id: 3, structures: {}, resource_slots: 1, deposits: [{ resource: "metallic_ore", richness: 0.95 }] };
  const system = { id: "home", builds: [], bodies: [rock, garden] };
  assert.equal(founding.bestMineBody(system).id, 2, "the habitable world first");
  garden.resource_slots = 1;
  assert.equal(founding.bestMineBody(system).id, 3, "a full home planet falls back to the richest ore");
}
console.log("Progression fixtures: contracts, physical pirate prizes, banked dossiers, readiness, colony advice, build progress (live bars) and post-victory navigation pass.");

if (process.argv.includes("--serve")) {
  const page = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
    <link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css">
    <style>html,body{height:auto;overflow:auto}body{margin:0;background:#070d18;font-family:system-ui}.deck{position:relative;inset:auto;pointer-events:auto;display:flex;flex-wrap:wrap;gap:20px;padding:20px}
    .fixture{width:435px;max-width:100%;background:var(--panel-bg);padding:12px;box-sizing:border-box}
    .icon{width:24px;height:24px;object-fit:contain;vertical-align:middle}h2{font-size:18px}
    </style><main class="deck"><div class="fixture"><h2>After the first victory</h2>${handoffPage}</div><div class="fixture"><h2>Optional opportunities</h2>${offers}</div>
    <div class="fixture"><h2>Dispatch setup</h2>${ui.operationCard(escort)}${ui.operationCard(salvage)}</div>
    <div class="fixture"><h2>Fleet and colony</h2>${fleetUi.readinessHtml(guard)}${fleetUi.confirmHtml(freighter, "haul-hub")}${empire.colonyPurposeHtml(prospect)}</div></main>`;
  const publicRoot = fileURLToPath(new URL("../public", import.meta.url));
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://fixture");
    if (url.pathname === "/") { res.setHeader("Content-Type", "text/html; charset=utf-8"); res.end(page); return; }
    let path;
    if (["/tokens.css", "/deck.css"].includes(url.pathname)) {
      path = fileURLToPath(new URL(`../src/styles${url.pathname}`, import.meta.url));
      res.setHeader("Content-Type", "text/css");
    } else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(publicRoot + sep)) { res.writeHead(403).end(); return; }
    } else { res.writeHead(404).end(); return; }
    try { res.end(readFileSync(path)); } catch { res.writeHead(404).end(); }
  });
  server.listen(0, "127.0.0.1", () => console.log(`Progression fixture: http://127.0.0.1:${server.address().port}/`));
}

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
deps["../../core/derive/construction"] = compile("core/derive/construction.ts");
const icons = deps["../../icons"] = compile("icons.ts");
deps["./format"] = { fmtDur: n => `${Math.round(n)}s` };
deps["./geo"] = { HYPERLIMIT_SU: 900, operationSystemName: () => "Freya" };
const fleets = deps["./fleet"] = deps["../../core/derive/fleet"] = compile("core/derive/fleet.ts");
const readiness = deps["../../core/derive/readiness"] = compile("core/derive/readiness.ts");
const colony = deps["../../core/derive/colony"] = compile("core/derive/colony.ts");
deps["../../prng"] = { hashId: () => 0 };
deps["../../core/derive/format"] = compile("core/derive/format.ts");
deps["../../core/derive/captains"] = { captainTitle: () => "Lieutenant" };
deps["../signature"] = { sheetFingerprint: x => JSON.stringify(x) };
deps["../dom"] = { renderDeferred: () => false, setHtml(root, html) { root.innerHTML = html; } };
deps["../battlewithdraw"] = compile("shell/battlewithdraw.ts");

const ghost = (id, kind, extra = {}) => ({ id, kind, own: true, owner: "1",
  pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 }, age: 8, damage: .04,
  composition: [{ kind, count: 1 }], fuel: 150, supplied: true, cargo: null,
  ...extra });
const freighter = ghost("102", "convoy", { fuel: 12, cargo_manifest: [
  { commodity: "alloys", units: 40 }, { commodity: "fuel", units: 60 }] });
const guard = ghost("101", "raider", { damage: .74, fuel: 2, guard_target: "102" });
state.ghosts = [guard, freighter];
const r = readiness.fleetReadiness(freighter);
assert.equal(r.cargoFree, 150, "mixed manifests share one hold");
assert.equal(r.fuel, 12);
assert.equal(readiness.fleetReadiness(ghost("3", "raider", { damage: undefined })).hull, null);
assert.match(readiness.dispatchWarnings(guard, state.galaxy.hub).join(" "), /26% hull.*Fuel short/);
assert.equal(readiness.dispatchWarnings(ghost("3", "raider", { fuel: 0 }), state.galaxy.hub, true).length, 0,
  "unlimited playtest jumps are not priced as warp cruises");
assert.match(readiness.dispatchWarnings(ghost("3", "convoy", { fuel: undefined }), state.galaxy.hub).join(""), /unavailable/);
assert.equal(readiness.dispatchWarnings({ ...guard, own: false }, state.galaxy.hub).length, 0);
const multi = readiness.intentReadinessWarnings({ ...state, emplacements: [] },
  { verb: "move", shipId: "101", shipIds: ["101", "102"], dest: state.galaxy.hub });
assert.match(multi.join(" "), /Freighter 102: Fuel short/, "all dispatched fleets, not only primary selection");
const refueled = { ...freighter, fuel: 900 };
assert.equal(readiness.dispatchWarnings(refueled, state.galaxy.hub).length, 0, "new telemetry clears the warning");

const home = { id: "home", owner: "1", bodies: [], food_state: "well_supplied",
  workforce: { units: 6, posted: 6 }, builds: [],
  stockpile: [{ commodity: "metallic_ore", units: 0 }, { commodity: "fuel", units: 3 }],
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
assert.deepEqual([...colony.colonyPurpose(electronics, home).imports], ["provisions", "silicates"]);
const depleted = { ...prospect, bodies: [{ id: 0, deposits: [{ resource: "metallic_ore", reserves: 0 }] }], opportunities: [] };
assert.equal(colony.colonyPurpose(depleted, home).exports.length, 0);
const mixed = { ...prospect, bodies: [...prospect.bodies, { id: 1, deposits: [{ resource: "biomass", reserves: 100 }] }] };
assert.equal(colony.colonyPurpose(mixed, home, mixed.bodies[0]).imports.length, 0,
  "another world in the system can feed a barren mining moon without interstellar imports");

deps["./geo"] = deps["../../core/derive/geo"] = compile("core/derive/geo.ts");
deps["./market"] = deps["../../core/derive/market"] = compile("core/derive/market.ts");
deps["./colony"] = colony;
const handoff = deps["../../core/derive/handoff"] = compile("core/derive/handoff.ts");
const handoffUi = deps["./handoff"] = compile("shell/deck/handoff.ts");
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
assert.match(ui.operationCard(escort), /Fuel short/);
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

const { DeckFleetRoutes } = compile("shell/deck/fleet.ts");
const fleetUi = new DeckFleetRoutes(root, ctx, { go() {}, notice() {} });
assert.match(fleetUi.readinessHtml(guard), /Guarding Freighter/);
assert.match(fleetUi.readinessHtml(guard), /26%/);
assert.match(readiness.intentReadinessWarnings(state, { verb: "command", shipId: "102",
  commands: [{ type: "HaulToMarketHub", fleet_id: "102", sell_on_arrival: false }] }).join(" "), /Fuel short/,
  "the shared confirmation retains the haul fuel warning");
const { DeckEmpireRoutes } = compile("shell/deck/empire.ts");
const empire = Object.create(DeckEmpireRoutes.prototype); empire.ctx = ctx;
assert.match(empire.colonyPurposeHtml(prospect), /Excellent mining/);
assert.equal(empire.colonyPurposeHtml(home), "", "home is not pitched as the next colony");
assert.match(icons.commodityIcon("provisions"), /provisions.png/);

// Exercise the real queue renderer: recipe time is not the job's boosted duration.
state.galaxy.build_options = [{ key: "convoy", build_secs: 12, costs: [] }];
const queue = (job, now) => {
  renderNow = now;
  return empire.queueHtml({ ...home, builds: [{ key: "convoy", body_id: 0, ...job }] });
};
for (const duration of [12, 9.6, 230 / 30, 18]) {
  const job = { start_time: 60, complete_time: 60 + duration };
  assert.match(queue(job, 60), /aria-valuenow="0".*width:0\.0%/,
    `a fresh ${duration}s job starts at zero, regardless of its recipe`);
  assert.match(queue(job, 60 + duration / 2), /aria-valuenow="50".*width:50\.0%/);
  assert.match(queue(job, 60 + duration), /aria-valuenow="100".*width:100\.0%/);
  assert.match(queue(job, 59), /width:0\.0%/, "clock slew cannot make progress negative");
  assert.match(queue(job, 80), /width:100\.0%.*completing/,
    "clock expiry does not remove a job before its completion report arrives");
}
state.galaxy.build_options[0].build_secs = 999;
assert.match(queue({ start_time: 60, complete_time: 69.6 }, 62.4), /width:25\.0%/,
  "a late first report uses the job's start, not the report's arrival time");
assert.match(queue({ start_time: 60, complete_time: 69.6 }, 64.8), /width:50\.0%/,
  "new recipes or bonuses never retime a reported job");
for (const start_time of [undefined, null, NaN, Infinity, 70, 71]) {
  const html = queue({ start_time, complete_time: 70 }, 60);
  assert.doesNotMatch(html, /role="progressbar"|NaN|Infinity/,
    "legacy or invalid timing retains the ETA, not a fabricated percentage");
  assert.match(html, /10s/);
}
const pausedJob = { start_time: 60, complete_time: null,
  work: { fraction: .375, per_second: 0, at_time: 63.6 } };
for (const now of [63.6, 70, 600]) {
  assert.match(queue(pausedJob, now), /width:37\.5%.*Paused · needs workforce/,
    "paused progress never creeps or vanishes when its former ETA expires");
  assert.doesNotMatch(queue(pausedJob, now), /completing|NaN|Infinity/);
}
const resumedJob = { ...pausedJob, complete_time: 76,
  work: { fraction: .375, per_second: 1 / 9.6, at_time: 70 } };
assert.match(queue(resumedJob, 70), /width:37\.5%/);
assert.match(queue(resumedJob, 71.2), /width:50\.0%/,
  "resuming continues earned work instead of restarting or using original elapsed time");
assert.match(queue(resumedJob, 100), /width:100\.0%.*completing/,
  "an estimated finish still waits for a completion report");
assert.match(queue({ ...resumedJob, work: { fraction: .5, per_second: 1 / 12, at_time: 71.2 } }, 74.2),
  /width:75\.0%/, "only remaining work follows the changed rate");
assert.doesNotMatch(queue({ complete_time: null }, 100), /role="progressbar"|completing/);
const waitingStructure = { key: "mining_complex", body_id: 0, queued: true,
  start_time: null, complete_time: null, work: { fraction: 0, per_second: 0, at_time: 60 } };
for (const now of [60, 69.6, 6000]) {
  const html = queue(waitingStructure, now);
  assert.match(html, /aria-valuenow="0".*width:0\.0%.*Queued/);
  assert.doesNotMatch(html, /Paused|Building ·|completing|NaN|Infinity/,
    "queued is report state: neither clock expiry nor another job finishing can start it locally");
}
const startedStructure = { ...waitingStructure, queued: false, start_time: 6000,
  complete_time: 6020, work: { fraction: 0, per_second: .05, at_time: 6000 } };
assert.match(queue(startedStructure, 6000), /width:0\.0%.*Building · 20s/);
assert.match(queue(startedStructure, 6010), /width:50\.0%.*Building · 10s/);
const queueWorld = { ...home,
  bodies: [{ id: 0, name: "Freya I", structures: {} }, { id: 1, name: "Freya II", structures: {} }],
  builds: [waitingStructure,
    { key: "habitat", body_id: 1, start_time: 60, complete_time: 80 },
    { key: "shipyard", body_id: 0, start_time: 60, complete_time: 80 },
    { ...waitingStructure, key: "orbital_warehouse" }] };
const receivedOrder = JSON.stringify(queueWorld.builds);
const planetGroups = deps["../../core/derive/construction"].buildsByPlanet(queueWorld.builds);
assert.equal(planetGroups.map(({ jobs }) => jobs.map(j => j.key).join(",")).join("|"),
  "shipyard,mining_complex,orbital_warehouse|habitat", "group per planet, active then FIFO waiting");
assert.equal(JSON.stringify(queueWorld.builds), receivedOrder, "rendering never mutates the served queue");
renderNow = 60;
const planetsHtml = empire.queueHtml(queueWorld);
assert.match(planetsHtml, /<h4>Freya I<\/h4>.*Shipyard.*Mining Complex.*Orbital Warehouse.*<h4>Freya II<\/h4>.*Habitat/);
const yardWorld = { ...home, structures: { shipyard: 2 }, builds: [],
  bodies: [{ id: 0, name: "First", structures: { shipyard: 1 } },
    { id: 1, name: "Second", structures: { shipyard: 2 } }],
  assignments: [{ body_id: 0, structure: "shipyard", staffing: 1, skill: 1 }] };
const hullOption = { key: "convoy", label: "Freighter", build_secs: 12, costs: [] };
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
renderNow = 60;

// Post-victory advice is assembled from the same received picture as the panels.
// Exercise the real derivation + both routed consumers, not an imitation checklist.
deps["../../core/derive/orders"] = { nextDecisionLabel: () => "No urgent decisions" };
state.name = "Frontier Test"; state.timeline = []; state.battles = [];
state.galaxy.systems = [
  { id: "home", name: "Freya", pos: { x: 0, y: 0 } },
  { id: "prospect", name: "Ore Haven", pos: { x: 80_000, y: 0 } },
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
state.founding = { stage: "complete_export", bounty_received: false, expansion_unlocked: false,
  protected: true, protection_min_until: 100, protection_max_until: 1000, survey_candidates: ["prospect", "garden"] };
assert.equal(handoff.postVictoryHandoff().length, 0, "a true/unreported victory must not expose the handoff");
assert.equal(handoffUi.handoffHtml(), "");
state.founding.bounty_received = true;
let goals = handoff.postVictoryHandoff();
const goal = id => handoff.postVictoryHandoff().find(g => g.id === id);
assert.deepEqual([...goals.map(g => g.id)], ["explore", "upgrade", "colony"]);
assert.equal(goal("explore").action.select, "academy");
assert.equal(goal("upgrade").action.select, "shipyard");
assert.equal(goal("upgrade").action.mode, "structures");
assert.equal(goal("colony").done, false);
assert.doesNotMatch(goal("colony").summary, /Excellent mining|Ore Haven/, "unarrived surveys cannot advertise a hidden jackpot");
assert.equal(goal("upgrade").costs[0].stock, 2);
home.stockpile.at(-1).units = 45;
assert.equal(goal("upgrade").costs[0].stock, 45, "stock updates with the received inventory");
freighter.docked = "Ehome";
assert.equal(goal("upgrade").costs[0].stock, 85, "construction includes served docked Freighter goods");
freighter.docked = null;
home.bodies[0].structures.academy = 1;
assert.equal(goal("explore").action.kind, "world", "an unstaffed Academy opens workforce controls");
home.assignments.push({ body_id: 4, structure: "academy", workers: 1, specialists: {} });
assert.equal(goal("explore").action.kind, "research");
state.founding.stage = "build_scout";
assert.equal(goal("explore").action.select, "scout");
home.builds.push({ key: "scout", body_id: 4, complete_time: 1 });
state.simTime = 100;
assert.equal(goal("explore").actionLabel, "View Scout build", "an expired build estimate is not an arrived hull");
const scout = ghost("103", "scout");
state.ghosts.push({ ...scout, own: false });
assert.equal(goal("explore").action.kind, "build", "rival hulls do not satisfy own readiness");
state.ghosts.pop(); state.ghosts.push(scout); home.builds = [];
assert.equal(goal("explore").action.id, "103");
prospect.bodies[0].geology = "ultra_rich";
assert.equal(goal("explore").status, "1/2 reports");
assert.match(goal("colony").summary, /Ore Haven: Excellent mining/);
assert.equal(goal("explore").prospect, "garden");
garden.bodies[0].geology = "average";
assert.equal(goal("explore").done, false, "received reports do not invent the server's grant/unlock event");
state.founding.expansion_unlocked = true; state.founding.stage = "build_colony";
assert.equal(goal("explore").done, true);
assert.equal(goal("explore").action.kind, "warehouse");
assert.equal(goal("colony").action.select, "colony");
assert.match(goal("colony").payoff, /Smelter.*provisions/);
home.structures.shipyard = 2; home.bodies[0].structures.shipyard = 2;
assert.equal(goal("upgrade").action.select, "corvette");
state.ghosts.push(ghost("104", "corvette"));
assert.equal(goal("upgrade").done, true);
state.ghosts.push(ghost("105", "colony"));
assert.equal(goal("colony").action.id, "105");
prospect.owner = "1";
assert.equal(goal("colony").done, false, "only the arrived founding milestone graduates the colony goal");
state.founding.stage = "complete";
assert.equal(goal("colony").done, true);
assert.equal(goal("colony").action.id, "prospect");
state.founding.stage = "build_colony"; prospect.owner = null;
state.ghosts = [guard, freighter, scout];
production.expires_at = state.simTime - 1;
assert.equal(goal("colony").funding, null, "expired offers are not offered as attainable funding");
production.expires_at = 900;
assert.equal(goal("colony").funding.id, "13");

const routes = [], worlds = [], focused = [];
const hooks = { go: r => routes.push(r), selectFleet: id => focused.push(id), openWorld: (...args) => worlds.push(args) };
const navigate = (act, id) => handoffUi.handleHandoffAction({ dataset: { deckAct: act, goal: id } }, ctx, hooks);
const sentBeforeNavigation = sent.length;
navigate("handoff-open", "explore");
assert.equal(routes.at(-1).name, "market"); assert.equal(routes.at(-1).query.tab, "warehouse");
navigate("handoff-open", "upgrade");
assert.equal(routes.at(-1).name, "build"); assert.equal(routes.at(-1).query.select, "corvette");
assert.equal(routes.at(-1).query.body, "4");
navigate("handoff-prospect", "colony");
assert.equal(routes.at(-1).params.id, "prospect");
navigate("handoff-contract", "colony");
assert.equal(routes.at(-1).query.contract, "13");
assert.equal(sent.length, sentBeforeNavigation, "handoff navigation never sends an order, accepts a job or pays a reward");
const withHandoff = new DeckStrategicRoutes(root, ctx, { ...hooks, openWorld: hooks.openWorld, selectFleets() {}, notice() {} });
const chosenContract = withHandoff.operationsHtml("13");
assert.ok(chosenContract.indexOf("Funding contract") < chosenContract.indexOf("Your next chapter"), "a funding link surfaces the chosen offer first");
assert.match(chosenContract, /data-deck-act="strategic-operation-accept"/);
assert.match(chosenContract, /Hull costs · local stock/);
assert.match(chosenContract, /900 cr/);
const { DeckCommandRoutes } = compile("shell/deck/command.ts");
const guide = { id: "guide", hidden: true, classList: { toggle() {} }, innerHTML: "" };
const command = new DeckCommandRoutes(root, guide, ctx, { ...hooks, focusFleet: hooks.selectFleet, inbox: () => [], notice() {} });
assert.match(command.commandHtml(), /Requirements &amp; contract rewards/);
command.renderFounding(true);
assert.match(guide.innerHTML, /Next objectives &amp; rewards/);
state.founding.stage = "complete"; state.founding.protected = false;
command.renderFounding(true);
assert.equal(guide.hidden, false, "unfinished optional goals survive tutorial completion");
state.ghosts.push(ghost("104", "corvette"));
command.renderFounding(true);
assert.equal(guide.hidden, true, "a finished handoff does not leave a permanent floating checklist");
state.founding.stage = "build_colony"; state.ghosts = [guard, freighter, scout];
const handoffPage = handoffUi.handoffHtml();
const { MobileParitySurfaces } = compile("shell/mobile/parity.ts");
const sheetsOpened = []; let refreshes = 0;
const mobile = new MobileParitySurfaces(ctx, { refresh() { refreshes++; } }, {
  openSheet: s => sheetsOpened.push(s), focusFleet: id => focused.push(id), focusSystem: id => located.push(id),
});
const mobileQueue = mobile.systemConstruction("home", queueWorld);
assert.match(mobileQueue, /<h4>Freya I<\/h4>.*Shipyard.*Mining Complex.*Queued.*Orbital Warehouse.*<h4>Freya II<\/h4>/);
assert.doesNotMatch(mobileQueue, /Paused · needs workforce/,
  "mobile also distinguishes a waiting structure from an unstaffed hull");
assert.match(mobile.handoffHtml(), /Your next chapter/);
assert.match(mobile.handoffHtml(), /data-kind="market"/, "the kit uses the existing Warehouse opener on mobile");
mobile.openHandoffGoal("next-goal", "upgrade");
assert.equal(sheetsOpened.at(-1).id, "shipyard"); assert.equal(mobile.selectedHull, "corvette");
assert.equal(sheetsOpened.at(-1).props.bodyId, 4);
mobile.openHandoffGoal("next-funding", "colony");
assert.equal(mobile.handoffContract, "13"); assert.equal(refreshes, 1);
assert.equal(sent.length, sentBeforeNavigation, "mobile handoff is navigation-only too");
console.log("Progression fixtures: contracts, readiness, colony advice, actual build progress, post-victory gates/requirements/stock/rewards/navigation pass.");

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

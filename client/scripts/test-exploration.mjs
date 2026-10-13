import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import sharp from "sharp";

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (name, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => deps[id] ?? {}, structuredClone });
  return exports;
};
const plain = value => JSON.parse(JSON.stringify(value));
const protocol = load("protocol.ts");
const equipment = load("core/derive/equipment.ts", { "../../protocol": protocol });
const contact = { id: "810", pos: { x: 40_000, y: 20_000 }, reported_at: 30, details: null };
const details = { kind: "derelict", name: "Drifting <wreck>", cargo: { alloys: 24 },
  modules: { whipple_armor: 1 }, programme: "prop_bunkerage", research_fraction: .15, restored_by: null };
const ghost = (id, kind, own = true) => ({ id, kind, own, pos: { x: 0, y: 0 },
  vel: { x: 0, y: 0 }, composition: [{ kind, count: 1 }], cargo_manifest: [] });
const st = { playerId: "1", simTime: 100, commandCenter: { x: 0, y: 0 },
  galaxy: { c: 400, systems: [], hyperlimit: 900 }, systems: [], operations: [],
  explorationSites: [contact], selectedExplorationSiteId: null,
  selectedShipIds: new Set(), pendingIntent: null, pendingOrders: new Map(),
  ghosts: [ghost("scout", "scout"), ghost("freighter", "convoy"), ghost("enemy", "scout", false)] };
const clock = { state: st, liveSimTime: () => st.simTime };
const fleets = load("core/derive/fleet.ts", { "../../state": clock, "../../protocol": protocol,
  "./geo": load("core/derive/geo.ts"), "./equipment": equipment });
const exploration = load("core/derive/exploration.ts", { "./fleet": fleets, "../../protocol": protocol });
const icons = load("icons.ts");
const commands = load("core/fleetorders.ts", { "./derive/fleet": fleets,
  "./derive/exploration": exploration, "../icons": icons,
  "./derive/format": { fmtDur: n => `${Math.max(0, Math.round(n))}s` } });
const renderer = { stateVersion: 0, centerOnWorld: p => { renderer.center = p; } };
const orders = load("core/derive/orders.ts", { "../../state": clock, "../fleetorders": commands,
  "./fleet": fleets, "../../icons": icons });
const intent = load("core/intent.ts", { "../state": clock, "../render": { renderer },
  "./fleetorders": commands, "./derive/fleet": fleets, "./derive/orders": orders });
const surface = load("shell/exploration.ts", { "../icons": icons,
  "../core/derive/fleet": fleets, "../core/derive/exploration": exploration,
  "../core/derive/equipment": equipment });
const sent = [];
intent.bindIntentCore(() => ({ connected: true, send: msg => sent.push(plain(msg)) }), () => {});
const ctx = { state: st, renderer, intent };
const action = (act, fleet = "scout") => surface.handleExplorationAction({ dataset: {
  exploreAct: act, site: contact.id, fleet,
} }, ctx);

assert.equal(exploration.siteArt(contact), null);
assert.match(surface.explorationHtml(st), /Unknown contact 810/);
assert.doesNotMatch(surface.explorationHtml(st), /art\/exploration|alloys|whipple|Drifting/);
action("open");
assert.equal(st.selectedExplorationSiteId, contact.id);
let html = surface.explorationHtml(st);
assert.match(html, /Travel here/);
assert.match(html, /investigate automatically on arrival/);
assert.doesNotMatch(html, /data-fleet="enemy"|data-fleet="freighter"/);
assert.equal(exploration.explorationOrder(st, contact, st.ghosts[1], "investigate"), null);
assert.equal(exploration.explorationOrder(st, contact, st.ghosts[0], "recover"), null);
assert.equal(exploration.explorationOrder(st, { ...contact, id: "unseen" }, st.ghosts[0], "investigate"), null);
action("investigate");
assert.equal(sent.length, 0, "Dispatch opens Confirm/Cancel; it does not send an order");
assert.deepEqual(plain(st.pendingIntent.dest), contact.pos);
const readiness = load("core/derive/readiness.ts", { "./fleet": fleets, "../../protocol": protocol });
st.ghosts[0].fuel = 0;
assert.match(readiness.intentReadinessWarnings(st, st.pendingIntent).join(" "), /Deuterium short/,
  "expeditions inherit the normal inline dispatch fuel warning");
// §founding R5: one preview grammar: the destination by name, then distance, ETA and fuel.
assert.match(commands.fleetCommandSummary(st.pendingIntent, st), /Move → Unknown contact 810 · [\d,]+ su · ETA ~\d+s · Deuterium ~\d+/);
intent.clearPendingIntent();
assert.equal(sent.length, 0);
action("investigate"); intent.confirmPendingIntent(); intent.confirmPendingIntent();
assert.deepEqual(sent, [{ type: "MoveShip", ship_id: "scout", dest: contact.pos }]);
assert.equal(contact.details, null, "Confirm must not reveal the server's discovery early");
st.ghosts[0].expedition = { site: contact.id, task: "investigate" };
st.ghosts[0].survey_progress = .4;
assert.match(surface.explorationHtml(st), /Investigating · 40%/);
st.simTime += 10_000;
assert.doesNotMatch(surface.explorationHtml(st), /art\/exploration|Drifting/,
  "elapsed time alone cannot unlock contents or the artwork");
assert.match(surface.explorationHtml(st), /Investigating · 40%/, "work progress is also a received fact");

contact.details = details; // Only the next received report changes this picture.
html = surface.explorationHtml(st);
assert.match(html, /art\/exploration\/derelict.png/);
assert.match(html, /Drifting &lt;wreck&gt;/);
assert.match(html, /Travel here/);
assert.match(html, /First copy: 15% research work/);
assert.doesNotMatch(html, /data-fleet="scout"|data-fleet="enemy"/);
action("recover", "freighter");
assert.equal(sent.length, 1);
intent.confirmPendingIntent();
assert.deepEqual(sent[1], { type: "MoveShip", ship_id: "freighter", dest: contact.pos });
assert.equal(contact.details.cargo.alloys, 24, "no optimistic depletion or teleporting cargo");
assert.equal(st.ghosts[1].cargo_manifest.length, 0);
assert.equal(commands.fleetCommandsValid([{ type: "ExploreSite", fleet_id: "scout", site_id: "810", task: "recover" }], st), false);

contact.details = { ...details, kind: "station", cargo: {}, modules: {} };
assert.match(surface.explorationHtml(st), /data-explore-act="restore"[^>]*disabled/);
st.ghosts[1].cargo_manifest = [{ commodity: "machinery", units: 12 }, { commodity: "electronics", units: 8 }];
assert.doesNotMatch(surface.explorationHtml(st), /data-explore-act="restore"[^>]*disabled/);
action("restore", "freighter");
assert.equal(sent.length, 2);
intent.confirmPendingIntent();
assert.equal(sent[2].task, "restore");
assert.equal(contact.details.restored_by, null);
contact.details.restored_by = "1";
assert.match(surface.explorationHtml(st), /Your sensor outpost/);
assert.doesNotMatch(surface.explorationHtml(st), /data-explore-act="restore"/);
action("center"); assert.equal(renderer.center, contact.pos);
action("back"); assert.equal(st.selectedExplorationSiteId, null);

// The normal galaxy click dispatches travel; Inspect/no fleet still opens the
// optional catalogue. No operation acceptance and no optimistic work is needed.
Object.assign(st, { battles: [], battleRecords: [], jumpDepartures: [], emplacements: [],
  selectedShipId: "scout", anchors: [] });
Object.assign(renderer, { worldToScreen: p => p, fleetScreenPosition: g => g.pos,
  fleetHitRadius: () => 24, battlePick: () => null, operationPick: () => null });
const beltsDerive = load("core/derive/belts.ts", { "../../protocol": protocol, "../../icons": icons, "./fleet": fleets });
const map = load("core/mapclick.ts", { "../protocol": protocol, "../state": clock,
  "./derive/fleet": fleets, "./derive/exploration": exploration, "./derive/belts": beltsDerive,
  "./derive/format": { operationTitle: () => "Privateer patrol" } });
const mapCtx = { ...ctx, jumpAiming: null, guardAiming: null, emplaceArmed: null };
const click = inspect => map.resolveMapClick(contact.pos.x, contact.pos.y,
  { shift: false, long: false, inspect }, mapCtx);
contact.details = null;
assert.deepEqual(plain(click(false).intent), { shipId: "scout", verb: "move",
  targetId: contact.id, dest: contact.pos });
assert.equal(sent.length, 3, "map click only stages an intent");
assert.equal(click(true).target.type, "exploration");
st.selectedShipId = null;
assert.equal(click(false).target.type, "exploration");
st.selectedShipId = "enemy";
assert.equal(click(false).target.type, "exploration");
st.selectedShipId = "freighter";
contact.details = details;
assert.equal(click(false).intent.verb, "move");
assert.equal(click(false).intent.shipId, "freighter");
// A contract tag under the cursor opens its contract, even with a fleet selected.
st.operations = [{ id: "O1", state: "offered" }];
renderer.operationPick = () => "O1";
const contractClick = click(false);
assert.deepEqual([contractClick.kind, contractClick.target.type, contractClick.target.id], ["select", "operation", "O1"], "a contract tag is never a move target");
assert.match(contractClick.target.readout, /click again for <b>Drifting &lt;wreck&gt;<\/b>/, "the contact beneath stays one click away");
renderer.operationPick = () => null;
st.operations = [];

// §belts: a barge selected + a click on a charted system with an ore belt
// stages a MineBelt order for its most valuable ore; it never sends it.
const beltSys = { id: "sif", name: "Sif", pos: { x: 50_000, y: 0 } };
st.galaxy.systems.push(beltSys);
st.systems.push({ id: "sif", owner: null, bodies: [{ id: 2, name: "Sif Belt", kind: "asteroid_belt", deposits: [
  { resource: "metallic_ore", richness: 0.2 }, { resource: "cuprite_ore", richness: 0.15 }] }] });
st.ghosts.push({ id: "barge", kind: "mining_barge", own: true, owner: "1", pos: { x: 40_000, y: 0 },
  vel: { x: 0, y: 0 }, composition: [{ kind: "mining_barge", count: 1 }] });
renderer.systemHitRadius = () => 15;
st.selectedShipId = "barge";
const clickBelt = () => map.resolveMapClick(beltSys.pos.x, beltSys.pos.y, { shift: false, long: false, inspect: false }, mapCtx);
assert.equal(clickBelt().intent.verb, "command");
assert.deepEqual(plain(clickBelt().intent.commands), [{ type: "MineBelt", fleet_id: "barge", system_id: "sif", body_id: 2, ore: "cuprite_ore" }]);
assert.equal(sent.length, 3, "a mining click only stages the order");
// §belts: a lone station selected + a click on that charted belt system
// stages an AnchorStation order at its richest belt.
const depot = { id: "depot", kind: "mining_belt_station", own: true, owner: "1", pos: { x: 40_000, y: 0 },
  vel: { x: 0, y: 0 }, composition: [{ kind: "mining_belt_station", count: 1 }] };
st.ghosts.push(depot);
st.selectedShipId = "depot";
assert.deepEqual(plain(clickBelt().intent.commands), [{ type: "AnchorStation", fleet_id: "depot", system_id: "sif", body_id: 2 }]);
depot.anchor = { system: "sif", body: 2, phase: "anchored", since: 0 };
assert.notEqual(clickBelt().intent?.commands?.[0]?.type, "AnchorStation", "an anchored station stays put");
st.ghosts.pop();
st.selectedShipId = "barge";
// §pockets: a found pocket worth more than the belt's own ore is the target.
st.systems.at(-1).bodies[0].pockets = [{ resource: "titanium_ore", richness: 0.6, reserves: 900 }];
assert.deepEqual(plain(clickBelt().intent.commands),
  [{ type: "MineBelt", fleet_id: "barge", system_id: "sif", body_id: 2, ore: "titanium_ore", pocket: true }]);
st.systems.at(-1).bodies[0].pockets[0].reserves = 0;
assert.equal(clickBelt().intent.commands[0].ore, "cuprite_ore", "a worked-out pocket is passed over");
delete st.systems.at(-1).bodies[0].pockets;
st.systems.at(-1).owner = "2";
assert.notEqual(clickBelt().intent?.verb, "command", "a rival's belt is not offered");
st.systems.at(-1).owner = null;
st.systems.at(-1).bodies[0].deposits = null;
assert.notEqual(clickBelt().intent?.verb, "command", "an uncharted belt is a move, not a mining order");

// §belts: the charter's barge lesson reads only served state: Shipyard, then
// the barge, then a one-click order to the belt the surveys charted.
st.systems.find(sys => sys.id === "sif").bodies[0].deposits = [
  { resource: "metallic_ore", richness: 0.2 }, { resource: "cuprite_ore", richness: 0.15 }];
const guide = load("core/derive/founding.ts", { "../../protocol": protocol, "./fleet": fleets,
  "./format": { fmtDur: n => `${Math.round(n)}s` }, "../../icons": icons });
const yardHome = { id: "home", owner: "1", bodies: [{ id: 0, name: "Home", structures: { shipyard: 1 } }],
  assignments: [{ body_id: 0, structure: "shipyard", workers: 1, specialists: {} }], builds: [],
  storage_used: 10, storage_cap: 500 };
st.systems.push(yardHome);
st.galaxy.systems.push({ id: "home", name: "Home", pos: { x: 0, y: 0 } });
// Industry · chart a nearby system: build a probe, then one click sends it.
// The fixture Scout is dwelling on an expedition (survey_progress .4), which
// is not a survey of the stop: it neither counts as charting nor gets pulled.
st.founding = { stage: "survey_candidates", local_opening: true, survey_candidates: ["sif"], belt_target: null,
  stipends: [], expansion_unlocked: false };
const barge = st.ghosts.pop();
let plan = guide.foundingPlan(st, yardHome);
assert.equal(plan.chapter.name, "Industry");
assert.deepEqual(plain(plan.checklist.map(item => item.done)), [true, true, false]);
assert.equal(plan.primary.action, "build-probe");
st.ghosts.push({ id: "probe", kind: "survey_probe", own: true, pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 },
  composition: [{ kind: "survey_probe", count: 1 }] });
assert.equal(guide.foundingPlan(st, yardHome).primary.action, "survey-stop");
assert.deepEqual(plain(guide.foundingCommands("survey-stop", st, yardHome)),
  [{ type: "SurveySystem", fleet_id: "probe", system_id: "sif" }]);
Object.assign(st.ghosts.at(-1), { pos: { x: 50_000, y: 60 }, survey_progress: 0.5 });
plan = guide.foundingPlan(st, yardHome);
assert.equal(plan.primary.action, "probe");
assert.match(plan.progress.join(" "), /Surveying Sif · 50%/);
st.ghosts.pop(); st.ghosts.push(barge);

// Reach: the barge, the belt, the haul home.
st.founding = { stage: "build_barge", local_opening: true, survey_candidates: ["sif"], belt_target: { system: "sif", body: 2 },
  stipends: [], expansion_unlocked: true };
plan = guide.foundingPlan(st, yardHome);
assert.equal(plan.chapter.name, "Reach");
assert.equal(plan.primary.action, "build-barge");
assert.match(plan.primary.copy, /Sif Belt/);
assert.deepEqual(plain(plan.checklist.map(item => item.done)), [false, false, false]);
yardHome.storage_used = 500;
assert.match(guide.foundingPlan(st, yardHome).primary.reason, /storage is full/, "a full home is named before the kit is bought");
yardHome.storage_used = 10;
yardHome.builds = [{ key: "mining_barge" }];
assert.match(guide.foundingPlan(st, yardHome).progress.join(" "), /Mining Barge under construction/);
yardHome.builds = [];
st.founding.stage = "mine_belt";
plan = guide.foundingPlan(st, yardHome);
assert.equal(plan.primary.action, "mine-belt");
assert.deepEqual(plain(guide.foundingCommands("mine-belt", st, yardHome)),
  [{ type: "MineBelt", fleet_id: "barge", system_id: "sif", body_id: 2, ore: "cuprite_ore" }]);
st.ghosts.at(-1).industry = { kind: "mine", system: "sif", body: 2, ore: "cuprite_ore", mined: 3, fraction: 0, status: "mining" };
plan = guide.foundingPlan(st, yardHome);
assert.equal(plan.primary.action, "barge");
assert.match(plan.progress.join(" "), /cutting .*Sif Belt in Sif/);
st.founding.stage = "haul_ore";
plan = guide.foundingPlan(st, yardHome);
assert.deepEqual(plain(plan.checklist.map(item => item.done)), [true, true, false]);
assert.equal(plan.primary.action, "barge-home");
assert.deepEqual(plain(guide.foundingCommands("barge-home", st, yardHome)),
  [{ type: "MoveShip", ship_id: "barge", dest: { x: 0, y: 0 } }]);
Object.assign(st.ghosts.at(-1), { industry: null, docked: "home", cargo_manifest: [{ commodity: "cuprite_ore", units: 40 }] });
assert.equal(guide.foundingPlan(st, yardHome).primary.action, "barge-unload");
assert.deepEqual(plain(guide.foundingCommands("barge-unload", st, yardHome)),
  [{ type: "SystemUnload", fleet_id: "barge", system: "home" }]);
st.ghosts.pop();
st.founding.stage = "mine_belt";
assert.equal(guide.foundingPlan(st, yardHome).primary.title, "Replace the Mining Barge", "a lost barge is rebuilt");
delete st.founding;
st.galaxy.systems.pop(); st.galaxy.systems.pop(); st.systems.pop(); st.systems.pop();

// Exercise the actual map layer with minimal Pixi primitives, without a GPU.
class Node {
  children = []; position = { set: (x, y) => { this.x = x; this.y = y; } };
  anchor = { set() {} }; addChild(...nodes) { this.children.push(...nodes); }
  destroy() { this.destroyed = true; }
}
class Graphics extends Node {
  clear() { return this; } poly() { return this; } stroke() { return this; } moveTo() { return this; }
  arc() { return this; } circle() { return this; } fill() { return this; } roundRect() { return this; }
}
const markers = load("core/derive/map-markers.ts", { "./exploration": exploration,
  "./format": { fmt: n => String(n), fmtDur: s => `${s}s`, operationTitle: () => "Contract" } });
const { ExplorationLayer } = load("explorationlayer.ts", { "pixi.js": {
  Container: Node, Sprite: Node, Text: Node, Graphics, Texture: { EMPTY: null },
}, "./core/derive/exploration": exploration, "./core/derive/map-markers": markers,
  "./map-glyph-textures": { loadMapGlyphs: async () => new Map(["contact-unknown", "site-derelict"].map(name => [name, { glyph: name }])) } });
const layer = new ExplorationLayer();
await layer.load();
contact.details = null;
layer.draw([contact], contact.id, p => p);
const marker = layer.markers.get(contact.id);
assert.equal(marker.sprite.texture.glyph, "contact-unknown", "an unknown contact shows a signal, never its hidden kind");
assert.equal(marker.sprite.tint, markers.CONTACT_COLOR);
contact.details = details;
layer.draw([contact], contact.id, p => ({ x: p.x * 2, y: p.y * 2 }), [], [], 24);
assert.equal(marker.sprite.texture.glyph, "site-derelict", "an identified contact shows its kind's glyph");
assert.equal(marker.sprite.width, markers.contactBadgePx(24) * .62, "badges grow with zoom");
assert.equal(marker.sprite.x, contact.pos.x * 2);
assert.equal(layer.pick(contact.pos.x * 2 + 5, contact.pos.y * 2 - 5), contact.id, "the badge is its own pick target");
assert.equal(layer.markers.get(contact.id), marker, "Views reuse the same marker");
layer.draw([], null, p => p);
assert.equal(marker.sprite.destroyed, true);
assert.equal(layer.markers.size, 0);

for (const kind of ["derelict", "station", "asteroids", "anomaly", "precursor"]) {
  const file = new URL(`../public/art/exploration/${kind}.png`, import.meta.url);
  const metadata = await sharp(file.pathname).metadata();
  assert.equal(metadata.hasAlpha, true);
  assert.equal(metadata.width, 256); assert.equal(metadata.height, 256);
}
console.log("Exploration: unknown/report-only glyphs and contents, Scout/Freighter roles, Confirm/Cancel, restoration, contract clicks, pooled sensor badges and five transparent panel illustrations pass.");

// Deep choices never piggyback on a travel click or an expired client timer.
contact.details = { ...details, opportunity: { task: "study", requirement: "research_team", seconds: 60,
  costs: {}, cargo: {}, blueprint: "nebula_spectrometer", has_lead: true }, studied: false, lead: null };
st.selectedExplorationSiteId = contact.id;
st.ghosts[0].expedition = null; st.ghosts[0].survey_progress = null; st.ghosts[0].fuel = 100;
assert.match(surface.explorationHtml(st), /Deep investigation/);
assert.match(surface.explorationHtml(st), /Recon Suite, Nebula Spectrometer or Fieldcraft 2 Captain/);
assert.doesNotMatch(surface.explorationHtml(st), /Follow lead/);
assert.equal(exploration.explorationOrder(st, contact, st.ghosts[0], "study"), null);
st.ghosts[0].loadouts = [{ kind: "scout", n: 1, modules: ["recon_suite"] }];
assert.equal(exploration.deepExpeditionReason(contact, st.ghosts[0]), "");
const count = sent.length;
action("study");
assert.equal(sent.length, count);
assert.match(commands.fleetCommandSummary(st.pendingIntent, st), /Deep investigation/);
intent.confirmPendingIntent();
assert.equal(sent.at(-1).type, "ExploreSite");
assert.equal(sent.at(-1).task, "study");
assert.equal(contact.details.studied, false);
contact.details.guarded = true;
assert.match(exploration.deepExpeditionReason(contact, st.ghosts[0]), /guardians/);
assert.equal(commands.fleetCommandsValid([{ type: "ExploreSite", fleet_id: "scout", site_id: contact.id, task: "study" }], st), false);
contact.details.guarded = false;
st.simTime += 1000;
assert.doesNotMatch(surface.explorationHtml(st), /Follow lead|license received/);
contact.details.studied = true;
const next = { id: "999", pos: { x: 90000, y: 50000 }, reported_at: st.simTime - 20, details: null };
st.explorationSites.push(next);
contact.details.lead = { site: next.id, pos: next.pos, clue: "An archive points onward." };
st.research = { programmes: [], blueprints: ["nebula_spectrometer"] };
assert.match(surface.explorationHtml(st), /Follow lead/);
surface.handleExplorationAction({ dataset: { exploreAct: "open", site: next.id } }, ctx);
assert.match(surface.explorationHtml(st), /Unknown contact 999/);
assert.doesNotMatch(surface.explorationHtml(st), /nebula_spectrometer|Prismatic Lance/);

// Journal writes are local CC annotations, not fleet commands; the server echo
// owns persistence. Unsaved text is read from the live, morph-preserved textarea.
const notes = [];
ctx.send = value => notes.push(plain(value));
st.explorationJournal = [];
surface.handleExplorationAction({ dataset: { exploreAct: "note", site: next.id, kind: "site" },
  closest: () => ({ querySelector: () => ({ value: "<img src=x onerror=alert(1)>" }) }) }, ctx);
assert.equal(notes[0].type, "AnnotateExploration");
assert.equal(notes[0].entry.note, "<img src=x onerror=alert(1)>");
assert.equal(st.explorationJournal.length, 0, "await the server's persisted echo");
assert.match(surface.explorationHtml(st), /&lt;img src=x onerror=alert\(1\)&gt;/);
assert.doesNotMatch(surface.explorationHtml(st), /<img src=x/);
st.explorationJournal = [notes[0].entry];
surface.handleExplorationAction({ dataset: { exploreAct: "pin", site: next.id, kind: "site" }, closest: () => null }, ctx);
assert.equal(notes[1].entry.pinned, true);
st.explorationJournal = [notes[1].entry];
surface.handleExplorationAction({ dataset: { exploreAct: "tab-journal" } }, ctx);
assert.match(surface.explorationHtml(st), /Discovery journal/);
assert.match(surface.explorationHtml(st), /★ Unknown contact 999/);
assert.equal(surface.explorationFocused(st), true);
surface.handleExplorationAction({ dataset: { exploreAct: "tab-blueprints" } }, ctx);
html = surface.explorationHtml(st);
assert.match(html, /Nebula Spectrometer/); assert.match(html, /Licensed/);
assert.match(html, /Survey Drive/); assert.match(html, /Undiscovered/);
assert.equal(sent.length, count + 1, "journal and blueprint browsing never dispatch a ship");
st.playerId = "another-owner"; st.explorationJournal = []; st.explorationSites = [];
assert.doesNotMatch(surface.explorationHtml(st), /onerror|★ Unknown/);
console.log("Discovery expansion: prepared work confirmation, guardian gates, report-only leads/licenses, journal escaping, pins and player isolation pass.");

// Notes use the reliable change-only section; absent keeps them, [] clears.
const reducer = load("core/session.ts", { "../wire.mjs": { PROTOCOL_VERSION: 35 },
  "../state": { syncRenderClock() {} }, "./derive/transactions": { emptyTransactions: () => ({}) },
  "./derive/market": { marketReservations: [], recentMarketOrders: [] }, "../battlehistory": { loadBattleMarks() {} } });
reducer.applyServerMessage({ type: "Sections", exploration_journal: [notes[1].entry] }, st);
assert.equal(st.explorationJournal[0].pinned, true);
reducer.applyServerMessage({ type: "Sections", rankings: [] }, st);
assert.equal(st.explorationJournal.length, 1);
reducer.applyServerMessage({ type: "Sections", exploration_journal: [] }, st);
assert.equal(st.explorationJournal.length, 0);
st.explorationJournal = [notes[1].entry]; st.explorationSites = [contact]; st.battleRecords = [];
reducer.applyServerMessage({ type: "Welcome", player_id: "new-owner", galaxy: { systems: [] } }, st);
assert.equal(st.explorationJournal.length, 0); assert.equal(st.explorationSites.length, 0);

const extraction = { ...contact, details: { ...details, opportunity: { task: "extract", requirement: "fuelled_freighter",
  costs: { deuterium: 6 }, cargo: { volatiles: 300 }, seconds: 45 } } };
const hauler = ghost("cargo", "convoy");
assert.match(exploration.deepExpeditionReason(extraction, hauler), /supplies/);
hauler.cargo_manifest = [{ commodity: "deuterium", units: 6 }, { commodity: "alloys", units: 400 }];
assert.match(exploration.deepExpeditionReason(extraction, hauler), /room/);
hauler.cargo_manifest[1].units = 390;
assert.equal(exploration.deepExpeditionReason(extraction, hauler), "");
assert.equal(exploration.explorationOrder({ explorationSites: [extraction] }, extraction, hauler, "study"), null);
console.log("Discovery journal reliable-section/reconnect handling and extraction previews pass.");

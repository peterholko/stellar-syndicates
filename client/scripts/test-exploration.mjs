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
  "./derive/exploration": exploration, "../icons": icons });
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
assert.match(readiness.intentReadinessWarnings(st, st.pendingIntent).join(" "), /Fuel short/,
  "expeditions inherit the normal inline dispatch fuel warning");
assert.match(commands.fleetCommandSummary(st.pendingIntent, st), /Move → 40000, 20000 su/);
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
  fleetHitRadius: () => 24, battlePick: () => null });
const map = load("core/mapclick.ts", { "../protocol": protocol, "../state": clock,
  "./derive/fleet": fleets, "./derive/exploration": exploration });
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

// Exercise the actual map layer with minimal Pixi primitives, without a GPU.
class Node {
  children = []; position = { set: (x, y) => { this.x = x; this.y = y; } };
  anchor = { set() {} }; addChild(...nodes) { this.children.push(...nodes); }
  destroy() { this.destroyed = true; }
}
class Graphics extends Node {
  clear() { return this; } poly() { return this; } stroke() { return this; }
  circle() { return this; } fill() { return this; } roundRect() { return this; }
}
const { ExplorationLayer } = load("explorationlayer.ts", { "pixi.js": {
  Container: Node, Sprite: Node, Text: Node, Graphics, Texture: { EMPTY: null },
  Assets: { load: async path => ({ path }) },
}, "./core/derive/exploration": exploration });
const layer = new ExplorationLayer();
await layer.load();
contact.details = null;
layer.draw([contact], contact.id, p => p);
const marker = layer.markers.get(contact.id);
assert.equal(marker.sprite.visible, false, "the unknown diamond must not select hidden-kind art");
contact.details = details;
layer.draw([contact], contact.id, p => ({ x: p.x * 2, y: p.y * 2 }));
assert.equal(marker.sprite.texture.path, "/art/exploration/derelict.png");
assert.equal(marker.sprite.width, exploration.EXPLORATION_MARKER_PX);
assert.equal(marker.sprite.x, contact.pos.x * 2);
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
console.log("Exploration: unknown/report-only art and contents, Scout/Freighter roles, Confirm/Cancel, restoration, pooled markers and five transparent runtime assets pass.");

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
  costs: { fuel: 6 }, cargo: { volatiles: 300 }, seconds: 45 } } };
const hauler = ghost("cargo", "convoy");
assert.match(exploration.deepExpeditionReason(extraction, hauler), /supplies/);
hauler.cargo_manifest = [{ commodity: "fuel", units: 6 }, { commodity: "alloys", units: 400 }];
assert.match(exploration.deepExpeditionReason(extraction, hauler), /room/);
hauler.cargo_manifest[1].units = 390;
assert.equal(exploration.deepExpeditionReason(extraction, hauler), "");
assert.equal(exploration.explorationOrder({ explorationSites: [extraction] }, extraction, hauler, "study"), null);
console.log("Discovery journal reliable-section/reconnect handling and extraction previews pass.");

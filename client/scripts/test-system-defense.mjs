import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (name, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => deps[id] ?? {}, structuredClone });
  return exports;
};
const plain = x => JSON.parse(JSON.stringify(x));
const protocol = load("protocol.ts");
const system = { id: "8", name: "Freya", pos: { x: 0, y: 0 } };
const ghost = (id, extra = {}) => ({ id, own: true, owner: "1", kind: "raider", count_class: "one",
  pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 }, age: 10, damage: .2, fuel: 50,
  composition: [{ kind: "raider", count: 1 }], ...extra });
const report = { id: "8", owner: "1", defense_tier: 2, structures: {}, bodies: [], assignments: [], stockpile: [] };
const post = { system: "8", station: system.pos, radius: 10_000 };
const st = { playerId: "1", simTime: 100, commandCenter: system.pos,
  galaxy: { c: 400, systems: [system], hyperlimit: 900 }, systems: [report],
  pendingOrders: new Map(), battles: [], timeline: [], orders: {}, raids: {}, emplacements: [],
  selectedShipIds: new Set(), pendingIntent: null,
  ghosts: [ghost("assigned", { defend_system: post, pos: { x: 5000, y: 0 } }),
    ghost("local", { docked: "E8" }), ghost("candidate", { pos: { x: 30_000, y: 0 } }),
    ghost("rival", { own: false, owner: "2" }), ghost("ally", { own: false, ally: true }),
    ghost("authority", { own: false, tca: true }), ghost("distant", { own: false, pos: { x: 99_999, y: 0 } })] };
const clock = { state: st, liveSimTime: () => st.simTime };
const icons = load("icons.ts");
const fleets = load("core/derive/fleet.ts", { "../../state": clock, "../../protocol": protocol });
const defense = load("core/derive/defense.ts", { "./fleet": fleets });
const readiness = load("core/derive/readiness.ts", { "./fleet": fleets, "../../protocol": protocol });
const aftermath = load("battleaftermath.ts");
const station = load("core/derive/station.ts");
const surface = load("shell/systemdefense.ts", { "../battleaftermath": aftermath,
  "../core/derive/defense": defense, "../core/derive/fleet": fleets,
  "../core/derive/readiness": readiness, "../protocol": protocol,
  "../core/derive/station": station, "../icons": icons });
let model = defense.systemDefense(st, system, report);
assert.deepEqual(plain(model.defenders.map(g => g.id)), ["assigned", "local"]);
assert.deepEqual(plain(model.contacts.map(g => g.id)), ["rival"]);
assert.equal(model.local.length, 1);
assert.equal(model.platforms, 2);
st.pendingOrders.set("candidate", [{ id: 1, fleet_id: "candidate", kind: "defend", target_id: "8", lost: false }]);
model = defense.systemDefense(st, system, report);
assert.equal(model.pending.length, 1);
assert.equal(model.assigned.length, 1, "pending command cannot manufacture a served defender");
st.ghosts.find(g => g.id === "assigned").shown = { x: 0, y: 0 };
assert.equal(defense.systemDefense(st, system, report).local.length, 1, "client estimates cannot claim local presence");
for (const theme of ["deck", "mobile"]) {
  const html = surface.systemDefenseHtml(st, system, report, 10_000, theme, true);
  assert.match(html, /2 platform tiers · 1 local fleet/);
  assert.match(html, /80% hull · 50 Deuterium · report 10s old/);
  assert.match(html, /Assigned · under way/);
  assert.match(html, /Defense assignment sent · fleet candidate · awaiting report/);
  assert.match(html, /defense-assign[^>]*data-fleet="candidate" disabled/);
  assert.match(html, /data-radius="10000" aria-pressed="true"/);
  assert.doesNotMatch(html, /System is safe|Guaranteed|data-fleet="rival"/);
}
assert.doesNotMatch(surface.systemDefenseHtml(st, system, report, 10_000, "deck"), /data-radius=/,
  "assignment controls start compact, not a full roster dumped into Overview");
st.timeline.push({ at_time: 50, text: "Pirate raid incoming: Freya — 2 raiders." });
assert.match(surface.systemDefenseHtml(st, system, report, 10_000, "deck"), /Raid warning received at 50s/);

st.pendingOrders.clear();
const commands = load("core/fleetorders.ts", { "./derive/fleet": fleets, "../icons": icons });
const orders = load("core/derive/orders.ts", { "../../state": clock, "../fleetorders": commands,
  "./fleet": fleets, "../../icons": icons });
const renderer = { stateVersion: 0 };
const intent = load("core/intent.ts", { "../state": clock, "../render": { renderer },
  "./fleetorders": commands, "./derive/fleet": fleets, "./derive/orders": orders });
const sent = [];
intent.bindIntentCore(() => ({ connected: true, send: msg => sent.push(plain(msg)) }), () => {});
const ctx = { state: st, renderer, intent, send: msg => sent.push(plain(msg)) };
surface.stageDefenseAction(ctx, "defense-assign", "8", "candidate", 20_000);
assert.equal(sent.length, 0, "Assign only previews");
assert.match(commands.fleetCommandSummary(st.pendingIntent, st), /Defend Freya · pursuit limit 20,000 su/);
assert.deepEqual(plain(st.pendingIntent.dest), system.pos);
intent.clearPendingIntent();
assert.equal(sent.length, 0, "Cancel changes neither assignment nor wire");
surface.stageDefenseAction(ctx, "defense-assign", "8", "candidate", 20_000);
intent.confirmPendingIntent(); intent.confirmPendingIntent();
assert.deepEqual(sent, [{ type: "DefendSystem", fleet_id: "candidate", system_id: "8", pursuit_radius: 20_000 }]);
assert.equal(st.ghosts.find(g => g.id === "candidate").defend_system, undefined);
surface.stageDefenseAction(ctx, "defense-release", "8", "assigned", 10_000);
assert.equal(sent.length, 1);
intent.confirmPendingIntent();
assert.equal(sent[1].type, "HoldFleet");
assert.equal(st.ghosts[0].defend_system, post, "Release does not locally erase the post");

const bad = { type: "DefendSystem", fleet_id: "candidate", system_id: "8", pursuit_radius: Infinity };
assert.equal(commands.fleetCommandsValid([bad], st), false);
report.owner = "2";
assert.equal(commands.fleetCommandsValid([{ ...bad, pursuit_radius: 10_000 }], st), false);
report.owner = "1";
assert.equal(commands.fleetCommandsValid([{ ...bad, fleet_id: "rival", pursuit_radius: 10_000 }], st), false);

// §orbital-defense: the station as the owner's report shows it.
assert.equal(station.stationName(5), "Star Fortress");
assert.equal(station.stationArtUrl(9, 512), "/art/derived/stations/v1/level-5-512.webp", "levels clamp to the ladder");
assert.equal(station.mountFitError(["mass_driver", "mass_driver", "whipple_armor"], false), "");
assert.match(station.mountFitError(["torpedo_rack", "torpedo_rack", "whipple_armor"], false), /needs 9 fitting points; a hardpoint has 8/);
assert.match(station.mountFitError(["point_defense_screen"], false), /weapons, armour and webs only/);
assert.match(station.mountFitError(["prismatic_lance"], false), /Platform Lances research/);
assert.equal(station.mountFitError(["prismatic_lance"], true), "");
assert.match(station.mountFitError(["mass_driver", "mass_driver", "mass_driver", "mass_driver"], false), /three slots/);
assert.deepEqual(plain(station.refitCrates(["mass_driver"], ["mass_driver", "mass_driver", "prismatic_lance", "whipple_armor"])),
  { mass_driver: 1, whipple_armor: 1 }, "only new crates are drawn; a lance comes with research");
const citadel = { level: 4, name: "Citadel", body_id: "81", mounts: [
  { modules: ["mass_driver", "mass_driver", "whipple_armor"], health: 1, offline: false },
  { modules: [], health: .6, offline: false },
  { modules: ["torpedo_rack"], health: .1, offline: true },
  { modules: [], health: 1, offline: false }] };
assert.equal(station.stationSummary(citadel), "Citadel · 3 of 4 hardpoints firing");
assert.equal(station.stationSummary({ ...citadel, mounts: citadel.mounts.map(m => ({ ...m, offline: true })) }),
  "Citadel · silenced: every hardpoint is out");
st.galaxy.build_options = [{ key: "orbital_defense", tiers: [
  { tier: 4, label: "Citadel", description: "Four hardpoints.", research_prerequisite: "weap_cm_v_grand_batteries", requires: { yard: "naval_drydock", tier: 1 } },
  { tier: 5, label: "Star Fortress", description: "Six hardpoints; one per corporation.", research_prerequisite: "weap_cm_v_star_fortress", requires: { yard: "capital_slipway", tier: 1 } }] }];
st.research = { programmes: [{ id: "weap_cm_v_star_fortress", name: "Star Fortress", state: "available" }] };
const fortified = { ...report, station: citadel, structures: { naval_drydock: 1 }, modules: { mass_driver: 1 } };
for (const theme of ["deck", "mobile"]) {
  const html = surface.systemDefenseHtml(st, system, fortified, 10_000, theme);
  assert.match(html, /<span>Citadel · 3 of 4 hardpoints firing · 1 local fleet<\/span>/);
  assert.match(html, /level-4-256\.webp/);
  assert.match(html, /Mass Driver ×2 · Whipple Armor · Firing/);
  assert.match(html, /Beam battery \(stock\) · Firing · 60% hull/);
  assert.match(html, /Knocked out · repairing, 10% \(back at 25%\)/);
  assert.match(html, /defense-refit"[^>]*data-mount="0" disabled title="Refitting needs an Ordnance Foundry in the system"/);
  assert.match(html, /Next: <b>Star Fortress<\/b>.*Needs Star Fortress research and Capital Slipway tier 1 in the system\./);
  assert.match(html, /Raise to Star Fortress/);
}
fortified.structures.ordnance_foundry = 1;
let draft = surface.stationDraftAction("defense-refit", { mount: "1" }, null, fortified).draft;
assert.deepEqual(plain(draft), { mount: 1, fit: [] }, "a refit starts from the fitted battery");
for (const module of ["mass_driver", "mass_driver", "whipple_armor", "stasis_web"]) {
  draft = surface.stationDraftAction("defense-refit-add", { mount: "1", module }, draft, fortified).draft;
}
assert.deepEqual(plain(draft.fit), ["mass_driver", "mass_driver", "whipple_armor"], "three slots");
let html = surface.systemDefenseHtml(st, system, fortified, 10_000, "deck", false, draft);
assert.match(html, /Not enough crates here: 2× Mass Driver \(have 1\), 1× Whipple Armor \(have 0\)\./);
assert.match(html, /defense-refit-confirm"[^>]*data-mount="1" disabled/);
assert.doesNotMatch(html, /Prismatic Lance/, "no lance before Platform Lances");
draft = surface.stationDraftAction("defense-refit-remove", { mount: "1", index: "1" }, draft, fortified).draft;
draft = surface.stationDraftAction("defense-refit-remove", { mount: "1", index: "1" }, draft, fortified).draft;
assert.deepEqual(plain(draft.fit), ["mass_driver"]);
html = surface.systemDefenseHtml(st, system, fortified, 10_000, "deck", false, draft);
assert.match(html, /defense-refit-confirm"[^>]*data-mount="1">Refit hardpoint 2/, "one driver crate on hand: refit allowed");
const confirmed = surface.stationDraftAction("defense-refit-confirm", { mount: "1" }, draft, fortified);
assert.equal(confirmed.draft, null);
assert.deepEqual(plain(confirmed.send), { type: "FitStation", system_id: "8", mount: 1, fit: ["mass_driver"] });
assert.equal(surface.stationDraftAction("defense-refit-cancel", { mount: "1" }, draft, fortified).draft, null);
assert.equal(surface.stationDraftAction("defense-assign", {}, draft, fortified), undefined, "not a refit action");
assert.match(surface.systemDefenseHtml(st, system, { ...fortified, station: { ...citadel, level: 5, name: "Star Fortress" } }, 10_000, "deck"),
  /The Star Fortress is the top of the ladder\./);
assert.match(surface.systemDefenseHtml(st, system, report, 10_000, "deck"), /2 platform tiers/, "legacy tiers still read as tiers");

console.log("System defense: served membership, report-only threats, compact desktop/mobile controls, readiness, pending separation, Confirm/Cancel and release pass; the station ladder's summary, hardpoints, refit editor and next level pass.");

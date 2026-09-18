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
const surface = load("shell/systemdefense.ts", { "../battleaftermath": aftermath,
  "../core/derive/defense": defense, "../core/derive/fleet": fleets,
  "../core/derive/readiness": readiness, "../protocol": protocol });
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
  assert.match(html, /80% hull · 50 fuel · report 10s old/);
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

console.log("System defense: served membership, report-only threats, compact desktop/mobile controls, readiness, pending separation, Confirm/Cancel and release pass.");

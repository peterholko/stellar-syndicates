import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function compile(source, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, ...globals });
  return exports;
}
const source = readFileSync(new URL("../src/battlehistory.ts", import.meta.url), "utf8");
const history = compile(source);
const { loadBattleMarks, saveBattleMarks, reportMarkKey, battleRecordMarkKey, recordForBattleReport, reportForBattleRecord } = history;
const data = new Map([["ss_battle_marks", JSON.stringify({ viewed: [1], dismissed: [1] })]]);
const storage = { getItem: (key) => data.get(key) ?? null, setItem: (key, value) => data.set(key, value) };
const pos = { x: 30_000, y: 0 };
const a = { id: "99", pos, outcome: "target_destroyed" };
const b = { id: "100", pos, outcome: "target_destroyed" };
const reportA = { id: 1, battle_id: a.id, pos, at_time: 40 };
const reportB = { id: 2, battle_id: b.id, pos, at_time: 40 };
const capture = { id: 1, captor: true, pos };
const state = (instance = "galaxy-A", playerId = "1") => ({
  playerId, galaxy: { instance_id: instance }, battleRecords: [a, b],
  battleReports: [reportA, reportB], captureReports: [capture],
  battleViewed: new Set(), battleDismissed: new Set(),
});

const original = state();
loadBattleMarks(original, storage);
assert.equal(original.battleDismissed.size, 0, "ignore ambiguous old numeric dismissals");
original.battleDismissed.add(reportMarkKey(reportA));
original.battleDismissed.add(reportMarkKey(capture));
original.battleViewed.add(reportMarkKey(reportA));
original.battleViewed.add(reportMarkKey(capture));
saveBattleMarks(original, storage);
const reloaded = state();
loadBattleMarks(reloaded, storage);
assert.ok(reloaded.battleDismissed.has(battleRecordMarkKey(a.id)));
assert.ok(reloaded.battleDismissed.has(reportMarkKey(capture)), "capture preferences survive too");
assert.ok(!reloaded.battleDismissed.has(battleRecordMarkKey(b.id)), "co-located battle has separate identity");
assert.notEqual(reportMarkKey(capture), reportMarkKey(reportA), "capture and battle counters cannot alias");
const unscoped = state();
delete unscoped.galaxy.instance_id;
for (const fresh of [state("galaxy-B"), state("galaxy-A", "2"), unscoped]) {
  fresh.battleDismissed.add(battleRecordMarkKey(a.id));
  fresh.battleViewed.add(battleRecordMarkKey(a.id));
  loadBattleMarks(fresh, storage);
  assert.equal(fresh.battleDismissed.size, 0, "fresh game, other player or unscoped legacy server must not inherit marks");
  assert.equal(fresh.battleViewed.size, 0);
}
original.battleReports = [];
saveBattleMarks(original, storage);
loadBattleMarks(reloaded, storage);
assert.ok(reloaded.battleDismissed.has(battleRecordMarkKey(a.id)), "keep dismissal while replay survives report pruning");

assert.equal(recordForBattleReport(reportA, [b, a]), a);
assert.equal(recordForBattleReport(reportB, [a, b]), b);
assert.equal(reportForBattleRecord(a, [reportB, reportA]), reportA);
assert.equal(reportForBattleRecord(b, [reportA, reportB]), reportB);
assert.equal(recordForBattleReport({ ...reportA, battle_id: undefined }, [a, b]), undefined,
  "legacy reports never guess a replay from coordinates, even at identical end times");
assert.equal(recordForBattleReport({ ...reportA, battle_id: "missing" }, [a, b]), undefined);

for (const raw of ["not JSON", "null", JSON.stringify({ viewed: [1, {}, "garbage"], dismissed: [1, null] })]) {
  const broken = state();
  loadBattleMarks(broken, { ...storage, getItem: () => raw });
  assert.equal(broken.battleDismissed.size, 0);
  assert.equal(broken.battleViewed.size, 0);
}
const unavailable = { getItem() { throw Error("storage blocked"); }, setItem() { throw Error("quota"); } };
loadBattleMarks(reloaded, unavailable);
reloaded.battleDismissed.add(battleRecordMarkKey(a.id));
assert.doesNotThrow(() => saveBattleMarks(reloaded, unavailable));
assert.ok(reloaded.battleDismissed.has(battleRecordMarkKey(a.id)), "memory-only dismissal still works");

// Exercise the actual Welcome reducer: reconnect reloads exactly its scope,
// while switching corporation or replacing the galaxy drops the old marks.
const sessionSource = readFileSync(new URL("../src/core/session.ts", import.meta.url), "utf8");
const deps = {
  "../battlehistory": { loadBattleMarks: (st) => loadBattleMarks(st, storage) },
  "../state": { syncRenderClock() {} },
  "./derive/market": { marketReservations: [], recentMarketOrders: [] },
  "../wire.mjs": { PROTOCOL_VERSION: 1 },
};
const { applyServerMessage } = compile(sessionSource, { require: (name) => deps[name] ?? {} });
const current = state();
const welcome = (instance, playerId) => ({ type: "Welcome", player_id: playerId, name: "Player",
  tick_hz: 30, pacing_scale: 4, tick: 0, sim_time: 0, galaxy: { instance_id: instance } });
applyServerMessage(welcome("galaxy-A", "1"), current);
assert.ok(current.battleDismissed.has(battleRecordMarkKey(a.id)), "same-game reconnect restores saved exact dismissal");
applyServerMessage(welcome("galaxy-A", "2"), current);
assert.equal(current.battleDismissed.size, 0, "Welcome changes player scope, not only DOM construction");
applyServerMessage(welcome("galaxy-A", "1"), current);
assert.ok(current.battleDismissed.has(battleRecordMarkKey(a.id)));
applyServerMessage(welcome("galaxy-B", "1"), current);
assert.equal(current.battleDismissed.size, 0, "a same-seed reset has a different server instance id");

// Exercise the real route/controller too: an engagement id and report counter
// may have the same digits. Only an explicit report route opens that report.
const routeState = state();
routeState.battles = [{ id: "1" }];
routeState.battleRecords = [
  { ...a, rounds: [], fidelity: "participant" },
  { ...b, rounds: [], fidelity: "participant" },
];
const routeDeps = {
  "../battlewithdraw": compile(readFileSync(new URL("../src/shell/battlewithdraw.ts", import.meta.url), "utf8"), { require: () => ({}) }),
  "../../battlehistory": { ...history, saveBattleMarks: (st) => saveBattleMarks(st, storage) },
  "../../core/derive/fleet": { recordForReport: (r) => recordForBattleReport(r, routeState.battleRecords) },
  "../../core/derive/format": { fmtDur: String, informationDelay: String },
  "../../core/derive/geo": { nearestSystemName: () => "Freya" },
  "../../state": { liveSimTime: () => 55 },
};
const { DeckStrategicRoutes } = compile(readFileSync(new URL("../src/shell/deck/strategic.ts", import.meta.url), "utf8"), {
  require: (name) => routeDeps[name] ?? {},
});
let backs = 0;
const routeUi = new DeckStrategicRoutes({}, { state: routeState, renderer: {} }, { back: () => backs++ });
const battleHtml = routeUi.battleReportHtml.bind(routeUi);
routeUi.ongoingBattleHtml = (battle) => `ongoing:${battle.id}`;
routeUi.battleReportHtml = (report) => `result:${report.battle_id}`;
routeUi.captureReportHtml = (report) => `capture:${report.id}`;
const route = (id, report) => ({ name: "battle", params: { id, report } });
assert.equal(routeUi.battleHtml(route("1")), "ongoing:1", "bare ids mean engagements, never report counters");
assert.equal(routeUi.battleHtml(route("1", "battle")), "result:99");
assert.equal(routeUi.battleHtml(route("1", "capture")), "capture:1");
assert.equal(routeUi.battleHtml(route("99")), "result:99", "record route finds only its exact report");
routeState.battleReports = [];
assert.match(routeUi.battleHtml(route("99")), /strategic-battle-dismiss-record/,
  "history remains dismissible even when its report was pruned or predates explicit links");
routeState.battleRecords[1].outcome = null;
assert.doesNotMatch(routeUi.battleHtml(route("100")), /strategic-battle-dismiss/,
  "a battle whose outcome has not arrived cannot be dismissed");
const click = (act, data = {}) => routeUi.handleAction({ dataset: { deckAct: act, ...data } }, route("99"));
click("strategic-battle-dismiss-record", { record: "100" });
assert.equal(backs, 0);
assert.equal(routeState.battleDismissed.size, 0);
click("strategic-battle-dismiss-record", { record: "99" });
assert.ok(routeState.battleDismissed.has(battleRecordMarkKey("99")));
assert.equal(backs, 1);
routeState.battleDismissed.clear();
routeState.battleReports = [reportA];
click("strategic-battle-dismiss", { report: "1", reportKind: "capture" });
assert.ok(routeState.battleDismissed.has(reportMarkKey(capture)));
assert.ok(!routeState.battleDismissed.has(reportMarkKey(reportA)), "capture click never dismisses a numerically equal battle report");
const completeReport = { ...reportA, learned_at: 55, you: "attacker", outcome: "target_destroyed", attacker_losses: [], target_losses: [] };
assert.match(battleHtml(completeReport), /strategic-battle-dismiss/);
assert.doesNotMatch(battleHtml({ ...completeReport, battle_id: undefined }), /strategic-battle-dismiss/,
  "legacy result cannot promise to dismiss a marker it cannot identify");

console.log("PASS: exact battle/report joins and routes, co-located fights, scoped persistence/reconnects, galaxy resets, player switches, legacy marks, captures, pruning and unavailable storage.");

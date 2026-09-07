// Actual aftermath controller, pure selectors and art, with served-only fixtures.
// Optional --serve opens a local visual fixture; never connects to a game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
function compile(source, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, ...globals });
  return exports;
}
const history = compile(src("battlehistory.ts"));
const helpers = compile(src("battleaftermath.ts"));
const icons = compile(src("icons.ts"));
const art = compile(src("shell/art.ts"));
const captains = compile(src("core/derive/captains.ts"), { require: () => ({}) });
const withdrawal = compile(src("shell/battlewithdraw.ts"), { require: () => ({}) });
const progression = { level: 1, title: "lieutenant", portrait_age: "young", command_capacity: 4, xp: 20,
  next_level_xp: 100, unspent: 0, attributes: { command: 1, navigation: 1, fieldcraft: 0, logistics: 0 } };
const survivor = { fleet_id: "101", kind: "raider", composition: { raider: 1 }, hull: .72, withdrew: false,
  guard_target: "102", captain: { id: 0, name: "Mara Venn", portrait: "mara_venn", before: progression,
    after: { ...progression, level: 2, title: "lieutenant_commander", xp: 105, next_level_xp: 300 } } };
const freighter = { fleet_id: "102", kind: "convoy", composition: { convoy: 1 }, hull: .9, withdrew: true,
  guard_target: null, captain: null };
const baseReport = { id: 1, battle_id: "99", pos: { x: 30_000, y: 0 }, at_time: 40, learned_at: 55,
  you: "attacker", outcome: "target_destroyed", attacker_losses: [], target_losses: [{ kind: "raider", count: 1 }],
  aftermath: { survivors: [survivor, freighter], bounty_credits: 4500 } };
function fixture() {
  let now = 1000;
  const report = structuredClone(baseReport);
  const state = { playerId: "1", galaxy: { instance_id: "fixture", systems: [
    { id: "8", name: "Near Yard", pos: { x: 35_000, y: 0 } },
    { id: "9", name: "Far Yard", pos: { x: 90_000, y: 0 } },
  ] }, systems: ["8", "9"].map((id) => ({
    id, owner: "1", bodies: [{ id: 0, parent: null }], structures: { ordnance_foundry: 1 },
    assignments: [{ structure: "ordnance_foundry", body_id: 0, staffing: 1, skill: 1 }],
    stockpile: [{ commodity: "alloys", units: 20 }, { commodity: "machinery", units: 10 }],
  })), ghosts: report.aftermath.survivors.map((s) => ({ id: s.fleet_id, own: true, owner: "1", kind: s.kind,
    pos: { x: 30_000, y: 0 }, damage: .04, docked: null, guard_target: null, captain: { xp: 9999 } })),
    battles: [], pendingOrders: new Map(), battleReports: [report], captureReports: [], battleRecords: [],
    battleViewed: new Set(), battleDismissed: new Set() };
  const sent = [], previews = [], selections = [];
  const deps = {
    "../../battlehistory": { ...history, saveBattleMarks() {} },
    "../../battleaftermath": helpers,
    "../battlewithdraw": withdrawal,
    "../../core/derive/fleet": { recordForReport: () => undefined, guardCapable: (g) => g.own && g.kind === "raider",
      shipKindLabel: (kind) => ({ raider: "Interceptor", convoy: "Freighter" }[kind] ?? kind) },
    "../../core/derive/captains": captains, "../art": art, "../../icons": icons,
    "../../core/derive/format": { fmt: (n) => Math.round(n).toLocaleString("en-US"), fmtDur: (n) => `${Math.round(n)}s`,
      informationDelay: (n) => `${Math.round(n)}s information delay` },
    "../../core/derive/geo": { nearestSystemName: () => "Freya" },
    "../../state": { liveSimTime: () => 60 },
  };
  const { DeckStrategicRoutes } = compile(src("shell/deck/strategic.ts"), {
    require: (name) => deps[name] ?? {}, performance: { now: () => now },
  });
  const ui = new DeckStrategicRoutes({}, { state, renderer: {}, send: (m) => sent.push(m),
    intent: { beginPendingIntent: intent => previews.push(intent) } },
    { notice() {}, selectFleets: (ids) => selections.push([...ids]) });
  return { state, report, ui, sent, previews, selections, advance: () => { now += 1600; },
    html: () => ui.battleReportHtml(report),
    click: (action, id = "101", reportId = "1") => ui.handleAction({
      dataset: { deckAct: `strategic-survivor-${action}`, fleet: id, report: reportId },
    }, { name: "battle", params: { report: "battle", id: reportId } }) };
}

const f = fixture();
let html = f.html();
assert.match(html, />Victory</);
assert.match(html, /72% hull/);
assert.match(html, /\+85 XP/);
assert.match(html, /Promoted · Lieutenant-Commander/);
assert.match(html, /\+4,500 Cr/);
assert.doesNotMatch(html, /9999|96% hull/, "never reconstruct battle results from newer telemetry");
assert.match(html, /player_interceptor|interceptor|raider/i);
f.report.aftermath.survivors[0].captain.name = "<script>bad</script>";
assert.doesNotMatch(f.html(), /<script>bad/);
f.click("select-all");
assert.deepEqual(f.selections[0], ["101", "102"]);
const ghostBefore = JSON.stringify(f.state.ghosts);
f.click("repair");
assert.equal(f.sent.length, 0, "repair opens confirmation, never sends on first click");
assert.equal(f.previews[0].verb, "move");
assert.equal(f.previews[0].shipId, "101");
assert.equal(f.previews[0].dest.x, 35_000);
f.click("repair");
assert.equal(f.sent.length, 0, "repeated first presses still cannot transmit");
assert.equal(JSON.stringify(f.state.ghosts), ghostBefore, "issuing repair must not repair/move the served fleet");
f.advance();
f.click("guard");
assert.equal(f.previews.at(-1).verb, "guard");
assert.equal(f.previews.at(-1).targetId, "102");
assert.equal(f.sent.length, 0);
assert.equal(JSON.stringify(f.state.ghosts), ghostBefore, "guard is a delayed order, not a local assignment");
f.advance();
f.state.pendingOrders.set("101", [{ fleet_id: "101", kind: "move" }]);
const previewCount = f.previews.length;
f.click("guard"); f.click("repair");
assert.equal(f.previews.length, previewCount, "do not stack accidental recovery orders");
f.state.pendingOrders.clear();
f.state.ghosts[0].guard_target = "102";
f.click("guard");
assert.equal(f.previews.length, previewCount, "already guarding needs no redundant order");
f.state.ghosts = [];
f.click("select"); f.click("repair"); f.click("guard");
assert.equal(f.previews.length, previewCount, "historical survivors that later vanished are not actionable");
assert.match(f.html(), /72% hull/, "later fleet losses do not erase battle history");
f.state.battleReports = [];
f.click("repair");
assert.equal(f.previews.length, previewCount, "a forged/unarrived report id cannot issue an aftermath action");
assert.equal(f.sent.length, 0);

for (const mutate of [
  (s) => { s.systems.forEach((r) => { r.owner = "2"; }); },
  (s) => { s.systems.forEach((r) => { r.assignments[0].staffing = 0; }); },
  (s) => { s.systems.forEach((r) => { r.assignments[0].body_id = 7; }); },
  (s) => { s.systems.forEach((r) => { r.stockpile = []; }); },
  (s) => { s.systems.forEach((r) => { r.structures = { shipyard: 3 }; }); },
]) {
  const unavailable = fixture(); mutate(unavailable.state);
  assert.equal(helpers.nearestRepairYard(unavailable.state, unavailable.state.ghosts[0]), undefined);
  unavailable.click("repair");
  assert.equal(unavailable.sent.length, 0);
  assert.match(unavailable.html(), /Repairs need a staffed/);
}
const lost = fixture();
lost.report.outcome = "attacker_destroyed"; lost.report.aftermath.survivors = [];
assert.match(lost.html(), />Defeat</);
assert.match(lost.html(), /No surviving fleets/);
assert.doesNotMatch(lost.html(), /strategic-survivor-/);
const legacy = fixture(); delete legacy.report.aftermath;
assert.match(legacy.html(), /Not recorded/);
assert.doesNotMatch(legacy.html(), /strategic-survivor-/);
console.log("PASS: frozen aftermath, hull/XP/promotion/bounty, losses, owner-only action eligibility, delayed recovery commands, duplicate presses, legacy reports and unavailable yards.");

if (process.argv.includes("--serve")) {
  const publicRoot = resolve(fileURLToPath(new URL("../public/", import.meta.url)));
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      const view = fixture();
      if (url.searchParams.has("legacy")) delete view.report.aftermath;
      if (url.searchParams.has("defeat")) { view.report.outcome = "attacker_destroyed"; view.report.aftermath.survivors = []; }
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css"></head><body><div class="deck"><aside class="deck-workspace" style="width:min(460px,100vw);right:0;top:0;bottom:0;pointer-events:auto"><header class="deck-workspace__header"><h1>Battle</h1></header><div class="deck-workspace__body">${view.html()}</div></aside></div></body></html>`);
      return;
    }
    let path;
    if (url.pathname === "/tokens.css" || url.pathname === "/deck.css") {
      path = fileURLToPath(new URL(`../src/styles${url.pathname}`, import.meta.url));
      response.setHeader("Content-Type", "text/css");
    } else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
    } else { response.writeHead(404).end(); return; }
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  server.listen(0, "127.0.0.1", () => console.log(`Aftermath fixture: http://127.0.0.1:${server.address().port}/`));
}

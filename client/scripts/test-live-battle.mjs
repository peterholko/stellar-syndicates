// Exercise the actual Deck transport and map drawing method with arrived-only
// records. No login, live server, Pixi context, timer sleeps or future battle data.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function compile(source, globals) {
  const exports = {};
  const js = ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText;
  vm.runInNewContext(js, { exports, ...globals });
  return exports;
}

const source = readFileSync(new URL("../src/shell/deck/theaters.ts", import.meta.url), "utf8");
const ordersSource = readFileSync(new URL("../src/core/derive/orders.ts", import.meta.url), "utf8");
const withdrawSource = readFileSync(new URL("../src/shell/battlewithdraw.ts", import.meta.url), "utf8");
const history = compile(readFileSync(new URL("../src/battlehistory.ts", import.meta.url), "utf8"), {});
const round = (i) => ({ tick: i * 15, counts: [[], []], notes: [], dealt: [0, 0] });
const record = () => ({
  id: "15s-battle", pos: { x: 30_000, y: 0 }, started_at: 0,
  own_side: 0, fidelity: "participant", raid: true, outcome: null,
  sides: [{ initial: [] }, { initial: [] }], rounds: [round(0), round(1)],
});

function fixture(pacingScale = 1, completed = false, semantic = true) {
  let now = 1_000, nextTimer = 0;
  const timers = new Map(), displayed = [], routes = [], sent = [];
  const viewerTimers = { close: null, aftermath: null };
  const clearTimer = (field) => { timers.delete(viewerTimers[field]); viewerTimers[field] = null; };
  const node = () => {
    const listeners = new Map();
    return { hidden: true, innerHTML: "", addEventListener(type, handler) {
      const handlers = listeners.get(type) ?? [];
      handlers.push(handler); listeners.set(type, handlers);
    }, emit(type, event) { for (const handler of listeners.get(type) ?? []) handler(event); },
    querySelector() { return null; }, classList: { add() {}, remove() {}, toggle() {} } };
  };
  const battle = record();
  if (completed) battle.outcome = "target_destroyed";
  const state = { battleRecords: [battle], battles: [{ id: battle.id, pos: battle.pos, age: 60, own: true, participants: ["101", "102", "pirate"] }],
    ghosts: [{ id: "101", own: true, kind: "raider" }, { id: "102", own: true, kind: "convoy" }, { id: "pirate", own: false, kind: "raider" }],
    pendingOrders: new Map(), groundRecords: [], tickHz: 30, pacingScale,
    commandCenter: { x: 0, y: 0 }, galaxy: { c: 400 } };
  const fleetHelpers = { WARP_FACTOR: 5, shipKindLabel: kind => ({ raider: "Interceptor", convoy: "Freighter" }[kind] ?? kind) };
  const clock = { state, liveSimTime: () => now / 1_000 };
  const orders = compile(ordersSource, { require: id => ({ "../../state": clock, "./fleet": fleetHelpers }[id] ?? {}) });
  const withdrawal = compile(withdrawSource, { require: id => ({
    "../state": clock, "../core/derive/fleet": fleetHelpers, "../core/derive/orders": orders,
  }[id] ?? {}) });
  const dependencies = {
    "../../battletheater": { theaterAvailable: () => false, theaterClose() {},
      theaterSetTime: (round, frac, live) => displayed.push({ round, frac, live }) },
    "../../core/derive/fleet": { ...fleetHelpers, battleReportForRecord: () => ({ id: 7 }), battleViewerTimers: viewerTimers,
      clearBattleAftermathTimer: () => clearTimer("aftermath"), clearBattleCloseTimer: () => clearTimer("close") },
    "../../core/derive/format": { fmtDur: (s) => `${s}s`, informationDelay: (s) => `${s}s information delay` },
    "../../core/derive/geo": { nearestSystemName: () => "Freya" },
    "../../groundtheater": { groundTheaterClose() {} },
    "../../state": clock,
    "../../core/derive/orders": orders,
    "../battlewithdraw": withdrawal,
    "../dom": { setHtml: (element, html) => { element.innerHTML = html; } },
  };
  const { DeckTheaters } = compile(source, {
    require: (id) => dependencies[id] ?? {}, performance: { now: () => now },
    requestAnimationFrame: () => 1, cancelAnimationFrame() {},
    window: { setTimeout: (callback, delay) => {
      const id = ++nextTimer; timers.set(id, { callback, at: now + delay }); return id;
    } },
  });
  const root = node(), card = node();
  const renderer = { viewMode: { type: "battle" }, exitBattleView() { this.viewMode = { type: "galaxy" }; } };
  const ctx = { state, renderer, send: command => sent.push(command) };
  const theater = new DeckTheaters(root, card, node(), node(), ctx,
    { go: (route) => routes.push(route), notice() {}, openDoctrine() {} }, new AbortController().signal);
  theater.openBattle(battle.id, { semantic });
  theater.tickBattle(now);
  const step = (seconds) => {
    for (let frame = 0; frame < Math.ceil(seconds * 60); frame++) {
      now += 1_000 / 60;
      theater.tickBattle(now);
      for (const [id, timer] of timers) if (timer.at <= now) { timers.delete(id); timer.callback(); }
    }
  };
  const click = (action, values = {}) => theater.battleClick({ target: { closest: (selector) =>
    selector === "[data-deck-theater-act]" ? { dataset: { deckTheaterAct: action, ...values } } : null } });
  return { theater, renderer, state, battle, root, card, displayed, routes, timers, step, click, sent, ctx, dependencies };
}

const cases = [];
const test = (name, run) => cases.push({ name, run });
const noReplayControls = (html) => assert.doesNotMatch(html, /deck-theater-(?:transport|tick|scrub|speeds)/);

test("live battles have no speed controls or round boxes", () => {
  const f = fixture();
  assert.match(f.card.innerHTML, /FOLLOWING LIGHT/);
  noReplayControls(f.card.innerHTML);
});

test("withdraw quotes one-way warp command delay, not report age, and requires confirmation", () => {
  const f = fixture();
  const target = { battle: f.battle.id, fleet: "101" };
  assert.match(f.card.innerHTML, />Withdraw Interceptor · ~15s</);
  assert.match(f.card.innerHTML, />Withdraw Freighter · ~15s</);
  assert.doesNotMatch(f.card.innerHTML, /data-fleet="pirate"|withdraw-confirm/);
  f.click("withdraw-confirm", target);
  assert.equal(f.sent.length, 0, "confirmation must be armed first");
  f.click("withdraw-ask", target);
  assert.equal(f.sent.length, 0, "first press never dispatches");
  assert.match(f.card.innerHTML, /The order will take about 15 seconds to reach this fleet/);
  assert.match(f.card.innerHTML, />Confirm withdraw Interceptor · ~15s</);
  f.click("withdraw-cancel", target);
  assert.equal(f.sent.length, 0);
  assert.doesNotMatch(f.card.innerHTML, /withdraw-confirm/);
  f.click("withdraw-ask", target);
  f.theater.onViewTick();
  assert.match(f.card.innerHTML, /Confirm withdraw/, "new Views preserve an armed confirmation");
  f.click("withdraw-confirm", { ...target, fleet: "102" });
  assert.equal(f.sent.length, 0, "one fleet's confirmation cannot order another");
  f.click("withdraw-confirm", target);
  assert.equal(JSON.stringify(f.sent), JSON.stringify([{ type: "Withdraw", fleet_id: "101" }]));
  f.click("withdraw-confirm", target);
  assert.equal(f.sent.length, 1, "a duplicate Confirm cannot send twice");
});

test("withdraw revalidates arrived battle membership, completion, and delay", () => {
  for (const invalidate of [f => { f.battle.outcome = "target_destroyed"; },
    f => { f.state.battles = []; }, f => { f.state.ghosts[0].own = false; },
    f => { f.state.battles[0].participants = ["102"]; }]) {
    const f = fixture(), target = { battle: "15s-battle", fleet: "101" };
    f.click("withdraw-ask", target);
    invalidate(f);
    f.click("withdraw-confirm", target);
    assert.equal(f.sent.length, 0, "stale controls cannot withdraw an ended battle or non-participant");
  }
  const f = fixture(), target = { battle: "15s-battle", fleet: "101" };
  f.click("withdraw-ask", target);
  f.state.commandCenter.x = 10_000;
  f.step(1.1);
  f.theater.onViewTick();
  assert.match(f.card.innerHTML, /about 10 seconds/);
  f.state.commandCenter = null;
  f.step(1.1);
  f.theater.onViewTick();
  assert.match(f.card.innerHTML, /Command delay unavailable/);
  assert.doesNotMatch(f.card.innerHTML, /~0s|NaN|Infinity/);
  f.click("close"); f.step(0.6);
  f.theater.openBattle(f.battle.id);
  assert.doesNotMatch(f.card.innerHTML, /withdraw-confirm/, "reopening does not retain an old confirmation");
});

test("an in-flight withdrawal uses its served remaining delay and never assumes a response", () => {
  const f = fixture(), target = { battle: "15s-battle", fleet: "101" };
  f.click("withdraw-ask", target);
  f.state.pendingOrders.set("101", [{ fleet_id: "101", kind: "withdraw", arrives_at: 16 }]);
  f.theater.onViewTick();
  assert.match(f.card.innerHTML, /disabled>Withdraw sent · ~15s remaining/);
  assert.doesNotMatch(f.card.innerHTML, /withdraw-confirm/);
  f.click("withdraw-confirm", target);
  assert.equal(f.sent.length, 0);
  f.step(16);
  f.theater.onViewTick();
  assert.match(f.card.innerHTML, /Withdraw sent · awaiting response/);
  assert.doesNotMatch(f.card.innerHTML, /Withdraw sent · ~-/);
});

test("mobile live battle uses the same delay and explicit confirmation", () => {
  const f = fixture();
  const deps = { ...f.dependencies,
    "../signature": { sheetFingerprint: value => JSON.stringify(value) },
    "../../core/derive/fleet": { ...f.dependencies["../../core/derive/fleet"], sideFamily: () => "beam" },
  };
  const { MobileBattleTheater } = compile(readFileSync(new URL("../src/shell/mobile/battle.ts", import.meta.url), "utf8"), {
    require: name => deps[name] ?? {},
  });
  const entry = { id: "battle", props: { id: f.battle.id } };
  let html = "";
  const mobile = new MobileBattleTheater(f.ctx, { refresh() {
    if (mobile.refreshNeeded(entry)) html = mobile.render(entry).html;
  } });
  const click = action => mobile.handleClick({ target: { closest: () => ({
    dataset: { mobileAct: `battle-withdraw-${action}`, battle: f.battle.id, fleet: "102" },
  }) } });
  html = mobile.render(entry).html;
  assert.match(html, />Withdraw Freighter · ~15s</);
  click("ask");
  assert.equal(f.sent.length, 0);
  assert.match(html, /about 15 seconds/);
  click("cancel");
  assert.equal(f.sent.length, 0);
  assert.doesNotMatch(html, /battle-withdraw-confirm/);
  click("ask"); click("confirm"); click("confirm");
  assert.equal(JSON.stringify(f.sent), JSON.stringify([{ type: "Withdraw", fleet_id: "102" }]));
  f.battle.outcome = "target_destroyed";
  assert.doesNotMatch(mobile.render(entry).html, /battle-withdraw-ask/);
});

test("final report does not expose replay controls before the ending plays", () => {
  const f = fixture();
  f.battle.rounds.push(round(2), round(3));
  f.battle.outcome = "target_destroyed";
  f.state.battles = [];
  f.theater.onViewTick();
  noReplayControls(f.card.innerHTML);
  assert.doesNotMatch(f.card.innerHTML, />COMPLETE</);
  for (const action of ["play", "speed", "round"]) {
    f.click(action, { speed: "16", round: "0" });
    assert.equal(f.theater.battleLive, true, "late/forged replay clicks cannot interrupt the live ending");
  }
});

test("live playback follows 4x game pace, without passing arrived light", () => {
  const f = fixture(4);
  for (let i = 2; i <= 9; i++) f.battle.rounds.push(round(i));
  f.theater.onViewTick();
  f.step(1.05);
  console.log(`4x live probe: ${f.theater.battleRound - 1}/8 arrived intervals played in 1.05 wall seconds`);
  assert.equal(f.theater.battleRound, 9);
  f.step(2);
  assert.equal(f.theater.battleRound, 9, "hold the last arrival, never simulate ahead");
  assert.equal(f.theater.battleLive, true, "no outcome has arrived");
});

test("the delayed final window plays, then the viewer stays open", () => {
  const f = fixture();
  f.battle.rounds.push(round(2), round(3));
  f.battle.outcome = "target_destroyed";
  f.state.battles = [];
  f.theater.onViewTick();
  f.step(4);
  assert.ok(f.displayed.some((time) => time.round === 2 && time.frac > 0.3 && time.frac < 0.8 && time.live),
    "show the last impact/explosion interval before the final frame");
  assert.equal(f.theater.battleRound, 3);
  assert.equal(f.theater.battleLive, false);
  assert.equal(f.theater.isOpen, true, "do not automatically eject the viewer to the report");
  assert.equal(f.routes.length, 0);
  assert.equal(f.timers.size, 0);
  assert.match(f.card.innerHTML, />COMPLETE</);
  assert.match(f.card.innerHTML, /data-deck-theater-act="play"/);
  assert.match(f.card.innerHTML, /data-deck-theater-act="report"/);
  assert.match(f.card.innerHTML, />Battle aftermath</);
  f.click("report"); f.step(0.6);
  assert.equal(f.theater.isOpen, false, "the player can explicitly open the result");
  assert.equal(f.routes.at(-1).params.report, "battle");
});

test("opening a completed historical replay retains transport and 4x default", () => {
  const f = fixture(4, true);
  assert.equal(f.theater.battleLive, false);
  assert.equal(f.theater.battleSpeed, 4);
  assert.match(f.card.innerHTML, /data-deck-theater-act="speed"/);
  assert.match(f.card.innerHTML, /data-deck-theater-act="round"/);
});

test("scrolling never exits a live battle or replay; the explicit close button does", () => {
  for (const completed of [false, true]) for (const semantic of [false, true]) {
    const f = fixture(1, completed, semantic);
    f.step(0.6); // entry animation has settled
    // Zoom in, nudge out, then keep zooming out well past the old 60px exit
    // threshold. The host must not intercept the theater's camera gestures.
    for (const deltaY of [-240, 5, 15, 45, 120, 960, 960]) {
      const event = { deltaY, prevented: false, stopped: false,
        preventDefault() { this.prevented = true; },
        stopPropagation() { this.stopped = true; } };
      f.root.emit("wheel", event);
      f.step(0.6);
      assert.equal(f.theater.isOpen, true, `wheel delta ${deltaY} must stay inside the battle`);
      assert.equal(event.prevented, false, "the overlay must leave camera zoom to the canvas");
      assert.equal(event.stopped, false);
    }
    assert.match(f.card.innerHTML, /data-deck-theater-act="close"/);
    f.click("close"); f.step(0.6);
    assert.equal(f.theater.isOpen, false, "X still closes the viewer");
    assert.equal(f.root.hidden, true);
    assert.equal(f.renderer.viewMode.type, "galaxy");
  }
});

test("battle marker survives split View/record packets and remains selectable", () => {
  // Compile the real drawing method, omitting the unrelated Pixi scene setup.
  const source = readFileSync(new URL("../src/render.ts", import.meta.url), "utf8");
  const file = ts.createSourceFile("render.ts", source, ts.ScriptTarget.Latest, true);
  const owner = file.statements.find((node) => ts.isClassDeclaration(node) && node.members.some((m) => m.name?.getText(file) === "drawBattles"));
  assert.ok(!owner.members.some((m) => ["drawIntercepts", "interceptLabel"].includes(m.name?.getText(file))),
    "no predicted-intercept circles, guidance lines or countdown labels remain");
  const method = owner.members.find((m) => m.name?.getText(file) === "drawBattles");
  const { MapProbe } = compile(`export class MapProbe { ${method.getText(file)} }`, {
    performance: { now: () => 1_000 }, BATTLE_ONGOING_PX: 40, COL_OWN: 1, COL_THREAT: 2,
    ...history,
  });
  const map = new MapProbe();
  let clears = 0;
  const graphics = new Proxy({}, { get: (_, name) => () => {
    if (name === "clear") clears++;
    return graphics;
  } });
  Object.assign(map, { battleGfx: graphics, battleSprites: new Map(), worldToScreen: (pos) => pos });
  const rec = record();
  const state = { battles: [{ id: rec.id, pos: rec.pos, own: true }], battleRecords: [rec], battleReports: [], battleDismissed: new Set() };
  const draw = () => {
    const before = clears;
    map.drawBattles(state);
    assert.equal(clears, before + 1, "battle graphics clear without the removed prediction pass");
    return map.battleHits.map((hit) => hit.id).join();
  };
  assert.equal(draw(), rec.id, "one marker while both channels describe an ongoing battle");
  state.battles = []; // lossy View arrives before reliable final record
  assert.equal(draw(), rec.id, "retain the arrived battle while its concluding packet is in flight");
  rec.outcome = "target_destroyed";
  assert.equal(draw(), rec.id, "the same marker becomes replay history, not a vanished icon");
  state.battleReports = [{ id: 7, battle_id: rec.id, pos: rec.pos }];
  state.battleDismissed.add(history.reportMarkKey(state.battleReports[0]));
  assert.equal(draw(), "", "explicit report dismissal still removes the historical marker");
  const later = { ...rec, id: "later-battle-at-same-position" };
  state.battleRecords.push(later);
  state.battleReports.push({ id: 8, battle_id: later.id, pos: rec.pos });
  assert.equal(draw(), later.id, "dismissing an earlier battle cannot hide a later fight at the same position");
  state.battleReports = [];
  assert.equal(draw(), later.id, "exact dismissal survives the shorter report retention window");
});

let failures = 0;
for (const { name, run } of cases) {
  try { run(); console.log(`PASS: ${name}`); }
  catch (error) { failures++; console.error(`FAIL: ${name}\n${error.message.split("\n")[0]}`); }
}
if (failures) process.exitCode = 1;

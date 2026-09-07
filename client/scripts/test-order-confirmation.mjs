// Actual intent machine and desktop/mobile handlers. A first click must never
// touch the wire or locally apply a setting; one explicit Confirm sends once.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const compile = (path, deps = {}, extra = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => deps[id] ?? {}, structuredClone, ...extra });
  return exports;
};
const plain = value => JSON.parse(JSON.stringify(value));
const icons = compile("icons.ts");
const protocol = compile("protocol.ts");
let tests = 0;
const test = (name, fn) => { fn(); tests++; console.log(`PASS ${name}`); };

function fixture() {
  const ghost = (id, kind, docked = null) => ({ id, kind, own: true, owner: "1", docked,
    pos: { x: 10_000, y: 0 }, vel: { x: 100, y: 0 }, age: 60, transit: "full", posture: "passive",
    fuel: 10, supplied: true, damage: 0, composition: [{ kind, count: 2 }],
    cargo_manifest: [{ commodity: "alloys", units: 20 }] });
  const state = { playerId: "1", simTime: 60, selectedShipId: "101", selectedShipIds: new Set(["101"]),
    selectedOrderId: null, ghosts: [ghost("101", "raider"), ghost("102", "convoy", "hub"),
      ghost("103", "convoy", "home"), { ...ghost("999", "raider"), own: false }],
    pendingIntent: null, pendingOrders: new Map(), orders: { "101": { x: 20_000, y: 0 } },
    raids: { "101": "999" }, commandCenter: { x: 0, y: 0 },
    galaxy: { c: 400, hub: { x: 80_000, y: 0 }, systems: [{ id: "home", name: "Freya", pos: { x: 0, y: 0 } }] },
    systems: [{ id: "home", owner: "1", pos: { x: 0, y: 0 } }],
    battles: [{ id: "battle", own: true, participants: ["101", "999"] }], battleRecords: [],
    captains: [{ id: 1 }], operations: [{ id: "op", briefing: { title: "Guard the export" } }],
    doctrine: { engagement: "avoid", retreat: "half", escort: "hold_station", convoy: "normal" },
    emplacements: [], commandSignals: [], wallet: { credits: 10_000 } };
  const clock = { state, liveSimTime: () => state.simTime };
  const renderer = { stateVersion: 0, siteError: () => null };
  const fleets = compile("core/derive/fleet.ts", { "../../state": clock, "../../protocol": protocol,
    "./geo": { HYPERLIMIT_SU: 900 } });
  const commands = compile("core/fleetorders.ts", { "../icons": icons, "./derive/fleet": fleets });
  const readiness = compile("core/derive/readiness.ts", { "../../protocol": protocol, "./fleet": fleets });
  const orders = compile("core/derive/orders.ts", { "../../state": clock,
    "./fleet": fleets, "../fleetorders": commands, "../../icons": icons,
    "./format": { fmt: String, fmtDur: n => `${n}s` }, "./geo": {} });
  const geo = { nearestKnownDock: () => ({ id: "home", name: "Freya", pos: { x: 0, y: 0 } }),
    gravityWellAt: () => null, systemName: () => "Freya" };
  const sent = [], events = [], notices = [];
  const net = { connected: true, send: msg => sent.push(structuredClone(msg)) };
  const intent = compile("core/intent.ts", { "../state": clock, "../render": { renderer },
    "./fleetorders": commands, "./derive/fleet": fleets, "./derive/geo": geo,
    "./derive/orders": orders });
  intent.bindIntentCore(() => net, batch => events.push(...batch));
  const root = { id: "test", innerHTML: "", addEventListener() {}, querySelector: () => null };
  const ctx = { state, renderer, net, intent, send: msg => sent.push(structuredClone(msg)) };
  const deps = { "../../state": clock, "../../core/derive/fleet": fleets,
    "../../core/derive/orders": orders, "../../core/derive/readiness": readiness,
    "../../core/derive/format": { informationDelay: age => `${age}s information delay` },
    "../../core/derive/nebula": { jumpRangeAt: () => 50_000 },
    "../../core/derive/geo": geo, "../../icons": icons, "../../protocol": protocol,
    "../../core/derive/market": { kitAffordable: () => true },
    "../signature": { sheetFingerprint: JSON.stringify },
    "../dom": { renderDeferred: () => false, setHtml: (node, html) => { node.innerHTML = html; } } };
  const { DeckFleetRoutes } = compile("shell/deck/fleet.ts", deps);
  const fleet = new DeckFleetRoutes(root, ctx, { notice: n => notices.push(n), go() {} });
  fleet.render = () => true;
  const { DeckCommandStrip } = compile("shell/deck/strip.ts", deps);
  const strip = new DeckCommandStrip(root, ctx, { notice: n => notices.push(n) }, new AbortController().signal);
  const { MobileSurfaces } = compile("shell/mobile/surfaces.ts", deps);
  // Handlers don't need mounted browser DOM or a network connection.
  const mobile = Object.create(MobileSurfaces.prototype);
  mobile.ctx = ctx; mobile.hooks = { notice: n => notices.push(n) };
  mobile.sheets = { refresh() {} };
  const fleetClick = (action, fields = {}, id = "101") => fleet.handleAction({ dataset: { deckAct: `fleet-${action}`, ...fields } },
    { name: "fleet", params: { id } });
  const stripClick = (action, fields = {}) => strip.activate({ target: { closest: () => ({ dataset: { deckCommand: action, ...fields } }) } });
  const mobileClick = (action, fields = {}) => mobile.handleClick({ target: { closest: () => ({ dataset: { mobileAct: action, id: "101", ...fields } }) } });
  return { state, sent, events, notices, net, intent, commands, orders, readiness, ctx, deps,
    fleet, strip, root, fleetClick, stripClick, mobileClick };
}

test("Stealth on the fleet panel previews; Cancel changes nothing; Confirm sends exactly once", () => {
  const f = fixture();
  const before = JSON.stringify([f.state.ghosts, f.state.orders, f.state.raids]);
  f.fleetClick("transit", { mode: "stealth" });
  assert.equal(f.sent.length, 0);
  assert.equal(f.fleet.requestedTransit.size, 0, "no optimistic setting before confirmation");
  assert.match(f.orders.intentSummary(f.state.pendingIntent), /Interceptor.*Transit → Stealth.*command ~5s/);
  f.strip.render(true);
  assert.match(f.root.innerHTML, /data-deck-command="confirm"/);
  assert.match(f.root.innerHTML, /data-deck-command="cancel-intent"/);
  f.intent.clearPendingIntent();
  assert.equal(JSON.stringify([f.state.ghosts, f.state.orders, f.state.raids]), before);
  assert.equal(f.sent.length, 0);
  f.fleetClick("transit", { mode: "stealth" });
  f.intent.confirmPendingIntent(); f.intent.confirmPendingIntent();
  assert.deepEqual(plain(f.sent), [{ type: "SetFleetTransit", fleet_id: "101", mode: "stealth" }]);
  assert.equal(JSON.stringify([f.state.ghosts, f.state.orders, f.state.raids]), before,
    "sending still waits for received telemetry to change the setting");
});

test("bottom strip and mobile Stealth/Recall also wait for Confirm", () => {
  for (const click of [(f) => f.stripClick("transit", { mode: "stealth" }),
    f => f.mobileClick("fleet-transit", { mode: "stealth" }),
    f => f.stripClick("recall"), f => f.mobileClick("fleet-recall")]) {
    const f = fixture(); click(f);
    assert.equal(f.sent.length, 0);
    assert.equal(f.state.raids["101"], "999");
    assert.doesNotMatch(f.notices.join(" "), /sent|dispatch/i);
    assert.equal(f.state.pendingIntent.verb, "command");
    f.intent.confirmPendingIntent(); f.intent.confirmPendingIntent();
    assert.equal(f.sent.length, 1);
  }
});

test("fleet-panel action family never transmits on first click", () => {
  for (const [action, fields, id] of [
    ["hold"], ["recall"], ["withdraw"], ["posture", { posture: "weapons_free" }],
    ["authority", { on: "1" }], ["authority", { on: "0" }], ["fuel-rescue"],
    ["split", { kind: "raider" }], ["merge", { from: "102" }],
    ["unload", {}, "102"], ["unload", {}, "103"], ["haul-hub", {}, "103"],
  ]) {
    const f = fixture(); f.fleetClick(action, fields, id);
    assert.equal(f.sent.length, 0, action);
    assert.equal(f.state.pendingIntent.verb, "command", action);
    f.intent.confirmPendingIntent(); assert.equal(f.sent.length, 1, action);
  }
});

test("all command payload families freeze, stage and dispatch through one gate", () => {
  const payloads = [
    { type: "HubLoad", fleet_id: "102", commodity: "fuel", units: 12 },
    { type: "SystemLoad", fleet_id: "103", system: "home", commodity: "alloys", units: 25 },
    { type: "HaulToSystem", fleet_id: "102", system: "home" },
    { type: "RefitShips", fleet_id: "102", ship: "convoy", from: [], to: ["mass_driver"], n: 1 },
    { type: "BuildEmplacement", builder: "101", emplacement: "deep_space_sensor" },
    { type: "SetFleetDoctrine", doctrine: { engagement: "defensive_only", retreat: "half", escort: "hold_station", convoy: "normal" } },
    { type: "AssignCaptain", captain_id: 1, fleet_id: "101" },
    { type: "ReserveCaptain", captain_id: 1 }, { type: "TrainCaptain", captain_id: 1, attribute: "navigation" },
    { type: "RecoverOperation", fleet_id: "102", operation_id: "op" },
    [{ type: "AssignOperationFleet", fleet_id: "101", operation_id: "op", protected_fleet: "102" },
      { type: "GuardFleet", interceptor_id: "101", target_id: "102" }],
  ];
  for (const payload of payloads) {
    const f = fixture(); f.intent.beginFleetCommand(payload);
    const expected = plain(Array.isArray(payload) ? payload : [payload]);
    assert.equal(f.sent.length, 0);
    if (payload.type === "RefitShips") payload.to.push("torpedo_rack");
    if (payload.type === "SystemLoad") payload.units = 999;
    f.state.simTime += 10; f.state.ghosts[0].age += 10;
    f.intent.confirmPendingIntent();
    assert.deepEqual(plain(f.sent), expected, JSON.stringify(expected));
    f.intent.confirmPendingIntent(); assert.equal(f.sent.length, expected.length);
  }
});

test("existing setting cancels a different preview; previews replace rather than enqueue", () => {
  const f = fixture();
  f.intent.beginFleetCommand({ type: "SetFleetTransit", fleet_id: "101", mode: "stealth" });
  f.intent.beginFleetCommand({ type: "SetFleetTransit", fleet_id: "101", mode: "full" });
  assert.equal(f.state.pendingIntent, null);
  f.intent.confirmPendingIntent(); assert.equal(f.sent.length, 0);
  f.intent.beginFleetCommand({ type: "HoldFleet", ship_id: "101" });
  f.intent.beginFleetCommand({ type: "RecallRaid", raider_id: "101" });
  f.intent.confirmPendingIntent();
  assert.deepEqual(plain(f.sent), [{ type: "RecallRaid", raider_id: "101" }]);
});

test("ownership, session, berth and battle are rechecked at Confirm", () => {
  for (const [command, invalidate] of [
    [{ type: "HoldFleet", ship_id: "101" }, f => { f.state.ghosts[0].own = false; }],
    [{ type: "HoldFleet", ship_id: "101" }, f => { f.state.playerId = "2"; }],
    [{ type: "HubUnload", fleet_id: "102" }, f => { f.state.ghosts[1].docked = null; }],
    [{ type: "Withdraw", fleet_id: "101" }, f => { f.state.battleRecords = [{ id: "battle", outcome: "won" }]; }],
    [[{ type: "AssignOperationFleet", fleet_id: "101", operation_id: "op" },
      { type: "GuardFleet", interceptor_id: "101", target_id: "102" }], f => { f.state.ghosts[1].own = false; }],
  ]) {
    const f = fixture(); f.intent.beginFleetCommand(command); invalidate(f); f.intent.confirmPendingIntent();
    assert.equal(f.sent.length, 0, "invalid group cannot send a partial assignment");
    assert.equal(f.state.pendingIntent, null);
  }
  const f = fixture(); f.intent.beginFleetCommand({ type: "HoldFleet", ship_id: "999" });
  assert.equal(f.state.pendingIntent, null, "a rival cannot be ordered");
});

test("disconnect preserves the preview without silently sending on reconnect", () => {
  const f = fixture(); f.intent.beginFleetCommand({ type: "HoldFleet", ship_id: "101" });
  f.net.connected = false; f.intent.confirmPendingIntent();
  assert.equal(f.sent.length, 0); assert.ok(f.state.pendingIntent);
  f.net.connected = true; assert.equal(f.sent.length, 0);
  f.intent.confirmPendingIntent(); assert.equal(f.sent.length, 1);
});

test("cargo, Authority and rescue risks remain in the shared confirmation", () => {
  const f = fixture();
  for (const [command, pattern] of [
    [{ type: "HaulToMarketHub", fleet_id: "103", sell_on_arrival: true }, /Fuel short/],
    [{ type: "SetEngageFreight", fleet_id: "101", on: true }, /citations/],
    [{ type: "RequestFuelRescue", fleet_id: "101" }, /3× market Fuel/],
  ]) {
    f.intent.beginFleetCommand(command);
    assert.match(f.readiness.intentReadinessWarnings(f.state, f.state.pendingIntent).join(" "), pattern);
    assert.equal(f.sent.length, 0);
  }
});

test("map Move retains its single confirmation and matching command payload", () => {
  const f = fixture();
  f.intent.beginPendingIntent({ verb: "move", shipId: "101", dest: { x: 25_000, y: 0 } });
  assert.equal(f.sent.length, 0);
  f.intent.confirmPendingIntent(); f.intent.confirmPendingIntent();
  assert.deepEqual(plain(f.sent), [{ type: "MoveShip", ship_id: "101", dest: { x: 25_000, y: 0 } }]);
});

test("no shell control bypasses confirmation for a fleet-command literal", () => {
  const parse = (path, text) => ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true);
  const walk = (node, visit) => { visit(node); ts.forEachChild(node, child => walk(child, visit)); };
  const commandTypes = new Set();
  const declarations = parse("fleetorders.ts", source("core/fleetorders.ts"));
  for (const node of declarations.statements) if (ts.isTypeAliasDeclaration(node) && node.name.text === "FleetCommand") {
    walk(node, child => { if (ts.isStringLiteral(child)) commandTypes.add(child.text); });
  }
  assert.ok(commandTypes.has("SetFleetTransit") && commandTypes.has("HubUnload"));
  const dir = new URL("../src/shell/", import.meta.url);
  for (const path of readdirSync(dir, { recursive: true }).filter(p => p.endsWith(".ts"))) {
    // The live viewer's shared prompt already has its own explicit Confirm and
    // recipient revalidation, covered by test:live-battle. Do not double-prompt.
    if (path === "battlewithdraw.ts") continue;
    const ast = parse(path, readFileSync(new URL(path, dir), "utf8"));
    walk(ast, node => {
      if (!ts.isCallExpression(node) || !ts.isPropertyAccessExpression(node.expression)
        || node.expression.name.text !== "send") return;
      const payload = node.arguments[0];
      if (!payload || !ts.isObjectLiteralExpression(payload)) return;
      const type = payload.properties.find(p => ts.isPropertyAssignment(p) && p.name.getText(ast) === "type");
      if (type && ts.isStringLiteral(type.initializer)) assert.ok(!commandTypes.has(type.initializer.text),
        `${path}: ${type.initializer.text} bypasses the order confirmation`);
    });
  }
});

console.log(`${tests} order-confirmation regression groups passed.`);

// §claims (protocol v90): the Authority's claims on new colonies, the price
// as served, and the Colony Ship's warning when it sails without one. Pure
// derive modules against fixtures; it never connects to a game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
function compile(path, deps = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {} });
  return exports;
}
const copy = (value) => JSON.parse(JSON.stringify(value));

const claims = compile("core/derive/claims.ts");
const system = { id: "s9", name: "Vega", pos: { x: 10_000, y: 0 }, claim_cost: 800 };
const charter = { claims: { held: [], mult: 1.6, at_war: false, closed: false } };
const offer = (s = system, dynamic = { id: "s9", owner: null }, extra = {}) =>
  copy(claims.claimOffer({ charter, season: null, ...extra }, s, dynamic));
assert.deepEqual(offer(), { kind: "price", cost: 1_280 }, "claim cost × the served multiplier");
assert.equal(offer(system, { id: "s9", owner: "7" }), null, "an owned system: no claim");
assert.equal(offer({ ...system, reserved: true }), null, "a home site held for a new charter");
assert.equal(offer(system, undefined, { season: { precursors: [{ system: "s9" }] } }), null, "a sealed world");
assert.equal(claims.claimMissing({ charter }, system, undefined), true);
charter.claims.held.push({ system: "s9", lapses_at: 900 });
assert.deepEqual(offer(), { kind: "held", lapsesAt: 900 });
assert.equal(claims.claimMissing({ charter }, system, undefined), false);
charter.claims.held = [];
charter.claims.closed = true;
assert.deepEqual(offer(), { kind: "closed" });
assert.equal(claims.claimMissing({ charter }, system, undefined), true, "closed: the ship would hold");
charter.claims.at_war = true;
assert.deepEqual(offer(), { kind: "war" }, "at war: none needed");
assert.equal(claims.claimMissing({ charter }, system, undefined), false);
charter.claims.at_war = false; charter.claims.closed = false;
assert.deepEqual(copy(claims.claimsHeld({ claims: { held: [{ system: "a", lapses_at: 9 }, { system: "b", lapses_at: 3 }] } })
  .map((c) => c.system)), ["b", "a"], "soonest lapse first");

// A Colony Ship sent without its claim is warned; with it, or anything else, not.
const fleet = { fleetCargoCapacity: () => 0, fleetHoldUsed: () => 0, ROOM_SCALE: 4, shipKindLabel: (k) => k,
  regimeStore: () => ({ served: 100, capacity: 100, fuel: "deuterium" }), estimatedFuelForLeg: () => 0,
  REACTOR_ENERGY: { fusion: 1, fission: 1 }, gradeBurning: () => "fusion", jumpAntimatter: () => 0 };
const readiness = compile("core/derive/readiness.ts", { "./claims": claims, "./fleet": fleet });
const colony = { id: "c1", own: true, kind: "colony", pos: { x: 0, y: 0 }, fuel: 100 };
const scout = { ...colony, id: "s1", kind: "scout" };
const view = { ghosts: [colony, scout], emplacements: [], systems: [{ id: "s9", owner: null }], charter,
  galaxy: { hub: { x: 70_000, y: 0 }, hyperlimit: 900, systems: [system] } };
const move = (id, dest) => readiness.intentReadinessWarnings(view, { verb: "move", shipId: id, dest });
assert.ok(move("c1", { x: 10_200, y: 0 }).includes(claims.NO_CLAIM_WARNING), "inside the well, no claim held");
assert.ok(!move("c1", { x: 20_000, y: 0 }).includes(claims.NO_CLAIM_WARNING), "open space");
assert.ok(!move("s1", system.pos).includes(claims.NO_CLAIM_WARNING), "a Scout needs no claim");
const order = readiness.intentReadinessWarnings(view, { verb: "command", shipId: "c1",
  commands: [{ type: "MoveShip", ship_id: "c1", dest: system.pos }] });
assert.ok(order.includes(claims.NO_CLAIM_WARNING), "a staged MoveShip too");
charter.claims.held.push({ system: "s9", lapses_at: 900 });
assert.ok(!move("c1", system.pos).includes(claims.NO_CLAIM_WARNING), "held: no warning");

// ── Both shells: Buy claim on an unowned system, and the Faction list ───────
assert.match(src("shell/deck/empire.ts"), /this\.claimHtml\(system, dynamic\)/);
assert.match(src("shell/mobile/parity.ts"), /this\.claimHtml\(systemId, dynamic\)/);

console.log("Claims: price as served, held, at war, closed, reserved and sealed worlds, and the Colony Ship warning pass.");

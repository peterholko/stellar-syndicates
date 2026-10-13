// Battle effect geometry: hull hardpoints (baked from the shipped GLBs), engine
// thrust from recorded tracks, seeded flicker and damage vents. Pure functions,
// so these run without a browser or GPU.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import { extractHardpoints, renderHardpoints } from "./build-ship-hardpoints.mjs";

const source = name => readFileSync(new URL(`../src/${name}.ts`, import.meta.url), "utf8");
function compile(name, deps = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => { assert.ok(id in deps, `unexpected dependency ${id}`); return deps[id]; } });
  return exports;
}
const plain = v => JSON.parse(JSON.stringify(v));
const prng = compile("prng");
const hardpoints = compile("ship-hardpoints.generated");
const fx = compile("battlefx", { "./prng": prng, "./ship-hardpoints.generated": hardpoints });
const catalog = compile("ship-models.generated");
const kinds = Object.keys(catalog.SHIP_MODELS);

// The baked table is exactly what the shipped hulls say (re-run
// `npm run assets:hardpoints` after replacing a model).
assert.equal(renderHardpoints(await extractHardpoints()), source("ship-hardpoints.generated"),
  "ship-hardpoints.generated.ts matches the shipped LOD2 hulls");
assert.deepEqual(Object.keys(hardpoints.SHIP_HARDPOINTS), kinds, "one entry per shipped hull");
assert.equal(hardpoints.SHIP_HARDPOINTS.mining_barge.engines.length, 2, "Barge has two authored exhausts");
assert.equal(hardpoints.SHIP_HARDPOINTS.mining_barge.guns.length, 0, "Mining cutters are not weapon hardpoints");

assert.equal(hardpoints.SHIP_HARDPOINTS.mining_belt_station.engines.length, 0, "Station has no main engine sockets");
assert.equal(hardpoints.SHIP_HARDPOINTS.mining_belt_station.guns.length, 0, "Station has no weapons");
const ARMED = ["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
for (const kind of kinds) {
  const h = hardpoints.SHIP_HARDPOINTS[kind];
  assert.equal(h.size[2], 2, `${kind}: nose-to-tail length is normalized to 2`);
  if (kind !== "mining_belt_station") assert.ok(h.engines.length >= 2, `${kind}: has engines`);
  const plumes = fx.enginePlumes(kind, 224); // k = 100 px per model unit
  if (kind === "mining_belt_station") assert.equal(plumes.length, 0, "Station generates no drive plumes");
  else assert.ok(plumes.length >= 1 && plumes.length <= h.engines.length);
  for (const p of plumes) {
    assert.ok(p.y > 40 && p.y < 110, `${kind}: plumes sit at the stern`);
    assert.ok(Math.abs(p.x) <= (h.size[0] / 2) * 100 + 1e-9, `${kind}: plumes lie within the hull's width`);
    assert.ok(p.r >= 3.5 - 1e-9 && p.r <= 9 * 1.75 + 1e-9, `${kind}: nozzle size is bounded by the hull`);
  }
  const guns = fx.gunPoints(kind, 224);
  if (ARMED.includes(kind)) assert.ok(guns.length >= 2 && guns.length === h.guns.length, `${kind}: fires from its barrels`);
  else assert.deepEqual(plain(guns), [[0, -80]], `${kind}: unarmed hull falls back to its nose`);
  const [hw, hl] = fx.hullHalfExtents(kind, 224);
  assert.ok(Math.abs(hl - 100) < 1e-9 && Math.abs(hw - h.size[0] * 50) < 1e-9);
}
// Nozzles stacked above one another share a plume (the battleship's 2×3 bank).
assert.equal(hardpoints.SHIP_HARDPOINTS.battleship.engines.length, 6);
assert.equal(fx.enginePlumes("battleship", 224).length, 3);
// Plumes scale with the drawn hull.
const small = fx.enginePlumes("cruiser", 112), large = fx.enginePlumes("cruiser", 224);
small.forEach((p, i) => assert.ok(Math.abs(large[i].x - 2 * p.x) < 1e-9 && Math.abs(large[i].r - 2 * p.r) < 1e-9));

// MAX_SPEED mirrors the sim's tactical rules exactly.
const rules = readFileSync(new URL("../../crates/sim/src/tactical/rules_v1.rs", import.meta.url), "utf8");
const body = rules.slice(rules.indexOf("pub fn max_speed"), rules.indexOf("pub fn attack_weight"));
const simSpeeds = {};
for (const [, names, speed] of body.matchAll(/((?:ShipKind::\w+\s*\|?\s*)+)=>\s*([\d.]+)/g)) {
  for (const [, name] of names.matchAll(/ShipKind::(\w+)/g)) {
    simSpeeds[name.replace(/[A-Z]/g, (c, i) => (i ? "_" : "") + c.toLowerCase())] = Number(speed);
  }
}
assert.deepEqual(plain(fx.MAX_SPEED), simSpeeds, "MAX_SPEED mirrors crates/sim tactical max_speed");
const tactical = readFileSync(new URL("../../crates/sim/src/tactical/v1.rs", import.meta.url), "utf8");
const simConst = (name) => Number(tactical.match(new RegExp(`pub const ${name}: f64 = ([\\d.]+);`))[1]);
assert.equal(fx.TORP_SPEED, simConst("TORP_SPEED"), "TORP_SPEED mirrors the sim");
// Hulls heavier than a destroyer turn at the engine's own limits (rules v9).
const v9 = readFileSync(new URL("../../crates/sim/src/tactical/v9.rs", import.meta.url), "utf8");
const turnBody = v9.slice(v9.indexOf("pub fn turn_rate"), v9.indexOf("pub(super) fn turn_capped"));
const simTurns = {};
for (const [, names, rate] of turnBody.matchAll(/((?:ShipKind::\w+\s*\|?\s*)+)=>\s*([\d.]+)/g)) {
  for (const [, name] of names.matchAll(/ShipKind::(\w+)/g)) {
    simTurns[name.replace(/[A-Z]/g, (c, i) => (i ? "_" : "") + c.toLowerCase())] = Number(rate);
  }
}
assert.ok(Object.keys(simTurns).length >= 11);
for (const [kind, rate] of Object.entries(simTurns)) assert.equal(fx.TURN_RATE[kind], rate, `${kind} turns at the engine's rate`);
for (const kind of ["raider", "corvette", "destroyer"]) assert.ok(!(kind in simTurns) && fx.TURN_RATE[kind] >= 1, `${kind} still turns freely`);
assert.equal(fx.PD_RADIUS, simConst("PD_RADIUS"), "PD_RADIUS mirrors the sim");
assert.equal(fx.PD_RADIUS_HEAVY, simConst("PD_RADIUS_HEAVY"), "PD_RADIUS_HEAVY mirrors the sim");

// Thrust: from the hull's speed and its acceleration along the heading on its
// curved track; bounded, flat out on withdrawal, and a pure function.
const thrust = fx.engineThrust;
for (const speed of [0, 10, 45, 200]) for (const burn of [-30, 0, 15, 90]) {
  const t = thrust(speed, burn, 45, false);
  assert.ok(t >= 0.1 && t <= 1, "thrust stays in [0.1, 1]");
  assert.equal(thrust(speed, burn, 45, true), 1, "a withdrawing side burns flat out");
}
assert.ok(thrust(0, 0, 45, false) < thrust(20, 0, 45, false), "faster ships burn harder");
assert.ok(thrust(20, 15, 45, false) > thrust(20, 0, 45, false), "accelerating shows a burn");
assert.equal(thrust(20, -15, 45, false), thrust(20, 0, 45, false), "braking never brightens the main engines");
assert.ok(thrust(20, 0, 23, false) > thrust(20, 0, 100, false), "the same speed is a harder burn for a slow hull");
assert.equal(thrust(12, 7, 45, false), thrust(12, 7, 45, false));

// Flicker noise: seeded, smooth and in range.
for (let t = 0; t < 20; t += 0.173) {
  const n = fx.noise1(42, t);
  assert.ok(n >= 0 && n < 1);
  assert.ok(Math.abs(fx.noise1(42, t + 0.001) - n) < 0.01, "flicker changes smoothly with replay time");
  assert.equal(fx.noise1(42, t), n);
}
assert.notEqual(fx.noise1(1, 3.5), fx.noise1(2, 3.5));

// Damage vents: stable per hull, inside its footprint; bands follow hull.
const vents = fx.damageVents(1234, "cruiser", 224, false);
assert.deepEqual(plain(vents), plain(fx.damageVents(1234, "cruiser", 224, false)), "vents are stable for a hull");
assert.notDeepEqual(plain(vents), plain(fx.damageVents(99, "cruiser", 224, false)));
const [hw, hl] = fx.hullHalfExtents("cruiser", 224);
for (const [x, y] of vents) assert.ok(Math.abs(x) <= hw && Math.abs(y) <= hl, "vents lie on the hull");
assert.deepEqual([1, 0.6, 0.5, 0.4, 0.3, 0.1, 0].map(fx.smokeVents), [0, 0, 1, 1, 2, 2, 0]);
assert.deepEqual([1, 0.3, 0.29, 0.01, 0].map(fx.hullOnFire), [false, false, true, true, false]);

console.log(`Battle FX: baked hardpoints match all ${kinds.length} game kinds; plumes, guns, thrust, flicker and damage vents passed.`);

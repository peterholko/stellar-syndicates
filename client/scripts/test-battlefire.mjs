import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";

const source = readFileSync(new URL("../src/battlefire.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText;
const { arrivedGunfire, matchBattleFrames, missedEndpoint } = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);
const tracks = [
  { cid: 1, side: 0, kind: "raider", plat: false, hpStart: 1, hpEnd: .98 },
  { cid: 2, side: 1, kind: "raider", plat: false, hpStart: .5, hpEnd: .17 },
];
const record = (gunfire, damage = [66, 4]) => ({ sides: [{ loadouts: [] }, { loadouts: [] }], rounds: [
  { dealt: [0, 0], frame: { gunfire: [] } },
  { dealt: damage, frame: { gunfire } },
] });
const hit = { side: 0, from: 1, to: 2, weapon: "beam", damage: 66 };
const miss = { side: 1, from: 2, to: 1, weapon: "beam", damage: 0 };

// Next frame is the evidence for this window, not the previous round's damage.
assert.deepEqual(arrivedGunfire(record([hit, miss]), 0, tracks), [
  { from: 0, to: 1, weapon: "beam", damage: 66 },
  { from: 1, to: 0, weapon: "beam", damage: 0 },
]);
assert.deepEqual(arrivedGunfire(record([]), 0, tracks), [], "empty evidence is a cooldown, never a cosmetic volley");
assert.deepEqual(arrivedGunfire(record([miss], [0, 0]), 0, tracks).map((s) => s.damage), [0], "a miss may fly but never impact");
assert.deepEqual(arrivedGunfire(record([hit]), 1, tracks), [], "no next arrived frame, no future fire");
const future = record([hit]);
future.rounds.push({ dealt: [999, 999], frame: { gunfire: [hit, hit, hit] } });
assert.deepEqual(arrivedGunfire(future, 0, tracks), arrivedGunfire(record([hit]), 0, tracks), "later reports never rescale a past volley");
assert.deepEqual(arrivedGunfire(record([{ ...hit, to: 999 }]), 0, tracks), [], "unshown combatant must not hit another hull's sprite");

// Legacy frames: recorded loss only, not fabricated fire or harm to civilians.
assert.equal(arrivedGunfire(record(undefined), 0, tracks).length, 2);
assert.deepEqual(arrivedGunfire(record(undefined, [0, 0]), 0, tracks), []);
assert.deepEqual(arrivedGunfire(record(undefined), 0, tracks.map((s) => ({ ...s, hpEnd: s.hpStart }))), []);
assert.deepEqual(arrivedGunfire(record(undefined, [66, 0]), 0, [{ ...tracks[0], kind: "convoy" }, tracks[1]]), []);

const frameShip = (cid, x, hp) => ({ cid, side: 0, kind: "raider", x, y: 0, hp });
const crossing = matchBattleFrames([frameShip(1, 0, 1), frameShip(2, 100, .5)], [frameShip(1, 100, .9), frameShip(2, 0, .5)]);
assert.deepEqual(crossing.map((t) => [t.from.cid, t.to.cid]), [[1, 1], [2, 2]], "crossing ships retain identity and damage");
const replaced = matchBattleFrames([frameShip(1, 0, 1)], [frameShip(3, 0, 1)]);
assert.equal(replaced[0].to, null, "a same-position reinforcement cannot inherit the dead ship's identity");
assert.equal(replaced[1].from, null);
const legacy = matchBattleFrames([frameShip(undefined, 0, 1)], [frameShip(undefined, 10, .9)]);
assert.equal(legacy[0].to.x, 10);
for (const [tx, ty] of [[100, 0], [0, 100], [-100, -100]]) {
  for (const sign of [-1, 1]) {
    const [x, y] = missedEndpoint(0, 0, tx, ty, 40, sign);
    assert.ok(Math.abs(Math.hypot(x - tx, y - ty) - 40) < 1e-9);
    assert.ok(Math.abs((x - tx) * tx + (y - ty) * ty) < 1e-9, "miss offset is perpendicular at every bearing");
  }
}
console.log("Battle gunfire: shot evidence, cooldowns, misses, arrival window, legacy fallback, stable identity and miss geometry passed.");

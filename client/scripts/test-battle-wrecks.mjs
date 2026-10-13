// What happens to a hull after its recorded death: break-up tables, staging,
// section motion, debris, cooling embers and shock fronts. Pure functions of
// the death and the replay time, so these run without a browser or GPU.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

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
const w = compile("battlewrecks", { "./prng": prng });
const kinds = Object.keys(compile("ship-models.generated").SHIP_MODELS);

// Every hull type breaks somewhere; the lightest shatter, a titan in five.
assert.deepEqual(Object.keys(w.WRECK_PIECES).sort(), [...kinds].sort(), "one break-up rule per hull type");
assert.equal(w.WRECK_PIECES.raider, 0);
assert.equal(w.WRECK_PIECES.titan, 5);
for (const kind of kinds) {
  const cuts = w.wreckCuts(kind), pieces = w.WRECK_PIECES[kind];
  assert.equal(cuts.length, Math.max(0, pieces - 1), `${kind}: one cut between each pair of sections`);
  assert.ok(cuts.every((c, i) => c > 0 && c < 1 && (i === 0 || c > cuts[i - 1])), `${kind}: cuts run nose to tail`);
  assert.deepEqual(plain(w.wreckCuts(kind)), plain(cuts), `${kind}: cuts are fixed per hull type`);
}

// Large hulls (8000+ mass) leave recorded wreckage in rules v9; the engine
// breaks them into these same sections (crates/sim/src/tactical/v9.rs).
assert.deepEqual([w.WRECK_PIECES.large_freighter, w.WRECK_PIECES.battleship, w.WRECK_PIECES.heavy_freighter,
  w.WRECK_PIECES.dreadnought, w.WRECK_PIECES.bulk_freighter, w.WRECK_PIECES.titan], [3, 3, 4, 4, 4, 5]);

// Staging: capitals burn before they break; a titan's death is the longest.
assert.equal(w.blastClass("titan", 32000), 3);
assert.equal(w.blastClass("battleship", 8000), 2);
assert.equal(w.blastClass("cruiser", 4000), 1);
assert.equal(w.blastClass("raider", 200), 0);
assert.deepEqual([0, 1, 2, 3].map(w.breachDelay), [0, 0, 0.8, 1.8]);

// Drift slows and stops; char darkens and settles.
assert.equal(w.drift(0, 0.5), 0);
for (let t = 0.1; t < 20; t += 0.7) {
  assert.ok(w.drift(t, 0.5) > w.drift(t - 0.1, 0.5), "debris keeps moving forward");
  assert.ok(w.drift(t, 0.5) < 1 / 0.5, "…but never past where drag stops it");
}
assert.equal(w.charOf(-0.1, 0.8), 0);
assert.ok(w.charOf(0.1, 0.8) > 0.15 && w.charOf(0.7, 0.8) < 0.5 + 1e-9, "a burning capital darkens before it breaks");
assert.ok(w.charOf(30, 0.8) <= 0.8 + 1e-9 && w.charOf(30, 0) <= 0.8 + 1e-9, "a wreck burns out to a dark hulk");

// Sections: the bow flies forward, the stern aft; capitals tumble slowly.
for (const kind of ["corvette", "cruiser", "titan"]) {
  const n = w.WRECK_PIECES[kind], cuts = w.wreckCuts(kind), cls = kind === "titan" ? 3 : 1;
  const motions = Array.from({ length: n }, (_, i) => w.pieceMotion(77, i, n, cuts, cls));
  assert.deepEqual(plain(motions), plain(Array.from({ length: n }, (_, i) => w.pieceMotion(77, i, n, cuts, cls))), "seeded");
  assert.ok(motions[0].dz < 0, `${kind}: the bow section separates forward`);
  assert.ok(motions[n - 1].dz > 0, `${kind}: the stern section separates aft`);
}
const heavy = w.pieceMotion(5, 0, 5, w.wreckCuts("titan"), 3), light = w.pieceMotion(5, 0, 2, w.wreckCuts("corvette"), 0);
assert.ok(Math.abs(heavy.roll) <= 0.175 && Math.abs(light.roll) <= 1.1, "capital sections tumble slowly");

// Debris scales with the hull; embers cool from white-hot to dark.
assert.equal(w.fragments(9, 200).length, w.fragmentCount(200));
assert.ok(w.fragmentCount(200) >= 6 && w.fragmentCount(32000) <= 40 && w.fragmentCount(32000) > w.fragmentCount(800));
assert.deepEqual(plain(w.fragments(9, 4000)), plain(w.fragments(9, 4000)), "debris is seeded");
assert.ok(new Set(w.fragments(9, 32000).map(f => f.type)).size === 3, "plates, struts and embers");
let glow = 2;
for (let age = 0; age <= 3.4; age += 0.2) {
  const e = w.emberColor(age);
  assert.ok(e.glow <= glow + 1e-9, "an ember only cools");
  glow = e.glow;
}
assert.equal(w.emberColor(3.3).glow, 0, "a cold ember no longer glows");

// Debris is cut from the hull's own art at the hull's own scale: light hulls
// break into a few big chunks, capitals shed small plates.
assert.ok(w.debrisCutFraction(200, 10) > w.debrisCutFraction(4000, 40), "an interceptor breaks into bigger pieces of itself");
assert.ok(w.debrisCutFraction(4000, 40) > w.debrisCutFraction(32000, 80), "a titan sheds smaller plates of itself");
for (const [mass, px] of [[1, 1], [200, 10], [32000, 80], [1e6, 400]]) {
  const f = w.debrisCutFraction(mass, px);
  assert.ok(f >= 0.06 && f <= 0.5, "every cut is a sensible piece of the art");
}
assert.equal(w.fragmentScale(4000), 0.8 + Math.pow(2, 0.3));
// A plain hull silhouette: an upright ellipse in a tall image.
const hullShape = (u, v) => (((u - 0.5) / 0.36) ** 2 + ((v - 0.5) / 0.46) ** 2 <= 1 ? 1 : 0);
const cuts = w.debrisCuts(4242, hullShape, 1.6, 0.16);
assert.equal(cuts.length, 8, "six torn plates and two spars");
assert.deepEqual(plain(w.debrisCuts(4242, hullShape, 1.6, 0.16)), plain(cuts), "a hull type always tears the same way");
assert.notDeepEqual(plain(w.debrisCuts(4243, hullShape, 1.6, 0.16)), plain(cuts), "another hull tears differently");
assert.equal(cuts.filter((c) => c.sliver).length, 2);
assert.ok(cuts.every((c) => hullShape(c.u, c.v) === 1), "every piece is centred on solid hull");
for (let i = 0; i < cuts.length; i++) for (let j = 0; j < i; j++) {
  assert.ok(Math.hypot(cuts[i].u - cuts[j].u, (cuts[i].v - cuts[j].v) * 1.6) > 0.04, "pieces come from different parts of the hull");
}
const extent = (c) => {
  const xs = c.outline.map((p) => p[0]), ys = c.outline.map((p) => p[1]);
  return [Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)];
};
assert.ok(cuts.every((c) => c.outline.length >= 6 && c.outline.every(([x, y]) => Math.abs(x) <= 1 && Math.abs(y) <= 1)), "outlines fit their cell");
assert.ok(cuts.filter((c) => c.sliver).every((c) => extent(c)[0] / extent(c)[1] >= 2.5), "spars are long and thin");
assert.ok(cuts.filter((c) => !c.sliver).every((c) => { const [x, y] = extent(c); return x / y > 0.5 && x / y < 2; }), "plates are not");
assert.ok(cuts.filter((c) => c.sliver).every((c) => Math.abs(Math.cos(c.rot)) < 0.31), "spars run along the hull");
assert.equal(w.debrisCuts(1, () => 0, 1, 0.2).length, 8, "an unreadable silhouette still yields pieces");
assert.equal(w.mix(0x000000, 0xffffff, 0.5), 0x808080);

// Shock fronts: nothing until the front arrives, a push after it passes,
// nothing beyond its reach; a titan's reaches furthest.
assert.equal(w.shockRadius(2, 0), 0);
assert.ok(w.shockRadius(3, 1) > w.shockRadius(2, 1) && w.shockRadius(2, 0.5) > w.shockRadius(2, 0.1));
assert.equal(w.shockPush(2, 0.02, 150), 0, "the front has not reached it yet");
assert.ok(w.shockPush(2, 2, 60) > 0, "a passed front has pushed it outward");
assert.equal(w.shockPush(2, 2, 500), 0, "beyond the front's reach nothing moves");
assert.ok(w.shockPush(2, 2, 40) >= w.shockPush(2, 0.3, 40), "the push builds, then holds");

console.log("Battle wrecks: break-up tables, staging, drift, char, sections, debris, art-cut debris, embers and shock fronts passed.");

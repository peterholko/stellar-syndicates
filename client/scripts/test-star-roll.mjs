// §star-roll (Peter, Nov 9 2026): a star turns about its north–south axis,
// never "as a clock". The real src/starroll.ts against a recording Pixi mock:
// the turn, the measured discs, the pulsar's swing and the shader's inputs.
// The look itself: the Vite dev server's /scripts/star-roll-fixture.html.
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = (name) => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (name, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (id) => deps[id] ?? {} });
  return exports;
};
const close = (a, b, label, eps = 1e-9) => assert.ok(Math.abs(a - b) < eps, `${label}: ${a} != ${b}`);

const point = () => ({ x: 0, y: 0, set(x, y = x) { this.x = x; this.y = y; } });
class Container {
  children = []; position = point(); scale = point(); visible = true; alpha = 1; rotation = 0;
  addChild(...children) { for (const child of children) { child.parent = this; this.children.push(child); } return children[0]; }
  destroy(options) { this.destroyed = options ?? true; for (const child of this.children) child.destroy?.(); }
}
class MeshGeometry { constructor(options) { Object.assign(this, options); } destroy() { this.destroyed = true; } }
class Mesh extends Container { constructor(options) { super(); Object.assign(this, options); } }
class Shader { constructor(options) { this.glProgram = options.glProgram; this.resources = { ...options.resources }; } destroy() { this.destroyed = true; } }
class UniformGroup { constructor(structure) { this.uniforms = Object.fromEntries(Object.entries(structure).map(([k, v]) => [k, v.value])); } }
const pixi = { Container, Mesh, MeshGeometry, Shader, UniformGroup, GlProgram: { from: (options) => options } };

const roll = load("starroll.ts", { "pixi.js": pixi });
const prng = load("prng.ts");
const stars = load("stars.ts", { "./prng": prng });
const art = load("starart.ts", { "./star-art.generated": load("star-art.generated.ts") });

// Every star type has a surface; only a black hole never turns.
for (const { slug } of stars.STAR_TYPES) {
  const surface = roll.STAR_SURFACES[slug];
  assert.ok(surface, `${slug} has a surface`);
  assert.equal(surface.period > 0, slug !== "black_hole", `${slug}: turns unless a black hole`);
  assert.equal(surface.discs.length, { binary_star: 2, black_hole: 0 }[slug] ?? 1, `${slug}: its discs`);
  // The measured photosphere lies inside the painting's visible footprint.
  const geometry = art.starArtwork("system", slug);
  for (const disc of surface.discs) {
    const reach = Math.hypot(disc.cx - geometry.anchor[0], disc.cy - geometry.anchor[1]) + disc.r;
    assert.ok(reach <= geometry.visualRatio / 2 + 0.01, `${slug}: disc inside the visible art`);
    if (surface.discs.length === 1 && slug !== "neutron_star") {
      assert.ok(disc.r > 0.2, `${slug}: the disc, not a hot spot`);
      assert.ok(Math.hypot(disc.cx - geometry.anchor[0], disc.cy - geometry.anchor[1]) < 0.03, `${slug}: centred on its art`);
    }
  }
}
assert.equal(roll.STAR_SURFACES.neutron_star.beam !== undefined, true, "a pulsar's beams swing");
assert.equal(Object.values(roll.STAR_SURFACES).filter((s) => s.beam !== undefined).length, 1, "only a pulsar's");
const [primary, companion] = roll.STAR_SURFACES.binary_star.discs;
assert.ok(companion.r < primary.r && companion.rate > primary.rate, "the binary's smaller star turns quicker");
assert.ok(Math.hypot(primary.cx - companion.cx, primary.cy - companion.cy) >= primary.r + companion.r, "the pair's discs do not overlap");

// The turn: a pure function of wall time, wrapping once per period.
const yellow = roll.starPeriod("yellow_star");
close(roll.starRoll("yellow_star", 0), 0, "rest at time zero");
close(roll.starRoll("yellow_star", yellow / 4), Math.PI / 2, "a quarter turn");
close(roll.starRoll("yellow_star", yellow * 3 + yellow / 2), Math.PI, "whole turns drop out", 1e-9);
close(roll.starRoll("yellow_star", 86_400 + yellow / 4), roll.starRoll("yellow_star", 86_400) + Math.PI / 2, "still exact after a day", 1e-6);
for (const t of [0.5, 13, 299.9, 1e5]) {
  const a = roll.starRoll("yellow_star", t);
  assert.ok(a >= 0 && a < Math.PI * 2, "within one turn");
}
close(roll.starRoll("binary_star", 140 / 8, 1.6), (1.6 / 8) * Math.PI * 2, "a disc's own rate");
assert.equal(roll.starRoll("black_hole", 123), 0, "a black hole never turns");
assert.equal(roll.starRoll("yellow_star", Number.NaN), 0, "a missing clock rests");
assert.equal(roll.starPeriod("no_such_star"), 0, "unknown art stays still");

// The pulsar: its painted pose at rest, a quarter turn on facing the viewer
// (straight up and down, shortest), mirrored at half a turn.
const beam = roll.STAR_SURFACES.neutron_star.beam;
const rest = roll.pulsarBeam(beam, 0);
close(rest.axis[0], rest.painted[0], "at rest, the painted axis");
close(rest.axis[1], rest.painted[1], "at rest, the painted axis (y)");
close(rest.length, 1, "full length at rest");
const quarter = roll.pulsarBeam(beam, Math.PI / 2);
close(quarter.axis[0], 0, "a quarter turn: vertical", 1e-12);
close(quarter.axis[1], -1, "a quarter turn: vertical (up)");
close(quarter.length, Math.sin((beam * Math.PI) / 180), "foreshortened toward the viewer");
const half = roll.pulsarBeam(beam, Math.PI);
close(half.axis[0], -rest.axis[0], "half a turn: mirrored");
close(half.axis[1], rest.axis[1], "half a turn: same height");

// The display object: placed like a Sprite, dressed per star, turned by time.
const texture = (size) => ({ width: size, height: size, source: { size } });
const star = new roll.RollingStar(texture(1254), "yellow_star");
const shaderOf = (s) => s.children[0].shader;
const uniforms = (s) => shaderOf(s).resources.starRoll.uniforms;
assert.equal(shaderOf(star).resources.uTexture.size, 1254, "samples the painting itself");
const disc = roll.STAR_SURFACES.yellow_star.discs[0];
assert.deepEqual([...uniforms(star).uDiscA].map((v) => +v.toFixed(4)), [disc.cx, disc.cy, disc.r, 1]);
assert.equal(uniforms(star).uDiscB[3], 0, "one disc");
assert.equal(uniforms(star).uBeamLen[1], 0, "no beams");
star.anchor.set(0.25, 0.75);
assert.deepEqual([...star.children[0].geometry.positions], [-313.5, -940.5, 940.5, -940.5, 940.5, 313.5, -313.5, 313.5],
  "the quad covers the texture about its anchor, in texture pixels");
star.roll(yellow / 4);
close(star.turn, Math.PI / 2, "turned by wall time");
close(uniforms(star).uRoll[0], Math.PI / 2, "the shader's turn", 1e-6);
assert.equal(star.rotation, 0, "never a clock spin");
star.texture = texture(512);
assert.equal(shaderOf(star).resources.uTexture.size, 512, "a sharper or smaller level swaps in");
assert.equal(star.children[0].geometry.positions[2] - star.children[0].geometry.positions[0], 512, "the quad follows it");
close(uniforms(star).uMeanBias, Math.log2(512 / 48), "the local mean stays the same share of the painting");
star.setStar("binary_star");
close(star.turn, roll.starRoll("binary_star", yellow / 4), "a new star keeps the clock");
assert.equal(uniforms(star).uDiscB[3], 1, "the binary's second star");
close(uniforms(star).uRoll[1], roll.starRoll("binary_star", yellow / 4, companion.rate), "at its own rate", 1e-6);
star.setStar("neutron_star");
assert.equal(uniforms(star).uBeamLen[1], 1, "a pulsar's beams swing");
star.roll(roll.starPeriod("neutron_star") / 4);
close(uniforms(star).uBeam[2], 0, "beams vertical at a quarter turn", 1e-6);
close(uniforms(star).uBeamLen[0], quarter.length, "and foreshortened", 1e-6);
for (const still of ["black_hole", null]) {
  star.setStar(still);
  star.roll(77);
  assert.equal(uniforms(star).uDiscA[3], 0, `${still}: nothing turns`);
  assert.equal(uniforms(star).uBeamLen[1], 0, `${still}: no beams`);
  assert.equal(star.turn, 0);
}
star.destroy();
assert.ok(star.children[0].geometry.destroyed && shaderOf(star).destroyed, "its own geometry and shader go with it");

// §star-flares (Peter, Nov 9 2026: "animate a bit of the solar flares from
// the larger stars"): the painted flares lift, sway and pulse on slow waves,
// and now and then one stretch of limb erupts. All a function of wall time.
for (const { slug } of stars.STAR_TYPES) {
  const flare = roll.STAR_SURFACES[slug].flare;
  assert.equal(!!flare, !["neutron_star", "black_hole"].includes(slug), `${slug}: flares unless beams or a black hole`);
  if (flare) assert.ok(flare.strength > 0 && flare.strength <= 1.1 && flare.reach > 1.1 && flare.reach <= 2 && flare.every > 8.2,
    `${slug}: a moving band beyond the limb, eruptions spaced past their ${8.2} s`);
}
{
  const { rise, hold, fall, lift } = roll.ERUPTION;
  const every = roll.STAR_SURFACES.yellow_star.flare.every;
  close(roll.starEruption("yellow_star", 0).lift, 0, "an eruption starts from nothing");
  close(roll.starEruption("yellow_star", rise + hold / 2).lift, lift, "held at its height");
  close(roll.starEruption("yellow_star", rise + hold + fall + 0.5).lift, 0, "and settles before the next");
  const first = roll.starEruption("yellow_star", 1), again = roll.starEruption("yellow_star", 2), next = roll.starEruption("yellow_star", every + 1);
  assert.equal(first.angle, again.angle, "one place per eruption");
  assert.notEqual(first.angle, next.angle, "the next erupts elsewhere");
  for (const t of [0, 7, 100, 5_000]) {
    const e = roll.starEruption("red_giant", t);
    assert.ok(e.angle >= 0 && e.angle < Math.PI * 2 && e.lift >= 0 && e.lift <= lift);
  }
  const plain = (value) => JSON.parse(JSON.stringify(value));
  assert.notDeepEqual(plain(roll.starEruption("binary_star", 1, 0)), plain(roll.starEruption("binary_star", 1, 1)), "a binary's stars erupt on their own clocks");
  assert.deepEqual(plain(roll.starEruption("neutron_star", 3)), { angle: 0, lift: 0 });
  const waves = roll.flareWaves(1_234.5);
  assert.equal(waves.lift.length, 4);
  for (const phase of [...waves.lift, ...waves.sway]) assert.ok(phase >= 0 && phase < Math.PI * 2, "phases wrap on the CPU");
  close(waves.lift[0], (0.21 * 1_234.5) % (Math.PI * 2), "drifting at its own pace", 1e-9);
  // The star's uniforms: its flares and the waves, nothing for a pulsar.
  const flaring = new roll.RollingStar(texture(1254), "yellow_star");
  flaring.roll(rise + hold / 2);
  const f = uniforms(flaring).uFlareA;
  const yellow = roll.STAR_SURFACES.yellow_star.flare;
  assert.deepEqual([...f].map((v) => +v.toFixed(4)), [yellow.strength, yellow.reach, +roll.starEruption("yellow_star", rise + hold / 2).angle.toFixed(4), lift]);
  assert.equal(uniforms(flaring).uFlareB[0], 0, "one disc, one limb");
  assert.ok(uniforms(flaring).uWaves.some((v) => v !== 0), "the waves move");
  flaring.setStar("binary_star");
  assert.ok(uniforms(flaring).uFlareB[0] > 0, "the binary's second star flares too");
  for (const still of ["neutron_star", "black_hole", null]) {
    flaring.setStar(still);
    assert.equal(uniforms(flaring).uFlareA[0], 0, `${still}: no flares`);
  }
}

// The shader reads exactly what the class writes.
const fragment = shaderOf(star).glProgram.fragment;
for (const name of Object.keys(uniforms(star))) assert.match(fragment, new RegExp(`uniform \\w+ ${name};`), `${name} declared`);
assert.match(shaderOf(star).glProgram.vertex, /uniform mat3 uTransformMatrix;[\s\S]*vColor = uColor \* uWorldColorAlpha;/,
  "placed and faded like any Pixi mesh");

// Nothing turns a star in the screen's plane any more.
const files = readdirSync(new URL("../src/", import.meta.url), { recursive: true }).filter((f) => f.endsWith(".ts"));
for (const file of files) assert.doesNotMatch(source(file), /starSpinAngle/, `${file}: the clock spin is gone`);
assert.match(source("systemview.ts"), /new RollingStar\(texture, slug\)/, "the System View's star rolls");
assert.match(source("render.ts"), /new RollingStar\(disc\.texture, slug\)/, "the galaxy map's resolved disc rolls with it");
assert.match(source("systemview.ts"), /const slug = this\.lastStarGeometry \? st\.slug : null;/, "icon art (battle backdrop) stays still");

console.log("Star roll: every type's surface and measured discs, the turn by wall time, a pulsar's swing and flash pose, the binary's own rates, the flares' waves and eruptions, sprite-like placement, texture swaps, and no clock spin anywhere passed.");

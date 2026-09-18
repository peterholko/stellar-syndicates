// Actual scenery builder, System View renderer and theater binding; no game,
// login or authoritative state. Graphics calls stand in for the GPU here.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (name, deps = {}, appendix = "") => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name) + appendix, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => deps[id] ?? {}, performance: { now: () => 0 } });
  return exports;
};
const plain = value => JSON.parse(JSON.stringify(value));
const point = () => ({ x: 0, y: 0, set(x, y = x) { this.x = x; this.y = y; } });
class Container {
  children = []; position = point(); scale = point(); pivot = point();
  visible = true; alpha = 1; eventMode = "auto";
  addChild(...children) { for (const child of children) { child.removeFromParent(); child.parent = this; this.children.push(child); } return children[0]; }
  removeFromParent() { if (this.parent) this.parent.children = this.parent.children.filter(c => c !== this); this.parent = null; }
  removeChildren() { const children = this.children; this.children = []; for (const child of children) child.parent = null; return children; }
  destroy() { this.destroyed = true; this.removeFromParent(); for (const child of this.children) child.destroy(); }
}
class Graphics extends Container {
  commands = [];
  clear() { this.commands = []; return this; }
}
for (const key of ["moveTo", "lineTo", "closePath", "fill", "stroke", "circle", "rect", "poly", "ellipse", "arc", "roundRect"]) {
  Graphics.prototype[key] = function (...args) { this.commands.push([key, ...args]); return this; };
}
class Sprite extends Container { constructor(texture) { super(); this.texture = texture; this.anchor = point(); } }
class Text extends Container { constructor({ text, style }) { super(); this.text = text; this.style = style; this.anchor = point(); } }
const pendingArt = new Map();
const pixi = { Container, Graphics, Sprite, Text, TextStyle: class { constructor(style) { Object.assign(this, style); } },
  Texture: { EMPTY: {} }, Assets: { load(url) {
    return new Promise(resolve => { const pending = pendingArt.get(url) ?? []; pending.push(resolve); pendingArt.set(url, pending); });
  } } };
const prng = load("prng.ts");
const stars = load("stars.ts", { "./prng": prng });
const planetArt = load("planetart.ts", { "./planet-art.generated": load("planet-art.generated.ts") });
const systems = load("systemview.ts", { "pixi.js": pixi, "./prng": prng, "./stars": stars, "./planetart": planetArt });
const backdrop = load("battlebackdrop.ts", { "./systemview": systems, "./stars": stars });
const system = { id: "8", name: "Freya", pos: { x: 20_000, y: 0 }, band: "rich" };
const body = (id, kind, parent = null) => ({ id, kind, parent, size: "large", name: `Freya ${id}`,
  deposits: [{ resource: "metallic_ore", richness: 50 }], structures: { shipyard: 3 }, habitable: true });
const report = { id: system.id, owner: "1", bodies: [body(1, "terrestrial"), body(2, "gas_giant"), body(3, "ice", 2)], stockpile: [999] };
const state = { galaxy: { systems: [system] }, systems: [report] };
const original = JSON.stringify(state);
const full = systems.buildVisualSystem(system, report.bodies);
const visual = backdrop.battleSystemScenery(system.id, state);
const geometry = value => value.planets.map(p => [p.id, p.kind, p.orbitRadius, p.radius, p.angle,
  p.moons.map(m => [m.id, m.orbitRadius, m.radius, m.angle])]);
assert.deepEqual(plain(geometry(visual)), plain(geometry(full)), "same planets, moons, scales and orbits as System View");
assert.deepEqual(plain(visual.asteroidBelts), plain(full.asteroidBelts));
assert.doesNotMatch(JSON.stringify(visual), /metallic_ore|shipyard|999|Freya/);
assert.equal(JSON.stringify(state), original, "sanitizing the backdrop never mutates the served view");
report.bodies[0].kind = "lava";
assert.equal(visual.planets[0].kind, "terrestrial", "snapshot is detached from later reports");
report.bodies[0].kind = "terrestrial";
assert.equal(backdrop.battleSystemScenery(null, state), null, "no invented system for deep space");
assert.equal(backdrop.battleSystemScenery(system.id, { ...state, systems: [] }).planets.length, 0, "unknown worlds stay absent");
assert.equal(backdrop.battleSystemScenery(system.id).planets.length, 0, "legacy callers retain a star-only fallback");

const scene = new systems.SystemViewScene({ sceneryOnly: true });
// Pass unsanitized data as a second line of defense: mode itself must exclude UI.
scene.setSystem(full, null);
scene.layout(1068, 672, backdrop.battleSceneryFrame(1068, 672));
scene.setDynamic(report.bodies, [{ key: "shipyard", body_id: 1 }], true);
scene.update("1", "1", 0, true);
assert.equal(scene.root.eventMode, "none");
assert.equal(scene.labels.children.length, 0);
assert.equal(scene.markers.children.length, 0);
assert.equal(scene.overlay.commands.length, 0);
assert.equal(scene.bodies.length, 0);
assert.equal(scene.pickBody(400, 300), null);
assert.ok(scene.orbitsGfx.commands.length > 0);
assert.ok(scene.bodiesGfx.commands.length > 0, "bodies still render before art arrives");
assert.ok(!scene.bodiesGfx.commands.some(([, style]) => style?.color === 0xb0894f || style?.color === 0x7fdc8a), "no resource pips or life halos");
const management = new systems.SystemViewScene();
management.setSystem(full, null); management.layout(1068, 672);
assert.equal(management.bodies.length, 3, "ordinary System View keeps its hit targets");
assert.ok(management.labels.children.length > 0);
assert.ok(!management.bodiesGfx.commands.some(([, style]) => style?.color === 0xb0894f || style?.color === 0x7fdc8a),
  "ordinary System View has no resource circles or habitability halos");
const loadedManagement = new systems.SystemViewScene();
for (const planet of full.planets) loadedManagement.kindTex.set(planet.kind, { width: 256 });
loadedManagement.moonTex = { width: 256 };
loadedManagement.setSystem(full, null); loadedManagement.layout(1068, 672);
assert.equal(loadedManagement.bodySprites.children.length, 3, "planet and moon artwork remains visible");
assert.equal(loadedManagement.bodiesGfx.commands.length, 0, "loaded artwork has no extra body decorations");
const selectedBody = loadedManagement.bodies[0];
assert.equal(loadedManagement.pickBody(selectedBody.sx, selectedBody.sy).id, selectedBody.detail.id);
loadedManagement.update(null, null, 0);
assert.ok(loadedManagement.overlay.commands.some(([op, style]) => op === "stroke" && style.color === 0xffffff),
  "the selected planet still has its selection indicator");
const detail = loadedManagement.detailFor(selectedBody.detail.id);
assert.equal(detail.habitable, true, "habitability remains in the planet's details");
assert.equal(detail.deposits[0].resource, "metallic_ore", "resource information is not removed");

// Late planet art must smooth at small sizes without resetting a picked world.
const firstPick = management.bodies[0];
management.pickBody(firstPick.sx, firstPick.sy);
const artTextures = [];
for (const [url, pending] of pendingArt) {
  if (!url.includes("/derived/planets/") && !url.endsWith("asteroid_belt_chunk.png")) continue;
  const texture = { width: 512, height: 512, source: {} };
  artTextures.push(texture);
  for (const resolve of pending) resolve(texture);
}
// Await the loader, Promise.all and cached-scene refresh.
for (let i = 0; i < 5; i++) await Promise.resolve();
assert.equal(artTextures.length, 9);
assert.ok(artTextures.every(t => t.source.autoGenerateMipmaps && t.source.scaleMode === "linear"),
  "planet/moon minification uses smooth GPU mipmaps");
assert.deepEqual(plain(management.selected), { sx: firstPick.sx, sy: firstPick.sy, r: firstPick.r },
  "art loading is not a new selection");
assert.equal(management.bodySprites.children.length, 3);
for (const [i, planet] of full.planets.entries()) {
  // Planets precede their own moons; this fixture's first two entries are planets.
  const sprite = management.bodySprites.children[i];
  const art = planetArt.planetArtwork(planet.kind);
  assert.ok(Math.abs(sprite.scale.x * sprite.texture.width * art.visualRatio - 2 * planet.radius) < 1e-10,
    "measured disk fills its visual radius, independently of transparent padding");
  assert.deepEqual(plain([sprite.anchor.x, sprite.anchor.y]), plain(art.anchor));
}

// Exercise the actual private binding path rather than a duplicate cache model.
const theater = load("battletheater.ts", { "pixi.js": pixi, "./prng": prng, "./stars": stars,
  "./systemview": systems, "./battlebackdrop": backdrop }, `
  export function bindProbe(rec, source) {
    layers ??= { backdrop: new Container(), debris: new Container(), ships: new Container(), fx: new Container(), ui: new Container() };
    bindRecord(rec, null, source);
    return { visual: backdropVisual, scene: backdropSystem, layer: layers.backdrop };
  }
  export function resizeProbe(width, height) { configureViewport({ width, height }); buildBackdrop(st.rec); }
`);
const record = { id: "battle-at-Freya", system: "8", rounds: [], outcome: null, sides: [] };
const first = theater.bindProbe(record, state);
const frozen = JSON.stringify(first.visual);
report.bodies.push(body(4, "ocean"));
report.bodies[0].structures.shipyard = 4;
const again = theater.bindProbe({ ...record, rounds: [{ notes: [] }] }, state);
assert.equal(again.visual, first.visual, "new rounds and Views do not rebuild scenery");
assert.equal(JSON.stringify(again.visual), frozen);
theater.resizeProbe(390, 600);
assert.equal(JSON.stringify(first.visual), frozen, "mobile resize cannot reread current administration");
assert.deepEqual(plain(first.scene.starLayoutPosition()), { x: 253.5, y: 276 });
assert.equal(first.layer.children.length, 2, "one starfield plus one cached system scene");
const deep = theater.bindProbe({ ...record, id: "deep-space", system: null }, state);
assert.equal(deep.visual, null);
assert.equal(deep.layer.children.length, 1, "old system scenery is detached from deep-space battles");
assert.equal(first.scene.root.destroyed, undefined, "resizing/switching never destroys shared scene assets");
// Complete star loading after switching away: it must not revive the old scene.
for (const resolve of pendingArt.get(stars.starIconUrl(stars.starTypeFor("8"))) ?? []) resolve({ width: 1254 });
await Promise.resolve();
assert.equal(deep.layer.children.length, 1);
theater.theaterClose();
const reopened = theater.bindProbe(record, state);
assert.equal(reopened.visual.planets.length, 3, "reopening may use newly received geography");
assert.equal(reopened.scene, first.scene, "one cached scenery scene for the theater lifetime");
assert.ok(reopened.scene.starSprite, "reopening uses already loaded, correctly anchored star art");
assert.equal(reopened.scene.root.eventMode, "none");
console.log("Battle backdrop: shared astronomy, served-only snapshot, no management UI, deep-space fallback, resize/reopen caching and late-art safety pass.");

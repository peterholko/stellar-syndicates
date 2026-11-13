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
// The rolling star's mesh pieces (src/starroll.ts): recorded, never drawn.
class MeshGeometry { constructor(options) { Object.assign(this, options); } destroy() {} }
class Mesh extends Container { constructor(options) { super(); Object.assign(this, options); } }
class Shader { constructor(options) { this.resources = { ...options.resources }; } destroy() {} }
class UniformGroup { constructor(structure) { this.uniforms = Object.fromEntries(Object.entries(structure).map(([k, v]) => [k, v.value])); } }
const pixi = { Container, Graphics, Sprite, Text, TextStyle: class { constructor(style) { Object.assign(this, style); } },
  Texture: { EMPTY: {} }, Assets: { load(url) {
    return new Promise(resolve => { const pending = pendingArt.get(url) ?? []; pending.push(resolve); pendingArt.set(url, pending); });
  } }, Mesh, MeshGeometry, Shader, UniformGroup, GlProgram: { from: options => options } };
const prng = load("prng.ts");
const stars = load("stars.ts", { "./prng": prng });
const planetArt = load("planetart.ts", { "./planet-art.generated": load("planet-art.generated.ts") });
const protocol = load("protocol.ts");
const layout = load("systemlayout.ts");
const bodyFx = load("systembodyfx.ts", { "pixi.js": pixi, "./systemlayout": layout });
const starRoll = load("starroll.ts", { "pixi.js": pixi });
const starFx = load("systemstarfx.ts", { "pixi.js": pixi, "./systembodyfx": bodyFx, "./systemlayout": layout, "./starroll": starRoll });
const trafficLayer = load("systemtrafficlayer.ts", { "pixi.js": pixi, "./protocol": protocol, "./systemlayout": layout, "./systembodyfx": bodyFx });
const systems = load("systemview.ts", { "pixi.js": pixi, "./prng": prng, "./stars": stars, "./planetart": planetArt,
  "./systemlayout": layout, "./systembodyfx": bodyFx, "./systemstarfx": starFx, "./starroll": starRoll, "./systemtrafficlayer": trafficLayer,
  "./planet-structure-art": { hasOrbitalShipConstruction: () => false }, "./icons": { label: (s) => s } });
const backdrop = load("battlebackdrop.ts", { "./systemview": systems, "./stars": stars, "./systemlayout": layout,
  "./systembodyfx": bodyFx, "./prng": prng });
const system = { id: "8", name: "Freya", pos: { x: 20_000, y: 0 }, band: "rich" };
const body = (id, kind, parent = null) => ({ id, kind, parent, size: "large", name: `Freya ${id}`,
  deposits: [{ resource: "metallic_ore", richness: 50 }], structures: { shipyard: 3 }, habitable: true });
for (const profile of ["mineral_highlands", "verdant_lowlands", "coastal_plains"]) {
  const home = { ...body(1, "terrestrial"), founding_camp: { profile, completed: [] } };
  const expected = profile === "coastal_plains" ? "ocean" : "terrestrial";
  assert.ok(systems.bodyArtUrl("8", home).includes(`/${expected}-`), "habitable founding globes match the living surface art");
  assert.equal(systems.buildVisualSystem(system, [home]).planets[0].kind, expected);
}
const report = { id: system.id, owner: "1", bodies: [body(1, "terrestrial"), body(2, "gas_giant"), body(3, "ice", 2),
  { ...body(4, "asteroid_belt"), habitable: false, belt: { kind: "metallic", radius: 0.5, width: 0.04 } }], stockpile: [999] };
const state = { galaxy: { systems: [system] }, systems: [report] };
const original = JSON.stringify(state);
const full = systems.buildVisualSystem(system, report.bodies);
const visual = backdrop.battleSystemScenery(system.id, state);
const geometry = value => value.planets.map(p => [p.id, p.kind, p.orbitRadius, p.radius, p.angle,
  p.moons.map(m => [m.id, m.orbitRadius, m.radius, m.angle])]);
assert.deepEqual(plain(geometry(visual)), plain(geometry(full)), "same planets, moons, scales and orbits as System View");
const beltShape = value => value.asteroidBelts.map(b => [b.id, b.kind, b.radius, b.width]);
assert.equal(full.asteroidBelts.length, 1, "the charted belt is part of the scenery");
assert.deepEqual(plain(beltShape(visual)), plain(beltShape(full)), "same belts as System View");
assert.ok(visual.asteroidBelts.every(b => b.name === "" && b.deposits.length === 0 && b.pockets.length === 0), "a backdrop belt keeps no name, ore or pockets");
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
scene.update("1", "1", 0);
assert.equal(scene.root.eventMode, "none");
assert.equal(scene.labels.children.length, 0);
assert.equal(scene.markers.children.length, 0);
assert.equal(scene.overlay.commands.length, 0);
assert.equal(scene.bodies.length, 0);
assert.equal(scene.pickBody(400, 300), null);
assert.equal(scene.pick(400, 300), null, "scenery has no traffic or body targets");
assert.ok(scene.orbitsGfx.commands.length > 0);
const discCommands = (target) => target.nodes.flatMap((node) => node.disc.commands);
assert.ok(discCommands(scene).length > 0, "bodies still render before art arrives");
assert.ok(!discCommands(scene).some(([, style]) => style?.color === 0xb0894f || style?.color === 0x7fdc8a), "no resource pips or life halos");
assert.equal(scene.traffic.root.visible, false, "scenery never draws fleets, fights or sieges");
assert.ok(scene.nodes.every((node) => node.orbitals.size === 0 && node.lights.commands.length === 0),
  "scenery never dresses worlds with infrastructure or settlement lights");
const management = new systems.SystemViewScene();
management.setSystem(full, null); management.layout(1068, 672);
assert.equal(management.bodies.length, 3, "ordinary System View keeps its hit targets");
assert.ok(management.labels.children.length > 0);
assert.ok(!discCommands(management).some(([, style]) => style?.color === 0xb0894f || style?.color === 0x7fdc8a),
  "ordinary System View has no resource circles or habitability halos");
const loadedManagement = new systems.SystemViewScene();
for (const planet of full.planets) loadedManagement.kindTex.set(planet.kind, { width: 256 });
loadedManagement.moonTex = { width: 256 };
loadedManagement.setSystem(full, null); loadedManagement.layout(1068, 672);
assert.equal(loadedManagement.nodes.filter((node) => node.art).length, 3, "planet and moon artwork remains visible");
assert.equal(discCommands(loadedManagement).length, 0, "loaded artwork has no extra body decorations");
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
assert.equal(artTextures.length, 11, "the moon, the belt chunk and all nine planet looks, Marginal ones included");
assert.ok(artTextures.every(t => t.source.autoGenerateMipmaps && t.source.scaleMode === "linear"),
  "planet/moon minification uses smooth GPU mipmaps");
assert.deepEqual(plain(management.selected), { sx: firstPick.sx, sy: firstPick.sy, r: firstPick.r },
  "art loading is not a new selection");
assert.equal(management.nodes.filter((node) => node.art).length, 3);
for (const [i, planet] of full.planets.entries()) {
  const sprite = management.nodes.filter((node) => !node.parent)[i].art;
  const art = planetArt.planetArtwork(planet.kind);
  assert.ok(Math.abs(sprite.scale.x * sprite.texture.width * art.visualRatio - 2 * planet.radius) < 1e-10,
    "measured disk fills its visual radius, independently of transparent padding");
  assert.deepEqual(plain([sprite.anchor.x, sprite.anchor.y]), plain(art.anchor));
}

// Exercise the actual private binding path rather than a duplicate cache model.
const starArt = load("starart.ts", { "./star-art.generated": load("star-art.generated.ts") });
const theater = load("battletheater.ts", { "pixi.js": pixi, "./prng": prng, "./stars": stars, "./starart": starArt,
  "./systemview": systems, "./battlebackdrop": backdrop, "./systembodyfx": bodyFx, "./planetart": planetArt }, `
  export function bindProbe(rec, source) {
    layers ??= { backdrop: new Container(), debris: new Container(), ships: new Container(), fx: new Container(), ui: new Container() };
    bindRecord(rec, null, source);
    return { visual: backdropVisual, scene: backdropSystem, layer: layers.backdrop, plan: backdropPlan, arenaWorld };
  }
  export function resizeProbe(width, height) { configureViewport({ width, height }); buildBackdrop(st.rec); }
`);
// A fight out in Freya's approach band keeps the distant system as scenery.
const record = { id: "battle-at-Freya", system: "8", pos: { x: 26_000, y: 0 }, rounds: [], outcome: null, sides: [] };
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
// §star-roll: the backdrop's icon art was never measured for a surface; it stays still.
assert.equal(reopened.scene.starSprite.shader.resources.starRoll.uniforms.uDiscA[3], 0, "the icon star does not roll");
assert.equal(reopened.scene.root.eventMode, "none");
assert.equal(reopened.plan.world, null, "a band fight has no world beneath it");
assert.ok(Math.abs(Math.cos(reopened.plan.starBearing) + 1) < 1e-9, "the star lies back along the fight's bearing");

// A fight inside platform cover stands in a world's orbit: that world up close.
theater.theaterClose();
const orbit = theater.bindProbe({ ...record, id: "battle-over-Freya", pos: { x: 20_150, y: 40 }, started_at: 0 }, state);
assert.ok(orbit.plan.world, "an orbital fight has its world");
// §star (Oct 30 2026): over the glare, the System View's own star painting,
// never the old map icon whose black-cut rim drew a dark ring.
const paintedStar = id => [...pendingArt.keys()].some(url =>
  starArt.starArtwork("system", stars.starTypeFor(id).slug).levels.some(level => level.url === url));
assert.ok(paintedStar("8"), "an orbital fight's star is its System View painting");
assert.equal(orbit.plan.world.kind, full.planets[0].kind, "the defended (main) world's own public art");
assert.equal(orbit.layer.children.length, 2, "starfield plus one close-up composition, no distant scenery");
assert.notEqual(orbit.layer.children[1], first.scene.root, "the close-up replaces the whole-system copy");
const plan = backdrop.battleBackdropPlan({ id: "battle-over-Freya", system: "8", pos: { x: 20_150, y: 40 }, started_at: 0 }, state);
assert.deepEqual(plain(plan), plain(orbit.plan), "the plan is a pure function of the record and served astronomy");
assert.doesNotMatch(JSON.stringify(plan), /Freya|metallic_ore|shipyard|999/, "no names, deposits or infrastructure");
const later = backdrop.battleBackdropPlan({ id: "battle-over-Freya", system: "8", pos: { x: 20_150, y: 40 }, started_at: 5_000 }, state);
assert.notEqual(later.starBearing, plan.starBearing, "the star's direction follows the world's pose when the fight began");
assert.equal(backdrop.battleBackdropPlan({ id: "deep", system: null }, state), null, "deep space has no place plan");
// Peter (Nov 4 2026): a strike on the second world is fought beside it, so the
// replay stands there, not at the home world; the home's Laser Battery stays home.
{
  const home = { ...body(1, "terrestrial"), structures: { orbital_defense: 1 }, population: 0.01, orbit: { radius: 0.6, angle: 0, period: 1e9 } };
  const second = { ...body(2, "ice"), habitable: false, structures: { mining_complex: 1 }, orbit: { radius: 0.95, angle: Math.PI, period: 1e9 } };
  const served = { galaxy: { systems: [system] }, systems: [{ id: "8", owner: "1", bodies: [home, second], stockpile: [] }] };
  const at = b => ({ x: system.pos.x + Math.cos(b.orbit.angle) * b.orbit.radius * layout.SYSTEM_UNIT_SU,
    y: system.pos.y + Math.sin(b.orbit.angle) * b.orbit.radius * layout.SYSTEM_UNIT_SU });
  const art = id => systems.buildVisualSystem(system, [home, second]).planets.find(p => p.id === String(id)).kind;
  assert.notEqual(art(1), art(2), "two worlds, two looks");
  const strike = backdrop.battleBackdropPlan({ id: "strike", system: "8", pos: at(second), started_at: 0 }, served);
  assert.equal(strike.world.kind, art(2), "the second world's own art");
  assert.equal(strike.world.battery, false, "no Laser Battery on it");
  const finale = backdrop.battleBackdropPlan({ id: "finale", system: "8", pos: at(home), started_at: 0 }, served);
  assert.equal(finale.world.kind, art(1));
  assert.equal(finale.world.battery, true, "the battery on its own world");
}
// §den (Oct 29 2026): a fight in a system of belts alone (a pirate den's)
// stands in its asteroid field with its star far off: never a world.
const denSystem = { id: "9", name: "Tarn", pos: { x: 40_000, y: 0 }, band: "rich" };
const denBelt = { ...body(5, "asteroid_belt"), habitable: false, structures: {}, belt: { kind: "icy", radius: 0.5, width: 0.04 } };
const denState = { galaxy: { systems: [system, denSystem] }, systems: [report, { id: "9", owner: null, bodies: [denBelt], stockpile: [] }] };
const denRecord = { ...record, id: "den-fight", system: "9", pos: { x: 40_100, y: 0 }, started_at: 0,
  rounds: [{ notes: [], frame: { world: { x: 0, y: 0, r: 80 }, ships: [] } }] };
const denPlan = backdrop.battleBackdropPlan(denRecord, denState);
assert.equal(denPlan.world, null, "no world in a belt's field");
assert.equal(denPlan.belt.kind, "icy", "the belt's own rocks");
assert.ok(Math.abs(Math.cos(denPlan.starBearing) + 1) < 1e-9, "its star lies back toward the system's centre");
assert.equal(plan.belt, undefined, "a system with a world keeps its world");
// §den (Peter, Nov 5 2026): out on the belt's ring, past the defenses' reach,
// the den's fight is still in the field, not before the star.
const ringPlan = backdrop.battleBackdropPlan({ ...denRecord, id: "den-ring", pos: { x: 42_600, y: 0 } }, denState);
assert.equal(ringPlan.belt?.kind, "icy", "2,600 su out, on the ring");
assert.equal(ringPlan.world, null);
// The founder's den before its system is charted: still the field, not the star.
const unchartedPlan = backdrop.battleBackdropPlan({ ...denRecord, id: "den-dark" },
  { ...denState, systems: [report, { id: "9", owner: null, bodies: [], stockpile: [] }], founding: { den: "9" } });
assert.ok(unchartedPlan.belt, "the den's fight is in its belt even uncharted");
assert.equal(backdrop.battleBackdropPlan({ ...denRecord, id: "dark" }, { ...denState, systems: [report] }).belt, undefined,
  "any other uncharted system keeps its star");
theater.theaterClose();
const den = theater.bindProbe(denRecord, denState);
assert.equal(den.plan.belt.kind, "icy");
assert.equal(den.layer.children.length, 2, "starfield plus the asteroid field");
const field = den.layer.children[1];
assert.ok(field.children.length > 40, "a field of rocks over its dust, with the star");
// §den (Oct 30 2026): the belt type's six single asteroids, smoothed when small.
const rockUrls = [...pendingArt.keys()].filter(url => url.includes("/battle-asteroids/")).sort();
assert.deepEqual(rockUrls, [1, 2, 3, 4, 5, 6].map(n => `/art/derived/battle-asteroids/v1/icy-${n}-256.webp`),
  "the icy belt's own six rocks, every one in play, never the System View's chunk");
const rockTextures = rockUrls.map(url => {
  const texture = { width: 256, height: 256, source: {} };
  for (const resolve of pendingArt.get(url)) resolve(texture);
  return texture;
});
for (let i = 0; i < 3; i++) await Promise.resolve();
assert.ok(rockTextures.every(t => t.source.autoGenerateMipmaps), "far rocks draw from mipmaps");
const rockSprites = field.children.filter(c => rockTextures.includes(c.texture));
assert.ok(rockSprites.length >= 200, "every rock takes its art");
assert.ok(rockSprites.every(s => s.scale.x > 0 && s.scale.x * 256 < 0.15 * 672), "no rock looms over the fight");
assert.ok(paintedStar("9"), "the den's star is its System View painting");
assert.equal(den.arenaWorld, null, "an older record's arena world is not drawn as a planet");
// The backdrop drifts against the camera, below the fight: still at the
// overview, a share of the battlefield's motion when following a hull.
assert.deepEqual({ ...backdrop.backdropDrift({ x: 534, y: 336 }, 1068, 672) }, { x: 0, y: 0 }, "the overview keeps the composed backdrop");
const followed = backdrop.backdropDrift({ x: 534 - 600, y: 336 + 200 }, 1068, 672);
assert.ok(followed.x < 0 && followed.y > 0, "it moves the way the battlefield moves");
assert.ok(Math.abs(followed.x) < 600 && Math.abs(followed.x) > 0, "slower than the hulls, never riding with them");
console.log("Battle backdrop: shared astronomy, served-only snapshot, no management UI, deep-space fallback, resize/reopen caching and late-art safety pass.");

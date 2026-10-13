// Production scene code with GPU/network boundaries substituted. These tests
// verify geometry, replay evidence and resource ownership, not GPU appearance.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

let now = 1000, nextTimer = 0;
const timers = new Map();
const clock = { devicePixelRatio: 2, setTimeout(fn, ms) { const id = ++nextTimer; timers.set(id, { fn, at: now + ms }); return id; },
  clearTimeout(id) { timers.delete(id); } };
const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
const plain = v => JSON.parse(JSON.stringify(v));
const source = name => readFileSync(new URL(`../src/${name}.ts`, import.meta.url), "utf8");
function compile(name, deps = {}, extra = {}, appendix = "") {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name) + appendix, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => { assert.ok(id in deps, `unexpected dependency ${id}`); return deps[id]; },
    performance: { now: () => now }, window: clock, AbortController, console, ...extra });
  return exports;
}
const catalog = compile("ship-models.generated"), prng = compile("prng");
const wrecks = compile("battlewrecks", { "./prng": prng });
const policy = compile("battle-model-policy", { "./ship-models.generated": catalog });
const effects = compile("battle-model-effects", { "./prng": prng });
const geometry = compile("battle-model-geometry", { three: THREE,
  "three/addons/utils/BufferGeometryUtils.js": { mergeGeometries }, "./battle-model-policy": policy });
const ship = (id, kind = "raider", more = {}) => ({ id, kind, x: 500, y: 340,
  angle: 0, canvasPx: 80, alpha: .7, eligible: true, ...more });
const burst = (id = "death:0", more = {}) => ({ id, seed: 1234, type: "death", x: 500, y: 340,
  radius: 46, age: .18, life: .68, color: 0xff8136, cls: 3, ...more });
const frame = (ships = [], bursts = [], more = {}) => ({ width: 1068, height: 672, ships, bursts, tier: 0, reducedMotion: false, ...more });
const kinds = Object.keys(catalog.SHIP_MODELS);

// Admission at both viewport sizes, deterministic ordering, no hidden hulls.
const crowd = Array.from({ length: 500 }, (_, i) => ship(i, kinds[i % kinds.length],
  { x: 50 + i % 950, y: 50 + i % 550, canvasPx: 35 + i % 260 }));
for (const narrow of [false, true]) for (const tier of [0, 3]) {
  const f = frame(crowd, [], { tier }), chosen = policy.pickBattleModels(f, narrow);
  const budget = policy.battleModelBudget(narrow, tier);
  assert.ok(chosen.length <= 80);
  assert.ok(new Set(chosen.map(s => s.kind)).size <= budget.kinds);
  assert.ok(chosen.reduce((sum, s) => sum + catalog.SHIP_MODELS[s.kind].triangles, 0) <= budget.triangles);
  assert.deepEqual(Array.from(policy.pickBattleModels(frame([...crowd].reverse(), [], { tier }), narrow), s => s.id), Array.from(chosen, s => s.id));
}
for (const more of [{ x: -1000 }, { y: NaN }, { alpha: 0 }, { eligible: false }, { angle: NaN }, { canvasPx: 3 }]) {
  assert.equal(policy.pickBattleModels(frame([ship(1, "raider", more)]), false).length, 0);
}
assert.equal(policy.pickBattleModels(frame([ship(1, "raider", { x: -10 })]), false).length, 1);
assert.equal(policy.pickBattleBursts(frame([], Array.from({ length: 200 }, (_, i) => burst(`b${i}`))), false).length, 16);
assert.equal(policy.pickBattleBursts(frame([], [burst("a", { age: -.01 }), burst("b", { age: .68 })]), false).length, 0);
const particlesAt = (age, reduced = false) => { const output = []; effects.visitBattleBurst(burst("death", { age }), reduced, p => output.push(plain(p))); return output; };
const sampled = particlesAt(.18); particlesAt(.45); particlesAt(.01);
assert.deepEqual(particlesAt(.18), sampled, "seeking back does not accumulate movement or random state");
assert.ok(sampled.some(p => p.kind === "shard") && sampled.some(p => p.kind === "fire") && sampled.some(p => p.kind === "ring"));
assert.ok(sampled.every(p => Object.values(p).every(v => typeof v !== "number" || Number.isFinite(v))));
assert.ok(particlesAt(.18, true).filter(p => p.kind === "shard").length < sampled.filter(p => p.kind === "shard").length);
assert.equal(particlesAt(.45).filter(p => p.kind === "shard").length, 0, "hot shrapnel goes out as it cools (cold metal is the hull's own debris)");
assert.equal(particlesAt(-.01).length, 0); assert.equal(particlesAt(.68).length, 0);

// Every actual shipped LOD2 is parsed, transformed and consolidated. Texture
// decoding alone is skipped; tests operate on its real triangles/UVs/materials.
class GeometryLoader extends GLTFLoader {
  constructor() { super(); this.register(parser => ({ name: "GEOMETRY_ACCEPTANCE", beforeRoot() { parser.assignTexture = () => Promise.resolve(null); } })); }
}
const realStage = compile("ship-model-stage", { three: THREE,
  "three/addons/loaders/GLTFLoader.js": { GLTFLoader: GeometryLoader },
  "three/addons/environments/RoomEnvironment.js": { RoomEnvironment: class {} }, "./ship-models.generated": catalog,
}, { fetch: async url => { const bytes = readFileSync(new URL(`../public${url.split("?")[0]}`, import.meta.url));
  return { ok: true, arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) }; } });
const camera = new THREE.OrthographicCamera(-534, 534, 336, -336, .1, 4000);
camera.position.set(0, 2000, 0); camera.up.set(0, 0, -1); camera.lookAt(0, 0, 0); camera.updateMatrixWorld(true);
let totalSections = 0, mergedSections = 0;
for (const [kind, spec] of Object.entries(catalog.SHIP_MODELS)) {
  const model = await realStage.loadShipModel(kind);
  const bounds = new THREE.Box3().setFromObject(model), originalMaterials = new Set();
  let sections = 0;
  model.traverse(m => { if (m instanceof THREE.Mesh) { sections++; originalMaterials.add(m.material); } });
  const instanced = new geometry.BattleInstancedHull(model);
  const triangles = instanced.meshes.reduce((n, m) => n + m.geometry.index.count / 3, 0);
  assert.equal(triangles, spec.triangles, `${kind}: no LOD2 triangles lost or added`);
  assert.ok(instanced.meshes.length <= originalMaterials.size, `${kind}: static sections consolidate by material`);
  totalSections += sections; mergedSections += instanced.meshes.length;
  const combined = new THREE.Box3();
  for (const m of instanced.meshes) {
    m.geometry.computeBoundingBox(); combined.union(m.geometry.boundingBox);
    assert.ok(m.geometry.getAttribute("uv")); assert.ok(m.geometry.getAttribute("normal"));
    assert.equal(m.instanceMatrix, instanced.meshes[0].instanceMatrix, "all sections share one instance transform buffer");
  }
  assert.ok(combined.min.distanceTo(bounds.min) < 1e-5 && combined.max.distanceTo(bounds.max) < 1e-5, `${kind}: transformed shape preserved`);
  for (const angle of [0, Math.PI / 4, Math.PI / 2, Math.PI, -Math.PI / 2]) {
    const s = ship(1, kind, { angle, x: 275, y: 209, canvasPx: 240, alpha: .37 });
    instanced.update([s, ship(2, kind)], 1068, 672);
    const matrix = new THREE.Matrix4(); instanced.meshes[0].getMatrixAt(0, matrix);
    const nose = new THREE.Vector3(0, 0, -1).applyMatrix4(matrix).project(camera);
    const tail = new THREE.Vector3(0, 0, 1).applyMatrix4(matrix).project(camera);
    const heading = Math.atan2(-(nose.y - tail.y) * 336, (nose.x - tail.x) * 534);
    assert.ok(Math.abs(Math.atan2(Math.sin(heading - angle), Math.cos(heading - angle))) < 1e-6, `${kind}: nose follows heading`);
    const center = new THREE.Vector3().applyMatrix4(matrix).project(camera);
    assert.ok(Math.abs((center.x + 1) * 534 - 275) < 1e-5);
    assert.ok(Math.abs((1 - center.y) * 336 - 209) < 1e-5);
    assert.ok(Math.abs(instanced.meshes[0].geometry.getAttribute("battleOpacity").getX(0) - .37) < 1e-6);
    assert.ok(instanced.meshes.every(m => m.count === 2));
    assert.ok(matrix.elements[13] === 0, "top-down hulls share one depth (the established look)");
  }
  // Wreck sections: every triangle in exactly one, nose to tail, and at rest
  // they sit exactly where the intact hull was drawn (a seamless hand-off).
  const pieces = wrecks.WRECK_PIECES[kind];
  if (pieces >= 2) {
    const chunks = new geometry.BattleHullChunks(instanced, wrecks.wreckCuts(kind));
    assert.equal(chunks.chunks.length, pieces, `${kind}: one section per piece`);
    assert.equal(chunks.chunks.reduce((n, c) => n + c.triangles, 0), spec.triangles, `${kind}: every triangle in exactly one section`);
    assert.ok(chunks.chunks.every((c, i) => i === 0 || c.pivot.z > chunks.chunks[i - 1].pivot.z), `${kind}: sections run nose to tail`);
    const pose = ship(1, kind, { angle: .7, x: 300, y: 260, canvasPx: 240 });
    const shownIds = chunks.update(Array.from({ length: pieces }, (_, c) => ({ ...pose, id: `w:${c}`, chunk: c, char: .3,
      offset: [0, 0, 0], tumble: [0, 0, 0] })), 1068, 672, -Math.acos(.8), .75);
    assert.equal(shownIds.length, pieces);
    instanced.update([pose], 1068, 672, -Math.acos(.8), .75);
    const whole = new THREE.Matrix4(); instanced.meshes[0].getMatrixAt(0, whole);
    for (const c of chunks.chunks) {
      const at = new THREE.Matrix4(); c.meshes[0].getMatrixAt(0, at);
      assert.ok(at.elements.every((v, i) => Math.abs(v - whole.elements[i]) < 1e-4), `${kind}: sections at rest match the hull`);
    }
    chunks.dispose();
  }
  instanced.update([], 1068, 672); assert.equal(instanced.group.visible, false);
  instanced.dispose(); realStage.disposeModel(model);
}
console.log(`Battle geometry: all ${kinds.length} game kinds with exact LOD2 meshes; ${totalSections} static sections → ${mergedSections} material batches.`);

// The angled theater: each hull lies IN the tilted battlefield, seen from its
// near (lower) edge, and lower hulls sit nearer the camera so they occlude.
const plane = Math.acos(.8), slope = Math.tan(plane);
const planeNormal = new THREE.Vector3(0, .8, -.6), planeSouth = new THREE.Vector3(0, .6, .8);
for (const h of [0, Math.PI / 4, 2, -2.5, Math.PI / 2, Math.PI]) {
  const angle = Math.atan2(Math.sin(h) * .8, Math.cos(h)); // the heading as drawn on screen
  const m = new THREE.Matrix4();
  geometry.battleHullMatrix(ship(1, "raider", { angle, x: 275, y: 520, canvasPx: 224 }), 1068, 672, m, -plane, slope);
  const axis = (x, y, z) => new THREE.Vector3(x, y, z).transformDirection(m);
  assert.ok(Math.abs(axis(0, 1, 0).dot(planeNormal) - 1) < 1e-9, "the deck faces the tilted plane's normal");
  const nose = axis(0, 0, -1);
  assert.ok(Math.abs(nose.dot(planeNormal)) < 1e-9, "the nose lies in the plane");
  const inPlane = Math.atan2(nose.dot(planeSouth), nose.x);
  assert.ok(Math.abs(Math.atan2(Math.sin(inPlane - h), Math.cos(inPlane - h))) < 1e-9, "the hull flies its recorded heading");
  assert.ok(Math.abs(Math.atan2(Math.sin(Math.atan2(nose.z, nose.x) - angle), Math.cos(Math.atan2(nose.z, nose.x) - angle))) < 1e-9,
    "the projected nose matches the 2D art");
  assert.deepEqual([m.elements[12], m.elements[14]], [275 - 534, 520 - 336], "depth never moves the hull on screen");
  assert.ok(Math.abs(m.elements[13] - (520 - 336) * slope) < 1e-9, "lower hulls sit nearer the camera");
}

// Actual runtime with a recording GPU. Includes multiple ships per material,
// bounded concurrent loading, pause, resize, eviction, errors and late results.
const requests = [], disposed = [], renders = [];
const warnings = [];
const canvases = [];
const makeCanvas = () => { const c = { width: 300, height: 150, listeners: new Map(),
  addEventListener(name, fn) { this.listeners.set(name, fn); }, removeEventListener(name) { this.listeners.delete(name); } };
  canvases.push(c); return c; };
const renderers = [];
class RecordingRenderer {
  debug = {};
  info = { render: { calls: 0, triangles: 0 } };
  constructor({ canvas }) { this.canvas = canvas; renderers.push(this); }
  setPixelRatio(r) { this.ratio = r; } setClearColor() {}
  setSize(w, h) { this.canvas.width = w * this.ratio; this.canvas.height = h * this.ratio; }
  render(scene) {
    this.scene = scene;
    this.info.render = { calls: 0, triangles: 0 }; const items = [];
    scene.traverseVisible(m => { if (m instanceof THREE.Mesh && m.count > 0) {
      this.info.render.calls++; this.info.render.triangles += (m.geometry.index?.count ?? m.geometry.attributes.position.count) / 3 * m.count;
      items.push({ count: m.count, material: m.material });
    } }); renders.push(items);
  }
  dispose() { this.disposed = true; } forceContextLoss() {}
}
const stage = { modelLighting: () => ({ scene: new THREE.Scene(), dispose() {} }),
  disposeModel: model => disposed.push(model),
  loadShipModel: (kind, signal, textureSize) => new Promise((resolve, reject) => requests.push({ kind, signal, textureSize, resolve, reject })) };
const { BattleModelRuntime } = compile("battle-model-runtime", { three: { ...THREE, WebGLRenderer: RecordingRenderer },
  "./ship-model-stage": stage, "./battle-model-geometry": geometry, "./battle-model-policy": policy,
  "./battle-model-effects": effects, "./battlewrecks": wrecks }, { document: { createElement: makeCanvas }, console: { warn: (...args) => warnings.push(args) } });
const hull = () => { const g = new THREE.Group(), material = new THREE.MeshStandardMaterial();
  for (let i = 0; i < 3; i++) { const m = new THREE.Mesh(new THREE.BoxGeometry(.5, .2, 2), material); m.position.x = i * .1; g.add(m); } return g; };
const runtime = new BattleModelRuntime(false);
const lastRenderer = () => renderers.at(-1);
const fleet = Array.from({ length: 30 }, (_, i) => ship(i));
let sample = runtime.render(frame(fleet), now);
assert.equal(requests.length, 1); assert.equal(sample.ships.size, 0, "not admitted before successful model load");
assert.equal(requests[0].textureSize, 1024);
const first = hull(); requests[0].resolve(first); await flush();
sample = runtime.render(frame(fleet), now);
assert.equal(sample.ships.size, 30); assert.equal(runtime.debug().drawCalls, 1, "30 hulls × 3 sections draw as ONE material batch");
assert.equal(runtime.debug().triangles, 30 * 36);
const count = renders.length;
assert.equal(runtime.render(frame(fleet), now + 40).changed, false); assert.equal(renders.length, count, "paused scene avoids redraw/upload");
const angledPose = { pitch: -plane, ringTilt: -plane, depthSlope: slope };
assert.equal(runtime.render(frame(fleet, [], angledPose), now + 60).changed, true, "a view change redraws a paused scene");
runtime.render(frame([], [burst("tilted", { y: 520 })], angledPose), now + 80);
let ringDepth = null;
lastRenderer().scene.traverse(m => { if (m instanceof THREE.InstancedMesh && m.geometry.type === "TorusGeometry" && m.count) {
  const at = new THREE.Matrix4(); m.getMatrixAt(0, at); ringDepth = at.elements[13]; } });
assert.ok(Math.abs(ringDepth - (46 * .2 + (520 - 336) * slope)) < 1e-3, "effects share the hulls' depth plane"); // float32 instance data
sample = runtime.render(frame(fleet, [burst()]), now + 100);
assert.ok(sample.bursts.has("death:0")); assert.ok(runtime.debug().drawCalls <= 6, "particle pools have fixed draw cost");
assert.equal(canvases[0].width, 1602);
runtime.render(frame([], [], { width: 390, height: 600 }), now + 200);
assert.equal(canvases[0].width, 585); assert.equal(runtime.debug().ships, 0, "empty/fogged frame clears every old instance");
sample = runtime.render(frame([ship(1, "cruiser"), ship(2, "titan"), ship(3, "scout")]), now + 300);
assert.equal(runtime.debug().loading.length, 2, "at most two parallel downloads");
runtime.dispose(); assert.equal(disposed.filter(m => m === first).length, 1);
assert.ok(requests.slice(1).every(r => r.signal.aborted));
for (const pending of requests.slice(1)) { const late = hull(); pending.resolve(late); await flush(); assert.ok(disposed.includes(late)); }
assert.equal(runtime.healthy, false);

const narrowRuntime = new BattleModelRuntime(true);
const ri = requests.length;
narrowRuntime.render(frame([ship(2, "corvette")]), now);
assert.equal(requests[ri].textureSize, 512);
requests[ri].reject(new Error("expected fixture download failure")); await flush();
assert.equal(warnings.length, 1);
narrowRuntime.render(frame([ship(2, "corvette")]), now + 100);
assert.equal(requests.length, ri + 1, "failed fetch does not retry every frame");
canvases.at(-1).listeners.get("webglcontextlost")({ preventDefault() {} });
assert.equal(narrowRuntime.healthy, false); narrowRuntime.dispose();

// Switching between disjoint fleets cannot accumulate all eighteen textures.
stage.loadShipModel = async () => hull();
const cacheRuntime = new BattleModelRuntime(false);
for (const batch of [kinds.slice(0, 8), kinds.slice(8, 16), kinds.slice(0, 8)]) {
  const f = frame(batch.map((kind, i) => ship(i, kind)));
  for (let i = 0; i < 6; i++) { cacheRuntime.render(f, now); await flush(); }
  assert.equal(cacheRuntime.debug().loading.length, 0);
  assert.equal(cacheRuntime.debug().ships, 8);
  assert.ok(cacheRuntime.debug().cached.length <= 8);
}
cacheRuntime.render(frame(), now + 11000);
assert.equal(cacheRuntime.debug().cached.length, 0);
cacheRuntime.dispose();

// The Pixi bridge cannot hide art before a successful render, retain old hulls
// on context loss, or recreate a closed viewer when a dynamic import resolves.
class Point { set(x, y = x) { this.x = x; this.y = y; } copyFrom(p) { this.x = p.x; this.y = p.y; } }
class Container { children = []; position = new Point(); scale = new Point(); rotation = 0; alpha = 1; visible = true;
  addChild(...c) { this.children.push(...c); return c[0]; }
  on(name, fn) { (this.handlers ??= {})[name] = fn; } emit(name) { this.handlers?.[name]?.(); } }
class Sprite extends Container { constructor(texture) { super(); this.texture = texture; this.anchor = new Point(); } }
class Texture { static EMPTY = {}; constructor(opts) { Object.assign(this, opts); } update() {} destroy() { this.destroyed = true; } }
class CanvasSource { updates = 0; constructor(opts) { Object.assign(this, opts); } resize(w, h) { this.width = w; this.height = h; }
  update() { this.updates++; } destroy() { this.destroyed = true; } }
let fake, imports = 0;
class RuntimeBoundary {
  healthy = true; ready = false; canvas = { width: 1068, height: 672 };
  constructor() { fake = this; }
  render(f) { return { ships: new Set(this.ready ? f.ships.map(s => s.id) : []), bursts: new Set(), wrecks: new Set(), changed: true }; }
  debug() { return {}; } dispose() { this.disposed = true; }
}
const bridgeDeps = { "pixi.js": { CanvasSource, Sprite, Texture } };
Object.defineProperty(bridgeDeps, "./battle-model-runtime", { get() { imports++; return { BattleModelRuntime: RuntimeBoundary }; } });
const { BattleModels } = compile("battle-models", bridgeDeps);
const bridge = new BattleModels();
bridge.render(frame(), now); await flush(); assert.equal(imports, 0);
bridge.render(frame([ship(1)]), now); bridge.close(); await flush();
assert.equal(fake, undefined, "late import after close cannot allocate a renderer");
bridge.render(frame([ship(1)]), now); await flush();
assert.equal(bridge.render(frame([ship(1)]), now).ships.size, 0);
fake.ready = true;
assert.equal(bridge.render(frame([ship(1)]), now).ships.size, 1); assert.equal(bridge.sprite.visible, true);
fake.healthy = false;
assert.equal(bridge.render(frame([ship(1)]), now).ships.size, 0);
assert.ok(fake.disposed); assert.equal(bridge.sprite.visible, false);
bridge.close();

// Exercise the real theater's schedule, model handoff and sprite fallback.
// No browser or state manipulation: synthetic, already-arrived records only.
class Graphics extends Container { clear() { return this; } }
for (const fn of ["poly", "fill", "stroke", "circle", "ellipse", "roundRect", "rect", "star", "moveTo", "lineTo"]) Graphics.prototype[fn] = function () { return this; };
class Text extends Container { constructor({ text, style }) { super(); this.text = text; this.style = style; this.anchor = new Point(); } }
const art = compile("shipart", { "./ship-art.generated": compile("ship-art.generated") });
const fire = compile("battlefire");
const hardpoints = compile("ship-hardpoints.generated");
const battlefx = compile("battlefx", { "./prng": prng, "./ship-hardpoints.generated": hardpoints });
const fxsprites = compile("battlefxsprites", { "pixi.js": { Sprite, Texture } });
class Rectangle { constructor(x, y, width, height) { Object.assign(this, { x, y, width, height }); } }
const theaterDeps = { "pixi.js": { Container, Sprite, Texture, Graphics, Text, TextStyle: class {}, Rectangle, Assets: { load: () => new Promise(() => {}) } },
  "./prng": prng, "./stars": {}, "./systemview": {}, "./battlebackdrop": {}, "./systembodyfx": {}, "./planetart": {}, "./render": { COL_OWN: 0x72cfff, COL_OTHER: 0xff6655 },
  "./battlefire": fire, "./shipart": art, "./battle-models": { BattleModels }, "./battlefx": battlefx, "./battlefxsprites": fxsprites,
  "./battlewrecks": wrecks, "./core/derive/station": compile("core/derive/station") };
const theater = compile("battletheater", theaterDeps, {}, `
  /// \`pointer\`: [ship index, Pixi event] pairs sent to the ships' roots after
  /// the first frame (the viewer pointing at hulls), then a second frame.
  export function probe(rec, time, coverage = [], pirateId = null, zoom = 1.3, angled = true, pointer = []) {
    layers = { ships: new Container(), ui: new Container(), wrecks: new Container() } as any;
    shipPool = []; camZoom = zoom; camX = camY = 0; following = false; hoverShip = null;
    viewAngled = angled; tiltY = angled ? THEATER_TILT : 1;
    st = { rec, pirateId, round: 0, frac: time, live: false, ships: [], fx: null, withdrawFrom: [Infinity, Infinity], windowKey: "" };
    let input;
    battleModels = { render(f) { input = f; return { ships: new Set(coverage), bursts: new Set() }; }, debug() {} } as any;
    frame(1 / 30);
    if (pointer.length) { for (const [i, ev] of pointer) (st.ships[i].root as any).emit(ev); frame(1 / 30); }
    return { input, fx: st.fx, hash: theaterHash(), ships: st.ships.map(s => ({ kind: s.kind, alpha: s.sprite.alpha,
      bodyAlpha: s.body.alpha, rootAlpha: s.root.alpha, hp: s.hp, reps: s.reps, chromeAlpha: s.chrome.alpha,
      chromeParent: layers.ui.children.includes(s.chrome), heading: s.spin.rotation, eligible: s.modelEligible,
      hullMeter: s.hull.visible, meterY: s.hull.position.y - s.root.position.y,
      pointable: (x, y) => s.root.hitArea.contains(x, y), zs: s.root.scale.x, px: s.modelPx,
      squash: s.root.scale.y / s.root.scale.x, depth: s.root.zIndex })) };
  }
  /// Recorded torpedoes at (round, frac): each track with its head (arena
  /// units), the ships' evidenced hit times, PD screens and 3D bursts.
  export function torpProbe(rec, round, frac) {
    layers = { ships: new Container(), ui: new Container(), wrecks: new Container() } as any;
    shipPool = []; camZoom = 1.3; camX = camY = 0; following = false; viewAngled = false; tiltY = 1;
    st = { rec, pirateId: null, round, frac, live: false, ships: [], fx: null, withdrawFrom: [Infinity, Infinity], windowKey: "" };
    let input;
    battleModels = { render(f) { input = f; return { ships: new Set(), bursts: new Set(), wrecks: new Set() }; }, debug() {} } as any;
    frame(1 / 30);
    return { torps: st.fx.torps.map((t) => ({ ...t, head: torpHead(t, frac) })), arcs: st.fx.arcs.length,
      pd: st.fx.pdShips.map((i) => st.ships[i].cid), cids: st.ships.map((v) => v.cid),
      hits: st.ships.map((v) => [...v.hitTimes]), ships: st.ships.map((v) => { const m = trackMotion(v, frac); return [m.x, m.y]; }),
      bursts: input.bursts.filter((b) => b.id.startsWith("torp")).map((b) => ({ id: b.id, type: b.type, x: b.x, y: b.y })), hash: theaterHash(),
      screen: st.ships.map((v) => [v.root.position.x, v.root.position.y]), pxPerUnit: SCALE * camZoom };
  }
  /// The follow camera after a few frames at (round, frac): a fresh battle
  /// (following, no hull chosen), at \`zoom\` (null = the overview).
  export function followProbe(rec, round, frac, zoom) {
    layers = { ships: new Container(), ui: new Container(), wrecks: new Container() } as any;
    shipPool = []; viewAngled = false; tiltY = 1; camX = camY = 0;
    st = { rec, pirateId: null, round, frac, live: false, ships: [], fx: null, withdrawFrom: [Infinity, Infinity], windowKey: "" };
    battleModels = { render() { return { ships: new Set(), bursts: new Set() }; }, debug() {} } as any;
    followCid = null; resetCamera();
    if (zoom !== null) { cameraOverview = false; camZoom = zoom; }
    frame(1 / 60);
    const shipAt = (cid) => { const v = st.ships.find((s) => s.inUse && s.cid === cid); if (!v) return null; const m = trackMotion(v, st.frac); return [m.x, m.y]; };
    return { cid: followCid, cam: [camX, camY], at: shipAt(followCid), zoom: camZoom, overview: overviewZoom(), step: (n = 1) => { for (let i = 0; i < n; i++) frame(1 / 60); return { cid: followCid, cam: [camX, camY], at: shipAt(followCid) }; } };
  }
  /// A click at a canvas point after one frame (free camera at \`zoom\`): the
  /// selection it makes, as the info card would read it.
  export function pickProbe(rec, round, frac, zoom, point) {
    layers = { ships: new Container(), ui: new Container(), wrecks: new Container() } as any;
    shipPool = []; viewAngled = false; tiltY = 1; camX = camY = 0; camZoom = zoom; cameraOverview = false;
    st = { rec, pirateId: null, round, frac, live: false, ships: [], fx: null, withdrawFrom: [Infinity, Infinity], windowKey: "" };
    battleModels = { render() { return { ships: new Set(), bursts: new Set() }; }, debug() {} } as any;
    following = false; followCid = null;
    frame(1 / 60);
    const at = (cid) => { const v = st.ships.find((s) => s.inUse && s.cid === cid); return v ? [v.root.position.x, v.root.position.y] : null; };
    const p = typeof point === "function" ? point(at) : point;
    pickAt(p[0], p[1]);
    return { cid: followCid, following, info: theaterSelection() };
  }
  /// Drive the frame-rate ladder with a run of frame times (s), from a warm
  /// stage at \`preferred\`: each frame's [cap, effects tier, scene time].
  export function paceProbe(preferred, dts) {
    preferredFps = targetFps = fpsEma = preferred; perfTier = 0; fastRetryAt = 0; fastRetryWait = 20; sceneClock = 10;
    return dts.map((dt) => { sceneClock += dt; pace(dt); return [targetFps, perfTier, sceneClock]; });
  }
  /// The 3D pose (hull pitch, ring tilt) while the view eases through \`t\`.
  export function poseAt(t) { tiltY = t; return [hullPitch(), ringTilt()]; }
  /// Positions and headings along the curved tracks at (round, frac).
  export function trackProbe(rec, round, frac) {
    layers = { ships: new Container(), ui: new Container(), wrecks: new Container() } as any;
    shipPool = []; camZoom = 1.3; camX = camY = 0; following = false; viewAngled = false; tiltY = 1;
    st = { rec, pirateId: null, round, frac, live: false, ships: [], fx: null, withdrawFrom: [Infinity, Infinity], windowKey: "" };
    battleModels = { render() { return { ships: new Set(), bursts: new Set() }; }, debug() {} } as any;
    frame(1 / 30);
    return st.ships.map((v) => { const m = trackMotion(v, frac);
      return { cid: v.cid, x: m.x, y: m.y, vx: m.vx, vy: m.vy, heading: v.spin.rotation - Math.PI / 2, speed: v.speed }; });
  }
  /// A frame at (round, frac) with effect sprites live: the 3D input, the
  /// light and smoke sprites, the death timeline and the ships.
  /// \`artDebris\`: every dead hull's art has loaded and been cut into debris.
  export function afterProbe(rec, round, frac, artDebris = false) {
    const tex = (name, width, height) => new Texture({ name, width, height });
    fxTex = { soft: tex("soft", 64, 64), hot: tex("hot", 64, 64), beam: tex("beam", 4, 64), spark: tex("spark", 64, 16),
      ring: tex("ring", 128, 128), puff: tex("puff", 64, 64), flare: tex("flare", 64, 64), plume: tex("plume", 32, 128),
      plate: tex("plate", 32, 32), strut: tex("strut", 32, 8) } as any;
    const light = new Container(), smoke = new Container(), debris = new Container();
    fxLight = new FxSpritePool(light, "add");
    fxSmoke = new FxSpritePool(smoke, "normal");
    fxDebris = new FxSpritePool(debris, "normal");
    layers = { ships: new Container(), ui: new Container(), wrecks: new Container() } as any;
    shipPool = []; camZoom = 1.3; camX = camY = 0; following = false; viewAngled = true; tiltY = THEATER_TILT;
    st = { rec, pirateId: null, round, frac, live: false, ships: [], fx: null, withdrawFrom: [Infinity, Infinity], windowKey: "" };
    debrisAtlases.clear();
    if (artDebris) for (const e of deathTimeline()) debrisAtlases.set(\`\${e.artUrl}|\${e.kind}\`, {
      plates: [0, 1, 2, 3, 4, 5].map((i) => tex(\`debris-plate-\${i}\`, 64, 64)), slivers: [0, 1].map((i) => tex(\`debris-spar-\${i}\`, 64, 64)) });
    let input;
    battleModels = { render(f) { input = f; return { ships: new Set(), bursts: new Set(), wrecks: new Set() }; }, debug() {} } as any;
    frame(1 / 30);
    debrisAtlases.clear();
    const shown = (layer) => layer.children.filter((c) => c.visible).map((c) => ({ tex: c.texture.name,
      x: c.position.x, y: c.position.y, w: c.scale.x * c.texture.width, h: c.scale.y * c.texture.height, alpha: c.alpha, tint: c.tint }));
    const out = { canvas: [CANVAS_W, CANVAS_H], light: shown(light), smoke: shown(smoke), debris: shown(debris),
      bursts: input.bursts.map((b) => ({ id: b.id, type: b.type, x: b.x, y: b.y })),
      wrecks: (input.wrecks ?? []).map((w) => ({ id: w.id, chunk: w.chunk, offset: [...w.offset], char: w.char, alpha: w.alpha, x: w.x, y: w.y })),
      timeline: deathTimeline().map((e) => ({ key: e.key, T: e.T, breachT: e.breachT, cls: e.cls, pieces: e.pieces, cid: e.cid, px: e.px })),
      zoomScale: Math.pow(camZoom, 0.85) / (SCALE * camZoom),
      rams: st.fx.rams.map((r) => ({ ...r })), hits: st.ships.map((v) => [...v.hitTimes]), cids: st.ships.map((v) => v.cid),
      tackles: st.fx.tackles.map((l) => [st.ships[l.from].cid, st.ships[l.to].cid]),
      webs: st.fx.webs.map((l) => [st.ships[l.from].cid, st.ships[l.to].cid]),
      ramBursts: input.bursts.filter((b) => b.id.startsWith("ram:")).map((b) => ({ id: b.id, x: b.x, y: b.y })),
      ships: st.ships.map((v) => ({ cid: v.cid, x: v.root.position.x, y: v.root.position.y, alpha: v.root.alpha })) };
    fxTex = null; fxLight = null; fxSmoke = null; fxDebris = null;
    return out;
  }
  /// The same frame with the effect sprites live (mock textures and pools).
  export function fxProbe(rec, time, withdrawFrom = [Infinity, Infinity]) {
    const tex = (name, width, height) => new Texture({ name, width, height });
    fxTex = { soft: tex("soft", 64, 64), hot: tex("hot", 64, 64), beam: tex("beam", 4, 64), spark: tex("spark", 64, 16),
      ring: tex("ring", 128, 128), puff: tex("puff", 64, 64), flare: tex("flare", 64, 64), plume: tex("plume", 32, 128),
      plate: tex("plate", 32, 32), strut: tex("strut", 32, 8) } as any;
    const light = new Container(), smoke = new Container();
    fxLight = new FxSpritePool(light, "add");
    fxSmoke = new FxSpritePool(smoke, "normal");
    layers = { ships: new Container(), ui: new Container(), wrecks: new Container() } as any;
    shipPool = []; camZoom = 1.3; camX = camY = 0; following = false; viewAngled = true; tiltY = THEATER_TILT;
    st = { rec, pirateId: null, round: 0, frac: time, live: false, ships: [], fx: null, withdrawFrom, windowKey: "" };
    battleModels = { render() { return { ships: new Set(), bursts: new Set() }; }, debug() {} } as any;
    frame(1 / 30);
    const shown = (layer) => layer.children.filter((c) => c.visible).map((c) => ({ tex: c.texture.name, blend: c.blendMode,
      x: c.position.x, y: c.position.y, w: c.scale.x * c.texture.width, h: c.scale.y * c.texture.height, rotation: c.rotation, alpha: c.alpha, tint: c.tint }));
    const out = { hash: theaterHash(), light: shown(light), smoke: shown(smoke),
      ships: st.ships.map((s) => ({ x: s.root.position.x, y: s.root.position.y, engines: s.engines.visible,
        flames: s.flames.filter((f, i) => i % 2 === 1 && f.visible).map((f) => f.scale.y * f.texture.height),
        blends: s.flames.map((f) => f.blendMode) })) };
    fxTex = null; fxLight = null; fxSmoke = null;
    return out;
  }
`);
const a = { cid: 1, side: 0, kind: "cruiser", x: -300, y: 0, hp: 1 };
const b = { cid: 2, side: 1, kind: "raider", x: 300, y: 0, hp: 1 };
const counts = [[{ kind: "cruiser", exact: 1 }], [{ kind: "raider", exact: 4 }]];
const record = { id: "3d-proof", sides: [{ corp: "own", loadouts: [] }, { corp: "pirate", loadouts: [] }], own_side: 0,
  rounds: [{ tick: 1, dealt: [0, 0], counts, notes: [], frame: { ships: [a, b], gunfire: [], deaths: [], torpedoes: [] } },
    { tick: 2, dealt: [20, 0], counts, notes: [], frame: { ships: [{ ...a, x: -250 }, { ...b, x: 250, hp: .8 }],
      gunfire: [{ side: 0, from: 1, to: 2, weapon: "beam", damage: 20 }], deaths: [], torpedoes: [] } }] };
const original = JSON.stringify(record);
let t = theater.probe(record, .5);
const hitTime = t.fx.beams[0].t; // cruiser is a light beam (capital >= 8000)
t = theater.probe(record, hitTime + .14 / 1.16 + .01, [0, 1]);
assert.ok(t.input.bursts.some(b => b.id === "beam:0"));
assert.ok(t.ships.every(s => s.alpha === 0 && s.bodyAlpha === 0 && s.chromeParent));
assert.equal(t.ships[1].reps, 4); assert.equal(t.ships[1].hp, .8); assert.ok(t.ships[1].chromeAlpha > 0);
// Hull meters: only the ship under the viewer's pointer shows one.
assert.ok(t.ships.every(s => !s.hullMeter), "no hull meters until the viewer points at a ship");
const damagedAt = hitTime + .14 / 1.16 + .01;
const pointed = theater.probe(record, damagedAt, [0, 1], null, 1.3, true, [[1, "pointerover"]]);
assert.deepEqual(plain(pointed.ships.map(s => s.hullMeter)), [false, true], "the pointed-at ship shows its meter");
assert.ok(pointed.ships[1].meterY > 0, "…just below the hull, clear of the tooltip above it");
assert.deepEqual(plain(theater.probe(record, .5, [], null, 1.3, true, [[0, "pointerover"]]).ships.map(s => s.hullMeter)), [true, false],
  "a hull at full strength shows a full meter when pointed at");
assert.deepEqual(plain(theater.probe(record, damagedAt, [], null, 1.3, true, [[1, "pointerover"], [1, "pointerout"]]).ships.map(s => s.hullMeter)),
  [false, false], "the meter goes when the pointer leaves");
assert.deepEqual(plain(theater.probe(record, damagedAt, [], null, 1.3, true, [[1, "pointerover"], [0, "pointerover"], [1, "pointerout"]]).ships.map(s => s.hullMeter)),
  [true, false], "moving straight onto another ship moves the meter with it");
assert.equal(pointed.hash, t.hash, "pointing at a ship cannot change combat presentation truth");
// The pointable area: the hull's footprint, never under 12 px on screen.
const raider = pointed.ships[1], cruiser = pointed.ships[0];
assert.ok(raider.pointable(0, 0) && !raider.pointable(400, 0), "a ship is pointed at on its hull, not across the field");
assert.ok(raider.pointable(11.5 / raider.zs, 0) && !raider.pointable(12.5 / raider.zs, 0), "a small hull is pointable 12 px out");
assert.ok(cruiser.pointable(cruiser.px * .5, 0), "a large hull is pointable along its length");
const without = theater.probe(record, hitTime + .14 / 1.16 + .01);
assert.ok(without.ships.every(s => s.alpha === 1 && s.bodyAlpha === 1));
assert.equal(without.hash, t.hash, "GPU readiness cannot change combat presentation truth");
assert.equal(theater.probe(record, .3, [], "pirate").ships[1].eligible, false, "privateer silhouette is preserved");
const missRecord = structuredClone(record); missRecord.rounds[1].dealt = [0, 0]; missRecord.rounds[1].frame.gunfire[0].damage = 0;
missRecord.rounds[1].frame.ships[1].hp = 1;
assert.ok(!theater.probe(missRecord, hitTime + .14 / 1.16 + .01).input.bursts.some(b => b.type === "impact"));
const frontier = { ...record, rounds: [record.rounds[0]] };
assert.equal(theater.probe(frontier, .5).input.bursts.length, 0, "unarrived shots cannot produce 3D effects");
const noFrames = { ...record, rounds: record.rounds.map(r => ({ ...r, frame: null })) };
const hidden = theater.probe(noFrames, .5); assert.equal(hidden.input.ships.length, 0); assert.equal(hidden.input.bursts.length, 0);
const loss = structuredClone(record); loss.rounds[1].frame.ships = [{ ...a, x: -250 }];
loss.rounds[1].frame.deaths = [{ step: 1, side: 1, kind: "raider", x: 300, y: 0 }];
const deathTime = theater.probe(loss, .01).fx.deaths[0].t;
assert.ok(!theater.probe(loss, deathTime - .001).input.bursts.some(b => b.type === "death"));
const death = theater.probe(loss, deathTime + .02, [0]);
assert.ok(death.input.bursts.some(b => b.type === "death")); assert.equal(death.ships[1].rootAlpha, 0);
assert.ok(!death.ships[1].pointable(0, 0), "a destroyed hull cannot be pointed at");
assert.equal(theater.probe(loss, deathTime + .02, [0], null, 1.3, true, [[1, "pointerover"]]).ships[1].hullMeter, false, "…nor show a meter");
assert.equal(JSON.stringify(record), original, "presentation never mutates the battle record");

// Recorded torpedoes fly their real paths across rounds: no snapping back to
// a salvo centroid, the real speed, launches from their launcher, hits on the
// hull the record names, interceptions by the hull that made them.
{
  const pos = { 1: [-400, 0], 2: [300, 0], 3: [150, 40], 4: [-400, 100] };
  const ship = (cid, side, kind, hp = 1) => ({ cid, side, kind, x: pos[cid][0], y: pos[cid][1], hp });
  const fleet = (hp) => [ship(1, 0, "destroyer"), ship(4, 0, "raider"), ship(2, 1, "cruiser", hp), ship(3, 1, "corvette")];
  const flight = { 7: [[-400, 0], [-260, 0], [-120, 0], [20, 0], [160, 0]], 9: [[-400, 100], [-262, 80], [-124, 60], [14, 40]],
    11: [[-400, 0], [-400, -40], [-400, -40], [-400, -40]] };
  const torpsAt = (r) => Object.entries(flight).flatMap(([id, path]) => (r >= 1 && r - 1 < path.length
    ? [{ id: Number(id), side: 0, to: 2, x: path[r - 1][0], y: path[r - 1][1] }] : []));
  const ends = { 5: [{ id: 9, side: 0, to: 2, x: 150, y: 20, end: "intercepted", by: 3 }, { id: 11, side: 0, to: 2, x: -330, y: -60, end: "wreck" }],
    6: [{ id: 7, side: 0, to: 2, x: 290, y: 0, end: "hit", damage: 120 }] };
  const counts = [[{ kind: "destroyer", exact: 1 }, { kind: "raider", exact: 1 }], [{ kind: "cruiser", exact: 1 }, { kind: "corvette", exact: 1 }]];
  const fish = { id: "recorded-torpedoes", own_side: 0,
    sides: [{ corp: "a", loadouts: [{ kind: "destroyer", modules: ["torpedo_rack"], n: 1 }, { kind: "raider", modules: ["torpedo_rack"], n: 1 }] },
      { corp: "d", loadouts: [{ kind: "corvette", modules: ["point_defense_screen"], n: 1 }, { kind: "cruiser", modules: [], n: 1 }] }],
    rounds: Array.from({ length: 8 }, (_, r) => ({ tick: r + 1, dealt: [r === 6 ? 120 : 0, 0], counts, notes: [],
      frame: { ships: fleet(r >= 6 ? 0.9 : 1), gunfire: [], deaths: [], torps: torpsAt(r), torp_ends: ends[r] ?? [],
        torpedoes: torpsAt(r).length ? [{ side: 0, x: 0, y: 0, n: torpsAt(r).length }] : [] } })) };
  const at = (r, f) => theater.torpProbe(fish, r, f);
  const head = (r, f, id) => at(r, f).torps.find((t) => t.id === id)?.head ?? null;
  for (let r = 1; r <= 4; r++) for (const id of [7, 9]) {
    const end = head(r, 1, id), start = head(r + 1, 0, id);
    if (!end || !start) continue;
    assert.ok(Math.hypot(end[0] - start[0], end[1] - start[1]) < 1e-6, `torpedo ${id} carries on from round ${r} without snapping back`);
  }
  const [a0, a1] = [head(2, 0, 7), head(2, 1, 7)];
  assert.ok(Math.abs(Math.hypot(a1[0] - a0[0], a1[1] - a0[1]) - 140) < 1e-6, "a torpedo flies 140 a round, its real speed");
  assert.ok(Math.abs(head(2, .5, 7)[0] + 190) < 1e-6, "…in a straight line between its recorded positions");
  const launch = at(0, .95).torps.find((t) => t.id === 7);
  assert.equal(launch.end, "launch");
  assert.equal(at(0, .95).cids[launch.from], 1, "it launches from the destroyer");
  assert.equal(head(0, .1, 7), null, "nothing flies before it is launched");
  assert.ok(Math.hypot(launch.head[0] + 400, launch.head[1]) < 1e-6, "…and it rides the launcher until it leaves");
  const intercept = at(4, .5).torps.find((t) => t.id === 9);
  assert.equal(intercept.end, "intercepted");
  assert.equal(at(4, .5).cids[intercept.by], 3, "shot down by the corvette that did it");
  assert.ok(Math.abs(intercept.tEnd - Math.min(.95, Math.hypot(150 - 14, 20 - 40) / 140)) < 1e-9, "…when it reaches that point at its real speed");
  assert.deepEqual(plain(at(4, .5).pd), [3], "only the PD-fitted hull screens");
  assert.ok(at(4, intercept.tEnd + .01).bursts.some((b) => b.id === "torp:9" && b.type === "flak"), "a flak burst where it died");
  const strike = at(5, .5).torps.find((t) => t.id === 7);
  assert.equal(strike.end, "hit");
  const cruiser = at(5, .5).cids.indexOf(2);
  assert.ok(at(5, .5).hits[cruiser].includes(strike.tEnd), "the hit is the cruiser's evidenced impact");
  const struck = at(5, strike.tEnd + .01), blast = struck.bursts.find((b) => b.id === "torp:7");
  const hull = struck.screen[cruiser]; // the recorded strike point sat 10 units short of the hull's centre
  assert.ok(blast && blast.type === "impact" && Math.hypot(blast.x - (hull[0] - 10 * struck.pxPerUnit), blast.y - hull[1]) < 1e-3,
    "the blast lands on the cruiser");
  assert.equal(head(5, strike.tEnd + .05, 7), null, "and the torpedo is gone");
  assert.equal(at(6, .5).torps.length, 0);
  assert.equal(at(3, .5).arcs, 0, "recorded frames never invent salvo arcs");
  const onWreck = at(4, .5).torps.find((t) => t.id === 11);
  assert.equal(onWreck.end, "wreck");
  assert.ok(at(4, onWreck.tEnd + .01).bursts.some((b) => b.id === "torp:11" && b.type === "impact"), "a torpedo that strikes wreckage detonates there");
  assert.deepEqual(plain(at(3, .5)), plain(at(3, .5)), "the same instant replays the same torpedoes");
  const legacy = structuredClone(fish);
  for (const r of legacy.rounds) { delete r.frame.torps; delete r.frame.torp_ends; }
  assert.ok(theater.torpProbe(legacy, 2, .5).arcs > 0 && theater.torpProbe(legacy, 2, .5).torps.length === 0,
    "salvo-only frames keep the cosmetic arcs");
}

// Rules v9: large-hull wreckage is the engine's. A battleship's three sections
// stay one hull until the engine breaks them (the blast meets them), then each
// moves exactly as recorded; a hull that rams wreckage takes a visible impact.
{
  const sideA = { cid: 1, side: 0, kind: "battleship", x: -300, y: 0, hp: 1 };
  const foe = { cid: 2, side: 1, kind: "raider", x: 300, y: 0, hp: 1 };
  const counts = [[{ kind: "battleship", exact: 1 }], [{ kind: "raider", exact: 1 }]];
  const spread = [0, 0, 8, 14, 18]; // the bow and stern fly apart from round 2
  const wrecksAt = (r) => (r < 1 ? undefined : [0, 1, 2].map((piece) => ({ hull: 1, piece,
    x: -290 + 70 * (1 - piece) + (piece === 0 ? spread[r] : piece === 2 ? -spread[r] : 0), y: piece === 1 ? 2 * Math.min(1, r - 1) : 0, r: 40 })));
  const rec = { id: "v9-wreckage", own_side: 0, sides: [{ corp: "a", loadouts: [] }, { corp: "b", loadouts: [] }],
    rounds: Array.from({ length: 5 }, (_, r) => ({ tick: r + 1, dealt: [0, r === 1 ? 8000 : 0], counts, notes: [],
      frame: { ships: r === 0 ? [sideA, foe] : [{ ...foe, hp: r >= 3 ? 0.8 : 1 }], torpedoes: [], torps: [], torp_ends: [],
        gunfire: r === 1 ? [{ side: 1, from: 2, to: 1, weapon: "beam", damage: 8000 }] : [],
        deaths: r === 1 ? [{ step: 1, side: 0, kind: "battleship", x: -290, y: 0, cid: 1 }] : [],
        wrecks: wrecksAt(r), rams: r === 3 ? [{ cid: 2, hull: 1, piece: 0, x: -150, y: 0, damage: 40 }] : [] } })) };
  const at = (t) => theater.afterProbe(rec, Math.floor(t), t - Math.floor(t));
  const event = at(.5).timeline[0];
  assert.ok(event.cid === 1 && event.cls === 2 && event.pieces === 3);
  assert.equal(event.breachT, 1, "it breaks when the engine breaks it");
  const whole = at(.97).wrecks;
  assert.equal(whole.length, 3);
  assert.ok(whole.every((w) => w.offset.every((v) => Math.abs(v) < 1e-9)), "until then the sections are one hull");
  const later = at(2), bow = later.wrecks.find((w) => w.chunk === 0);
  const scale = event.px / 2.24 * later.zoomScale; // arena units per model unit
  // The bow's recorded displacement from the hull it was part of (heading +x).
  const centre = [(-290 + 70 + 8 - 290 - 290 - 70 - 8) / 3, 2 / 3];
  const dx = (-290 + 70 + 8 - centre[0]) - 70, dy = (0 - centre[1]) - 0;
  assert.ok(Math.abs(bow.offset[0] - dy / scale) < 1e-6 && Math.abs(bow.offset[2] + dx / scale) < 1e-6,
    "after the breach each section sits where the engine recorded it");
  assert.ok(bow.offset[2] < 0 && later.wrecks.find((w) => w.chunk === 2).offset[2] > 0, "bow forward, stern aft");
  const struck = at(2.5), raider = struck.cids.indexOf(2); // round 3's ram plays in the window ending there
  assert.equal(struck.rams.length, 1);
  assert.ok(struck.hits[raider].includes(struck.rams[0].t), "a ram is the rammed hull's impact");
  assert.ok(at(2 + struck.rams[0].t + .01).ramBursts.length === 1, "with a flash where it struck");
}

// Rules v11: a held hull's tacklers and a web's target are drawn as links
// between the two hulls, from the evidence of the step that ends the window.
{
  const runner = { cid: 1, side: 1, kind: "battleship", x: 600, y: 0, hp: 1 };
  const tackler = { cid: 2, side: 0, kind: "raider", x: 450, y: 60, hp: 1 };
  const counts = [[{ kind: "raider", exact: 1 }], [{ kind: "battleship", exact: 1 }]];
  const rec = { id: "v11-links", own_side: 0, sides: [{ corp: "a", loadouts: [] }, { corp: "b", loadouts: [] }],
    rounds: Array.from({ length: 3 }, (_, r) => ({ tick: r + 1, dealt: [0, 0], counts, notes: [],
      frame: { ships: [{ ...tackler, x: 450 + 20 * r }, { ...runner, x: 600 + 20 * r }], torpedoes: [], torps: [], torp_ends: [],
        gunfire: [], deaths: [], tackles: r >= 1 ? [{ from: 2, to: 1 }] : [], webs: r === 2 ? [{ from: 2, to: 1 }] : [] } })) };
  const first = theater.afterProbe(rec, 0, .5), second = theater.afterProbe(rec, 1, .5);
  const own = (v) => JSON.parse(JSON.stringify(v)); // across the vm realm
  assert.deepEqual(own(first.tackles), [[2, 1]], "round 1's hold plays in the window ending there");
  assert.deepEqual(own(first.webs), []);
  assert.deepEqual(own(second.webs), [[2, 1]]);
  const beams = (p) => p.light.filter((s) => s.tex === "beam");
  assert.ok(beams(second).length >= 2, "a tackle line and a web line");
  const from = second.ships.find((v) => v.cid === 2);
  assert.ok(beams(second).every((b) => Math.hypot(b.x - from.x, b.y - from.y) < 2),
    "each link starts on screen at the tackler");
  const legacy = structuredClone(rec);
  for (const r of legacy.rounds) { delete r.frame.tackles; delete r.frame.webs; }
  assert.deepEqual(own(theater.afterProbe(legacy, 1, .5).tackles), [], "older frames draw no links");
}

// The view angle is a camera control: the same record keeps the same truth.
const diagonal = structuredClone(record); diagonal.rounds[1].frame.ships[0] = { ...a, x: -250, y: 50 };
const angledView = theater.probe(diagonal, .9), topDown = theater.probe(diagonal, .9, [], null, 1.3, false);
assert.equal(angledView.hash, topDown.hash, "the view angle cannot change combat presentation truth");
assert.ok(angledView.ships.every(s => Math.abs(s.squash - .8) < 1e-12) && topDown.ships.every(s => s.squash === 1),
  "flat art lies on the tilted plane");
assert.ok(angledView.ships[0].depth > angledView.ships[1].depth, "the nearer (lower) hull draws over the farther one");
const rise = v => v.input.ships[0].y - v.input.ships[1].y;
assert.ok(rise(topDown) > 0 && Math.abs(rise(angledView) - .8 * rise(topDown)) < 1e-9, "the plane foreshortens vertically only");
assert.equal(angledView.input.ships[0].x, topDown.input.ships[0].x);
const flatHeading = topDown.input.ships[0].angle;
assert.ok(Math.abs(flatHeading) > .1 && Math.abs(flatHeading) < 1.4, "fixture flies a diagonal");
assert.ok(Math.abs(angledView.input.ships[0].angle - Math.atan2(Math.sin(flatHeading) * .8, Math.cos(flatHeading))) < 1e-9,
  "3D hulls are posed at the projected heading");
assert.deepEqual([topDown.input.pitch, topDown.input.ringTilt, topDown.input.depthSlope], [-Math.PI / 10, -.18, 0],
  "top-down leans the same way as the angled view: seen from the lower edge");
// The follow camera: a battle opens on the overview, following the player's
// heaviest hull; zooming in centres on it; when it falls, the camera follows
// the next and glides there rather than jumping.
{
  const own = (cid, kind, x, y) => ({ cid, side: 0, kind, x, y, hp: 1 });
  const foe = { cid: 9, side: 1, kind: "raider", x: 400, y: 100, hp: 1 };
  const frameOf = (ships) => ({ ships, gunfire: [], deaths: [], torpedoes: [] });
  const counts = [[{ kind: "cruiser", exact: 1 }, { kind: "raider", exact: 1 }], [{ kind: "raider", exact: 1 }]];
  const fight = { id: "follow-proof", sides: [{ corp: "own", loadouts: [] }, { corp: "foe", loadouts: [] }], own_side: 0,
    rounds: [0, 1, 2].map((k) => ({ tick: k, dealt: [0, 0], counts, notes: [], frame: frameOf([
      own(1, "raider", -500, -200 + 20 * k), own(2, "cruiser", -600 + 30 * k, 300), foe]) })) };
  const opening = theater.followProbe(fight, 0, .5, null);
  assert.equal(opening.cid, 2, "follows the player's heaviest hull");
  assert.deepEqual(plain(opening.cam), [0, 0], "opening on the overview: the whole battle in view");
  assert.equal(opening.zoom, opening.overview);
  const close = theater.followProbe(fight, 0, .5, 9);
  assert.ok(Math.hypot(close.cam[0] - close.at[0], close.cam[1] - close.at[1]) < 1e-9, "zoomed in, the camera centres on it");
  const mid = theater.followProbe(fight, 0, .5, 1.5);
  assert.ok(Math.abs(mid.cam[0]) < Math.abs(mid.at[0]) && Math.abs(mid.cam[0]) > 0, "between, it leans toward it inside the overview's frame");
  // The cruiser is gone from the next window: follow the raider, gliding.
  const lost = structuredClone(fight);
  for (const r of lost.rounds.slice(1)) r.frame.ships = r.frame.ships.filter((v) => v.cid !== 2);
  const glide = theater.followProbe(lost, 1, .2, 9);
  assert.equal(glide.cid, 1, "the next heaviest of the player's hulls");
  const late = glide.step(60);
  assert.ok(Math.hypot(late.cam[0] - late.at[0], late.cam[1] - late.at[1]) < 1e-6, "…reached within the glide");
  const watched = theater.followProbe({ ...fight, own_side: null }, 0, .5, 9);
  assert.equal(watched.cid, null, "a battle the player only watches follows no one");
}

// Picking: a click selects the nearest hull within reach (22 px, or its own
// drawn size), so a distant interceptor is easy to hit; empty space selects
// nothing. The selection reads as the info card shows it.
{
  const ships = [{ cid: 1, side: 0, kind: "raider", x: -600, y: 0, hp: .8 }, { cid: 2, side: 1, kind: "cruiser", x: 400, y: 200, hp: 1 }];
  const counts = [[{ kind: "raider", exact: 1 }], [{ kind: "cruiser", exact: 1 }]];
  const fight = { id: "pick-proof", sides: [{ corp: "own", loadouts: [{ kind: "raider", n: 1, modules: ["mass_driver", "mass_driver"] }] },
    { corp: "foe", loadouts: [] }], own_side: 0,
    rounds: [0, 1].map((k) => ({ tick: k, dealt: [0, 0], counts, notes: [], frame: { ships, gunfire: [], deaths: [], torpedoes: [] } })) };
  const near = theater.pickProbe(fight, 0, .3, 1.3, (at) => [at(1)[0] + 15, at(1)[1] - 10]);
  assert.equal(near.cid, 1, "a click 18 px off a distant interceptor still picks it");
  assert.equal(near.following, true, "…and follows it");
  assert.deepEqual(plain({ ...near.info, art: undefined }), { cid: 1, side: 0, own: true, kind: "raider", label: near.info.label,
    hull: .8, reps: 1, station: 0, plat: false, fits: [["mass_driver", "mass_driver"]], flagship: null, status: "fighting", following: true });
  const empty = theater.pickProbe(fight, 0, .3, 1.3, [5, 5]);
  assert.equal(empty.cid, null, "a click on empty space selects nothing");
  assert.equal(empty.info, null);
}

// The frame-rate ladder: 60 fps on a desktop stage; a stage that can't hold
// it drops to 30 before any effect is cut, then tries 60 again after 20 s,
// waiting twice as long after each failed try. Phones keep 30 and shed effects.
{
  const frames = (n, fps) => Array(n).fill(1 / fps);
  assert.deepEqual(plain(theater.paceProbe(60, frames(600, 60)).at(-1).slice(0, 2)), [60, 0], "a stage that holds 60 keeps it");
  const slow = theater.paceProbe(60, frames(120, 40));
  assert.deepEqual(plain(slow.at(-1).slice(0, 2)), [30, 0], "can't hold 60: steps down to 30");
  assert.ok(slow.every(([, tier]) => tier === 0), "…before cutting a single effect");
  const run = theater.paceProbe(60, [...frames(120, 40), ...frames(30 * 70, 30)]);
  const tries = run.flatMap(([cap], i) => i > 0 && cap === 60 && run[i - 1][0] === 30 ? [run[i][2]] : []);
  assert.equal(tries.length, 2, "tries 60 again, twice in 70 s");
  assert.ok(Math.abs(tries[0] - (10 + 12 / 40 + 20)) < .05, `the first try comes 20 s after the drop (${tries[0]})`);
  assert.ok(tries[1] - tries[0] > 40 && tries[1] - tries[0] < 41, "a failed try doubles the wait");
  assert.ok(run.every(([, tier]) => tier === 0), "a stage that holds 30 never loses an effect");
  const phone = theater.paceProbe(30, frames(120, 18));
  assert.deepEqual(plain(phone.at(-1).slice(0, 2)), [30, 3], "a phone stage keeps 30 and sheds effects when even 30 is too much");
}

// Toggling the view eases one steady lean: hulls and rings never roll back
// through flat to the far side on the way (they did when top-down leaned +18°).
let lean = theater.poseAt(1);
for (let t = .99; t >= .8 - 1e-9; t -= .01) {
  const next = theater.poseAt(t);
  assert.ok(next[0] < 0 && next[1] < 0 && next[0] <= lean[0] + 1e-12 && next[1] <= lean[1] + 1e-12,
    `the toggle leans steadily (view ${t.toFixed(2)}: ${next.map(v => v.toFixed(3))})`);
  lean = next;
}
assert.ok(Math.abs(angledView.input.pitch + plane) < 1e-12 && Math.abs(angledView.input.ringTilt + plane) < 1e-12
  && Math.abs(angledView.input.depthSlope - slope) < 1e-12, "angled hulls and rings lie in the plane");
const store = new Map();
const localStorage = { getItem: k => store.get(k) ?? null, setItem: (k, v) => store.set(k, String(v)) };
const remembered = compile("battletheater", theaterDeps, { localStorage });
assert.equal(remembered.theaterViewAngled(), true, "angled by default");
remembered.theaterSetView(false);
assert.equal(store.get("ss_theater_view"), "topdown");
assert.equal(compile("battletheater", theaterDeps, { localStorage }).theaterViewAngled(), false, "the viewer's choice is remembered");
assert.equal(compile("battletheater", theaterDeps, { localStorage: { getItem() { throw new Error("blocked"); } } }).theaterViewAngled(), true,
  "blocked storage falls back to the angled view");

// Curved tracks: exact at every recorded keyframe, velocity carried smoothly
// across rounds, no overshoot when a hull stops, headings along the curve.
const lap = structuredClone(record); // the cruiser bends north, then holds
const at = (x, y, hp = 1) => ({ ...a, x, y, hp });
lap.rounds = [0, 1, 2, 3, 4].map((i) => ({ ...structuredClone(record.rounds[1]), tick: i + 1,
  frame: { ships: [[at(-300, 0), at(-250, 0), at(-205, 25), at(-180, 70), at(-180, 70)][i], { ...b }],
    gunfire: [], deaths: [], torpedoes: [] } }));
const track = (round, frac) => theater.trackProbe(lap, round, frac)[0];
const near = (p, x, y) => Math.hypot(p.x - x, p.y - y) < 1e-9;
assert.ok(near(track(1, 0), -250, 0) && near(track(1, 1), -205, 25) && near(track(2, 0), -205, 25), "keyframes are exact");
const endOf1 = track(1, 1), startOf2 = track(2, 0);
assert.ok(Math.hypot(endOf1.vx - startOf2.vx, endOf1.vy - startOf2.vy) < 1e-9, "velocity carries across the keyframe");
assert.ok(track(1, .5).speed > 40, "a moving hull keeps moving through mid-round");
const angleGap = (p, q) => Math.abs(Math.atan2(Math.sin(p - q), Math.cos(p - q)));
assert.ok(angleGap(track(1, .5).heading, Math.atan2(track(1, .5).vy, track(1, .5).vx)) < 0.15, "the hull points along its curve");
for (let f = 0; f <= 1; f += 0.05) {
  const p = track(2, f);
  assert.ok(p.y <= 70 + 1e-9 && p.x <= -180 + 1e-9, "stopping at the next keyframe never overshoots it");
}
assert.ok(near(track(3, .5), -180, 70) && track(3, .5).speed === 0, "a holding hull stays put");

// Facing: hulls come about at their class's turn rate and hold their facing
// while station-keeping (the engine records positions only).
const course = (kind, xs, ys = xs.map(() => 0)) => ({ ...structuredClone(record), rounds: xs.map((x, i) => ({
  ...structuredClone(record.rounds[1]), tick: i + 1,
  frame: { ships: [{ ...a, kind, x, y: ys[i] }, { ...b, x: 300, y: 0 }], gunfire: [], deaths: [], torpedoes: [] } })) });
const outAndBack = Array.from({ length: 48 }, (_, i) => (i <= 4 ? -300 + i * 20 : -220 - (i - 4) * 20));
const facings = (rec) => Array.from({ length: rec.rounds.length - 1 }, (_, r) => theater.trackProbe(rec, r, 0)[0].heading);
const titanTurn = facings(course("titan", outAndBack));
for (let r = 1; r < titanTurn.length; r++) {
  assert.ok(angleGap(titanTurn[r], titanTurn[r - 1]) <= 0.085 + 1e-9, "a titan comes about at most 0.085 rad a round");
}
assert.ok(Math.cos(titanTurn[14]) > 0.5, "ten rounds after reversing, a titan still faces mostly the old way");
assert.ok(Math.cos(titanTurn[45]) < -0.99, "…and has come fully about some 37 rounds later");
assert.ok(Math.cos(facings(course("raider", outAndBack))[7]) < -0.99, "an interceptor reverses within a couple of rounds");
for (let r = 1; r < titanTurn.length; r++) {
  const mid = theater.trackProbe(course("titan", outAndBack), r - 1, .5)[0].heading;
  assert.ok(angleGap(mid, titanTurn[r - 1]) <= 0.0425 + 1e-9, "the turn is spread evenly through the round");
}
const jitter = Array.from({ length: 12 }, (_, i) => -300 + (i % 2 ? 0.6 : -0.6));
const station = facings(course("battleship", jitter, jitter.map((_, i) => (i % 3) * 0.4)));
assert.ok(station.every((h) => h === station[0]), "a hull holding station keeps its facing");
assert.ok(Math.cos(station[0]) > 0.99, "…facing the enemy it first saw");

// §orbital-defense: a station's hardpoints (rules v15) stand on a ring over
// its art rather than in the engine's line, never move, carry no hull art,
// and go up like a heavy hull when knocked out.
{
  const mount = (cid, y) => ({ cid, side: 1, kind: "corvette", plat: true, station: 3, x: 400, y, hp: 1 });
  const line = [mount(10, -20), mount(11, 0), mount(12, 20)];
  const raider = { cid: 1, side: 0, kind: "raider", x: -300, y: 0, hp: 1 };
  const stationCounts = [[{ kind: "raider", exact: 1 }], []];
  const fort = { id: "station-ring", sides: [{ corp: "own", loadouts: [] }, { corp: "foe", loadouts: [] }], own_side: 0, rounds: [
    { tick: 1, dealt: [0, 0], counts: stationCounts, notes: [], frame: { ships: [raider, ...line], gunfire: [], deaths: [], torpedoes: [] } },
    { tick: 2, dealt: [0, 0], counts: stationCounts, notes: [], frame: { ships: [{ ...raider, x: -200 }, ...line], gunfire: [], deaths: [], torpedoes: [] } },
    { tick: 3, dealt: [900, 0], counts: stationCounts, notes: [], frame: { ships: [{ ...raider, x: -150 }, line[0], line[2]], gunfire: [],
      deaths: [{ step: 2, side: 1, kind: "corvette", x: 400, y: 0, cid: 11 }], torpedoes: [] } }] };
  const at = (round, frac) => theater.trackProbe(fort, round, frac).filter((v) => v.cid >= 10);
  const ring = at(0, .3);
  assert.equal(ring.length, 3);
  for (const v of ring) assert.ok(Math.abs(Math.hypot(v.x - 400, v.y) - 130 * 0.26) < 1e-6, "on a ring over the Defense Station's art");
  assert.ok(Math.abs(ring[0].x - 400) < 1e-6 && ring[0].y < 0, "the first hardpoint at the top");
  assert.deepEqual(plain(at(1, .8).map((v) => [v.x, v.y])), plain(ring.map((v) => [v.x, v.y])), "hardpoints never move");
  const shown = theater.probe(fort, .3).ships.filter((v, i) => i > 0);
  assert.ok(shown.every((v) => !v.eligible), "no hull model for a hardpoint");
  const blow = theater.afterProbe(fort, 1, .01).timeline.find((e) => e.cid === 11);
  assert.ok(blow && blow.cls === 1 && blow.pieces === 0 && blow.px === 6, "a hardpoint goes up like a heavy hull, in one piece");
}

// Deaths: the blast is where the hull is seen to die; debris follows and
// outlives its round; capitals burn, cook off and only then break; a titan's
// reactor lights the battlefield; wrecks linger past the record; past eight
// capital wrecks the oldest fades. All seeded on replay time.
const lossRec = { ...structuredClone(loss), id: "afterlife-loss" };
const blow = theater.afterProbe(lossRec, 0, .01).timeline[0];
assert.ok(blow && blow.cls === 0 && blow.pieces === 0, "an interceptor shatters at the blow");
const seen = theater.afterProbe(lossRec, 0, blow.T - 1e-4).ships.find((v) => v.cid === 2);
const blast = theater.afterProbe(lossRec, 0, blow.T + .02).bursts.find((b) => b.id === `death:${blow.key}`);
assert.ok(blast && Math.hypot(blast.x - seen.x, blast.y - seen.y) < 3, "the blast is where the hull was seen to die");
const debrisOf = (probe) => probe.debris.filter((sp) => sp.tex === "plate" || sp.tex === "strut");
assert.equal(debrisOf(theater.afterProbe(lossRec, 0, blow.T - 1e-4)).length, 0, "no debris before the blow");
const thrown = theater.afterProbe(lossRec, 0, Math.min(.99, blow.T + .4));
assert.ok(debrisOf(thrown).length > 0, "the blast throws debris");
assert.deepEqual(plain(theater.afterProbe(lossRec, 0, Math.min(.99, blow.T + .4))), plain(thrown), "the same instant draws the same wreckage");
assert.ok(debrisOf(theater.afterProbe(lossRec, 1, .6)).length > 0, "debris outlives the round it was thrown in");
// Once the hull's art has loaded, its debris is cut from that art: torn
// plates and spars of the hull itself, which tumble, and glow hot at first.
const artThrown = theater.afterProbe(lossRec, 0, Math.min(.99, blow.T + .4), true);
const cutOf = (probe) => probe.debris.filter((sp) => sp.tex.startsWith("debris-"));
assert.ok(!artThrown.smoke.some((sp) => sp.tex.startsWith("debris-")), "debris lies in the plane, beneath the hulls, not over them");
assert.ok(cutOf(artThrown).length > 0 && debrisOf(artThrown).length === 0, "debris is cut from the hull's own art");
assert.equal(cutOf(artThrown).length, debrisOf(thrown).length, "one cut piece for every fragment");
assert.ok(cutOf(artThrown).some((sp) => sp.tex.startsWith("debris-spar")) && cutOf(artThrown).some((sp) => sp.tex.startsWith("debris-plate")),
  "torn plates and long spars");
assert.deepEqual(plain(theater.afterProbe(lossRec, 0, Math.min(.99, blow.T + .4), true)), plain(artThrown), "the same instant shows the same pieces");
assert.ok(cutOf(artThrown).some((sp) => sp.h < sp.w * .95), "pieces tumble, foreshortening as they turn");
const red = (c) => c >> 16, blue = (c) => c & 255;
assert.ok(cutOf(theater.afterProbe(lossRec, 0, Math.min(.99, blow.T + .05), true)).every((sp) => red(sp.tint) > blue(sp.tint) + 40), "fresh pieces are hot");
const grey = (c) => c >> 16 === (c >> 8 & 255) && (c >> 8 & 255) === (c & 255);
const cooled = cutOf(theater.afterProbe(lossRec, 1, .6, true));
assert.ok(cooled.every((sp) => grey(sp.tint)), "…and cool to scorched metal");
assert.ok(new Set(cooled.map((sp) => sp.tint)).size > 1, "…lit by how each piece has turned");

const capital = (kind, id) => ({ ...structuredClone(record), id, rounds: Array.from({ length: 5 }, (_, i) => ({
  ...structuredClone(record.rounds[1]), tick: i + 1,
  frame: { ships: i === 0 ? [{ ...a, kind, x: -300, y: 0 }, { ...b }] : [{ ...b }],
    gunfire: i === 1 ? [{ side: 1, from: 2, to: 1, weapon: "beam", damage: 50 }] : [],
    deaths: i === 1 ? [{ step: 1, side: 0, kind, x: -290, y: 0 }] : [], torpedoes: [] } })) });
const probeAt = (rec, t) => theater.afterProbe(rec, Math.floor(t), t - Math.floor(t));
const bsRec = capital("battleship", "afterlife-battleship");
const bs = theater.afterProbe(bsRec, 0, .01).timeline[0];
assert.ok(bs.cls === 2 && Math.abs(bs.breachT - bs.T - .8) < 1e-9 && bs.pieces === 3);
const smoulder = probeAt(bsRec, bs.T + .3);
assert.ok(!smoulder.bursts.some((b) => b.id === `death:${bs.key}`), "a capital does not explode at the blow");
assert.equal(smoulder.wrecks.length, 3, "its hull stays whole, as three sections at rest");
assert.ok(smoulder.wrecks.every((p) => p.offset.every((v) => v === 0)), "…with no separation before the breach");
let cookOff = 0;
for (let t = bs.T + .02; t < bs.breachT; t += .04) cookOff += probeAt(bsRec, t).bursts.filter((b) => b.id.startsWith(`internal:${bs.key}:`)).length;
assert.ok(cookOff > 0, "it cooks off before it breaks");
assert.ok(probeAt(bsRec, bs.breachT + .05).bursts.some((b) => b.id === `death:${bs.key}` && b.type === "death"), "then the breach");
const apart = probeAt(bsRec, bs.breachT + 1.5);
assert.ok(apart.wrecks.some((p) => Math.hypot(...p.offset) > .05), "the sections separate");
assert.ok(apart.wrecks.every((p) => p.char > smoulder.wrecks[0].char), "…burning darker");

const titanRec = capital("titan", "afterlife-titan");
const titan = theater.afterProbe(titanRec, 0, .01).timeline[0];
assert.ok(titan.cls === 3 && Math.abs(titan.breachT - titan.T - 1.8) < 1e-9 && titan.pieces === 5);
const washOf = (probe) => probe.light.some((sp) => sp.tex === "soft" && sp.w > probe.canvas[0] * 2);
assert.ok(washOf(probeAt(titanRec, titan.breachT + .02)), "a titan's reactor lights the whole battlefield");
assert.ok(!washOf(probeAt(titanRec, titan.breachT - .05)), "…only once it lets go");
assert.ok(theater.afterProbe(titanRec, titanRec.rounds.length + 1, .5).wrecks.length > 0, "past the record the wreck lingers (the players' epilogue)");

const fleetRec = { ...structuredClone(record), id: "afterlife-fleet", rounds: Array.from({ length: 12 }, (_, i) => ({
  ...structuredClone(record.rounds[1]), tick: i + 1,
  frame: { ships: [...Array.from({ length: 9 }, (_, k) => k).filter((k) => k >= i)
    .map((k) => ({ ...a, cid: 10 + k, kind: "battleship", x: -300, y: (k - 4) * 70 })), { ...b }],
    gunfire: i >= 1 && i <= 9 ? [{ side: 1, from: 2, to: 9 + i, weapon: "beam", damage: 50 }] : [],
    deaths: i >= 1 && i <= 9 ? [{ step: 1, side: 0, kind: "battleship", x: -300, y: (i - 5) * 70 }] : [], torpedoes: [] } })) };
const settled = theater.afterProbe(fleetRec, 13, .5);
assert.equal(settled.timeline.filter((e) => e.cls === 2).length, 9);
assert.ok(!settled.wrecks.some((p) => p.id.startsWith(`${settled.timeline[0].key}:`)), "past eight capital wrecks the oldest fades away");
assert.ok(settled.wrecks.some((p) => p.id.startsWith(`${settled.timeline[1].key}:`)), "…and the rest remain");

// Effects are cosmetic and seeded: live sprites change nothing the hash
// covers, and the same instant always draws the same sprites.
const fxAt = hitTime + 0.02; // the cruiser's bolt has just left its barrel
const lit = theater.fxProbe(record, fxAt);
assert.equal(lit.hash, theater.probe(record, fxAt).hash, "effects cannot change combat presentation truth");
assert.deepEqual(plain(theater.fxProbe(record, fxAt)), plain(lit), "the same instant draws identical effect sprites");
assert.ok(lit.light.length > 0 && lit.light.every((s) => s.blend === "add"), "light effects blend additively");
assert.ok(lit.ships.every((s) => s.blends.every((b) => b === "add")), "engine flames blend additively");
// The bolt leaves one of the cruiser's barrels, not the middle of its hull.
const flash = lit.light.find((s) => s.tex === "flare");
assert.ok(flash, "a muzzle flash marks the firing barrel");
const shooter = lit.ships[0];
const offset = Math.hypot(flash.x - shooter.x, flash.y - shooter.y);
assert.ok(offset > 0.5 && offset < 40, "the muzzle sits on the hull, off its centre");
// Engines: a hull moving along its track burns harder than one holding
// station, and a withdrawing side burns flat out.
const holding = structuredClone(record);
holding.rounds[1].frame.ships = [{ ...a }, { ...b, hp: .8 }];
const flame = (probe, i) => Math.max(...probe.ships[i].flames);
const cruising = structuredClone(record); // 20 units a round: well under the cruiser's top speed
cruising.rounds[1].frame.ships = [{ ...a, x: -280 }, { ...b, x: 280, hp: .8 }];
const still = theater.fxProbe(holding, .5), moving = theater.fxProbe(cruising, .5);
assert.ok(moving.ships.every((s) => s.engines) && still.ships.every((s) => s.engines));
assert.ok(flame(moving, 0) > flame(still, 0) * 1.5, "a moving hull's plumes are longer");
assert.ok(flame(theater.fxProbe(cruising, .5, [0, Infinity]), 0) > flame(moving, 0) * 1.3, "withdrawal burns flat out");
// Damage: a hull below 30% vents smoke and burns; an intact one does not.
const burning = structuredClone(record);
for (const round of burning.rounds) round.frame.ships[1].hp = .25;
const hurt = theater.fxProbe(burning, .5);
assert.ok(hurt.smoke.some((s) => s.tex === "puff"), "a badly damaged hull vents smoke");
assert.ok(hurt.light.some((s) => s.tex === "hot"), "a hull below 30% burns");
const intact = structuredClone(holding);
for (const round of intact.rounds) round.frame.ships[1].hp = 1;
const calm = theater.fxProbe(intact, .5);
assert.equal(calm.smoke.length, 0, "intact hulls trail no smoke");
assert.ok(!calm.light.some((s) => s.tex === "hot"), "intact hulls do not burn");
console.log("Battle models: budgets, real geometry/UVs, instancing, wreck sections, headings, tilted plane + depth order, view toggle, additive seeded effects, barrels, engine thrust, damage smoke, staged deaths + wrecks + art-cut debris, recorded torpedoes, recorded wreckage + rams, tackle + web links, hover hull meters, hit evidence, no-future-data, fallback and cleanup passed.");

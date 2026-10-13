// Exercise the real admission/transition/cache code without a browser. Only
// GPU and network boundaries are substituted; transforms use actual Three.js.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";

let now = 1000, nextTimer = 0;
const timers = new Map();
const clock = { setTimeout(fn, ms) { const id = ++nextTimer; timers.set(id, { fn, at: now + ms }); return id; },
  clearTimeout(id) { timers.delete(id); } };
function advance(ms) { now += ms; for (const [id, timer] of [...timers]) if (timer.at <= now) { timers.delete(id); timer.fn(); } }
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };
function compile(name, deps = {}, extra = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(readFileSync(new URL(`../src/${name}.ts`, import.meta.url), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText, { exports, require: name => {
    assert.ok(name in deps, `unexpected dependency ${name}`); return deps[name];
  }, performance: { now: () => now }, window: clock, console: { warn() {} }, AbortController, ...extra });
  return exports;
}
const catalog = compile("ship-models.generated");
const policy = compile("map-ship-model-policy", { "./ship-models.generated": catalog });
const candidate = (id, kind = "raider", more = {}) => ({ id, kind, x: 500, y: 400, canvasPx: 90,
  angle: -Math.PI / 2, alpha: .97, selected: false, escorts: 0, ...more });
assert.equal(policy.mapModelBlend(4), 0);
assert.equal(policy.mapModelBlend(12), 0);
assert.equal(policy.mapModelBlend(22), 1);
let previous = 0;
for (let r = 12; r <= 22; r += .01) {
  const blend = policy.mapModelBlend(r); assert.ok(blend >= previous); previous = blend;
}
assert.equal(policy.MAP_MODEL_TILT, -Math.acos(.8), "live hulls lean like the battle viewer's angled view (about 37 degrees)");
const crowd = Array.from({ length: 500 }, (_, i) => candidate(`fleet-${i}`, Object.keys(catalog.SHIP_MODELS)[i % 18],
  { canvasPx: 100 + i % 300, x: 50 + i % 900, escorts: i % 7 === 0 ? 6 : 0 }));
crowd[499].selected = true; crowd[499].canvasPx = 40;
const chosen = policy.pickMapModels(crowd, 1000, 800);
assert.equal(chosen[0].id, crowd[499].id, "selected visible ship gets first budget priority");
assert.ok(chosen.length <= policy.MAP_MODEL_SLOTS);
assert.ok(chosen.reduce((n, ship) => n + catalog.SHIP_MODELS[ship.kind].triangles * (1 + ship.escorts), 0) <= policy.MAP_MODEL_TRIANGLES);
assert.deepEqual(Array.from(policy.pickMapModels([...crowd].reverse(), 1000, 800), s => s.id), Array.from(chosen, s => s.id), "stable regardless of wire order");
for (const changed of [{ x: -1000 }, { x: 2000 }, { y: 2000 }, { alpha: 0 }, { x: NaN }, { canvasPx: 8 }]) {
  assert.equal(policy.pickMapModels([candidate("hidden", "raider", changed)], 1000, 800).length, 0);
}
assert.equal(policy.pickMapModels([candidate("edge", "raider", { x: -20 })], 1000, 800).length, 1, "partially visible hull remains eligible");
assert.equal(policy.pickMapModels([candidate("huge", "titan", { x: -1000, canvasPx: 3000 })], 1000, 800).length, 1, "oversized close-up crossing the viewport remains visible");
const closeChosen = policy.pickMapModels(crowd, 1000, 800, policy.MAP_MODEL_CLOSE_SLOTS);
assert.equal(closeChosen.length, 2);
assert.equal(closeChosen[0].id, crowd[499].id, "selection keeps first priority in the close-up budget");
assert.equal(policy.mapModelCloseUp([candidate("small", "raider", { canvasPx: 400 })], false, false), false);
assert.equal(policy.mapModelCloseUp([candidate("large", "titan", { canvasPx: 600 })], false, false), true);
assert.equal(policy.mapModelCloseUp([candidate("seam", "raider", { canvasPx: 450 })], false, true), true, "quality hysteresis holds its upgrade");
assert.equal(policy.mapModelCloseUp([candidate("small", "raider", { canvasPx: 300 })], false, true), false);

// Production bridge: no import at far zoom, poster retained until a real sample,
// monotonic crossfade, one atlas upload, no stale hull after removal/context loss.
let imports = 0, fakeRuntime;
class RuntimeBoundary {
  healthy = true; ready = false; canvas = { width: 3072, height: 1536 };
  constructor(narrow, closeUp = false) {
    Object.assign(this, { narrow, closeUp, columns: closeUp ? 2 : 4,
      slotCount: closeUp ? 2 : 8, tileSize: (narrow ? 384 : 768) * (closeUp ? 2 : 1) });
    fakeRuntime = this;
  }
  render(draws) { this.draws = draws; return this.ready; }
  sample(draw) { return this.ready ? draw : undefined; }
  cancelPending() { this.cancelled = true; }
  debug() { return { healthy: this.healthy }; }
  dispose() { this.disposed = true; }
}
class SourceBoundary { updates = 0; update() { this.updates++; } destroy() { this.destroyed = true; } }
class TextureBoundary { static EMPTY = {}; constructor(options) { Object.assign(this, options); } destroy() {} }
class RectangleBoundary { constructor(x, y, w, h) { Object.assign(this, { x, y, w, h }); } }
const moduleDependencies = {
  "pixi.js": { CanvasSource: SourceBoundary, Texture: TextureBoundary, Rectangle: RectangleBoundary },
  "./map-ship-model-policy": policy,
};
Object.defineProperty(moduleDependencies, "./map-ship-model-runtime", { get() { imports++; return { MapShipModelRuntime: RuntimeBoundary }; } });
const { MapShipModels } = compile("map-ship-models", moduleDependencies);
const bridge = new MapShipModels();
const sprite = { alpha: .62, destroyed: false };
const model = { visible: false, destroyed: false, scale: { set() {} }, anchor: { set() {} } };
const item = { ...candidate("own", "cruiser", { alpha: .62 }), sprite, model };
function frame(zoom, items = [item]) { bridge.beginFrame(); items.forEach(ship => bridge.add(ship)); bridge.endFrame(zoom, 1000, 800, now); advance(16); }
frame(4); await flush(); assert.equal(imports, 0, "normal map never imports 3D");
frame(24); await flush(); assert.equal(imports, 1);
frame(24); assert.equal(sprite.alpha, .62); assert.equal(model.visible, false, "loading cannot hide the sprite");
fakeRuntime.ready = true;
let oldAlpha = sprite.alpha;
for (let i = 0; i < 20; i++) { frame(24); assert.ok(sprite.alpha <= oldAlpha); oldAlpha = sprite.alpha; }
assert.equal(sprite.alpha, 0); assert.equal(model.alpha, .62, "served staleness alpha survives in 3D");
assert.equal(fakeRuntime.draws[0].tilt, policy.MAP_MODEL_TILT, "fully live hulls take the whole angled lean");
frame(17); assert.equal(fakeRuntime.draws[0].tilt, policy.mapModelBlend(17) * policy.MAP_MODEL_TILT,
  "the lean grows with the fade, starting from the flat top-down sprite");
frame(4); assert.equal(sprite.alpha, .62); assert.equal(model.visible, false);
frame(24); fakeRuntime.healthy = false; frame(24);
assert.equal(sprite.alpha, .62); assert.equal(model.visible, false); assert.ok(fakeRuntime.disposed);
advance(31_000); frame(24); await flush(); fakeRuntime.ready = true; frame(24);
frame(24, []); assert.equal(model.visible, false, "removed or hidden ghosts disappear immediately");
advance(policy.MAP_MODEL_IDLE_MS + 1); assert.ok(fakeRuntime.disposed, "idle renderer is released");

const largeItem = { ...item, canvasPx: 1600 };
frame(384, [largeItem]); await flush();
assert.equal(fakeRuntime.closeUp, true);
assert.equal(fakeRuntime.tileSize, 1536);
frame(384, [largeItem]);
assert.equal(sprite.alpha, .62); assert.equal(model.visible, false, "quality upgrade retains artwork until the replacement is ready");
fakeRuntime.ready = true;
for (let i = 0; i < 20; i++) frame(384, [largeItem]);
assert.equal(model.visible, true); assert.equal(model.texture.frame.w, 1536);
const largeRuntime = fakeRuntime;
frame(24); await flush();
assert.ok(largeRuntime.disposed, "zooming back releases high-resolution hulls and their context");
assert.equal(fakeRuntime.closeUp, false); assert.equal(fakeRuntime.slotCount, 8);
bridge.suspend();

// Production runtime: real Three groups/cameras/clones with a recording GPU.
const requests = [], disposed = [], gpuFrames = [];
const canvas = { width: 1, height: 1, listeners: new Map(), addEventListener(n, cb) { this.listeners.set(n, cb); }, removeEventListener(n) { this.listeners.delete(n); } };
class RecordingRenderer {
  info = { render: { triangles: 0, calls: 0 }, reset() { this.render.triangles = this.render.calls = 0; } };
  setPixelRatio() {} setClearColor() {} setScissorTest() {} clear() {} setScissor() {}
  setSize(w, h) { canvas.width = w; canvas.height = h; }
  setViewport(x, y, w, h) { this.viewport = { x, y, w, h }; }
  render(scene, camera) {
    scene.updateMatrixWorld(true); camera.updateMatrixWorld(true);
    let meshes = 0;
    scene.traverseVisible(object => { if (object instanceof THREE.Mesh) { meshes++; this.info.render.calls++; this.info.render.triangles += object.geometry.index.count / 3; } });
    const active = scene.children.find(child => child.visible);
    const lead = active.children[0].children.at(-1);
    const front = new THREE.Vector3(0, 0, -1).applyMatrix4(lead.matrixWorld).project(camera);
    const back = new THREE.Vector3(0, 0, 1).applyMatrix4(lead.matrixWorld).project(camera);
    const box = new THREE.Box3().setFromObject(active);
    const up = new THREE.Vector3(0, 1, 0).transformDirection(lead.matrixWorld);
    gpuFrames.push({ viewport: this.viewport, meshes, angle: Math.atan2(-(front.y - back.y), front.x - back.x), up,
      fits: Math.max(Math.abs(box.min.x), Math.abs(box.max.x), Math.abs(box.min.z), Math.abs(box.max.z)) < camera.right });
  }
  dispose() {} forceContextLoss() {}
}
const stage = {
  modelLighting: () => ({ scene: new THREE.Scene(), dispose() {} }),
  disposeModel: model => disposed.push(model),
  loadShipModel: (kind, signal) => new Promise((resolve, reject) => requests.push({ kind, signal, resolve, reject })),
};
const { MapShipModelRuntime } = compile("map-ship-model-runtime", {
  "three": { ...THREE, WebGLRenderer: RecordingRenderer }, "./ship-model-stage": stage, "./map-ship-model-policy": policy,
}, { document: { createElement: () => canvas } });
const hull = () => { const g = new THREE.Group(); g.add(new THREE.Mesh(new THREE.BoxGeometry(.8, .4, 2), new THREE.MeshStandardMaterial())); return g; };
const runtime = new MapShipModelRuntime(false);
const normalAtlasPixels = canvas.width * canvas.height;
const draw = { id: "lead", kind: "raider", slot: 0, escorts: 0, angle: 0, tilt: policy.MAP_MODEL_TILT };
runtime.render([draw, { ...draw, id: "second", slot: 1 }], now);
assert.equal(requests.length, 1, "same hull type downloads once for many fleets");
assert.equal(runtime.sample(draw), undefined);
const owned = hull(); requests[0].resolve(owned); await flush(); advance(40);
assert.equal(runtime.render([draw], now), true); assert.ok(runtime.sample(draw));
const frames = gpuFrames.length; advance(1000);
assert.equal(runtime.render([draw], now), false); assert.equal(gpuFrames.length, frames, "steady ship needs no 3D render");
runtime.render([{ ...draw, angle: .1 }], now);
advance(1);
assert.equal(runtime.render([{ ...draw, angle: .2 }], now), false, "changing heading stays under 30 3D updates/sec");
for (const angle of [0, Math.PI / 4, Math.PI / 2, Math.PI, -Math.PI / 2]) {
  advance(40); runtime.render([{ ...draw, angle }], now);
  const shown = gpuFrames.at(-1).angle;
  assert.ok(Math.abs(Math.atan2(Math.sin(shown - angle), Math.cos(shown - angle))) < 1e-6, `projected heading ${angle}`);
  // Hull tops lean toward the screen's top edge (world -Z), so the camera sees
  // each hull from the bottom edge, as in the battle viewer and System View.
  const { up } = gpuFrames.at(-1);
  assert.ok(up.z < 0, `seen from the bottom edge at heading ${angle}`);
  assert.ok(Math.abs(Math.acos(up.y) - Math.acos(.8)) < 1e-9, `leans the full angle at heading ${angle}`);
}
advance(40); runtime.render([{ ...draw, escorts: 6, slot: 5 }], now);
assert.equal(gpuFrames.at(-1).meshes, 7); assert.equal(gpuFrames.at(-1).viewport.y, 0, "bottom row aligns with top-origin atlas frames");
assert.equal(runtime.debug().triangles, 84);
advance(40); runtime.render([{ ...draw, kind: "cruiser" }, { ...draw, kind: "titan", id: "titan", slot: 1 }, { ...draw, kind: "scout", id: "scout", slot: 2 }], now);
assert.equal(runtime.debug().loading.length, 2, "network concurrency bounded");
runtime.cancelPending(); assert.ok(requests.slice(1).every(r => r.signal.aborted));
const late = hull(); requests[1].resolve(late); await flush(); assert.ok(disposed.includes(late), "late aborted source disposed");
runtime.dispose(); assert.ok(disposed.includes(owned)); assert.equal(disposed.filter(m => m === owned).length, 1, "shared source disposed exactly once");
const later = hull(); requests[2].resolve(later); await flush(); assert.ok(disposed.includes(later));
assert.equal(runtime.healthy, false);

for (const narrow of [false, true]) {
  const closeRuntime = new MapShipModelRuntime(narrow, true);
  assert.equal(canvas.width * canvas.height, normalAtlasPixels / (narrow ? 4 : 1), "close-up uses the same atlas pixel budget");
  assert.equal(closeRuntime.slotCount, 2);
  assert.equal(closeRuntime.tileSize, narrow ? 768 : 1536);
  assert.equal(closeRuntime.debug().textureSize, narrow ? 1024 : 2048);
  closeRuntime.dispose();
}

// Parse all actual shipped GLBs through the production normalization code.
// Texture decoding is omitted here because these checks concern the geometry,
// scene transforms and atlas frustum, rather than a browser's raster output.
class GeometryLoader extends GLTFLoader {
  constructor() { super(); this.register(parser => ({ name: "GEOMETRY_ACCEPTANCE", beforeRoot() { parser.assignTexture = () => Promise.resolve(null); } })); }
}
const geometryStage = compile("ship-model-stage", {
  "three": THREE, "three/addons/loaders/GLTFLoader.js": { GLTFLoader: GeometryLoader },
  "three/addons/environments/RoomEnvironment.js": { RoomEnvironment: class {} }, "./ship-models.generated": catalog,
}, { fetch: async url => {
  const bytes = readFileSync(new URL(`../public${url.split("?")[0]}`, import.meta.url));
  return { ok: true, arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) };
} });
for (const [kind, spec] of Object.entries(catalog.SHIP_MODELS)) {
  const source = await geometryStage.loadShipModel(kind);
  stage.loadShipModel = async () => source;
  const realHullRuntime = new MapShipModelRuntime(false);
  const item = { ...draw, kind };
  realHullRuntime.render([item], now); await flush(); advance(40);
  for (const angle of [0, Math.PI / 4, Math.PI / 2, Math.PI * .75]) {
    advance(40); realHullRuntime.render([{ ...item, angle }], now);
    assert.equal(realHullRuntime.debug().triangles, spec.triangles, `${kind}: actual rendered mesh is exact LOD2`);
    assert.ok(gpuFrames.at(-1).fits, `${kind}: rotated mesh fits its atlas tile`);
  }
  realHullRuntime.dispose(); geometryStage.disposeModel(source);
}

// Resize only in-memory map copies. Shared image sources resize once, material
// color spaces stay intact, and cancellation closes a newly decoded bitmap.
class Bitmap {
  closed = false;
  constructor(width, height) { Object.assign(this, { width, height }); }
  close() { this.closed = true; }
}
const resizedImages = [];
let cancelDuringResize = null;
const { resizeModelTextures } = compile("ship-model-stage", {
  "three": THREE,
  "three/addons/loaders/GLTFLoader.js": { GLTFLoader: class {} },
  "three/addons/environments/RoomEnvironment.js": { RoomEnvironment: class {} },
  "./ship-models.generated": catalog,
}, { ImageBitmap: Bitmap, createImageBitmap: async (_source, options) => {
  const resized = new Bitmap(options.resizeWidth, options.resizeHeight); resizedImages.push(resized);
  cancelDuringResize?.abort(); return resized;
} });
const textureSource = new Bitmap(2048, 1024);
const albedo = new THREE.Texture(textureSource); albedo.colorSpace = THREE.SRGBColorSpace;
const roughness = new THREE.Texture(textureSource);
const textured = new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshStandardMaterial({ map: albedo, roughnessMap: roughness }));
await resizeModelTextures(textured, 1024);
assert.equal(resizedImages.length, 1); assert.equal(albedo.image, roughness.image);
assert.equal(albedo.image.width, 1024); assert.equal(albedo.image.height, 512);
assert.ok(textureSource.closed); assert.equal(albedo.colorSpace, THREE.SRGBColorSpace);
cancelDuringResize = new AbortController();
await assert.rejects(() => resizeModelTextures(textured, 512, cancelDuringResize.signal), /abort/i);
assert.ok(resizedImages.at(-1).closed); assert.equal(albedo.image.width, 1024, "aborted resize never installs partial texture");

console.log("Map 3D: all 18 actual LOD2 meshes/framing, zoom transition, 500-fleet budgets, selected priority, viewport culling, loading/context fallback, stale alpha, idle cleanup, shared source ownership, abort races, texture resizing, 30fps dirty rendering, formation clones, atlas alignment and projected headings passed.");

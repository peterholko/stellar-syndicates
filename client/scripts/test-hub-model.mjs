// The Market Hub's Authority Nexus: the web GLB derived from the approved art,
// the baked map sprite and panel hero, the shared framing, the living vortex,
// and the map bridge that fades the live station in over the flat sprite.
// --serve opens the real renderer on the hub; it never contacts a game server.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import sharp from "sharp";
import * as THREE from "three";
import * as BufferGeometryUtils from "three/addons/utils/BufferGeometryUtils.js";
import ts from "typescript";
import { HUB_BAKE, HUB_SOURCE, readGlb, triangleCount } from "./build-hub-model-assets.mjs";

const client = new URL("../", import.meta.url);
const read = path => readFileSync(new URL(path, client));
const src = path => read(`src/${path}`).toString("utf8");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const close = (actual, expected, label, tolerance = 1e-9) =>
  assert.ok(Math.abs(actual - expected) <= tolerance, `${label}: ${actual} != ${expected}`);
const compile = (path, deps = {}, globals = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, ...globals });
  return exports;
};

const { HUB_MODEL } = compile("hub-model.generated.ts");
const shipModels = compile("ship-models.generated.ts");
const mapPolicy = compile("map-ship-model-policy.ts", { "./ship-models.generated": shipModels });
const policy = compile("hub-model-policy.ts", { "./map-ship-model-policy": mapPolicy });

// --- The web GLB: the approved station, display-sized textures, nothing else changed.
const source = read(HUB_SOURCE);
const glb = read(`public${HUB_MODEL.url.split("?")[0]}`);
assert.equal(HUB_MODEL.url.split("?v=")[1], hash(glb).slice(0, 12), "the catalog busts caches by content");
const original = readGlb(source), web = readGlb(glb);
assert.equal(triangleCount(web.json), 11_440, "the approved 11,440 triangles");
assert.ok(glb.length < 2_500_000 && glb.length * 8 < source.length, `a web-sized station (${glb.length} bytes)`);
const imageViews = new Set(web.json.images.map(image => image.bufferView));
assert.equal(web.json.images.length, 10);
for (const [index, view] of web.json.bufferViews.entries()) {
  const start = view.byteOffset ?? 0;
  assert.equal(start % 4, 0, "buffer views stay 4-byte aligned");
  assert.ok(start + view.byteLength <= web.bin.length, "views stay inside the binary chunk");
  const bytes = web.bin.subarray(start, start + view.byteLength);
  if (imageViews.has(index)) continue;
  const before = original.json.bufferViews[index];
  assert.ok(bytes.equals(original.bin.subarray(before.byteOffset ?? 0, (before.byteOffset ?? 0) + before.byteLength)),
    `view ${index}: geometry and animation bytes are untouched`);
}
for (const image of web.json.images) {
  const view = web.json.bufferViews[image.bufferView];
  const bytes = web.bin.subarray(view.byteOffset, view.byteOffset + view.byteLength);
  assert.equal(image.mimeType, "image/jpeg");
  assert.equal(bytes.readUInt16BE(0), 0xffd8, `${image.name}: JPEG data`);
  const meta = await sharp(bytes).metadata();
  assert.ok(Math.max(meta.width, meta.height) <= 1024, `${image.name}: display-sized`);
}
const strip = json => JSON.stringify({ ...json, images: undefined, bufferViews: undefined, buffers: undefined, asset: undefined });
assert.equal(strip(web.json), strip(original.json), "nodes, meshes, materials, samplers and the collapse clip are unchanged");
const manifest = JSON.parse(read("public/art/derived/hub-model/v1/manifest.json"));
const bake = JSON.parse(read(`${HUB_BAKE}/bake-manifest.json`));
assert.equal(manifest.sourceSha256, hash(source), "built from the approved GLB");
assert.equal(bake.sourceSha256, hash(source), "the art was baked from the same GLB");
const master = read(`${HUB_BAKE}/top.png`);
const top = read(`public${HUB_MODEL.sprite.split("?")[0]}`), hero = read(`public${HUB_MODEL.hero.split("?")[0]}`);
assert.equal(hash(master), bake.topSha256, "the sprite master is the baked render");
assert.equal(HUB_MODEL.sprite.split("?v=")[1], hash(top).slice(0, 12));
const spriteMeta = await sharp(top).metadata();
assert.deepEqual([spriteMeta.format, spriteMeta.width, spriteMeta.height, spriteMeta.hasAlpha], ["webp", 1254, 1254, true], "the master's size, as WebP");
assert.ok(top.length * 5 < master.length, `a light first load (${top.length} bytes)`);
const alphaOf = async bytes => (await sharp(bytes).ensureAlpha().extractChannel(3).raw().toBuffer());
const [masterAlpha, spriteAlpha] = await Promise.all([alphaOf(master), alphaOf(top)]);
assert.ok(masterAlpha.equals(spriteAlpha), "lossless alpha: the measured silhouette is the master's");
assert.equal(HUB_MODEL.hero.split("?v=")[1], hash(hero).slice(0, 12));
const heroMeta = await sharp(hero).metadata();
assert.deepEqual([heroMeta.format, heroMeta.width, heroMeta.height, heroMeta.hasAlpha], ["webp", 512, 384, true]);

// --- Framing: the station's own vertices, normalized and posed like hub-model-stage.ts.
const nodeMeshes = new Map();
const walk = (index, assembly) => {
  const node = web.json.nodes[index];
  const owner = node.extras?.assembly ?? assembly;
  if (node.mesh !== undefined) nodeMeshes.set(node.mesh, owner);
  for (const child of node.children ?? []) walk(child, owner);
};
for (const root of web.json.scenes[web.json.scene ?? 0].nodes) walk(root, null);
assert.ok(web.json.nodes.every(node => !node.matrix && !node.translation && !node.rotation && !node.scale),
  "no node transforms (the framing reads raw positions)");
const vertices = [];
for (const [meshIndex, mesh] of web.json.meshes.entries()) for (const primitive of mesh.primitives) {
  const accessor = web.json.accessors[primitive.attributes.POSITION];
  const view = web.json.bufferViews[accessor.bufferView];
  assert.equal(accessor.componentType, 5126);
  const data = new Float32Array(web.bin.buffer.slice(web.bin.byteOffset + view.byteOffset + (accessor.byteOffset ?? 0),
    web.bin.byteOffset + view.byteOffset + (accessor.byteOffset ?? 0) + accessor.count * 12));
  for (let i = 0; i < accessor.count; i++) vertices.push({ p: new THREE.Vector3(data[i * 3], data[i * 3 + 1], data[i * 3 + 2]), assembly: nodeMeshes.get(meshIndex) });
}
const bounds = new THREE.Box3().setFromPoints(vertices.map(v => v.p));
const diameter = Math.max(bounds.max.x - bounds.min.x, bounds.max.z - bounds.min.z);
const center = bounds.getCenter(new THREE.Vector3());
const posed = tilt => {
  const matrix = new THREE.Matrix4().makeRotationX(tilt)
    .multiply(new THREE.Matrix4().makeRotationY(policy.HUB_YAW))
    .multiply(new THREE.Matrix4().makeScale(2 / diameter, 2 / diameter, 2 / diameter))
    .multiply(new THREE.Matrix4().makeTranslation(-center.x, 0, -center.z));
  return vertices.map(v => ({ ...v, q: v.p.clone().applyMatrix4(matrix) }));
};
// Camera straight down from +Y with screen-up −Z: screen x = x, screen up = −z, nearer = +y.
const extent = points => Math.max(...points.map(({ q }) => Math.max(Math.abs(q.x), Math.abs(q.z))));
for (let step = 0; step <= 8; step++) {
  const tilt = policy.hubModelTilt(step / 8);
  assert.ok(extent(posed(tilt)) <= policy.HUB_MODEL_FIELD * .985, `the leaning station fits its live tile (${step}/8)`);
}
const flat = posed(0);
assert.ok(extent(flat) <= policy.HUB_SPRITE_FIELD, "the top-down station fits the sprite canvas");
// The baked sprite was rendered with today's framing: its measured span matches.
const { data: alpha, info } = await sharp(top).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
let minX = info.width, maxX = -1, minY = info.height, maxY = -1;
for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
  if (alpha[(y * info.width + x) * 4 + 3] < 128) continue;
  minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y);
}
const predicted = extent(flat) / policy.HUB_SPRITE_FIELD * info.width;
assert.ok(Math.abs(maxX - minX + 1 - predicted) <= 4 && Math.abs(maxY - minY + 1 - predicted) <= 4,
  `the sprite was baked with the current framing (${maxX - minX + 1} px vs ${predicted.toFixed(1)} px); rerun npm run assets:hub:bake`);
assert.ok(Math.abs((minX + maxX + 1) / 2 - info.width / 2) <= 1 && Math.abs((minY + maxY + 1) / 2 - info.height / 2) <= 1, "centred on the aperture");
const fill = Number(src("render.ts").match(/const HUB_ART_FILL = (\d+) \/ (\d+);/).slice(1).reduce((a, b) => a / b));
close(fill, (maxX - minX + 1) / info.width, "HUB_ART_FILL is the sprite's measured span", .5 / info.width);
// The vortex stays in view: how far into the aperture (1 = its rim) does any
// stabilizer part in front of the disc reach, as projected for a view?
const reach = (yaw, tilt) => {
  const matrix = new THREE.Matrix4().makeRotationX(tilt).multiply(new THREE.Matrix4().makeRotationY(yaw))
    .multiply(new THREE.Matrix4().makeScale(2 / diameter, 2 / diameter, 2 / diameter));
  const aperture = 4 * 2 / diameter;
  return Math.min(...vertices.filter(v => v.assembly?.startsWith("stabilizer-")).map(v => v.p.clone().applyMatrix4(matrix))
    .filter(q => q.y > 0).map(q => Math.hypot(q.x / aperture, q.z / (aperture * Math.cos(tilt)))));
};
const heroTilt = -(Math.PI / 2 - policy.HUB_HERO_ELEVATION);
assert.ok(reach(policy.HUB_YAW, policy.hubModelTilt(1)) > .75, "the map's full lean keeps the vortex clear");
assert.ok(reach(policy.HUB_YAW, heroTilt) > .7, "towers on the diagonals keep the hero's vortex clear");
assert.ok(reach(0, heroTilt) < .4, "the yaw matters: on the axes the near tower covers the hero's vortex");

// --- Policy.
for (const zoom of [1, 12, 14, 17, 22, 40]) close(policy.hubModelBlend(zoom), mapPolicy.mapModelBlend(zoom), `the hull ramp at ${zoom}×`);
close(policy.hubModelTilt(0), 0, "the flat sprite's pose at the start of the fade");
assert.ok(policy.hubModelTilt(1) < 0 && policy.hubModelTilt(1) === mapPolicy.MAP_MODEL_TILT, "leans −37° like the map's hulls, from the bottom edge");
const tower = new THREE.Vector3(0, 1, 0).applyAxisAngle(new THREE.Vector3(1, 0, 0), policy.hubModelTilt(1));
assert.ok(-tower.z > 0, "the towers lean toward the screen's top: the camera sits at the bottom edge");
close(policy.hubModelTilePx(300) / policy.HUB_MODEL_FIELD, 300 / policy.HUB_SPRITE_FIELD, "the live tile and the sprite share the station's unit");
assert.ok(policy.HUB_MODEL_TILES.every(size => (size & (size - 1)) === 0), "power-of-two tiles for mipmaps");
assert.ok(policy.HUB_MODEL_TILES.at(-1) * extent(flat) / policy.HUB_MODEL_FIELD >= 2 * 720, "enough tile for the 720 px hub at 2× DPR");
assert.equal(policy.hubModelTileSize(137, 2), 512, "a small hub draws from the small tile");
assert.equal(policy.hubModelTileSize(300, 2), 1024);
assert.equal(policy.hubModelTileSize(800, 2), 2048);
assert.equal(policy.hubModelTileSize(800, 3), 2048, "pixel ratio counts up to 2");
assert.equal(policy.hubModelTileSize(300, 1), 512);
assert.equal(policy.hubModelTileSize(5000, 2), 2048, "the largest tile caps it");
assert.equal(policy.hubModelTileSize(450, 2, 2048), 2048, "a larger tile is kept until the need drops well below it");
assert.equal(policy.hubModelTileSize(400, 2, 2048), 1024);
assert.deepEqual([policy.hubModelFps(512), policy.hubModelFps(1024), policy.hubModelFps(2048)], [30, 30, 20], "the 16 MB tile redraws less often");
// Traffic flies in only once the station is large enough to show its small hulls.
assert.equal(policy.hubTrafficWanted(299, false), false);
assert.equal(policy.hubTrafficWanted(300, false), true, "on from a 300 px station");
assert.equal(policy.hubTrafficWanted(270, true), true, "hysteresis: stays on through small zooms back");
assert.equal(policy.hubTrafficWanted(259, true), false, "off below 260 px");
const trafficModule = compile("wormhole-traffic.ts", { three: THREE, "three/addons/utils/BufferGeometryUtils.js": BufferGeometryUtils });
{
  // Every visible hull and its wake stays inside the live tile at every lean.
  const toStation = tilt => new THREE.Matrix4().makeRotationX(tilt).multiply(new THREE.Matrix4().makeRotationY(policy.HUB_YAW))
    .multiply(new THREE.Matrix4().makeScale(2 / diameter, 2 / diameter, 2 / diameter));
  // A wake trails behind its hull: tail offset .48 plus wake .75 of the hull's length.
  const reach = .36 * policy.HUB_TRAFFIC_SCALE * (.48 + .75) * 2 / diameter;
  let worst = 0;
  for (let step = 0; step <= 4; step++) {
    const matrix = toStation(policy.hubModelTilt(step / 4));
    for (let lane = 0; lane < 10; lane++) {
      const angle = Math.PI / 4 + (lane % 4) * Math.PI / 2 + Math.floor(lane / 4) * .16;
      for (let p = trafficModule.TRAFFIC_VISIBLE_FROM; p <= .97; p += .005) {
        const q = trafficModule.trafficRoute(p, angle).applyMatrix4(matrix);
        worst = Math.max(worst, Math.abs(q.x) + reach, Math.abs(q.z) + reach);
      }
    }
  }
  assert.ok(worst <= policy.HUB_MODEL_FIELD, `arriving traffic fits the live tile (${worst.toFixed(3)} of ${policy.HUB_MODEL_FIELD})`);
}

// The label hugs the leaning station: its lower edge, measured from the model.
close(policy.HUB_RIM, extent(flat), "the rim is the model's top-down half span", 5e-4);
for (let step = 0; step <= 8; step++) {
  const tilt = policy.hubModelTilt(step / 8);
  const bottom = Math.max(...posed(tilt).map(({ q }) => q.z));
  close(policy.hubLeanBottom(tilt), bottom, `the leaning station's lower edge (${step}/8)`, .006);
}
assert.ok(policy.hubModelOnScreen(10, 10, 100, 800, 600) && policy.hubModelOnScreen(-40, 300, 100, 800, 600));
assert.ok(!policy.hubModelOnScreen(-60, 300, 100, 800, 600) && !policy.hubModelOnScreen(NaN, 0, 100, 800, 600));

// --- The living vortex (the asset's own runtime checks, on the ported module).
const effectModule = compile("wormhole-effect.ts", { three: THREE });
{
  const texture = new THREE.Texture();
  const original = new THREE.MeshBasicMaterial({ map: texture, side: THREE.DoubleSide });
  const geometry = new THREE.CircleGeometry(4, 64);
  const hull = new THREE.Mesh(geometry, original);
  const group = new THREE.Group();
  group.userData.assembly = "wormhole";
  const membrane = new THREE.Mesh(geometry, original);
  group.add(membrane);
  assert.throws(() => effectModule.createWormholeEffect(new THREE.Group()), /wormhole assembly/);
  const effect = effectModule.createWormholeEffect(group);
  assert.equal(hull.material, original, "the station's materials stay");
  assert.ok(membrane.material.isShaderMaterial && membrane.material.uniforms.uAtlas.value === texture, "reuses the atlas");
  effect.update(1 / 60);
  const first = effect.elapsed;
  assert.ok(first > 0);
  effect.setPlaying(false); effect.update(.02);
  assert.equal(effect.elapsed, first, "pause holds the frame");
  effect.setPlaying(true); effect.setPower(0);
  assert.equal(effect.active, false, "a closed wormhole needs no frames");
  effect.setPower(1);
  effect.update(10);
  assert.ok(effect.elapsed - first <= .051, "a long gap is one short step");
  for (let i = 0; i < Math.ceil(effectModule.TIME_PERIOD / .05); i++) effect.update(.05);
  assert.ok(effect.elapsed >= 0 && effect.elapsed < effectModule.TIME_PERIOD, "time wraps");
  let disposed = false;
  texture.addEventListener("dispose", () => { disposed = true; });
  effect.dispose(); effect.dispose();
  assert.equal(membrane.material, original, "disposal restores the export material");
  assert.equal(disposed, false, "the shared atlas stays alive");
  const still = effectModule.createWormholeEffect(group, { playing: false, time: policy.HUB_VORTEX_STILL_TIME });
  assert.equal(still.active, false);
  assert.equal(still.elapsed, policy.HUB_VORTEX_STILL_TIME, "a still frame holds the baked phase");
  still.dispose();
}

// --- Decorative traffic (the asset's runtime, on stand-in hulls with real three.js).
{
  const hull = (length, color) => {
    const group = new THREE.Group();
    const texture = new THREE.Texture();
    group.add(new THREE.Mesh(new THREE.BoxGeometry(length * .4, length * .2, length), new THREE.MeshStandardMaterial({ color, map: texture })));
    group.add(new THREE.Mesh(new THREE.BoxGeometry(length * .2, length * .1, length * .3).translate(0, .1, 0), new THREE.MeshStandardMaterial({ color })));
    return { group, texture };
  };
  const hulls = { interceptor: hull(2, 0xffffff), "tiny-freighter": hull(3, 0xddccaa), "small-freighter": hull(4, 0xccbb99), "medium-freighter": hull(5, 0xbbaa88) };
  const traffic = trafficModule.createWormholeTraffic(Object.fromEntries(Object.entries(hulls).map(([type, { group }]) => [type, group])));
  assert.equal(traffic.stats.ships, 10);
  assert.deepEqual({ ...traffic.stats.shipCounts }, { "tiny-freighter": 3, interceptor: 3, "small-freighter": 2, "medium-freighter": 2 }, "the asset's mix");
  const visible = () => traffic.snapshot().filter(ship => ship.visible);
  assert.ok(visible().length >= 5, "a stream is already under way");
  assert.ok(visible().every(ship => ship.renderedLength <= trafficModule.TRAFFIC_LENGTHS[ship.type] + 1e-9), "hulls at the asset's scale");
  const larger = trafficModule.createWormholeTraffic(Object.fromEntries(Object.entries(hulls).map(([type, { group }]) => [type, group])), { scale: 2 });
  const grown = larger.snapshot().filter(ship => ship.visible && ship.renderedLength > 0);
  assert.ok(grown.some(ship => ship.renderedLength > trafficModule.TRAFFIC_LENGTHS[ship.type] * 1.5), "the scale option enlarges the hulls");
  larger.dispose();
  const before = traffic.snapshot()[0].position;
  for (let i = 0; i < 20; i++) traffic.update(.05);
  assert.notDeepEqual(traffic.snapshot()[0].position, before, "ships fly");
  for (let i = 0; i < 36 / .05; i++) traffic.update(.05);
  assert.ok(visible().length >= 5, "arrivals keep coming");
  traffic.setEnabled(false);
  assert.equal(traffic.active, false, "hidden traffic needs no frames");
  assert.equal(traffic.group.visible, false);
  const held = JSON.stringify(traffic.snapshot());
  traffic.update(.05);
  assert.equal(JSON.stringify(traffic.snapshot()), held, "hidden traffic holds still");
  let textureFreed = false;
  hulls.interceptor.texture.addEventListener("dispose", () => { textureFreed = true; });
  const station = new THREE.Group();
  station.add(traffic.group);
  traffic.dispose(); traffic.dispose();
  assert.equal(traffic.group.parent, null, "leaves the station");
  assert.equal(traffic.group.children.length, 0);
  assert.equal(textureFreed, false, "the shared hull textures outlive the traffic");
}

// --- The map bridge: load on demand, crossfade, lean, release.
{
  let clock = 0;
  const timers = new Map();
  let timerId = 0;
  const windowStub = {
    setTimeout: (fn, ms) => { timers.set(++timerId, { fn, at: clock + ms }); return timerId; },
    clearTimeout: id => timers.delete(id),
    devicePixelRatio: 2,
  };
  const advance = ms => {
    clock += ms;
    for (const [id, timer] of [...timers]) if (timer.at <= clock) { timers.delete(id); timer.fn(); }
  };
  const runtimes = [];
  class FakeRuntime {
    ready = false; healthy = true; renders = []; resizes = []; traffic = []; disposed = false; changed = true;
    constructor(url, tileSize) { this.url = url; this.tileSize = tileSize; runtimes.push(this); }
    resize(size) { this.resizes.push(size); this.tileSize = size; }
    setTraffic(on) { this.traffic.push(on); }
    render(tilt, now) { if (this.throws) throw new Error("context lost"); this.renders.push({ tilt, now }); return this.changed; }
    dispose() { this.disposed = true; }
    debug() { return {}; }
  }
  class Sprite {
    constructor(texture) {
      this.texture = texture; this.alpha = 1; this.visible = true; this.destroyed = false;
      this.anchor = { set() {} };
      this.position = { set: (x, y) => { this.x = x; this.y = y; } };
      this.scale = { x: 1, set: value => { this.scale.x = value; } };
    }
  }
  class Texture { static EMPTY = { empty: true }; constructor(options) { this.options = options; } destroy() { this.destroyed = true; } }
  class CanvasSource { constructor(options) { this.options = options; this.updates = 0; } update() { this.updates++; } destroy() { this.destroyed = true; } }
  const warnings = [];
  const { HubModel } = compile("hub-model.ts", {
    "pixi.js": { CanvasSource, Sprite, Texture },
    "./hub-model.generated": { HUB_MODEL: { url: "/hub.glb" } },
    "./hub-model-policy": policy,
    "./hub-model-runtime": { HubModelRuntime: FakeRuntime },
  }, { window: windowStub, performance: { now: () => clock }, console: { warn: (...args) => warnings.push(args) } });
  const flush = () => new Promise(resolve => setImmediate(resolve));
  const hub = new HubModel();
  const flatSprite = new Sprite({ width: 1254 });
  flatSprite.scale.x = .1;
  const frame = (zoom, x = 400, y = 300, placed = true) => {
    hub.beginFrame();
    if (placed) hub.place(flatSprite, x, y, zoom);
    advance(16);
    hub.endFrame(800, 600, clock);
  };
  frame(4);
  await flush();
  assert.equal(runtimes.length, 0, "no three.js or GLB below the 3D zoom");
  assert.equal(flatSprite.alpha, 1);
  assert.equal(hub.sprite.visible, false);
  frame(22);
  await flush();
  assert.equal(runtimes.length, 1, "the runtime loads once the station is close");
  assert.deepEqual([runtimes[0].url, runtimes[0].tileSize], ["/hub.glb", 512], "a tile sized for the drawn station");
  frame(22);
  assert.equal(hub.sprite.visible, false, "the flat sprite holds while the GLB loads");
  assert.equal(flatSprite.alpha, 1);
  runtimes[0].ready = true;
  for (let i = 0; i < 20; i++) frame(22);
  const runtime = runtimes[0];
  close(runtime.renders.at(-1).tilt, mapPolicy.MAP_MODEL_TILT, "fully leaning at 22×");
  assert.ok(hub.sprite.visible && hub.sprite.alpha === 1 && flatSprite.alpha === 0, "all live 3D once faded in");
  close(hub.sprite.scale.x, .1 * 1254 * policy.HUB_MODEL_FIELD / policy.HUB_SPRITE_FIELD / 512, "the live tile covers the sprite exactly");
  const half = 100, radius = half / policy.HUB_SPRITE_FIELD, leaning = policy.hubLeanBottom(mapPolicy.MAP_MODEL_TILT);
  const drop = hub.labelDrop(half);
  assert.ok(drop < half && drop > radius * leaning, `the label moves up under the leaning rim (${drop.toFixed(1)} of ${half})`);
  close(drop - radius * leaning, half - radius * policy.HUB_RIM, "keeping the flat sprite's clearance");
  assert.deepEqual([hub.sprite.x, hub.sprite.y], [400, 300]);
  assert.equal(hub.sprite.texture.options.source, hub.sprite.texture.options.source);
  const uploads = hub.sprite.texture.options.source.updates;
  assert.ok(uploads >= 20, "each changed render is uploaded");
  assert.equal(hub.sprite.texture.options.source.options.autoGenerateMipmaps, true, "mipmapped: drawn below tile size");
  runtime.changed = false;
  frame(22);
  assert.equal(hub.sprite.texture.options.source.updates, uploads, "an unchanged render uploads nothing");
  for (let i = 0; i < 40; i++) frame(17);
  close(hub.sprite.alpha, policy.hubModelBlend(17), "mid-ramp crossfade", 1e-9);
  close(flatSprite.alpha, 1 - policy.hubModelBlend(17), "the flat sprite takes the rest");
  close(runtime.renders.at(-1).tilt, policy.hubModelTilt(policy.hubModelBlend(17)), "the lean follows the blend");
  // A closer look needs more pixels: the tile grows with the drawn station and
  // Pixi takes the resized canvas afresh; the live station still covers the sprite.
  assert.deepEqual(runtime.traffic, [false], "no traffic around a small station");
  const smallSource = hub.sprite.texture.options.source;
  flatSprite.scale.x = .5;
  frame(22);
  assert.deepEqual(runtime.resizes, [2048]);
  assert.deepEqual(runtime.traffic, [false, true], "traffic flies in once the station is drawn large");
  assert.ok(hub.sprite.texture.options.source !== smallSource && smallSource.destroyed, "a new texture for the resized canvas");
  close(hub.sprite.scale.x, .5 * 1254 * policy.HUB_MODEL_FIELD / policy.HUB_SPRITE_FIELD / 2048, "still covers the sprite exactly");
  flatSprite.scale.x = .3;
  frame(22);
  assert.deepEqual(runtime.resizes, [2048], "no rebuild for a small zoom back");
  assert.deepEqual(runtime.traffic, [false, true], "and the traffic keeps flying");
  flatSprite.scale.x = .1;
  frame(22);
  assert.deepEqual(runtime.resizes, [2048, 512], "a far zoom-out returns to the small tile");
  assert.deepEqual(runtime.traffic, [false, true, false], "and stops the traffic");
  frame(22, -2000);
  assert.ok(!hub.sprite.visible && flatSprite.alpha === 1, "off screen: the flat sprite only");
  assert.equal(hub.labelDrop(100), 100, "the flat sprite's label position");
  advance(14_000);
  assert.equal(runtime.disposed, false, "kept briefly for a return");
  frame(22, 400, 300, false);
  advance(2_000);
  assert.equal(runtime.disposed, true, "released after idling");
  frame(22);
  await flush();
  assert.equal(runtimes.length, 2, "reloads when wanted again");
  runtimes[1].ready = true;
  frame(22);
  runtimes[1].throws = true;
  frame(22);
  assert.ok(runtimes[1].disposed && flatSprite.alpha === 1 && warnings.length === 1, "a failed render falls back to the sprite");
  frame(22);
  await flush();
  assert.equal(runtimes.length, 2, "no retry storm");
  advance(30_000);
  frame(22);
  await flush();
  assert.equal(runtimes.length, 3, "retries after 30 s");
  hub.suspend();
  assert.ok(runtimes[2].disposed && !hub.sprite.visible, "suspend releases at once");
}

// --- Wiring: three.js stays out of the eager bundle; the map frame drives the bridge.
const render = src("render.ts"), bridge = src("hub-model.ts");
assert.doesNotMatch(render, /from "\.\/hub-model-runtime"|from "three"/, "render.ts never imports three.js");
assert.match(bridge, /import type \{ HubModelRuntime \} from "\.\/hub-model-runtime"/);
assert.match(bridge, /import\("\.\/hub-model-runtime"\)/, "the runtime is a dynamic import");
assert.match(render, /this\.hubModel\.beginFrame\(\);\n\s*if \(!state\.galaxy\) \{ this\.shipModels\.suspend\(\); this\.hubModel\.suspend\(\); return; \}/);
assert.match(render, /this\.hubModel\.endFrame\(this\.viewW, this\.viewH, performance\.now\(\)\);/);
assert.match(render, /if \(paused\) \{ this\.shipModels\.suspend\(\); this\.hubModel\.suspend\(\); this\.app\.stop\(\); \}/, "a covered map releases the station");
assert.match(render, /this\.bodyLayer\.addChild\(this\.hubSprite, this\.hubModel\.sprite\)/, "the live station sits just above the sprite");
assert.match(src("hub-model-runtime.ts"), /now - this\.lastRender < 1000 \/ hubModelFps\(this\.tileSize\) - 1/, "the vortex redraws at a capped rate");
assert.doesNotMatch(bridge + src("hub-model-runtime.ts"), /matchMedia|reduced-motion/, "the vortex flows like the pulsars, whatever the reduced-motion setting");
assert.match(src("shell/deck/market.ts"), /<img src="\$\{HUB_MODEL\.hero\}"/, "the station panel shows the baked Nexus");
const runtimeSource = src("hub-model-runtime.ts");
assert.match(runtimeSource, /"medium-freighter": "convoy"/, "the traffic's hulls are the game's own models");
assert.ok(runtimeSource.indexOf("this.traffic?.dispose()") < runtimeSource.indexOf("this.station?.dispose()"), "traffic leaves before the station frees its assets");
assert.match(runtimeSource, /this\.station\.local\.add\(this\.traffic\.group\)/, "traffic flies in the station's own frame, leaning with it");

console.log(`Hub model: web GLB ${(glb.length / 1e6).toFixed(2)} MB from ${(source.length / 1e6).toFixed(1)} MB with untouched geometry, baked sprite/hero provenance and framing, tile fit at every lean (traffic included), clear aperture, living vortex, decorative traffic, and the map bridge's load/crossfade/lean/tile/traffic/release passed.`);

// --serve: the real renderer over a canned galaxy, centred on the hub, with an
// own Interceptor parked beside it. ?zoom=N sets the magnification (default
// 192×), ?ship=0 hides the Interceptor. No game server.
if (process.argv.includes("--serve")) {
  const { createServer } = await import("node:http");
  const { resolve, sep } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const { build } = await import("vite");
  const clientRoot = fileURLToPath(client);
  const module = path => JSON.stringify(resolve(clientRoot, `src/${path}`));
  const result = await build({ root: clientRoot, configFile: false, publicDir: false, logLevel: "error",
    plugins: [{ name: "hub-fixture", resolveId: id => id.endsWith("virtual:hub-fixture") ? "\0hub-fixture" : undefined,
      load: id => id === "\0hub-fixture" ? `
        import { renderer } from ${module("render.ts")};
        import { state } from ${module("state.ts")};
        const params = new URLSearchParams(location.search);
        (async () => {
        state.playerId = "preview"; state.link = "online";
        state.galaxy = { hub: { x: 0, y: 0 }, radius: 300000, nebulas: [], c: 400, sensor_range: 80000,
          jump_range: 50000, jump_spool_s: 10, hyperlimit: 900, scout_sensor_mult: 1, convoy_sensor_mult: 1,
          sensor_array_base: 0, sensor_array_per_tier: 0, defense_platform_radius: 0, build_options: [],
          systems: [["Freya", 60000, -20000], ["Vega", -90000, 40000], ["Aster", 30000, 110000], ["Lyra", -40000, -120000]]
            .map(([name, x, y], i) => ({ id: String(i + 1), name, pos: { x, y }, band: "rich", body_count: 3 })) };
        await renderer.init(document.getElementById("app"));
        renderer.setGalaxy(state.galaxy);
        let zoom = Number(params.get("zoom") ?? 192);
        // Parked just outside the rim: 1.35 station radii out at 96× (the hub's
        // radius is 90 px there), inside the zone where hulls take their System
        // View size beside the hub. Past 96× the station grows with the camera.
        const fit = renderer.scale / renderer.zoomFactor();
        if (params.get("ship") !== "0") state.ghosts = [{ id: "beside-hub", owner: "preview", own: true, kind: "raider",
          pos: { x: 1.35 * 90 / (96 * fit), y: 0 }, vel: { x: 0, y: -.001 }, age: 0, jumped: false, route: null,
          count_class: "one", composition: [{ kind: "raider", count: 1 }], cargo: null, docked: null }];
        renderer.stateVersion++;
        // Frame the pair: centre between the hub and its neighbour.
        const focus = () => state.ghosts.length ? { x: state.ghosts[0].pos.x / 2, y: 0 } : state.galaxy.hub;
        const apply = () => { renderer.zoomByFactor(zoom / renderer.zoomFactor()); renderer.centerOnWorld(focus(), 0); };
        apply();
        addEventListener("keydown", event => {
          if (event.key === "+" || event.key === "=") { zoom *= 1.25; apply(); }
          if (event.key === "-") { zoom /= 1.25; apply(); }
        });
        window.__hub = { setZoom(value) { zoom = value; apply(); }, debug: () => renderer.hubModelDebug(),
          screen: () => renderer.worldToScreen(state.galaxy.hub) };
        let settling = 30;
        const status = document.getElementById("status");
        const frame = () => {
          // Layout settles over the first frames; keep the framing until it has.
          if (settling-- > 0) apply();
          renderer.update(state);
          const debug = renderer.hubModelDebug();
          document.documentElement.dataset.hub = JSON.stringify(debug);
          status.textContent = renderer.zoomFactor().toFixed(1) + "× · live " + debug.mix.toFixed(2) + " · " + (debug.runtime ? (debug.runtime.ready ? debug.runtime.frames + " frames" : "loading") : "flat sprite");
          requestAnimationFrame(frame);
        };
        requestAnimationFrame(frame);
        })();
      ` : undefined }], build: { write: false, minify: false, lib: { entry: "virtual:hub-fixture", formats: ["es"] } } });
  const output = (Array.isArray(result) ? result[0] : result).output;
  const bundle = output.find(file => file.type === "chunk" && file.isEntry).code;
  const files = new Map(output.map(file => [`/${file.fileName}`, file.type === "chunk" ? file.code : file.source]));
  const publicRoot = resolve(clientRoot, "public");
  const types = { glb: "model/gltf-binary", svg: "image/svg+xml", webp: "image/webp", png: "image/png", jpg: "image/jpeg", json: "application/json" };
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Hub model fixture</title><style>html,body{margin:0;height:100%;background:#03060c;overflow:hidden}#app{position:fixed;inset:0}#status{position:fixed;left:12px;bottom:12px;padding:6px 8px;border-radius:4px;background:#0b1320cc;color:#b9cbdb;font:12px ui-monospace,monospace}</style></head><body><div id="app"></div><div id="status"></div><script type="module" src="/fixture.js"></script></body></html>`);
      return;
    }
    if (url.pathname === "/fixture.js") { response.setHeader("Content-Type", "text/javascript"); response.end(bundle); return; }
    if (files.has(url.pathname)) { response.setHeader("Content-Type", url.pathname.endsWith(".css") ? "text/css" : "text/javascript"); response.end(files.get(url.pathname)); return; }
    if (!url.pathname.startsWith("/art/")) { response.writeHead(404).end(); return; }
    const path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
    if (!path.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
    const type = types[path.split(".").pop()];
    if (type) response.setHeader("Content-Type", type);
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  const port = Number(process.argv.find(arg => arg.startsWith("--port="))?.slice("--port=".length)) || 0;
  server.listen(port, "127.0.0.1", () => console.log(`Hub model fixture: http://127.0.0.1:${server.address().port}/`));
}

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import vm from "node:vm";
import ts from "typescript";
import sharp from "sharp";
import { SHIP_MODELS } from "./ship-model-catalog.mjs";
import { buildShipModelAssets } from "./build-ship-model-assets.mjs";

const client = new URL("../", import.meta.url);
const read = file => readFile(new URL(file, client));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const compile = text => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText, { exports });
  return exports;
};
const { SHIP_MODELS: catalog } = compile((await read("src/ship-models.generated.ts")).toString());
const protocol = ts.createSourceFile("protocol.ts", (await read("src/protocol.ts")).toString(), ts.ScriptTarget.Latest, true);
const kinds = protocol.statements.find(s => ts.isTypeAliasDeclaration(s) && s.name.text === "ShipKind").type.types.map(t => t.literal.text);
assert.deepEqual(Object.keys(catalog).sort(), kinds.sort(), "Every real game hull has a model");
const baked = JSON.parse(await read("art-src/ship-models/game-ready-v1/bake-manifest.json"));
const art = JSON.parse(await read("public/art/derived/ships/models-v1/manifest.json"));
let totalBytes = 0;
for (const { kind, id } of SHIP_MODELS) {
  const model = catalog[kind];
  assert.equal(model.id, id);
  assert.match(model.url, new RegExp(`/${id}-lod2\\.glb\\?v=`), `${kind}: lowest mesh only`);
  const bytes = await read(`public${model.url.split("?")[0]}`);
  const original = await read(`art-src/ship-models/interceptor-lod-experiment-v1/models/${id}-lod2.glb`);
  assert.equal(hash(bytes), hash(original), `${kind}: exact approved mesh and textures`);
  assert.equal(hash(bytes), baked.ships[kind].sourceSha256, `${kind}: map baked from this model`);
  assert.equal(hash(bytes), art.ships[kind].modelSha256, `${kind}: map provenance retained`);
  const gltf = JSON.parse(bytes.subarray(20, 20 + bytes.readUInt32LE(12)).toString());
  const triangles = gltf.meshes.reduce((n, mesh) => n + mesh.primitives.reduce((m, p) => m + gltf.accessors[p.indices].count / 3, 0), 0);
  assert.equal(triangles, model.triangles);
  assert.equal(triangles, baked.ships[kind].triangles);
  assert.ok(gltf.images.length >= 3, `${kind}: color, roughness and emissive textures`);
  assert.ok(gltf.images.every(image => image.bufferView !== undefined && !image.uri), `${kind}: self-contained textures`);
  if (kind === "mining_barge") {
    assert.ok(triangles >= 4000 && triangles <= 5000, "Barge retains its silhouette within the LOD2 budget");
    const sockets = gltf.nodes.filter(node => node.name?.startsWith("socket."));
    assert.deepEqual(sockets.map(node => node.name).sort(), ["socket.engine.port", "socket.engine.starboard", "socket.mining.port", "socket.mining.starboard"]);
    for (const socket of sockets) assert.deepEqual(socket.extras.direction,
      [0, 0, socket.name.startsWith("socket.engine.") ? 1 : -1], `${socket.name}: authored facing`);
  }
  if (kind === "mining_belt_station") {
    assert.ok(triangles >= 7000 && triangles <= 8500, "Station fits the LOD2 budget");
    const sockets = gltf.nodes.filter(node => node.name?.startsWith("socket."));
    const directions = {
      "socket.docking.barge.port": [-1, 0, 0], "socket.docking.barge.starboard": [1, 0, 0],
      "socket.docking.hauler": [0, 0, -1],
      ...Object.fromEntries([1, 2, 3, 4].map(i => [`socket.anchor.${i}`, [0, -1, 0]])),
    };
    assert.deepEqual(sockets.map(node => node.name).sort(), Object.keys(directions).sort());
    for (const socket of sockets) assert.deepEqual(socket.extras.direction, directions[socket.name], `${socket.name}: authored facing`);
  }
  for (const view of ["top", "bottom", "perspective"]) {
    const image = await read(`art-src/ship-models/game-ready-v1/${kind}-${view}.png`);
    assert.equal(hash(image), baked.ships[kind][`${view}Sha256`]);
    const meta = await sharp(image).metadata();
    assert.equal(meta.width, view === "top" ? 1254 : 768);
    assert.ok(meta.hasAlpha);
    const stats = await sharp(image).stats();
    assert.ok(stats.channels[3].max === 255 && stats.channels[3].min === 0, `${kind}/${view}: opaque hull, transparent background`);
    assert.ok(stats.channels.slice(0, 3).every(c => c.stdev > 10), `${kind}/${view}: rendered texture detail`);
  }
  const poster = await sharp(await read(`public${model.poster}`)).metadata();
  assert.equal(poster.width, 768); assert.ok(poster.hasAlpha);
  totalBytes += bytes.length;
}
const shipped = await readdir(new URL("public/art/derived/ship-models/v1/", client));
assert.equal(shipped.filter(f => f.endsWith(".glb")).length, 20);
assert.ok(shipped.every(f => !/-lod[01]|original/.test(f)), "No higher-detail model payloads shipped");
assert.equal((await buildShipModelAssets()).written, 0, "Production copy is idempotent");
console.log(`Ship models: all ${kinds.length} game kinds, exact LOD2 bytes/textures, triangle counts, ${SHIP_MODELS.length * 3} verified model renders, posters and reproducible packaging passed (${(totalBytes / 1048576).toFixed(1)} MiB; individual lazy model payloads).`);

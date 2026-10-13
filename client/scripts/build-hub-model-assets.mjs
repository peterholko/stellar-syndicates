// The Market Hub's station, the Authority Nexus, for the web: the approved GLB
// with its ten embedded textures re-encoded as display-sized JPEG (geometry,
// materials and the WormholeCollapse clip untouched), plus the flat map sprite
// and panel hero baked from that GLB by `npm run assets:hub:bake`.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
export const HUB_SOURCE = "art-src/ship-models/authority-nexus-textured-v1/authority-nexus.glb";
export const HUB_BAKE = "art-src/hub-models/authority-nexus-game-ready-v1";
const destination = path.join(client, "public/art/derived/hub-model/v1");
const recipe = "nexus-jpeg-1024-v1";
const MAX_TEXTURE = 1024;
const TRIANGLES = 11_440;
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
async function writeChanged(file, data) {
  if (hash(await readFile(file).catch(() => Buffer.alloc(0))) === hash(data)) return 0;
  await writeFile(file, data);
  return 1;
}

/** Split a GLB into its JSON and binary chunks. */
export function readGlb(bytes) {
  assert.equal(bytes.toString("ascii", 0, 4), "glTF", "GLB magic");
  assert.equal(bytes.readUInt32LE(4), 2, "glTF 2.0");
  const jsonLength = bytes.readUInt32LE(12);
  assert.equal(bytes.toString("ascii", 16, 20), "JSON");
  const json = JSON.parse(bytes.subarray(20, 20 + jsonLength).toString("utf8"));
  const binHeader = 20 + jsonLength;
  assert.equal(bytes.toString("ascii", binHeader + 4, binHeader + 8), "BIN\0");
  const bin = bytes.subarray(binHeader + 8, binHeader + 8 + bytes.readUInt32LE(binHeader));
  return { json, bin };
}

export function triangleCount(json) {
  return json.meshes.reduce((n, mesh) => n + mesh.primitives.reduce((m, p) => m + json.accessors[p.indices].count / 3, 0), 0);
}

/** Re-encode every embedded image; every other buffer view keeps its bytes. */
async function webGlb(source) {
  const { json, bin } = readGlb(source);
  const images = new Map((json.images ?? []).map((image, index) => [image.bufferView, index]));
  const views = [];
  for (const [index, view] of json.bufferViews.entries()) {
    const bytes = bin.subarray(view.byteOffset ?? 0, (view.byteOffset ?? 0) + view.byteLength);
    if (!images.has(index)) { views.push(bytes); continue; }
    const image = json.images[images.get(index)];
    const meta = await sharp(bytes).metadata();
    assert.ok(!meta.hasAlpha, `${image.name}: an opaque map (JPEG carries no alpha)`);
    const ratio = Math.min(1, MAX_TEXTURE / Math.max(meta.width, meta.height));
    views.push(await sharp(bytes).resize(Math.round(meta.width * ratio), Math.round(meta.height * ratio), { kernel: "lanczos3" })
      .jpeg({ quality: 88, chromaSubsampling: "4:4:4", mozjpeg: true }).toBuffer());
    image.mimeType = "image/jpeg";
    image.name = image.name?.replace(/\.png$/, ".jpg");
  }
  // Re-pack the binary chunk; 4-byte alignment keeps every accessor aligned.
  let offset = 0;
  const chunks = [];
  for (const [index, bytes] of views.entries()) {
    const pad = (4 - offset % 4) % 4;
    if (pad) { chunks.push(Buffer.alloc(pad)); offset += pad; }
    json.bufferViews[index].byteOffset = offset;
    json.bufferViews[index].byteLength = bytes.length;
    chunks.push(bytes);
    offset += bytes.length;
  }
  const tail = (4 - offset % 4) % 4;
  if (tail) chunks.push(Buffer.alloc(tail));
  const binary = Buffer.concat(chunks);
  json.buffers = [{ byteLength: binary.length }];
  json.asset = { ...json.asset, extras: { ...(json.asset.extras ?? {}), webRecipe: recipe } };
  let text = Buffer.from(JSON.stringify(json), "utf8");
  if (text.length % 4) text = Buffer.concat([text, Buffer.alloc(4 - text.length % 4, 0x20)]);
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "ascii");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + 8 + text.length + 8 + binary.length, 8);
  const chunk = (type, data) => {
    const head = Buffer.alloc(8);
    head.writeUInt32LE(data.length, 0);
    head.write(type, 4, "ascii");
    return Buffer.concat([head, data]);
  };
  return Buffer.concat([header, chunk("JSON", text), chunk("BIN\0", binary)]);
}

export async function buildHubModelAssets() {
  await mkdir(destination, { recursive: true });
  const source = await readFile(path.join(client, HUB_SOURCE));
  const sourceSha256 = hash(source);
  const glb = await webGlb(source);
  const parsed = readGlb(glb);
  assert.equal(triangleCount(parsed.json), TRIANGLES, "the approved station geometry, unchanged");
  for (const name of ["structure", "containment-field", "stabilizer-1", "stabilizer-2", "stabilizer-3", "stabilizer-4", "wormhole"]) {
    assert.ok(parsed.json.nodes.some(node => node.name === name && node.extras?.assembly === name), `assembly ${name}`);
  }
  assert.ok(parsed.json.animations?.some(clip => clip.name === "WormholeCollapse"), "the collapse clip survives");
  let written = await writeChanged(path.join(destination, "authority-nexus.glb"), glb);
  const glbSha256 = hash(glb);

  // The baked art must come from this exact source GLB (rerun the bake after
  // changing it), just as the hull renders are checked against their models.
  const bake = JSON.parse(await readFile(path.join(client, HUB_BAKE, "bake-manifest.json"), "utf8"));
  assert.equal(bake.sourceSha256, sourceSha256, "the station GLB changed; rerun npm run assets:hub:bake");
  const top = await readFile(path.join(client, HUB_BAKE, "top.png"));
  const hero = await readFile(path.join(client, HUB_BAKE, "hero.png"));
  assert.equal(hash(top), bake.topSha256, "top render provenance");
  assert.equal(hash(hero), bake.heroSha256, "hero render provenance");
  const topMeta = await sharp(top).metadata();
  assert.ok(topMeta.hasAlpha && topMeta.width === 1254 && topMeta.height === 1254, "a transparent 1254 px master");
  // The map sprite keeps the master's 1254 px (no resampling) as a lossy WebP
  // with lossless alpha: a tenth of the PNG for every player's first load, and
  // the same visible span HUB_ART_FILL measures.
  const sprite = await sharp(top).webp({ quality: 90, alphaQuality: 100, effort: 6 }).toBuffer();
  written += await writeChanged(path.join(destination, "authority-nexus-top-1254.webp"), sprite);
  const heroMeta = await sharp(hero).metadata();
  assert.ok(heroMeta.hasAlpha && heroMeta.width === 1024 && heroMeta.height === 768, "a transparent 1024 × 768 hero");
  const heroWebp = await sharp(hero).resize(512, 384, { kernel: "lanczos3" }).webp({ quality: 90, alphaQuality: 100 }).toBuffer();
  written += await writeChanged(path.join(destination, "authority-nexus-hero-512.webp"), heroWebp);

  const url = file => `/art/derived/hub-model/v1/${file}`;
  const catalog = {
    title: "Authority Nexus",
    triangles: TRIANGLES,
    url: `${url("authority-nexus.glb")}?v=${glbSha256.slice(0, 12)}`,
    sprite: `${url("authority-nexus-top-1254.webp")}?v=${hash(sprite).slice(0, 12)}`,
    hero: `${url("authority-nexus-hero-512.webp")}?v=${hash(heroWebp).slice(0, 12)}`,
  };
  const manifest = { recipe, source: HUB_SOURCE, sourceSha256, sourceBytes: source.length,
    glbBytes: glb.length, glbSha256, ...catalog };
  written += await writeChanged(path.join(destination, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  written += await writeChanged(path.join(client, "src/hub-model.generated.ts"),
    "// Generated by scripts/build-hub-model-assets.mjs. Edit the approved station, not this catalog.\n" +
    `export const HUB_MODEL = ${JSON.stringify(catalog, null, 2)} as const;\n`);
  return { sourceBytes: source.length, glbBytes: glb.length, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log("Hub model:", await buildHubModelAssets());
}

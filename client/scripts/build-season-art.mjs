// Season art is generated from the native masters without changing them.
// Inventory framing is normalized here; every Gate stage keeps its full canvas
// so changing construction stage cannot recenter the structure on the map.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const destination = path.join(client, "public/art/derived/season/v1");
const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
const recipe = "season-v1-alpha-frame-chroma-2";
export const CAPTAIN_GEAR = ["tactical-datacore", "admirals-datacore", "astrogation-suite", "pathfinders-astrolabe",
  "survey-kit", "precursor-lens", "quartermasters-ledger", "chandlers-seal"];
export const GATE_STAGES = [1, 2, 3, 4, 5].map(stage => `gate-stage-${stage}`);
export const PIRATE_LINE_SHIPS = ["pirate-reaver", "pirate-warbarge"];
const hash = bytes => createHash("sha256").update(bytes).digest("hex");

async function writeChanged(file, bytes) {
  if (hash(await readFile(file).catch(() => Buffer.alloc(0))) === hash(bytes)) return 0;
  await writeFile(file, bytes);
  return 1;
}

function bounds(data, width, height) {
  let left = width, top = height, right = -1, bottom = -1;
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    if (data[(y * width + x) * 4 + 3] < 128) continue;
    left = Math.min(left, x); right = Math.max(right, x);
    top = Math.min(top, y); bottom = Math.max(bottom, y);
  }
  assert.ok(right > left && bottom > top, "Source must contain a visible object");
  return { left, top, width: right - left + 1, height: bottom - top + 1 };
}

// The brief requests green-screen pirate masters for later keying. Only green
// dominance is keyed, retaining dark hulls and the warm recessed engines.
function keyGreen(data) {
  for (let i = 0; i < data.length; i += 4) {
    const red = data[i], green = data[i + 1], blue = data[i + 2];
    const excess = green - Math.max(red, blue);
    if (green < 70 || excess <= 18) continue;
    const key = Math.min(1, (excess - 18) / 72);
    data[i + 3] = Math.round(data[i + 3] * (1 - key));
    // Remove green spill from the few partially transparent boundary pixels.
    data[i + 1] = Math.max(red, blue);
  }
}

async function fitSprite(data, info, size, widthFill, heightFill, box) {
  // A small native-pixel allowance retains antialiasing around the solid hull.
  const pad = 4;
  const left = Math.max(0, box.left - pad), top = Math.max(0, box.top - pad);
  const width = Math.min(info.width, box.left + box.width + pad) - left;
  const height = Math.min(info.height, box.top + box.height + pad) - top;
  const scale = Math.min(size * widthFill / width, size * heightFill / height);
  const w = Math.max(1, Math.round(width * scale)), h = Math.max(1, Math.round(height * scale));
  const sprite = await sharp(data, { raw: info }).extract({ left, top, width, height })
    .resize(w, h, { kernel: "lanczos3" }).png().toBuffer();
  return sharp({ create: { width: size, height: size, channels: 4, background: transparent } })
    .composite([{ input: sprite, left: Math.round((size - w) / 2), top: Math.round((size - h) / 2) }])
    .png({ compressionLevel: 9 }).toBuffer();
}

export async function buildSeasonArt() {
  await mkdir(destination, { recursive: true });
  const manifest = { recipe, mastersUnmodified: true, assets: {} }, catalog = {};
  const assets = [
    ...CAPTAIN_GEAR.map(id => ({ id, family: "gear", source: `art-src/captain-gear-v1/${id}.png`, sizes: [64, 128, 256] })),
    ...GATE_STAGES.map(id => ({ id, family: "gate", source: `art-src/season-gate-v1/${id}.png`, sizes: [64, 128, 256, 512] })),
    ...PIRATE_LINE_SHIPS.map(id => ({ id, family: "pirate", source: `art-src/npc-ships/${id}-chroma.png`, sizes: [128, 256, 512, 1254] })),
  ];
  let written = 0;
  for (const asset of assets) {
    const bytes = await readFile(path.join(client, asset.source));
    const meta = await sharp(bytes).metadata();
    assert.deepEqual([meta.width, meta.height], [1254, 1254], `${asset.id}: native square master`);
    if (asset.family !== "pirate") assert.ok(meta.hasAlpha, `${asset.id}: generated alpha required`);
    const { data, info } = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    if (asset.family === "pirate") keyGreen(data);
    const box = bounds(data, info.width, info.height);
    const levels = {};
    for (const size of asset.sizes) {
      const png = asset.family === "gate"
        ? await sharp(data, { raw: info }).resize(size, size).png({ compressionLevel: 9 }).toBuffer()
        : await fitSprite(data, info, size, asset.family === "gear" ? .78 : .92,
          asset.family === "gear" ? .68 : .92, box);
      const file = `${asset.id}-${size}.png`;
      written += await writeChanged(path.join(destination, file), png);
      levels[size] = `/art/derived/season/v1/${file}?v=${hash(png).slice(0, 12)}`;
    }
    manifest.assets[asset.id] = { family: asset.family, source: asset.source, sourceSha256: hash(bytes),
      nativeSolidBounds: box, framing: asset.family === "gate" ? "full canvas; common stage alignment"
        : asset.family === "gear" ? "fit within 78% width and 68% height; preserve proportions"
          : "green keyed; fit within 92% width and height; preserve proportions", levels };
    catalog[asset.id] = levels;
  }
  written += await writeChanged(path.join(destination, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  written += await writeChanged(path.join(client, "src/season-art.generated.ts"),
    "// Generated by scripts/build-season-art.mjs. Native sources remain in art-src.\n" +
    `export const SEASON_ART = ${JSON.stringify(catalog, null, 2)} as const;\n`);
  return { sources: assets.length, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log("Season art:", await buildSeasonArt());
}

// Galaxy map glyphs: flat white silhouettes the map tints by meaning (contacts
// cyan, contracts by issuer). Sliced from the four generated sheets in
// art-src/galaxy-map-glyphs-v1 (white on black, 3 × 2 cells of 512 px). Black
// becomes transparency, the generator's grey fringes snap to a clean edge, and
// every glyph is trimmed and fitted to the same box so the set reads evenly.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
export const GLYPH_SOURCE = "art-src/galaxy-map-glyphs-v1";
const destination = path.join(client, "public/art/derived/map-glyphs/v1");
const CELL = 512;
/** Every glyph's longer side fills this share of its square. */
const FILL = .84;
const SIZES = [64, 128];
/** Luminance levels: below LOW is void, above HIGH is ink; between, the edge. */
const LOW = 56, HIGH = 200;
export const GLYPH_SHEETS = [
  ["01-exploration-sites.png", ["site-derelict", "site-station", "site-asteroids", "site-anomaly", "site-precursor", "contact-unknown"]],
  ["02-contracts-part-1.png", ["contract-bounty", "contract-patrol", "contract-salvage", "contract-escort", "contract-combat", "contract-enforcement"]],
  ["03-contracts-part-2.png", ["contract-control", "contract-mandate", "contract-megaproject", "contract-survey", "contract-delivery", "pin-flag"]],
  ["04-contract-issuers.png", ["issuer-market", "issuer-survey-office", "issuer-salvage-office", "issuer-regional-council", "issuer-syndicate", "issuer-authority"]],
];
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
async function writeChanged(file, data) {
  if (hash(await readFile(file).catch(() => Buffer.alloc(0))) === hash(data)) return 0;
  await writeFile(file, data);
  return 1;
}

/** One cell → a trimmed, centred white glyph with clean alpha, at `size` px. */
export async function glyphFromCell(sheet, column, row, size) {
  const { data } = await sharp(sheet).extract({ left: column * CELL, top: row * CELL, width: CELL, height: CELL })
    .greyscale().raw().toBuffer({ resolveWithObject: true });
  const alpha = Buffer.alloc(CELL * CELL);
  let minX = CELL, minY = CELL, maxX = -1, maxY = -1, ink = 0, rim = 0;
  for (let y = 0; y < CELL; y++) for (let x = 0; x < CELL; x++) {
    const i = y * CELL + x;
    const a = Math.round(Math.min(1, Math.max(0, (data[i] - LOW) / (HIGH - LOW))) * 255);
    alpha[i] = a;
    if (a < 128) continue;
    ink++;
    if (x < 6 || y < 6 || x >= CELL - 6 || y >= CELL - 6) rim++;
    minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y);
  }
  assert.ok(maxX >= minX, `cell ${column},${row}: empty`);
  assert.equal(rim, 0, `cell ${column},${row}: the glyph runs into its neighbour`);
  const width = maxX - minX + 1, height = maxY - minY + 1;
  const rgba = Buffer.alloc(CELL * CELL * 4, 255);
  for (let i = 0; i < CELL * CELL; i++) rgba[i * 4 + 3] = alpha[i];
  const scale = size * FILL / Math.max(width, height);
  const w = Math.max(1, Math.round(width * scale)), h = Math.max(1, Math.round(height * scale));
  const glyph = await sharp(rgba, { raw: { width: CELL, height: CELL, channels: 4 } })
    .extract({ left: minX, top: minY, width, height })
    .resize(w, h, { kernel: "lanczos3" }).png().toBuffer();
  const png = await sharp({ create: { width: size, height: size, channels: 4, background: { r: 255, g: 255, b: 255, alpha: 0 } } })
    .composite([{ input: glyph, left: Math.round((size - w) / 2), top: Math.round((size - h) / 2) }])
    .png({ compressionLevel: 9 }).toBuffer();
  return { png, coverage: ink / (width * height), aspect: width / height };
}

export async function buildMapGlyphs() {
  await mkdir(destination, { recursive: true });
  const validation = JSON.parse(await readFile(path.join(client, GLYPH_SOURCE, "image-validation.json"), "utf8"));
  const catalog = {}, manifest = { source: GLYPH_SOURCE, fill: FILL, levels: [LOW, HIGH], glyphs: {} };
  let written = 0;
  for (const [file, names] of GLYPH_SHEETS) {
    const sheet = await readFile(path.join(client, GLYPH_SOURCE, file));
    const recorded = validation.sheets.find(entry => entry.file === file);
    assert.equal(hash(sheet), recorded?.sha256, `${file}: the sheet differs from its recorded generation`);
    const meta = await sharp(sheet).metadata();
    assert.deepEqual([meta.width, meta.height], [CELL * 3, CELL * 2], `${file}: a 3 × 2 sheet of 512 px cells`);
    for (const [index, name] of names.entries()) {
      const urls = {};
      for (const size of SIZES) {
        const { png, coverage } = await glyphFromCell(sheet, index % 3, Math.floor(index / 3), size);
        assert.ok(coverage > .08 && coverage < .9, `${name}: a solid silhouette (${coverage.toFixed(2)} ink)`);
        const out = `${name}-${size}.png`;
        written += await writeChanged(path.join(destination, out), png);
        urls[size] = `/art/derived/map-glyphs/v1/${out}?v=${hash(png).slice(0, 12)}`;
      }
      catalog[name] = urls[128];
      manifest.glyphs[name] = { sheet: file, cell: index, ...urls };
    }
  }
  written += await writeChanged(path.join(destination, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  const names = Object.keys(catalog);
  written += await writeChanged(path.join(client, "src/map-glyphs.generated.ts"),
    "// Generated by scripts/build-map-glyphs.mjs from art-src/galaxy-map-glyphs-v1. White silhouettes, tinted on the map.\n" +
    `export type MapGlyph = ${names.map(name => JSON.stringify(name)).join(" | ")};\n` +
    `export const MAP_GLYPHS: Record<MapGlyph, string> = ${JSON.stringify(catalog, null, 2)};\n`);
  return { glyphs: names.length, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log("Map glyphs:", await buildMapGlyphs());
}

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));
export const STAR_ART_SIZES = [32, 64, 128, 256, 512, 1024, 1254];
export const STAR_ART_VERSION = "2026-09-08";
const recipe = "full-canvas-lanczos3-rgba-v1";
const sourceSets = {
  galaxy: "stars-galaxy-lens-2026-09-08",
  system: "stars-2026-09-08",
};
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function writeIfChanged(file, text) {
  if (await readFile(file, "utf8").catch(() => null) !== text) await writeFile(file, text);
}

// Geometry belongs to the master, not to each thumbnail's thresholded pixels.
// Keep the full canvas at every level; independent trimming would move the
// anchor and change the apparent size whenever the renderer changes resolution.
async function geometry(source) {
  const { data, info } = await sharp(source).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let left = info.width, top = info.height, right = -1, bottom = -1;
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    if (data[(y * info.width + x) * 4 + 3] < 16) continue;
    left = Math.min(left, x); right = Math.max(right, x);
    top = Math.min(top, y); bottom = Math.max(bottom, y);
  }
  assert.ok(right > left && bottom > top, "star master must have visible pixels");
  return {
    anchor: [(left + right + 1) / (2 * info.width), (top + bottom + 1) / (2 * info.height)],
    visualRatio: Math.max(right - left + 1, bottom - top + 1) / info.width,
  };
}

export async function buildStarDerivatives() {
  const outputRoot = path.join(clientRoot, "public", "art", "derived", "stars", STAR_ART_VERSION);
  await mkdir(outputRoot, { recursive: true });
  const manifestFile = path.join(outputRoot, "manifest.json");
  const previous = await readJson(manifestFile).catch(() => null);
  const expected = (await readJson(path.join(clientRoot, "public", "art", "celestial_sprites", "stars", "icons", "manifest.json")))
    .stars.map(star => star.slug);
  const manifest = {
    version: STAR_ART_VERSION, recipe, masterSize: 1254, sizes: STAR_ART_SIZES,
    // Select by physical framebuffer pixels, not camera ratio or CSS size alone:
    // required = visible CSS diameter / visualRatio * renderer.resolution.
    // Choose the first level >= required. Beyond the native master a raster
    // cannot promise new detail; do not manufacture larger, upscaled files.
    selection: "smallest size >= visible CSS diameter / visualRatio * renderer resolution; native-master ceiling",
    sets: {},
  };
  let sources = 0, written = 0;
  for (const [family, sourceDir] of Object.entries(sourceSets)) {
    const inputRoot = path.join(clientRoot, "art-src", sourceDir);
    const prompts = await readJson(path.join(inputRoot, "prompts.json"));
    assert.deepEqual(prompts.stars.map(star => star.slug), expected, `${family}: complete, stable star set`);
    const outputDir = path.join(outputRoot, family);
    await mkdir(outputDir, { recursive: true });
    manifest.sets[family] = [];
    for (const star of prompts.stars) {
      const input = path.join(inputRoot, star.file);
      const bytes = await readFile(input);
      const sourceSha256 = sha256(bytes);
      const meta = await sharp(bytes).metadata();
      assert.ok(meta.hasAlpha, `${family}/${star.slug}: transparent master required`);
      assert.equal(meta.width, 1254); assert.equal(meta.height, 1254);
      const old = previous?.recipe === recipe && previous?.sets?.[family]?.find(s => s.slug === star.slug);
      const sameSource = old?.sourceSha256 === sourceSha256;
      const levels = [];
      for (const size of STAR_ART_SIZES) {
        const file = `${star.slug}-${size}.png`;
        const output = path.join(outputDir, file);
        const oldLevel = sameSource && old.levels.find(level => level.size === size);
        if (oldLevel && await stat(output).then(s => s.size === oldLevel.bytes).catch(() => false)) {
          levels.push(oldLevel);
          continue;
        }
        if (size === meta.width) {
          // Highest level is byte-for-byte the approved original, never enlarged.
          await copyFile(input, output);
        } else {
          // Each tier is sampled DIRECTLY from the master, not another thumbnail.
          // RGBA PNG stays lossless after resampling; no palette quantization,
          // sharpening, cropping, recoloring or flattening of the lens-flare alpha.
          await sharp(bytes).resize(size, size, { kernel: "lanczos3", withoutEnlargement: true })
            .png({ compressionLevel: 9, palette: false }).toFile(output);
        }
        const derived = await readFile(output);
        const hash = sha256(derived);
        levels.push({ size, file, bytes: derived.length, sha256: hash,
          url: `/art/derived/stars/${STAR_ART_VERSION}/${family}/${file}?v=${hash.slice(0, 12)}` });
        written++;
      }
      manifest.sets[family].push({ slug: star.slug, title: star.title, source: `art-src/${sourceDir}/${star.file}`,
        sourceSha256, ...await geometry(bytes), levels });
      sources++;
    }
  }
  await writeIfChanged(manifestFile, JSON.stringify(manifest, null, 2) + "\n");
  // Bundle just the runtime catalog. Image requests keep their content hashes;
  // no extra manifest fetch, source paths or prompt text reach the renderer.
  const catalog = Object.fromEntries(Object.entries(manifest.sets).map(([family, stars]) => [family,
    Object.fromEntries(stars.map(star => [star.slug, {
      anchor: star.anchor, visualRatio: star.visualRatio,
      levels: star.levels.map(({ size, url }) => ({ size, url })),
    }])),
  ]));
  await writeIfChanged(path.join(clientRoot, "src", "star-art.generated.ts"),
    "// Generated by scripts/build-star-derivatives.mjs. Edit the approved masters, not this catalog.\n" +
    `export const STAR_ART = ${JSON.stringify(catalog, null, 2)} as const;\n`);
  return { sources, variants: sources * STAR_ART_SIZES.length, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log("star derivatives:", await buildStarDerivatives());
}

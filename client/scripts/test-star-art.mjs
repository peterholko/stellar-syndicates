// No server or browser required: validate every delivered resolution against
// its approved master, including alpha and the common normalized geometry.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import sharp from "sharp";
import { buildStarDerivatives, STAR_ART_SIZES, STAR_ART_VERSION } from "./build-star-derivatives.mjs";

const root = new URL("../", import.meta.url);
const output = new URL(`public/art/derived/stars/${STAR_ART_VERSION}/`, root);
const manifestFile = new URL("manifest.json", output);
const manifest = JSON.parse(await readFile(manifestFile, "utf8"));
const catalogFile = new URL("src/star-art.generated.ts", root);
const catalogText = await readFile(catalogFile, "utf8");
const catalog = JSON.parse(catalogText.slice(catalogText.indexOf("= ") + 2, catalogText.lastIndexOf(" as const;")));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const expectedSizes = [32, 64, 128, 256, 512, 1024, 1254];
assert.deepEqual(STAR_ART_SIZES, expectedSizes);
assert.deepEqual(manifest.sizes, expectedSizes);
assert.equal(manifest.masterSize, 1254);
assert.deepEqual(Object.keys(manifest.sets), ["galaxy", "system"]);
assert.deepEqual(manifest.sets.galaxy.map(s => s.slug), manifest.sets.system.map(s => s.slug));
assert.equal(new Set(manifest.sets.galaxy.map(s => s.slug)).size, 10);
let checked = 0;
for (const [family, stars] of Object.entries(manifest.sets)) {
  for (const star of stars) {
    assert.deepEqual(catalog[family][star.slug], { anchor: star.anchor, visualRatio: star.visualRatio,
      levels: star.levels.map(({ size, url }) => ({ size, url })) }, "runtime catalog matches generated assets");
    const source = await readFile(new URL(star.source, root));
    assert.equal(hash(source), star.sourceSha256, "approved source is unchanged");
    assert.ok(star.anchor.every(value => value > 0 && value < 1));
    assert.ok(star.visualRatio > 0 && star.visualRatio <= 1);
    assert.deepEqual(star.levels.map(level => level.size), expectedSizes);
    for (const level of star.levels) {
      const file = new URL(`${family}/${level.file}`, output);
      const bytes = await readFile(file);
      const meta = await sharp(bytes).metadata();
      const label = `${family}/${star.slug}/${level.size}`;
      assert.equal(meta.width, level.size, `${label}: width`);
      assert.equal(meta.height, level.size, `${label}: height`);
      assert.equal(meta.hasAlpha, true, `${label}: graded transparency`);
      assert.equal(meta.isPalette, false, `${label}: no color/alpha quantization`);
      assert.ok(level.size <= manifest.masterSize, `${label}: no invented upscaled tier`);
      assert.equal(bytes.length, level.bytes);
      assert.equal(hash(bytes), level.sha256);
      assert.equal(level.url, `/art/derived/stars/${STAR_ART_VERSION}/${family}/${level.file}?v=${level.sha256.slice(0, 12)}`);
      if (level.size === manifest.masterSize) {
        assert.deepEqual(bytes, source, `${label}: native level must be byte-identical`);
      } else {
        const expected = await sharp(source).resize(level.size, level.size, { kernel: "lanczos3", withoutEnlargement: true })
          .png({ compressionLevel: 9, palette: false }).toBuffer();
        assert.deepEqual(bytes, expected, `${label}: direct master downsample, no chained thumbnails or trim`);
      }
      const { data, info } = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      let clear = 0, partial = 0, peak = 0;
      for (let i = 3; i < data.length; i += 4) {
        const a = data[i];
        if (a === 0) clear++;
        if (a > 0 && a < 255) partial++;
        peak = Math.max(peak, a);
      }
      assert.ok(clear > level.size * level.size * .15, `${label}: empty transparent backdrop`);
      assert.ok(partial > 0 && peak > 200, `${label}: soft edges and a strong core`);
      for (const pixel of [0, info.width - 1, (info.height - 1) * info.width, info.width * info.height - 1]) {
        assert.ok(data[pixel * 4 + 3] <= 2, `${label}: no baked-in corner/background`);
      }
      checked++;
    }
  }
}
assert.equal(checked, 140);

// Actual build is incremental: a second pass must not re-encode or touch assets.
const allFiles = [manifestFile, catalogFile, ...Object.entries(manifest.sets).flatMap(([family, stars]) =>
  stars.flatMap(star => star.levels.map(level => new URL(`${family}/${level.file}`, output))))];
const mtimes = await Promise.all(allFiles.map(async file => (await stat(file)).mtimeMs));
const repeated = await buildStarDerivatives();
assert.deepEqual(repeated, { sources: 20, variants: 140, written: 0 });
assert.deepEqual(await Promise.all(allFiles.map(async file => (await stat(file)).mtimeMs)), mtimes);

// A deliberately blurry chain would be detected by the direct-source check.
const example = manifest.sets.galaxy.find(s => s.slug === "yellow_star");
const source = await readFile(new URL(example.source, root));
const small = await sharp(source).resize(32, 32).png().toBuffer();
const inflated = await sharp(small).resize(256, 256).ensureAlpha().raw().toBuffer();
const actual = await sharp(fileURLToPath(new URL("galaxy/yellow_star-256.png", output))).ensureAlpha().raw().toBuffer();
assert.notDeepEqual(inflated, actual, "regression fixture distinguishes an upscaled thumbnail from native detail");
console.log("Star art: 20 masters / 140 RGBA variants; direct-source sampling, native identity, transparent edges, shared anchors and idempotent build passed.");

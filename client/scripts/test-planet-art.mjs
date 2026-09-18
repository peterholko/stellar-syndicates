// Asset-level checks: real transparent cutouts, gentle direct-source sampling,
// and common geometry across all resolutions. No live game required.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import sharp from "sharp";
import { buildPlanetDerivatives, PLANET_ART_KINDS, PLANET_ART_SIZES, PLANET_ART_VERSION } from "./build-planet-derivatives.mjs";

const root = new URL("../", import.meta.url);
const output = new URL(`public/art/derived/planets/${PLANET_ART_VERSION}/`, root);
const manifestFile = new URL("manifest.json", output);
const catalogFile = new URL("src/planet-art.generated.ts", root);
const manifest = JSON.parse(await readFile(manifestFile, "utf8"));
const text = await readFile(catalogFile, "utf8");
const catalog = JSON.parse(text.slice(text.indexOf("= ") + 2, text.lastIndexOf(" as const;")));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
assert.deepEqual(PLANET_ART_SIZES, [32, 64, 128, 256, 512, 1024, 1254]);
assert.deepEqual(manifest.planets.map(p => p.slug), PLANET_ART_KINDS);
assert.equal(new Set(manifest.planets.map(p => p.sourceSha256)).size, 8, "distinct art for every body type");
let checked = 0;
for (const planet of manifest.planets) {
  assert.deepEqual(catalog[planet.slug], { anchor: planet.anchor, visualRatio: planet.visualRatio,
    levels: planet.levels.map(({ size, url }) => ({ size, url })) });
  assert.ok(planet.anchor.every(value => value > .45 && value < .55), "globe centered in its canvas");
  assert.ok(planet.visualRatio > .8 && planet.visualRatio < 1, "large complete disk, not a tiny globe or clipped limb");
  assert.deepEqual(planet.levels.map(level => level.size), PLANET_ART_SIZES);
  const source = await readFile(new URL(planet.source, root));
  assert.equal(hash(source), planet.sourceSha256);
  if (["desert", "ocean", "gas_giant", "barren", "moon"].includes(planet.slug)) {
    const original = await sharp(new URL(`art-src/planets-${PLANET_ART_VERSION}/originals/${planet.slug}.png`, root).pathname)
      .removeAlpha().raw().toBuffer();
    const cleanedRgb = await sharp(source).removeAlpha().raw().toBuffer();
    assert.deepEqual(cleanedRgb, original, `${planet.slug}: cleanup changes alpha only, not the painted surface`);
    const { data, info } = await sharp(source).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
      if (Math.hypot(x - info.width / 2, y - info.height / 2) < 550) {
        assert.equal(data[(y * info.width + x) * 4 + 3], 255, "interior surface cannot acquire transparency holes");
      }
    }
  }
  for (const level of planet.levels) {
    const bytes = await readFile(new URL(level.file, output));
    const meta = await sharp(bytes).metadata();
    const label = `${planet.slug}/${level.size}`;
    assert.equal(meta.width, level.size, label);
    assert.equal(meta.height, level.size, label);
    assert.equal(meta.hasAlpha, true, `${label}: actual RGBA`);
    assert.equal(meta.isPalette, false, `${label}: no color quantization`);
    assert.equal(bytes.length, level.bytes);
    assert.equal(hash(bytes), level.sha256);
    assert.equal(level.url, `/art/derived/planets/${PLANET_ART_VERSION}/${level.file}?v=${level.sha256.slice(0, 12)}`);
    const expected = level.size === 1254 ? source : await sharp(source)
      .resize(level.size, level.size, { kernel: "mitchell", withoutEnlargement: true })
      .png({ compressionLevel: 9, palette: false }).toBuffer();
    assert.deepEqual(bytes, expected, `${label}: one gentle downsample from native, no sharpen or inflated thumbnail`);
    const { data, info } = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    let clear = 0, partial = 0;
    for (let i = 3; i < data.length; i += 4) {
      if (data[i] === 0) clear++;
      if (data[i] > 0 && data[i] < 255) partial++;
    }
    assert.ok(clear > info.width * info.height * .15, `${label}: no opaque or painted checkerboard background`);
    assert.ok(partial > 0, `${label}: antialiased limb`);
    for (const pixel of [0, info.width - 1, (info.height - 1) * info.width, info.width * info.height - 1]) {
      assert.ok(data[pixel * 4 + 3] <= 2, `${label}: empty corners`);
    }
    checked++;
  }
}
assert.equal(checked, 56);
const files = [manifestFile, catalogFile, ...manifest.planets.flatMap(p => p.levels.map(l => new URL(l.file, output)))];
const mtimes = await Promise.all(files.map(async file => (await stat(file)).mtimeMs));
assert.deepEqual(await buildPlanetDerivatives(), { sources: 8, variants: 56, written: 0 });
assert.deepEqual(await Promise.all(files.map(async file => (await stat(file)).mtimeMs)), mtimes);
console.log("Planet art: 8 distinct masters / 56 RGBA variants; transparent edges, direct Mitchell sampling, common geometry and idempotent build pass.");

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const root = path.join(client, "public/art/derived/planet-shipyards/v1");
const families = ["shipyard", "naval_drydock", "capital_slipway"];
const assets = families.flatMap(family => Array.from({ length: 6 }, (_, i) => ({ family, tier: i + 1, variant: "" })))
  .concat(Array.from({ length: 6 }, (_, i) => ({ family: "naval_drydock", tier: i + 1, variant: "-occupied" })));
const sizes = [128, 256, 512];
const recipe = "generated-alpha-native-aspect-v1";

export async function buildPlanetShipyardArt() {
  await mkdir(root, { recursive: true });
  const manifestPath = path.join(root, "manifest.json");
  const oldText = await readFile(manifestPath, "utf8").catch(() => "");
  const old = oldText ? JSON.parse(oldText) : null;
  const manifest = { recipe, sizes, assets: {} };
  let written = 0;
  for (const { family, tier, variant } of assets) {
    await mkdir(path.join(root, family), { recursive: true });
    const source = `art-src/planet-shipyards-v1/${family}/tier-${tier}${variant}.png`;
    const bytes = await readFile(path.join(client, source));
    const sourceSha256 = createHash("sha256").update(bytes).digest("hex");
    const id = `${family}/${tier}${variant}`;
    const prior = old?.recipe === recipe && old.assets[id];
    const cached = prior?.sourceSha256 === sourceSha256;
    const meta = await sharp(bytes).metadata();
    assert.ok(meta.hasAlpha && meta.width >= 1024, `${id}: generated transparent master`);
    if (!cached) {
      const alpha = await sharp(bytes).extractChannel("alpha").stats();
      assert.ok(alpha.channels[0].min === 0 && alpha.channels[0].max === 255
        && alpha.channels[0].mean < 230, `${id}: actual transparency`);
    }
    const levels = [];
    for (const size of sizes) {
      const file = `${family}/tier-${tier}${variant}-${size}.webp`;
      const output = path.join(root, file);
      const previous = cached && prior.levels.find(level => level.size === size);
      if (previous && await stat(output).then(s => s.size === previous.bytes).catch(() => false)) {
        levels.push(previous);
        continue;
      }
      // Resize/encode only: keep the generated alpha, padding and native aspect.
      // The renderer fits orbital art inside a horizon-safe, wide rectangle.
      await sharp(bytes).resize({ width: size }).webp({ quality: 92, alphaQuality: 100, effort: 4 }).toFile(output);
      levels.push({ size, bytes: (await stat(output)).size, url: `/art/derived/planet-shipyards/v1/${file}` });
      written++;
    }
    manifest.assets[id] = { source, sourceSha256, width: meta.width, height: meta.height, levels };
  }
  const text = `${JSON.stringify(manifest, null, 2)}\n`;
  if (text !== oldText) await writeFile(manifestPath, text);
  return { sources: assets.length, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) console.log(await buildPlanetShipyardArt());

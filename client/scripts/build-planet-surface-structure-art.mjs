import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const sourceRoot = path.join(client, "art-src/planet-surface-structures-v2");
const root = path.join(client, "public/art/derived/planet-surface-structures/v2");
const sizes = [128, 256, 512];
const recipe = "shared-building-biome-native-alpha-v1";
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
// §gaia/§marginal: the Oct 2026 additions were normalised to the framing the v2
// prompts asked for, which the approved set itself overshoots (buildings fill
// about 94% of the canvas width, grounds x 2–98%). Each family is enlarged once,
// about one fixed canvas point, so its tiers still share a framing and stand at
// their neighbours' scale. Masters stay untouched; every size is still a
// downscale from the 1024 px master.
export const FRAMING = {
  "buildings/living_terraces": { scale: 1.16, anchor: [0.5, 0.38] },
  "grounds/marginal": { scale: 1.2, anchor: [0.5, 0.755] },
};

async function writeChanged(file, text) {
  if (await readFile(file, "utf8").catch(() => "") !== text) await writeFile(file, text);
}

export async function buildPlanetSurfaceStructureArt() {
  const catalog = JSON.parse(await readFile(path.join(sourceRoot, "source-manifest.json"), "utf8"));
  // Later approved art (§gaia Living Terraces, the Marginal ground) is listed
  // separately so the original review set's provenance manifest stays untouched.
  const additions = JSON.parse(await readFile(path.join(sourceRoot, "additions-manifest.json"), "utf8"));
  // A replacement (Agroplex IV–VI) takes the place of the original entry for the
  // same family and tier, and ships under the original's id. The original master
  // stays on disk and in the source manifest as provenance.
  const replacements = new Map(additions.replacements.map(a => [a.replaces, a]));
  const originals = catalog.buildings.filter(a => replacements.has(a.file));
  assert.equal(originals.length, replacements.size, "every replacement names an original master");
  // The shipyard task promoted the ground yard already, and replaced the two
  // surface support-terminal concepts with true floating orbital facilities.
  const buildings = [...catalog.buildings.map(a => replacements.get(a.file) ?? a), ...additions.buildings]
    .filter(a => !["shipyard", "naval_drydock", "capital_slipway"].includes(a.family));
  const biomes = [...catalog.biomes, ...additions.biomes];
  const assets = [...buildings, ...biomes];
  assert.equal(buildings.length, 142);
  assert.equal(biomes.length, 13);
  assert.equal(new Set(assets.map(a => a.file)).size, 155);
  const families = Object.fromEntries([...new Set(buildings.map(a => a.family))].sort().map(key => {
    const tiers = buildings.filter(a => a.family === key).map(a => a.tier).sort((a, b) => a - b);
    const max = key === "command_center" ? 3 : key === "service_berth" ? 1 : 6;
    assert.deepEqual(tiers, Array.from({ length: max }, (_, i) => i + 1), `${key}: complete tiers`);
    return [key, max];
  }));
  await mkdir(root, { recursive: true });
  const manifestPath = path.join(root, "manifest.json");
  const old = JSON.parse(await readFile(manifestPath, "utf8").catch(() => "null"));
  const manifest = { recipe, sizes, families, biomes: biomes.map(a => a.key), assets: {} };
  let written = 0;
  for (const asset of assets) {
    const id = (asset.replaces ?? asset.file).replace(/\.png$/, "");
    assert.match(id, /^(buildings\/[a-z_]+\/tier-[1-6]|grounds\/[a-z-]+)$/);
    const source = path.join(sourceRoot, asset.file);
    const bytes = await readFile(source), sourceSha256 = sha256(bytes);
    assert.equal(sourceSha256, asset.validation.sha256, `${id}: approved master is unchanged`);
    const prompt = JSON.parse(await readFile(path.join(sourceRoot, asset.promptFile), "utf8"));
    assert.ok(prompt.prompt, `${id}: original generation prompt retained`);
    const framing = FRAMING[id.replace(/\/tier-\d$/, "")] ?? null;
    const prior = old?.recipe === recipe && old.assets[id];
    const cached = prior?.sourceSha256 === sourceSha256
      && JSON.stringify(prior.framing ?? null) === JSON.stringify(framing);
    const meta = await sharp(bytes).metadata();
    assert.ok(meta.hasAlpha && meta.width >= 1024 && meta.height === meta.width, `${id}: native square alpha master`);
    if (!cached) {
      const alpha = (await sharp(bytes).extractChannel("alpha").stats()).channels[0];
      assert.ok(alpha.min === 0 && alpha.max >= 250 && alpha.mean < 230, `${id}: real transparent surroundings`);
    }
    const levels = [];
    await mkdir(path.dirname(path.join(root, id)), { recursive: true });
    for (const size of sizes) {
      const output = path.join(root, `${id}-${size}.webp`);
      const previous = cached && prior.levels.find(level => level.size === size);
      if (previous && await stat(output).then(s => s.size === previous.bytes).catch(() => false)) {
        levels.push(previous);
        continue;
      }
      // Encoding only, plus a family's one shared FRAMING. Never trim individual
      // tiers: their shared framing is what keeps a building seated on the same
      // biome layer across upgrades.
      let image = sharp(bytes);
      if (framing) {
        const scaled = Math.round(size * framing.scale);
        image = image.resize(scaled, scaled).extract({ width: size, height: size,
          left: Math.round(framing.anchor[0] * (scaled - size)), top: Math.round(framing.anchor[1] * (scaled - size)) });
      } else image = image.resize(size, size);
      await image.webp({ quality: 92, alphaQuality: 100, effort: 4 }).toFile(output);
      const encoded = await readFile(output);
      levels.push({ size, bytes: encoded.length, sha256: sha256(encoded),
        url: `/art/derived/planet-surface-structures/v2/${id}-${size}.webp` });
      written++;
    }
    manifest.assets[id] = { source: path.relative(client, source), sourceSha256,
      width: meta.width, height: meta.height, ...(framing && { framing }), levels };
  }
  await writeChanged(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await writeChanged(path.join(client, "src/planet-surface-structure-art.generated.ts"),
    `// Generated by scripts/build-planet-surface-structure-art.mjs. Do not edit.\n`
    + `export const SURFACE_STRUCTURE_TIERS: Readonly<Record<string, number>> = ${JSON.stringify(families, null, 2)};\n`
    + `export const SURFACE_GROUND_LAYERS: readonly string[] = ${JSON.stringify(manifest.biomes, null, 2)};\n`);
  return { sources: assets.length, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) console.log(await buildPlanetSurfaceStructureArt());

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const version = "2026-09-16";
const recipe = "transparent-tier-contain-padding-v1";
const sourceRoot = path.join(client, "art-src", `structure-tiers-${version}`);
const outputRoot = path.join(client, "public", "art", "derived", "structures", version);
const sizes = [128, 256, 512];

export async function buildStructureTierArt() {
  const catalog = JSON.parse(await readFile(path.join(sourceRoot, "catalog.json"), "utf8"));
  const families = ["mining-complex", ...catalog.families.map(family => family.id)];
  assert.equal(new Set(families).size, 25, "Every structure has six approved designs");
  await mkdir(outputRoot, { recursive: true });
  const manifestPath = path.join(outputRoot, "manifest.json");
  const previousText = await readFile(manifestPath, "utf8").catch(() => "");
  const previous = previousText ? JSON.parse(previousText) : null;
  const manifest = { version, recipe, sizes, structures: {} };
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let written = 0;
  for (const family of families) {
    const key = family.replaceAll("-", "_");
    const directory = path.join(outputRoot, key);
    await mkdir(directory, { recursive: true });
    manifest.structures[key] = {};
    for (let tier = 1; tier <= 6; tier++) {
      const source = path.join(sourceRoot, family, `tier-${tier}.png`);
      const bytes = await readFile(source);
      const sourceSha256 = createHash("sha256").update(bytes).digest("hex");
      const old = previous?.recipe === recipe && previous.structures[key]?.[tier];
      const cached = old?.sourceSha256 === sourceSha256;
      if (!cached) {
        const meta = await sharp(bytes).metadata();
        assert.ok(meta.hasAlpha && meta.width >= 512 && meta.width === meta.height, `${family} ${tier}: square transparent master`);
        const alpha = await sharp(bytes).extractChannel("alpha").stats();
        assert.ok(alpha.channels[0].min === 0 && alpha.channels[0].max === 255
          && alpha.channels[0].mean < 240, `${family} ${tier}: real cutout, not a baked background`);
      }
      const levels = [];
      for (const size of sizes) {
        const file = `tier-${tier}-${size}.webp`;
        const output = path.join(directory, file);
        const oldLevel = cached && old.levels.find(level => level.size === size);
        if (oldLevel && await stat(output).then(s => s.size === oldLevel.bytes).catch(() => false)) {
          levels.push(oldLevel);
          continue;
        }
        // Match the old 6/128 transparent padding ratio. Only empty padding is
        // trimmed; the authored building is never repainted or upscaled. All
        // resolutions share one framing, so a srcset swap cannot resize it.
        const pad = size * 6 / 128;
        await sharp(bytes).trim({ background: transparent, threshold: 3 })
          .resize(size - pad * 2, size - pad * 2, { fit: "contain", background: transparent })
          .extend({ top: pad, bottom: pad, left: pad, right: pad, background: transparent })
          .webp({ quality: 92, alphaQuality: 100, effort: 4 }).toFile(output);
        levels.push({ size, bytes: (await stat(output)).size,
          url: `/art/derived/structures/${version}/${key}/${file}` });
        written++;
      }
      manifest.structures[key][tier] = { source: path.relative(client, source), sourceSha256, levels };
    }
  }
  const text = `${JSON.stringify(manifest, null, 2)}\n`;
  if (text !== previousText) await writeFile(manifestPath, text);
  return { sources: families.length * 6, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  console.log(await buildStructureTierArt());
}

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const sourceRoot = path.join(client, "art-src/planet-surface-structures-v1/command-center");
const outputRoot = path.join(client, "public/art/derived/command-center/v1");
const recipe = "generated-alpha-contain-padding-v1";
const sizes = [128, 256, 512];

export async function buildCommandCenterArt() {
  await mkdir(outputRoot, { recursive: true });
  const manifestPath = path.join(outputRoot, "manifest.json");
  const previousText = await readFile(manifestPath, "utf8").catch(() => "");
  const previous = previousText ? JSON.parse(previousText) : null;
  const manifest = { recipe, sizes, tiers: {} };
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let written = 0;
  for (let tier = 1; tier <= 3; tier++) {
    const source = path.join(sourceRoot, `tier-${tier}.png`);
    const bytes = await readFile(source);
    const sourceSha256 = createHash("sha256").update(bytes).digest("hex");
    const old = previous?.recipe === recipe && previous.tiers[tier];
    const cached = old?.sourceSha256 === sourceSha256;
    if (!cached) {
      const meta = await sharp(bytes).metadata();
      assert.ok(meta.hasAlpha && meta.width >= 512 && meta.width === meta.height, `Command ${tier}: transparent square master`);
      const alpha = await sharp(bytes).extractChannel("alpha").stats();
      assert.ok(alpha.channels[0].min === 0 && alpha.channels[0].max === 255 && alpha.channels[0].mean < 240,
        `Command ${tier}: actual transparency, not a painted checkerboard`);
    }
    const levels = [];
    for (const size of sizes) {
      const file = `tier-${tier}-${size}.webp`;
      const output = path.join(outputRoot, file);
      const oldLevel = cached && old.levels.find(level => level.size === size);
      if (oldLevel && await stat(output).then(s => s.size === oldLevel.bytes).catch(() => false)) {
        levels.push(oldLevel);
        continue;
      }
      // Only resize/encode generated alpha; never repaint the approved building.
      const pad = size * 6 / 128;
      await sharp(bytes).trim({ background: transparent, threshold: 3 })
        .resize(size - pad * 2, size - pad * 2, { fit: "contain", background: transparent })
        .extend({ top: pad, bottom: pad, left: pad, right: pad, background: transparent })
        .webp({ quality: 92, alphaQuality: 100, effort: 4 }).toFile(output);
      levels.push({ size, bytes: (await stat(output)).size, url: `/art/derived/command-center/v1/${file}` });
      written++;
    }
    manifest.tiers[tier] = { source: path.relative(client, source), sourceSha256, levels };
  }
  const text = `${JSON.stringify(manifest, null, 2)}\n`;
  if (text !== previousText) await writeFile(manifestPath, text);
  return { sources: 3, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) console.log(await buildCommandCenterArt());

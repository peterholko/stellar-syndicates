import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const version = "v1";
const recipe = "transparent-station-contain-padding-v1";
const sourceRoot = path.join(client, "art-src", "orbital-defense-v1");
const outputRoot = path.join(client, "public", "art", "derived", "stations", version);
const sizes = [128, 256, 512];
const levelCount = 5;

export async function buildStationArt() {
  await mkdir(outputRoot, { recursive: true });
  const manifestPath = path.join(outputRoot, "manifest.json");
  const previousText = await readFile(manifestPath, "utf8").catch(() => "");
  const previous = previousText ? JSON.parse(previousText) : null;
  const manifest = { version, recipe, sizes, stations: {} };
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let written = 0;
  for (let level = 1; level <= levelCount; level++) {
    const source = path.join(sourceRoot, `level-${level}.png`);
    const bytes = await readFile(source);
    const sourceSha256 = createHash("sha256").update(bytes).digest("hex");
    const old = previous?.recipe === recipe && previous.stations[level];
    const cached = old?.sourceSha256 === sourceSha256;
    if (!cached) {
      // Masters are native-resolution cutouts from the approved lineup, so the
      // small levels are well under 512 px: no size floor, only real alpha.
      const meta = await sharp(bytes).metadata();
      assert.ok(meta.hasAlpha && meta.width === meta.height, `Station ${level}: square transparent master`);
      const alpha = await sharp(bytes).extractChannel("alpha").stats();
      assert.ok(alpha.channels[0].min === 0 && alpha.channels[0].max === 255
        && alpha.channels[0].mean < 240, `Station ${level}: real cutout, not a baked background`);
    }
    const levels = [];
    for (const size of sizes) {
      const file = `level-${level}-${size}.webp`;
      const output = path.join(outputRoot, file);
      const oldLevel = cached && old.levels.find(entry => entry.size === size);
      if (oldLevel && await stat(output).then(s => s.size === oldLevel.bytes).catch(() => false)) {
        levels.push(oldLevel);
        continue;
      }
      // Match the structure tiers' 6/128 transparent padding ratio. Only empty
      // padding is trimmed; the station is never repainted. Small masters are
      // enlarged for the bigger sizes (Lanczos-3), and every resolution shares
      // one framing, so a srcset swap cannot resize the station.
      const pad = size * 6 / 128;
      await sharp(bytes).trim({ background: transparent, threshold: 3 })
        .resize(size - pad * 2, size - pad * 2, { fit: "contain", background: transparent, kernel: sharp.kernel.lanczos3 })
        .extend({ top: pad, bottom: pad, left: pad, right: pad, background: transparent })
        .webp({ quality: 92, alphaQuality: 100, effort: 4 }).toFile(output);
      levels.push({ size, bytes: (await stat(output)).size, url: `/art/derived/stations/${version}/${file}` });
      written++;
    }
    manifest.stations[level] = { source: path.relative(client, source), sourceSha256, levels };
  }
  const text = `${JSON.stringify(manifest, null, 2)}\n`;
  if (text !== previousText) await writeFile(manifestPath, text);
  return { sources: levelCount, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  console.log(await buildStationArt());
}

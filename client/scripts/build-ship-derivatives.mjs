import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const version = "2026-09-08";
const recipe = "native-rgba-visible-length-v1";
const sizes = [128, 256, 512, 1024, 1254];
const sourceRoot = path.join(client, "art-src", `ships-${version}`);
const outputRoot = path.join(client, "public", "art", "derived", "ships", version);
const combat = new Set(["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"]);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const json = async file => JSON.parse(await readFile(file, "utf8"));
async function writeIfChanged(file, text) {
  if (await readFile(file, "utf8").catch(() => null) !== text) await writeFile(file, text);
}

export async function hullGeometry(bytes) {
  const { data, info } = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let left = info.width, top = info.height, right = -1, bottom = -1;
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    if (data[(y * info.width + x) * 4 + 3] < 128) continue;
    left = Math.min(left, x); right = Math.max(right, x);
    top = Math.min(top, y); bottom = Math.max(bottom, y);
  }
  assert.ok(bottom > top && right > left, "Hull must have a solid visible silhouette");
  return { anchor: [(left + right + 1) / (2 * info.width), (top + bottom + 1) / (2 * info.height)],
    lengthRatio: (bottom - top + 1) / info.height };
}

export async function buildShipDerivatives() {
  await mkdir(outputRoot, { recursive: true });
  const manifestFile = path.join(outputRoot, "manifest.json");
  const previous = await json(manifestFile).catch(() => null);
  const prompts = await json(path.join(sourceRoot, "prompts.json"));
  assert.equal(prompts.ships.length, 13, "Every approved hull, including Authority freight");
  const freightRoot = path.join(client, "art-src", "freighters-2026-09-16");
  const freight = await json(path.join(freightRoot, "prompts.json"));
  assert.equal(freight.ships.length, 6, "Six player freight sizes");
  const ships = [...prompts.ships.filter(s => s.kind !== "convoy").map(s => ({ ...s, input: path.join(sourceRoot, `${s.slug}.png`) })),
    ...freight.ships.map(s => ({ ...s, input: path.join(freightRoot, `${s.slug}-alpha.png`) }))];
  const oldRoot = path.join(client, "public", "art", "ship_sprites");
  const interceptor = await hullGeometry(await readFile(path.join(oldRoot, "raider_attack_ship.png")));
  const manifest = { version, recipe, sizes, ships: {}, formations: {} };
  let written = 0;
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  for (const ship of ships) {
    const input = ship.input;
    const bytes = await readFile(input);
    const meta = await sharp(bytes).metadata();
    assert.ok(meta.hasAlpha && meta.width === 1254 && meta.height === 1254, `${ship.slug}: native transparent master`);
    const geometry = await hullGeometry(bytes);
    const legacy = await hullGeometry(await readFile(path.join(oldRoot, ship.existing)));
    // The old Interceptor's visible length is the ruler for the approved combat
    // ladder. Civilians retain their previous footprint despite new PNG padding.
    const referenceRatio = combat.has(ship.kind) ? interceptor.lengthRatio : legacy.lengthRatio;
    const sourceSha256 = hash(bytes);
    const old = previous?.recipe === recipe && previous.ships[ship.kind];
    const levels = [];
    for (const size of sizes) {
      const file = `${ship.kind}-${size}.png`;
      const output = path.join(outputRoot, file);
      const cached = old?.sourceSha256 === sourceSha256 && old.levels.find(level => level.size === size);
      if (cached && await stat(output).then(s => s.size === cached.bytes).catch(() => false)) {
        levels.push(cached);
        continue;
      }
      // Full canvas at every level: neither zoom nor a late texture upgrade can
      // recenter/trim the hull differently. The largest tier IS the original;
      // no invented detail, resharpening, recoloring, or artificial enlargement.
      if (size === meta.width) await copyFile(input, output);
      else await sharp(bytes).resize(size, size, { kernel: "lanczos3", withoutEnlargement: true })
        .png({ compressionLevel: 9 }).toFile(output);
      const outputBytes = await readFile(output);
      levels.push({ size, bytes: outputBytes.length,
        url: `/art/derived/ships/${version}/${file}?v=${hash(outputBytes).slice(0, 12)}` });
      written++;
    }
    manifest.ships[ship.kind] = { source: path.relative(client, input), sourceSha256,
      ...geometry, referenceRatio, calib: referenceRatio / geometry.lengthRatio, levels };
    // Panels use the SAME design, at small/Retina sizes, not the old separate
    // builder silhouettes. Fixed layout dimensions remain controlled by CSS.
    const panels = path.join(outputRoot, "panels");
    await mkdir(panels, { recursive: true });
    for (const size of [128, 256]) {
      const output = path.join(panels, `${ship.kind}-${size}.png`);
      if (old?.sourceSha256 === sourceSha256 && await stat(output).catch(() => null)) continue;
      await sharp(bytes).resize(size, size, { withoutEnlargement: true }).png({ compressionLevel: 9 }).toFile(output);
      written++;
    }
    // Existing multi-hull LOD keeps its lead hull and honest count badge. Rebuild
    // those composites too, or a second ship would resurrect the retired art.
    const family = ({ convoy: "freighter", raider: "raider", corvette: "corvette", scout: "scout" })[ship.kind];
    if (!family) continue;
    const formationSize = 512;
    for (const [tier, escorts] of [["wing", 2], ["squadron", 4], ["armada", 6]]) {
      const file = `fleet_${family}_${tier}.png`;
      const output = path.join(outputRoot, file);
      const key = `${family}_${tier}`;
      const cached = previous?.recipe === recipe && previous.formations[key];
      if (cached?.sourceSha256 === sourceSha256 && await stat(output).then(s => s.size === cached.bytes).catch(() => false)) {
        manifest.formations[key] = cached;
        continue;
      }
      const escort = await sharp(bytes).resize(154, 154).png().toBuffer();
      const layers = Array.from({ length: escorts }, (_, i) => ({ input: escort,
        left: i % 2 ? 350 : 8, top: 28 + Math.floor(i / 2) * 105 }));
      layers.push({ input: await sharp(bytes).resize(formationSize, formationSize).png().toBuffer(), left: 0, top: 0 });
      await sharp({ create: { width: formationSize, height: formationSize, channels: 4, background: transparent } })
        .composite(layers).png({ compressionLevel: 9 }).toFile(output);
      const outputBytes = await readFile(output);
      manifest.formations[key] = { sourceSha256, bytes: outputBytes.length,
        url: `/art/derived/ships/${version}/${file}?v=${hash(outputBytes).slice(0, 12)}` };
      written++;
    }
  }
  await writeIfChanged(manifestFile, JSON.stringify(manifest, null, 2) + "\n");
  const catalog = Object.fromEntries(Object.entries(manifest.ships).map(([kind, ship]) => [kind, {
    anchor: ship.anchor, lengthRatio: ship.lengthRatio, referenceRatio: ship.referenceRatio, calib: ship.calib,
    levels: ship.levels.map(({ size, url }) => ({ size, url })),
  }]));
  await writeIfChanged(path.join(client, "src", "ship-art.generated.ts"),
    "// Generated by scripts/build-ship-derivatives.mjs. Edit the approved masters, not this catalog.\n" +
    `export const SHIP_ART = ${JSON.stringify(catalog, null, 2)} as const;\n` +
    `export const SHIP_FORMATIONS: Record<string, string> = ${JSON.stringify(Object.fromEntries(
      Object.entries(manifest.formations).map(([key, value]) => [key, value.url])), null, 2)};\n`);
  return { sources: ships.length, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log("ship derivatives:", await buildShipDerivatives());
}

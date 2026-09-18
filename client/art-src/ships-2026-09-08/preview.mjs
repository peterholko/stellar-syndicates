// Art-review sheets only. All resized/composited images are copies; masters and
// the game's current sprite files are left untouched. Run with Node.
import assert from 'node:assert/strict';
import sharp from 'sharp';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const root = new URL('./', import.meta.url);
const manifest = JSON.parse(await readFile(new URL('prompts.json', root), 'utf8'));
const oldRoot = new URL('../../public/art/ship_sprites/', root);
const background = '#07101b';
const combat = new Set(['interceptor', 'corvette', 'destroyer', 'cruiser', 'battleship', 'dreadnought', 'titan']);
const measurements = [];
const escape = value => value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
assert.equal(manifest.ships.length, 13);
assert.equal(new Set(manifest.ships.map(ship => ship.slug)).size, 13);

async function text(layers, value, x, y, width, size = 20, color = '#dae7f5') {
  const { data, info } = await sharp({ text: {
    text: `<span foreground="${color}">${escape(value)}</span>`,
    font: `Arial ${size}`, width, align: 'centre', rgba: true,
  } }).png().toBuffer({ resolveWithObject: true });
  layers.push({ input: data, left: x + Math.floor((width - info.width) / 2), top: y });
}

async function bounds(file) {
  const bytes = await readFile(file);
  const meta = await sharp(bytes).metadata();
  assert.ok(meta.hasAlpha, `${file}: real alpha channel required`);
  assert.equal(meta.width, meta.height, `${file}: square master`);
  const { data, info } = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let left = info.width, top = info.height, right = -1, bottom = -1, clear = 0, opaque = 0;
  const alpha = (x, y) => data[(y * info.width + x) * 4 + 3];
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    const a = alpha(x, y);
    if (a === 0) clear++;
    if (a > 240) opaque++;
    if (a >= 16) { left = Math.min(left, x); top = Math.min(top, y); right = Math.max(right, x); bottom = Math.max(bottom, y); }
  }
  assert.ok(clear > info.width * info.height * .20, `${file}: backdrop must be empty`);
  assert.ok(opaque > info.width * info.height * .02, `${file}: solid hull required`);
  for (const [x, y] of [[0, 0], [info.width - 1, 0], [0, info.height - 1], [info.width - 1, info.height - 1]]) {
    // Some generated edges contain a single alpha-quantization step (1/255).
    // Preserve the supplied alpha; this is visually transparent, not a fill.
    assert.ok(alpha(x, y) <= 1, `${file}: effectively transparent corner`);
  }
  assert.ok(left > 0 && top > 0 && right < info.width - 1 && bottom < info.height - 1, `${file}: hull must not clip the canvas`);
  return { canvas: [info.width, info.height], bbox: { left, top, width: right - left + 1, height: bottom - top + 1 },
    transparentFraction: clear / (info.width * info.height), sha256: createHash('sha256').update(bytes).digest('hex') };
}

for (const ship of manifest.ships) {
  const file = fileURLToPath(new URL(`${ship.slug}.png`, root));
  const data = await bounds(file);
  assert.ok(data.canvas[0] >= 1024, `${ship.slug}: native high-resolution master required`);
  measurements.push({ slug: ship.slug, file: `${ship.slug}.png`, ...data });
}

async function thumbnail(ship, size) {
  const measure = measurements.find(row => row.slug === ship.slug);
  return sharp(fileURLToPath(new URL(measure.file, root))).extract(measure.bbox)
    .resize(size, size, { fit: 'contain', background: '#00000000', withoutEnlargement: true }).png().toBuffer();
}

async function sheet(ships, columns, title, file) {
  const cellW = 320, cellH = 420, header = 92;
  const width = columns * cellW, height = header + Math.ceil(ships.length / columns) * cellH + 16;
  const layers = [];
  await text(layers, title, 0, 22, width, 26);
  await text(layers, 'New hull candidates · preview scale, not gameplay size · current game art unchanged', 0, 61, width, 14, '#8fa4be');
  for (const [i, ship] of ships.entries()) {
    const x = (i % columns) * cellW, y = header + Math.floor(i / columns) * cellH;
    await text(layers, ship.title, x + 8, y + 4, cellW - 16, 21);
    layers.push({ input: await thumbnail(ship, 272), left: x + 24, top: y + 40 });
    layers.push({ input: await thumbnail(ship, 48), left: x + 72, top: y + 332 });
    layers.push({ input: await thumbnail(ship, 72), left: x + 180, top: y + 320 });
    await text(layers, '48 px', x + 52, y + 396, 88, 12, '#8fa4be');
    await text(layers, '72 px', x + 168, y + 396, 96, 12, '#8fa4be');
  }
  await sharp({ create: { width, height, channels: 4, background } }).composite(layers).png()
    .toFile(fileURLToPath(new URL(file, root)));
}

await sheet(manifest.ships.filter(ship => combat.has(ship.slug)), 4, 'STELLAR SYNDICATES · COMBAT HULLS', 'combat-overview.png');
await sheet(manifest.ships.filter(ship => !combat.has(ship.slug)), 3, 'STELLAR SYNDICATES · CIVILIAN & SUPPORT', 'support-overview.png');

// The equal-height sheets above compare designs, not vessel scale. This sheet
// makes the requested capital hierarchy explicit using visible hull lengths.
// These are proposed art-presentation ratios, not sim dimensions or map caps.
const scaleLadder = [
  ['interceptor', 1], ['corvette', 1.5], ['destroyer', 2.5],
  ['cruiser', 4], ['battleship', 5], ['dreadnought', 6], ['titan', 8],
];
const referenceLength = 76, gap = 20, padding = 32, baseline = 728;
const scaleLayers = [];
const scaleColumns = scaleLadder.map(([slug, ratio]) => {
  const ship = manifest.ships.find(row => row.slug === slug);
  const measure = measurements.find(row => row.slug === slug);
  const height = Math.round(referenceLength * ratio);
  const width = Math.round(height * measure.bbox.width / measure.bbox.height);
  assert.ok(width <= measure.bbox.width && height <= measure.bbox.height, `${slug}: scale preview must not upscale the master`);
  return { ship, measure, ratio, width, height, column: Math.max(116, width + 8) };
});
const scaleWidth = padding * 2 + scaleColumns.reduce((sum, row) => sum + row.column, 0) + gap * (scaleColumns.length - 1);
await text(scaleLayers, 'STELLAR SYNDICATES · CAPITAL SHIP SCALE', 0, 24, scaleWidth, 29);
await text(scaleLayers, 'One shared length scale — capitals dwarf the Interceptor', 0, 68, scaleWidth, 20, '#aebfd3');
let scaleX = padding;
for (const row of scaleColumns) {
  const file = fileURLToPath(new URL(row.measure.file, root));
  scaleLayers.push({ input: await sharp(file).extract(row.measure.bbox)
    .resize(row.width, row.height, { fit: 'fill', withoutEnlargement: true }).png().toBuffer(),
    left: scaleX + Math.floor((row.column - row.width) / 2), top: baseline - row.height });
  await text(scaleLayers, row.ship.title, scaleX, baseline + 20, row.column, 18);
  await text(scaleLayers, `${row.ratio}× length`, scaleX, baseline + 49, row.column, 17, '#87cddd');
  scaleX += row.column + gap;
}
await text(scaleLayers, 'Visual scale proposal only · existing in-game sizes and ship artwork remain unchanged', 0, baseline + 93, scaleWidth, 15, '#8fa4be');
await sharp({ create: { width: scaleWidth, height: baseline + 132, channels: 4, background } })
  .composite(scaleLayers).png().toFile(fileURLToPath(new URL('combat-scale-preview.png', root)));

const comparison = [];
const width = 1600, cellW = 320, cellH = 320, header = 92;
await text(comparison, 'EXISTING HULL / NEW CANDIDATE', 0, 22, width, 26);
await text(comparison, 'Matched visible extents for comparison only — no gameplay sizes or silhouettes have been replaced', 0, 61, width, 15, '#8fa4be');
for (const [i, ship] of manifest.ships.entries()) {
  const x = i % 5 * cellW, y = header + Math.floor(i / 5) * cellH;
  await text(comparison, ship.title, x, y + 6, cellW, 19);
  const old = fileURLToPath(new URL(ship.existing, oldRoot));
  const oldBounds = await bounds(old);
  comparison.push({ input: await sharp(old).extract(oldBounds.bbox).resize(144, 220, { fit: 'contain', background: '#00000000' }).png().toBuffer(), left: x + 8, top: y + 44 });
  const measure = measurements.find(row => row.slug === ship.slug);
  comparison.push({ input: await sharp(fileURLToPath(new URL(measure.file, root))).extract(measure.bbox)
    .resize(144, 220, { fit: 'contain', background: '#00000000' }).png().toBuffer(), left: x + 168, top: y + 44 });
  await text(comparison, 'EXISTING', x + 8, y + 278, 144, 12, '#8fa4be');
  await text(comparison, 'NEW', x + 168, y + 278, 144, 12, '#8fa4be');
}
await sharp({ create: { width, height: header + 3 * cellH + 16, channels: 4, background } })
  .composite(comparison).png().toFile(fileURLToPath(new URL('comparison.png', root)));
await writeFile(new URL('measurements.json', root), JSON.stringify({ alphaThreshold: 16, ships: measurements }, null, 2) + '\n');
console.log('Verified 13 native, transparent, unclipped hull masters; saved combat/support overviews and comparison sheet. Existing game assets untouched.');

// Art-review sheets only: resize/composite copies; never mutate source sprites
// or switch the live renderer. Run with Node from any working directory.
import sharp from 'sharp';
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const root = new URL('./', import.meta.url);
const previous = new URL('../../public/art/celestial_sprites/stars/icons/', root);
const manifest = JSON.parse(await readFile(new URL('prompts.json', root), 'utf8'));
const old = JSON.parse(await readFile(new URL('manifest.json', previous), 'utf8'));
assert.equal(manifest.stars.length, 10);
assert.deepEqual(manifest.stars.map(s => s.slug), old.stars.map(s => s.slug));
const background = '#060d19';
const overview = [];
const comparison = [];
const measurements = [];
const escape = s => s.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
async function text(layers, value, x, y, width, size = 17, color = '#d8e6f5') {
  const { data, info } = await sharp({ text: { text: `<span foreground="${color}">${escape(value)}</span>`,
    font: `Arial ${size}`, width, align: 'centre', rgba: true } }).png().toBuffer({ resolveWithObject: true });
  layers.push({ input: data, left: x + Math.floor((width - info.width) / 2), top: y });
}
async function bounds(file) {
  const meta = await sharp(file).metadata();
  assert.ok(meta.hasAlpha, `${file}: transparent PNG required`);
  const { data, info } = await sharp(file).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let left = info.width, top = info.height, right = -1, bottom = -1, clear = 0, maxAlpha = 0;
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    const a = data[(y * info.width + x) * 4 + 3];
    if (!a) clear++;
    maxAlpha = Math.max(maxAlpha, a);
    if (a >= 16) { left = Math.min(left, x); top = Math.min(top, y); right = Math.max(right, x); bottom = Math.max(bottom, y); }
  }
  assert.ok(clear > info.width * info.height * .15 && maxAlpha > 240, `${file}: empty backdrop and solid subject`);
  assert.ok(right > left && bottom > top, `${file}: nonempty art`);
  return { canvas: [info.width, info.height], bbox: { left, top, width: right - left + 1, height: bottom - top + 1 },
    center: [(left + right + 1) / 2, (top + bottom + 1) / 2], visualDiameter: Math.max(right - left + 1, bottom - top + 1) };
}

await text(overview, 'STELLAR SYNDICATES · NEW STAR ARTWORK', 0, 22, 1600, 27);
await text(overview, '10 transparent sprites · large previews and 32 / 48 / 72 px thumbnails · not installed in the live game', 0, 65, 1600, 16, '#8b9db3');
await text(comparison, 'EXISTING ART / NEW CANDIDATES', 0, 24, 1600, 27);
await text(comparison, 'Equal visible extents for artwork comparison — not a change to gameplay size or zoom', 0, 67, 1600, 16, '#8b9db3');
for (const [i, star] of manifest.stars.entries()) {
  const file = fileURLToPath(new URL(star.file, root));
  const m = await bounds(file);
  assert.deepEqual(m.canvas, [1254, 1254]);
  measurements.push({ slug: star.slug, file: star.file, ...m });
  const x = (i % 5) * 320, y = 108 + Math.floor(i / 5) * 370;
  await text(overview, star.title, x, y + 4, 320, 18);
  overview.push({ input: await sharp(file).resize(232, 232).png().toBuffer(), left: x + 44, top: y + 38 });
  for (const [j, size] of [32, 48, 72].entries()) {
    const left = x + 38 + j * 88;
    overview.push({ input: await sharp(file).resize(size, size).png().toBuffer(), left: left + Math.floor((72 - size) / 2), top: y + 285 + Math.floor((72 - size) / 2) });
    await text(overview, String(size), left, y + 358, 72, 12, '#7e93ad');
  }
  const cy = 112 + Math.floor(i / 5) * 270;
  await text(comparison, star.title, x, cy, 320, 18);
  const oldFile = fileURLToPath(new URL(old.stars.find(s => s.slug === star.slug).file, previous));
  for (const [j, path] of [oldFile, file].entries()) {
    const b = await bounds(path);
    const input = await sharp(path).extract(b.bbox).resize(128, 128, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } }).png().toBuffer();
    comparison.push({ input, left: x + 24 + j * 144, top: cy + 45 });
    await text(comparison, j ? 'NEW' : 'EXISTING', x + 24 + j * 144, cy + 197, 128, 13, '#8b9db3');
  }
}
await sharp({ create: { width: 1600, height: 862, channels: 4, background } }).composite(overview)
  .png().toFile(fileURLToPath(new URL('overview.png', root)));
await sharp({ create: { width: 1600, height: 652, channels: 4, background } }).composite(comparison)
  .png().toFile(fileURLToPath(new URL('comparison.png', root)));
await writeFile(new URL('measurements.json', root), JSON.stringify({ alphaThreshold: 16, stars: measurements }, null, 2) + '\n');
console.log('All 10 types present, 1254px RGBA, empty backgrounds and readable alpha bounds; overview/comparison saved. Live art untouched.');

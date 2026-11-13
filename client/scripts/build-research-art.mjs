// §research-panel (Nov 1 2026): research emblems. The six field badges are cut
// from Peter's approved July sheet with explicit hexagon masks (the sheet has a
// painted background, so a rectangle would carry it along). Programme emblems
// come from whichever approved masters exist in art-src/research-emblems-v1
// (prompts in docs/research-art-prompts.md). A missing master is normal while
// the art rolls out; a malformed master or an unknown id stops the build.
// §research-sheets (Nov 3 2026): programme emblems also arrive as ChatGPT
// sheets (art-src/research-emblems-v1/sheets/<name>.png, up to six badges on a
// transparent canvas, keys in sheets/sheets.json in reading order; prompts in
// docs/research-emblem-sheets.md). Each badge is found by the clear gaps and
// framed like a single master. A single `<key>.png` wins over its sheet cell.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const version = "v1";
// Every emblem, field or programme, shares one framing: the hexagon centred,
// 88% of the square's height (the prompts' 6% top and bottom margins).
export const HEX_FILL = 0.88;
export const FIELD_SIZES = [64, 128, 256];
export const PROGRAMME_SIZES = [64, 128, 256, 512];
const recipe = "research-hex-88-v1";
const sheetPath = path.join(client, "art-src/research-topics-icons/research-topics-icons.png");
const mastersDir = path.join(client, "art-src/research-emblems-v1");
const outputRoot = path.join(client, "public/art/derived/research", version);
const generatedPath = path.join(client, "src/research-art.generated.ts");
const transparent = { r: 0, g: 0, b: 0, alpha: 0 };

// Each badge's outer rim on the 1536 × 1024 sheet, measured edge by edge (top
// point, then clockwise). The hexagons are slightly wider than regular ones.
export const FIELD_HEX = {
  propulsion: [[368.5, 129.4], [522.0, 213.2], [522.0, 381.3], [369.0, 468.1], [213.0, 381.1], [213.1, 212.9]],
  materials: [[767.8, 129.3], [923.0, 211.6], [923.0, 381.8], [768.2, 467.9], [613.0, 381.2], [613.0, 212.1]],
  computation: [[1167.3, 129.3], [1322.0, 212.2], [1322.0, 381.4], [1168.0, 468.2], [1013.0, 381.5], [1013.0, 211.7]],
  weapons: [[367.9, 517.9], [521.0, 600.0], [521.0, 766.2], [368.8, 853.2], [213.1, 765.7], [213.0, 600.3]],
  hulls: [[768.0, 518.3], [923.0, 600.6], [923.0, 766.9], [768.4, 854.3], [612.1, 766.9], [612.0, 600.8]],
  life: [[1167.9, 518.4], [1323.1, 601.5], [1322.9, 766.6], [1168.7, 853.3], [1013.0, 766.1], [1013.0, 600.9]],
};
// A thin bright line runs just outside each rim; the mask stops this far inside
// the measured edge so neither it nor the grey backdrop survives.
const FIELD_INSET = 1.25;

const hash = bytes => createHash("sha256").update(bytes).digest("hex");

/** Move every edge of a clockwise (y-down) polygon `d` px inward. */
export function insetPolygon(points, d) {
  const lines = points.map((a, i) => {
    const b = points[(i + 1) % points.length];
    const length = Math.hypot(b[0] - a[0], b[1] - a[1]);
    const nx = -(b[1] - a[1]) / length, ny = (b[0] - a[0]) / length;
    return { x: a[0] + nx * d, y: a[1] + ny * d, dx: b[0] - a[0], dy: b[1] - a[1] };
  });
  return lines.map((line, i) => {
    const prev = lines[(i + lines.length - 1) % lines.length];
    const det = prev.dx * line.dy - prev.dy * line.dx;
    const t = ((line.x - prev.x) * line.dy - (line.y - prev.y) * line.dx) / det;
    return [prev.x + prev.dx * t, prev.y + prev.dy * t];
  });
}

/** One field badge: a square crop centred on the hexagon, sized so the masked
 * hexagon is HEX_FILL of its height, with the polygon as its alpha. */
async function cutFieldBadge(sheet, field) {
  const hex = insetPolygon(FIELD_HEX[field], FIELD_INSET);
  const xs = hex.map(p => p[0]), ys = hex.map(p => p[1]);
  const height = Math.max(...ys) - Math.min(...ys);
  const side = Math.round(height / HEX_FILL);
  const left = Math.round((Math.min(...xs) + Math.max(...xs)) / 2 - side / 2);
  const top = Math.round((Math.min(...ys) + Math.max(...ys)) / 2 - side / 2);
  const points = hex.map(([x, y]) => `${(x - left).toFixed(2)},${(y - top).toFixed(2)}`).join(" ");
  const mask = await sharp(Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="${side}" height="${side}"><polygon points="${points}" fill="#fff"/></svg>`))
    .ensureAlpha().extractChannel(3).raw().toBuffer();
  const rgb = await sharp(sheet).extract({ left, top, width: side, height: side }).removeAlpha().raw().toBuffer();
  return sharp(rgb, { raw: { width: side, height: side, channels: 3 } })
    .joinChannel(mask, { raw: { width: side, height: side, channels: 1 } }).png().toBuffer();
}

function alphaBounds(data, width, height, floor = 128) {
  let left = width, top = height, right = -1, bottom = -1;
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    if (data[(y * width + x) * 4 + 3] < floor) continue;
    if (x < left) left = x;
    if (x > right) right = x;
    if (y < top) top = y;
    if (y > bottom) bottom = y;
  }
  return right < left ? null : { left, top, width: right - left + 1, height: bottom - top + 1 };
}

/** A delivered master must be a real cutout of one centred badge: real alpha,
 * clear corners, and a hexagon's proportions. Anything else is an error, not a
 * silent fallback, so a bad delivery is caught before it reaches the panel. */
export function checkProgrammeMaster(id, data, info) {
  assert.equal(info.width, info.height, `${id}: square master`);
  assert.ok(info.width >= 1024, `${id}: native master (1254 px), not a preview`);
  let clear = 0;
  for (let i = 3; i < data.length; i += 4) if (data[i] === 0) clear++;
  const share = clear / (info.width * info.height);
  assert.ok(share > 0.2, `${id}: real transparency outside the badge (${(share * 100).toFixed(1)}% clear)`);
  const corner = Math.round(info.width * 0.02);
  for (const [cx, cy] of [[0, 0], [info.width - corner, 0], [0, info.height - corner], [info.width - corner, info.height - corner]]) {
    for (let y = cy; y < cy + corner; y++) for (let x = cx; x < cx + corner; x++) {
      assert.ok(data[(y * info.width + x) * 4 + 3] <= 8, `${id}: transparent corners (no backdrop)`);
    }
  }
  const box = alphaBounds(data, info.width, info.height);
  assert.ok(box, `${id}: a visible badge`);
  const ratio = box.width / box.height;
  assert.ok(ratio > 0.7 && ratio < 1.1, `${id}: one hexagon badge (width/height ${ratio.toFixed(2)})`);
  assert.ok(box.height > info.height * 0.5, `${id}: the badge fills the canvas (${box.height} of ${info.height} px)`);
  return box;
}

// A sheet badge is about 450 px tall (88% of a 512 px cell); below this the
// 512 px level would be a visible upscale.
export const SHEET_MIN_BADGE = 360;

/** Opaque regions of a sheet in reading order: bands of rows holding any opaque
 * pixel, then runs of columns inside each band, each tightened to its alpha.
 * Gaps narrower than `gap` px are bridged, so a stray speck never splits a badge. */
export function findSheetBadges(data, info, gap = 6) {
  const { width, height } = info;
  const opaque = (x, y) => data[(y * width + x) * 4 + 3] >= 128;
  const runs = (length, filled) => {
    const out = [];
    let start = -1, last = -1;
    for (let i = 0; i < length; i++) {
      if (!filled(i)) continue;
      if (start >= 0 && i - last > gap) { out.push([start, last]); start = -1; }
      if (start < 0) start = i;
      last = i;
    }
    if (start >= 0) out.push([start, last]);
    return out;
  };
  const any = (a0, a1, b0, b1, at) => { for (let a = a0; a <= a1; a++) for (let b = b0; b <= b1; b++) if (at(a, b)) return true; return false; };
  const boxes = [];
  for (const [y0, y1] of runs(height, y => any(0, width - 1, y, y, opaque))) {
    for (const [x0, x1] of runs(width, x => any(y0, y1, x, x, (y, xx) => opaque(xx, y)))) {
      let top = y1, bottom = y0;
      for (let y = y0; y <= y1; y++) if (any(x0, x1, y, y, opaque)) { top = Math.min(top, y); bottom = Math.max(bottom, y); }
      boxes.push({ left: x0, top, width: x1 - x0 + 1, height: bottom - top + 1 });
    }
  }
  return boxes;
}

/** A delivered sheet must hold exactly its planned badges, apart on real alpha,
 * each a hexagon large enough for the 512 px level. Returns their boxes in
 * reading order. */
export function checkSheet(name, data, info, count) {
  let clear = 0;
  for (let i = 3; i < data.length; i += 4) if (data[i] === 0) clear++;
  const share = clear / (info.width * info.height);
  assert.ok(share > 0.2, `${name}: real transparency between the badges (${(share * 100).toFixed(1)}% clear); ask for the sheet on a transparent background`);
  const boxes = findSheetBadges(data, info);
  assert.equal(boxes.length, count, `${name}: ${boxes.length} badge${boxes.length === 1 ? "" : "s"} found, ${count} expected (touching badges merge; a backdrop, shadow or caption counts as one)`);
  boxes.forEach((box, i) => {
    const ratio = box.width / box.height;
    assert.ok(ratio > 0.7 && ratio < 1.1, `${name} badge ${i + 1}: one hexagon (width/height ${ratio.toFixed(2)})`);
    assert.ok(box.height >= SHEET_MIN_BADGE, `${name} badge ${i + 1}: ${box.height} px tall, ${SHEET_MIN_BADGE} needed for the 512 px level`);
  });
  return boxes;
}

/** Trim to the badge and re-centre it at HEX_FILL, so an image model's loose
 * margins never make one emblem larger than its neighbours. */
async function frameProgramme(data, info, box, size) {
  const pad = 3;
  const left = Math.max(0, box.left - pad), top = Math.max(0, box.top - pad);
  const width = Math.min(info.width, box.left + box.width + pad) - left;
  const height = Math.min(info.height, box.top + box.height + pad) - top;
  const scale = Math.min(size * HEX_FILL / box.height, size * 0.96 / box.width);
  const w = Math.max(1, Math.round(width * scale)), h = Math.max(1, Math.round(height * scale));
  const badge = await sharp(data, { raw: info }).extract({ left, top, width, height })
    .resize(w, h, { kernel: sharp.kernel.lanczos3 }).png().toBuffer();
  return sharp({ create: { width: size, height: size, channels: 4, background: transparent } })
    .composite([{ input: badge, left: Math.round((size - w) / 2), top: Math.round((size - h) / 2) }]).png().toBuffer();
}

async function writeWebp(png, file, root) {
  const output = path.join(root, file);
  const bytes = await sharp(png).webp({ quality: 90, alphaQuality: 100, effort: 4 }).toBuffer();
  const old = await readFile(output).catch(() => null);
  if (!old || !old.equals(bytes)) await writeFile(output, bytes);
  return { url: `/art/derived/research/${version}/${file}?v=${hash(bytes).slice(0, 12)}`, bytes: bytes.length, written: old?.equals(bytes) ? 0 : 1 };
}

/** Reuse last build's outputs when the source, the recipe and the files on disk
 * all still match; otherwise derive again. */
async function cachedLevels(previous, key, sourceSha256, sizes, root) {
  const old = previous?.recipe === recipe ? previous.art?.[key] : null;
  if (!old || old.sourceSha256 !== sourceSha256) return null;
  for (const size of sizes) {
    const level = old.levels?.[size];
    if (!level) return null;
    const file = path.join(root, level.url.replace(/^\/art\/derived\/research\/[^/]+\//, "").replace(/\?.*$/, ""));
    if (!await stat(file).then(s => s.size === level.bytes).catch(() => false)) return null;
  }
  return old.levels;
}

export async function knownProgrammes(dir = mastersDir) {
  const prompts = JSON.parse(await readFile(path.join(dir, "prompts.json"), "utf8"));
  return new Map(prompts.assets.map(asset => [asset.key, asset.field.toLowerCase()]));
}

/** The sheets on disk with their planned keys, by name. */
export async function landedSheets(dir = mastersDir) {
  const folder = path.join(dir, "sheets");
  const planText = await readFile(path.join(folder, "sheets.json"), "utf8").catch(() => null);
  if (!planText) return [];
  const plan = JSON.parse(planText);
  const files = (await readdir(folder)).filter(name => /\.(png|webp)$/i.test(name)).sort();
  return files.map(file => {
    const name = file.replace(/\.(png|webp)$/i, "");
    assert.ok(plan[name], `sheets/${file}: no sheet "${name}" in sheets.json`);
    return { name, file, keys: plan[name] };
  });
}

/** Every programme with art on disk: single masters and the cells of landed sheets. */
export async function deliveredProgrammes(dir = mastersDir) {
  const singles = (await readdir(dir)).filter(name => name.endsWith(".png")).map(name => name.slice(0, -4));
  const sheets = await landedSheets(dir);
  return [...new Set([...singles, ...sheets.flatMap(sheet => sheet.keys)])].sort();
}

/** `masters`, `output` and `generated` default to the game's folders; the
 * research art test points them at a scratch directory. */
export async function buildResearchArt({ masters = mastersDir, output = outputRoot, generated: generatedFile = generatedPath } = {}) {
  await mkdir(path.join(output, "fields"), { recursive: true });
  await mkdir(path.join(output, "programmes"), { recursive: true });
  const manifestPath = path.join(output, "manifest.json");
  const previousText = await readFile(manifestPath, "utf8").catch(() => "");
  const previous = previousText ? JSON.parse(previousText) : null;
  const manifest = { version, recipe, hexFill: HEX_FILL, art: {} };
  const fields = {}, programmes = {};
  let written = 0;

  const sheet = await readFile(sheetPath);
  const sheetSha256 = hash(Buffer.concat([sheet, Buffer.from(JSON.stringify([FIELD_HEX, FIELD_INSET]))]));
  for (const field of Object.keys(FIELD_HEX)) {
    const key = `field:${field}`;
    let levels = await cachedLevels(previous, key, sheetSha256, FIELD_SIZES, output);
    if (!levels) {
      const badge = await cutFieldBadge(sheet, field);
      levels = {};
      for (const size of FIELD_SIZES) {
        const png = await sharp(badge).resize(size, size, { kernel: sharp.kernel.lanczos3 }).png().toBuffer();
        const out = await writeWebp(png, `fields/${field}-${size}.webp`, output);
        levels[size] = { url: out.url, bytes: out.bytes };
        written += out.written;
      }
    }
    manifest.art[key] = { source: path.relative(client, sheetPath), sourceSha256: sheetSha256, levels };
    fields[field] = Object.fromEntries(FIELD_SIZES.map(size => [size, levels[size].url]));
  }

  const known = await knownProgrammes(masters);
  const delivered = (await readdir(masters, { withFileTypes: true }))
    .filter(entry => entry.isFile() && entry.name.toLowerCase().endsWith(".png")).map(entry => entry.name).sort();
  for (const name of delivered) {
    const id = name.slice(0, -4);
    assert.ok(known.has(id), `research-emblems-v1/${name}: no programme has the id "${id}"`);
    const bytes = await readFile(path.join(masters, name));
    const sourceSha256 = hash(bytes);
    const key = `programme:${id}`;
    let levels = await cachedLevels(previous, key, sourceSha256, PROGRAMME_SIZES, output);
    if (!levels) {
      const meta = await sharp(bytes).metadata();
      assert.ok(meta.hasAlpha, `${id}: transparent PNG master`);
      const { data, info } = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      const box = checkProgrammeMaster(id, data, info);
      levels = {};
      for (const size of PROGRAMME_SIZES) {
        const out = await writeWebp(await frameProgramme(data, info, box, size), `programmes/${id}-${size}.webp`, output);
        levels[size] = { url: out.url, bytes: out.bytes };
        written += out.written;
      }
    }
    manifest.art[key] = { source: path.relative(client, path.join(masters, name)), sourceSha256, levels };
    programmes[id] = Object.fromEntries(PROGRAMME_SIZES.map(size => [size, levels[size].url]));
  }

  const sheets = await landedSheets(masters);
  let cells = 0;
  for (const { name, file, keys } of sheets) {
    for (const id of keys) assert.ok(known.has(id), `sheets/${file}: no programme has the id "${id}"`);
    const sheetFile = path.join(masters, "sheets", file);
    const bytes = await readFile(sheetFile);
    const sheetSha256 = hash(bytes);
    // The cache key carries the cell and the plan's size, so an edited
    // sheets.json slices (and checks) the sheet again.
    const cellKey = index => `${sheetSha256}#${index + 1}/${keys.length}`;
    const pending = keys.map((id, index) => ({ id, index })).filter(({ id }) => !programmes[id]);
    const cached = new Map();
    for (const { id, index } of pending) {
      const levels = await cachedLevels(previous, `programme:${id}`, cellKey(index), PROGRAMME_SIZES, output);
      if (levels) cached.set(id, levels);
    }
    let sliced = null;
    if (cached.size < pending.length) {
      const meta = await sharp(bytes).metadata();
      assert.ok(meta.hasAlpha, `${name}: transparent sheet (real alpha)`);
      const { data, info } = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      sliced = { data, info, boxes: checkSheet(name, data, info, keys.length) };
    }
    for (const { id, index } of pending) {
      let levels = cached.get(id);
      if (!levels) {
        levels = {};
        for (const size of PROGRAMME_SIZES) {
          const out = await writeWebp(await frameProgramme(sliced.data, sliced.info, sliced.boxes[index], size), `programmes/${id}-${size}.webp`, output);
          levels[size] = { url: out.url, bytes: out.bytes };
          written += out.written;
        }
      }
      manifest.art[`programme:${id}`] = { source: `${path.relative(client, sheetFile)}#${index + 1}`, sourceSha256: cellKey(index), levels };
      programmes[id] = Object.fromEntries(PROGRAMME_SIZES.map(size => [size, levels[size].url]));
      cells++;
    }
  }

  const text = `${JSON.stringify(manifest, null, 2)}\n`;
  if (text !== previousText) { await writeFile(manifestPath, text); written++; }
  const generated = "// Generated by scripts/build-research-art.mjs. Masters stay in art-src.\n" +
    "// Programme emblems appear here only once their approved master exists.\n" +
    `export const RESEARCH_FIELD_ART: Readonly<Record<string, Readonly<Record<"64" | "128" | "256", string>>>> = ${JSON.stringify(fields, null, 2)};\n` +
    `export const RESEARCH_PROGRAMME_ART: Readonly<Record<string, Readonly<Record<"64" | "128" | "256" | "512", string>>>> = ${JSON.stringify(programmes, null, 2)};\n`;
  const oldGenerated = await readFile(generatedFile, "utf8").catch(() => "");
  if (generated !== oldGenerated) { await writeFile(generatedFile, generated); written++; }
  return { fields: Object.keys(fields).length, programmes: delivered.length + cells, sheets: sheets.length, known: known.size, written };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log("Research art:", await buildResearchArt());
}

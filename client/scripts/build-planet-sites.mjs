// Chooses the 24 structure footprint addresses for every planet-surface painting.
// Buildings must stand on open land, never on forest, water, lava, cliffs, pits
// or rock. Each painting is classified from colour and local texture; candidate
// footprints are scored by how much of their base ellipse is open ground; and a
// colony grows outward from the most open spot in the depth band where the
// ground's apparent pitch matches the sprites' 35° camera. Spacing is as generous
// as the painting's open land allows.
//
// Run after replacing or adding a painting, review the overlays, then commit
// src/planet-sites.generated.ts:
//   npm run assets:sites -- --review <dir>
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const artRoot = path.join(client, "art-src", "planet-view");
const output = path.join(client, "src", "planet-sites.generated.ts");

/** Keys are planetSurface() values. Overrides tune the classifier per painting. */
export const PAINTINGS = {
  // Vegetated worlds: open land is grassland, so bare rock, cliffs and beaches
  // count as rough even where they are pale and smooth.
  "highland-plateau": { file: "habitable-v1/highland-plateau.png", vegetated: true },
  "savanna-world": { file: "habitable-v1/savanna-world.png", vegetated: true },
  "volcanic-highlands": { file: "habitable-v1/volcanic-highlands.png", vegetated: true },
  "boreal-world": { file: "habitable-v1/boreal-world.png", vegetated: true },
  "archipelago-world": { file: "habitable-v1/archipelago-world.png", vegetated: true },
  "temperate-basin": { file: "habitable-v1/temperate-basin.png", vegetated: true },
  terrestrial: { file: "surfaces/terrestrial-surface-v1.png", vegetated: true },
  desert: { file: "surfaces/desert-surface-v1.png" },
  ocean: { file: "surfaces/ocean-surface-v1.png" },
  ice: { file: "surfaces/ice-surface-v1.png" },
  // The open ground between the lava channels is dark basalt, so only very dark
  // rock counts as rough there.
  lava: { file: "surfaces/lava-surface-v1.png", darkV: .13 },
  barren: { file: "surfaces/barren-surface-v1.png" },
  marginal: { file: "surfaces/marginal-surface-v1.png" },
};

// ---- Geometry shared with planet.ts / planet-panel.css ----------------------
/** A scale-1 sprite is 7.5% of the plane's width (--planet-building: 7.5cqw). */
export const SPRITE = 0.075;
/** Mirrors siteScale() in planet.ts: equal ground looks smaller toward the limb. */
export const siteScale = yPct => Math.min(1, Math.max(0.6, 0.64 + (yPct / 100 - 0.5) * 1.15));
/** The clickable box as a share of the sprite (.deck-planet__building in CSS). */
export const HIT = { w: 0.8, h: 0.72 };
/** Slots a new colony occupies: catalogue order for the mine (0), farms (2, 7) and
 * quarters (18); the storehouse and local command take the lowest free (1, 3). */
export const OPENING = [0, 1, 2, 3, 7, 18];
const LATER = Array.from({ length: 24 }, (_, i) => i).filter(i => !OPENING.includes(i));
/** The painting's limb haze, in percent of height (see test-planet-panel.mjs). */
export const limbPct = xPct => 19 + 13 * ((xPct - 50) / 50) ** 2;

// ---- Terrain classification -------------------------------------------------
const W = 836, H = 470; // analysis grid, half the 1672 × 941 masters
const OPEN = 0, WATER = 1, LAVA = 2, DARK = 3, ROUGH = 4, SKY = 5;
const DEFAULTS = { darkV: .27, darkFrac: .22, waterFrac: .3, lavaFrac: .08, grassFrac: .7, tex: 25, radius: 6, blur: 1.5, vegetated: false };

function boxMean(src, r) {
  const I = new Float64Array((W + 1) * (H + 1));
  for (let y = 0; y < H; y++) {
    let row = 0;
    for (let x = 0; x < W; x++) { row += src[y * W + x]; I[(y + 1) * (W + 1) + x + 1] = I[y * (W + 1) + x + 1] + row; }
  }
  const out = new Float32Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const x0 = Math.max(0, x - r), x1 = Math.min(W, x + r + 1), y0 = Math.max(0, y - r), y1 = Math.min(H, y + r + 1);
    out[y * W + x] = (I[y1 * (W + 1) + x1] - I[y0 * (W + 1) + x1] - I[y1 * (W + 1) + x0] + I[y0 * (W + 1) + x0]) / ((x1 - x0) * (y1 - y0));
  }
  return out;
}

/** Per-pixel class. Water is strongly saturated blue, lava a saturated glow, dark
 * areas are forest or rock shadow, and rough ground (cliffs, pits, ridges) has
 * strong texture once fine grass detail is blurred away; on vegetated worlds any
 * ground that is not mostly grass-coloured is rough too. The rest is open land. */
async function classify(file, overrides) {
  const o = { ...DEFAULTS, ...overrides };
  const raw = await sharp(file).resize(W, H).removeAlpha().raw().toBuffer();
  const soft = await sharp(file).resize(W, H).removeAlpha().blur(o.blur).raw().toBuffer();
  const N = W * H, L = new Float32Array(N), L2 = new Float32Array(N);
  const water = new Float32Array(N), lava = new Float32Array(N), dark = new Float32Array(N), grass = new Float32Array(N);
  for (let i = 0; i < N; i++) {
    const r = raw[3 * i], g = raw[3 * i + 1], b = raw[3 * i + 2];
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn, v = mx / 255, s = mx ? d / mx : 0;
    let h = 0;
    if (d) h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
    h = (h * 60 + 360) % 360;
    water[i] = h >= 170 && h <= 260 && s >= .32 && (v >= .3 || s >= .6) ? 1 : 0;
    lava[i] = (h <= 45 || h >= 345) && s >= .75 && v >= .65 ? 1 : 0;
    dark[i] = v < o.darkV ? 1 : 0;
    grass[i] = h >= 38 && h <= 110 && s >= .3 && v >= .2 ? 1 : 0;
    const l = 0.299 * soft[3 * i] + 0.587 * soft[3 * i + 1] + 0.114 * soft[3 * i + 2];
    L[i] = l; L2[i] = l * l;
  }
  const m1 = boxMean(L, o.radius), m2 = boxMean(L2, o.radius);
  const wf = boxMean(water, o.radius), lf = boxMean(lava, o.radius), df = boxMean(dark, o.radius), gf = boxMean(grass, o.radius);
  const cls = new Uint8Array(N);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, tex = Math.sqrt(Math.max(0, m2[i] - m1[i] * m1[i]));
    cls[i] = 100 * (y + .5) / H < limbPct(100 * (x + .5) / W) + 4 ? SKY
      : wf[i] >= o.waterFrac ? WATER : lf[i] >= o.lavaFrac ? LAVA : df[i] >= o.darkFrac ? DARK
      : tex >= o.tex || (o.vegetated && gf[i] < o.grassFrac) ? ROUGH : OPEN;
  }
  return cls;
}

// ---- Placement --------------------------------------------------------------
/** Share of a footprint's base ellipse (the contact-shadow ellipse in the CSS,
 * scaled by k) that is open land. Coordinates are analysis-grid pixels. */
function openShare(cls, cx, cy, fp, k) {
  const ex = cx + .03 * fp, ey = cy + .17 * fp, a = .46 * fp * k, b = .23 * fp * k;
  let n = 0, open = 0;
  for (let y = Math.floor(ey - b); y <= Math.ceil(ey + b); y++) for (let x = Math.floor(ex - a); x <= Math.ceil(ex + a); x++) {
    if (((x - ex) / a) ** 2 + ((y - ey) / b) ** 2 > 1) continue;
    n++;
    if (x >= 0 && y >= 0 && x < W && y < H && cls[y * W + x] === OPEN) open++;
  }
  return n ? open / n : 0;
}

/** Share of water or lava inside the building's whole box: a building whose base
 * is on a shoreline still reads as standing in the lake when water shows behind it. */
function hazardShare(cls, cx, cy, fp) {
  let n = 0, wet = 0;
  for (let y = Math.floor(cy - HIT.h * fp / 2); y <= Math.ceil(cy + .4 * fp); y++) for (let x = Math.floor(cx - HIT.w * fp / 2); x <= Math.ceil(cx + HIT.w * fp / 2); x++) {
    n++;
    const c = x >= 0 && y >= 0 && x < W && y < H ? cls[y * W + x] : SKY;
    if (c === WATER || c === LAVA || c === SKY) wet++;
  }
  return n ? wet / n : 1;
}

/** Quality tiers: 0–1 are clean open land; 2+ are fallbacks for paintings with
 * too little room, kept off real overlap and reported as rough. */
const TIERS = [
  { core: .97, margin: .85, hazard: .02 }, { core: .93, margin: .7, hazard: .02 },
  { core: .85, margin: 0, hazard: .05 }, { core: .6, margin: 0, hazard: .15 }, { core: 0, margin: 0, hazard: 1 },
];
const GAPS = [.6, .45, .32, .2, .12, .06];
const MIN_GAP = GAPS[GAPS.length - 1];

function placeColony(cls) {
  const cands = [];
  for (let y = Math.round(H * .47); y <= Math.round(H * .92); y += 2) for (let x = 4; x < W - 4; x += 2) {
    const yPct = 100 * y / H, s = siteScale(yPct), fp = SPRITE * W * s;
    if (x - fp / 2 < W * .01 || x + fp / 2 > W * .99 || y + fp / 2 > H * .99) continue;
    if (100 * (y - fp / 2) / H < limbPct(100 * x / W) + 3) continue;
    cands.push({ x, y, fp, core: openShare(cls, x, y, fp, 1), margin: openShare(cls, x, y, fp, 1.35), hazard: hazardShare(cls, x, y, fp) });
  }
  // Seed: the roomiest clean spot where the ground's pitch matches the sprites'.
  const clean = cands.filter(c => c.core >= TIERS[0].core && c.margin >= TIERS[0].margin && c.hazard <= TIERS[0].hazard);
  const band = clean.filter(c => c.y / H >= .58 && c.y / H <= .76);
  let seed = null, best = -1;
  for (const c of band.length ? band : clean) {
    const room = openShare(cls, c.x, c.y, c.fp * 3, 1);
    if (room > best) { best = room; seed = c; }
  }
  // A fixed hash of the position, so growth is organic but reproducible.
  const jitter = c => { const h = Math.sin(c.x * 12.9898 + c.y * 78.233) * 43758.5453; return h - Math.floor(h); };
  const clash = (a, b, gap) => {
    const g = gap * (a.fp + b.fp) / 2;
    return Math.abs(a.x - b.x) < HIT.w * (a.fp + b.fp) / 2 + g && Math.abs(a.y - b.y) < HIT.h * (a.fp + b.fp) / 2 + g;
  };
  const grow = gap => {
    const chosen = seed ? [{ ...seed, tier: 0 }] : [];
    for (const [tier, req] of TIERS.entries()) {
      while (chosen.length < 24) {
        const mx = chosen.reduce((s, c) => s + c.x, 0) / (chosen.length || 1), my = chosen.reduce((s, c) => s + c.y, 0) / (chosen.length || 1);
        let pick = null, cost = Infinity;
        for (const c of cands) {
          if (c.core < req.core || c.margin < req.margin || c.hazard > req.hazard) continue;
          if (chosen.some(o => clash(o, c, tier <= 1 ? gap : MIN_GAP))) continue;
          // Ground distance: screen height is foreshortened by roughly 1.6.
          const d = Math.hypot(c.x - mx, 1.6 * (c.y - my)) * (1 + .45 * jitter(c)) - 20 * c.margin;
          if (d < cost) { cost = d; pick = c; }
        }
        if (!pick) break;
        chosen.push({ ...pick, tier });
      }
    }
    return chosen;
  };
  let chosen = null, spacing = 0;
  for (const gap of GAPS) {
    const attempt = grow(gap), count = attempt.filter(c => c.tier <= 1).length;
    if (!chosen || count > chosen.filter(c => c.tier <= 1).length) { chosen = attempt; spacing = gap; }
    if (count >= 24) break;
  }
  assert.equal(chosen.length, 24, "every painting offers 24 non-overlapping addresses");
  // Growth order to slots: the six most central spots serve the opening colony.
  const sites = new Array(24);
  chosen.forEach((c, i) => { sites[i < 6 ? OPENING[i] : LATER[i - 6]] = c; });
  return { sites, spacing };
}

async function review(file, cls, sites, out) {
  const base = await sharp(file).resize(W * 2, H * 2).removeAlpha().raw().toBuffer();
  const tint = { [WATER]: [40, 110, 255], [LAVA]: [255, 140, 0], [DARK]: [120, 0, 160], [ROUGH]: [230, 30, 60], [SKY]: [60, 60, 60] };
  for (let y = 0; y < H * 2; y++) for (let x = 0; x < W * 2; x++) {
    const c = cls[(y >> 1) * W + (x >> 1)];
    if (c === OPEN) continue;
    const k = 3 * (y * W * 2 + x), a = c === SKY ? .2 : .5;
    for (let j = 0; j < 3; j++) base[k + j] = Math.round(base[k + j] * (1 - a) + tint[c][j] * a);
  }
  let svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W * 2}" height="${H * 2}">`;
  sites.forEach((c, slot) => {
    const fp = 2 * c.fp, x = 2 * c.x, y = 2 * c.y, stroke = c.tier <= 1 ? "#fff" : "#ff4040";
    svg += `<rect x="${x - HIT.w * fp / 2}" y="${y - HIT.h * fp / 2}" width="${HIT.w * fp}" height="${HIT.h * fp}" fill="none" stroke="${stroke}" stroke-width="2"/>`;
    svg += `<ellipse cx="${x + .03 * fp}" cy="${y + .17 * fp}" rx="${.46 * fp}" ry="${.23 * fp}" fill="none" stroke="#00ffd0" stroke-width="1.5"/>`;
    svg += `<text x="${x}" y="${y}" fill="#fff" font-size="18" font-family="Arial" font-weight="bold" text-anchor="middle" stroke="#000" stroke-width="3" paint-order="stroke">${slot}</text>`;
  });
  await sharp(base, { raw: { width: W * 2, height: H * 2, channels: 3 } })
    .composite([{ input: Buffer.from(`${svg}</svg>`), left: 0, top: 0 }]).jpeg({ quality: 82 }).toFile(out);
}

export async function buildPlanetSites({ reviewDir } = {}) {
  if (reviewDir) await mkdir(reviewDir, { recursive: true });
  const table = {}, report = [];
  for (const [surface, { file, ...overrides }] of Object.entries(PAINTINGS)) {
    const source = path.join(artRoot, file), bytes = await readFile(source);
    const meta = await sharp(bytes).metadata();
    assert.ok(Math.abs(meta.width / meta.height - 16 / 9) < .01, `${file}: 16:9 master`);
    const cls = await classify(source, overrides);
    const { sites, spacing } = placeColony(cls);
    const rough = sites.map((c, slot) => c.tier <= 1 ? -1 : slot).filter(slot => slot >= 0);
    table[surface] = {
      source: createHash("sha256").update(bytes).digest("hex"),
      rough,
      sites: sites.map(c => [+(100 * c.x / W).toFixed(1), +(100 * c.y / H).toFixed(1)]),
    };
    report.push(`${surface}: spacing ${spacing}, ${24 - rough.length}/24 on clean open land`);
    if (reviewDir) await review(source, cls, sites, path.join(reviewDir, `${surface}.jpg`));
  }
  const body = Object.entries(table).map(([surface, t]) =>
    `  ${JSON.stringify(surface)}: {\n    source: ${JSON.stringify(t.source)},\n    rough: ${JSON.stringify(t.rough)},\n    sites: [${t.sites.map(([x, y]) => `[${x}, ${y}]`).join(", ")}],\n  },`).join("\n");
  const text = `// Generated by scripts/build-planet-sites.mjs from the painting masters. Do not edit.
// Each painting lists 24 footprint anchors [x%, y%] indexed by site slot, placed on
// open land. Slots 0, 1, 2, 3, 7 and 18 hold the opening colony. \`rough\` lists the
// slots whose base could not be kept on clean open land because the painting has
// too little room; \`source\` is the master's sha256.
export interface PlanetSiteTable { source: string; rough: readonly number[]; sites: readonly (readonly [number, number])[] }
export const PLANET_SITES: Readonly<Record<string, PlanetSiteTable>> = {
${body}
};
`;
  const previous = await readFile(output, "utf8").catch(() => "");
  if (text !== previous) await writeFile(output, text);
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const i = process.argv.indexOf("--review");
  const report = await buildPlanetSites({ reviewDir: i > 0 ? path.resolve(process.argv[i + 1]) : undefined });
  console.log(report.join("\n"));
}

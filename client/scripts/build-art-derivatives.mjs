import { mkdir, readdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";
import { buildStarDerivatives } from "./build-star-derivatives.mjs";
import { buildShipDerivatives } from "./build-ship-derivatives.mjs";
import { buildHubModelAssets } from "./build-hub-model-assets.mjs";
import { buildMapGlyphs } from "./build-map-glyphs.mjs";
import { buildPlanetDerivatives } from "./build-planet-derivatives.mjs";
import { buildPlanetWorkbenchArt } from "./build-planet-workbench-art.mjs";
import { buildStructureTierArt } from "./build-structure-tier-art.mjs";
import { buildStationArt } from "./build-station-art.mjs";
import { buildCommandCenterArt } from "./build-command-center-art.mjs";
import { buildPlanetShipyardArt } from "./build-planet-shipyard-art.mjs";
import { buildPlanetSurfaceStructureArt } from "./build-planet-surface-structure-art.mjs";
import { buildFoundingStoryArt, buildPirateCrewArt, buildStoryEventArt, buildTraderPortraitArt } from "./build-founding-story-art.mjs";
import { buildAsteroidQuarryArt } from "./build-asteroid-quarry-art.mjs";
import { buildSignalArt } from "./build-signal-art.mjs";
import { buildShipyardInteriorArt } from "./build-shipyard-interior-art.mjs";
import { buildSeasonArt } from "./build-season-art.mjs";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));
const publicArt = path.join(clientRoot, "public", "art");
const sourceArt = path.join(clientRoot, "art-src");

async function stale(source, output) {
  try {
    const [sourceStat, outputStat] = await Promise.all([stat(source), stat(output)]);
    return outputStat.mtimeMs < sourceStat.mtimeMs;
  } catch {
    return true;
  }
}

async function derive(source, output, transform) {
  if (!(await stale(source, output))) return false;
  await transform(sharp(source)).toFile(output);
  return true;
}

async function buildCaptains() {
  const sourceDir = path.join(publicArt, "captains");
  const outputDir = path.join(publicArt, "derived", "captains");
  await mkdir(outputDir, { recursive: true });
  const sources = (await readdir(sourceDir)).filter((name) => name.endsWith(".png")).sort();
  let written = 0;
  for (const name of sources) {
    const source = path.join(sourceDir, name);
    const stem = path.basename(name, ".png");
    for (const size of [96, 192]) {
      const resized = (image) => image.resize(size, size, { fit: "cover" });
      written += Number(await derive(
        source,
        path.join(outputDir, `${stem}-${size}.webp`),
        (image) => resized(image).webp({ quality: size === 96 ? 78 : 82, effort: 5 }),
      ));
      written += Number(await derive(
        source,
        path.join(outputDir, `${stem}-${size}.avif`),
        (image) => resized(image).avif({ quality: size === 96 ? 48 : 52, effort: 5 }),
      ));
    }
  }
  return { sources: sources.length, written };
}

async function buildLore() {
  const sourceDir = path.join(publicArt, "lore_illustrations");
  const outputDir = path.join(publicArt, "derived", "lore");
  await mkdir(outputDir, { recursive: true });
  const sources = (await readdir(sourceDir)).filter((name) => name.endsWith(".png")).sort();
  let written = 0;
  for (const name of sources) {
    const source = path.join(sourceDir, name);
    const stem = path.basename(name, ".png");
    for (const width of [768, 1280]) {
      written += Number(await derive(
        source,
        path.join(outputDir, `${stem}-${width}.webp`),
        (image) => image.resize({ width, withoutEnlargement: true }).webp({ quality: width === 768 ? 74 : 80, effort: 5 }),
      ));
    }
  }
  return { sources: sources.length, written };
}

async function buildLoginBackgrounds() {
  const sourceDir = path.join(sourceArt, "login");
  const outputDir = path.join(publicArt, "derived", "login");
  await mkdir(outputDir, { recursive: true });
  const sources = (await readdir(sourceDir)).filter((name) => name.endsWith(".png")).sort();
  let written = 0;
  for (const name of sources) {
    written += Number(await derive(
      path.join(sourceDir, name),
      path.join(outputDir, `${path.basename(name, ".png")}.webp`),
      // Full native resolution on both shells: cover crops can magnify the
      // artwork, so don't serve a thumbnail or upscale a pretend 4K master.
      (image) => image.webp({ quality: 92, effort: 5 }),
    ));
  }
  return { sources: sources.length, written };
}

/** §hub-visit: the Market Hub as an arriving freighter sees it, the Market
 * pop-out's left column (Peter, Oct 28 2026). A tall cut of the login
 * painting's right side: the hub's wormhole above, a freighter's engines
 * below. Native pixels only; bump the file version when the cut changes. */
async function buildHubVisitArt() {
  const source = path.join(sourceArt, "login", "login-market-horizon-v1.png");
  const outputDir = path.join(publicArt, "derived", "hub-visit");
  await mkdir(outputDir, { recursive: true });
  const written = Number(await derive(
    source, path.join(outputDir, "hub-visit-v1.webp"),
    (image) => image.extract({ left: 1100, top: 0, width: 572, height: 941 }).webp({ quality: 88, effort: 5 }),
  ));
  return { sources: 1, written };
}

async function buildAuthoritySeal() {
  const source = path.join(sourceArt, "authority-v2", "terran-charter-authority-seal.png");
  const outputDir = path.join(publicArt, "derived", "authority-v2");
  await mkdir(outputDir, { recursive: true });
  let written = 0;
  for (const size of [96, 192, 288, 384]) {
    written += Number(await derive(
      source, path.join(outputDir, `terran-charter-authority-seal-${size}.webp`),
      // Keep the tall emblem's proportions and generated transparency. Lossless
      // variants preserve the fine orbital ring on high-density displays.
      image => image.resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
        .webp({ lossless: true, effort: 5 }),
    ));
  }
  return { sources: 1, written };
}

// §season: the news feed's sources (docs/gnn-art-prompts.md), cut to the
// emblem so it fills its square: 48 for the ticker and News rows, 128 for the
// story pop-up's header.
export const NEWS_EMBLEMS = ["gnn-emblem", "free-signal-emblem"];
async function buildNewsEmblems() {
  const outputDir = path.join(publicArt, "derived", "gnn-v1");
  await mkdir(outputDir, { recursive: true });
  let written = 0;
  for (const slug of NEWS_EMBLEMS) {
    for (const size of [48, 128]) {
      written += Number(await derive(
        path.join(sourceArt, "gnn-v1", `${slug}.png`), path.join(outputDir, `${slug}-${size}.webp`),
        image => image.trim().resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
          .webp({ lossless: true, effort: 5 }),
      ));
    }
  }
  return { sources: NEWS_EMBLEMS.length, written };
}

// The game's logo above the sign-in card, cropped to the artwork so the card's
// width sets its size. Widths serve 1×–3× of the card's 480 px.
const LOGIN_BANNER_WIDTHS = [480, 960, 1440];
async function buildLoginBanner() {
  const source = path.join(publicArt, "stellar_syndicates_login_banner_v1.png");
  const outputDir = path.join(publicArt, "derived", "login");
  await mkdir(outputDir, { recursive: true });
  const outputs = LOGIN_BANNER_WIDTHS.map((width) => ({ width, file: path.join(outputDir, `stellar-syndicates-banner-v1-${width}.webp`) }));
  const due = [];
  for (const output of outputs) if (await stale(source, output.file)) due.push(output);
  if (!due.length) return { sources: 1, written: 0 };
  const { data, info } = await sharp(source).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let left = info.width, right = -1, top = info.height, bottom = -1;
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    if (data[(y * info.width + x) * 4 + 3] <= 8) continue;
    left = Math.min(left, x); right = Math.max(right, x); top = Math.min(top, y); bottom = Math.max(bottom, y);
  }
  if (right < 0) throw new Error("the login banner is empty");
  for (const { width, file } of due) {
    await sharp(source).extract({ left, top, width: right - left + 1, height: bottom - top + 1 })
      .resize({ width, withoutEnlargement: true }).webp({ quality: 90, alphaQuality: 100, effort: 5 }).toFile(file);
  }
  return { sources: 1, written: due.length };
}

async function buildPwaIcons() {
  const source = path.join(publicArt, "stellar_syndicates_logo.png");
  const outputDir = path.join(publicArt, "pwa");
  await mkdir(outputDir, { recursive: true });
  const variants = [
    { name: "icon-192.png", size: 192, logo: 0.86 },
    { name: "icon-512.png", size: 512, logo: 0.86 },
    { name: "icon-maskable-512.png", size: 512, logo: 0.70 },
    { name: "apple-touch-icon.png", size: 180, logo: 0.82 },
  ];
  let written = 0;
  for (const variant of variants) {
    const output = path.join(outputDir, variant.name);
    if (!(await stale(source, output))) continue;
    const logoSize = Math.round(variant.size * variant.logo);
    const logo = await sharp(source).resize(logoSize, logoSize, { fit: "contain" }).png().toBuffer();
    await sharp({
      create: {
        width: variant.size,
        height: variant.size,
        channels: 4,
        background: "#05070d",
      },
    }).composite([{ input: logo, gravity: "center" }]).png({ compressionLevel: 9 }).toFile(output);
    written++;
  }
  return { sources: variants.length, written };
}

async function buildStructureIcons() {
  const sourceDir = path.join(sourceArt, "ui-icons", "structures");
  const outputDir = path.join(publicArt, "ui_icons", "structures");
  await mkdir(outputDir, { recursive: true });
  const sources = (await readdir(sourceDir)).filter((name) => name.endsWith(".png")).sort();
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let written = 0;
  // 128px for panels and rows; a 256px twin (same 6/128 padding ratio) for the
  // planet scene's footprints, which grow past 128 CSS pixels on large screens.
  const variants = [{ suffix: "", inner: 116, pad: 6 }, { suffix: "-256", inner: 232, pad: 12 }];
  for (const name of sources) {
    const stem = path.basename(name, ".png");
    for (const { suffix, inner, pad } of variants) {
      written += Number(await derive(
        path.join(sourceDir, name),
        path.join(outputDir, `${stem}${suffix}.png`),
        (image) => image
          .trim({ background: transparent, threshold: 3 })
          .resize(inner, inner, { fit: "contain", background: transparent })
          .extend({ top: pad, bottom: pad, left: pad, right: pad, background: transparent })
          .png({ compressionLevel: 9 }),
      ));
    }
  }
  return { sources: sources.length, written };
}

async function buildResourceIcons() {
  const sourceDir = path.join(sourceArt, "ui-icons", "resources");
  const outputDir = path.join(publicArt, "ui_icons", "resource");
  await mkdir(outputDir, { recursive: true });
  const sources = (await readdir(sourceDir)).filter((name) => name.endsWith(".png")).sort();
  let written = 0;
  for (const name of sources) {
    written += Number(await derive(
      path.join(sourceDir, name), path.join(outputDir, name),
      (image) => image.resize(64, 64, { fit: "contain", background: "#0b1422" }).png({ compressionLevel: 9 }),
    ));
  }
  // New ore/material masters retain alpha; ship only inventory-sized variants.
  const oreSource = path.join(sourceArt, "ores-2026-09-16");
  const oreOutput = path.join(outputDir, "ores-2026-09-16");
  await mkdir(oreOutput, { recursive: true });
  for (const name of (await readdir(oreSource)).filter(n => n.endsWith(".png"))) {
    for (const size of [64, 128]) {
      written += Number(await derive(path.join(oreSource, name),
        path.join(oreOutput, `${path.basename(name, ".png")}-${size}.png`),
        image => image.resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } }).png({ compressionLevel: 9 })));
    }
  }
  return { sources: sources.length, written };
}

// §goods-icons (Peter, Oct 24 2026: "all the resource icons to look the
// same"): every good's icon comes from one style of master, the ore set's
// prompt (docs/resource-icon-prompts.md). Each is trimmed to its artwork and
// re-centred at one fill, so a stockpile reads as one set whatever each
// master's margins. A later folder's master replaces an earlier one's.
const GOODS_SOURCES = ["ores-2026-09-16", "fuel-goods-2026-10-12", "goods-2026-10-24"];
const GOODS_FILL = 0.9;
async function buildGoodsIcons() {
  const outputDir = path.join(publicArt, "ui_icons", "resource", "goods-v1");
  await mkdir(outputDir, { recursive: true });
  const masters = new Map();
  for (const folder of GOODS_SOURCES) {
    const dir = path.join(sourceArt, folder);
    for (const name of (await readdir(dir).catch(() => [])).filter((n) => n.endsWith(".png")).sort()) {
      masters.set(path.basename(name, ".png"), path.join(dir, name));
    }
  }
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let written = 0;
  for (const [slug, source] of masters) {
    const outputs = [64, 128].map((size) => ({ size, file: path.join(outputDir, `${slug}-${size}.png`) }));
    const due = [];
    for (const output of outputs) if (await stale(source, output.file)) due.push(output);
    if (!due.length) continue;
    const { data, info } = await sharp(source).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    let left = info.width, right = -1, top = info.height, bottom = -1;
    for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
      if (data[(y * info.width + x) * 4 + 3] <= 8) continue;
      left = Math.min(left, x); right = Math.max(right, x); top = Math.min(top, y); bottom = Math.max(bottom, y);
    }
    if (right < 0) throw new Error(`${slug}: an empty goods icon master`);
    for (const { size, file } of due) {
      const inner = Math.round(size * GOODS_FILL);
      const art = await sharp(source).extract({ left, top, width: right - left + 1, height: bottom - top + 1 })
        .resize(inner, inner, { fit: "contain", background: transparent }).png().toBuffer();
      const pad = (size - inner) / 2;
      await sharp(art).extend({ top: Math.floor(pad), bottom: Math.ceil(pad), left: Math.floor(pad), right: Math.ceil(pad), background: transparent })
        .png({ compressionLevel: 9 }).toFile(file);
      written++;
    }
  }
  // icons.ts GOODS_ICONS names the goods using these; test-ore-economy keeps
  // that list and these masters in step.
  return { sources: masters.size, written };
}

// §belts: one chunk master per belt type (the Stony type keeps the original
// sprite). 256 px for the System View's scattered chunks, 512 px for the belt's
// world panel. Masters keep the cluster at ~43% of the canvas, like the
// original, so CHUNK_ART_FILL holds for every type.
async function buildBeltChunks() {
  const sourceDir = path.join(sourceArt, "belt-chunks-v1");
  const outputDir = path.join(publicArt, "derived", "belt-chunks", "v1");
  await mkdir(outputDir, { recursive: true });
  const sources = (await readdir(sourceDir)).filter((name) => name.endsWith(".png")).sort();
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let written = 0;
  for (const name of sources) {
    for (const size of [256, 512]) {
      written += Number(await derive(path.join(sourceDir, name),
        path.join(outputDir, `${path.basename(name, ".png")}-${size}.png`),
        (image) => image.resize(size, size, { fit: "contain", background: transparent }).png({ compressionLevel: 9 })));
    }
  }
  return { sources: sources.length, written };
}

// §den (Oct 30 2026): the rocks behind a den's battles
// (docs/battle-asteroid-prompts.md). Each belt type's sheet holds six
// asteroids in a 3 × 2 grid of 512 px cells. Each rock is cut to its cell's
// largest silhouette (alpha ≥ 8, which drops the generator's matte haze and
// stray pixels), centred in a square it fills, and sized to 256.
export const BATTLE_ASTEROID_KINDS = ["stony", "metallic", "icy", "shattered"];
export const BATTLE_ASTEROIDS_PER_KIND = 6;
const ROCK_CELL = 512;
const ROCK_ALPHA = 8;
const ROCK_PAD = 4;
async function buildBattleAsteroids() {
  const sourceDir = path.join(sourceArt, "battle-asteroids-v1");
  const outputDir = path.join(publicArt, "derived", "battle-asteroids", "v1");
  await mkdir(outputDir, { recursive: true });
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let sources = 0, written = 0;
  for (const kind of BATTLE_ASTEROID_KINDS) {
    const source = path.join(sourceDir, `${kind}-sheet.png`);
    try { await stat(source); } catch { continue; }
    sources++;
    const outputs = Array.from({ length: BATTLE_ASTEROIDS_PER_KIND }, (_, i) => path.join(outputDir, `${kind}-${i + 1}-256.webp`));
    if (!(await Promise.all(outputs.map((output) => stale(source, output)))).some(Boolean)) continue;
    const { data, info } = await sharp(source).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    if (info.width !== 3 * ROCK_CELL || info.height !== 2 * ROCK_CELL) throw new Error(`Battle asteroid sheets are 1536 × 1024: ${source}`);
    for (const [cell, output] of outputs.entries()) {
      const x0 = (cell % 3) * ROCK_CELL, y0 = Math.floor(cell / 3) * ROCK_CELL;
      const at = (x, y) => ((y0 + y) * info.width + x0 + x) * 4;
      // The cell's largest 8-connected silhouette.
      const label = new Int32Array(ROCK_CELL * ROCK_CELL);
      let best = { id: 0, size: 0 };
      for (let start = 0, id = 1; start < label.length; start++) {
        if (label[start] || data[at(start % ROCK_CELL, Math.floor(start / ROCK_CELL)) + 3] < ROCK_ALPHA) continue;
        const stack = [start];
        let size = 0;
        label[start] = id;
        while (stack.length) {
          const i = stack.pop(), x = i % ROCK_CELL, y = Math.floor(i / ROCK_CELL);
          size++;
          for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
            const nx = x + dx, ny = y + dy, n = ny * ROCK_CELL + nx;
            if (nx < 0 || ny < 0 || nx >= ROCK_CELL || ny >= ROCK_CELL || label[n] || data[at(nx, ny) + 3] < ROCK_ALPHA) continue;
            label[n] = id;
            stack.push(n);
          }
        }
        if (size > best.size) best = { id, size };
        id++;
      }
      if (!best.size) throw new Error(`No rock in cell ${cell + 1}: ${source}`);
      const rock = Buffer.alloc(label.length * 4);
      let left = ROCK_CELL, top = ROCK_CELL, right = 0, bottom = 0;
      for (let i = 0; i < label.length; i++) {
        if (label[i] !== best.id) continue;
        const x = i % ROCK_CELL, y = Math.floor(i / ROCK_CELL);
        data.copy(rock, i * 4, at(x, y), at(x, y) + 4);
        left = Math.min(left, x); top = Math.min(top, y); right = Math.max(right, x); bottom = Math.max(bottom, y);
      }
      left = Math.max(0, left - ROCK_PAD); top = Math.max(0, top - ROCK_PAD);
      right = Math.min(ROCK_CELL - 1, right + ROCK_PAD); bottom = Math.min(ROCK_CELL - 1, bottom + ROCK_PAD);
      const trimmed = await sharp(rock, { raw: { width: ROCK_CELL, height: ROCK_CELL, channels: 4 } })
        .extract({ left, top, width: right - left + 1, height: bottom - top + 1 }).png().toBuffer();
      await sharp(trimmed).resize(256, 256, { fit: "contain", background: transparent, kernel: "lanczos3" })
        .webp({ quality: 88, alphaQuality: 100, effort: 5 }).toFile(output);
      written++;
    }
  }
  return { sources, written };
}

async function buildNebulas() {
  const sourceDir = path.join(sourceArt, "nebulas");
  const outputDir = path.join(publicArt, "nebulas");
  await mkdir(outputDir, { recursive: true });
  const sources = (await readdir(sourceDir)).filter((name) => name.endsWith(".png")).sort();
  const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
  let written = 0;
  for (const name of sources) {
    written += Number(await derive(
      path.join(sourceDir, name),
      path.join(outputDir, name),
      (image) => image
        .resize(1024, 1024, { fit: "contain", background: transparent })
        .png({ compressionLevel: 9 }),
    ));
  }
  return { sources: sources.length, written };
}

const [captains, lore, pwa, structures, nebulas, login, loginBanner, resources, goods, stars, ships, planets, planetWorkbench, structureTiers, foundingStory, authority, commandCenters, planetShipyards, surfaceStructures, stations, hub, glyphs, beltChunks, storyEvents, traderPortraits, pirateCrew, quarryArt, signals, shipyardInteriors, seasonArt, hubVisit, newsEmblems, battleAsteroids] = await Promise.all([
  buildCaptains(), buildLore(), buildPwaIcons(), buildStructureIcons(), buildNebulas(), buildLoginBackgrounds(), buildLoginBanner(),
  buildResourceIcons(), buildGoodsIcons(), buildStarDerivatives(), buildShipDerivatives(), buildPlanetDerivatives(),
  buildPlanetWorkbenchArt(), buildStructureTierArt(), buildFoundingStoryArt(), buildAuthoritySeal(),
  buildCommandCenterArt(),
  buildPlanetShipyardArt(),
  buildPlanetSurfaceStructureArt(),
  buildStationArt(),
  buildHubModelAssets(),
  buildMapGlyphs(),
  buildBeltChunks(),
  buildStoryEventArt(),
  buildTraderPortraitArt(),
  buildPirateCrewArt(),
  buildAsteroidQuarryArt(),
  buildSignalArt(),
  buildShipyardInteriorArt(),
  buildSeasonArt(),
  buildHubVisitArt(),
  buildNewsEmblems(),
  buildBattleAsteroids(),
]);
console.log(
  `art derivatives: ${captains.sources} captain portraits (${captains.written} written), ` +
  `${lore.sources} lore illustrations (${lore.written} written), ` +
  `${pwa.sources} PWA icons (${pwa.written} written), ` +
  `${structures.sources} structure icons (${structures.written} written), ` +
  `${structureTiers.sources} structure tiers (${structureTiers.written} written), ` +
  `${commandCenters.sources} command center levels (${commandCenters.written} written), ` +
  `${planetShipyards.sources} planet shipyard levels (${planetShipyards.written} written), ` +
  `${surfaceStructures.sources} surface structures/biome layers (${surfaceStructures.written} written), ` +
  `${stations.sources} defense station levels (${stations.written} written), ` +
  `${resources.sources} resource icons (${resources.written} written), ` +
  `${goods.sources} goods icons (${goods.written} written), ` +
  `${nebulas.sources} nebula textures (${nebulas.written} written), ` +
  `${login.sources} login backgrounds (${login.written} written), ` +
  `login banner (${loginBanner.written} written), ` +
  `${stars.sources} star masters / ${stars.variants} sizes (${stars.written} written), ` +
  `${ships.sources} ship masters (${ships.written} written), ` +
  `${planets.sources} planet masters / ${planets.variants} sizes (${planets.written} written), ` +
  `planet workbench (${planetWorkbench.written} written), ` +
  `${foundingStory.sources} founding story scenes (${foundingStory.written} written), ` +
  `${storyEvents.sources} story events, ${storyEvents.pending} awaiting art (${storyEvents.written} written), ` +
  `${traderPortraits.sources} trader portraits, ${traderPortraits.pending} awaiting art (${traderPortraits.written} written), ` +
  `${pirateCrew.sources} pirate portraits, ${pirateCrew.pending} awaiting art (${pirateCrew.written} written), ` +
  `${quarryArt.sources} asteroid quarry tiers, ${quarryArt.pending} awaiting art (${quarryArt.written} written), ` +
  `${shipyardInteriors.sources} shipyard interiors (${shipyardInteriors.written} written), ` +
  `${seasonArt.sources} season gear/gate/pirate masters (${seasonArt.written} written), ` +
  `Market Hub arrival (${hubVisit.written} written), ` +
  `${newsEmblems.sources} news emblems (${newsEmblems.written} written), ` +
  `${signals.sources} signal paintings, ${signals.pending} awaiting art (${signals.written} written), ` +
  `${authority.sources} Authority seal (${authority.written} written), ` +
  `hub station ${(hub.glbBytes / 1e6).toFixed(2)} MB (${hub.written} written), ` +
  `${glyphs.glyphs} map glyphs (${glyphs.written} written), ` +
  `${beltChunks.sources} belt chunk types (${beltChunks.written} written), ` +
  `${battleAsteroids.sources} battle asteroid sheets (${battleAsteroids.written} written)`,
);

import { mkdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));
export const FOUNDING_STORY_ART = ["wormhole-opening-selected", "stellar-charter", "first-landing", "stay-vigilant-firing"];
export const FOUNDING_STORY_WIDTHS = [768, 1280, 1672];

export async function buildFoundingStoryArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/founding-intro/v1");
  await mkdir(outputRoot, { recursive: true });
  let written = 0;
  for (const slug of FOUNDING_STORY_ART) {
    const source = path.join(clientRoot, `art-src/founding-intro-v1/${slug}.png`);
    const meta = await sharp(source).metadata();
    if (meta.width !== 1672 || meta.height !== 941) throw new Error(`Unexpected prologue dimensions: ${source}`);
    for (const width of FOUNDING_STORY_WIDTHS) {
      const output = path.join(outputRoot, `${slug}-${width}.webp`);
      try { if ((await stat(output)).mtimeMs >= (await stat(source)).mtimeMs) continue; } catch { /* first build */ }
      // Every size comes directly from the approved master, with its full frame.
      await sharp(source).resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }).toFile(output);
      written++;
    }
  }
  return { sources: FOUNDING_STORY_ART.length, written };
}

/** §charter-v5: the charter's event pop-ups (client/src/story-events.ts), in
 * the prologue's format. A master not yet delivered is skipped; the pop-up
 * shows its interim image until it lands. The counterfeit and Hub-lane
 * paintings retired with charter v4. §season (Oct 29 2026): the GNN stories
 * (docs/gnn-art-prompts.md); §chapters: the chapter openings
 * (docs/chapter-art-prompts.md). */
export const STORY_EVENT_ART = ["free-trader-inbound", "pirate-raid", "new-colony", "gate-opens", "relics-awaken", "pirate-vendetta",
  "decree-levies", "decree-inspections", "decree-martial", "sabotage-at-the-hub", "convoy-massacre", "quarantine", "relic-seizure",
  "keystone-crackdown", "false-flag-exposed", "sealed-world-found", "authority-spy", "authority-war", "seal-fires", "charter-holds",
  "chapter-pirates", "chapter-expansion", "chapter-reach"];

/** §season: the GNN paintings came from ChatGPT at 1536 × 1024 and stay as
 * delivered (art-src/gnn-v1/generation-manifest.json). The build cuts each
 * one's 16:9 band, 1536 × 864, this many rows from the top (0–160, framing its
 * subject), and sizes it to the 1672 × 941 frame. */
export const STORY_EVENT_BAND = {
  "decree-levies": 80, "decree-inspections": 80, "decree-martial": 80, "sabotage-at-the-hub": 40, "convoy-massacre": 40,
  "quarantine": 64, "relic-seizure": 64, "keystone-crackdown": 80, "false-flag-exposed": 40, "sealed-world-found": 80,
  "authority-spy": 0, "authority-war": 80, "seal-fires": 80, "charter-holds": 64,
  // §chapters (Oct 30 2026, story-events-v1/chapter-generation-manifest.json):
  // the privateer's trail and the Hub's glint clear of the top; the den's
  // wreck and its star kept whole.
  "chapter-pirates": 64, "chapter-expansion": 64, "chapter-reach": 0,
};

export async function buildStoryEventArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/story-events/v1");
  await mkdir(outputRoot, { recursive: true });
  const script = (await stat(fileURLToPath(import.meta.url))).mtimeMs;
  let sources = 0, written = 0;
  for (const slug of STORY_EVENT_ART) {
    const source = path.join(clientRoot, `art-src/story-events-v1/${slug}.png`);
    try { await stat(source); } catch { continue; }
    sources++;
    const meta = await sharp(source).metadata();
    const band = STORY_EVENT_BAND[slug];
    const native = meta.width === 1672 && meta.height === 941;
    if (!native && !(band !== undefined && meta.width === 1536 && meta.height === 1024)) throw new Error(`Unexpected story event dimensions: ${source}`);
    for (const width of FOUNDING_STORY_WIDTHS) {
      const output = path.join(outputRoot, `${slug}-${width}.webp`);
      // A band moves with this script, so a cut master rebuilds when it changes.
      try { if ((await stat(output)).mtimeMs >= Math.max((await stat(source)).mtimeMs, native ? 0 : script)) continue; } catch { /* first build */ }
      const image = sharp(source);
      if (!native) image.extract({ left: 0, top: band, width: 1536, height: 864 });
      await image.resize({ width, withoutEnlargement: native, kernel: "lanczos3" }).webp({ quality: 88, effort: 5 }).toFile(output);
      written++;
    }
  }
  return { sources, pending: STORY_EVENT_ART.length - sources, written };
}

/** §persona: the free traders in the trading panel
 * (client/src/core/derive/trader.ts): Teo Brannick, and since §traders (Oct
 * 27 2026) Ines Varga, Mags Halloran and Rafi Mensah, each a square portrait and two
 * optional expressions. A master not yet delivered is skipped; the panel
 * falls back to the neutral portrait, then to the trader's ship. */
export const TRADER_PORTRAIT_ART = ["teo-brannick", "ines-varga", "mags-halloran", "rafi-mensah"]
  .flatMap((trader) => [trader, `${trader}-pleased`, `${trader}-scoff`]);
export const TRADER_PORTRAIT_SIZE = 320;
/** The panel shows the portrait at 90–140 px (2× screens want 280), where the full frame leaves his
 * face too small to read: the same square around the face from all three
 * masters (fractions of the master), so a change of mood never jumps. */
export const TRADER_PORTRAIT_CROP = { left: 0.143, top: 0.113, size: 0.75 };

/** §vendetta: the pirate families' faces (client/src/core/derive/vendetta.ts):
 * each family's nephew and uncle, plain and smug, angry and shocked, cut like
 * Teo's (the same framing, so the same crop). */
export const PIRATE_FAMILIES = ["showboat", "hothead", "collector", "scrapper", "fatalist"];
export const PIRATE_MOODS = ["smug", "angry", "shocked"];
export const PIRATE_PORTRAIT_ART = PIRATE_FAMILIES.flatMap(family => ["", "-uncle"].flatMap(role =>
  [`${family}${role}`, ...PIRATE_MOODS.map(mood => `expressions/${family}${role}-${mood}`)]));

export async function buildPirateCrewArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/pirates/v1");
  await mkdir(outputRoot, { recursive: true });
  let sources = 0, written = 0;
  for (const slug of PIRATE_PORTRAIT_ART) {
    const source = path.join(clientRoot, `art-src/pirate-portraits-v1/${slug}.png`);
    try { await stat(source); } catch { continue; }
    sources++;
    const meta = await sharp(source).metadata();
    if (meta.width !== meta.height || meta.width < 512) throw new Error(`Pirate portraits are square, at least 512 px: ${source}`);
    const output = path.join(outputRoot, `${path.basename(slug)}-${TRADER_PORTRAIT_SIZE}.webp`);
    try { if ((await stat(output)).mtimeMs >= Math.max((await stat(source)).mtimeMs, (await stat(fileURLToPath(import.meta.url))).mtimeMs)) continue; } catch { /* first build */ }
    const side = Math.round(meta.width * TRADER_PORTRAIT_CROP.size);
    const crop = { left: Math.round(meta.width * TRADER_PORTRAIT_CROP.left), top: Math.round(meta.height * TRADER_PORTRAIT_CROP.top), width: side, height: side };
    await sharp(source).extract(crop).resize(TRADER_PORTRAIT_SIZE, TRADER_PORTRAIT_SIZE).webp({ quality: 86, effort: 5 }).toFile(output);
    written++;
  }
  return { sources, pending: PIRATE_PORTRAIT_ART.length - sources, written };
}

export async function buildTraderPortraitArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/free-trader/v1");
  await mkdir(outputRoot, { recursive: true });
  let sources = 0, written = 0;
  for (const slug of TRADER_PORTRAIT_ART) {
    const source = path.join(clientRoot, `art-src/free-trader-portrait-v1/${slug}.png`);
    try { await stat(source); } catch { continue; }
    sources++;
    const meta = await sharp(source).metadata();
    if (meta.width !== meta.height || meta.width < 512) throw new Error(`Trader portraits are square, at least 512 px: ${source}`);
    const output = path.join(outputRoot, `${slug}-${TRADER_PORTRAIT_SIZE}.webp`);
    try { if ((await stat(output)).mtimeMs >= Math.max((await stat(source)).mtimeMs, (await stat(fileURLToPath(import.meta.url))).mtimeMs)) continue; } catch { /* first build */ }
    const side = Math.round(meta.width * TRADER_PORTRAIT_CROP.size);
    const crop = { left: Math.round(meta.width * TRADER_PORTRAIT_CROP.left), top: Math.round(meta.height * TRADER_PORTRAIT_CROP.top), width: side, height: side };
    await sharp(source).extract(crop).resize(TRADER_PORTRAIT_SIZE, TRADER_PORTRAIT_SIZE).webp({ quality: 86, effort: 5 }).toFile(output);
    written++;
  }
  return { sources, pending: TRADER_PORTRAIT_ART.length - sources, written };
}

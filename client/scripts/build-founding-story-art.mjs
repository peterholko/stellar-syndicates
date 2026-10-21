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
 * paintings retired with charter v4. */
export const STORY_EVENT_ART = ["free-trader-inbound", "pirate-raid"];

export async function buildStoryEventArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/story-events/v1");
  await mkdir(outputRoot, { recursive: true });
  let sources = 0, written = 0;
  for (const slug of STORY_EVENT_ART) {
    const source = path.join(clientRoot, `art-src/story-events-v1/${slug}.png`);
    try { await stat(source); } catch { continue; }
    sources++;
    const meta = await sharp(source).metadata();
    if (meta.width !== 1672 || meta.height !== 941) throw new Error(`Unexpected story event dimensions: ${source}`);
    for (const width of FOUNDING_STORY_WIDTHS) {
      const output = path.join(outputRoot, `${slug}-${width}.webp`);
      try { if ((await stat(output)).mtimeMs >= (await stat(source)).mtimeMs) continue; } catch { /* first build */ }
      await sharp(source).resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }).toFile(output);
      written++;
    }
  }
  return { sources, pending: STORY_EVENT_ART.length - sources, written };
}

/** §persona: Teo Brannick, the free trader, in the trading panel
 * (client/src/core/derive/trader.ts): a square portrait and two optional
 * expressions. A master not yet delivered is skipped; the panel falls back
 * to the neutral portrait, then to the Long Odds. */
export const TRADER_PORTRAIT_ART = ["teo-brannick", "teo-brannick-pleased", "teo-brannick-scoff"];
export const TRADER_PORTRAIT_SIZE = 256;
/** The panel shows the portrait at 72–112 px, where the full frame leaves his
 * face too small to read: the same square around the face from all three
 * masters (fractions of the master), so a change of mood never jumps. */
export const TRADER_PORTRAIT_CROP = { left: 0.143, top: 0.113, size: 0.75 };

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

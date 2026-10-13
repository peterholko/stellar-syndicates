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

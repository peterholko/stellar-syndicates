// §asteroid-quarry: the quarry's six tiers (art-src/asteroid-quarry-v2/tier-N.png,
// 1672 × 941 with alpha, the rock and its machinery): the belt scene's wide
// sprite at three widths, and the same art trimmed and framed square for menus.
import { mkdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));
export const QUARRY_TIERS = 6;
export const QUARRY_SCENE_WIDTHS = [640, 1280, 1672];
export const QUARRY_ICON_SIZES = [128, 256, 512];

export async function buildAsteroidQuarryArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/asteroid-quarry/v2");
  await mkdir(outputRoot, { recursive: true });
  const script = (await stat(fileURLToPath(import.meta.url))).mtimeMs;
  let sources = 0, written = 0;
  for (let tier = 1; tier <= QUARRY_TIERS; tier++) {
    const source = path.join(clientRoot, `art-src/asteroid-quarry-v2/tier-${tier}.png`);
    let changed;
    try { changed = Math.max((await stat(source)).mtimeMs, script); } catch { continue; }
    sources++;
    const meta = await sharp(source).metadata();
    if (meta.width !== 1672 || meta.height !== 941 || !meta.hasAlpha) throw new Error(`Quarry tiers are 1672 × 941 with alpha: ${source}`);
    const fresh = async output => { try { return (await stat(output)).mtimeMs >= changed; } catch { return false; } };
    for (const width of QUARRY_SCENE_WIDTHS) {
      const output = path.join(outputRoot, `tier-${tier}-${width}.webp`);
      if (await fresh(output)) continue;
      await sharp(source).resize({ width }).webp({ quality: 86, alphaQuality: 100, effort: 5 }).toFile(output);
      written++;
    }
    const trimmed = await sharp(source).trim({ threshold: 1 }).toBuffer();
    for (const size of QUARRY_ICON_SIZES) {
      const output = path.join(outputRoot, `icon-tier-${tier}-${size}.webp`);
      if (await fresh(output)) continue;
      await sharp(trimmed).resize(size, size, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 } })
        .webp({ quality: 86, alphaQuality: 100, effort: 5 }).toFile(output);
      written++;
    }
  }
  return { sources, pending: QUARRY_TIERS - sources, written };
}

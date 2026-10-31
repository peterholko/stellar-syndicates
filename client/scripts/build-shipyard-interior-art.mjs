import { mkdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));

/** §shipyard-view (Peter, Oct 21 2026): the empty yard interiors a hull under
 * construction is rendered over (client/src/ship-construction-view.ts), one
 * per yard. Each master is a 1672 × 941 plate with a clear central volume. */
export const SHIPYARD_INTERIOR_ART = ["shipyard-ii-empty-bay", "orbital-shipyard-empty-bay", "capital-slipway-empty-bay"];
export const SHIPYARD_INTERIOR_WIDTHS = [768, 1280, 1672];

export async function buildShipyardInteriorArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/shipyard-interiors/v1");
  await mkdir(outputRoot, { recursive: true });
  let sources = 0, written = 0;
  for (const slug of SHIPYARD_INTERIOR_ART) {
    const source = path.join(clientRoot, `art-src/shipyard-interiors-v1/${slug}.png`);
    try { await stat(source); } catch { continue; }
    sources++;
    const meta = await sharp(source).metadata();
    if (meta.width !== 1672 || meta.height !== 941) throw new Error(`Unexpected shipyard interior dimensions: ${source}`);
    for (const width of SHIPYARD_INTERIOR_WIDTHS) {
      const output = path.join(outputRoot, `${slug}-${width}.webp`);
      try { if ((await stat(output)).mtimeMs >= (await stat(source)).mtimeMs) continue; } catch { /* first build */ }
      await sharp(source).resize({ width, withoutEnlargement: true }).webp({ quality: 86, effort: 5 }).toFile(output);
      written++;
    }
  }
  return { sources, pending: SHIPYARD_INTERIOR_ART.length - sources, written };
}

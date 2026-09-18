import { mkdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));
const sourceRoot = path.join(clientRoot, "art-src/planet-view");
const outputRoot = path.join(clientRoot, "public/art/derived/planet-workbench/v1");

// Reuse the approved empty landscapes; buildings are live DOM sprites, never
// baked into the background (an undeveloped or rival world must stay empty).
export async function buildPlanetWorkbenchArt() {
  await mkdir(outputRoot, { recursive: true });
  let written = 0;
  const derive = async (source, output, transform) => {
    try { if ((await stat(output)).mtimeMs >= (await stat(source)).mtimeMs) return; } catch { /* first build */ }
    await transform(sharp(source)).toFile(output);
    written++;
  };
  for (const kind of ["barren", "desert", "terrestrial", "ocean", "ice", "lava"]) {
    for (const width of [768, 1536]) {
      await derive(path.join(sourceRoot, `surfaces/${kind}-surface-v1.png`),
        path.join(outputRoot, `${kind}-${width}.webp`),
        image => image.resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }));
    }
  }
  for (const size of [128, 256]) {
    await derive(path.join(sourceRoot, "workforce-v1.png"), path.join(outputRoot, `workforce-${size}.png`),
      image => image.trim({ threshold: 12 }).resize(size, size * .75, {
        fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 },
      }).png());
  }
  return { written };
}

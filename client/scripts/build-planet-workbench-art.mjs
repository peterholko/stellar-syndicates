import { mkdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));
const sourceRoot = path.join(clientRoot, "art-src/planet-view");
const outputRoot = path.join(clientRoot, "public/art/derived/planet-workbench/v1");
export const HABITABLE_SURFACES = [
  "highland-plateau", "savanna-world", "volcanic-highlands", "boreal-world",
  "archipelago-world", "temperate-basin",
];
export const HABITABLE_SURFACE_WIDTHS = [768, 1280, 1672];
/** §belt-view: one painted field per belt type (the sim's BeltKind). */
export const BELT_KINDS = ["stony", "metallic", "icy", "shattered"];
/// §planet-orbit: every painting also comes extended upward by this many rows of
/// space, for the planet view's orbit layer (docs/planet-orbit-space-prompt.md).
/// The original 1672 × 941 painting is the bottom rows, pixel-identical (checked
/// below), so building sites stay in the 16:9 ground frame. The founding screen
/// keeps the 16:9 originals.
export const ORBIT_SPACE_ROWS = 240;
const GROUND_ROWS = 941;
/** A repainted master (`<kind>-surface-<version>.png` and its `-orbit`). The
 * Barren v2 (Oct 28 2026) drops v1's baked-in pads, roads, lights and quarry;
 * its orbit version is v1's approved space over the new ground. */
const SURFACE_VERSION = { barren: "v2" };

/** The extension may add space above, never touch the painting beneath it. */
async function assertGroundUnchanged(original, extended) {
  const meta = await sharp(extended).metadata();
  if (meta.width !== 1672 || meta.height !== GROUND_ROWS + ORBIT_SPACE_ROWS) {
    throw new Error(`Unexpected orbit painting dimensions ${meta.width}×${meta.height}: ${extended}`);
  }
  const [a, b] = await Promise.all([
    sharp(original).removeAlpha().raw().toBuffer(),
    sharp(extended).removeAlpha().extract({ left: 0, top: ORBIT_SPACE_ROWS, width: 1672, height: GROUND_ROWS }).raw().toBuffer(),
  ]);
  if (!a.equals(b)) throw new Error(`Orbit painting changed the original below its added space: ${extended}`);
}

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
  const orbitOutput = path.join(clientRoot, "public/art/derived/planet-workbench/orbit-v1");
  await mkdir(orbitOutput, { recursive: true });
  for (const kind of ["barren", "desert", "terrestrial", "ocean", "ice", "lava", "marginal"]) {
    for (const width of [768, 1536]) {
      await derive(path.join(sourceRoot, `surfaces/${kind}-surface-${SURFACE_VERSION[kind] ?? "v1"}.png`),
        path.join(outputRoot, `${kind}-${width}.webp`),
        image => image.resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }));
    }
    const original = path.join(sourceRoot, `surfaces/${kind}-surface-${SURFACE_VERSION[kind] ?? "v1"}.png`);
    const extended = path.join(sourceRoot, `surfaces/${kind}-surface-${SURFACE_VERSION[kind] ?? "v1"}-orbit.png`);
    await assertGroundUnchanged(original, extended);
    for (const width of [768, 1536]) {
      await derive(extended, path.join(orbitOutput, `${kind}-${width}.webp`),
        image => image.resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }));
    }
  }
  const habitableOutput = path.join(clientRoot, "public/art/derived/planet-workbench/habitable-v1");
  await mkdir(habitableOutput, { recursive: true });
  const habitableOrbitOutput = path.join(clientRoot, "public/art/derived/planet-workbench/habitable-orbit-v1");
  await mkdir(habitableOrbitOutput, { recursive: true });
  for (const kind of HABITABLE_SURFACES) {
    const source = path.join(sourceRoot, `habitable-v1/${kind}.png`);
    const meta = await sharp(source).metadata();
    // Width descriptors must name real pixels; don't silently upscale a smaller
    // replacement or distort a differently framed painting under the footprints.
    if (meta.width !== 1672 || meta.height !== 941) throw new Error(`Unexpected surface dimensions: ${source}`);
    for (const width of HABITABLE_SURFACE_WIDTHS) {
      await derive(source, path.join(habitableOutput, `${kind}-${width}.webp`),
        image => image.resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }));
    }
    const extended = path.join(sourceRoot, `habitable-v1/${kind}-orbit.png`);
    await assertGroundUnchanged(source, extended);
    for (const width of HABITABLE_SURFACE_WIDTHS) {
      await derive(extended, path.join(habitableOrbitOutput, `${kind}-${width}.webp`),
        image => image.resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }));
    }
  }
  // §belt-view: a belt's view is a painted asteroid field per belt type
  // (docs/belt-view-art.md), painted at the orbit size: space above, the field in
  // the same 16:9 ground frame the building sites use.
  const beltOutput = path.join(clientRoot, "public/art/derived/planet-workbench/belt-v1");
  await mkdir(beltOutput, { recursive: true });
  for (const kind of BELT_KINDS) {
    const source = path.join(clientRoot, `art-src/belt-view/${kind}-belt-v1.png`);
    const meta = await sharp(source).metadata();
    if (meta.width !== 1672 || meta.height !== GROUND_ROWS + ORBIT_SPACE_ROWS) throw new Error(`Unexpected belt painting dimensions: ${source}`);
    for (const width of HABITABLE_SURFACE_WIDTHS) {
      await derive(source, path.join(beltOutput, `${kind}-belt-${width}.webp`),
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

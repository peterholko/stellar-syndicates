import { mkdir, readdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));
const publicArt = path.join(clientRoot, "public", "art");

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

const [captains, lore] = await Promise.all([buildCaptains(), buildLore()]);
console.log(
  `art derivatives: ${captains.sources} captain portraits (${captains.written} written), ` +
  `${lore.sources} lore illustrations (${lore.written} written)`,
);

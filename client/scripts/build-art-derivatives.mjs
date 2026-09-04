import { mkdir, readdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

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
  for (const name of sources) {
    written += Number(await derive(
      path.join(sourceDir, name),
      path.join(outputDir, name),
      (image) => image
        .trim({ background: transparent, threshold: 3 })
        .resize(116, 116, { fit: "contain", background: transparent })
        .extend({ top: 6, bottom: 6, left: 6, right: 6, background: transparent })
        .png({ compressionLevel: 9 }),
    ));
  }
  return { sources: sources.length, written };
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

const [captains, lore, pwa, structures, nebulas] = await Promise.all([
  buildCaptains(), buildLore(), buildPwaIcons(), buildStructureIcons(), buildNebulas(),
]);
console.log(
  `art derivatives: ${captains.sources} captain portraits (${captains.written} written), ` +
  `${lore.sources} lore illustrations (${lore.written} written), ` +
  `${pwa.sources} PWA icons (${pwa.written} written), ` +
  `${structures.sources} structure icons (${structures.written} written), ` +
  `${nebulas.sources} nebula textures (${nebulas.written} written)`,
);

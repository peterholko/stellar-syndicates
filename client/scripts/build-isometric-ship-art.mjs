import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const client = fileURLToPath(new URL("..", import.meta.url));
const sourceDir = path.join(client, "art-src", "ships-isometric-v1");
const outputDir = path.join(client, "public", "art", "derived", "ships-isometric", "v1");
const ships = {
  raider: "interceptor",
  cruiser: "cruiser",
  titan: "titan",
  convoy: "freighter",
  colony: "colony",
  builder: "construction",
};
const sizes = [256, 512, 1024];
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");

await mkdir(outputDir, { recursive: true });
const manifest = { version: "v1", orientation: "nose upper right; stern lower left", ships: {} };

for (const [kind, name] of Object.entries(ships)) {
  const source = path.join(sourceDir, `${name}.png`);
  const bytes = await readFile(source);
  const metadata = await sharp(bytes).metadata();
  if (!metadata.hasAlpha) throw new Error(`${name}: transparent master required`);
  const levels = [];
  for (const size of sizes) {
    const filename = `${name}-${size}.webp`;
    const output = path.join(outputDir, filename);
    const derivative = await sharp(bytes)
      .resize(size, size, {
        fit: "contain",
        background: { r: 0, g: 0, b: 0, alpha: 0 },
        withoutEnlargement: true,
      })
      .webp({ quality: 90, alphaQuality: 100, effort: 5 })
      .toBuffer();
    await writeFile(output, derivative);
    levels.push({ size, url: `/art/derived/ships-isometric/v1/${filename}?v=${sha256(derivative).slice(0, 12)}` });
  }
  manifest.ships[kind] = {
    name,
    source: `art-src/ships-isometric-v1/${name}.png`,
    sourceSha256: sha256(bytes),
    width: metadata.width,
    height: metadata.height,
    levels,
  };
}

await writeFile(path.join(outputDir, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
console.log(`isometric ship art: ${Object.keys(ships).length} masters, ${Object.keys(ships).length * sizes.length} derivatives`);

// Asset-specific, user-authorized cleanup of the approved V6 preview.
// Remove the background and selection brackets without regenerating the art.
// Run from any directory: node client/scripts/prepare-battle-marker.mjs
import sharp from "sharp";

const source = new URL("../art-src/battle-marker-v6-source.png", import.meta.url);
const master = new URL("../art-src/battle-marker-v6.png", import.meta.url);
const runtime = new URL("../public/art/battle_in_progress_v6.png", import.meta.url);
const { data, info } = await sharp(source.pathname).removeAlpha().raw().toBuffer({ resolveWithObject: true });
const { width, height } = info;
if (width !== 1254 || height !== 1254) throw new Error("V6 cleanup coordinates require the approved 1254px source");

// These four rectangles contain only the baked selector and empty background;
// none intersects a ship or the explosion. Selection now belongs to the renderer.
const brackets = [[240, 225, 385, 360], [880, 225, 1010, 360], [240, 880, 385, 1010], [880, 880, 1010, 1010]];
const pixels = width * height;
const background = new Uint8Array(pixels);
const queue = new Int32Array(pixels);
let head = 0, tail = 0;
const eligible = (i) => {
  const x = i % width, y = Math.floor(i / width);
  if (brackets.some(([x0, y0, x1, y1]) => x >= x0 && x < x1 && y >= y0 && y < y1)) return true;
  const p = i * 3;
  // The preview's near-black background has minor compression/render noise.
  return Math.max(Math.abs(data[p] - 2), Math.abs(data[p + 1] - 3), Math.abs(data[p + 2] - 13)) <= 12;
};
const visit = (i) => {
  if (background[i] || !eligible(i)) return;
  background[i] = 1;
  queue[tail++] = i;
};
for (let x = 0; x < width; x++) { visit(x); visit((height - 1) * width + x); }
for (let y = 0; y < height; y++) { visit(y * width); visit(y * width + width - 1); }
// Flood only background connected to the outside; enclosed dark hull panels
// remain opaque. This avoids a global dark-color key cutting holes in ships.
while (head < tail) {
  const i = queue[head++], x = i % width;
  if (x > 0) visit(i - 1);
  if (x + 1 < width) visit(i + 1);
  if (i >= width) visit(i - width);
  if (i + width < pixels) visit(i + width);
}
const rgba = Buffer.alloc(pixels * 4);
for (let i = 0; i < pixels; i++) {
  if (background[i]) continue;
  data.copy(rgba, i * 4, i * 3, i * 3 + 3);
  rgba[i * 4 + 3] = 255;
}
await sharp(rgba, { raw: { width, height, channels: 4 } }).png({ compressionLevel: 9 }).toFile(master.pathname);
const transparent = { r: 0, g: 0, b: 0, alpha: 0 };
await sharp(master.pathname)
  .trim({ background: transparent, threshold: 1 })
  .resize(244, 244, { fit: "contain", background: transparent })
  .extend({ top: 6, bottom: 6, left: 6, right: 6, background: transparent })
  .png({ compressionLevel: 9 }).toFile(runtime.pathname);
console.log("Prepared V6 battle sprite: original fleets/explosion preserved, brackets removed, 256px RGBA runtime.");

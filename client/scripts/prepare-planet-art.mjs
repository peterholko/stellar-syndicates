// User-authorized removal of baked-in checkerboards from these five generated
// planet sprites. The source RGB is preserved; only the outside alpha changes.
// Not a general color key: the dark/moon-gray surface is protected by the globe's
// outline, so gray clouds, bright ice and shadowed terrain cannot become holes.
import assert from "node:assert/strict";
import { constants } from "node:fs";
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import sharp from "sharp";

const root = new URL("../art-src/planets-2026-09-08/", import.meta.url);
const originals = new URL("originals/", root);
const kinds = ["desert", "ocean", "gas_giant", "barren", "moon"];
await mkdir(originals, { recursive: true });

for (const kind of kinds) {
  const source = new URL(`${kind}.png`, originals);
  const output = new URL(`${kind}.png`, root);
  // Keep the exact generated file; repeat runs always use it, never an already
  // feathered cutout. Refuse to overwrite an existing preserved original.
  await copyFile(output, source, constants.COPYFILE_EXCL).catch(error => {
    if (error.code !== "EEXIST") throw error;
  });
  const bytes = await readFile(source);
  const meta = await sharp(bytes).metadata();
  assert.equal(meta.hasAlpha, false, `${kind}: expected the original opaque source`);
  assert.equal(meta.width, 1254);
  assert.equal(meta.height, 1254);
  const { data, info } = await sharp(bytes).removeAlpha().raw().toBuffer({ resolveWithObject: true });
  const { width, height } = info;
  const cx = width / 2, cy = height / 2;
  const count = 2048;
  const radii = [];
  for (let a = 0; a < count; a++) {
    const angle = a * Math.PI * 2 / count;
    let edge = null;
    // Inspection of these fixed masters puts every limb between 575 and 600px
    // from center. Search outside-in across that annulus only. The checkerboard
    // is bright neutral gray; the solid limb is colored or has a dark edge.
    for (let r = 618; r >= 550; r -= .5) {
      const x = Math.round(cx + Math.cos(angle) * r);
      const y = Math.round(cy + Math.sin(angle) * r);
      const i = (y * width + x) * 3;
      const hi = Math.max(data[i], data[i + 1], data[i + 2]);
      const lo = Math.min(data[i], data[i + 1], data[i + 2]);
      if (hi - lo > 8 || hi < 160) { edge = r; break; }
    }
    assert.ok(edge !== null && edge > 565 && edge < 610, `${kind}: unexpected limb at angle ${a}`);
    radii.push(edge);
  }
  // A small angular median removes isolated checker-compositing noise while
  // keeping the actual slightly elliptical silhouette, not forcing a new circle.
  const outline = radii.map((_, i) => {
    const near = Array.from({ length: 9 }, (_, j) => radii[(i + j - 4 + count) % count]);
    return near.sort((a, b) => a - b)[4] - .5;
  });
  const rgba = Buffer.alloc(width * height * 4);
  let clear = 0, partial = 0;
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const dx = x - cx, dy = y - cy;
    const angle = (Math.atan2(dy, dx) / (2 * Math.PI) + 1) % 1 * count;
    const index = Math.floor(angle), mix = angle - index;
    const edge = outline[index] * (1 - mix) + outline[(index + 1) % count] * mix;
    // Feather inward by 1.5 native pixels to exclude the precomposited matte.
    const alpha = Math.round(255 * Math.max(0, Math.min(1, (edge - Math.hypot(dx, dy)) / 1.5)));
    const i = y * width + x;
    data.copy(rgba, i * 4, i * 3, i * 3 + 3);
    rgba[i * 4 + 3] = alpha;
    if (!alpha) clear++;
    else if (alpha < 255) partial++;
  }
  assert.ok(clear > width * height * .25 && partial > 0);
  const cleaned = await sharp(rgba, { raw: { width, height, channels: 4 } })
    .png({ compressionLevel: 9, palette: false }).toBuffer();
  // Idempotent: do not replace the cleaned master if its bytes are unchanged.
  if (!(await readFile(output)).equals(cleaned)) {
    await writeFile(output, cleaned);
  }
  console.log(`${kind}: transparent cutout; source RGB retained; ${(100 * clear / (width * height)).toFixed(1)}% clear, ${partial} edge pixels`);
}

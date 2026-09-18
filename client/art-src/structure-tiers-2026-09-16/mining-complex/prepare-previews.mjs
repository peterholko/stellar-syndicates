// Preview-only derivatives. The six generated masters and the live game assets
// are unchanged; no background removal or other artistic processing occurs here.
import assert from "node:assert/strict";
import sharp from "../../../node_modules/sharp/lib/index.js";

for (let tier = 1; tier <= 6; tier++) {
  const source = new URL(`tier-${tier}.png`, import.meta.url);
  const meta = await sharp(source.pathname).metadata();
  const stats = await sharp(source.pathname).stats();
  assert.ok(meta.hasAlpha && stats.channels[3].min === 0, `Tier ${tier}: real transparency`);
  assert.equal(meta.width, 1254);
  assert.equal(meta.height, 1254);
  for (const pixels of [96, 150, 256, 512]) {
    await sharp(source.pathname).resize(pixels, pixels, { kernel: "mitchell", withoutEnlargement: true })
      .png().toFile(new URL(`tier-${tier}-${pixels}.png`, import.meta.url).pathname);
  }
  console.log(`Tier ${tier}: 1254px RGBA master; 96/150/256/512px previews`);
}

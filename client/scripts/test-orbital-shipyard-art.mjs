import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import sharp from "sharp";
import { buildPlanetShipyardArt } from "./build-planet-shipyard-art.mjs";

const client = new URL("../", import.meta.url);
await buildPlanetShipyardArt();
const manifest = JSON.parse(await readFile(new URL("public/art/derived/planet-shipyards/v1/manifest.json", client), "utf8"));
assert.equal(Object.keys(manifest.assets).length, 24);
for (let tier = 1; tier <= 6; tier++) {
  assert.ok(manifest.assets[`naval_drydock/${tier}`], "idle variant remains available");
  assert.ok(manifest.assets[`naval_drydock/${tier}-occupied`], "each tier has an occupied variant");
}
const hashes = new Set();
for (const [id, asset] of Object.entries(manifest.assets)) {
  const bytes = await readFile(new URL(asset.source, client));
  const hash = createHash("sha256").update(bytes).digest("hex");
  assert.equal(hash, asset.sourceSha256);
  assert.ok(!hashes.has(hash), `${id}: distinct upgrade artwork`);
  hashes.add(hash);
  const prompt = JSON.parse(await readFile(new URL(asset.source.replace(".png", ".prompt.json"), client), "utf8"));
  assert.ok(prompt.prompt, `${id}: original generation prompt retained`);
  const nativeAlpha = await sharp(bytes).extractChannel("alpha").stats();
  assert.equal(nativeAlpha.channels[0].min, 0);
  assert.equal(nativeAlpha.channels[0].max, 255);
  for (const level of asset.levels) {
    const image = sharp(await readFile(new URL(`public${level.url}`, client)));
    const meta = await image.metadata();
    assert.equal(meta.width, level.size);
    assert.ok(meta.hasAlpha);
    assert.ok(Math.abs(meta.height / meta.width - asset.height / asset.width) < .01, "native proportions, never flattened");
    const alpha = await image.extractChannel("alpha").stats();
    assert.equal(alpha.channels[0].min, 0, "outside remains fully transparent");
  }
}
assert.equal((await buildPlanetShipyardArt()).written, 0, "unchanged masters reuse all derivatives");
console.log("PASS: 18 idle shipyard tiers plus 6 occupied Orbital Shipyards, 72 alpha-preserving WebPs, prompts, hashes, native aspect ratios and derivative cache.");

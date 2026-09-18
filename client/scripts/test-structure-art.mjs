// Validate the real registry, the authored catalogue and every shipping size.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import vm from "node:vm";
import sharp from "sharp";
import ts from "typescript";

const root = new URL("../", import.meta.url);
const src = await readFile(new URL("src/icons.ts", root), "utf8");
const icons = {};
vm.runInNewContext(ts.transpileModule(src, { compilerOptions: {
  target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
} }).outputText, { exports: icons });
const manifest = JSON.parse(await readFile(new URL("public/art/derived/structures/2026-09-16/manifest.json", root), "utf8"));
const structures = Object.keys(manifest.structures);
assert.equal(structures.length, 25);
const catalog = await readFile(new URL("src/shell/deck/empire.ts", root), "utf8");
const descriptions = catalog.slice(catalog.indexOf("const STRUCTURE_DESCRIPTION"), catalog.indexOf("import { MODULES"));
assert.deepEqual(structures.sort(), [...descriptions.matchAll(/^  (\w+): "/gm)].map(match => match[1]).sort(), "all current structures have tiered art");
let bytes = 0;
for (const key of structures) {
  const variants = manifest.structures[key];
  assert.equal(Object.keys(variants).length, 6);
  assert.equal(new Set(Object.values(variants).map(variant => variant.sourceSha256)).size, 6, `${key}: genuinely distinct tier masters`);
  assert.notEqual(icons.structureIcon(key), "build");
  for (let tier = 1; tier <= 6; tier++) {
    const html = icons.structureImage(key, tier, "lg", 'Inspect "structure"', "fixture", "150px");
    assert.match(html, /class="icon icon--lg fixture"/);
    assert.match(html, /title="Inspect &quot;structure&quot;"/);
    assert.match(html, /sizes="150px"/);
    for (const level of variants[tier].levels) {
      assert.ok(html.includes(`${level.url} ${level.size}w`), `${key} ${tier}: actual URL in srcset`);
      const file = await readFile(new URL(`public${level.url}`, root));
      bytes += file.length;
      assert.equal(file.length, level.bytes);
      const meta = await sharp(file).metadata();
      assert.equal(meta.width, level.size);
      assert.equal(meta.height, level.size);
      assert.equal(meta.hasAlpha, true);
      const alpha = await sharp(file).extractChannel("alpha").stats();
      assert.equal(alpha.channels[0].min, 0);
      assert.equal(alpha.channels[0].max, 255);
      assert.ok(alpha.channels[0].mean < 240, `${key} ${tier}: real transparency remains at ${level.size}px`);
    }
  }
  for (const tier of [undefined, NaN, -1, 0]) assert.ok(icons.structureImage(key, tier).includes("/tier-1-128.webp"));
  assert.ok(icons.structureImage(key, 100).includes("/tier-6-128.webp"));
  assert.ok(icons.structureImage(key, 2.9).includes("/tier-2-128.webp"));
  assert.ok(icons.icon(icons.structureIcon(key)).includes("/tier-1-128.webp"), "generic structure symbols use tier I");
}
assert.equal(icons.structureImage("unknown_future_structure", 3), icons.icon("build"), "unknown kinds retain the safe build fallback");
console.log(`structure art: 25 structures × 6 tiers × 3 sizes; ${(bytes / 1024 / 1024).toFixed(1)} MiB total, transparency and registry OK`);

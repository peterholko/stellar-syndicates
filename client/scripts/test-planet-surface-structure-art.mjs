// Shipping assets and the real scene selector. No live game or simulation.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import vm from "node:vm";
import sharp from "sharp";
import ts from "typescript";
import { buildPlanetSurfaceStructureArt, FRAMING } from "./build-planet-surface-structure-art.mjs";

const client = new URL("../", import.meta.url);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const source = name => readFile(new URL(`src/${name}`, client), "utf8");
const deps = {};
async function compile(name) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(await source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: key => deps[key] ?? {} });
  return exports;
}

await buildPlanetSurfaceStructureArt();
deps["./icons"] = await compile("icons.ts");
deps["./planet-surface-structure-art.generated"] = await compile("planet-surface-structure-art.generated.ts");
const { planetStructureImage, isOrbitalYard } = await compile("planet-structure-art.ts");
const { SURFACE_STRUCTURE_TIERS: families, SURFACE_GROUND_LAYERS: biomes } = deps["./planet-surface-structure-art.generated"];
const manifestFile = new URL("public/art/derived/planet-surface-structures/v2/manifest.json", client);
const manifest = JSON.parse(await readFile(manifestFile, "utf8"));
assert.equal(Object.keys(manifest.assets).length, 155, "142 ground buildings + 13 shared biomes; shipyards already have their own pipeline");
const empire = await source("shell/deck/empire.ts");
const descriptions = empire.slice(empire.indexOf("const STRUCTURE_DESCRIPTION"), empire.indexOf("import { MODULES"));
const catalogue = [...descriptions.matchAll(/^  (\w+): "/gm)].map(match => match[1]);
const allFamilies = { ...families, shipyard: 6, naval_drydock: 6, capital_slipway: 6 };
// §power: the Power Plant draws the Fuel Refinery's scene art until its own lands;
// §fuel-overhaul: so do the Uranium Mill and the Antimatter Plant.
const standIns = ["power_plant", "uranium_mill", "antimatter_plant"];
assert.deepEqual(Object.keys(allFamilies).sort(), catalogue.filter(key => !standIns.includes(key)).sort(), "every live structure kind has dedicated scene art");
for (const key of standIns) assert.equal(planetStructureImage(key, 2, "64px", false, "temperate-basin"), planetStructureImage("fuel_refinery", 2, "64px", false, "temperate-basin"));
// §gaia: the terraces draw their own six tiers; §marginal worlds have their own ground.
assert.match(planetStructureImage("living_terraces", 6, "64px", false, "temperate-basin"), /\/buildings\/living_terraces\/tier-6-256\.webp/);
assert.match(planetStructureImage("habitat", 1, "64px", false, "marginal"), /\/grounds\/marginal-256\.webp/, "buildings on Marginal worlds stand on Marginal ground");
// Agroplex IV–VI ship the Oct 2026 redo under their old ids; the originals stay as provenance.
for (const tier of [4, 5, 6]) {
  const masters = "art-src/planet-surface-structures-v2/buildings/agroplex";
  assert.equal(manifest.assets[`buildings/agroplex/tier-${tier}`].source, `${masters}/tier-${tier}-v2.png`);
  assert.ok((await stat(new URL(`${masters}/tier-${tier}.png`, client))).size > 0, `original Agroplex ${tier} kept`);
}
assert.equal(Object.values(allFamilies).reduce((sum, count) => sum + count, 0), 160);
const { HABITABLE_SURFACES } = await compile("planet-surfaces.ts");
assert.deepEqual([...biomes].sort(), [...HABITABLE_SURFACES, "terrestrial", "desert", "ocean", "ice", "lava", "barren", "marginal"].sort(), "every painted surface has its ground");

let encodedBytes = 0;
const hashes = new Set();
for (const [id, asset] of Object.entries(manifest.assets)) {
  assert.ok(asset.source.startsWith("art-src/"), "production never depends on output/ or a generated-images temp directory");
  const original = await readFile(new URL(asset.source, client));
  assert.equal(hash(original), asset.sourceSha256);
  assert.ok(!hashes.has(asset.sourceSha256), `${id}: no duplicate tiers/grounds`);
  hashes.add(asset.sourceSha256);
  const prompt = JSON.parse(await readFile(new URL(asset.source.replace(".png", ".prompt.json"), client), "utf8"));
  assert.ok(prompt.prompt);
  assert.deepEqual(asset.levels.map(level => level.size), [128, 256, 512]);
  for (const level of asset.levels) {
    const bytes = await readFile(new URL(`public${level.url}`, client));
    assert.equal(bytes.length, level.bytes);
    assert.equal(hash(bytes), level.sha256);
    const image = sharp(bytes), metadata = await image.metadata();
    assert.equal(metadata.width, level.size);
    assert.equal(metadata.height, level.size, "native square framing, no tier-dependent trim");
    assert.ok(metadata.hasAlpha);
    const alpha = (await image.extractChannel("alpha").stats()).channels[0];
    assert.equal(alpha.min, 0, "true transparent surroundings");
    assert.ok(alpha.max >= 250 && alpha.mean < 230, "not a flattened background");
    encodedBytes += bytes.length;
  }
}

// §gaia/§marginal: the Oct 2026 additions are enlarged to their neighbours' scale;
// the approved set keeps its native framing.
async function bounds(id, min) {
  const url = manifest.assets[id].levels.find(level => level.size === 512).url;
  const { data, info } = await sharp(await readFile(new URL(`public${url}`, client))).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let left = 1, top = 1, right = 0, bottom = 0;
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    if (data[(y * info.width + x) * 4 + 3] <= min) continue;
    left = Math.min(left, x / info.width); right = Math.max(right, x / info.width);
    top = Math.min(top, y / info.height); bottom = Math.max(bottom, y / info.height);
  }
  return { left, top, right, bottom };
}
assert.deepEqual(manifest.assets["buildings/living_terraces/tier-6"].framing, FRAMING["buildings/living_terraces"]);
assert.deepEqual(manifest.assets["grounds/marginal"].framing, FRAMING["grounds/marginal"]);
assert.equal(manifest.assets["grounds/desert"].framing, undefined);
const marginal = await bounds("grounds/marginal", 32), desert = await bounds("grounds/desert", 32);
for (const side of ["left", "top", "right", "bottom"]) {
  assert.ok(Math.abs(marginal[side] - desert[side]) < .04, `Marginal ground covers the same patch as the others (${side})`);
}
for (let tier = 1; tier <= 6; tier++) {
  const terraces = await bounds(`buildings/living_terraces/tier-${tier}`, 200);
  assert.ok(terraces.right - terraces.left > .78, `terraces ${tier}: near their neighbours' width`);
  assert.ok(Math.abs(terraces.bottom - .89) < .015, `terraces ${tier}: front foot where the v2 set stands`);
  assert.ok(terraces.top > .02, `terraces ${tier}: the roofline stays inside the canvas`);
}

for (const [family, maxTier] of Object.entries(allFamilies)) {
  for (let tier = 1; tier <= maxTier; tier++) {
    const oldIcon = deps["./icons"].structureImage(family, tier);
    for (const biome of [...biomes, null]) {
      const html = planetStructureImage(family, tier, '100px" onerror="bad', true, biome);
      assert.match(html, /sizes="100px&quot; onerror=&quot;bad"/, "attributes remain escaped");
      assert.doesNotMatch(html, /src="[^\"]*\.png|\/structures\/2026|command-center\/v1/, "no masters or old menu art in scene");
      const terrain = biome && !isOrbitalYard(family) && family !== "service_berth";
      assert.equal(html.includes('class="planet-structure-ground"'), Boolean(terrain));
      if (terrain) assert.ok(html.includes(`/grounds/${biome}-256.webp`));
      const variant = family === "naval_drydock" ? "-occupied" : "";
      assert.ok(html.includes(`/${family}/tier-${tier}${variant}-256.webp`));
      for (const url of new Set([...html.matchAll(/(\/art\/[^\s\"]+\.webp)/g)].map(match => match[1]))) {
        assert.ok((await stat(new URL(`public${url}`, client))).size > 0, `${family}/${tier}/${biome}: served derivative exists`);
      }
    }
    assert.equal(deps["./icons"].structureImage(family, tier), oldIcon, "scene selection never mutates menu/inventory icons");
  }
  for (const invalid of [NaN, undefined, -5, 0, Infinity]) assert.match(planetStructureImage(family, invalid, "80px"), /tier-1-/);
  assert.ok(planetStructureImage(family, 999, "80px").includes(`/tier-${maxTier}-`));
  assert.ok(planetStructureImage(family, 2.9, "80px").includes(`/tier-${Math.min(2, maxTier)}-`));
}
assert.doesNotMatch(planetStructureImage("warehouse", 1, "80px", false, "../../invalid"), /planet-structure-ground/);
assert.equal(planetStructureImage("unknown_future_structure", 1, "80px"),
  deps["./icons"].structureImage("unknown_future_structure", 1, "lg", undefined, "", "80px"));
assert.ok(!deps["./icons"].structureImage("warehouse", 1).includes("planet-surface-structures"), "menus stay on the original icon collection");
const before = (await stat(manifestFile)).mtimeMs;
assert.equal((await buildPlanetSurfaceStructureArt()).written, 0, "no re-encoding unchanged masters");
assert.equal((await stat(manifestFile)).mtimeMs, before, "no manifest churn");
console.log(`PASS: all 28 families / 160 tiers, 13 biomes, special tier caps, orbital exclusions, 465 native-alpha derivatives (${(encodedBytes / 1048576).toFixed(1)} MiB), provenance, existing icons and cache.`);

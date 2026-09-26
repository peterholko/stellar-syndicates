// Approved habitable panoramas: resolution, deterministic selection and no
// gameplay coupling. No live galaxy, account or network connection needed.
import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import vm from "node:vm";
import ts from "typescript";
import sharp from "sharp";
import { buildPlanetWorkbenchArt, HABITABLE_SURFACES, HABITABLE_SURFACE_WIDTHS } from "./build-planet-workbench-art.mjs";

const root = new URL("../", import.meta.url);
async function load(name, deps = {}) {
  const exports = {};
  const source = await readFile(new URL(`src/${name}`, root), "utf8");
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] });
  return exports;
}
const surfaces = await load("planet-surfaces.ts", { "./prng": await load("prng.ts") });
const { planetSurface, planetSurfaceArt, FOUNDING_SURFACES } = surfaces;
assert.deepEqual(Array.from(surfaces.HABITABLE_SURFACES), HABITABLE_SURFACES);
const body = { id: 3, kind: "terrestrial", habitable: true };
const original = structuredClone(body);
const globe = "/art/derived/planets/2026-09-08/terrestrial-512.png";
const chosen = planetSurface(globe, "home", body);
assert.ok(HABITABLE_SURFACES.includes(chosen));
assert.equal(planetSurface(globe, "home", { ...body, name: "Renamed", structures: { habitat: 5 }, population: 1,
  geology: "ultra_rich", deposits: [{ resource: "rare_elements", richness: 2 }], special: "volcanic_mantle" }), chosen,
  "private resources, received economy and rename cannot change scenery or disclose hidden features");
assert.deepEqual(body, original, "selecting art is read-only");
assert.equal(planetSurface(globe, "home", { ...body, kind: "ocean" }), "archipelago-world");
assert.equal(planetSurface(globe, "home", { ...body, kind: "gas_giant" }), null);
for (const [profile, surface] of Object.entries(FOUNDING_SURFACES)) {
  for (const completed of [[], ["prospect", "quarters", "storehouse", "surface_workings"]]) {
    assert.equal(planetSurface(globe, "home", { ...body, founding_camp: { profile, completed } }), surface);
  }
}
const seen = new Set([planetSurface(globe, "home", { ...body, kind: "ocean" })]);
for (let id = 0; id < 100; id++) seen.add(planetSurface(globe, "home", { ...body, id }));
assert.deepEqual([...seen].sort(), [...HABITABLE_SURFACES].sort(), "all six paintings are reachable on legacy and new habitable worlds");
for (const surface of ["terrestrial", "barren", "desert", "ice", "lava", "ocean"]) {
  assert.equal(planetSurface(`/art/${surface}-512.png`, "home", { ...body, habitable: false }), surface);
  assert.ok(planetSurfaceArt(surface).srcset.includes(`/v1/${surface}-1536.webp 1536w`));
}
assert.equal(planetSurface("/art/moon-512.png", "home", { ...body, kind: "rocky", habitable: false }), "barren");
assert.equal(planetSurface("/art/gas_giant-512.png"), null);

let totalBytes = 0;
const outputs = [];
for (const surface of HABITABLE_SURFACES) {
  const sourcePath = new URL(`art-src/planet-view/habitable-v1/${surface}.png`, root);
  const source = await readFile(sourcePath);
  const meta = await sharp(source).metadata();
  assert.equal(meta.width, 1672);
  assert.equal(meta.height, 941);
  const art = planetSurfaceArt(surface);
  for (const width of HABITABLE_SURFACE_WIDTHS) {
    const url = `/art/derived/planet-workbench/habitable-v1/${surface}-${width}.webp`;
    assert.ok(art.srcset.includes(`${url} ${width}w`), "every source descriptor resolves to a real derivative");
    const file = new URL(`public${url}`, root);
    const bytes = await readFile(file);
    const image = await sharp(bytes).metadata();
    assert.equal(image.width, width);
    assert.equal(image.height, Math.round(meta.height * width / meta.width));
    assert.ok(image.width <= meta.width, "never upscale");
    assert.equal(image.format, "webp");
    const expected = await sharp(source).resize({ width, withoutEnlargement: true })
      .webp({ quality: 88, effort: 5 }).toBuffer();
    assert.deepEqual(bytes, expected, `${surface}/${width}: derive directly from the approved master`);
    outputs.push(file); totalBytes += bytes.length;
  }
  assert.equal(art.src, `/art/derived/planet-workbench/habitable-v1/${surface}-768.webp`);
  assert.equal(art.full, `/art/derived/planet-workbench/habitable-v1/${surface}-1672.webp`);
}
const mtimes = await Promise.all(outputs.map(async file => (await stat(file)).mtimeMs));
assert.equal((await buildPlanetWorkbenchArt()).written, 0, "repeat builds leave valid derivatives untouched");
assert.deepEqual(await Promise.all(outputs.map(async file => (await stat(file)).mtimeMs)), mtimes);
console.log(`Planet surfaces: 6 approved masters, 18 WebP variants (${(totalBytes / 1024 / 1024).toFixed(2)} MiB total), stable habitat selection and unchanged nonhabitable art passed.`);

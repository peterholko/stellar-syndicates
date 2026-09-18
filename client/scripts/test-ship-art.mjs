import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import vm from "node:vm";
import sharp from "sharp";
import ts from "typescript";
import { hullGeometry, buildShipDerivatives } from "./build-ship-derivatives.mjs";

const client = new URL("../", import.meta.url);
const source = file => readFile(new URL(`src/${file}`, client), "utf8");
function compile(text, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText, { exports, ...globals });
  return exports;
}
const generated = compile(await source("ship-art.generated.ts"));
const art = compile(await source("shipart.ts"), { require: () => generated });
const close = (actual, expected, message) => assert.ok(Math.abs(actual - expected) < 1e-9,
  `${message}: ${actual} != ${expected}`);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const manifest = JSON.parse(await readFile(new URL("public/art/derived/ships/2026-09-08/manifest.json", client), "utf8"));
const kinds = ["raider", "convoy", "scout", "corvette", "colony", "builder", "transport",
  "destroyer", "cruiser", "battleship", "dreadnought", "titan", "freighter",
  "tiny_freighter", "small_freighter", "large_freighter", "heavy_freighter", "bulk_freighter"];
assert.deepEqual(Object.keys(generated.SHIP_ART).sort(), [...kinds].sort());
let checked = 0;
for (const kind of kinds) {
  const hull = manifest.ships[kind];
  const master = await readFile(new URL(hull.source, client));
  assert.equal(hash(master), hull.sourceSha256, `${kind}: provenance`);
  const geometry = await hullGeometry(master);
  close(hull.lengthRatio, geometry.lengthRatio, `${kind}: visible hull measured from master`);
  close(hull.calib * hull.lengthRatio, hull.referenceRatio, `${kind}: padding cancels`);
  for (const level of hull.levels) {
    const bytes = await readFile(new URL(`public${level.url.split("?")[0]}`, client));
    const meta = await sharp(bytes).metadata();
    assert.equal(meta.width, level.size); assert.equal(meta.height, level.size);
    assert.ok(meta.hasAlpha, `${kind}/${level.size}: real transparent background`);
    assert.ok(level.size <= 1254, "No artificial enlargement");
    assert.ok(level.url.endsWith(hash(bytes).slice(0, 12)), "Content-versioned runtime URL");
    if (level.size === 1254) assert.equal(hash(bytes), hash(master), `${kind}: native tier is byte-identical`);
    const actual = await hullGeometry(bytes);
    // The Battleship's thin nose antenna loses ~3px at 128px. Calibration stays
    // keyed to the master, so this minification detail never changes its size.
    assert.ok(Math.abs(actual.lengthRatio - geometry.lengthRatio) * level.size <= 3,
      `${kind}/${level.size}: resampling must not erase hull length`);
    for (let axis = 0; axis < 2; axis++) assert.ok(
      Math.abs(actual.anchor[axis] - geometry.anchor[axis]) * level.size <= 1.5,
      `${kind}/${level.size}: fixed master anchor, subpixel alpha rounding only`);
    checked++;
  }
  for (const resolution of [1, 2]) for (const pixels of [32, 76, 128, 256, 512, 768, 1400]) {
    const requested = pixels * hull.calib * resolution;
    const expected = hull.levels.find(level => level.size >= requested) ?? hull.levels.at(-1);
    assert.equal(art.shipArtUrl(kind, pixels, resolution), expected.url, `${kind}: smallest adequate native tier at ${resolution}×`);
  }
  for (const size of [128, 256]) {
    const file = new URL(`public/art/derived/ships/2026-09-08/panels/${kind}-${size}.png`, client);
    const meta = await sharp(await readFile(file)).metadata();
    assert.ok(meta.hasAlpha && meta.width === size && meta.height === size);
  }
}
assert.equal(Object.keys(generated.SHIP_FORMATIONS).length, 12);
for (const url of Object.values(generated.SHIP_FORMATIONS)) {
  const bytes = await readFile(new URL(`public${url.split("?")[0]}`, client));
  const meta = await sharp(bytes).metadata();
  assert.ok(meta.hasAlpha && meta.width === 512 && meta.height === 512);
  assert.ok(url.endsWith(hash(bytes).slice(0, 12)));
}

// Exercise the actual battle presentation, not a second copy of its formula.
const theaterText = await source("battletheater.ts");
const ast = ts.createSourceFile("battletheater.ts", theaterText, ts.ScriptTarget.Latest, true);
const constants = ["MASS", "KIND_LABEL", "PIRATE_RAIDER_ART", "pirateRaiderArt"];
const declarations = ast.statements.filter(ts.isVariableStatement).flatMap(n => [...n.declarationList.declarations])
  .filter(n => constants.includes(n.name.getText(ast)));
const functions = ast.statements.filter(n => ts.isFunctionDeclaration(n)
  && ["spritePx", "theaterShipAppearance"].includes(n.name?.text));
const { hashId } = compile(await source("prng.ts"));
const theater = compile(`${declarations.map(n => `const ${n.getText(ast)};`).join("\n")}
  ${functions.map(n => n.getText(ast).replace(/^export /, "")).join("\n")}
  export { spritePx, theaterShipAppearance };`, { ...art, hashId });
const battle = { id: "hull-art-proof", sides: [{ corp: "player" }, { corp: "pirate" }] };
const ratios = { raider: 1, corvette: 1.5, destroyer: 2.5, cruiser: 4, battleship: 5, dreadnought: 6, titan: 8 };
function checkBattleScale(sizeFor) {
  for (const [kind, ratio] of Object.entries(ratios)) {
    const appearance = theater.theaterShipAppearance(battle, 0, kind, "pirate");
    assert.match(appearance.url, /\/derived\/ships\/2026-09-08\//);
    const length = sizeFor(kind) * appearance.calib * art.shipArtwork(kind).lengthRatio;
    const reference = sizeFor("raider") * art.shipArtwork("raider").referenceRatio;
    close(length / reference, ratio, `${kind}: battle hierarchy`);
    assert.deepEqual(appearance.anchor, art.shipArtwork(kind).anchor);
  }
}
checkBattleScale(theater.spritePx);
// Teeth: the former capped mass curve cannot meet the approved length ladder.
assert.throws(() => checkBattleScale(kind => kind === "raider" ? 10 : 64), /battle hierarchy/);
const privateer = theater.theaterShipAppearance(battle, 1, "raider", "pirate");
assert.equal(privateer.label, "Privateer");
assert.match(privateer.url, /privateer_raider_ship|pirate_corsair|pirate_boarding_raider/);
assert.ok([.73, .70].includes(privateer.calib), "Pirate size/art stays independent");
for (const kind of ["convoy", "freighter", "colony", "builder", "transport", "scout"]) {
  const masses = { convoy: 4500, freighter: 6000, colony: 6000, builder: 2500, transport: 7000, scout: 80 };
  close(theater.spritePx(kind), Math.max(10, Math.min(64, 7 * (masses[kind] / 100) ** .4)), `${kind}: established battle sizing`);
}
const icons = compile(await source("icons.ts"));
for (const kind of kinds) {
  const html = icons.icon(kind === "freighter" ? "authorityFreighter" : kind, "md");
  assert.ok(html.includes(`/panels/${kind}-128.png`) && html.includes(`/panels/${kind}-256.png 2x`));
}
const renderer = await source("render.ts");
assert.ok(renderer.includes("sp.sprite.anchor.set(marker.anchor?.[0]"), "Map uses measured anchor");
assert.ok(theaterText.includes("v.sprite.anchor.set(v.plat ? 0.5 : appearance.anchor?.[0]"), "Battle uses measured anchor");
for (const text of [renderer, theaterText]) {
  assert.ok(text.includes("source.autoGenerateMipmaps = true"), "Smooth minification");
  assert.ok(text.includes("source.scaleMode = \"linear\""), "Trilinear filtering");
  assert.doesNotMatch(text, /ship_sprites\/(raider_attack_ship|corporate_freighter|titan_flagship)\.png/,
    "No stale single-hull art path");
}
assert.equal((await buildShipDerivatives()).written, 0, "Rebuilding unchanged masters is idempotent");
console.log(`Ship artwork: ${kinds.length} masters, ${checked} native/downsampled tiers, ${kinds.length * 2} panel images and 12 formations passed; map/battle geometry, DPI, provenance, unchanged existing civilian/NPC sizes and rollback teeth passed.`);

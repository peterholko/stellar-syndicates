// Exercise the real catalog, async texture cache, map draw expressions and
// System View transforms without a server or GPU. No alternate sizing model.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
function compile(text, deps = {}, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => deps[id] ?? {}, ...globals });
  return exports;
}
const plain = value => JSON.parse(JSON.stringify(value));
const close = (actual, expected, label) => assert.ok(Math.abs(actual - expected) < 1e-8, `${label}: ${actual} != ${expected}`);
const catalog = compile(source("star-art.generated.ts"));
const artSource = source("starart.ts");
const art = compile(artSource, { "./star-art.generated": catalog });
const cacheModule = compile(source("startextures.ts"), { "./starart": art });
const prng = compile(source("prng.ts"));
const stars = compile(source("stars.ts"), { "./prng": prng });
const texture = size => ({ width: size, height: size, source: { autoGenerateMipmaps: false, scaleMode: "nearest" } });

function checkLevels(selector) {
  for (const family of ["galaxy", "system"]) for (const star of stars.STAR_TYPES) {
    const definition = art.starArtwork(family, star.slug);
    for (const resolution of [1, 1.25, 1.5, 2]) for (const diameter of [8, 22, 32, 44, 77, 154, 300, 600]) {
      const required = diameter / definition.visualRatio * resolution;
      const level = selector(definition, diameter, resolution);
      const expected = definition.levels.find(l => l.size >= required) ?? definition.levels.at(-1);
      assert.equal(level.url, expected.url, "physical pixels including transparent padding and DPR");
    }
    for (const level of definition.levels) {
      const boundary = level.size * definition.visualRatio / 2;
      assert.equal(selector(definition, boundary - 1e-7, 2).size, level.size, "tier boundary, no premature upscaling");
      assert.ok(selector(definition, boundary + 1e-7, 2).size >= level.size);
    }
    assert.equal(art.starArtSrcset(family, star.slug).split(", ").length, 7);
    assert.match(art.starArtUrl(family, star.slug), new RegExp(`/${family}/${star.slug}-256\\.png\\?v=`));
  }
}
checkLevels(art.starArtLevel);
assert.throws(() => checkLevels(compile(artSource.replace("* Math.max(1, renderResolution)", "* 1"),
  { "./star-art.generated": catalog }).starArtLevel), /physical pixels/);
assert.throws(() => checkLevels(compile(artSource.replace("/ art.visualRatio", "/ 1"),
  { "./star-art.generated": catalog }).starArtLevel), /physical pixels/);

const requests = new Map(); let changes = 0;
const cache = new cacheModule.StarTextureCache(() => changes++, url => new Promise((resolve, reject) => {
  assert.ok(!requests.has(url), "in-flight fetch is deduplicated");
  requests.set(url, { resolve, reject });
}));
const yellow = art.starArtwork("galaxy", "yellow_star");
const pendingLow = cache.ensure("galaxy", "yellow_star", 20, 1);
assert.equal(cache.ensure("galaxy", "yellow_star", 20, 1), pendingLow);
const pendingHigh = cache.ensure("galaxy", "yellow_star", 230, 2);
const high = art.starArtLevel(yellow, 230, 2);
const highTexture = texture(high.size);
requests.get(high.url).resolve(highTexture);
await pendingHigh;
assert.equal(cache.get("galaxy", "yellow_star", 20, 1).texture, highTexture);
assert.equal(highTexture.source.autoGenerateMipmaps, true, "minification has mipmaps");
assert.equal(highTexture.source.scaleMode, "linear", "trilinear min/mag/mipmap filtering");
const low = art.starArtLevel(yellow, 20, 1);
requests.get(low.url).resolve(texture(low.size)); await pendingLow;
assert.equal(changes, 1, "late low-res completion cannot replace the sharper texture");
const requestCount = requests.size;
for (let px = 8; px < 230; px++) cache.get("galaxy", "yellow_star", px, 2);
assert.equal(requests.size, requestCount, "wheel movement uses resident mips, no network toggling");
const systemPending = cache.ensure("system", "yellow_star", 30, 2);
const systemDefinition = art.starArtwork("system", "yellow_star");
const systemLevel = art.starArtLevel(systemDefinition, 30, 2);
requests.get(systemLevel.url).resolve(texture(systemLevel.size)); await systemPending;
assert.notEqual(cache.get("system", "yellow_star", 30, 2).texture, highTexture, "art families cannot cross-contaminate");
const failed = cache.ensure("galaxy", "red_dwarf", 30, 2);
const failedLevel = art.starArtLevel(art.starArtwork("galaxy", "red_dwarf"), 30, 2);
requests.get(failedLevel.url).reject(new Error("missing test asset")); await failed;
assert.equal(cache.get("galaxy", "red_dwarf", 30, 2), null, "missing asset leaves the map's primitive fallback");

const point = () => ({ x: 0, y: 0, set(x, y = x) { this.x = x; this.y = y; } });
class Container {
  children = []; position = point(); scale = point(); pivot = point();
  visible = true; alpha = 1;
  addChild(...children) { for (const child of children) { child.parent = this; this.children.push(child); } return children[0]; }
  removeChildren() { const all = this.children; this.children = []; return all; }
  destroy() { this.destroyed = true; if (this.parent) this.parent.children = this.parent.children.filter(c => c !== this); }
}
class Graphics extends Container { clear() { return this; } }
for (const name of ["moveTo", "lineTo", "closePath", "fill", "stroke", "circle", "rect", "poly", "ellipse", "arc", "roundRect"]) {
  Graphics.prototype[name] = function () { return this; };
}
class Sprite extends Container { constructor(texture) { super(); this.texture = texture; this.anchor = point(); } }
class Text extends Container { constructor({ text, style }) { super(); this.text = text; this.style = style; this.anchor = point(); } }
const pixi = { Container, Graphics, Sprite, Text, Texture: { EMPTY: {} },
  TextStyle: class { constructor(style) { Object.assign(this, style); } }, Assets: { load: () => new Promise(() => {}) } };
const systems = compile(source("systemview.ts"), { "./stars": stars, "./prng": prng, "pixi.js": pixi });

const renderSource = source("render.ts");
const ast = ts.createSourceFile("render.ts", renderSource, ts.ScriptTarget.Latest, true);
const owner = ast.statements.find(n => ts.isClassDeclaration(n) && n.name?.text === "Renderer");
const method = name => owner.members.find(n => n.name?.getText(ast) === name);
const constantNames = ["BAND_SIZE", "BODY_ZOOM_START", "ZOOM_MAX_FACTOR", "SHIP_NATIVE_ZOOM_START", "MACHINE_ZOOM_END", "BODY_HIT_CAP_PX"];
const constants = ast.statements.filter(ts.isVariableStatement).flatMap(n => [...n.declarationList.declarations])
  .filter(n => constantNames.includes(n.name.getText(ast)));
assert.equal(constants.length, constantNames.length);
let loadedNode, drawNode;
const visit = node => {
  if (ts.isBlock(node)) {
    const index = node.statements.findIndex(n => ts.isVariableStatement(n)
      && n.declarationList.declarations.some(d => d.name.getText(ast) === "loaded"));
    if (index >= 0) [loadedNode, drawNode] = [node.statements[index], node.statements[index + 1]];
  }
  ts.forEachChild(node, visit);
};
visit(method("drawSystems"));
assert.ok(loadedNode && ts.isIfStatement(drawNode));
const { Renderer } = compile(`${constants.map(n => `const ${n.getText(ast)};`).join("\n")}
 export class Renderer {
 ${["bodyFor", "starDiameters", "deepZoomPx", "systemHitRadius", "prepareSystemScene", "refreshSystemStar", "warmStarTextures"].map(n => method(n).getText(ast)).join("\n")}
 fitScale() { return 1; }
 drawStar(st, sys, s, rendered) { const owner = "mine"; ${loadedNode.getText(ast)} ${drawNode.getText(ast)} }
 }`, {}, { Sprite, starTypeFor: stars.starTypeFor, STAR_TYPES: stars.STAR_TYPES,
  starArtwork: art.starArtwork, buildVisualSystem: systems.buildVisualSystem });
const ids = new Map();
for (let i = 0; ids.size < 10 && i < 10000; i++) ids.set(stars.starTypeFor(String(i)).slug, String(i));
assert.equal(ids.size, 10);
let cases = 0;
for (const [width, height, resolution] of [[390, 844, 2], [1366, 768, 1], [1920, 1080, 2], [3840, 2160, 2]]) {
  for (const st of stars.STAR_TYPES) {
    const scene = new systems.SystemViewScene(); scene.layout(width, height);
    const renderer = new Renderer();
    renderer.systemScene = scene;
    renderer.app = { renderer: { resolution } };
    renderer.viewW = width; renderer.viewH = height;
    renderer.cameraRect = { x: 0, y: 0, w: width, h: height };
    renderer.systemBodies = new Map(); renderer.bodyLayer = new Container();
    renderer.starTextures = { get(family, slug, visible, density) {
      const definition = art.starArtwork(family, slug);
      const level = art.starArtLevel(definition, visible, density);
      return { texture: texture(level.size), size: level.size, art: definition };
    } };
    const sys = { id: ids.get(st.slug), name: "Test", pos: { x: 100, y: 200 }, band: "rich" };
    renderer.prepareSystemScene(sys, []);
    renderer.mode = { type: "system", systemId: sys.id }; renderer.transition = null;
    const systemArt = art.starArtwork("system", st.slug);
    close(scene.starSprite.scale.x * scene.starSprite.texture.width * systemArt.visualRatio * scene.worldRoot.scale.x,
      .17 * .42 * Math.min(width, height), "unchanged System View visible diameter");
    for (const zoom of [.9, 4, 24, 72, 80, 88, 96, 153.6]) {
      renderer.scale = zoom;
      const diameter = renderer.starDiameters(sys).rendered;
      assert.ok(diameter <= scene.starVisibleDiameterPx(), "galaxy never exceeds the system star's existing cap");
      renderer.drawStar(st, sys, { x: 310, y: 270 }, diameter);
      const sprite = renderer.systemBodies.get(sys.id);
      const mapArt = art.starArtwork("galaxy", st.slug);
      close(sprite.texture.width * sprite.scale.x * mapArt.visualRatio, diameter, "map visible footprint independent of source resolution");
      assert.deepEqual(plain(sprite.anchor), { x: mapArt.anchor[0], y: mapArt.anchor[1] });
      assert.equal(sprite.position.x, 310); assert.equal(sprite.position.y, 270);
      assert.ok(sprite.scale.x * resolution <= 1, "no texture upscaling at supported viewport/zoom/DPR");
      cases++;
    }
    // Late upgrade during a handoff keeps selection, planet picks and the star
    // anchor. The content/counter-scale pair still cancel to exactly one.
    scene.selected = { sx: 4, sy: 5, r: 6 };
    const selection = scene.selected, targets = scene.bodies;
    scene.content.pivot.set(200, 150); scene.content.position.set(210, 155);
    scene.content.scale.set(.35); scene.setStarCounterScale(.35);
    scene.setStarArt(texture(1254), systemArt);
    assert.equal(scene.selected, selection); assert.equal(scene.bodies, targets);
    close(scene.content.scale.x * scene.starLayer.scale.x, 1, "fixed transition anchor");
    assert.deepEqual(plain(scene.content.pivot), { x: 200, y: 150 });
    scene.resetContentTransform();
    close(scene.starLayer.scale.x, 1, "neutral after finalize");
    renderer.mode = { type: "galaxy" };
    const held = scene.starSprite.texture;
    renderer.refreshSystemStar();
    assert.equal(scene.starSprite.texture, held, "late load cannot revive an exited system");
  }
}
assert.match(source("shell/deck/empire.ts"), /starArtSrcset\("system", star.slug\)/);
assert.doesNotMatch(renderSource, /load\(starIconUrl/);
console.log(`Star rendering: ${cases} viewport/type/zoom cases; DPR-aware tiers, mipmaps, async races/failures, stable anchors/hit targets, and selection-preserving system upgrades passed.`);

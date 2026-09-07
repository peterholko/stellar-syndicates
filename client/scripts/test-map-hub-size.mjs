// Run the actual renderer methods with tiny Pixi stand-ins: protect the hub's
// visible footprint, its independent zoom curve, and panel-insensitive sizing.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import sharp from "sharp";
import ts from "typescript";

const source = readFileSync(new URL("../src/render.ts", import.meta.url), "utf8");
const close = (actual, expected, label) =>
  assert.ok(Math.abs(actual - expected) < 1e-8, `${label}: ${actual} != ${expected}`);

class Sprite {
  constructor(texture) {
    this.texture = texture;
    this.anchor = { set() {} };
    this.position = { set: (x, y) => { this.x = x; this.y = y; } };
    this.scale = { set: value => { this.drawScale = value; } };
  }
  get height() {
    return (this.texture.height ?? this.texture.width) * this.drawScale;
  }
}

function fixture(text = source, overrides = {}) {
  const ast = ts.createSourceFile("render.ts", text, ts.ScriptTarget.Latest, true);
  const constants = ["ZOOM_MIN_FACTOR", "ZOOM_MAX_FACTOR", "INITIAL_HOME_ZOOM_FACTOR",
    "MACHINE_ZOOM_END", "HUB_SIZE_STOPS", "HUB_ART_FILL"];
  const declarations = ast.statements.filter(ts.isVariableStatement)
    .flatMap(n => [...n.declarationList.declarations])
    .filter(n => constants.includes(n.name.getText(ast)));
  assert.equal(declarations.length, constants.length);
  const names = ["viewW", "viewH", "viewportRect", "cameraRect", "setCameraRect",
    "galaxyBounds", "fitScale", "worldToScreen", "drawHubBody", "hubRenderedPx", "hubHitRadius"];
  const owner = ast.statements.find(n => ts.isClassDeclaration(n) && n.name?.text === "Renderer");
  const methods = owner.members.filter(n => names.includes(n.name?.getText(ast)));
  assert.equal(methods.length, names.length);
  const exports = {};
  const fixtureSource = `${declarations.map(n => {
    const name = n.name.getText(ast);
    return `const ${name in overrides ? `${name} = ${JSON.stringify(overrides[name])}` : n.getText(ast)};`;
  }).join("\n")}
    export class Renderer { ${methods.map(n => n.getText(ast)).join("\n")} }`;
  vm.runInNewContext(ts.transpileModule(fixtureSource, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, Sprite });
  const renderer = new exports.Renderer();
  renderer.app = { renderer: { width: 1920, height: 1080, resolution: 1 } };
  renderer.initialized = true;
  renderer.cameraRectOverride = null;
  renderer.systemScene = { layout() {} };
  renderer.bodyLayer = { addChild() {} };
  renderer.hubText = { position: { set: (x, y) => { renderer.labelX = x; renderer.labelY = y; } } };
  renderer.texHub = { width: 1254 };
  renderer.texStation = { width: 256 };
  renderer.cx = renderer.cy = 0;
  renderer.galaxy = { hub: { x: 0, y: 0 }, systems: [
    { pos: { x: -500_000, y: -500_000 } },
    { pos: { x: 500_000, y: 500_000 } },
  ] };
  return renderer;
}

function atZoom(renderer, zoom) {
  renderer.scale = zoom * renderer.fitScale({ x: 0, y: 0, w: renderer.viewW, h: renderer.viewH });
}

const stops = [[.9, 44], [4, 64], [24, 120], [96, 180]];
function checkEndpoints(renderer) {
  for (const [zoom, size] of [...stops, [.1, 44], [153.6, 180]]) {
    atZoom(renderer, zoom);
    close(renderer.hubRenderedPx(), size, `visible size at ${zoom}x`);
    close(renderer.hubHitRadius(), size / 2, `pick radius at ${zoom}x`);
  }
}
checkEndpoints(fixture());

// Window/DPR changes may move the physical fit scale, but the same full-map
// magnification has the same CSS-pixel targets on desktop and mobile.
for (const [width, height, resolution] of [[1920, 1080, 1], [1366, 768, 1], [390, 844, 2]]) {
  const renderer = fixture();
  renderer.app.renderer = { width: width * resolution, height: height * resolution, resolution };
  checkEndpoints(renderer);
}

const renderer = fixture();
let previous = 44;
let maxStep = 0;
for (let zoom = .45; zoom <= 192; zoom *= 1.02) {
  atZoom(renderer, zoom);
  const size = renderer.hubRenderedPx();
  assert.ok(size >= previous - 1e-8 && size >= 44 && size <= 180, `bounded monotone growth at ${zoom}`);
  maxStep = Math.max(maxStep, size - previous);
  previous = size;
}
assert.ok(maxStep < 1.4, "a 2% camera zoom must not balloon the station");
for (const [zoom] of stops) {
  atZoom(renderer, zoom * (1 - 1e-6));
  const before = renderer.hubRenderedPx();
  atZoom(renderer, zoom * (1 + 1e-6));
  assert.ok(Math.abs(renderer.hubRenderedPx() - before) < 1e-6, `smooth join at ${zoom}x`);
}

function checkPanels(renderer) {
  for (const zoom of [.9, 4, 24, 70, 84, 96]) {
    renderer.setCameraRect(null);
    atZoom(renderer, zoom);
    const scale = renderer.scale;
    const size = renderer.hubRenderedPx();
    const fullFit = renderer.fitScale();
    for (const rect of [{ x: 0, y: 50, w: 590, h: 660 }, { x: 0, y: 0, w: 1, h: 1 }, null]) {
      renderer.setCameraRect(rect);
      close(renderer.scale, scale, "panels do not zoom the camera");
      close(renderer.hubRenderedPx(), size, "panels do not resize the hub");
      close(renderer.hubHitRadius(), size / 2, "panels do not resize its pick target");
      // Existing camera/star/ship callers must still fit the available map.
      if (rect) assert.notEqual(renderer.fitScale(), fullFit);
      else close(renderer.fitScale(), fullFit, "default fit restores with the panel closed");
    }
  }
}
checkPanels(fixture());

const asset = source.match(/load\("(\/art\/wormhole[^"]+)"\)/)?.[1];
assert.ok(asset, "measure the hub asset the renderer actually loads");
const { data, info } = await sharp(new URL(`../public${asset}`, import.meta.url).pathname)
  .ensureAlpha().raw().toBuffer({ resolveWithObject: true });
let minX = info.width, maxX = -1, minY = info.height, maxY = -1;
for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
  if (data[(y * info.width + x) * info.channels + info.channels - 1] < 128) continue;
  minX = Math.min(minX, x); maxX = Math.max(maxX, x);
  minY = Math.min(minY, y); maxY = Math.max(maxY, y);
}
assert.ok(maxX >= minX, "hub has visible artwork");
const fill = Math.max(maxX - minX + 1, maxY - minY + 1) / info.width;

function checkDraw(renderer) {
  // Higher-resolution versions of the same silhouette must not enlarge it.
  for (const width of [info.width, 2048, 4096]) {
    renderer.texHub = { width };
    for (const [zoom, size] of stops) {
      atZoom(renderer, zoom);
      renderer.drawHubBody();
      close(renderer.hubSprite.drawScale * width * fill, size, "drawn visible extent matches target");
      close(renderer.hubHitRadius() * 2, size, "pick diameter follows the artwork");
      close(renderer.labelX, renderer.hubSprite.x, "label stays centered on the hub");
      close(renderer.labelY, renderer.hubSprite.y + renderer.hubSprite.height / 2 + 6,
        "label clears the entire sprite at every zoom and texture resolution");
      assert.ok(renderer.hubSprite.drawScale * 2 < 1, "enough native detail even at 2x DPR");
    }
  }
  renderer.texHub = null;
  renderer.drawHubBody();
  close(renderer.hubSprite.drawScale * renderer.texStation.width, 28, "loading fallback unchanged");
  close(Math.max(24, renderer.hubHitRadius()), 24, "fallback retains minimum click target");
  close(renderer.labelY, renderer.hubSprite.y + 14 + 6, "label also clears the loading fallback");
}
checkDraw(fixture());

// Follow zoom/pan and late-loaded art without waiting for a background redraw.
const labelRenderer = fixture();
labelRenderer.cx = 210;
labelRenderer.cy = 135;
atZoom(labelRenderer, 96);
labelRenderer.texHub = null;
labelRenderer.drawHubBody();
close(labelRenderer.labelY, 155, "fallback label after panning");
labelRenderer.texHub = { width: info.width, height: info.height };
labelRenderer.drawHubBody();
close(labelRenderer.labelX, 210, "loaded hub label follows the camera");
assert.ok(labelRenderer.labelY > 230, "late-loaded hub moves the label beyond its lower edge");

// Teeth without touching repository files: undo each underlying fix in memory.
const fullFit = "this.fitScale({ x: 0, y: 0, w: this.viewW, h: this.viewH })";
assert.ok(source.includes(fullFit));
assert.throws(() => checkPanels(fixture(source.replace(fullFit, "this.fitScale()"))), /panels do not resize/);
const interpolation = "return from.px + (to.px - from.px) * s;";
assert.ok(source.includes(interpolation));
const nativeCap = source.replace(interpolation,
  "return from.px + ((to.zoom === ZOOM_MAX_FACTOR ? this.texHub.width : to.px) - from.px) * s;");
assert.throws(() => checkEndpoints(fixture(nativeCap)), /visible size/);
assert.throws(() => checkDraw(fixture(source, { HUB_ART_FILL: .93 })), /drawn visible extent/);
const labelPosition = "h.y + this.hubSprite.height / 2 + 6";
assert.ok(source.includes(labelPosition));
assert.throws(() => checkDraw(fixture(source.replace(labelPosition, "h.y + 13"))), /label clears/);

console.log(`Hub sizing: 44/64/120/180px visible stops; max 2% zoom step ${maxStep.toFixed(2)}px; panel/DPR stability, texture independence, art/pick calibration, and rollback teeth passed.`);

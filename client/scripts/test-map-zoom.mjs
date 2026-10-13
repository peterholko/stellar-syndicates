// Real camera and desktop/mobile gesture methods; no browser or rendering mocks
// stand in for the zoom math, selection priority, or semantic threshold logic.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), 'utf8');
function extract(filename, ownerName, names, constants) {
  const ast = ts.createSourceFile(filename, source(filename), ts.ScriptTarget.Latest, true);
  const owner = ast.statements.find(n => ts.isClassDeclaration(n) && n.name?.text === ownerName);
  const members = names.map(name => {
    const member = owner.members.find(n => n.name?.getText(ast) === name);
    assert.ok(member, `${filename}: ${name}`);
    return member.getText(ast);
  });
  const declarations = ast.statements.filter(ts.isVariableStatement).flatMap(n => [...n.declarationList.declarations])
    .filter(n => constants.includes(n.name.getText(ast)));
  assert.equal(declarations.length, constants.length);
  return { members: members.join('\n'), constants: declarations.map(n => `const ${n.getText(ast)};`).join('\n') };
}
function compile(text) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, performance: { now: () => 100 }, Map, Math });
  return exports;
}
const methods = ['worldToScreen', 'screenToWorld', 'clampScale', 'zoomAt', 'zoomByFactor', 'panBy',
  'zoomFactor', 'viewMode', 'atMaxZoom', 'atSystemZoomThreshold', 'atBattleZoomThreshold', 'fleetUnderPoint',
  'systemEndpointCamera', 'startTransition', 'isSystemScrubbing', 'adjustSystemScrub',
  'beginSystemScrubIn', 'beginSystemScrubOut', 'enterSystemView', 'exitSystemView'];
const extracted = extract('render.ts', 'Renderer', methods,
  ['ZOOM_MIN_FACTOR', 'ZOOM_MAX_FACTOR', 'SYSTEM_ZOOM_FACTOR', 'MACHINE_ZOOM_END', 'BATTLE_ZOOM_FACTOR', 'clamp01']);
const { Camera } = compile(`${extracted.constants}
export class Camera {
  ${extracted.members}
  constructor() {
    this.galaxy = {}; this.scale = .02; this.cx = 150; this.cy = 270;
    this.mode = { type: 'galaxy' }; this.transition = null; this.ghosts = new Map();
    this.cameraRect = { x: 50, y: 80, w: 1100, h: 640 };
    this.galaxyRoot = {}; this.systemScene = { clearSelection() {}, currentId() { return 'star'; } };
  }
  fitScale() { return .02; }
  prepareSystemScene(system) { this.systemFocus = system.pos; }
  dressSystemScene() {}
  systemHitRadius() { return 55; }
  battlePick() { return null; }
}`);
const close = (a, b, label) => assert.ok(Math.abs(a-b) < 1e-7, `${label}: ${a} != ${b}`);
const camera = new Camera();
camera.scale = 96 * camera.fitScale();
const pixel = { x: 630, y: 342 }, world = camera.screenToWorld(pixel.x, pixel.y);
assert.equal(camera.atMaxZoom(), false);
assert.equal(camera.atSystemZoomThreshold(), true);
for (const factor of [2, 2, 10]) {
  camera.zoomAt(pixel.x, pixel.y, factor);
  const shown = camera.worldToScreen(world);
  close(shown.x, pixel.x, 'cursor-anchored zoom x');
  close(shown.y, pixel.y, 'cursor-anchored zoom y');
}
close(camera.zoomFactor(), 384, 'new maximum');
assert.equal(camera.atMaxZoom(), true);
camera.panBy(31, -17);
close(camera.worldToScreen(world).x, pixel.x+31, 'pan x at maximum');
close(camera.worldToScreen(world).y, pixel.y-17, 'pan y at maximum');
const center = { x: 600, y: 400 }, centeredWorld = camera.screenToWorld(center.x, center.y);
camera.zoomByFactor(.5);
close(camera.zoomFactor(), 192, 'zoom-out button');
close(camera.worldToScreen(centeredWorld).x, center.x, 'button anchor x');
close(camera.worldToScreen(centeredWorld).y, center.y, 'button anchor y');
camera.zoomByFactor(1e-9); close(camera.zoomFactor(), .9, 'minimum unchanged');
camera.scale = 14.88 * camera.fitScale(); assert.equal(camera.atBattleZoomThreshold(), true);
assert.equal(camera.atSystemZoomThreshold(), false);

const system = { id: 'star', pos: { x: 4000, y: -3000 } };
for (const zoom of [4, 96, 192, 384]) {
  const r = new Camera(); r.scale = zoom*r.fitScale();
  const before = { cx: r.cx, cy: r.cy, scale: r.scale };
  r.enterSystemView(system, []);
  assert.ok(r.transition.camTo.scale >= before.scale, 'entering a star from inspection never zooms backward');
  close(r.transition.camTo.scale/r.fitScale(), Math.max(96, zoom), 'system approach');
  r.exitSystemView();
  for (const key of ['cx', 'cy', 'scale']) close(r.transition.camTo[key], before[key], `exit restores ${key}`);
}

// Execute both shell gesture handlers with the production camera. A visible
// fleet in front of a star continues to 384×; an exposed star still enters at 96×.
for (const [file, className, gesture, constant] of [
  ['shell/deck/map.ts', 'DeckMapInteraction', 'wheel', 'SYSTEM_SCRUB_STEP'],
  ['shell/mobile/map.ts', 'MobileMapInteraction', 'applyPinch', 'SCRUB_PER_LOG_SCALE'],
]) {
  const shell = extract(file, className, [gesture, 'systemUnderPoint', 'systemById'], [constant]);
  const { Interaction } = compile(`${shell.constants}
    export class Interaction { constructor(ctx) { this.ctx = ctx; this.hooks = {}; } ${shell.members} }`);
  const r = new Camera(); r.scale = 96*r.fitScale();
  const star = { id: 'nearby', pos: r.screenToWorld(500, 300) };
  const marker = { container: { visible: true }, sprite: { visible: true, width: 260 }, shown: star.pos };
  r.ghosts.set('fleet', marker);
  const shellInstance = new Interaction({ renderer: r, state: { galaxy: { systems: [star] }, systems: [], battles: [] } });
  const gestureIn = () => gesture === 'wheel'
    ? shellInstance.wheel({ deltaY: -100, clientX: 500, clientY: 300, preventDefault() {} })
    : shellInstance.applyPinch(1.3, { x: 500, y: 300 });
  gestureIn(); assert.ok(r.zoomFactor() > 96, `${gesture}: fleet close-up crosses the old maximum`);
  assert.equal(r.isSystemScrubbing(), false, `${gesture}: fleet prevents accidental system entry`);
  marker.container.visible = false;
  gestureIn(); assert.equal(r.isSystemScrubbing(), true, `${gesture}: hidden fleet cannot block star entry`);
  assert.ok(r.transitionTargetRaw > 0, `${gesture}: star handoff progresses`);
  close(r.transition.camTo.scale, r.scale, `${gesture}: deep approach does not retreat`);
  r.transition = null; r.scale = 80*r.fitScale();
  assert.equal(r.beginSystemScrubIn(star, []), false, 'system doorway does not start early');
}
console.log('Map zoom: 384× clamp, cursor/button anchors, panning, exact camera restoration, 96× star and unchanged battle thresholds, desktop wheel and mobile pinch fleet priority passed.');

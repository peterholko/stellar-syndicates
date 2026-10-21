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
  } }).outputText, { exports, performance: { now: () => 100 }, Map, Math, ORBIT_TILT: 0.6 });
  return exports;
}
const methods = ['worldToScreen', 'screenToWorld', 'clampScale', 'zoomAt', 'zoomByFactor', 'panBy',
  'zoomFactor', 'viewMode', 'atMaxZoom', 'atSystemZoomThreshold', 'atBattleZoomThreshold', 'fleetUnderPoint',
  'systemEndpointCamera', 'startTransition', 'isSystemScrubbing', 'adjustSystemScrub',
  'beginSystemScrubIn', 'beginSystemScrubOut', 'enterSystemView', 'exitSystemView',
  'approachGrowth', 'handoffGrowFrom', 'handoffCamFrom', 'approachSystemAt'];
const extracted = extract('render.ts', 'Renderer', methods,
  ['ZOOM_MIN_FACTOR', 'ZOOM_MAX_FACTOR', 'SYSTEM_ZOOM_FACTOR', 'MACHINE_ZOOM_END', 'BATTLE_ZOOM_FACTOR', 'clamp01',
    'SCHEMATIC_GROW_FROM', 'APPROACH_HINT_START', 'APPROACH_REACH_MIN', 'APPROACH_PICK_SLACK_PX']);
const { Camera } = compile(`${extracted.constants}
export class Camera {
  ${extracted.members}
  constructor() {
    this.galaxy = {}; this.scale = .02; this.cx = 150; this.cy = 270;
    this.mode = { type: 'galaxy' }; this.transition = null; this.ghosts = new Map();
    this.cameraRect = { x: 50, y: 80, w: 1100, h: 640 };
    this.galaxyRoot = {}; this.approachVisuals = new Map();
    this.systemScene = { clearSelection() {}, currentId() { return 'star'; }, resetZoom() {}, restScale: () => 300,
      starLayoutPosition: () => ({ x: 600, y: 400 }) };
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
  close(r.transition.growFrom, Math.min(1, .35 * Math.max(96, zoom) / 96), 'the schematic starts where the preview stands');
  r.exitSystemView();
  for (const key of ['cx', 'cy', 'scale']) close(r.transition.camTo[key], before[key], `exit restores ${key}`);
}

// Execute both shell gesture handlers with the production camera. A visible
// fleet in front of a star continues to 384×; an exposed star still enters at 96×.
for (const [file, className, gesture, constant] of [
  ['shell/deck/map.ts', 'DeckMapInteraction', 'wheel', 'SYSTEM_SCRUB_STEP'],
  ['shell/mobile/map.ts', 'MobileMapInteraction', 'applyPinch', 'SCRUB_PER_LOG_SCALE'],
]) {
  const shell = extract(file, className, [gesture, 'systemUnderPoint', 'systemById',
    ...(gesture === 'wheel' ? ['beginScrubIn', 'zoomIn', 'doorwaySystem'] : [])], [constant]);
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

  // Off the star but on its orbit preview: the next inward gesture goes in.
  r.scale = 96*r.fitScale(); r.cx = 150; r.cy = 270;
  const charted = { id: 'charted', pos: r.screenToWorld(500, 300) };
  shellInstance.ctx.state.galaxy.systems = [charted];
  r.galaxy = shellInstance.ctx.state.galaxy;
  r.approachVisuals.set('charted', { vis: { planets: [{ orbitRadius: .9, radius: .05 }], asteroidBelts: [] } });
  const offStar = { x: 500 + .8*300*.35, y: 300 };
  if (gesture === 'wheel') shellInstance.wheel({ deltaY: -100, clientX: offStar.x, clientY: offStar.y, preventDefault() {} });
  else shellInstance.applyPinch(1.3, offStar);
  assert.equal(r.isSystemScrubbing(), true, `${gesture}: the orbit preview is the doorway, not only the star`);
  close(r.zoomFactor(), 96, `${gesture}: no galaxy zoom past it`);

  // At the deepest camera a hull no longer holds the gesture.
  r.transition = null; r.zoomAt(500, 300, 4); close(r.zoomFactor(), 384, 'deepest camera');
  marker.container.visible = true; marker.shown = charted.pos;
  if (gesture === 'wheel') shellInstance.wheel({ deltaY: -100, clientX: 500, clientY: 300, preventDefault() {} });
  else shellInstance.applyPinch(1.3, { x: 500, y: 300 });
  assert.equal(r.isSystemScrubbing(), true, `${gesture}: at 384× a hull no longer blocks the star`);

  if (gesture === 'wheel') {
    // The + step at the doorway enters the star the map is centred on, and
    // otherwise keeps zooming; mid-handoff it advances the handoff.
    r.transition = null; r.scale = 96*r.fitScale();
    const rect = r.cameraRect, middle = { x: rect.x + rect.w/2, y: rect.y + rect.h/2 };
    r.cx += middle.x - r.worldToScreen(charted.pos).x; r.cy += middle.y - r.worldToScreen(charted.pos).y;
    marker.container.visible = false;
    shellInstance.ctx.state.selectedSystemId = null;
    shellInstance.zoomIn();
    assert.equal(r.isSystemScrubbing(), true, '+ at the doorway: into the centred star');
    const before = r.transitionTargetRaw; shellInstance.zoomIn();
    assert.ok(r.transitionTargetRaw > before, '+ mid-handoff: the handoff advances');
    r.transition = null; r.cx += 2000;
    shellInstance.zoomIn();
    assert.equal(r.isSystemScrubbing(), false, '+ over deep space: no system');
    close(r.zoomFactor(), 96*1.3, '+ over deep space: the camera zooms');
    r.transition = null; r.scale = 96*r.fitScale();
    r.cx += middle.x - 300 - r.worldToScreen(charted.pos).x; r.cy += middle.y - r.worldToScreen(charted.pos).y;
    assert.equal(r.approachSystemAt(middle.x, middle.y), null, 'nothing under the centre');
    shellInstance.ctx.state.selectedSystemId = 'charted';
    const shown = r.worldToScreen(charted.pos);
    assert.ok(shown.x > rect.x && shown.x < rect.x + rect.w, 'selected star on screen, off the centre');
    shellInstance.zoomIn();
    assert.equal(r.isSystemScrubbing(), true, '+ at the doorway: the selected star while on screen');
  }
}
// §system-approach: at the doorway the whole orbit preview is the way in, not
// only the star; an uncharted star still offers a near-zone allowance.
{
  const r = new Camera(); r.scale = 96*r.fitScale();
  const star = { id: 'charted', pos: r.screenToWorld(500, 300) };
  r.galaxy = { systems: [star] };
  r.approachVisuals.set('charted', { vis: { planets: [{ orbitRadius: .9, radius: .05 }], asteroidBelts: [] } });
  const k = 300 * .35;
  assert.equal(r.approachSystemAt(500 + .9*k, 300)?.id, 'charted', 'on the outer orbit');
  assert.equal(r.approachSystemAt(500, 300 + .9*.6*k + 10)?.id, 'charted', 'on its near side');
  assert.equal(r.approachSystemAt(500 + .95*k + 60, 300), null, 'deep space beyond it');
  r.approachVisuals.delete('charted');
  assert.equal(r.approachSystemAt(500 + .6*k, 300)?.id, 'charted', 'uncharted: a near-zone allowance');
  assert.equal(r.approachSystemAt(500 + .9*k + 20, 300), null, 'uncharted: not the whole preview width');
  const near = { id: 'near', pos: r.screenToWorld(560, 300) };
  r.galaxy = { systems: [star, near] };
  assert.equal(r.approachSystemAt(545, 300)?.id, 'near', 'overlapping previews: nearest star');
  r.scale = 30*r.fitScale();
  assert.equal(r.approachSystemAt(500, 300), null, 'no preview, no doorway, before the approach band');
  r.scale = 96*r.fitScale(); r.zoomAt(500, 300, 4);
  r.galaxy = { systems: [star] };
  r.approachVisuals.set('charted', { vis: { planets: [{ orbitRadius: .9, radius: .05 }], asteroidBelts: [] } });
  assert.equal(r.approachSystemAt(500 + .9*300, 300)?.id, 'charted', 'past the doorway the preview caps at the fit');
  assert.equal(r.beginSystemScrubIn(star, []), true);
  close(r.transition.growFrom, 1, 'entering from the deepest camera: nothing shrinks');
}

// Retreats start from the star's current place: a panned orrery's star is
// off-centre, and the galaxy camera picks the handoff up from there.
{
  const r = new Camera(); r.scale = 96*r.fitScale();
  const star = { id: 'star', pos: { x: 4000, y: -3000 } };
  r.enterSystemView(star, []);
  r.transition = null; r.mode = { type: 'system', systemId: 'star' };
  r.systemScene.starLayoutPosition = () => ({ x: 720, y: 333 });
  assert.equal(r.beginSystemScrubOut(star), true);
  const from = r.transition.camFrom;
  close(from.cx + star.pos.x*from.scale, 720, 'retreat starts at the star x');
  close(from.cy + star.pos.y*from.scale, 333, 'retreat starts at the star y');
  close(r.transition.growFrom, .35, 'and ends on the doorway preview');
}

console.log('Map zoom: 384× clamp, cursor/button anchors, panning, exact camera restoration, 96× star and unchanged battle thresholds, desktop wheel and mobile pinch fleet priority, the orbit preview as the doorway (charted, uncharted, nearest, deepest camera, + step) and off-centre retreats passed.');

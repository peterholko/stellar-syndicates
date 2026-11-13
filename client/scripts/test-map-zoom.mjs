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
  } }).outputText, { exports, performance: { now: () => 100 }, Map, Math, ORBIT_TILT: 0.6, SYSTEM_UNIT_SU: 860 });
  return exports;
}
const methods = ['worldToScreen', 'screenToWorld', 'clampScale', 'zoomAt', 'zoomByFactor', 'panBy',
  'zoomFactor', 'viewMode', 'atMaxZoom', 'atSystemZoomThreshold', 'atBattleZoomThreshold', 'fleetUnderPoint',
  'systemEndpointCamera', 'startTransition', 'isSystemScrubbing', 'adjustSystemScrub',
  'beginSystemScrubIn', 'beginSystemScrubOut', 'enterSystemView', 'exitSystemView',
  'previewScale', 'diveTilt', 'handoffGrowFrom', 'handoffCamFrom', 'approachSystemAt',
  'flyIntoSystem', 'tickFlight', 'cancelSystemScrub', 'isFlying', 'syncSystemSelection', 'resetView',
  'beginCameraTween', 'centerOnWorld', 'ensureWorldVisible', 'frameRaid', 'raidFrameCamera', 'tickCameraTween'];
const extracted = extract('render.ts', 'Renderer', methods,
  ['ZOOM_MIN_FACTOR', 'ZOOM_MAX_FACTOR', 'SYSTEM_ZOOM_FACTOR', 'MACHINE_ZOOM_END', 'BATTLE_ZOOM_FACTOR', 'clamp01',
    'SCHEMATIC_GROW_FROM', 'APPROACH_HINT_START', 'APPROACH_REACH_MIN', 'APPROACH_PICK_SLACK_PX', 'smooth01',
    'easeInOut', 'FLY_MS_PER_DOUBLING', 'FLY_MIN_MS', 'FLY_MAX_MS', 'FLY_HANDOFF_MS',
    'RAID_FRAME_MS', 'RAID_FRAME_SHARE', 'RAID_FRAME_MARGIN', 'RAID_FRAME_MAX_ZOOM']);
const { Camera } = compile(`${extracted.constants}
export class Camera {
  ${extracted.members}
  constructor() {
    this.galaxy = {}; this.scale = .001; this.cx = 150; this.cy = 270;
    this.mode = { type: 'galaxy' }; this.transition = null; this.ghosts = new Map();
    this.cameraRect = { x: 50, y: 80, w: 1100, h: 640 };
    this.galaxyRoot = {}; this.approachVisuals = new Map();
    this.systemScene = { clearSelection() {}, currentId() { return 'star'; }, resetZoom() {}, restScale: () => 300,
      starLayoutPosition: () => ({ x: 600, y: 400 }) };
  }
  fitScale() { return .001; } // a 400,000 su galaxy on a laptop screen
  prepareSystemScene(system) { this.systemFocus = system.pos; }
  dressSystemScene() {}
  systemHitRadius() { return 55; }
  battlePick() { return null; }
  trackingInsetAt() { return false; }
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
  // §real-coordinates: the preview is to the map's true scale (860 su a unit).
  close(r.transition.growFrom, Math.max(96, zoom) * r.fitScale() * 860 / 300, 'the schematic starts where the preview stands');
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
  const offStar = { x: 500 + .8*r.scale*860, y: 300 };
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
  const k = r.scale * 860; // the map's own scale, a unit = 860 su
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
  assert.equal(r.approachSystemAt(500 + .9*r.scale*860, 300)?.id, 'charted', 'past the doorway the preview keeps true scale');
  assert.equal(r.beginSystemScrubIn(star, []), true);
  close(r.transition.growFrom, r.scale * 860 / 300, 'entering from the deepest camera: the preview as it stands');
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
  close(r.transition.growFrom, 96 * r.fitScale() * 860 / 300, 'and ends on the doorway preview');
}

// §home-step: a fly-in zooms at an even pace while the star slides to the
// centre, hands off at the doorway as the wheel does (never resting at 0),
// and Back returns to where it began. A pan, or Esc, takes over.
{
  const r = new Camera(); r.scale = 4*r.fitScale();
  const star = { id: 'home', pos: r.screenToWorld(300, 200) };
  const opening = { cx: r.cx, cy: r.cy, scale: r.scale };
  assert.equal(r.flyIntoSystem(star, []), true);
  assert.equal(r.isFlying(), true);
  assert.equal(r.flyIntoSystem(star, []), false, 'one flight at a time');
  const duration = r.flight.duration;
  close(duration, Math.log2(24) * 300, '300 ms a doubling');
  r.tickFlight(100 + duration / 2);
  close(r.zoomFactor(), Math.sqrt(4 * 96), 'halfway in time, halfway in zoom (geometric)');
  const mid = r.worldToScreen(star.pos);
  close(mid.x, (300 + 600) / 2, 'the star slides toward the centre x');
  close(mid.y, (200 + 400) / 2, 'and y');
  r.tickFlight(100 + duration);
  assert.equal(r.isFlying(), false);
  close(r.zoomFactor(), 96, 'arrives at the doorway');
  const end = r.worldToScreen(star.pos);
  close(end.x, 600, 'centred x'); close(end.y, 400, 'centred y');
  assert.equal(r.isSystemScrubbing(), true, 'then hands off into the System View');
  assert.ok(r.transitionTargetRaw > 0, 'a handoff resting at 0 would end on the galaxy');
  r.tickFlight(100 + duration + 600);
  assert.equal(r.transitionTargetRaw, 1, 'the clock drives the handoff home');
  assert.deepEqual(JSON.parse(JSON.stringify(r.savedGalaxyCam)), opening, 'Back returns to where the flight began');
  r.cancelSystemScrub();
  assert.equal(r.transitionTargetRaw, 0, 'Esc in the handoff retreats as a wheel would');
}
{
  const r = new Camera(); r.scale = 4*r.fitScale();
  const star = { id: 'home', pos: r.screenToWorld(300, 200) };
  r.flyIntoSystem(star, []); r.panBy(5, 5);
  assert.equal(r.isFlying(), false, 'a pan takes over');
  r.flyIntoSystem(star, []); r.zoomAt(500, 300, 1.2);
  assert.equal(r.isFlying(), false, 'so does a zoom');
  r.flyIntoSystem(star, []); const scale = r.scale; r.cancelSystemScrub();
  assert.equal(r.isFlying(), false, 'Esc stops the flight');
  assert.equal(r.scale, scale, 'where it is');
  r.mode = { type: 'system', systemId: 'x' };
  assert.equal(r.flyIntoSystem(star, []), false, 'only from the galaxy');
}

// One selection in the System View (Peter, Oct 21 2026). A selected fleet
// drops the world's ring; a world picked on either shell lets go of fleets
// before its panel opens; every shell pulse of a world does the same.
{
  const r = new Camera(); let cleared = 0;
  r.systemScene = { clearSelection() { cleared++; } };
  r.syncSystemSelection({ selectedShipId: null, selectedShipIds: new Set() });
  assert.equal(cleared, 0, 'no fleet: the world keeps its ring');
  r.syncSystemSelection({ selectedShipId: 'interceptor', selectedShipIds: new Set() });
  r.syncSystemSelection({ selectedShipId: null, selectedShipIds: new Set(['group']) });
  assert.equal(cleared, 2, 'a selected fleet takes it');
}
for (const [file, className, opened] of [['shell/deck/map.ts', 'DeckMapInteraction', 'open systemBody'],
  ['shell/mobile/map.ts', 'MobileMapInteraction', 'sheet planet']]) {
  const mobile = className === 'MobileMapInteraction';
  const shell = extract(file, className, ['applyResult', ...(mobile ? ['deselectFleets'] : [])], []);
  const { Interaction } = compile(`export class Interaction { constructor(ctx, hooks) { this.ctx = ctx; this.hooks = hooks; } syncArmedChip() {} ${shell.members} }`);
  const calls = [];
  const map = new Interaction({ state: {}, renderer: { stateVersion: 0, viewMode: { type: 'system', systemId: 'home' } },
    intent: { deselectFleets: () => calls.push('deselect') } },
  { openTarget: (target) => calls.push(`open ${target.type}`), openSheet: (sheet) => calls.push(`sheet ${sheet.id}`) });
  map.applyResult({ kind: 'select', target: { type: 'systemBody', detail: { id: '12', name: 'Freya 12' } } }, false, false);
  assert.deepEqual(calls, ['deselect', opened], `${file}: the world takes the one selection`);
}
for (const file of ['shell/deck/index.ts', 'shell/deck/founding-guide.ts', 'shell/mobile/parity.ts']) {
  const lines = source(file).split('\n');
  const pulses = lines.flatMap((line, i) => line.includes('pulseSystemBody(') ? [i] : []);
  assert.ok(pulses.length, `${file} pulses a world`);
  for (const i of pulses) assert.match(lines[i - 1], /deselectFleets\(\);/, `${file}:${i + 1}: fleets go before a world's ring`);
}
assert.match(source('shell/mobile/index.ts'), /deselectFleets: \(\) => this\.map\?\.deselectFleets\(\)/);

// §raid-frame (Peter, Oct 27 2026): a raid's first sighting flies the map once
// onto the raided home's star and the raider, centred in the map below the
// raid window, their separation about half that space but never past 80×
// (short of the doorway). A way out of a System View lands on the same frame.
// Then the camera holds (the window follows the raider); a drag takes over.
{
  const inset = extract('trackinginset.ts', 'TrackingInset', ['place'],
    ['INSET_W', 'INSET_SHARE', 'INSET_ASPECT', 'MIN_AREA_W', 'MIN_AREA_H', 'MARGIN', 'CENTER_GAP']);
  const { Inset } = compile(`${inset.constants}\nexport class Inset { ${inset.members} }`);
  const raidCamera = (zoom, window = true) => {
    const r = new Camera(); r.scale = zoom * r.fitScale();
    r.trackingInsetOn = window; r.trackingInset = new Inset();
    return r;
  };
  const inside = (p, box, label) => assert.ok(p.x > box.x && p.x < box.x + box.w && p.y > box.y && p.y < box.y + box.h,
    `${label}: ${JSON.stringify(p)} outside ${JSON.stringify(box)}`);
  const home = { x: 100_000, y: 50_000 };
  const raider = { x: home.x + 2_850, y: home.y - 930 }; // ~3,000 su out, as first sighted
  const mid = { x: (raider.x + home.x) / 2, y: (raider.y + home.y) / 2 };

  const r = raidCamera(4);
  const rect = r.cameraRect, window = r.trackingInset.place(rect, 'center');
  const below = { x: rect.x + 12, y: window.y + window.h + 12, w: rect.w - 24, h: rect.y + rect.h - window.y - window.h - 24 };
  const centre = { x: below.x + below.w / 2, y: below.y + below.h / 2 };
  const opening = r.worldToScreen(mid);
  r.frameRaid(raider, home);
  assert.equal(r.cameraTween.duration, 800, 'one flight');
  r.tickCameraTween(100 + 400);
  close(r.zoomFactor(), Math.sqrt(4 * 80), 'halfway in time, halfway in zoom (geometric)');
  close(r.worldToScreen(mid).x, (opening.x + centre.x) / 2, 'the raid slides straight to its place x');
  close(r.worldToScreen(mid).y, (opening.y + centre.y) / 2, 'and y');
  r.tickCameraTween(100 + 800);
  assert.equal(r.cameraTween, null);
  close(r.zoomFactor(), 80, 'a raid 3,000 su out: 80×, short of the doorway');
  close(r.worldToScreen(mid).x, centre.x, 'centred in the map below the window x');
  close(r.worldToScreen(mid).y, centre.y, 'and y');
  inside(r.worldToScreen(home), below, 'the home star clear of the window');
  inside(r.worldToScreen(raider), below, 'the raider clear of the window');
  const held = { cx: r.cx, cy: r.cy, scale: r.scale };
  r.tickCameraTween(100 + 60_000);
  assert.deepEqual({ cx: r.cx, cy: r.cy, scale: r.scale }, held, 'then the map holds');
  r.panBy(10, 0);
  close(r.cx, held.cx + 10, 'and a drag is the player\'s');

  // Sighted farther out, the zoom spans half the space below the window.
  const far = { x: home.x + 60_000, y: home.y };
  const wide = raidCamera(4);
  wide.frameRaid(far, home); wide.tickCameraTween(100 + 800);
  close(wide.worldToScreen(far).x - wide.worldToScreen(home).x, below.w / 2, 'half the frame apart');
  const tall = raidCamera(4), high = { x: home.x, y: home.y - 60_000 };
  tall.frameRaid(high, home); tall.tickCameraTween(100 + 800);
  close(tall.worldToScreen(home).y - tall.worldToScreen(high).y, below.h / 2, 'the frame\'s height when it comes from above');

  // Without the window (the mobile shell), the whole map frames it.
  const bare = raidCamera(90, false);
  bare.frameRaid(raider, home); bare.tickCameraTween(100 + 800);
  close(bare.worldToScreen(mid).x, rect.x + rect.w / 2, 'no window: the map\'s middle x');
  close(bare.worldToScreen(mid).y, rect.y + rect.h / 2, 'and y');
  close(bare.zoomFactor(), 80, 'flying out from deeper too');

  // A route opening under way never cancels the flight.
  const flying = raidCamera(4);
  flying.frameRaid(raider, home);
  flying.ensureWorldVisible({ x: home.x + 300_000, y: home.y });
  assert.notEqual(flying.cameraTween, null, 'a panel opening mid-flight leaves it be');

  // From a System View: the shell's way out lands on the frame, in one move.
  const inside0 = raidCamera(4);
  inside0.enterSystemView({ id: 'home', pos: home }, []);
  inside0.transition = null; inside0.mode = { type: 'system', systemId: 'home' };
  inside0.exitSystemView();
  inside0.frameRaid(raider, home);
  const way = inside0.transition.camTo;
  close(way.scale / inside0.fitScale(), 80, 'the way out lands on the frame');
  close(way.cx + mid.x * way.scale, centre.x, 'its place x'); close(way.cy + mid.y * way.scale, centre.y, 'and y');
  close(inside0.transition.growFrom, way.scale * 860 / 300, 'the orrery hands off to the preview there');
  assert.equal(inside0.cameraTween, undefined, 'one move, not two');

  // Never into a System View, a theater, or a wheel's handoff.
  const system = raidCamera(4);
  system.mode = { type: 'system', systemId: 'home' };
  system.frameRaid(raider, home);
  assert.equal(system.cameraTween, undefined, 'a System View still open stays');
  const scrub = raidCamera(96);
  scrub.galaxy = { systems: [] };
  assert.equal(scrub.beginSystemScrubIn({ id: 'home', pos: home }, []), true);
  const camTo = { ...scrub.transition.camTo };
  scrub.frameRaid(raider, home);
  assert.deepEqual({ ...scrub.transition.camTo }, camTo, 'a wheel handoff keeps its way');
  assert.equal(scrub.cameraTween, undefined);

  // A pan alone is unchanged: the target slides to the middle at one zoom.
  const pan = raidCamera(8);
  const target = { x: 20_000, y: -10_000 };
  pan.centerOnWorld(target);
  pan.tickCameraTween(100 + 160);
  close(pan.zoomFactor(), 8, 'a pan keeps its zoom');
  pan.tickCameraTween(100 + 320);
  close(pan.worldToScreen(target).x, rect.x + rect.w / 2, 'centred x'); close(pan.worldToScreen(target).y, rect.y + rect.h / 2, 'centred y');
}
console.log('Map zoom: 384× clamp, cursor/button anchors, panning, exact camera restoration, 96× star and unchanged battle thresholds, desktop wheel and mobile pinch fleet priority, the orbit preview as the doorway (charted, uncharted, nearest, deepest camera, + step), off-centre retreats, the home-step fly-in, one selection in the System View and the raid frame passed.');


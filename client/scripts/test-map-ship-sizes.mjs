// Exercise the actual galaxy renderer's size, art selection, draw expression,
// and hit-radius methods without a Pixi context or a running game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import sharp from "sharp";
import ts from "typescript";

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
function compile(text, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, ...globals });
  return exports;
}
const { fleetExactCount } = compile(source("protocol.ts"));
const { hashId } = compile(source("prng.ts"));
const generatedArt = compile(source("ship-art.generated.ts"));
const shipArt = compile(source("shipart.ts"), { require: () => generatedArt });
const { shipArtwork } = shipArt;
const layout = compile(source("systemlayout.ts"));
const rendererSource = source("render.ts");

function fixture(overrides = {}, text = rendererSource) {
  const ast = ts.createSourceFile("render.ts", text, ts.ScriptTarget.Latest, true);
  const owner = ast.statements.find(n => ts.isClassDeclaration(n) && n.name?.text === "Renderer");
  const constants = ["PRIVATEER_ART_CALIB", "PIRATE_CORSAIR_ART_CALIB", "PIRATE_BOARDING_ART_CALIB",
    "INTERCEPTOR_MAP_SCALE", "PRIVATEER_MAP_SCALE", "SHIP_PX_CONVOY", "SHIP_PX_RAIDER",
    "SHIP_PX_CORVETTE", "SHIP_PX_COLONY", "SHIP_PX_SCOUT", "SHIP_ZOOM_MIN", "SHIP_ZOOM_MAX",
    "SHIP_NATIVE_ZOOM_START", "MACHINE_ZOOM_END", "SHIP_MAX_PX", "SHIP_CLASS_MAX_PX", "TIER_SCALE",
    "FLEET_LEAD_CALIB", "LOD_ICON_ZOOM_MAX", "LOD_ICON_CALIB", "SYSTEM_ZOOM_FACTOR", "ZOOM_MAX_FACTOR",
    "APPROACH_HINT_START", "SHIP_SYSTEM_SCALE_FULL", "SHIP_SYSTEM_CLEAR_SU", "SCHEMATIC_GROW_FROM",
    "ZOOM_MIN_FACTOR", "INITIAL_HOME_ZOOM_FACTOR", "HUB_SIZE_STOPS", "HUB_NEAR_RADII", "HUB_CLEAR_RADII",
    "clamp01", "smooth01"];
  const declarations = ast.statements.filter(ts.isVariableStatement)
    .flatMap(n => [...n.declarationList.declarations])
    .filter(n => constants.includes(n.name.getText(ast)));
  assert.equal(declarations.length, constants.length);
  const methods = ["deepZoomPx", "shipSizePx", "nearStarBlend", "nearHubBlend", "hubZoom", "hubRenderedPx",
    "shipHitRadius", "fleetHitRadius", "texFor", "fleetFamily", "fleetTier", "lodIconMarker", "fleetMarker"];
  const members = owner.members.filter(n => methods.includes(n.name?.getText(ast)));
  assert.equal(members.length, methods.length);
  // Use drawGhost's real target expression too: omitting the pirate flag there
  // must fail even when the size helper and the hit-radius path are correct.
  const draw = owner.members.find(n => n.name?.getText(ast) === "drawGhost");
  let target;
  const visit = n => {
    if (ts.isVariableDeclaration(n) && n.name.getText(ast) === "targetPx") target = n.initializer;
    ts.forEachChild(n, visit);
  };
  visit(draw);
  assert.ok(target);
  const { Renderer } = compile(`${declarations.map(n => {
    const name = n.name.getText(ast);
    return name in overrides ? `const ${name} = ${JSON.stringify(overrides[name])};` : `const ${n.getText(ast)};`;
  }).join("\n")}
    export class Renderer {
      ghosts = new Map();
      galaxy = null;
      ${members.map(n => n.getText(ast)).join("\n")}
      fitScale() { return 1; }
      drawnSize(ghost) {
        const marker = this.fleetMarker(ghost);
        const sp = this.ghosts.get(ghost.id) ?? {};
        return ${target.getText(ast)};
      }
    }`, { fleetExactCount, hashId, ...shipArt, NEAR_ZONE_SU: layout.NEAR_ZONE_SU, systemShipPx: layout.systemShipPx });
  const renderer = new Renderer();
  for (const name of ["Convoy", "Raider", "Corvette", "Colony", "Scout", "Destroyer", "Cruiser",
    "Battleship", "Dreadnought", "Titan", "AuthorityFreighter", "Transport", "Builder", "MiningBarge", "MiningBeltStation",
    "Privateer", "PirateCorsair", "PirateBoarding", "IconFreighter", "IconRaider", "IconCorvette"]) {
    renderer[`tex${name}`] = { name, width: 256 };
  }
  renderer.texFleet = new Map(["freighter", "raider", "corvette", "scout"].flatMap(family =>
    ["wing", "squadron", "armada"].map(tier => [`${family}_${tier}`, { name: `${family}_${tier}`, width: 256 }])));
  renderer.freightTextures = new Map(["tiny_freighter", "small_freighter", "large_freighter", "heavy_freighter", "bulk_freighter"]
    .map(kind => [kind, { name: kind, width: 256 }]));
  return renderer;
}

const close = (actual, expected, message) =>
  assert.ok(Math.abs(actual - expected) < 1e-9, `${message}: ${actual} != ${expected}`);
const ghost = (kind, count = 1, extra = {}) => ({ id: "101", kind, own: true,
  composition: [{ kind, count }], count_class: "one", ...extra });
const zooms = [.5, .9, 1, 1.3, 1.6, 2.499, 2.5, 4, 8, 12, 12.01, 15, 18, 21, 23.99, 24, 48, 72, 96, 144, 192, 288, 384];
const normalCanvas = { raider: 64, convoy: 89.6, freighter: 89.6, corvette: 76.8,
  colony: 102.4, scout: 48, builder: 83.2, transport: 92.8, destroyer: 83.2,
  cruiser: 96, battleship: 112, dreadnought: 131.2, titan: 153.6 };
const classCaps = { scout: 72, corvette: 144, destroyer: 240, cruiser: 384,
  battleship: 480, dreadnought: 576, titan: 768 };
const deepCanvas = { raider: 96, convoy: 120, freighter: 120,
  colony: 120, builder: 120, transport: 120, ...classCaps };
const changedClasses = new Set(["scout", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"]);
const current = fixture();
assert.equal(current.fleetMarker(ghost("mining_barge")).tex.name, "MiningBarge", "Barge selects its dedicated map texture");
assert.equal(current.fleetMarker(ghost("mining_belt_station")).tex.name, "MiningBeltStation", "Station selects its dedicated map texture");
current.scale = 10;
close(current.shipSizePx("mining_belt_station"), 86 * 1.6, "Station retains its normal map freight base");
current.scale = 96;
close(current.shipSizePx("mining_belt_station"), 180, "Station retains its 96× class cap");
// Both reference curves run the real interpolation, not copied sizing math.
const unscaled = fixture({ INTERCEPTOR_MAP_SCALE: 1, PRIVATEER_MAP_SCALE: 1, SHIP_CLASS_MAX_PX: classCaps });
const beforeHierarchy = fixture({ SHIP_CLASS_MAX_PX: {} });
const variants = new Map();
current.scale = 4;
for (let id = 0; id < 100; id++) {
  const g = ghost("raider", 1, { id: String(id), own: false, pirate: true });
  variants.set(current.fleetMarker(g).tex.name, g);
}
assert.equal(variants.size, 3, "all three privateer silhouettes are exercised");
const scenarios = [
  ...Object.keys(normalCanvas).flatMap(kind => [1, 2, 4, 8].map(count =>
    ({ ghost: ghost(kind, count), factor: kind === "raider" ? .8 : 1 }))),
  { ghost: ghost("freighter", 1, { migrant: true }), factor: 1 },
  { ghost: ghost("raider", 1, { own: false }), factor: .8 },
  ...[...variants.values()].flatMap(g => ["raider", "corvette"].flatMap(kind => [1, 2, 4, 8].map(count =>
    ({ ghost: { ...g, kind, composition: [{ kind, count }] }, factor: .9 })))),
  ...["one", "two_to_three", "four_to_seven", "eight_to_fifteen"].map(count_class =>
    ({ ghost: ghost("raider", 1, { composition: undefined, count_class }), factor: .8 })),
];
function check(renderer) {
  for (const zoom of zooms) {
    renderer.scale = unscaled.scale = beforeHierarchy.scale = zoom;
    for (const { ghost: g, factor } of scenarios) {
      const label = `${g.kind}/${g.pirate ? renderer.fleetMarker(g).tex.name : g.count_class} at r=${zoom}`;
      close(renderer.drawnSize(g), unscaled.drawnSize(g) * factor, `${label} proportional size`);
      close(renderer.fleetHitRadius(g), renderer.drawnSize(g) / 2, `${label} matching pick/overlay radius`);
      assert.equal(renderer.fleetMarker(g).tex.name, beforeHierarchy.fleetMarker(g).tex.name, "art selection is unchanged");
      if (zoom <= 12 || g.pirate || !changedClasses.has(g.kind)) {
        close(renderer.drawnSize(g), beforeHierarchy.drawnSize(g), `${label} preserved curve`);
      }
    }
  }
}
function checkEndpoints(renderer) {
  for (const zoom of [4, 12, 24, 96]) {
    renderer.scale = zoom;
    for (const [kind, normal] of Object.entries(normalCanvas)) {
      close(renderer.shipSizePx(kind), zoom <= 12 ? normal * (kind === "raider" ? .8 : 1) : deepCanvas[kind],
        `${kind} endpoint at r=${zoom}`);
    }
  }
}

function checkMonotonic(renderer, kinds = Object.keys(normalCanvas)) {
  for (const kind of kinds) {
    let previous = 0;
    for (let step = 9; step <= 3840; step++) {
      renderer.scale = step / 10;
      const size = renderer.shipSizePx(kind);
      assert.ok(size >= previous - 1e-9, `${kind} shrank at r=${renderer.scale}: ${previous} -> ${size}`);
      previous = size;
    }
  }
}
check(current);
checkEndpoints(current);
checkMonotonic(current);
// Freight progression must remain readable at every zoom, using six distinct
// single-hull designs even when a fleet has enough ships for a formation icon.
const freightKinds = ["tiny_freighter", "small_freighter", "convoy", "large_freighter", "heavy_freighter", "bulk_freighter"];
checkMonotonic(current, freightKinds);
for (const zoom of zooms) {
  current.scale = zoom;
  let previousLength = 0;
  for (const kind of freightKinds) {
    const g = ghost(kind);
    const visibleLength = current.drawnSize(g) * shipArtwork(kind).lengthRatio;
    assert.ok(visibleLength > previousLength, `${kind} must be visibly larger than the previous freight size at ${zoom}`);
    previousLength = visibleLength;
    close(current.fleetHitRadius(g), current.drawnSize(g) / 2, `${kind}: freight art and pick radius agree`);
    if (kind !== "convoy") assert.equal(current.fleetMarker(ghost(kind, 8)).tex.name, kind,
      "new freight sizes never borrow a different hull's formation art");
  }
}
current.scale = 96;
for (const [index, cap] of [80, 100, 120, 160, 200, 240].entries()) {
  close(current.shipSizePx(freightKinds[index]), cap, "freight close-up cap");
}
// Execute the real map-picking radius expression: the Scout's reduced art must
// not reduce the existing 24px minimum target, nor cap a capital's larger one.
const pickAst = ts.createSourceFile("mapclick.ts", source("core/mapclick.ts"), ts.ScriptTarget.Latest, true);
const pickRadii = [];
const visitPick = n => {
  if (ts.isVariableDeclaration(n) && n.name.getText(pickAst) === "radius"
      && n.initializer?.getText(pickAst).includes("renderer.fleetHitRadius(ghost)")) pickRadii.push(n.initializer);
  ts.forEachChild(n, visitPick);
};
visitPick(pickAst);
assert.equal(pickRadii.length, 1);
const { pickRadius } = compile(`export function pickRadius(renderer, ghost) { return ${pickRadii[0].getText(pickAst)}; }`);
for (const zoom of [.9, 4, 24, 96, 192, 384]) {
  current.scale = zoom;
  close(pickRadius(current, ghost("scout")), Math.max(24, current.drawnSize(ghost("scout")) / 2), `Scout pick radius at ${zoom}`);
  close(pickRadius(current, ghost("titan")), current.drawnSize(ghost("titan")) / 2, `Titan pick radius at ${zoom}`);
}
// No pop at the 12/24 boundaries; LOD and fleet-count changes keep lead parity.
for (const kind of Object.keys(normalCanvas)) {
  for (const boundary of [1.6, 2.5, 12, 24, 96, 384]) {
    current.scale = boundary - 1e-6;
    const before = current.drawnSize(ghost(kind));
    current.scale = boundary + 1e-6;
    assert.ok(Math.abs(current.drawnSize(ghost(kind)) - before) < .001, `${kind} seam at ${boundary}`);
  }
  for (const zoom of zooms) {
    current.scale = zoom;
    for (const count of [2, 4, 8]) {
      close(current.drawnSize(ghost(kind, count)), current.drawnSize(ghost(kind)), `${kind} lead parity`);
    }
  }
}

// Inspection grows the actual drawn hull and its pick/overlay radius together,
// preserving every ship's visible size ratio and the existing 96× endpoint.
for (const { ghost: g } of scenarios) {
  current.scale = 96;
  const original = current.drawnSize(g);
  for (const [zoom, multiplier] of [[192, 2], [384, 4], [614.4, 4]]) {
    current.scale = zoom;
    close(current.drawnSize(g), original * multiplier, `${g.kind}: inspection magnification at ${zoom}`);
    close(current.fleetHitRadius(g), original * multiplier / 2, `${g.kind}: inspection hit radius`);
  }
}

// A later bad tuning value must not reintroduce shrinking through the ramp.
const undersizedCap = fixture({ SHIP_CLASS_MAX_PX: { titan: 80 } });
checkMonotonic(undersizedCap);
undersizedCap.scale = 24;
close(undersizedCap.shipSizePx("titan"), normalCanvas.titan, "cap floors at normal indicator");

current.scale = 96;
close(current.drawnSize(variants.get("Privateer")), 78.84, "classic privateer deep cap");
close(current.drawnSize(variants.get("PirateCorsair")), 75.6, "corsair deep cap");
close(current.drawnSize(variants.get("PirateBoarding")), 75.6, "boarding privateer deep cap");
// Missing formation art still falls back to the correctly scaled single hull.
current.texFleet.clear();
close(current.drawnSize(ghost("raider", 8)), 96 * shipArtwork("raider").calib, "formation fallback");

// Actual asset bounds check: canvas padding must not erase the visible ladder.
// Alpha >= 128 excludes the faint exhaust/fringe, measuring nose-to-tail length.
const hulls = [
  ["scout", "scout_utility_ship.png"], ["raider", "raider_attack_ship.png"],
  ["convoy", "corporate_freighter.png"], ["corvette", "corvette_escort_ship.png"],
  ["destroyer", "destroyer_line_ship.png"], ["cruiser", "cruiser_line_ship.png"],
  ["battleship", "battleship_line_ship.png"], ["dreadnought", "dreadnought_line_ship.png"],
  ["titan", "titan_flagship.png"],
];
const lengths = new Map();
for (const [kind] of hulls) {
  const url = shipArtwork(kind).levels.at(-1).url;
  const { data, info } = await sharp(new URL(`../public${url.split("?")[0]}`, import.meta.url).pathname)
    .ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let minY = info.height, maxY = -1;
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    if (data[(y * info.width + x) * info.channels + info.channels - 1] < 128) continue;
    minY = Math.min(minY, y); maxY = Math.max(maxY, y);
  }
  assert.ok(maxY >= minY, `${kind} has a visible hull`);
  const canvasPx = current.drawnSize(ghost(kind));
  assert.ok(canvasPx <= info.width, `${kind} does not exceed its native canvas`);
  const length = (maxY - minY + 1) / info.width * canvasPx;
  const previous = [...lengths.values()].at(-1) ?? 0;
  assert.ok(length > previous, `${kind} visible hull preserves the ladder`);
  lengths.set(kind, length);
}
assert.ok(lengths.get("scout") < lengths.get("convoy") * .65, "Scout stays much shorter than Freighter");
for (const [kind, ratio] of Object.entries({ corvette: 1.5, destroyer: 2.5, cruiser: 4, battleship: 5, dreadnought: 6, titan: 8 })) {
  // Measure the native pixels, not just declared canvas targets. A thumbnail's
  // one-pixel alpha rounding is checked separately by test-ship-art.mjs.
  close(lengths.get(kind) / lengths.get("raider"), ratio, `${kind}: approved visible length ratio`);
  close(current.shipSizePx(kind) * shipArtwork(kind).calib * shipArtwork(kind).lengthRatio,
    current.shipSizePx("raider") * shipArtwork("raider").calib * shipArtwork("raider").lengthRatio * ratio,
    `${kind}: exact native hull ratio`);
}

// Teeth: class-cap rollback, the original shrinking ramp, and base-only role
// reduction each fail their own acceptance checks without mutating repo files.
assert.throws(() => checkEndpoints(beforeHierarchy), /endpoint/);
const guard = "const maxPx = Math.max(indicator, classCap);";
assert.ok(rendererSource.includes(guard));
const noGuard = rendererSource.replace(guard, "const maxPx = classCap;");
assert.throws(() => checkMonotonic(fixture({ SHIP_CLASS_MAX_PX: {} }, noGuard)), /dreadnought shrank/);
assert.throws(() => checkMonotonic(fixture({ SHIP_CLASS_MAX_PX: { titan: 80 } }, noGuard), ["titan"]), /titan shrank/);
assert.throws(() => check(fixture({ INTERCEPTOR_MAP_SCALE: 1, PRIVATEER_MAP_SCALE: 1 })),
  /proportional size/);
const scaledReturn = "const full = this.deepZoomPx(indicator, maxPx) * mapScale * inspection;";
assert.ok(rendererSource.includes(scaledReturn));
const baseOnly = rendererSource.replace(scaledReturn, "const full = this.deepZoomPx(indicator * mapScale, maxPx) * inspection;");
assert.throws(() => check(fixture({}, baseOnly)), /proportional size/);
// §system-approach: near a star, hulls ease to their System View size while
// its orbit preview shows and grow back as they clear the system. Past the
// 96× doorway they keep the System View's proportion to the growing preview.
const near = fixture();
const star = { systems: [{ pos: { x: 0, y: 0 } }] };
near.galaxy = star;
const blend = (zoom, x) => { near.scale = zoom; return near.nearStarBlend({ x, y: 0 }); };
const zone = layout.NEAR_ZONE_SU;
close(blend(40, 0), 0, "no ease before the orbit preview starts");
close(blend(52, 0), 0.5, "half way through the approach");
close(blend(64, 0), 1, "fully system-sized by 64×");
close(blend(96, zone), 1, "throughout the near zone");
close(blend(96, 2.5 * zone), 0.5, "growing back while clearing the system");
close(blend(96, 4 * zone), 0, "full size once clear of it");
close(blend(144, 0), 1, "still system-sized past the doorway");
close(blend(384, 0), 1, "all the way to the zoom limit");
near.galaxy = null;
close(blend(96, 0), 0, "no galaxy, no ease");
near.galaxy = star;
near.scale = 80;
for (const { ghost: g } of scenarios) {
  const label = `${g.kind}/${g.pirate ? near.fleetMarker(g).tex.name : g.count_class} near a star`;
  const mult = near.fleetMarker(g).mult;
  near.ghosts.delete(g.id);
  const machine = near.drawnSize(g);
  near.ghosts.set(g.id, { nearStar: 1 });
  const shrunk = near.drawnSize(g);
  close(shrunk, Math.min(machine / mult, layout.systemShipPx(g.kind)) * mult, `${label}: System View size`);
  close(near.fleetHitRadius(g), shrunk / 2, `${label}: picks follow the eased size`);
  near.ghosts.set(g.id, { nearStar: 0.5 });
  close(near.drawnSize(g), (machine + shrunk) / 2, `${label}: eases linearly`);
  near.ghosts.delete(g.id);
}
assert.ok(near.shipSizePx("cruiser", false, 1) < near.shipSizePx("cruiser") / 10, "a cruiser leaving a star is a tenth of its open-space size");
const systemSize = layout.systemShipPx("cruiser");
for (const [zoom, expected, why] of [[96, systemSize, "at the doorway: its System View size"],
  [192, systemSize, "never smaller than its System View size while the preview catches up"],
  [96 / 0.35, systemSize, "the preview reaches System View scale"],
  [384, systemSize * 0.35 * 4, "then it grows with the preview, in System View proportion"]]) {
  near.scale = zoom;
  close(near.shipSizePx("cruiser", false, 1), expected, `cruiser near a star at r=${zoom.toFixed(1)}: ${why}`);
}

// §hub-approach: beside the Market Hub hulls take the same System View size,
// so the Authority Nexus dwarfs them; the zone is measured in station radii.
const hubbed = fixture();
const hubGalaxy = { hub: { x: 0, y: 0 }, systems: [] };
hubbed.galaxy = hubGalaxy;
const hubBlend = (zoom, radii) => {
  hubbed.scale = zoom;
  return hubbed.nearHubBlend({ x: radii * hubbed.hubRenderedPx() / 2 / zoom, y: 0 });
};
close(hubBlend(40, 0), 0, "no ease before the approach window");
close(hubBlend(52, 1), 0.5, "half way through the approach, beside the station");
close(hubBlend(64, 2), 1, "small by 64×");
close(hubBlend(96, 3), 1, "out to three station radii");
close(hubBlend(96, 4.5), 0.5, "growing back while leaving");
close(hubBlend(96, 6), 0, "full size six radii out");
close(hubBlend(384, 2), 1, "all the way to the zoom limit");
close(hubBlend(200, 2.5), hubBlend(300, 2.5), "past 96× the zone is fixed in the world: the station grows with the camera");
hubbed.galaxy = null;
close(hubBlend(96, 0), 0, "no galaxy, no ease");
hubbed.galaxy = hubGalaxy;
for (const zoom of [96, 192, 384]) {
  hubbed.scale = zoom;
  const beside = hubbed.shipSizePx("raider", false, 0, 1);
  close(beside, layout.systemShipPx("raider") * Math.max(1, 0.35 * zoom / 96), `an Interceptor beside the hub at ${zoom}×: its System View size`);
  assert.ok(hubbed.hubRenderedPx() >= 8 * beside, `the station dwarfs it at ${zoom}× (${(hubbed.hubRenderedPx() / beside).toFixed(1)}×)`);
  close(hubbed.shipSizePx("titan", false, 0.3, 0.8), hubbed.shipSizePx("titan", false, 0.8), "near a star and the hub, the stronger ease wins");
}
hubbed.scale = 96;
for (const { ghost: g } of scenarios) {
  const mult = hubbed.fleetMarker(g).mult;
  hubbed.ghosts.delete(g.id);
  const machine = hubbed.drawnSize(g);
  hubbed.ghosts.set(g.id, { nearHub: 1 });
  const small = hubbed.drawnSize(g);
  close(small, Math.min(machine / mult, layout.systemShipPx(g.kind)) * mult, `${g.kind} beside the hub: System View size`);
  close(hubbed.fleetHitRadius(g), small / 2, `${g.kind} beside the hub: picks follow the eased size`);
  hubbed.ghosts.delete(g.id);
}

console.log(`Map ship sizing: ${scenarios.length} fleet cases × ${zooms.length} zooms passed; approved visible capital ratios, monotonic growth, seams, formation/hit-radius parity, unchanged civilian/Interceptor/privateer size curves, near-star and beside-hub easing (zoom, distance, proportion past the doorway, picks) and rollback teeth passed.`);
console.log(`Deep-zoom visible lengths: ${[...lengths].map(([kind, length]) => `${kind} ${length.toFixed(1)}px`).join(", ")}`);

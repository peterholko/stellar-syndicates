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
const rendererSource = source("render.ts");

function fixture(overrides = {}, text = rendererSource) {
  const ast = ts.createSourceFile("render.ts", text, ts.ScriptTarget.Latest, true);
  const owner = ast.statements.find(n => ts.isClassDeclaration(n) && n.name?.text === "Renderer");
  const constants = ["PRIVATEER_ART_CALIB", "PIRATE_CORSAIR_ART_CALIB", "PIRATE_BOARDING_ART_CALIB",
    "INTERCEPTOR_MAP_SCALE", "PRIVATEER_MAP_SCALE", "SHIP_PX_CONVOY", "SHIP_PX_RAIDER",
    "SHIP_PX_CORVETTE", "SHIP_PX_COLONY", "SHIP_PX_SCOUT", "SHIP_ZOOM_MIN", "SHIP_ZOOM_MAX",
    "SHIP_NATIVE_ZOOM_START", "MACHINE_ZOOM_END", "SHIP_MAX_PX", "SHIP_CLASS_MAX_PX", "TIER_SCALE",
    "FLEET_LEAD_CALIB", "LOD_ICON_ZOOM_MAX", "LOD_ICON_CALIB"];
  const declarations = ast.statements.filter(ts.isVariableStatement)
    .flatMap(n => [...n.declarationList.declarations])
    .filter(n => constants.includes(n.name.getText(ast)));
  assert.equal(declarations.length, constants.length);
  const methods = ["deepZoomPx", "shipSizePx", "shipHitRadius", "fleetHitRadius", "texFor",
    "fleetFamily", "fleetTier", "lodIconMarker", "fleetMarker"];
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
      ${members.map(n => n.getText(ast)).join("\n")}
      fitScale() { return 1; }
      drawnSize(ghost) {
        const marker = this.fleetMarker(ghost);
        return ${target.getText(ast)};
      }
    }`, { fleetExactCount, hashId });
  const renderer = new Renderer();
  for (const name of ["Convoy", "Raider", "Corvette", "Colony", "Scout", "Destroyer", "Cruiser",
    "Battleship", "Dreadnought", "Titan", "AuthorityFreighter", "Transport", "Builder",
    "Privateer", "PirateCorsair", "PirateBoarding", "IconFreighter", "IconRaider", "IconCorvette"]) {
    renderer[`tex${name}`] = { name, width: 256 };
  }
  renderer.texFleet = new Map(["freighter", "raider", "corvette", "scout"].flatMap(family =>
    ["wing", "squadron", "armada"].map(tier => [`${family}_${tier}`, { name: `${family}_${tier}`, width: 256 }])));
  return renderer;
}

const close = (actual, expected, message) =>
  assert.ok(Math.abs(actual - expected) < 1e-9, `${message}: ${actual} != ${expected}`);
const ghost = (kind, count = 1, extra = {}) => ({ id: "101", kind, own: true,
  composition: [{ kind, count }], count_class: "one", ...extra });
const zooms = [.5, .9, 1, 1.3, 1.6, 2.499, 2.5, 4, 8, 12, 12.01, 15, 18, 21, 23.99, 24, 48, 72, 96];
const normalCanvas = { raider: 64, convoy: 89.6, freighter: 89.6, corvette: 76.8,
  colony: 102.4, scout: 48, builder: 83.2, transport: 92.8, destroyer: 83.2,
  cruiser: 96, battleship: 112, dreadnought: 131.2, titan: 153.6 };
const deepCanvas = { raider: 96, convoy: 120, freighter: 120, corvette: 112,
  colony: 120, scout: 72, builder: 120, transport: 120, destroyer: 136,
  cruiser: 160, battleship: 184, dreadnought: 208, titan: 232 };
const changedClasses = new Set(["scout", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"]);
const current = fixture();
// Both reference curves run the real interpolation, not copied sizing math.
const unscaled = fixture({ INTERCEPTOR_MAP_SCALE: 1, PRIVATEER_MAP_SCALE: 1 });
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
    for (let step = 9; step <= 960; step++) {
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
for (const zoom of [.9, 4, 24, 96]) {
  current.scale = zoom;
  close(pickRadius(current, ghost("scout")), zoom < 12 ? 24 : 36, `Scout pick radius at ${zoom}`);
  close(pickRadius(current, ghost("titan")), current.drawnSize(ghost("titan")) / 2, `Titan pick radius at ${zoom}`);
}
// No pop at the 12/24 boundaries; LOD and fleet-count changes keep lead parity.
for (const kind of Object.keys(normalCanvas)) {
  for (const boundary of [1.6, 2.5, 12, 24]) {
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
close(current.drawnSize(ghost("raider", 8)), 96, "formation fallback");

// Actual asset bounds check: canvas padding must not erase the visible ladder.
// Alpha >= 128 excludes the faint exhaust/fringe, measuring nose-to-tail length.
const hulls = [
  ["scout", "scout_utility_ship.png"], ["raider", "raider_attack_ship.png"],
  ["corvette", "corvette_escort_ship.png"], ["convoy", "corporate_freighter.png"],
  ["destroyer", "destroyer_line_ship.png"], ["cruiser", "cruiser_line_ship.png"],
  ["battleship", "battleship_line_ship.png"], ["dreadnought", "dreadnought_line_ship.png"],
  ["titan", "titan_flagship.png"],
];
const lengths = new Map();
for (const [kind, file] of hulls) {
  const { data, info } = await sharp(new URL(`../public/art/ship_sprites/${file}`, import.meta.url).pathname)
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
assert.ok(lengths.get("titan") > lengths.get("convoy") * 1.9, "Titan reads near twice Freighter length");

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
const scaledReturn = "return this.deepZoomPx(indicator, maxPx) * mapScale;";
assert.ok(rendererSource.includes(scaledReturn));
const baseOnly = rendererSource.replace(scaledReturn, "return this.deepZoomPx(indicator * mapScale, maxPx);");
assert.throws(() => check(fixture({}, baseOnly)), /proportional size/);
console.log(`Map ship sizing: ${scenarios.length} fleet cases × ${zooms.length} zooms passed; per-class caps, monotonic growth, seams, formation/hit-radius parity, unchanged civilian/Interceptor/privateer curves, and rollback teeth passed.`);
console.log(`Deep-zoom visible lengths: ${[...lengths].map(([kind, length]) => `${kind} ${length.toFixed(1)}px`).join(", ")}`);

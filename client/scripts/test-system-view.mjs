// The System View's real layout math, picture derivation and scene: berths,
// the near zone, edge arrows, in-system battles, wrecks and sieges,
// with fog/participant gating. Pixi stands in as a recording mock; no server.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = (name) => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (name, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (id) => deps[id] ?? {}, performance: { now: () => 0 } });
  return exports;
};
const plain = (value) => JSON.parse(JSON.stringify(value));
const close = (a, b, label, eps = 1e-9) => assert.ok(Math.abs(a - b) < eps, `${label}: ${a} != ${b}`);

// --- Pixi mock (records draw calls; no GPU) --------------------------------
const point = () => ({ x: 0, y: 0, set(x, y = x) { this.x = x; this.y = y; } });
class Container {
  children = []; position = point(); scale = point(); pivot = point(); anchor = point();
  visible = true; alpha = 1; eventMode = "auto"; rotation = 0; zIndex = 0;
  addChild(...children) { for (const child of children) { child.removeFromParent?.(); child.parent = this; this.children.push(child); } return children[0]; }
  removeFromParent() { if (this.parent) this.parent.children = this.parent.children.filter((c) => c !== this); this.parent = null; }
  removeChildren() { const children = this.children; this.children = []; for (const child of children) child.parent = null; return children; }
  destroy() { this.destroyed = true; this.removeFromParent(); for (const child of [...this.children]) child.destroy(); }
}
class Graphics extends Container { commands = []; clear() { this.commands = []; return this; } }
for (const key of ["moveTo", "lineTo", "closePath", "fill", "stroke", "circle", "rect", "poly", "ellipse", "arc", "roundRect"]) {
  Graphics.prototype[key] = function (...args) { this.commands.push([key, ...args]); return this; };
}
class Sprite extends Container { constructor(texture) { super(); this.texture = texture; this.tint = 0xffffff; } }
class Text extends Container {
  constructor({ text, style }) { super(); this.text = text; this.style = style; }
  get width() { return String(this.text).length * 6; }
  get height() { return 12; }
}
const pixi = { Container, Graphics, Sprite, Text, TextStyle: class { constructor(style) { Object.assign(this, style); } },
  Texture: { EMPTY: { width: 1 } }, Assets: { load: () => new Promise(() => {}) } };

const prng = load("prng.ts");
const stars = load("stars.ts", { "./prng": prng });
const planetArt = load("planetart.ts", { "./planet-art.generated": load("planet-art.generated.ts") });
const protocol = load("protocol.ts");
const layout = load("systemlayout.ts");
const bodyFx = load("systembodyfx.ts", { "pixi.js": pixi, "./systemlayout": layout });
const starFx = load("systemstarfx.ts", { "pixi.js": pixi, "./systembodyfx": bodyFx, "./systemlayout": layout });
const history = load("battlehistory.ts");
const blockadeModule = load("core/derive/blockade.ts", { "./format": { fmtDur: secs => `${Math.round(secs)}s` } });
const traffic = load("systemtraffic.ts", { "./battlehistory": history, "./systemlayout": layout, "./core/derive/blockade": blockadeModule });
const trafficLayer = load("systemtrafficlayer.ts", { "pixi.js": pixi, "./protocol": protocol, "./systemlayout": layout, "./systembodyfx": bodyFx });
const systems = load("systemview.ts", { "pixi.js": pixi, "./prng": prng, "./stars": stars, "./planetart": planetArt,
  "./systemlayout": layout, "./systembodyfx": bodyFx, "./systemstarfx": starFx, "./systemtrafficlayer": trafficLayer,
  "./planet-structure-art": { hasOrbitalShipConstruction: (builds, body) => builds.some((b) => b.body_id === body && b.key === "cruiser") },
  "./icons": { label: (s) => s.replace(/_/g, " ") } });

// --- Layout: tilt, motion, near zone, docking slots -------------------------
assert.deepEqual(plain(layout.project({ x: 1, y: 1 })), { x: 1, y: layout.ORBIT_TILT }, "the orbital plane is foreshortened vertically");
const p = layout.planetPeriod(0.2);
close(p, layout.PLANET_PERIOD_S, "innermost period");
assert.ok(layout.planetPeriod(0.8) > layout.planetPeriod(0.4), "outer worlds turn slower (Kepler)");
close(layout.orbitalAngle(1, p, 0), 1, "phase at time zero");
close(layout.orbitalAngle(1, p, p), 1, "one period returns to the same phase", 1e-6);
assert.notEqual(layout.orbitalAngle(1, p, p / 4), 1, "bodies move between");
close(layout.orbitalAngle(1, p, Number.NaN), 1, "a missing clock poses at the seed");
assert.equal(layout.NEAR_ZONE_SU, layout.PLATFORM_COVER_SU, "the near zone reaches as far as the defenses do");
assert.equal(layout.nearRadius(0, 1.08, 1.5), 1.08, "inside the dock radius → the zone's inner edge");
assert.equal(layout.nearRadius(1e9, 1.08, 1.5), 1.5, "beyond the near zone → its outer edge");
close(layout.nearRadius((layout.DOCK_RADIUS_SU + layout.NEAR_ZONE_SU) / 2, 1.08, 1.5), 1.29,
  "drawn to scale: halfway out is halfway across the zone");
let last = 0;
for (const d of [300, 500, 800, 1100, 1290]) {
  const r = layout.nearRadius(d, 1.08, 1.5);
  assert.ok(r > last, "the zone is monotonic in distance");
  last = r;
}
assert.equal(layout.nearOuterFor(200, 2000, 254), layout.NEAR_OUTER_MIN, "never cramped below the minimum");
assert.equal(layout.nearOuterFor(5000, 5000, 254), layout.NEAR_OUTER_MAX, "never wider than the maximum");
assert.equal(layout.formatAge(0.4), "live");
assert.equal(layout.formatAge(42), "42s");
assert.equal(layout.formatAge(23 * 60), "23m");
assert.equal(layout.formatAge(3600 + 5 * 60), "1h 05m");
close(layout.pictureAge({ x: 300, y: 400 }, { x: 0, y: 0 }, 50), 10, "light age = distance / c");

const body = (id, extra = {}) => ({ id, parent: null, habitable: false, size: "medium", kind: "rocky", population: 0, structures: {}, ...extra });
const roster = [
  body(1, { habitable: true, kind: "terrestrial", population: 1.2, structures: { command_center: 1, warehouse: 2, garrison: 1 } }),
  body(2, { kind: "gas_giant", size: "large", structures: { naval_drydock: 2 } }),
  body(3, { parent: 2, structures: { defense_platform: 3 } }),
];
assert.equal(layout.berthAnchor("cruiser", roster), 2, "warships park at the orbital shipyard");
assert.equal(layout.berthAnchor("convoy", roster), 1, "freighters park at the warehouse world");
assert.equal(layout.berthAnchor("transport", roster), 1, "landing forces park at the garrison");
assert.equal(layout.berthAnchor("scout", roster), 1, "civil hulls fall back to the command world");
assert.equal(layout.mainWorld(roster), 1, "the populated world leads");
assert.equal(layout.defendedWorld(roster), 3, "a fight at the star is drawn at the platforms");
const rivalRoster = roster.map((b) => ({ ...b, population: 0, structures: {} }));
assert.equal(layout.berthAnchor("cruiser", rivalRoster), 1, "fogged rival structures fall back to public geography");
assert.equal(layout.mainWorld([]), null);

// --- Picture derivation ------------------------------------------------------
const star = { x: 10_000, y: 0 };
const system = { id: "9", name: "Tarsis", pos: star, band: "rich" };
const wireBody = (b) => ({ ...b, name: `Tarsis ${b.id}`, environment: "terran", geology: null, special: null, special_effect: null,
  habitat_capacity_mult: 1, population_growth_mult: 1, provisions_mult: 1, construction_time_mult: 1, mineral_extraction_mult: null,
  resource_slots: null, industrial_slots: null, infrastructure_slots: null, ship_build_time_mult: null, deposits: null,
  migration_policy: "closed", inbound_migrants: 0 });
const report = { id: "9", owner: "me", bodies: roster.map(wireBody), builds: [], blockade: null, ground: null, defense_tier: 2 };
const ghost = (id, kind, extra = {}) => ({ id, kind, owner: "me", own: true, pos: star, vel: { x: 0, y: 0 }, age: 0, jumped: false,
  route: null, passengers: {}, cargo: null, count_class: "one", composition: [{ kind, count: 1 }], signature: null, posture: null, ...extra });
const input = (over = {}) => ({
  system, dynamic: report, galaxy: { c: 100, siege_secs: 600, sensor_range: 80_000 },
  ghosts: [], battles: [], battleRecords: [], groundRecords: [], battleDismissed: new Set(),
  commandCenter: { x: 10_000, y: 3_000 }, playerId: "me", simTime: 1000,
  selectedShipId: null, selectedShipIds: new Set(), selectedBattleId: null, ...over,
});

const docked = ghost("f1", "cruiser", { docked: "E9" });
const dockedElsewhere = ghost("f2", "convoy", { docked: "E4" });
const hub = ghost("f3", "convoy", { docked: "hub" });
const near = ghost("r1", "raider", { own: false, owner: "rival", composition: null, count_class: "two_to_three",
  pos: { x: star.x + 600, y: 800 }, vel: { x: -10, y: -10 } });
const closing = ghost("r4", "raider", { own: false, owner: "rival", composition: null,
  pos: { x: star.x + 3000, y: 4000 }, vel: { x: -10, y: -10 } });
const loiter = ghost("r2", "destroyer", { own: false, owner: "rival", composition: null, pos: { x: star.x + 120, y: 50 } });
const allyLoiter = ghost("a1", "corvette", { own: false, ally: true, owner: "ally", pos: { x: star.x + 100, y: 0 } });
const inboundPirate = ghost("p1", "raider", { own: false, pirate: true, owner: "pirates", composition: null,
  pos: { x: star.x + 40_000, y: 0 }, vel: { x: -50, y: 0 } });
const passingPirate = ghost("p2", "raider", { own: false, pirate: true, owner: "pirates", composition: null,
  pos: { x: star.x + 40_000, y: 0 }, vel: { x: 0, y: 50 } });
const ownInbound = ghost("f4", "scout", { pos: { x: star.x - 30_000, y: 0 }, vel: { x: 40, y: 0 }, speed: 40,
  path: [{ pos: { x: star.x - 15_000, y: 0 } }, { pos: star }] });
const far = ghost("r3", "raider", { own: false, owner: "rival", pos: { x: star.x + 200_000, y: 0 }, vel: { x: -50, y: 0 } });
const leaving = ghost("r5", "raider", { own: false, owner: "rival", pos: { x: star.x + 3000, y: 0 }, vel: { x: 50, y: 0 } });
const longGone = ghost("r6", "raider", { own: false, owner: "rival", pos: { x: star.x + 40_000, y: 0 }, vel: { x: 50, y: 0 } });
const allyLeaving = ghost("a2", "corvette", { own: false, ally: true, owner: "ally", pos: { x: star.x - 3000, y: 0 }, vel: { x: -50, y: 0 } });
const vega = { name: "Vega", pos: { x: star.x, y: 30_000 } };
const ownLeaving = ghost("f5", "convoy", { pos: { x: star.x, y: 2000 }, vel: { x: 0, y: 40 }, speed: 40, path: [{ pos: vega.pos }] });
const ownParked = ghost("f6", "corvette", { pos: { x: star.x + 5000, y: 0 } });
const picture = traffic.deriveSystemPicture(input({
  ghosts: [docked, dockedElsewhere, hub, near, closing, loiter, allyLoiter, inboundPirate, passingPirate, ownInbound, far,
    leaving, longGone, allyLeaving, ownLeaving, ownParked],
  selectedShipId: "f1", selectedShipIds: new Set(["f1"]), places: [vega],
}));
const placement = (id) => picture.fleets.find((f) => f.id === id)?.placement ?? null;
assert.deepEqual(plain(placement("f1")), { mode: "berth", body: 2, slot: 0, slots: 1 }, "a berthed cruiser parks at its yard world");
assert.equal(placement("f2"), null, "berthed elsewhere is not at this star");
assert.equal(placement("f3"), null, "the Hub's berths are not this star's");
assert.equal(placement("r1").mode, "near");
close(placement("r1").distance, 1000, "the near zone keeps the true distance");
close(placement("r1").bearing, Math.atan2(800, 600), "the near zone keeps the true bearing");
assert.equal(placement("r4").mode, "inbound", "beyond the near zone, a hostile closing in is an edge arrow");
close(placement("r4").eta, (5000 - layout.DOCK_RADIUS_SU) / 14, "its ETA from served motion");
assert.equal(placement("r2").mode, "cordon", "a hostile loitering at the star rings its main world");
assert.equal(placement("r2").body, 1);
assert.equal(placement("a1").mode, "near", "an ally at the star is no cordon");
assert.equal(placement("p1").mode, "inbound", "a pirate closing on the star is an inbound chevron");
close(placement("p1").eta, (40_000 - layout.DOCK_RADIUS_SU) / 50, "ETA from served motion");
assert.equal(placement("p2"), null, "a contact not closing is not inbound");
assert.equal(placement("f4").mode, "inbound", "own fleets announce their own destination");
close(placement("f4").eta, 30_000 / 40, "own ETA from the flight plan");
assert.equal(placement("r3"), null, "beyond sensor range is not drawn");
assert.deepEqual(plain(placement("r5")), { mode: "outbound", bearing: 0, eta: null, to: null },
  "a hostile that just left is an outward edge arrow");
assert.equal(placement("r6"), null, "one that left long ago is the galaxy map's business");
assert.equal(placement("a2"), null, "allies get no edge arrows");
assert.deepEqual(plain(placement("f5")), { mode: "outbound", bearing: Math.PI / 2, eta: 28_000 / 40, to: "Vega" },
  "an own fleet leaving names its destination and arrival from its flight plan");
assert.equal(placement("f6"), null, "a fleet parked beyond the near zone stays on the galaxy map");
assert.ok(picture.fleets.find((f) => f.id === "f1").selected);
assert.equal(picture.fleets.find((f) => f.id === "r1").tone, "rival");
assert.equal(picture.fleets.find((f) => f.id === "p1").tone, "pirate");
assert.equal(picture.siege, null, "no blockade, no landing → no siege");
assert.equal(picture.platformCover, true, "own defense platforms draw their umbrella");
close(picture.age, 30, "the picture's light age from command");
assert.deepEqual(plain(traffic.deriveSystemPicture(input({ ghosts: [docked, near] }))),
  plain(traffic.deriveSystemPicture(input({ ghosts: [docked, near] }))), "deterministic");

// §belts: your anchored station and the barges working its belt sit on the band;
// a barge still flying in, and a rival's station, do not.
const depot = ghost("s1", "mining_belt_station", { pos: { x: star.x + 300, y: 0 },
  anchor: { system: "9", body: 20, phase: "anchored", since: 0 } });
const cutter = ghost("b1", "mining_barge", { pos: { x: star.x + 200, y: 100 },
  industry: { kind: "mine", system: "9", body: 20, ore: "bornite_ore", mined: 4, fraction: 0, status: "mining" } });
const inbound = ghost("b2", "mining_barge", { pos: { x: star.x + 9000, y: 0 },
  industry: { kind: "mine", system: "9", body: 20, ore: "bornite_ore", mined: 0, fraction: 0, status: "travelling" } });
const theirs = ghost("s2", "mining_belt_station", { own: false, owner: "rival", pos: { x: star.x + 300, y: 50 } });
const banded = traffic.deriveSystemPicture(input({ ghosts: [depot, cutter, inbound, theirs] }));
const band = (id) => banded.fleets.find((f) => f.id === id)?.placement ?? null;
assert.deepEqual(plain(band("s1")), { mode: "belt", body: 20, slot: 1, slots: 2 }, "the station holds the band");
assert.deepEqual(plain(band("b1")), { mode: "belt", body: 20, slot: 0, slots: 2 }, "beside its barge");
assert.notEqual(band("b2")?.mode, "belt", "a barge still on its way is not on the band");
assert.notEqual(band("s2").mode, "belt", "a rival's anchor is private: just a hull at the star");

// Slots: selected first, then own, then hull kind; per world.
const fleetMany = ["destroyer", "convoy", "scout", "cruiser"].map((kind, i) => ghost(`m${i}`, kind, { docked: "9" }));
const many = traffic.deriveSystemPicture(input({ ghosts: fleetMany, selectedShipId: "m2", selectedShipIds: new Set(["m2"]) }));
const yardSlots = many.fleets.filter((f) => f.placement.body === 2).map((f) => [f.id, f.placement.slot, f.placement.slots]);
assert.deepEqual(plain(yardSlots), [["m0", 1, 2], ["m3", 0, 2]].sort((a, b) => a[0].localeCompare(b[0])), "warships share the yard world's ring");
assert.equal(many.fleets.find((f) => f.id === "m2").placement.slot, 1, "selection never reshuffles a world's slots");
const crowdSel = Array.from({ length: 6 }, (_, i) => ghost(`s${i}`, "convoy", { docked: "9" }));
const lifted = traffic.deriveSystemPicture(input({ ghosts: crowdSel, selectedShipId: "s5", selectedShipIds: new Set(["s5"]) }));
assert.equal(lifted.fleets.find((f) => f.id === "s5").placement.slot, 2, "a selected fleet past the cap is lifted into view");

// Battles and wrecks at this star; dismissed wrecks are gone; others are elsewhere.
const count = (kind, exact) => ({ kind, exact, class: "one" });
const record = (id, pos, outcome, systemId = "9") => ({ id, pos, system: systemId, started_at: 900, raid: false, fidelity: "participant",
  own_side: 1, sides: [{ corp: "rival", posture: null, platform_tiers: 0, initial: [] }, { corp: "me", posture: null, platform_tiers: 2, initial: [] }],
  rounds: [{ tick: 1, counts: [[], []], kills: [[count("raider", 1)], []], dealt: [10, 12], notes: [] }], light_frontier_tick: 1, outcome });
const fighter = ghost("b1", "corvette", { pos: { x: star.x + 200, y: 0 } });
const attacker = ghost("b2", "raider", { own: false, owner: "rival", pos: { x: star.x + 250, y: 0 } });
const fight = traffic.deriveSystemPicture(input({
  ghosts: [fighter, attacker],
  battles: [{ id: "orbit", pos: { x: star.x + 200, y: 0 }, age: 3, started_at: 950, own: true, participants: ["b1", "b2"] },
    { id: "close", pos: { x: star.x + 1200, y: 0 }, age: 9, started_at: 950, own: false, participants: [] },
    { id: "beyond", pos: { x: star.x + 6000, y: 0 }, age: 9, started_at: 950, own: false, participants: [] },
    { id: "elsewhere", pos: { x: star.x + 90_000, y: 0 }, age: 9, started_at: 950, own: false, participants: [] }],
  battleRecords: [record("orbit", { x: star.x + 200, y: 0 }, null), record("wreck", { x: star.x + 5000, y: 0 }, "target_destroyed"),
    record("gone", { x: star.x + 7000, y: 0 }, "target_destroyed"), record("other", { x: 0, y: 0 }, "target_destroyed", "4")],
  battleDismissed: new Set([history.battleRecordMarkKey("gone")]),
}));
assert.deepEqual(plain(fight.battles.map((b) => b.id).sort()), ["close", "orbit"],
  "only fights at this star: tagged, or untagged inside the near zone");
assert.deepEqual(plain(fight.battles.find((b) => b.id === "orbit").anchor), { kind: "body", body: 3 }, "inside platform cover → the platform world");
assert.deepEqual(plain(fight.battles.find((b) => b.id === "close").anchor), { kind: "body", body: 3 }, "so is the zone's edge");
assert.ok(fight.battles.find((b) => b.id === "orbit").intensity > 0.5, "arrived kills and damage brighten the fight");
assert.deepEqual(plain(fight.fleets.map((f) => [f.id, f.placement.mode, f.placement.side])), [["b1", "battle", 1], ["b2", "battle", 0]],
  "participants face off by the record's sides");
assert.deepEqual(plain(fight.wrecks.map((w) => w.id)), ["wreck"], "concluded, served, not dismissed, at this star");
assert.equal(fight.wrecks[0].anchor.kind, "near", "a fight tagged to this star beyond the defenses sits at the zone's edge");

// Sieges: participant-only fields; a third party sees none.
const besieged = { ...report, blockade: { by: "rival", since: 100, by_me: false, siege_since: 700 }, ground: { suppression: 0.4 } };
const siege = traffic.deriveSystemPicture(input({ dynamic: besieged, ghosts: [loiter],
  groundRecords: [{ id: "landing", system: "9", outcome: null, attacking: false, rounds: [{ marines_frac: 0.6, defenders_frac: 0.5, suppression: 0.4 }] }] })).siege;
assert.equal(siege.target, 1, "the siege is fought over the main world");
close(siege.progress, 0.5, "siege clock from the served onset");
close(siege.suppression, 0.4, "garrison pinned");
assert.deepEqual(plain(siege.landing), { id: "landing", marines: 0.6, defenders: 0.5 });
assert.equal(traffic.deriveSystemPicture(input({ dynamic: { ...report, owner: "rival", blockade: null } })).siege, null,
  "a third party's report carries no blockade, so no siege is drawn");

// --- The scene: traffic drawn, picked, capped; scenery stays inert -----------
const bodiesWire = roster.map(wireBody);
const visual = systems.buildVisualSystem(system, bodiesWire);
// §marginal: only a world colonists can settle wears the lush art; the rule
// that keeps one garden per home leaves a Marginal world, which must look it.
const world = (id, kind, habitable, environment) => ({ ...wireBody(body(id, { kind, habitable })), environment });
const looks = systems.buildVisualSystem(system, [
  world(11, "terrestrial", true, "terran"), world(12, "terrestrial", false, "marginal"), world(13, "terrestrial", false, "hostile"),
  world(14, "terrestrial", false, "uninhabitable"), world(15, "ocean", true, "gaia"), world(16, "ocean", false, "marginal"),
]);
assert.deepEqual(plain(looks.planets.map((p) => p.kind)), ["terrestrial", "marginal", "marginal", "barren", "ocean", "marginal_ocean"]);
const marginalScene = new systems.SystemViewScene();
marginalScene.setSystem(looks, null);
assert.match(marginalScene.planetDetail(looks.planets[1]).kindLabel, /Marginal world · can't be settled/);
assert.match(marginalScene.planetDetail(looks.planets[5]).kindLabel, /Marginal ocean · can't be settled/);
assert.ok(visual.planets.every((planet) => planet.radius <= 0.078), "no world outgrows the star");
const scene = new systems.SystemViewScene();
scene.setSystem(visual, null);
scene.layout(1200, 800, { x: 0, y: 40, w: 900, h: 640 });
scene.setDynamic(bodiesWire, [{ key: "cruiser", body_id: 2 }], true);
scene.setReport({ ...report, bodies: bodiesWire, builds: [{ key: "cruiser", body_id: 2, complete_time: null }], converters: [], food_state: "rationing",
  storage_used: 0, storage_cap: 100 }, true);
const art = (g) => ({ tex: { width: 128 }, mult: 1 });
scene.setShipArt(art, null);
scene.setPicture(picture);
scene.update("me", "me", 1000, 0);
scene.update("me", "me", 1100, 0);
const yardWorld = scene.bodyScreen("2");
const cruiser = scene.traffic.fleetScreen("f1");
assert.ok(cruiser && Math.hypot(cruiser.x - yardWorld.sx, cruiser.y - yardWorld.sy) < yardWorld.r + 40, "the cruiser moors beside its yard world");
assert.deepEqual(plain(scene.pick(cruiser.x, cruiser.y)), { kind: "fleet", id: "f1" }, "fleets win the click");
assert.match(scene.hover(cruiser.x, cruiser.y), /Cruiser · berthed here/);
assert.equal(scene.modelCandidates().length, 1, "only the selected hull may go live 3D");
assert.ok(scene.modelCandidates()[0].canvasPx >= 32);
const home = scene.bodyScreen("1");
assert.equal(scene.pick(home.sx, home.sy).kind, "body", "worlds remain clickable under no traffic");
assert.ok(scene.nodes.find((node) => node.id === "2").orbitals.size > 0, "the orbital yard orbits its world");
assert.ok(scene.chipTexts.some((t) => t.visible && /rationing/.test(t.text)), "owner chips name food trouble");
assert.ok(scene.chipTexts.some((t) => t.visible && /⚒ 1/.test(t.text)), "owner chips count construction");
// §power: a browning-out planet names its share on its own chip.
scene.setReport({ ...report, bodies: bodiesWire.map((b) => b.id === 2 ? { ...b, power: { demand: 4.4, supply: 3, power: .68, burn: 0, fuel: "fuel", no_fuel: false } } : b),
  builds: [], converters: [], food_state: "well_supplied", storage_used: 0, storage_cap: 100 }, true);
scene.update("me", "me", 1105, 0);
assert.ok(scene.chipTexts.some((t) => t.visible && /power 68%/.test(t.text)), "owner chips name a brownout");
// The near zone draws no rings and no label; the defenses' reach shows only
// while the pointer rests on the world that carries them.
const coverStrokes = () => scene.coverGfx.commands.filter(([op]) => op === "stroke").length;
assert.ok(!scene.hud.children.some((child) => /LOCAL SPACE/.test(String(child.text ?? ""))), "no local-space label");
scene.setPicture(traffic.deriveSystemPicture(input({ ghosts: [] })));
scene.update("me", "me", 1110, 0);
assert.equal(coverStrokes(), 0, "no ring marks the near zone");
const platformWorld = scene.bodyScreen("3");
assert.match(scene.hover(platformWorld.sx, platformWorld.sy), /defenses reach the dashed ring/);
scene.update("me", "me", 1120, 0);
assert.ok(coverStrokes() > 0, "pointing at the defended world shows the defenses' reach");
assert.doesNotMatch(scene.hover(home.sx, home.sy), /defenses/, "a world without defenses shows none");
scene.update("me", "me", 1130, 0);
assert.equal(coverStrokes(), 0);
scene.hover(platformWorld.sx, platformWorld.sy);
scene.clearHover();
scene.update("me", "me", 1140, 0);
assert.equal(coverStrokes(), 0, "leaving the map hides it again");
scene.setPicture(picture);
scene.setReport({ ...report, owner: "rival", bodies: bodiesWire }, false);
scene.update("rival", "me", 1200, 0);
assert.ok(scene.chipTexts.every((t) => !t.visible), "a rival system shows no owner chips");
const strokes = (color) => scene.overlay.commands.filter(([op, style]) => op === "stroke" && style?.color === color).length;
assert.ok(strokes(0xff7a6b) > 0, "a rival's star keeps its red ring");
scene.update("me", "me", 1250, 0);
assert.equal(strokes(0x4fc3ff), 0, "your own star shows no ownership ring inside its system");
assert.equal(strokes(0xff7a6b), 0);
// Zoom clamps and recentres.
assert.equal(scene.zoomAt(home.sx, home.sy, 50), systems.SYSTEM_ZOOM_MAX);
assert.equal(scene.zoomAt(home.sx, home.sy, 0.001), 1);
assert.deepEqual(plain(scene.focusTarget), { x: 0, y: 0 }, "fully zoomed out recentres on the star");
// Dragging pans the orrery at any zoom; the worlds follow the cursor and stay clickable.
scene.update("me", "me", 1260, 0);
assert.equal(scene.atRest(), true);
const atFit = scene.bodyScreen("1");
scene.panBy(120, -40);
scene.update("me", "me", 1261, 0);
const panned = scene.bodyScreen("1");
assert.ok(Math.abs(panned.sx - atFit.sx - 120) < 1e-6 && Math.abs(panned.sy - atFit.sy + 40) < 1e-6, "the scene moves with the drag");
assert.equal(scene.atRest(), false, "a panned orrery is not at its fit");
assert.equal(scene.pick(panned.sx, panned.sy).kind, "body", "a panned world is still clickable");
// The camera cannot be dragged away from the system: the near zone bounds it.
scene.panBy(-1e7, 1e7);
assert.ok(Math.abs(scene.focusTarget.x) <= scene.nearOuter + 1e-9);
assert.ok(Math.abs(scene.focusTarget.y) <= scene.nearOuter + 1e-9);
assert.ok(Math.abs(scene.focusTarget.x - scene.focus.x) < 1e-9, "a drag tracks the cursor, it does not ease");
// A drag takes the camera from a followed world; Fit recentres everything.
scene.resetZoom(true);
const framed = scene.bodyScreen("1");
scene.zoomAt(framed.sx, framed.sy, 2);
assert.ok(scene.follow, "zooming toward a world follows it");
scene.panBy(30, 0);
assert.equal(scene.follow, null, "dragging lets the followed world go");
scene.resetZoom(true);
assert.equal(scene.atRest(), true);
assert.deepEqual(plain(scene.focus), { x: 0, y: 0 });
// Docked hulls fold into a token past the per-world cap at fit zoom.
const crowd = Array.from({ length: 7 }, (_, i) => ghost(`c${i}`, "convoy", { docked: "9" }));
scene.setPicture(traffic.deriveSystemPicture(input({ ghosts: crowd })));
scene.update("me", "me", 1300, 0);
assert.ok(scene.traffic.textPool.some((t) => t.visible && t.text === "+4 docked"), "seven berths at fit zoom → three drawn + '+4 docked'");
// Star FX: a pulsar's flash is a soft additive glow, never a magnified polygon,
// and flare shapes tessellate at screen-like size before scaling back down.
{
  const fx = load("systemstarfx.ts", { "pixi.js": pixi, "./systembodyfx": { glowSpriteTexture: () => ({ width: 128 }) }, "./systemlayout": layout });
  const pulsar = new fx.StarFx();
  pulsar.configure("neutron_star", 0.085);
  pulsar.update(0, null); // a beam faces the viewer: full flash
  const flash = pulsar.over.children.find((c) => c instanceof Sprite && c.blendMode === "add");
  assert.ok(flash?.visible && Math.abs(flash.alpha - 0.6) < 1e-9, "the pulsar flashes as a soft additive glow");
  const shapes = pulsar.over.children.find((c) => c instanceof Graphics);
  assert.equal(shapes.commands.filter(([op]) => op === "circle").length, 0, "no disc is drawn for the flash");
  pulsar.update(7 / 4, null); // beams side-on
  assert.ok(flash.alpha < 1e-6, "dark between sweeps");
  const magnetar = new fx.StarFx();
  magnetar.configure("magnetar", 0.085);
  magnetar.update(1, null);
  const ring = magnetar.over.children.find((c) => c instanceof Graphics);
  const [, , , radius] = ring.commands.find(([op]) => op === "circle");
  assert.ok(radius > 50 && Math.abs(radius * ring.scale.x - 0.085 * (1.1 + (1 / 6.5) * 2.4)) < 1e-9,
    "the flare shell is built large and scaled to the star");
  assert.equal(magnetar.over.children.find((c) => c instanceof Sprite)?.visible, false, "only a pulsar flashes");
}
// Scenery mode never draws traffic, chips or labels.
const scenery = new systems.SystemViewScene({ sceneryOnly: true });
scenery.setSystem(visual, null);
scenery.layout(1068, 672);
scenery.setPicture(picture);
scenery.setReport(report, true);
scenery.update("me", "me", 0, 0);
assert.equal(scenery.traffic.root.visible, false);
assert.equal(scenery.pick(500, 300), null);
assert.equal(scenery.labels.children.length, 0);
assert.equal(scenery.markers.children.length, 0);

// --- The galaxy map's orbit preview draws the System View's own worlds -------
// The real drawApproachHint from render.ts, compiled alone with stand-ins for
// the renderer's fields: the same art, size and star-facing light as the
// System View, far-side worlds behind the star, grey dots only before the art.
{
  const renderSource = source("render.ts");
  const rast = ts.createSourceFile("render.ts", renderSource, ts.ScriptTarget.Latest, true);
  const owner = rast.statements.find((n) => ts.isClassDeclaration(n) && n.name?.text === "Renderer");
  const hintMethod = owner.members.find((n) => n.name?.getText(rast) === "drawApproachHint");
  const constant = (name) => rast.statements.filter(ts.isVariableStatement).flatMap((n) => [...n.declarationList.declarations])
    .find((d) => d.name.getText(rast) === name).getText(rast);
  const exportsHint = {};
  vm.runInNewContext(ts.transpileModule(`const ${constant("SCHEMATIC_GROW_FROM")}; const ${constant("SYSTEM_ZOOM_FACTOR")};
    exports.HintRenderer = class { approachVisuals = new Map(); galaxy = { c: 100 };
      constructor(scene) { this.systemScene = scene; }
      ${hintMethod.getText(rast)} }`, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText,
  { exports: exportsHint, Sprite, Texture: pixi.Texture, buildVisualSystem: systems.buildVisualSystem, liveSimTime: () => 0,
    orbitalAngle: layout.orbitalAngle, planetPeriod: layout.planetPeriod, ORBIT_TILT: layout.ORBIT_TILT,
    terminatorTexture: () => ({ width: 128 }), towardStarRotation: bodyFx.towardStarRotation, planetArtwork: planetArt.planetArtwork });
  let artLoaded = true;
  const baseScale = 300;
  const scene = { starVisibleDiameterPx: () => 0.17 * baseScale, planetTexture: (kind) => artLoaded ? { width: 512, kind } : null };
  const hint = new exportsHint.HintRenderer(scene);
  const preview = { worlds: [], worldsBack: new Container(), worldsFront: new Container() };
  const star = { x: 500, y: 400 };
  const planets = systems.buildVisualSystem(system, bodiesWire).planets;
  const drawn = new Graphics();
  hint.drawApproachHint(drawn, preview, system, bodiesWire, star, 1, 96, { commandCenter: null });
  const k = baseScale * 0.35;
  assert.equal(drawn.commands.filter(([op]) => op === "circle").length, 0, "no grey dots once the art is in");
  assert.ok(preview.worldsBack.visible && preview.worldsFront.visible);
  assert.equal(preview.worlds.filter((w) => w.art.visible).length, planets.length, "one world per planet; moons wait for the System View");
  planets.forEach((planet, i) => {
    const world = preview.worlds[i];
    const art = planetArt.planetArtwork(planet.kind);
    assert.equal(world.art.texture.kind, planet.kind, `${planet.name}: its own kind's art`);
    close(world.art.scale.x, (2 * planet.radius * k) / (art.visualRatio * 512), `${planet.name}: the System View's size at the doorway`);
    assert.equal(world.art.parent, Math.sin(planet.angle) < 0 ? preview.worldsBack : preview.worldsFront,
      `${planet.name}: depth-sorted against the star like the System View`);
    const toStar = Math.atan2(star.y - world.art.position.y, star.x - world.art.position.x);
    close(world.art.rotation, bodyFx.towardStarRotation(toStar), `${planet.name}: lit from its star`);
    close(world.shade.rotation, toStar + Math.PI, `${planet.name}: night side away from the star`);
  });
  artLoaded = false;
  const before = new Graphics();
  hint.drawApproachHint(before, preview, system, bodiesWire, star, 1, 96, { commandCenter: null });
  assert.equal(before.commands.filter(([op]) => op === "circle").length, planets.length, "grey dots until the art loads");
  assert.ok(preview.worlds.every((w) => !w.art.visible) && !preview.worldsFront.visible, "and no stale art");
}

// §orbital-defense: a world's station is drawn once in orbit, larger and a
// little farther out at each level; legacy platform tiers keep their ring.
const stationOf = level => bodyFx.orbitalItems({ orbital_defense: level }, 7).filter(item => item.kind === "orbital_defense");
for (let level = 1; level <= 5; level++) {
  const [item, ...extra] = stationOf(level);
  assert.equal(extra.length, 0, "one station per world");
  assert.equal(item.tier, level);
  if (level > 1) {
    const [lower] = stationOf(level - 1);
    assert.ok(item.px > lower.px && item.orbit > lower.orbit, `level ${level} outgrows level ${level - 1}`);
  }
}
assert.equal(stationOf(9)[0].tier, 5, "the ladder tops out at the Star Fortress");
assert.equal(stationOf(0).length, 0, "no station, nothing in orbit");
assert.equal(bodyFx.orbitalItems({ defense_platform: 3 }, 7).filter(item => item.kind === "defense_platform").length, 3,
  "legacy platform tiers keep their own ring");

// --- §belts: charted belts arrive as bodies and sit between planet orbits ----
{
  const belt = (id, kind, radius, deposits) => ({ ...wireBody(body(id, { kind: "asteroid_belt", size: "large",
    belt: { kind, radius, width: 0.05 } })), deposits });
  const planetsWire = [10, 11, 12].map((id) => wireBody(body(id, { size: "large" })));
  const wire = [...planetsWire, belt(20, "metallic", 0.31, [{ resource: "bornite_ore", richness: 0.24, reserves: null }]),
    belt(21, "icy", 0.32, [{ resource: "volatiles", richness: 0.3, reserves: null }])];
  const vis = systems.buildVisualSystem(system, wire);
  assert.equal(vis.planets.length, 3, "a belt is never drawn as a planet");
  assert.deepEqual(plain(vis.asteroidBelts.map((b) => [b.id, b.kind])), [["20", "metallic"], ["21", "icy"]]);
  const orbits = vis.planets.map((p) => p.orbitRadius).sort((x, y) => x - y);
  for (const b of vis.asteroidBelts) {
    const inside = orbits.findIndex((r) => r > b.radius);
    assert.ok(inside > 0, "each belt lies between two planet orbits");
    assert.ok(b.radius - b.width / 2 > orbits[inside - 1] && b.radius + b.width / 2 < orbits[inside], "its band clears both orbits");
  }
  assert.notEqual(vis.asteroidBelts[0].radius, vis.asteroidBelts[1].radius, "two belts take two gaps");
  assert.equal(systems.bodyArtUrl("9", wire[3]), "/art/derived/belt-chunks/v1/metallic-512.png", "a belt's art is its type's rock");
  assert.equal(systems.beltChunkUrl("stony"), systems.CHUNK_ART_URL, "the Stony type keeps the original chunk");
  assert.equal(systems.beltChunkUrl("icy"), "/art/derived/belt-chunks/v1/icy-256.png");
  assert.equal(layout.mainWorld(wire.map((b) => ({ ...b, size: b.kind === "asteroid_belt" ? "huge" : "small" }))), 10,
    "a belt is never the main world, however large");
  const beltScene = new systems.SystemViewScene();
  beltScene.setSystem(vis, null);
  beltScene.layout(1200, 800, { x: 0, y: 40, w: 900, h: 640 });
  beltScene.setDynamic(wire, [], true);
  beltScene.setShipArt(g => ({ tex: { width: 128, kind: g.kind }, mult: 1 }), null);
  beltScene.setPicture({ ...banded, fleets: banded.fleets.filter(f => ["s1", "b1"].includes(f.id))
    .map(f => ({ ...f, selected: f.id === "s1" })) });
  beltScene.update("me", "me", 1000, 0);
  assert.equal(beltScene.traffic.markers.get("s1").sprite.texture.kind, "mining_belt_station", "belt station requests its own hull art");
  assert.equal(beltScene.traffic.markers.get("b1").sprite.texture.kind, "mining_barge", "working barge keeps its own hull art");
  assert.equal(beltScene.modelCandidates()[0].kind, "mining_belt_station", "selected belt station admits its own live model");
  const b0 = vis.asteroidBelts[0];
  const ox = beltScene.worldRoot.position.x, oy = beltScene.worldRoot.position.y, k = beltScene.sceneScale;
  // A point on the belt's band, away from any world's disc.
  let hit = null;
  for (let a = 0; a < Math.PI * 2 && !hit; a += 0.05) {
    const sx = ox + Math.cos(a) * b0.radius * k, sy = oy + Math.sin(a) * b0.radius * k * layout.ORBIT_TILT;
    const pick = beltScene.pick(sx, sy);
    if (pick?.kind === "body" && pick.detail.id === "20") hit = { sx, sy, pick };
  }
  assert.ok(hit, "a belt's band is clickable");
  assert.equal(hit.pick.detail.kindLabel, "Metallic belt");
  assert.match(beltScene.hover(hit.sx, hit.sy), /Metallic belt · bornite ore ×0\.24 · open belt/);
  // §pockets: a find landing in the report glints and joins the hover at once.
  const found = wire.map((b) => b.id === 20 ? { ...b, pockets: [{ resource: "titanium_ore", richness: 0.9, reserves: 1800 }] } : b);
  beltScene.setReport({ id: vis.systemId, owner: null, bodies: found, builds: [] }, false);
  assert.equal(vis.asteroidBelts[0].pockets.length, 1);
  assert.match(beltScene.hover(hit.sx, hit.sy), /×0\.24 · 1 pocket found · open belt/);
  beltScene.setReport({ id: vis.systemId, owner: null, bodies: wire, builds: [] }, false);
  assert.doesNotMatch(beltScene.hover(hit.sx, hit.sy), /pocket/);
  assert.equal(beltScene.pick(ox + 2, oy + 2), null, "the empty middle is not a belt");
  // Each type draws its own chunk art untinted; only a missing texture falls
  // back to the Stony chunk recoloured.
  const chunks = beltScene.beltChunks.children;
  assert.ok(chunks.length === 0 || chunks.every((c) => c.tint === 0xffffff || c.tint === systems.BELT_META.metallic.tint
    || c.tint === systems.BELT_META.icy.tint), "chunks use their type's art or its fallback tint");
}

console.log("System view: tilted layout, Kepler motion, near zone to scale, docking slots, fog-safe picture (berths, near zone, cordon, inbound and outbound arrows, battles, wrecks, sieges), hover-only defense reach, picks, caps, 3D candidates, galaxy orbit preview art, stations by level and inert scenery, and charted belts between orbits passed.");

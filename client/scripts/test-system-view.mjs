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
const shipart = load("shipart.ts", { "./ship-art.generated": load("ship-art.generated.ts") });
const trafficLayer = load("systemtrafficlayer.ts", { "pixi.js": pixi, "./protocol": protocol, "./systemlayout": layout, "./systembodyfx": bodyFx, "./shipart": shipart });
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
// §real-coordinates: one true scale, a unit = SYSTEM_UNIT_SU, held at the frame's edge.
assert.equal(layout.SYSTEM_UNIT_SU, 860, "mirrors the sim's System View unit");
close(layout.trueRadius(430, 1.5), 0.5, "to scale");
assert.equal(layout.trueRadius(1e9, 1.5), 1.5, "beyond the frame → its edge");
close(layout.NEAR_OUTER_MAX, layout.NEAR_ZONE_SU / layout.SYSTEM_UNIT_SU, "the frame reaches the defenses' reach at most");
// Served orbits put worlds (and the ships at them) at true places.
const orbiting = [{ id: 1, parent: null, kind: "rocky", orbit: { radius: 0.5, angle: 0, period: 100 } },
  { id: 2, parent: 1, kind: "ice", orbit: { radius: 0.1, angle: Math.PI / 2, period: 0 } }];
close(layout.bodyOffset(orbiting, 1, 0).x, 0.5, "a planet at its phase");
close(layout.bodyOffset(orbiting, 1, 25).y, -0.5, "a quarter turn on: counter-clockwise on screen");
close(layout.bodyOffset(orbiting, 2, 0).y, 0.1, "a moon rides its planet");
const starAt = { x: 1000, y: 0 };
assert.equal(layout.worldAt(orbiting, starAt, { x: 1000 + 430 + 10, y: 0 }, 0), 1, "a fleet beside a world is at it");
assert.equal(layout.worldAt(orbiting, starAt, { x: 1000 + 430, y: 86 }, 0), 2, "the nearest world wins");
assert.equal(layout.worldAt(orbiting, starAt, { x: 1000 + 200, y: 0 }, 0), null, "between worlds is at none");
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
// §merchant: the free trader berths beside the home it serves, not a depot.
const depotRoster = [body(1, { habitable: true, kind: "terrestrial", population: 1.2, structures: { command_center: 1 } }),
  body(2, { kind: "gas_giant", size: "large", structures: { orbital_warehouse: 1 } })];
assert.equal(layout.fleetBerth({ kind: "convoy" }, depotRoster), 2, "a freighter parks at the orbital warehouse");
assert.equal(layout.fleetBerth({ kind: "convoy", merchant: true }, depotRoster), 1, "the free trader orbits the home world");

// --- Picture derivation ------------------------------------------------------
const star = { x: 10_000, y: 0 };
const system = { id: "9", name: "Tarsis", pos: star, band: "rich" };
// §real-coordinates: served orbits, frozen (period 0) so places are exact: world
// 1 east of the star, world 2 south, moon 3 beside world 2.
const ORBITS = { 1: { radius: 0.4, angle: 0, period: 0 }, 2: { radius: 0.8, angle: Math.PI / 2, period: 0 },
  3: { radius: 0.08, angle: 0, period: 0 } };
const wireBody = (b) => ({ orbit: ORBITS[b.id], ...b, name: `Tarsis ${b.id}`, environment: "terran", geology: null, special: null, special_effect: null,
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
const atWorld1 = { x: star.x + 0.4 * 860, y: 0 };
const loiter = ghost("r2", "destroyer", { own: false, owner: "rival", composition: null, pos: atWorld1 });
const allyLoiter = ghost("a1", "corvette", { own: false, ally: true, owner: "ally", pos: { x: star.x + 100, y: 0 } });
const allyVisit = ghost("a3", "corvette", { own: false, ally: true, owner: "ally", pos: { x: star.x, y: 0.8 * 860 + 5 } });
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
  ghosts: [docked, dockedElsewhere, hub, near, closing, loiter, allyLoiter, allyVisit, inboundPirate, passingPirate, ownInbound, far,
    leaving, longGone, allyLeaving, ownLeaving, ownParked],
  selectedShipId: "f1", selectedShipIds: new Set(["f1"]), places: [vega],
}));
const placement = (id) => picture.fleets.find((f) => f.id === id)?.placement ?? null;
assert.deepEqual(plain(placement("f1")), { mode: "berth", body: 2, slot: 0, slots: 1 }, "a berthed cruiser parks at its yard world");
assert.equal(placement("f2"), null, "berthed elsewhere is not at this star");
const trader = ghost("t1", "convoy", { own: false, owner: "tca", tca: true, merchant: true, docked: "E9" });
assert.deepEqual(plain(traffic.deriveSystemPicture(input({ dynamic: { ...report, bodies: depotRoster.map(wireBody) }, ghosts: [trader] }))
  .fleets[0].placement), { mode: "berth", body: 1, slot: 0, slots: 1 }, "the docked free trader orbits the home world, not the star");
assert.equal(placement("f3"), null, "the Hub's berths are not this star's");
assert.equal(placement("r1").mode, "near");
close(placement("r1").distance, 1000, "the near zone keeps the true distance");
close(placement("r1").bearing, Math.atan2(800, 600), "the near zone keeps the true bearing");
assert.equal(placement("r4").mode, "inbound", "beyond the near zone, a hostile closing in is an edge arrow");
close(placement("r4").eta, (5000 - layout.DOCK_RADIUS_SU) / 14, "its ETA from served motion");
assert.equal(placement("r2").mode, "cordon", "a hostile at rest at a world rings that world");
assert.equal(placement("r2").body, 1);
assert.equal(placement("a1").mode, "near", "between worlds: its true place");
assert.deepEqual(plain(placement("a3")), { mode: "cordon", body: 2, slot: 0, slots: 1 }, "a visitor at a world rings it too");
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
const onRing = Math.atan2(100, 200);
assert.deepEqual(plain(band("s1")), { mode: "belt", body: 20, slot: 1, slots: 2, bearing: onRing }, "the station holds the band");
assert.deepEqual(plain(band("b1")), { mode: "belt", body: 20, slot: 0, slots: 2, bearing: onRing }, "beside its barge, where they are");
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
assert.deepEqual(plain(fight.battles.find((b) => b.id === "orbit").anchor), { kind: "near", distance: 200, bearing: 0 },
  "a fight is drawn where it is fought");
assert.deepEqual(plain(fight.battles.find((b) => b.id === "close").anchor), { kind: "near", distance: 1200, bearing: 0 }, "so is one at the zone's edge");
assert.ok(fight.battles.find((b) => b.id === "orbit").intensity > 0.5, "arrived kills and damage brighten the fight");
assert.deepEqual(plain(fight.fleets.map((f) => [f.id, f.placement.mode, f.placement.side])), [["b1", "battle", 1], ["b2", "battle", 0]],
  "participants face off by the record's sides");
assert.deepEqual(plain(fight.wrecks.map((w) => w.id)), ["wreck"], "concluded, served, not dismissed, at this star");
assert.equal(fight.wrecks[0].anchor.kind, "near", "a wreck lies where its fight was");

// Sieges: participant-only fields; a third party sees none.
const besieged = { ...report, blockade: { by: "rival", since: 100, by_me: false, siege_since: 700 }, ground: { suppression: 0.4 } };
const siege = traffic.deriveSystemPicture(input({ dynamic: besieged, ghosts: [loiter],
  groundRecords: [{ id: "landing", system: "9", outcome: null, attacking: false, rounds: [{ marines_frac: 0.6, defenders_frac: 0.5, suppression: 0.4 }] }] })).siege;
assert.equal(siege.target, 1, "the siege is fought over the world its besiegers hold");
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
// A full orbit changes position and the night side, never the painted axis.
// Include a moon, a rewind, and a long-running clock to catch accumulated spin.
{
  const orbitingScene = new systems.SystemViewScene();
  for (const kind of ["terrestrial", "gas_giant"]) orbitingScene.kindTex.set(kind, { width: 512 });
  orbitingScene.moonTex = { width: 512 };
  const moving = bodiesWire.map((b) => ({ ...b, orbit: { ...b.orbit, period: b.parent == null ? 100 : 25 } }));
  orbitingScene.setSystem(systems.buildVisualSystem(system, moving), null);
  orbitingScene.layout(1200, 800);
  for (const node of orbitingScene.nodes) {
    assert.ok(node.art, "exercise loaded planet and moon artwork");
    node.shade = new Sprite({ width: 128 }); // headless canvas has no shade texture
  }
  const positions = [];
  for (const time of [0, 25, 50, 75, 100, 100000.25, 0]) {
    orbitingScene.update("me", "me", 1000 + time * 1000, time);
    positions.push(orbitingScene.nodes.map((n) => [n.lx, n.ly]));
    for (const node of orbitingScene.nodes) {
      close(node.art.rotation, 0, `${node.id}: no sprite roll at time ${time}`);
      const away = (Math.cos(node.shade.rotation) * node.lx + Math.sin(node.shade.rotation) * node.ly) / Math.hypot(node.lx, node.ly);
      close(away, 1, `${node.id}: shadow still faces away from the star`);
    }
  }
  assert.notDeepEqual(plain(positions[0]), plain(positions[1]), "worlds still move around their orbits");
  assert.deepEqual(plain(positions[0]), plain(positions[4]), "one period returns to the same positions");
  assert.deepEqual(plain(positions[0]), plain(positions[6]), "rewinding restores the original positions and axes");
}
// One selection (Peter, Oct 21 2026): a fleet taking it drops the world's ring
// and its pulse (Renderer.syncSystemSelection calls clearSelection).
const overlayStrokes = (color) => scene.overlay.commands.filter(([op, style]) => op === "stroke" && style?.color === color).length;
scene.update("me", "me", 1101, 0);
assert.equal(overlayStrokes(0xffffff), 1, "the picked world's ring");
scene.pulseBody("2");
scene.update("me", "me", 1102, 0);
assert.deepEqual([overlayStrokes(0xffffff), overlayStrokes(0x4fc3ff)], [1, 1], "a pulse moves the one ring");
scene.clearSelection();
scene.update("me", "me", 1103, 0);
assert.deepEqual([overlayStrokes(0xffffff), overlayStrokes(0x4fc3ff)], [0, 0], "no ring and no pulse");
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
// §tracking (Peter, Oct 27 2026): the orrery keeps its sizes; a selected fleet
// gets a small zoomed camera instead (trackinginset.ts).
{
  const raiderArt = () => ({ tex: { width: 128 }, mult: 0.87 });
  scene.setShipArt(raiderArt, null);
  const tracked = traffic.deriveSystemPicture(input({ ghosts: [ghost("t1", "raider", { docked: "9" }), inboundPirate],
    selectedShipId: "t1", selectedShipIds: new Set(["t1"]) }));
  scene.setPicture(tracked);
  scene.update("me", "me", 1400, 0);
  const target = scene.trackTarget("t1");
  const marker = scene.traffic.markers.get("t1");
  assert.ok(target.px >= 32 && scene.modelCandidates().some((c) => c.id === "t1"),
    "a selected Interceptor (its art drawn at 0.87) still reaches its live 3D hull");
  assert.deepEqual(plain({ x: target.x, y: target.y, title: target.title, color: target.color }),
    { x: marker.x, y: marker.y, title: "INTERCEPTOR", color: 0x4fc3ff }, "the hull as the orrery draws it, in its tone");
  close(target.lx, (marker.x - scene.starLayout.x) / scene.sceneScale, "and its place in scene units");
  assert.equal(scene.trackTarget("p1"), null, "an edge arrow is not tracked");
  assert.equal(scene.trackTarget("gone"), null);
  // The inset draws a scene of its own: worlds and hulls only, its own camera.
  const own = new systems.SystemViewScene({ tracking: true });
  own.setSystem(visual, null);
  own.setShipArt(raiderArt, null);
  own.setReport({ ...report, bodies: bodiesWire, builds: [], converters: [], food_state: "well_supplied", storage_used: 0, storage_cap: 100 }, true);
  own.setPicture(tracked);
  const frameOn = (focus, now) => { own.trackCamera(94, 58, 330, focus); own.update("me", "me", now, 0); return own.trackTarget("t1"); };
  let seen = frameOn({ x: target.lx, y: target.ly }, 1500);
  seen = frameOn({ x: seen.lx, y: seen.ly }, 1500);
  close(own.sceneScale, 330, "a fixed scale of its own, whatever the orrery's limits", 1e-9);
  assert.ok(Math.abs(seen.x - 47) < 1e-6 && Math.abs(seen.y - 29) < 1e-6, "locked on the fleet");
  const ownMarker = own.traffic.markers.get("t1");
  assert.ok(own.root.visible && ownMarker.root.visible && ownMarker.sprite.visible, "drawn off-stage: the hull");
  assert.ok([own.labels, own.hud, own.markers, own.overlay, own.orbitsGfx, own.coverGfx].every((l) => !l.visible),
    "no names, chips, rings or orbit lines");
  assert.ok(!ownMarker.chrome.visible && !ownMarker.label.visible && !ownMarker.badge.visible && !own.traffic.texts.visible,
    "no pips, labels or badges");
  assert.equal(own.traffic.markers.get("p1").root.visible, false, "no edge arrows");
  assert.ok(marker.chrome.visible, "the orrery keeps its own");
  // The orrery's pans and zooms never reframe it (Peter: "the tracking window
  // shouldn't move its frame when the player pans or zooms").
  const before = plain(frameOn({ x: seen.lx, y: seen.ly }, 1500));
  scene.zoomAt(target.x, target.y, 2.5);
  scene.panBy(140, -60);
  for (let i = 1; i <= 20; i++) scene.update("me", "me", 1400 + i * 100, 0);
  assert.notDeepEqual(plain(scene.trackTarget("t1")), plain(target), "the orrery moved");
  assert.deepEqual(plain(frameOn({ x: seen.lx, y: seen.ly }, 1500)), before, "the inset's picture did not");
  scene.resetZoom(true);
  scene.setShipArt(art, null);
}
// §raid-camera (Peter, Oct 27 2026): the tracking window follows the raid in,
// the raider drawn where it really is, however far out.
{
  const base = traffic.deriveSystemPicture(input({ ghosts: [inboundPirate, docked] }));
  const raid = traffic.trackedHull(base, inboundPirate, star);
  const hull = raid.fleets.find((f) => f.id === "p1");
  assert.deepEqual(plain({ ...hull.placement, tone: hull.tone, hostile: hull.hostile, selected: hull.selected }),
    { mode: "near", distance: 40_000, bearing: 0, tone: "pirate", hostile: true, selected: true },
    "a hull at its true place, not an edge arrow");
  close(hull.heading, Math.PI, "heading in");
  assert.equal(raid.fleets.filter((f) => f.id === "p1").length, 1);
  assert.deepEqual(plain(raid.fleets.find((f) => f.id === "f1")), plain(base.fleets.find((f) => f.id === "f1")), "the rest as drawn");
  const inZone = traffic.deriveSystemPicture(input({ ghosts: [near] }));
  assert.equal(traffic.trackedHull(inZone, near, star), inZone, "a hull already drawn keeps its place");
  // The tracking scene draws it there, not held at the near zone's edge.
  const cam = new systems.SystemViewScene({ tracking: true });
  cam.setSystem(visual, null);
  cam.setShipArt(art, null);
  cam.setPicture(raid);
  cam.trackCamera(95, 59, 330, { x: 0, y: 0 }, true);
  cam.update("me", "me", 2000, 0);
  const out = cam.trackTarget("p1");
  close(out.lx, 40_000 / 860, "at its true distance", 1e-6);
  // Its camera follows it every frame: hulls ride the camera's moves and ease
  // toward new places; only a cut (a new shot) snaps them.
  cam.trackCamera(95, 59, 330, { x: out.lx, y: out.ly }, false);
  cam.update("me", "me", 2016, 0);
  const framed = cam.trackTarget("p1");
  close(framed.lx, out.lx, "a camera move alone never slides it", 1e-6);
  close(framed.x, 47.5, "framed on it", 1e-6);
  const closer = { ...inboundPirate, pos: { x: star.x + 39_000, y: 0 } };
  cam.setPicture(traffic.trackedHull(traffic.deriveSystemPicture(input({ ghosts: [closer] })), closer, star));
  cam.trackCamera(95, 59, 330, { x: framed.lx, y: framed.ly }, false);
  cam.update("me", "me", 2032, 0);
  const eased = cam.trackTarget("p1");
  assert.ok(eased.lx < out.lx && eased.lx > 39_000 / 860, "eased toward its new place");
  cam.trackCamera(95, 59, 330, { x: eased.lx, y: eased.ly }, true);
  cam.update("me", "me", 2048, 0);
  close(cam.trackTarget("p1").lx, 39_000 / 860, "a cut snaps it there", 1e-6);
  // The privateer loots from the star itself: parked there, it keeps the way it came.
  const flying = cam.traffic.markers.get("p1").angle;
  const parked = { ...inboundPirate, pos: { ...star }, vel: { x: 0, y: 0 } };
  cam.setPicture(traffic.trackedHull(traffic.deriveSystemPicture(input({ ghosts: [parked], selectedShipId: "p1" })), parked, star));
  cam.update("me", "me", 2064, 0);
  close(cam.traffic.markers.get("p1").angle, flying, "parked on the star, its heading kept", 1e-9);
  close(flying, Math.PI, "flying in from the east");
}
// The inset itself: its corner, magnification, camera and centring.
{
  class Matrix {
    constructor() { this.identity(); }
    identity() { Object.assign(this, { a: 1, b: 0, c: 0, d: 1, tx: 0, ty: 0 }); return this; }
    translate(x, y) { this.tx += x; this.ty += y; return this; }
    apply(p) { return { x: this.a * p.x + this.c * p.y + this.tx, y: this.b * p.x + this.d * p.y + this.ty }; }
  }
  const RenderTexture = { create: ({ width, height, resolution }) => ({ width, height, source: { resolution },
    resize(w, h, r) { this.width = w; this.height = h; this.source.resolution = r; } }) };
  const { TrackingInset, TRACKING_SCALE } = load("trackinginset.ts", { "pixi.js": { ...pixi, Matrix, RenderTexture } });
  const inset = new TrackingInset();
  const cams = [];
  const renders = [];
  const cam = { trackCamera: (w, h, scale, focus, cut) => cams.push({ w, h, scale, focus: plain(focus), cut }) };
  const gpu = { resolution: 2, render: (o) => renders.push({ fleet: o.transform.apply({ x: 60, y: 40 }), clear: o.clear, clearColor: o.clearColor }) };
  const area = { x: 0, y: 72, w: 1048, h: 928 };
  const fleet = (px, lx, ly, x = 60, y = 40) => ({ x, y, px, lx, ly, title: "INTERCEPTOR", color: 0x4fc3ff });
  assert.equal(inset.prepare(cam, "t1", fleet(36, 0.2, -0.1, 500, 300), area), true);
  const mag = 0.38 * 450 / 36;
  const [vw, vh] = [Math.round(450 / mag), Math.round(279 / mag)];
  assert.deepEqual(cams[0], { w: vw, h: vh, scale: TRACKING_SCALE, focus: { x: 0.2, y: -0.1 }, cut: true },
    "its own camera: a fixed scale, on the fleet's place, in the inset's size over the magnification; a new shot");
  assert.deepEqual(plain(inset.box), { x: 12, y: 84, w: 450, h: 279 }, "the map area's top-left corner, 50% larger (Peter)");
  inset.render(gpu, {}, fleet(36, 0.25, -0.05));
  close(renders[0].fleet.x, vw / 2, "the fleet exactly at the centre");
  close(renders[0].fleet.y, (vh + 20 * vh / 279) / 2, "beneath the caption");
  assert.deepEqual(plain(renders[0].clearColor), [0, 0, 0, 0], "clear, over its starfield");
  close(inset.texture.source.resolution, 2 * 450 / vw, "the texture's resolution gives the magnification back", 1e-9);
  assert.deepEqual([inset.view.width, inset.view.height], [450, 279]);
  assert.match(inset.caption.text, /TRACKING · INTERCEPTOR/);
  assert.ok(inset.contains(20, 100) && !inset.contains(400, 400), "its clicks are its own");
  inset.prepare(cam, "t1", fleet(36, 9, 9), area);
  assert.deepEqual([cams[1].focus, cams[1].cut], [{ x: 0.25, y: -0.05 }, false], "the same fleet: framed where it stood, no cut");
  inset.prepare(cam, "t2", fleet(120, 0.5, 0.5), area);
  assert.deepEqual([cams[2].focus, cams[2].w, cams[2].cut], [{ x: 0.5, y: 0.5 }, 200, true],
    "a new fleet is a new shot, from its place in the orrery; a big hull never under 2.25×");
  assert.throws(() => inset.render({ resolution: 1, render: () => { throw new Error("lost"); } }, {}, fleet(120, 0.5, 0.5)),
    "a failed render reaches the renderer, which hides the inset");
  assert.equal(inset.prepare(cam, "t2", fleet(36, 0, 0), { x: 0, y: 0, w: 400, h: 600 }), false, "a map too small spares no corner");
  assert.equal(inset.root.visible, false);
  assert.equal(inset.contains(20, 100), false);
  // §raid-camera: centred just above the map's middle, the galaxy framing the
  // raid beneath it (§raid-frame); at the top on a short map.
  assert.equal(inset.prepare(cam, "p1", fleet(36, 46, 0), area, "center"), true);
  assert.deepEqual(plain(inset.box), { x: 299, y: 221, w: 450, h: 279 });
  assert.deepEqual(plain(inset.place(area, "center")), plain(inset.box), "the frame beneath knows where it sits");
  assert.ok(inset.box.y + inset.box.h <= area.y + area.h / 2 - 36, "clear of the map's lower half");
  // §raid-frame: the raid's news along the window's foot, in its tone,
  // centred in a band of its own; shrunk to fit a narrow window; gone with it.
  const raidNote = "RAIDER DETECTED · ON COURSE FOR FREYA";
  inset.render(gpu, {}, { ...fleet(36, 46, 0), color: 0xe08a2c }, raidNote);
  const rects = () => inset.band.commands.filter((c) => c[0] === "rect").map((c) => c.slice(1));
  assert.deepEqual(rects(), [[299, 221, 450, 20], [299, 221 + 279 - 22, 450, 22]], "the caption's band, and the note's along the foot");
  assert.equal(inset.note.visible, true);
  assert.equal(inset.note.text, raidNote);
  assert.equal(inset.note.style.fill, 0xe08a2c, "in the raider's tone");
  close(inset.note.position.x + inset.note.width / 2, 299 + 450 / 2, "centred");
  assert.ok(inset.note.position.y > 221 + 279 - 22 && inset.note.position.y + inset.note.height < 221 + 279, "inside its band");
  inset.render(gpu, {}, fleet(36, 46, 0), raidNote.repeat(3));
  close(inset.note.scale.x, (450 - 16) / (raidNote.length * 3 * 6), "a long one shrinks to fit", 1e-9);
  inset.render(gpu, {}, fleet(36, 46, 0));
  assert.equal(inset.note.visible, false, "no note, no band");
  assert.equal(rects().length, 1);
  inset.prepare(cam, "p1", fleet(36, 46, 0), { ...area, h: 500 }, "center");
  assert.equal(inset.box.y, 84);
  // Wiring: its scene is built on demand for the system it shows (the
  // orrery's, or the raided home) from the orrery's data (never its camera),
  // drawn after the live 3D hulls, on the desktop deck, and nothing on it
  // reaches the map beneath.
  const renderSrc = source("render.ts");
  assert.match(renderSrc, /private trackingScene = new SystemViewScene\(\{ tracking: true \}\);/);
  for (const call of ["setSystem(buildVisualSystem(sys, dyn?.bodies ?? [])", "setStarArt(loaded.texture, loaded.art)", "setShipArt(",
    "setHaze(this.hazeFor(sys))", "setDynamic(dyn?.bodies ?? []"]) {
    assert.ok(renderSrc.includes(`this.trackingScene.${call}`), `the tracking scene gets ${call}`);
  }
  assert.match(renderSrc, /let tracked: string \| null = this\.updateRaidCamera\(state\);/, "an incoming raid takes the window first");
  assert.match(renderSrc, /tracked = this\.updateTrackingScene\(state, dyn, report, mine, shown, observed\);/);
  assert.match(renderSrc, /prepare\(this\.trackingScene, raid\.id, seen, this\.cameraRect, "center"\)/, "the raid's window is centred");
  const raidCamera = renderSrc.slice(renderSrc.indexOf("private updateRaidCamera("), renderSrc.indexOf("private showTrackingSystem("));
  assert.ok(raidCamera.includes("trackedHull(deriveSystemPicture(") && !raidCamera.includes("shipModels.add"),
    "the raider at its true place; pirate culture art never goes live 3D");
  const boot = source("boot.ts");
  assert.match(boot, /const home = foundingHomeSystemId\(\);\n\s*if \(home\) activeShell\?\.focusRaid\?\.\(id, home\);\n\s*renderer\.trackRaid\(id, home\);/,
    "it starts with the raid's focus");
  assert.match(boot, /if \(event\.key === "pirates"\) renderer\.trackRaid\(null\);/, "and ends as the raid's story opens");
  // §raid-frame: both shells leave a System View, open the raider's panel
  // (or sheet) and only then frame the raid, so the frame fits the map left;
  // nothing follows it any more.
  for (const file of ["shell/deck/index.ts", "shell/mobile/index.ts"]) {
    const shell = source(file);
    const focus = shell.slice(shell.indexOf("focusRaid(id: string, homeId: string): void {"), shell.indexOf("onCore(events: CoreEvent[]): void {"));
    assert.match(focus, /renderer\.exitSystemView\(\);[\s\S]*this\.map\.focusFleet\(id\);\n\s*renderer\.frameRaid\(raider\.pos, home\.pos\);/, `${file}: out, the panel, then the frame`);
    assert.doesNotMatch(shell, /\.follow\(/, `${file}: no follow`);
  }
  assert.doesNotMatch(renderSrc, /followId|tickFollow/, "the old follow is gone");
  // Its note (Peter: "Raider detected on course for <System>"): the raid's
  // window only, while the raider is outside the hyperlimit; a notice on mobile.
  const raidTop = renderSrc.slice(renderSrc.indexOf("private updateRaidCamera("), renderSrc.indexOf("private showTrackingSystem("));
  assert.match(raidTop, /this\.raidNote = null;/, "cleared every frame");
  assert.match(raidTop, /> this\.galaxy\.hyperlimit\) \{\n\s*this\.raidNote = `RAIDER DETECTED · ON COURSE FOR \$\{sys\.name\.toUpperCase\(\)\}`;/,
    "until it crosses the hyperlimit to loot");
  assert.match(renderSrc, /this\.trackingInset\.render\(this\.app\.renderer, this\.trackingScene\.root, target, id === this\.raid\?\.id \? this\.raidNote : null, work\);/,
    "never on a selected fleet's window");
  assert.match(source("shell/mobile/index.ts"),
    /renderer\.frameRaid\(raider\.pos, home\.pos\);\n\s*this\.notices\?\.push\(\{ html: `<b>Raider detected<\/b> · on course for \$\{escapeHtml\(home\.name\)\}`, tone: "bad", durationMs: 20_000 \}\);/,
    "mobile: the same news as a notice");
  // Its course: one dashed amber line from the raider (as drawn) to the home
  // star's disc, under the hulls, while the raid camera tracks it (both shells).
  const course = renderSrc.slice(renderSrc.indexOf("private drawRaidCourse("), renderSrc.indexOf("private updateReacquisitions("));
  assert.ok(course.includes("screenById.get(raid.id)") && course.includes("this.systemHitRadius(home)") && course.includes("COL_PIRATE"),
    "from the drawn raider to the star's disc, in pirate amber");
  assert.match(renderSrc, /this\.drawProjections\(state, screenById\);\n\s*this\.drawRaidCourse\(screenById\);/);
  assert.match(renderSrc, /this\.raidCourseGfx, \/\/ §raid-frame, under the ghosts\n\s*this\.ghostsLayer,/);
  const raidTick = renderSrc.slice(renderSrc.indexOf("private updateRaidCamera("), renderSrc.indexOf("const dyn = state.systems.find((s) => s.id === sys.id);"));
  assert.ok(raidTick.indexOf("RAID_CAMERA_LOST_MS") < raidTick.indexOf("!this.trackingInsetOn"), "a raid ends on time without the window too");
  assert.match(source("core/mapclick.ts"), /resolveMapClick\([\s\S]*?if \(renderer\.trackingInsetAt\(sx, sy\)\) return \{ kind: "none" \};/,
    "a click on it over the galaxy reaches nothing beneath");
  assert.match(source("shell/deck/map.ts"), /if \(renderer\.viewMode\.type !== "galaxy" \|\| renderer\.trackingInsetAt\(clientX, clientY\)\) \{/, "nor a hover");
  assert.match(renderSrc, /this\.shipModels\.add\(\{ \.\.\.candidate, id: `tracking:\$\{candidate\.id\}`/, "its live 3D hull under a key of its own");
  assert.match(renderSrc, /this\.hubModel\.endFrame\([^;]*\);\n\s*this\.renderTrackingInset\(tracked, /, "after the live 3D hulls");
  assert.match(renderSrc, /systemScenePick\(sx: number, sy: number\): SystemScenePick \| null \{\n\s*if \([^\n]*this\.trackingInset\.contains\(sx, sy\)\) return null;/);
  assert.match(source("core/mapclick.ts"), /if \(ctx\.renderer\.trackingInsetAt\(sx, sy\)\) return \{ kind: "none" \};/, "a click on it keeps the selection");
  assert.match(source("shell/deck/map.ts"), /if \(renderer\.trackingInsetAt\(event\.clientX, event\.clientY\)\) return;/, "the wheel on it never zooms the orrery");
  assert.match(source("shell/deck/index.ts"), /ctx\.renderer\.setTrackingInset\(true\);/);
  assert.doesNotMatch(source("shell/mobile/index.ts"), /setTrackingInset/, "desktop only");
}
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
  const member = (name) => owner.members.find((n) => n.name?.getText(rast) === name).getText(rast);
  const constant = (name) => rast.statements.filter(ts.isVariableStatement).flatMap((n) => [...n.declarationList.declarations])
    .find((d) => d.name.getText(rast) === name).getText(rast);
  const exportsHint = {};
  let previewTime = 0;
  vm.runInNewContext(ts.transpileModule(`${["clamp01", "smooth01", "SYSTEM_ZOOM_FACTOR", "APPROACH_HINT_START"]
    .map((name) => `const ${constant(name)};`).join(" ")}
    exports.HintRenderer = class { approachVisuals = new Map(); galaxy = { c: 100 }; handoffGrowth = null; scale = 1;
      constructor(scene) { this.systemScene = scene; }
      fitScale() { return 0.001; }
      ${member("previewScale")}
      ${member("diveTilt")}
      ${hintMethod.getText(rast)} }`, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText,
  { exports: exportsHint, Sprite, Texture: pixi.Texture, buildVisualSystem: systems.buildVisualSystem, liveSimTime: () => previewTime,
    orbitalAngle: layout.orbitalAngle, planetPeriod: layout.planetPeriod, ORBIT_TILT: layout.ORBIT_TILT, SYSTEM_UNIT_SU: layout.SYSTEM_UNIT_SU,
    terminatorTexture: () => ({ width: 128 }), planetArtwork: planetArt.planetArtwork });
  let artLoaded = true;
  const baseScale = 300;
  const scene = { starVisibleDiameterPx: () => 0.17 * baseScale, restScale: () => baseScale,
    planetTexture: (kind) => artLoaded ? { width: 512, kind } : null };
  const hint = new exportsHint.HintRenderer(scene);
  const preview = { worlds: [], orbits: new Graphics(), worldsBack: new Container(), worldsFront: new Container() };
  const star = { x: 500, y: 400 };
  const planets = systems.buildVisualSystem(system, bodiesWire).planets;
  const drawn = new Graphics();
  hint.scale = 0.001 * 96;
  hint.drawApproachHint(drawn, preview, system, bodiesWire, star, 1, 96, { commandCenter: null });
  const k = hint.scale * layout.SYSTEM_UNIT_SU; // §real-coordinates: the map's own true scale
  assert.equal(drawn.commands.filter(([op]) => op === "circle").length, 0, "no grey dots once the art is in");
  assert.ok(preview.worldsBack.visible && preview.worldsFront.visible);
  assert.equal(preview.worlds.filter((w) => w.art.visible).length, planets.length, "one world per planet; moons wait for the System View");
  planets.forEach((planet, i) => {
    const world = preview.worlds[i];
    const art = planetArt.planetArtwork(planet.kind);
    assert.equal(world.art.texture.kind, planet.kind, `${planet.name}: its own kind's art`);
    close(world.art.scale.x, (2 * planet.radius * k) / (art.visualRatio * 512), `${planet.name}: the System View's size, to the map's scale`);
    assert.equal(world.art.parent, Math.sin(planet.angle) < 0 ? preview.worldsBack : preview.worldsFront,
      `${planet.name}: depth-sorted against the star like the System View`);
    const toStar = Math.atan2(star.y - world.art.position.y, star.x - world.art.position.x);
    close(world.art.rotation, 0, `${planet.name}: its axis matches the upright System View`);
    close(world.shade.rotation, toStar + Math.PI, `${planet.name}: night side away from the star`);
  });
  // Past the doorway the preview keeps growing with the camera, at true
  // scale; mid-handoff it rides the schematic's scale.
  const outer = planets.reduce((a, p) => (p.orbitRadius > a.orbitRadius ? p : a));
  const outerRx = (zoom) => {
    const g = new Graphics();
    hint.scale = 0.001 * zoom;
    hint.drawApproachHint(g, preview, system, bodiesWire, star, 1, zoom, { commandCenter: null });
    assert.equal(g.commands.filter(([op]) => op === "ellipse").length, 0, "orbits never cross the star's face");
    return preview.orbits.commands.filter(([op]) => op === "ellipse").map(([, , , rx]) => rx).reduce((a, b) => Math.max(a, b), 0);
  };
  close(outerRx(96), outer.orbitRadius * k, "doorway: true scale");
  close(outerRx(192), outer.orbitRadius * k * 2, "past the doorway the preview grows with the camera");
  close(outerRx(384), outer.orbitRadius * k * 4, "…to the zoom limit");
  hint.handoffGrowth = { id: system.id, scale: 0.7 };
  close(outerRx(153.6), outer.orbitRadius * baseScale * 0.7, "mid-handoff: the schematic's own scale");
  hint.handoffGrowth = { id: "elsewhere", scale: 0.7 };
  close(outerRx(96), outer.orbitRadius * k, "other stars keep their own preview");
  hint.handoffGrowth = null;
  artLoaded = false;
  const before = new Graphics();
  hint.scale = 0.001 * 96;
  hint.drawApproachHint(before, preview, system, bodiesWire, star, 1, 96, { commandCenter: null });
  assert.equal(before.commands.filter(([op]) => op === "circle").length, planets.length, "grey dots until the art loads");
  assert.ok(preview.worlds.every((w) => !w.art.visible) && !preview.worldsFront.visible, "and no stale art");
  artLoaded = true;
  const movingSystem = { ...system, id: "axis-regression" };
  const moving = bodiesWire.map((b) => ({ ...b, orbit: { ...b.orbit, period: 100 } }));
  for (previewTime of [0, 25, 50, 75, 100, 0]) {
    hint.drawApproachHint(new Graphics(), preview, movingSystem, moving, star, 1, 96, { commandCenter: null });
    for (const world of preview.worlds) {
      close(world.art.rotation, 0, "the galaxy preview keeps the same axis throughout an orbit");
      const dx = world.art.position.x - star.x, dy = world.art.position.y - star.y;
      close((Math.cos(world.shade.rotation) * dx + Math.sin(world.shade.rotation) * dy) / Math.hypot(dx, dy), 1,
        "the preview shadow follows the changing light direction");
    }
  }

}

// --- Travelling hulls near a star sit where the System View will draw them --
// The real placement from render.ts, compiled alone. §real-coordinates: every
// hull is at its true place at every zoom; over the 40→96× dive the plane about
// a star tilts into the System View's (its true scale, ORBIT_TILT), and through
// a handoff it rides the schematic's own scale. Farther out it eases back flat.
{
  const renderSource = source("render.ts");
  const rast = ts.createSourceFile("render.ts", renderSource, ts.ScriptTarget.Latest, true);
  const owner = rast.statements.find((n) => ts.isClassDeclaration(n) && n.name?.text === "Renderer");
  const method = (name) => owner.members.find((n) => n.name?.getText(rast) === name).getText(rast);
  const constant = (name) => rast.statements.filter(ts.isVariableStatement).flatMap((n) => [...n.declarationList.declarations])
    .find((d) => d.name.getText(rast) === name).getText(rast);
  const exportsPlace = {};
  vm.runInNewContext(ts.transpileModule(`${["clamp01", "smooth01", "SYSTEM_ZOOM_FACTOR",
    "APPROACH_HINT_START", "SHIP_SYSTEM_CLEAR_SU"].map((name) => `const ${constant(name)};`).join(" ")}
    exports.PlaceRenderer = class { handoffGrowth = null; cx = 40; cy = -30; scale = 1;
      constructor(scene, galaxy) { this.systemScene = scene; this.galaxy = galaxy; }
      fitScale() { return 0.001; }
      worldToScreen(p) { return { x: this.cx + p.x * this.scale, y: this.cy + p.y * this.scale }; }
      ${method("previewScale")}
      ${method("diveTilt")}
      ${method("nearStarPlacement")}
      ${method("shipScreen")}
      ${method("routeScreen")} }`, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText,
  { exports: exportsPlace, NEAR_ZONE_SU: layout.NEAR_ZONE_SU, ORBIT_TILT: layout.ORBIT_TILT, SYSTEM_UNIT_SU: layout.SYSTEM_UNIT_SU });
  const baseScale = 300;
  const starPos = { x: 100_000, y: 50_000 };
  const r = new exportsPlace.PlaceRenderer({ restScale: () => baseScale },
    { systems: [{ id: "far", pos: { x: -200_000, y: 0 } }, { id: system.id, pos: starPos }] });
  const at = (zoom) => { r.scale = 0.001 * zoom; };
  const point = (d, bearing) => ({ x: starPos.x + Math.cos(bearing) * d, y: starPos.y + Math.sin(bearing) * d });
  const truth = (p) => r.worldToScreen(p);
  // The System View's own placement (systemtrafficlayer planeToScreen) about
  // the star, at a scale of k px per unit.
  const systemView = (d, bearing, k) => {
    const star = truth(starPos);
    const radius = layout.trueRadius(d, Infinity) * k;
    return { x: star.x + Math.cos(bearing) * radius, y: star.y + Math.sin(bearing) * radius * layout.ORBIT_TILT };
  };
  const same = (a, b, label) => { close(a.x, b.x, `${label} x`, 1e-6); close(a.y, b.y, `${label} y`, 1e-6); };

  at(30);
  same(r.shipScreen(point(800, 0.7)), truth(point(800, 0.7)), "below 40× every hull is drawn flat and true");
  assert.equal(r.shipScreen(point(800, 0.7)).tilt, 0);
  at(96);
  for (const [d, bearing] of [[120, 1.1], [260, 0.3], [800, 0.7], [1_300, -2.1], [1_000, Math.PI / 2]]) {
    same(r.shipScreen(point(d, bearing)), systemView(d, bearing, r.scale * layout.SYSTEM_UNIT_SU),
      `at the doorway, ${d} su: where the System View draws it, to scale`);
  }
  assert.equal(r.shipScreen(point(800, 0.7)).tilt, 1, "the heading takes the plane's foreshortening");
  same(r.routeScreen(starPos), truth(starPos), "the star stays at the star");
  const clear = 4 * layout.NEAR_ZONE_SU;
  same(r.shipScreen(point(clear + 1, 2.5)), truth(point(clear + 1, 2.5)), "far out: flat");
  assert.equal(r.shipScreen(point(clear + 1, 2.5)).tilt, 0);
  // Continuous at the near zone's edge and outward from the star along a bearing.
  const radial = (d) => { const p = r.shipScreen(point(d, 1)); return Math.hypot(p.x - truth(starPos).x, p.y - truth(starPos).y); };
  close(radial(1_300), radial(1_300.001), "no seam at the near zone's edge", 1e-3);
  close(radial(clear - 0.001), radial(clear), "no seam where it flattens", 1e-3);
  for (let d = 300; d < clear; d += 100) assert.ok(radial(d + 100) > radial(d), `outward stays outward (${d} su)`);
  // Mid-handoff it rides the schematic's own scale, so the pictures coincide.
  at(150);
  r.handoffGrowth = { id: system.id, scale: 0.7 };
  same(r.shipScreen(point(900, -0.4)), systemView(900, -0.4, baseScale * 0.7), "mid-handoff: the schematic's scale");
  r.handoffGrowth = null;
  // Halfway through the dive, half the tilt.
  at(68);
  const half = r.shipScreen(point(800, 0.7));
  const t = truth(point(800, 0.7)), s0 = truth(starPos);
  same(half, { x: t.x, y: s0.y + (t.y - s0.y) * (1 - 0.5 * (1 - layout.ORBIT_TILT)) }, "halfway through the dive");
}

// §orbital-defense: a world's station is drawn once in orbit, larger and a
// little farther out at each level; legacy platform tiers keep their ring.
const stationOf = level => bodyFx.orbitalItems({ orbital_defense: level }, 7).filter(item => item.kind === "orbital_defense");
for (let level = 1; level <= 6; level++) {
  const [item, ...extra] = stationOf(level);
  assert.equal(extra.length, 0, "one station per world");
  assert.equal(item.tier, level);
  if (level > 1) {
    const [lower] = stationOf(level - 1);
    assert.ok(item.px > lower.px && item.orbit > lower.orbit, `level ${level} outgrows level ${level - 1}`);
  }
}
assert.equal(stationOf(9)[0].tier, 6, "the ladder tops out at the Star Fortress");
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

// §charting (Oct 28 2026): a known pirate base is drawn on the system's belt,
// named as the galaxy map names it; the founding den reads PRIVATEER DEN.
{
  const view = source("systemview.ts"), render = source("render.ts");
  assert.match(render, /state\.founding\?\.den === report\.id \? "☠ PRIVATEER DEN"/, "the founding den keeps the map's name");
  assert.match(render, /this\.systemScene\.setReport\(report, mine, pirate\)/, "the name reaches the System View");
  assert.match(view, /const pirateBelt = this\.pirateLabel \? this\.vis\?\.asteroidBelts\[0\] : undefined;/, "on the first charted belt");
  assert.match(view, /this\.pirateText\.visible = false;\n    \}/, "and nowhere when there is none");
}

// §tracking (Oct 28 2026): a tracked Mining Barge cutting ore works among its
// belt's rocks under a cutting beam; nothing is drawn for any other fleet.
{
  const render = source("render.ts"), inset = source("trackinginset.ts");
  assert.match(render, /if \(!work \|\| work\.status !== "mining"\) return null;/, "only a barge at work");
  assert.match(render, /return \{ rockUrl: beltChunkUrl\(beltKind\(belt\?\.kind\)\) \};/, "its own belt's rocks");
  assert.match(render, /this\.renderTrackingInset\(tracked, tracked \? this\.trackingWork\(state, tracked\) : null\)/);
  assert.match(inset, /this\.pane\.addChild\(this\.backdrop, this\.rocks, this\.view, this\.beam, this\.band\)/,
    "rocks behind the scene, the beam over it");
  assert.match(inset, /if \(!work \|\| !tex\) \{\n\s+this\.rocks\.visible = false;/, "no work, no rocks");
}

console.log("System view: tilted layout, Kepler motion, near zone to scale, docking slots, fog-safe picture (berths, near zone, cordon, inbound and outbound arrows, battles, wrecks, sieges), hover-only defense reach, picks, caps, 3D candidates, the tracking inset (its own scene and camera, untouched by the orrery's pans and zooms, cuts, corner, centring, wiring), the raid camera (the raider at its true place, centred, wiring), the raid frame's wiring, course line and note, galaxy orbit preview art, stations by level and inert scenery, and charted belts between orbits passed.");

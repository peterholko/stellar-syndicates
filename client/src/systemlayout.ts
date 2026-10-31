// ============================================================================
// SYSTEM VIEW LAYOUT — pure geometry shared by the orrery, its traffic layer
// and the galaxy map's approach hints. No Pixi, no state, no I/O.
// ============================================================================
//
// §real-coordinates: the sim owns positions inside a system now. Every world
// orbits its star (BodyView.orbit, laid out by the sim exactly as this view
// always drew it) and every ship at a system is at one of its worlds. The
// view draws them all to one true scale, 1 unit = SYSTEM_UNIT_SU:
//
//   * the TILTED ORBITAL PLANE — a fixed oblique view of the plane. Positions
//     are foreshortened vertically; sprites stay upright and round.
//   * ORBITAL MOTION — every body turns about its parent as a function of its
//     served orbit and time. A remote system is posed at its OBSERVED time,
//     so its picture visibly runs behind.
//   * the NEAR ZONE — out to the defenses' reach, contacts are drawn at their
//     true distance and bearing; farther out they appear only as arrows at
//     the frame's edge, and the galaxy map shows the rest.

import type { BodyView, ShipKind, Vec2 } from "./protocol";

/** Vertical foreshortening of the orbital plane (cos of the view inclination). */
export const ORBIT_TILT = 0.6;

/** Mirrors `sim::ship::DOCK_RADIUS`: the Hub's docking radius (a system's
 * is its whole gravity well). */
export const DOCK_RADIUS_SU = 260;
/** Mirrors `sim::body::SYSTEM_UNIT_SU`: one System View unit in galaxy su. */
export const SYSTEM_UNIT_SU = 860;
/** Mirrors `sim::transit::HYPERLIMIT`: a star's gravity well. */
export const WELL_SU = 900;
/** Mirrors `sim::build::DEFENSE_PLATFORM_RADIUS` (the wire also carries it). */
export const PLATFORM_COVER_SU = 1_300;
/** The near zone's edge: the defenses' reach, which is also how close a fight
 * must be for the server to tag it as fought at this star. */
export const NEAR_ZONE_SU = PLATFORM_COVER_SU;

/** The frame's preferred (the defenses' reach) and minimum outer radius
 * (normalized). */
export const NEAR_OUTER_MAX = NEAR_ZONE_SU / SYSTEM_UNIT_SU;
export const NEAR_OUTER_MIN = 1.2;

/** A hull's resting on-screen length (px) in the System View, before
 * `SYSTEM_SHIP_SCALE`. The galaxy map eases ships near a star toward the same
 * size during the approach, so a hull keeps its size across the handoff. */
const SYSTEM_SHIP_PX: Record<ShipKind, number> = {
  scout: 15, survey_probe: 11, mining_barge: 19, raider: 18, corvette: 20, tiny_freighter: 17, small_freighter: 19, convoy: 22,
  large_freighter: 25, heavy_freighter: 27, bulk_freighter: 29, freighter: 25, colony: 25,
  transport: 23, builder: 21, destroyer: 24, cruiser: 27, battleship: 30, dreadnought: 34, titan: 39,
  mining_belt_station: 30, combat_freighter: 18,
};
/** Hulls read at a fixed, legible size beside worlds tens of pixels across. */
const SYSTEM_SHIP_SCALE = 1.25;

export function systemShipPx(kind: ShipKind): number {
  return (SYSTEM_SHIP_PX[kind] ?? 20) * SYSTEM_SHIP_SCALE;
}

/** Sim seconds for one orbit at the innermost planetary radius (Kepler beyond). */
export const PLANET_PERIOD_S = 420;
const PLANET_PERIOD_RADIUS = 0.2;
/** Sim seconds for a first moon's orbit; outer moons are slower. */
export const MOON_PERIOD_S = 150;

const TAU = Math.PI * 2;

export interface PlanePoint {
  x: number;
  y: number;
}

/** Plane → projected local coordinates (still normalized, star at origin). */
export function project(p: PlanePoint): PlanePoint {
  return { x: p.x, y: p.y * ORBIT_TILT };
}

/** A point on a circle of the orbital plane, at an angle, projected. */
export function orbitPoint(radius: number, angle: number): PlanePoint {
  return { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius * ORBIT_TILT };
}

/** Depth of a plane point: +1 nearest the viewer (screen bottom), −1 farthest. */
export function depthOf(angle: number): number {
  return Math.sin(angle);
}

/** Kepler-flavoured period for a planet at a normalized orbit radius. */
export function planetPeriod(orbitRadius: number): number {
  return PLANET_PERIOD_S * Math.pow(Math.max(0.05, orbitRadius) / PLANET_PERIOD_RADIUS, 1.5);
}

/** A moon's period around its parent, by its index (inner moons are faster). */
export function moonPeriod(index: number): number {
  return MOON_PERIOD_S * (1 + 0.55 * index);
}

/** The cosmetic orbital angle at `time` (sim seconds). Counter-clockwise on
 * screen. Deterministic, so every viewer poses a system identically. */
export function orbitalAngle(baseAngle: number, period: number, time: number): number {
  if (!Number.isFinite(time) || period <= 0) return baseAngle;
  const turns = time / period;
  return baseAngle - (turns - Math.floor(turns)) * TAU;
}

/** Galaxy distance from the star → its radius on the plane (normalized), to
 * scale, held at the frame's edge `outer` beyond it. */
export function trueRadius(distanceSu: number, outer: number): number {
  return Math.max(0, Math.min(distanceSu / SYSTEM_UNIT_SU, outer));
}

/** A served body's offset from its star at `time` (normalized, the untilted
 * plane): a moon's adds its planet's. Null without a served orbit. */
export function bodyOffset(bodies: readonly BodyView[], id: number, time: number): PlanePoint | null {
  const body = bodies.find((b) => b.id === id);
  const orbit = body?.orbit;
  if (!body || !orbit || body.kind === "asteroid_belt") return null;
  const a = orbitalAngle(orbit.angle, orbit.period, time);
  const own = { x: Math.cos(a) * orbit.radius, y: Math.sin(a) * orbit.radius };
  if (body.parent === null) return own;
  const parent = bodyOffset(bodies, body.parent, time);
  return parent ? { x: parent.x + own.x, y: parent.y + own.y } : null;
}

/** How close (su) a fleet at rest must be to a world to be AT it. */
const AT_WORLD_SU = 30;

/** The world (planet or moon) a fleet at `pos` is at, at `time`: the nearest
 * within reach, else null. */
export function worldAt(bodies: readonly BodyView[], star: Vec2, pos: Vec2, time: number): number | null {
  const x = (pos.x - star.x) / SYSTEM_UNIT_SU;
  const y = (pos.y - star.y) / SYSTEM_UNIT_SU;
  let best: number | null = null;
  let bestD = AT_WORLD_SU / SYSTEM_UNIT_SU;
  for (const b of bodies) {
    const o = bodyOffset(bodies, b.id, time);
    if (!o) continue;
    const d = Math.hypot(o.x - x, o.y - y);
    if (d <= bestD) { best = b.id; bestD = d; }
  }
  return best;
}

/** The zone's outer radius for a camera rectangle: as wide as the tilted
 * ellipse allows, never cramped below the minimum. */
export function nearOuterFor(halfW: number, halfH: number, sceneScale: number): number {
  if (sceneScale <= 0) return NEAR_OUTER_MIN;
  const fit = Math.min(halfW / sceneScale, halfH / (sceneScale * ORBIT_TILT)) * 0.94;
  return Math.max(NEAR_OUTER_MIN, Math.min(NEAR_OUTER_MAX, fit));
}

/** The light age of a system's picture as seen from command (sim seconds). */
export function pictureAge(pos: PlanePoint, commandCenter: PlanePoint | null, c: number): number {
  if (!commandCenter || !(c > 0)) return 0;
  return Math.hypot(pos.x - commandCenter.x, pos.y - commandCenter.y) / c;
}

/** "LIVE", "12s", "4m", "1h 05m" — the System View's light-age stamp. */
export function formatAge(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 1) return "live";
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  const h = Math.floor(seconds / 3600);
  return `${h}h ${String(Math.round((seconds - h * 3600) / 60)).padStart(2, "0")}m`;
}

// --- Docking slots: which world a berthed hull visibly parks at --------------
//
// Docking is semantic in the sim (a fleet is berthed AT the system). The view
// parks each berthed fleet beside the world that hosts the facility it uses,
// from the viewer's OWN structure reports only. A rival's bodies arrive with
// empty structure maps (server fog), so their fleets fall back to public
// geography and nothing private is implied.

export interface AnchorBody {
  id: number;
  parent: number | null;
  habitable: boolean;
  size: string;
  kind: string;
  population: number;
  structures: Record<string, number>;
}

const WARSHIPS = new Set(["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"]);
// §holds: a Combat Freighter berths by the stores it unloads into.
const FREIGHT = new Set(["tiny_freighter", "small_freighter", "convoy", "large_freighter", "heavy_freighter", "bulk_freighter", "freighter",
  "combat_freighter"]);

const PREFERENCES: Record<string, string[]> = {
  warship: ["capital_slipway", "naval_drydock", "shipyard", "garrison", "command_center"],
  freight: ["orbital_warehouse", "warehouse", "service_berth", "command_center"],
  transport: ["garrison", "command_center", "naval_drydock", "shipyard"],
  civil: ["shipyard", "command_center", "warehouse"],
};

function role(kind: string): keyof typeof PREFERENCES {
  if (WARSHIPS.has(kind)) return "warship";
  if (FREIGHT.has(kind)) return "freight";
  if (kind === "transport") return "transport";
  return "civil";
}

const SIZE_RANK: Record<string, number> = { tiny: 0, small: 1, medium: 2, large: 3, huge: 4 };

/** The public "main world" of a system: populated (own reports), else the
 * first habitable planet, else the largest planet. Null for an empty roster. */
export function mainWorld(all: readonly AnchorBody[]): number | null {
  // §belts: a belt is never a system's main world.
  const bodies = all.filter((b) => b.kind !== "asteroid_belt");
  if (!bodies.length) return null;
  const populated = [...bodies].filter((b) => b.population > 0).sort((a, b) => b.population - a.population || a.id - b.id)[0];
  if (populated) return populated.id;
  const command = bodies.find((b) => (b.structures.command_center ?? 0) > 0);
  if (command) return command.id;
  const habitable = bodies.find((b) => b.parent === null && b.habitable);
  if (habitable) return habitable.id;
  const planets = bodies.filter((b) => b.parent === null);
  const pool = planets.length ? planets : bodies;
  return [...pool].sort((a, b) => (SIZE_RANK[b.size] ?? 2) - (SIZE_RANK[a.size] ?? 2) || a.id - b.id)[0].id;
}

/** The world a berthed fleet of `kind` parks beside. */
export function berthAnchor(kind: string, bodies: readonly AnchorBody[]): number | null {
  for (const key of PREFERENCES[role(kind)]) {
    const host = bodies
      .filter((b) => (b.structures[key] ?? 0) > 0)
      .sort((a, b) => (b.structures[key] ?? 0) - (a.structures[key] ?? 0) || a.id - b.id)[0];
    if (host) return host.id;
  }
  return mainWorld(bodies);
}

/** The world a berthed ghost parks beside: the free trader at the home it
 * serves (the main world), every other fleet by its role's facilities. */
export function fleetBerth(g: { kind: string; merchant?: boolean }, bodies: readonly AnchorBody[]): number | null {
  return g.merchant ? mainWorld(bodies) : berthAnchor(g.kind, bodies);
}

/** The world a siege is visibly fought over. */
export function siegeTarget(bodies: readonly AnchorBody[]): number | null {
  return mainWorld(bodies);
}

/** The world a system's defense platforms stand on (own reports), else the
 * main world: where a battle at the star is drawn. */
export function defendedWorld(bodies: readonly AnchorBody[]): number | null {
  const platforms = bodies
    .filter((b) => (b.structures.defense_platform ?? 0) > 0)
    .sort((a, b) => (b.structures.defense_platform ?? 0) - (a.structures.defense_platform ?? 0) || a.id - b.id)[0];
  return platforms ? platforms.id : mainWorld(bodies);
}

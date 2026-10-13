// ============================================================================
// SYSTEM VIEW LAYOUT — pure geometry shared by the orrery, its traffic layer
// and the galaxy map's approach hints. No Pixi, no state, no I/O.
// ============================================================================
//
// The System View is still a PRESENTATION of system-level facts (see the hard
// boundary in systemview.ts): nothing here creates intra-system positions in
// the sim. Three cosmetic mappings live here so every consumer agrees on them:
//
//   * the TILTED ORBITAL PLANE — a fixed oblique view of the schematic plane.
//     Positions are foreshortened vertically; sprites stay upright and round.
//   * COSMETIC ORBITAL MOTION — every body turns about its parent as a pure
//     function of (public seed, time). Same for every viewer; a remote system
//     is posed at its OBSERVED time, so its picture visibly runs behind.
//   * the NEAR ZONE — the star's immediate neighbourhood in galaxy space
//     (docking radius → the defenses' reach), drawn to scale just outside the
//     outermost orbit, keeping each contact's true bearing. Contacts farther
//     out appear only as arrows at its edge; the galaxy map shows the rest.

import type { ShipKind } from "./protocol";

/** Vertical foreshortening of the orbital plane (cos of the view inclination). */
export const ORBIT_TILT = 0.6;

/** Mirrors `sim::ship::DOCK_RADIUS`: inside this a fleet is AT the system. */
export const DOCK_RADIUS_SU = 260;
/** Mirrors `sim::build::DEFENSE_PLATFORM_RADIUS` (the wire also carries it). */
export const PLATFORM_COVER_SU = 1_300;
/** The near zone's edge: the defenses' reach, which is also how close a fight
 * must be for the server to tag it as fought at this star. */
export const NEAR_ZONE_SU = PLATFORM_COVER_SU;

/** Normalized radius where the near zone begins (planets end near 0.96). */
export const NEAR_INNER = 1.08;
/** The zone's preferred and minimum outer radius (normalized). */
export const NEAR_OUTER_MAX = 1.5;
export const NEAR_OUTER_MIN = 1.2;

/** A hull's resting on-screen length (px) in the System View, before
 * `SYSTEM_SHIP_SCALE`. The galaxy map eases ships near a star toward the same
 * size during the approach, so a hull keeps its size across the handoff. */
const SYSTEM_SHIP_PX: Record<ShipKind, number> = {
  scout: 15, survey_probe: 11, mining_barge: 19, raider: 18, corvette: 20, tiny_freighter: 17, small_freighter: 19, convoy: 22,
  large_freighter: 25, heavy_freighter: 27, bulk_freighter: 29, freighter: 25, colony: 25,
  transport: 23, builder: 21, destroyer: 24, cruiser: 27, battleship: 30, dreadnought: 34, titan: 39,
  mining_belt_station: 30,
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

/** Galaxy distance from the star → near-zone radius (normalized). Linear from
 * the docking radius to the zone's edge, so distances read true. */
export function nearRadius(distanceSu: number, inner: number, outer: number): number {
  const t = (distanceSu - DOCK_RADIUS_SU) / (NEAR_ZONE_SU - DOCK_RADIUS_SU);
  return inner + Math.max(0, Math.min(1, t)) * (outer - inner);
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
const FREIGHT = new Set(["tiny_freighter", "small_freighter", "convoy", "large_freighter", "heavy_freighter", "bulk_freighter", "freighter"]);

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

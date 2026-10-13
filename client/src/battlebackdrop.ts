import type { BattleRecordView } from "./protocol";
import type { ViewState } from "./state";
import { starTypeFor } from "./stars";
import { buildVisualSystem, type VisualSystem } from "./systemview";
import type { PlanetArtKind } from "./planetart";
import { ORBIT_TILT, PLATFORM_COVER_SU, defendedWorld, orbitalAngle, planetPeriod, siegeTarget, type AnchorBody } from "./systemlayout";
import { hasRings } from "./systembodyfx";
import { hashId } from "./prng";

export type BattleScenerySource = Pick<ViewState, "galaxy" | "systems">;

/** Snapshot astronomy from the player's served picture when the theater opens.
 * Never retain infrastructure, resource pips, population or ownership in a replay
 * backdrop. Unknown bodies stay absent; later Views cannot animate administration
 * behind a historical battle. The same builder preserves the System View layout. */
export function battleSystemScenery(systemId: string | null, source?: BattleScenerySource): VisualSystem | null {
  if (!systemId) return null;
  const system = source?.galaxy?.systems.find((s) => s.id === systemId);
  if (!system) return { systemId, starType: starTypeFor(systemId).slug, planets: [], asteroidBelts: [] };
  const bodies = source?.systems.find((s) => s.id === systemId)?.bodies ?? [];
  const visual = buildVisualSystem(system, bodies);
  visual.name = "";
  for (const planet of visual.planets) {
    planet.name = "";
    planet.deposits = [];
    planet.structures = {};
    planet.habitable = false;
    for (const moon of planet.moons) {
      moon.name = "";
      moon.deposits = [];
      moon.structures = {};
    }
  }
  for (const belt of visual.asteroidBelts) {
    belt.name = "";
    belt.deposits = [];
    belt.pockets = [];
  }
  return visual;
}

/** Fixed screen-space scenery, slightly off-center; never the tactical camera.
 * A few outer orbits may crop naturally at the viewport, as a distant backdrop. */
export function battleSceneryFrame(width: number, height: number) {
  return { x: width * 0.05, y: -height * 0.14, w: width * 1.2, h: height * 1.2 };
}

/** The place a system battle is fought, frozen from the served picture at open.
 * A fight inside the defense platforms' reach stands in a world's orbit (the
 * same world the System View draws it beside): the backdrop is that world up
 * close, lit from its star. Further out, only the star's direction is known. */
export interface BattleBackdropPlan {
  systemId: string;
  starSlug: string;
  /** Screen direction (rad) from the fight toward its star. */
  starBearing: number;
  /** The world beneath the fight — public art only, no administration. */
  world: { kind: PlanetArtKind; seed: number; ringed: boolean } | null;
}

export function battleBackdropPlan(
  rec: Pick<BattleRecordView, "id" | "system"> & { pos?: { x: number; y: number }; started_at?: number },
  source?: BattleScenerySource,
): BattleBackdropPlan | null {
  if (!rec.system) return null;
  const starSlug = starTypeFor(rec.system).slug;
  const fallbackBearing = -2.4 + ((hashId(`${rec.id}:star`) % 1000) / 1000 - 0.5) * 0.9;
  const system = source?.galaxy?.systems.find((s) => s.id === rec.system);
  if (!system) return { systemId: rec.system, starSlug, starBearing: fallbackBearing, world: null };
  const report = source?.systems.find((s) => s.id === rec.system);
  const bodies = report?.bodies ?? [];
  const dx = (rec.pos?.x ?? system.pos.x) - system.pos.x;
  const dy = (rec.pos?.y ?? system.pos.y) - system.pos.y;
  const distance = Math.hypot(dx, dy);
  if (distance > PLATFORM_COVER_SU || !bodies.length) {
    // Beyond the defenses' reach: the star lies back along the fight's bearing.
    return { systemId: rec.system, starSlug, starBearing: distance > 1 ? Math.atan2(-dy, -dx) : fallbackBearing, world: null };
  }
  const anchors: AnchorBody[] = bodies.map((b) => ({
    id: b.id, parent: b.parent, habitable: b.habitable, size: b.size, kind: b.kind,
    population: b.population ?? 0, structures: b.structures ?? {},
  }));
  const worldId = report?.blockade ? siegeTarget(anchors) : defendedWorld(anchors);
  const body = bodies.find((b) => b.id === worldId);
  if (!body) return { systemId: rec.system, starSlug, starBearing: fallbackBearing, world: null };
  // Pose the world as the System View posed it when the fight began; its star
  // then lies back along that orbital radius (in the tilted view).
  const visual = buildVisualSystem(system, bodies);
  const planet = visual.planets.find((p) => p.id === String(body.id) || p.moons.some((m) => m.id === String(body.id)));
  let starBearing = fallbackBearing;
  if (planet) {
    const a = orbitalAngle(planet.angle, planetPeriod(planet.orbitRadius), rec.started_at ?? 0);
    starBearing = Math.atan2(-Math.sin(a) * ORBIT_TILT, -Math.cos(a));
  }
  const seed = hashId(`${rec.system}:${body.id}:dress`);
  const moon = body.parent !== null;
  const kind: PlanetArtKind = moon || !planet ? "moon" : planet.kind;
  return {
    systemId: rec.system,
    starSlug,
    starBearing,
    world: { kind, seed, ringed: !moon && planet ? hasRings(planet.kind, seed) : false },
  };
}

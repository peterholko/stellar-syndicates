import type { ViewState } from "./state";
import { starTypeFor } from "./stars";
import { buildVisualSystem, type VisualSystem } from "./systemview";

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
  return visual;
}

/** Fixed screen-space scenery, slightly off-center; never the tactical camera.
 * A few outer orbits may crop naturally at the viewport, as a distant backdrop. */
export function battleSceneryFrame(width: number, height: number) {
  return { x: width * 0.05, y: -height * 0.14, w: width * 1.2, h: height * 1.2 };
}

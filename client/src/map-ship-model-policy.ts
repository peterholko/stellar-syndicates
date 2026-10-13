import type { ShipKind } from "./protocol";
import { SHIP_MODELS } from "./ship-models.generated";

// The established map machine-size ramp is 12–24×. Start loading only inside
// that range; by 22× an available hull is entirely live 3D.
export const MAP_MODEL_START = 12;
export const MAP_MODEL_FULL = 22;
export const MAP_MODEL_SLOTS = 8;
export const MAP_MODEL_CLOSE_SLOTS = 2;
export const MAP_MODEL_TRIANGLES = 120_000;
export const MAP_MODEL_FADE_MS = 240;
export const MAP_MODEL_IDLE_MS = 15_000;
export const MAP_MODEL_FRAME_PADDING = Math.SQRT2;
/** A live hull's lean (rad) once fully 3D: the battle viewer's angled view
 * (−acos 0.8 ≈ −37°), seen from the screen's bottom edge like every angled view
 * in the game. The map's flat sprites stay straight top-down, so the lean grows
 * with the model blend and the crossfade starts from an exact match. */
export const MAP_MODEL_TILT = -Math.acos(0.8);

/** Spend the same atlas area on two larger samples for close inspection.
 * Hysteresis prevents repeated reloads when a hull hovers near the size seam. */
export function mapModelCloseUp(ships: readonly MapModelCandidate[], narrow: boolean, current: boolean): boolean {
  const normalTile = narrow ? 384 : 768;
  const threshold = normalTile * (current ? .75 : 1);
  return ships.some(ship => ship.canvasPx * MAP_MODEL_FRAME_PADDING > threshold);
}

export interface MapModelCandidate {
  id: string;
  kind: ShipKind;
  x: number;
  y: number;
  canvasPx: number;
  angle: number;
  alpha: number;
  selected: boolean;
  /** The existing formation's cosmetic escorts, never undisclosed composition. */
  escorts: number;
}

export function mapModelBlend(zoom: number): number {
  const t = Math.max(0, Math.min(1, (zoom - MAP_MODEL_START) / (MAP_MODEL_FULL - MAP_MODEL_START)));
  return t * t * (3 - 2 * t);
}

export function mapModelVisible(ship: MapModelCandidate, width: number, height: number): boolean {
  const radius = ship.canvasPx * MAP_MODEL_FRAME_PADDING / 2;
  return ship.alpha > 0 && ship.canvasPx >= 32 && Number.isFinite(ship.x + ship.y + ship.angle)
    && ship.x + radius >= 0 && ship.y + radius >= 0
    && ship.x - radius <= width && ship.y - radius <= height;
}

/** A bounded set even if hundreds of fleet markers overlap. A selected ship
 * wins first, then visible size and proximity. Stable ids break ties. */
export function pickMapModels<T extends MapModelCandidate>(ships: readonly T[], width: number, height: number, slots = MAP_MODEL_SLOTS): T[] {
  const score = (ship: T) => ship.canvasPx - Math.hypot(ship.x - width / 2, ship.y - height / 2) * .15;
  const sorted = ships.filter(ship => mapModelVisible(ship, width, height)).sort((a, b) =>
    Number(b.selected) - Number(a.selected) || score(b) - score(a) || a.id.localeCompare(b.id));
  const picked: T[] = [];
  let triangles = 0;
  for (const ship of sorted) {
    const cost = SHIP_MODELS[ship.kind].triangles * (1 + ship.escorts);
    if (triangles + cost > MAP_MODEL_TRIANGLES) continue;
    picked.push(ship);
    triangles += cost;
    if (picked.length === slots) break;
  }
  return picked;
}

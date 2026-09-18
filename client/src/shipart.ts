import type { ShipKind } from "./protocol";
import { SHIP_ART } from "./ship-art.generated";

/** Approved visible nose-to-tail hierarchy, shared by map close-ups and the
 * battle theater. This is presentation only, never hull mass/range/collision. */
export const COMBAT_HULL_LENGTH_SCALE: Partial<Record<ShipKind, number>> = {
  raider: 1, corvette: 1.5, destroyer: 2.5, cruiser: 4, battleship: 5, dreadnought: 6, titan: 8,
};

export const shipArtwork = (kind: ShipKind) => SHIP_ART[kind];

/** `canvasPx` uses the renderer's established size ruler. Calibration cancels
 * source padding; framebuffer density chooses a real derivative, not an upscale.
 * Beyond 1254 native pixels the original is the honest detail ceiling. */
export function shipArtUrl(kind: ShipKind, canvasPx = 256, resolution = 1): string {
  const art = shipArtwork(kind);
  const needed = canvasPx * art.calib * Math.max(1, resolution);
  return (art.levels.find(level => level.size >= needed) ?? art.levels.at(-1)!).url;
}

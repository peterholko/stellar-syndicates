import type { GhostView, ShipKind } from "./protocol";
import { SHIP_ART } from "./ship-art.generated";

/** A hull's art: its ship kind, or a hull that flies in the sim as another
 * kind. §merchant: the free trader flies the Authority Freighter's hull. */
export type ShipArtKind = ShipKind | "free_trader";

/** The art a fleet wears: the free trader its own, a migrant run the colony
 * ship's passenger silhouette, every other fleet its hull's. */
export const fleetArtKind = (g: Pick<GhostView, "kind" | "merchant" | "migrant">): ShipArtKind =>
  g.merchant ? "free_trader" : g.migrant ? "colony" : g.kind;

/** Approved visible nose-to-tail hierarchy, shared by map close-ups and the
 * battle theater. This is presentation only, never hull mass/range/collision. */
export const COMBAT_HULL_LENGTH_SCALE: Partial<Record<ShipKind, number>> = {
  raider: 1, corvette: 1.5, destroyer: 2.5, cruiser: 4, battleship: 5, dreadnought: 6, titan: 8,
};

export const shipArtwork = (kind: ShipArtKind) => SHIP_ART[kind];

/** `canvasPx` uses the renderer's established size ruler. Calibration cancels
 * source padding; framebuffer density chooses a real derivative, not an upscale.
 * Beyond 1254 native pixels the original is the honest detail ceiling. */
export function shipArtUrl(kind: ShipArtKind, canvasPx = 256, resolution = 1): string {
  const art = shipArtwork(kind);
  const needed = canvasPx * art.calib * Math.max(1, resolution);
  return (art.levels.find(level => level.size >= needed) ?? art.levels.at(-1)!).url;
}

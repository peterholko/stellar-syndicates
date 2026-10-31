// §shipyard-view (Peter, Oct 21 2026): how a hull under construction stands in
// its yard's interior plate (art-src/shipyard-interiors-v1). Pure data: the
// renderer (ship-construction-runtime.ts) and the panels read it, tests pin it.
import type { ShipArtKind } from "./shipart";

export type YardKind = "shipyard" | "naval_drydock" | "capital_slipway";

export interface YardScene {
  /** The interior plate's slug (public/art/derived/shipyard-interiors/v1). */
  plate: string;
  /** Where the hull's centre sits on the 16:9 plate, as fractions. */
  cx: number;
  cy: number;
  /** Camera elevation above the deck, and the hull's heading (radians about
   * the vertical; 0 points the nose away from the camera). */
  elevation: number;
  heading: number;
  /** The longest hull this yard builds spans `fill` of the plate's width;
   * nothing spans more than `maxW` × `maxH` of the plate (its clear volume). */
  fill: number;
  maxW: number;
  maxH: number;
  /** A deck under the hull (a contact shadow) rather than open vacuum. */
  floor: boolean;
  /** Warm work-light tint, 0–1, matching the plate's lamps. */
  warm: number;
}

const DEG = Math.PI / 180;

export const YARD_SCENES: Record<YardKind, YardScene> = {
  shipyard: { plate: "shipyard-ii-empty-bay", cx: 0.52, cy: 0.69, elevation: 34 * DEG, heading: -135 * DEG,
    fill: 0.5, maxW: 0.56, maxH: 0.42, floor: true, warm: 0.8 },
  naval_drydock: { plate: "orbital-shipyard-empty-bay", cx: 0.47, cy: 0.52, elevation: 18 * DEG, heading: 135 * DEG,
    fill: 0.62, maxW: 0.64, maxH: 0.56, floor: false, warm: 0.45 },
  capital_slipway: { plate: "capital-slipway-empty-bay", cx: 0.53, cy: 0.54, elevation: 16 * DEG, heading: 130 * DEG,
    fill: 0.68, maxW: 0.7, maxH: 0.55, floor: false, warm: 0.45 },
};

/** Presentation lengths, nose to tail, so a yard's hulls read in proportion:
 * never mass, range or collision. Warships follow COMBAT_HULL_LENGTH_SCALE. */
const HULL_LENGTH: Partial<Record<ShipArtKind, number>> = {
  survey_probe: 0.5, scout: 0.8, raider: 1, tiny_freighter: 1.1, combat_freighter: 1.2, mining_barge: 1.2, small_freighter: 1.3,
  colony: 1.4, corvette: 1.5, convoy: 1.6, mining_belt_station: 1.6,
  destroyer: 2.5, large_freighter: 2.6, heavy_freighter: 3, bulk_freighter: 3.6, cruiser: 4, battleship: 5,
  dreadnought: 6, titan: 8,
};

/** Which yard builds a hull (sim SHIP_YARD), for the plate it stands in. */
export const HULL_YARD: Partial<Record<ShipArtKind, YardKind>> = {
  tiny_freighter: "shipyard", small_freighter: "shipyard", convoy: "shipyard", scout: "shipyard", colony: "shipyard",
  survey_probe: "shipyard", mining_barge: "shipyard", mining_belt_station: "shipyard", raider: "shipyard", corvette: "shipyard",
  combat_freighter: "shipyard",
  large_freighter: "naval_drydock", heavy_freighter: "naval_drydock", bulk_freighter: "naval_drydock",
  destroyer: "naval_drydock", cruiser: "naval_drydock", battleship: "naval_drydock",
  dreadnought: "capital_slipway", titan: "capital_slipway",
};

export const isYardKind = (value: string | null | undefined): value is YardKind =>
  value === "shipyard" || value === "naval_drydock" || value === "capital_slipway";

/** The share of the plate's width a hull spans in its yard: the yard's longest
 * hull spans its `fill`; shorter ones less, compressed so a probe still reads. */
export function hullFill(kind: ShipArtKind, yard: YardKind): number {
  const scene = YARD_SCENES[yard];
  const lengths = Object.entries(HULL_YARD).filter(([, y]) => y === yard).map(([k]) => HULL_LENGTH[k as ShipArtKind] ?? 1);
  const longest = Math.max(...lengths);
  const length = HULL_LENGTH[kind] ?? longest;
  return scene.fill * Math.min(1, length / longest) ** 0.6;
}

const PLATES = "/art/derived/shipyard-interiors/v1";
export const PLATE_WIDTHS = [768, 1280, 1672] as const;

export function plateUrls(yard: YardKind): { src: string; srcset: string } {
  const slug = YARD_SCENES[yard].plate;
  return { src: `${PLATES}/${slug}-1280.webp`, srcset: PLATE_WIDTHS.map(w => `${PLATES}/${slug}-${w}.webp ${w}w`).join(", ") };
}

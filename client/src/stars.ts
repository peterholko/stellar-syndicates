// Star-type assignment — a deterministic, CLIENT-ONLY flavor layer. Each star
// system is given a star type as a pure function of its (server-assigned) id, so
// a system is ALWAYS the same type across frames / reloads / sessions. It affects
// NOTHING mechanical (deposits, production, ownership, fog) — only the map icon +
// the System-view concept art. Both render.ts (map) and main.ts (panel) import
// this so the assignment is single-sourced. Art in /art/celestial_sprites/stars.
//
// FUTURE IDEA (not built): star type could later influence system properties —
// e.g. exotic stars as special/hazardous systems. That would be a SIM change.

export interface StarType {
  slug: string;
  title: string;
  exotic: boolean;
  // Map-icon metadata (from the galaxy-map star-icons manifest). All icons share a
  // 1254px canvas but the VISIBLE star fills a different area/offset per type, so we
  // use `center` (visible-star centre, in canvas px) to place it at the system, and
  // `visualDiameter` (visible extent, canvas px) to size the visible star — NOT the
  // whole transparent canvas — to a consistent on-map diameter.
  file: string;
  center: [number, number];
  visualDiameter: number;
}

// The star-icon canvas all icons are authored on (see the icons manifest).
export const STAR_ICON_CANVAS = 1254;

// 6 realistic (common) + 4 exotic (rare). The map-icon set has 10 types (the older
// `hypergiant` / `anomaly` are not in this icon set, so they're dropped — every
// assigned type has both a map icon AND a concept portrait). `title` matches the
// manifest.
export const STAR_TYPES: StarType[] = [
  { slug: "red_dwarf", title: "Red Dwarf", exotic: false, file: "red_dwarf_icon.png", center: [627.5, 613.0], visualDiameter: 402 },
  { slug: "yellow_star", title: "Yellow Star (Sun-like)", exotic: false, file: "yellow_star_icon.png", center: [613.5, 605.0], visualDiameter: 532 },
  { slug: "white_star", title: "White Star", exotic: false, file: "white_star_icon.png", center: [632.5, 598.0], visualDiameter: 762 },
  { slug: "blue_giant", title: "Blue Giant", exotic: false, file: "blue_giant_star.png", center: [626.5, 626.0], visualDiameter: 1031 },
  { slug: "red_giant", title: "Red Giant", exotic: false, file: "red_giant_icon.png", center: [630.0, 611.5], visualDiameter: 939 },
  { slug: "white_dwarf", title: "White Dwarf", exotic: false, file: "white_dwarf_icon.png", center: [626.0, 626.5], visualDiameter: 257 },
  { slug: "neutron_star", title: "Neutron Star / Pulsar", exotic: true, file: "neutron_star_icon.png", center: [637.5, 633.0], visualDiameter: 1082 },
  { slug: "binary_star", title: "Binary Star", exotic: true, file: "binary_star_icon.png", center: [638.0, 604.5], visualDiameter: 774 },
  { slug: "black_hole", title: "Black Hole", exotic: true, file: "black_hole_icon.png", center: [632.5, 582.5], visualDiameter: 1019 },
  { slug: "magnetar", title: "Magnetar", exotic: true, file: "magnetar_icon.png", center: [627.5, 606.0], visualDiameter: 838 },
];

const REALISTIC = STAR_TYPES.filter((s) => !s.exotic);
const EXOTIC = STAR_TYPES.filter((s) => s.exotic);

// Fraction of systems whose star is an EXOTIC type — kept low so exotics feel
// special. Tunable. (~1 in 6 systems here.)
export const EXOTIC_FRACTION = 0.16;

// Deterministic 32-bit hash of the id string (FNV-1a) — shared home in
// prng.ts. The sim replicates this bit-for-bit (node parity test).
import { hashId } from "./prng";

/// The star type for a system id — stable and rarity-weighted (exotics rare).
export function starTypeFor(id: string): StarType {
  const h = hashId(id);
  const roll = (h % 100000) / 100000; // [0,1)
  const pool = roll < EXOTIC_FRACTION ? EXOTIC : REALISTIC;
  return pool[(h >>> 17) % pool.length];
}

// Sprite anchor (0..1) placing the VISIBLE star centre at the system position.
export const starAnchor = (t: StarType): [number, number] => [t.center[0] / STAR_ICON_CANVAS, t.center[1] / STAR_ICON_CANVAS];
// Visible-star extent as a fraction of the canvas — divide the target on-map
// diameter by this (× texture width) to size the visible star, not the canvas.
export const starVisualRatio = (t: StarType): number => t.visualDiameter / STAR_ICON_CANVAS;

export const starIconUrl = (t: StarType) => `/art/celestial_sprites/stars/icons/${t.file}`;
export const starConceptUrl = (slug: string) => `/art/celestial_sprites/stars/${slug}_concept.png`;

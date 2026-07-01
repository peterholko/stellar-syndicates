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
}

// 6 realistic (common) + 6 exotic (rare). `title` matches the art manifest.
export const STAR_TYPES: StarType[] = [
  { slug: "red_dwarf", title: "Red Dwarf", exotic: false },
  { slug: "yellow_star", title: "Yellow Star (Sun-like)", exotic: false },
  { slug: "white_star", title: "White Star", exotic: false },
  { slug: "blue_giant", title: "Blue Giant", exotic: false },
  { slug: "red_giant", title: "Red Giant", exotic: false },
  { slug: "white_dwarf", title: "White Dwarf", exotic: false },
  { slug: "neutron_star", title: "Neutron Star / Pulsar", exotic: true },
  { slug: "binary_star", title: "Binary Star", exotic: true },
  { slug: "black_hole", title: "Black Hole", exotic: true },
  { slug: "magnetar", title: "Magnetar", exotic: true },
  { slug: "hypergiant", title: "Hypergiant", exotic: true },
  { slug: "anomaly", title: "Anomaly", exotic: true },
];

const REALISTIC = STAR_TYPES.filter((s) => !s.exotic);
const EXOTIC = STAR_TYPES.filter((s) => s.exotic);

// Fraction of systems whose star is an EXOTIC type — kept low so exotics feel
// special. Tunable. (~1 in 6 systems here.)
export const EXOTIC_FRACTION = 0.16;

// Deterministic 32-bit hash of the id string (FNV-1a).
function hashId(id: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h >>> 0;
}

/// The star type for a system id — stable and rarity-weighted (exotics rare).
export function starTypeFor(id: string): StarType {
  const h = hashId(id);
  const roll = (h % 100000) / 100000; // [0,1)
  const pool = roll < EXOTIC_FRACTION ? EXOTIC : REALISTIC;
  return pool[(h >>> 17) % pool.length];
}

export const starIconUrl = (slug: string) => `/art/celestial_sprites/stars/png/128/${slug}.png`;
export const starConceptUrl = (slug: string) => `/art/celestial_sprites/stars/${slug}_concept.png`;

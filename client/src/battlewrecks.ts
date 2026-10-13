// Pure rules for what happens to a hull after its recorded death: how it
// breaks, when, how its sections and debris drift, and how blasts push them.
// No Pixi and no clock — every value is a function of the death event and the
// replay time, so pausing and scrubbing always show the same wreckage. The 3D
// hull chunks and the flat-art slices both read this module, so both paths
// break a hull at the same places.
import type { ShipKind } from "./protocol";
import { mulberry32 } from "./prng";

/// Sections a hull breaks into (0 = it shatters into debris only).
export const WRECK_PIECES: Record<ShipKind, number> = {
  raider: 0, scout: 0, survey_probe: 0, tiny_freighter: 0,
  corvette: 2, small_freighter: 2, builder: 2, destroyer: 2, mining_barge: 2, mining_belt_station: 4,
  cruiser: 3, convoy: 3, colony: 3, transport: 3, freighter: 3, battleship: 3, large_freighter: 3,
  heavy_freighter: 4, dreadnought: 4, bulk_freighter: 4,
  titan: 5,
};

/// Where a hull breaks: cut fractions from nose (0) to tail (1), uneven but
/// fixed per hull type (the 3D chunks are built once per type).
export function wreckCuts(kind: ShipKind): number[] {
  const n = WRECK_PIECES[kind] ?? 0;
  if (n < 2) return [];
  const rng = mulberry32(hashKind(kind));
  const cuts: number[] = [];
  for (let j = 1; j < n; j++) cuts.push(j / n + (rng() - 0.5) * (0.5 / n));
  return cuts;
}

function hashKind(kind: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < kind.length; i++) h = Math.imul(h ^ kind.charCodeAt(i), 0x01000193);
  return h >>> 0;
}

/// Explosion class by hull mass: 0 light, 1 medium, 2 capital, 3 titan.
export function blastClass(kind: ShipKind, mass: number): 0 | 1 | 2 | 3 {
  return kind === "titan" ? 3 : mass >= 8000 ? 2 : mass >= 2000 ? 1 : 0;
}

/// Rounds between the mortal blow and the breach: capitals burn and cook off
/// first; a titan's death is a set piece; anything lighter breaks at once.
export function breachDelay(cls: 0 | 1 | 2 | 3): number {
  return cls === 3 ? 1.8 : cls === 2 ? 0.8 : 0;
}

/// Distance covered after `t` rounds at initial speed 1 under drag `k`.
export function drift(t: number, k: number): number {
  return t <= 0 ? 0 : (1 - Math.exp(-k * t)) / k;
}

/// How dark a hull has burned (0 intact … 0.8 a cold black hulk).
export function charOf(sinceBlow: number, delay: number): number {
  if (sinceBlow < 0) return 0;
  if (sinceBlow < delay) return 0.15 + 0.35 * (sinceBlow / delay);
  return Math.min(0.8, 0.5 + 0.15 * (sinceBlow - delay));
}

/// One section's motion after the breach, in model units and radians (the
/// hull is 2 units nose to tail, nose toward −z). Seeded per death + section.
export interface PieceMotion {
  /** Separation direction in the hull frame (x starboard, z aft). */ dx: number; dz: number;
  /** Separation speed, model units per round (before drag). */ speed: number;
  /** Tumble rates, radians per round: yaw (in the plane), roll, pitch. */ yaw: number; roll: number; pitch: number;
}

export function pieceMotion(seed: number, piece: number, pieces: number, cuts: number[], cls: number): PieceMotion {
  const rng = mulberry32((seed ^ Math.imul(piece + 1, 0x9e3779b1)) >>> 0);
  const lo = piece === 0 ? 0 : cuts[piece - 1], hi = piece === pieces - 1 ? 1 : cuts[piece];
  const along = (lo + hi) / 2 - 0.5; // −0.5 nose … +0.5 tail
  const heavy = cls >= 2;
  return {
    dx: (rng() - 0.5) * 0.9,
    dz: Math.sign(along || rng() - 0.5) * (0.6 + rng() * 0.4),
    speed: (heavy ? 0.22 : 0.7) * (0.6 + rng() * 0.8),
    yaw: (rng() - 0.5) * (heavy ? 0.16 : 1.1),
    roll: (rng() - 0.5) * (heavy ? 0.35 : 2.2),
    pitch: (rng() - 0.5) * (heavy ? 0.12 : 0.9),
  };
}

/// Debris fragments by hull mass (before the performance ladder thins them).
export function fragmentCount(mass: number): number {
  return Math.max(6, Math.min(40, Math.round(4 + Math.pow(mass, 0.38))));
}

export interface Fragment {
  /** 0 metal plate, 1 strut, 2 ember. */ type: 0 | 1 | 2;
  angle: number; speed: number; size: number; spin: number; glint: number;
}

/// How big a hull's debris runs: fragment sizes are multiples of this.
export function fragmentScale(mass: number): number {
  return 0.8 + Math.min(1.6, Math.pow(mass / 2000, 0.3));
}

/// The debris a blast throws (arena units per round; sizes in px at zoom 1).
export function fragments(seed: number, mass: number): Fragment[] {
  const rng = mulberry32((seed ^ 0x5eed) >>> 0);
  const n = fragmentCount(mass);
  const scale = fragmentScale(mass);
  return Array.from({ length: n }, () => {
    const roll = rng();
    const type: 0 | 1 | 2 = roll < 0.3 ? 2 : roll < 0.55 ? 1 : 0;
    return {
      type,
      angle: rng() * Math.PI * 2,
      speed: 14 + rng() * 70,
      size: (type === 2 ? 1.6 : 2.2) * scale * (0.6 + rng() * 0.8),
      spin: (rng() - 0.5) * 9,
      glint: rng() * Math.PI * 2,
    };
  });
}

/// A drawn debris piece's longest side, in multiples of its fragment size.
export const PLATE_SPAN = 1.5;
export const SLIVER_SPAN = 1.8;

/// Debris is cut from the hull's own flat art at the art's own scale: a plate
/// cut spans this fraction of the art's width, so a piece drawn at its typical
/// size shows the plating at the same scale as the intact hull. Light hulls
/// break into a few big chunks; capitals shed small plates.
export function debrisCutFraction(mass: number, px: number): number {
  const typical = PLATE_SPAN * 2.2 * fragmentScale(mass); // a typical plate's span, px at zoom 1
  return Math.max(0.06, Math.min(0.5, typical / Math.max(1, px)));
}

/// One piece of debris cut from a hull's art.
export interface DebrisCut {
  /** Centre in the art, as fractions of its width and height. */ u: number; v: number;
  /** The cell's width as a fraction of the art's width. */ span: number;
  /** How the cut is turned against the art, radians. */ rot: number;
  /** Torn outline in cell units (−1…1 on both axes). */ outline: [number, number][];
  /** A long thin spar rather than a plate. */ sliver: boolean;
}

/// Where a hull's debris comes from: `plates` irregular torn plates and
/// `slivers` long spars running along the hull, each centred on solid metal
/// (`solid(u, v)` is the art's opacity there) and spread apart. `aspect` is the
/// art's height over its width. Seeded, so a hull type always tears the same.
export function debrisCuts(seed: number, solid: (u: number, v: number) => number, aspect: number,
  fraction: number, plates = 6, slivers = 2): DebrisCut[] {
  const rng = mulberry32((seed ^ 0xdeb5) >>> 0);
  const cuts: DebrisCut[] = [];
  for (let i = 0; i < plates + slivers; i++) {
    const sliver = i >= plates;
    const span = fraction * (sliver ? SLIVER_SPAN / PLATE_SPAN : 1);
    // Solid metal under the centre and most of the piece, away from earlier cuts.
    const reach = span * 0.35;
    let best: [number, number] | null = null, bestScore = -1;
    for (let tries = 0; tries < 48; tries++) {
      const u = 0.08 + rng() * 0.84, v = 0.08 + rng() * 0.84;
      if (solid(u, v) < 0.85) continue;
      let cover = 0;
      for (let q = 0; q < 8; q++) {
        const a = (q / 8) * Math.PI * 2;
        if (solid(u + Math.cos(a) * reach, v + (Math.sin(a) * reach) / aspect) >= 0.5) cover++;
      }
      const apart = cuts.reduce((m, c) => Math.min(m, Math.hypot(c.u - u, (c.v - v) * aspect)), Infinity);
      const score = cover / 8 + Math.min(1, apart / span);
      if (score > bestScore) { bestScore = score; best = [u, v]; }
      if (cover >= 6 && apart >= span * 0.8) break;
    }
    const [u, v] = best ?? [0.5, 0.5];
    cuts.push({
      u, v, span, sliver,
      // Spars run roughly along the hull (nose-up art); plates turn freely.
      rot: sliver ? Math.PI / 2 + (rng() - 0.5) * 0.6 : rng() * Math.PI * 2,
      outline: sliver ? sliverOutline(rng) : plateOutline(rng),
    });
  }
  return cuts;
}

function plateOutline(rng: () => number): [number, number][] {
  const n = 6 + Math.floor(rng() * 3);
  const ax = 0.72 + rng() * 0.28, ay = 0.72 + rng() * 0.28;
  const bite = Math.floor(rng() * n); // one torn bite out of the rim
  return Array.from({ length: n }, (_, j) => {
    const a = ((j + (rng() - 0.5) * 0.7) / n) * Math.PI * 2;
    const r = (0.62 + rng() * 0.38) * (j === bite ? 0.55 : 1);
    return [Math.cos(a) * r * ax, Math.sin(a) * r * ay] as [number, number];
  });
}

function sliverOutline(rng: () => number): [number, number][] {
  const thin = 1 / (3 + rng() * 1.5); // half-thickness: a spar 3–4.5× longer than wide
  const edge = (x: number, side: number): [number, number] => [x + (rng() - 0.5) * 0.12, side * thin * (0.6 + rng() * 0.4)];
  return [
    [0.86 + rng() * 0.14, (rng() - 0.5) * thin], edge(0.5, 1), edge(0, 1), edge(-0.5, 1),
    [-(0.86 + rng() * 0.14), (rng() - 0.5) * thin], edge(-0.5, -1), edge(0, -1), edge(0.5, -1),
  ];
}

/// An ember cools from white-hot through orange and red to dark; returns its
/// tint and brightness at `age` rounds (brightness 0 once cold).
export function emberColor(age: number): { tint: number; glow: number } {
  const stops: [number, number, number][] = [[0, 0xfff0b8, 1], [0.6, 0xffa23c, 0.85], [1.6, 0xd2401c, 0.55], [3.2, 0x5a1a10, 0]];
  if (age <= 0) return { tint: stops[0][1], glow: 1 };
  for (let i = 1; i < stops.length; i++) {
    const [t1, c1, g1] = stops[i], [t0, c0, g0] = stops[i - 1];
    if (age <= t1) {
      const u = (age - t0) / (t1 - t0);
      return { tint: mix(c0, c1, u), glow: g0 + (g1 - g0) * u };
    }
  }
  return { tint: stops[stops.length - 1][1], glow: 0 };
}

/// Blend two 0xRRGGBB colours (u = 0 → a, 1 → b).
export function mix(a: number, b: number, u: number): number {
  const ch = (c: number, s: number) => (c >> s) & 255;
  const m = (s: number) => Math.round(ch(a, s) + (ch(b, s) - ch(a, s)) * u);
  return (m(16) << 16) | (m(8) << 8) | m(0);
}

/// A blast's shock front: radius (arena units) `age` rounds after it.
export function shockRadius(cls: number, age: number): number {
  return age <= 0 ? 0 : SHOCK_REACH[cls] * (1 - Math.exp(-age / 0.15));
}
const SHOCK_REACH = [70, 110, 190, 320];

/// How far a passing shock front has pushed something `dist` from the blast
/// (arena units, outward). Zero until the front reaches it.
export function shockPush(cls: number, age: number, dist: number): number {
  const reach = SHOCK_REACH[cls];
  if (age <= 0 || dist >= reach) return 0;
  const passed = Math.max(0, Math.min(1, (shockRadius(cls, age) - dist) / 25));
  return passed * passed * (3 - 2 * passed) * 14 * (cls + 1) * (1 - dist / reach);
}

/// Capital wrecks stay for the battle; past this many, the oldest fade.
export const MAX_WRECKS = 8;

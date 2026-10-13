// Pure geometry and timing for the battle viewer's cosmetic effects: where a
// hull's engines and guns sit in ship-local pixels, how hard its engines burn
// along its recorded track, seeded flicker, and where a damaged hull vents.
// No Pixi and no clock: every value is a function of the record, the replay
// time and the hull's size, so pausing and scrubbing always repeat a frame.
import type { ShipKind } from "./protocol";
import { mulberry32 } from "./prng";
import { SHIP_HARDPOINTS } from "./ship-hardpoints.generated";

/// Battle hulls are drawn canvasPx / 2.24 per model unit (battle-model-geometry);
/// ship-local pixels use the same scale so flat art and 3D hulls agree.
const PX_PER_UNIT = 1 / 2.24;

/// Mirrors the sim's tactical max_speed (crates/sim/src/tactical/rules_v1.rs):
/// arena units per recorded round (one engine step).
export const MAX_SPEED: Record<ShipKind, number> = {
  convoy: 40, tiny_freighter: 40, small_freighter: 40, large_freighter: 38, heavy_freighter: 34,
  bulk_freighter: 30, builder: 35, raider: 100, corvette: 65, colony: 33, scout: 115, survey_probe: 120, mining_barge: 30, mining_belt_station: 15, destroyer: 55,
  cruiser: 45, battleship: 36, dreadnought: 29, titan: 23, transport: 30, freighter: 32,
};

/// A torpedo's speed, arena units per round (mirrors the sim's `TORP_SPEED`).
export const TORP_SPEED = 140;
/// Point-defense bubble radii (mirror the sim's `PD_RADIUS` / `PD_RADIUS_HEAVY`;
/// the heavy bubble is a fitted Dreadnought's or a defense platform's).
export const PD_RADIUS = 180;
export const PD_RADIUS_HEAVY = 400;

/// How fast each hull can come about, in radians per round (one engine step).
/// Hulls heavier than a destroyer use the engine's own turn limits (rules v9
/// `turn_rate`, so their noses follow their recorded courses exactly): a
/// Cruiser 0.30, a Titan 0.085, about 37 rounds to reverse its heading.
/// Lighter hulls turn as freely as the engine lets them; these are a sense of
/// mass: an Interceptor flicks around.
export const TURN_RATE: Record<ShipKind, number> = {
  raider: 2.2, scout: 2.4, survey_probe: 2.6, corvette: 1.4, destroyer: 1.0, cruiser: 0.30, battleship: 0.20,
  dreadnought: 0.13, titan: 0.085, tiny_freighter: 1.2, small_freighter: 1.0, convoy: 0.28,
  large_freighter: 0.20, heavy_freighter: 0.14, bulk_freighter: 0.10, freighter: 0.24,
  colony: 0.24, builder: 0.6, transport: 0.21, mining_barge: 0.5, mining_belt_station: 0.08,
};

/// Wrap an angle difference into (−π, π].
export function wrapAngle(a: number): number {
  const w = a - Math.ceil((a - Math.PI) / (2 * Math.PI)) * 2 * Math.PI;
  return w <= -Math.PI ? w + 2 * Math.PI : w;
}

/// Turn from `heading` toward `desired` by at most `rate` radians.
export function turnToward(heading: number, desired: number, rate: number): number {
  const d = wrapAngle(desired - heading);
  return heading + Math.max(-rate, Math.min(rate, d));
}

/// One engine plume in ship-local pixels: nose up, +y toward the stern.
export interface Plume { x: number; y: number; r: number }

/// The plumes of a hull drawn `px` wide. Nozzles stacked above one another
/// (same plan position) share one, slightly larger, plume.
export function enginePlumes(kind: ShipKind, px: number): Plume[] {
  const k = px * PX_PER_UNIT;
  const merged: { x: number; z: number; n: number }[] = [];
  for (const [x, , z] of SHIP_HARDPOINTS[kind]?.engines ?? []) {
    const same = merged.find((m) => Math.hypot(m.x - x, m.z - z) < 0.05);
    if (same) same.n++;
    else merged.push({ x, z, n: 1 });
  }
  return merged.map((m) => {
    let gap = Infinity;
    for (const o of merged) if (o !== m) gap = Math.min(gap, Math.hypot(o.x - m.x, o.z - m.z));
    const room = Number.isFinite(gap) ? gap / 2 : 0.07;
    const r = Math.max(0.035, Math.min(0.09, room)) * (1 + 0.25 * (m.n - 1));
    return { x: m.x * k, y: m.z * k, r: r * k };
  });
}

/// Gun barrels in ship-local pixels (nose up). An unarmed hull reports one
/// point near its nose so any fallback fire still leaves the bow.
export function gunPoints(kind: ShipKind, px: number): [number, number][] {
  const k = px * PX_PER_UNIT;
  const guns = SHIP_HARDPOINTS[kind]?.guns ?? [];
  return guns.length ? guns.map(([x, , z]) => [x * k, z * k]) : [[0, -0.8 * k]];
}

/// Half width and half length of the hull's footprint in ship-local pixels.
export function hullHalfExtents(kind: ShipKind, px: number): [number, number] {
  const k = px * PX_PER_UNIT;
  const size = SHIP_HARDPOINTS[kind]?.size ?? [0.8, 0.3, 2];
  return [(size[0] / 2) * k, (size[2] / 2) * k];
}

/// Main-engine output in [0.1, 1] from a hull's motion along its track:
/// `speed` (arena units per round) and `burn`, its acceleration along the
/// heading (units per round²; braking passes 0). A withdrawing side burns
/// flat out.
export function engineThrust(speed: number, burn: number, vmax: number, withdrawing: boolean): number {
  if (withdrawing) return 1;
  const cruise = speed / Math.max(1, vmax);
  const push = Math.max(0, burn) / Math.max(1, vmax);
  return Math.max(0.1, Math.min(1, 0.12 + 0.65 * cruise + 0.9 * push));
}

function hash01(seed: number, i: number): number {
  let x = (seed ^ Math.imul(i, 0x9e3779b1)) | 0;
  x = Math.imul(x ^ (x >>> 15), 0x2c1b3c6d);
  x = Math.imul(x ^ (x >>> 12), 0x297a2d39);
  x ^= x >>> 15;
  return (x >>> 0) / 4294967296;
}

/// Smooth seeded value noise in [0, 1): flicker as a function of replay time.
export function noise1(seed: number, t: number): number {
  const i = Math.floor(t);
  const f = t - i;
  const u = f * f * (3 - 2 * f);
  return hash01(seed, i) * (1 - u) + hash01(seed, i + 1) * u;
}

/// Where a damaged hull vents smoke and fire: three ship-local points on its
/// footprint, stable for the whole battle (seed by the hull, not the window).
export function damageVents(seed: number, kind: ShipKind, px: number, plat: boolean): [number, number][] {
  const rng = mulberry32(seed);
  const [hw, hl] = plat ? [px * 0.32, px * 0.32] : hullHalfExtents(kind, px);
  return [0, 1, 2].map(() => [(rng() * 2 - 1) * hw * 0.45, (rng() * 2 - 1) * hl * 0.6]);
}

/// Smoke vents open as the recorded hull falls: one below 60%, two below 40%.
export function smokeVents(hp: number): number {
  return hp <= 0 ? 0 : hp < 0.4 ? 2 : hp < 0.6 ? 1 : 0;
}

/// Below 30% hull the vents burn and throw sparks.
export function hullOnFire(hp: number): boolean {
  return hp > 0 && hp < 0.3;
}

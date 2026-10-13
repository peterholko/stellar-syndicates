// ============================================================================
// SYSTEM VIEW BODY DRESSING — light, night-side lights, rings and the orbital
// infrastructure drawn around each world. Presentation only.
// ============================================================================
//
// Fog: every owner-only input here (structures, population) arrives EMPTY for a
// rival's bodies (server fog), so a rival world is simply drawn undeveloped.
// Determinism: per-body scatter (lights, ring presence, orbit phases) comes from
// the public body seed, never Math.random(), so every viewer sees one picture.

import { Graphics, Texture } from "pixi.js";
import { ORBIT_TILT } from "./systemlayout";

/** The baked highlight of every planet master points up-left (screen −135°). */
export const BAKED_LIGHT_ANGLE = -Math.PI * 0.75;

/** Rotation that turns a planet master's baked highlight toward its star. */
export function towardStarRotation(toStar: number): number {
  return toStar - BAKED_LIGHT_ANGLE;
}

let shadeTexture: Texture | null | undefined;
let glowTexture: Texture | null | undefined;

function canvasTexture(size: number, paint: (ctx: CanvasRenderingContext2D, size: number) => void): Texture | null {
  if (typeof document === "undefined") return null;
  try {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = size;
    const ctx = canvas.getContext("2d");
    if (!ctx) return null;
    paint(ctx, size);
    const texture = Texture.from(canvas);
    texture.source.scaleMode = "linear";
    return texture;
  } catch {
    return null; // headless/test canvas: bodies keep their baked lighting
  }
}

/** A terminator disc: clear on the lit side (−x), deep shadow on the night
 * side (+x), with soft limb darkening. Rotate +x away from the star. */
export function terminatorTexture(): Texture | null {
  if (shadeTexture !== undefined) return shadeTexture;
  shadeTexture = canvasTexture(128, (ctx, size) => {
    const r = size / 2;
    ctx.beginPath();
    ctx.arc(r, r, r - 0.5, 0, Math.PI * 2);
    ctx.clip();
    const night = ctx.createLinearGradient(0, 0, size, 0);
    night.addColorStop(0, "rgba(2,4,10,0)");
    night.addColorStop(0.44, "rgba(2,4,10,0.04)");
    night.addColorStop(0.58, "rgba(2,4,10,0.5)");
    night.addColorStop(0.74, "rgba(2,4,10,0.78)");
    night.addColorStop(1, "rgba(2,4,10,0.9)");
    ctx.fillStyle = night;
    ctx.fillRect(0, 0, size, size);
    const limb = ctx.createRadialGradient(r, r, r * 0.62, r, r, r);
    limb.addColorStop(0, "rgba(2,4,10,0)");
    limb.addColorStop(1, "rgba(2,4,10,0.32)");
    ctx.fillStyle = limb;
    ctx.fillRect(0, 0, size, size);
  });
  return shadeTexture;
}

/** A soft white radial glow (tinted per use) for coronae, flashes and haze. */
export function glowSpriteTexture(): Texture | null {
  if (glowTexture !== undefined) return glowTexture;
  glowTexture = canvasTexture(128, (ctx, size) => {
    const r = size / 2;
    const g = ctx.createRadialGradient(r, r, 0, r, r, r);
    g.addColorStop(0, "rgba(255,255,255,0.9)");
    g.addColorStop(0.25, "rgba(255,255,255,0.42)");
    g.addColorStop(0.6, "rgba(255,255,255,0.1)");
    g.addColorStop(1, "rgba(255,255,255,0)");
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, size, size);
  });
  return glowTexture;
}

/** Deterministic [0,1) stream from a body seed (mulberry32). */
export function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** About half of gas giants wear rings; public, seeded, identical for all. */
export function hasRings(kind: string, seed: number): boolean {
  return kind === "gas_giant" && seeded(seed ^ 0x51a9)() < 0.55;
}

/** Ring halves in the planet's local (normalized) frame: the far arc goes
 * behind the globe, the near arc in front. Rings lie in the orbital plane. */
export function drawRings(back: Graphics, front: Graphics, radius: number, seed: number, pixelScale: number): void {
  back.clear();
  front.clear();
  const rnd = seeded(seed ^ 0x7e11);
  const bands: [number, number, number, number][] = [
    [1.42 + rnd() * 0.08, 0.06, 0xd8c39a, 0.5],
    [1.62 + rnd() * 0.1, 0.1, 0xcab48a, 0.38],
    [1.9 + rnd() * 0.1, 0.05, 0xb9a47e, 0.26],
  ];
  const squash = ORBIT_TILT * 0.62;
  const segments = Math.max(48, Math.min(160, Math.ceil(radius * pixelScale * 2.4)));
  const arc = (g: Graphics, rx: number, from: number, to: number) => {
    for (let i = 0; i <= segments / 2; i++) {
      const a = from + (to - from) * (i / (segments / 2));
      const x = Math.cos(a) * rx;
      const y = Math.sin(a) * rx * squash;
      if (i === 0) g.moveTo(x, y);
      else g.lineTo(x, y);
    }
  };
  for (const [scale, width, color, alpha] of bands) {
    const rx = radius * scale;
    arc(back, rx, Math.PI, Math.PI * 2);
    back.stroke({ width: radius * width, color, alpha: alpha * 0.75 });
    arc(front, rx, 0, Math.PI);
    front.stroke({ width: radius * width, color, alpha });
  }
}

/** Night-side settlement lights in the shade's frame (+x = away from star).
 * Count follows the owner's population report; rivals report none. */
export function drawCityLights(g: Graphics, radius: number, population: number, seed: number, pixelScale: number): void {
  g.clear();
  if (!(population > 0)) return;
  const count = Math.min(26, 3 + Math.floor(6 * Math.log10(1 + population * 1000)));
  const rnd = seeded(seed ^ 0xc17e);
  const dot = Math.max(radius * 0.045, 0.9 / Math.max(1, pixelScale));
  for (let i = 0; i < count; i++) {
    const a = (rnd() - 0.5) * Math.PI * 0.92;
    const d = Math.sqrt(rnd()) * 0.86;
    const x = Math.cos(a) * d * radius;
    const y = Math.sin(a) * d * radius;
    if (x < radius * 0.18) continue; // lights only beyond the terminator
    const warm = rnd() < 0.8 ? 0xffd27a : 0x9fe7ff;
    g.circle(x, y, dot * (0.7 + rnd() * 0.6)).fill({ color: warm, alpha: 0.8 + rnd() * 0.2 });
  }
  // A faint clustered glow where most people live.
  g.circle(radius * 0.48, 0, radius * 0.3).fill({ color: 0xffc15a, alpha: Math.min(0.16, 0.04 + population * 0.05) });
}

// --- Orbital infrastructure -------------------------------------------------

export type OrbitalKind = "naval_drydock" | "capital_slipway" | "orbital_warehouse" | "defense_platform" | "sensor_array"
  | "orbital_defense";

export interface OrbitalItem {
  key: string;
  kind: OrbitalKind;
  tier: number;
  /** Orbit radius around the body, as a multiple of its radius. */
  orbit: number;
  /** Base phase (rad) and angular speed (rad per sim second). */
  phase: number;
  speed: number;
  /** On-screen size in px. */
  px: number;
}

/** Only these structures are drawn in orbit; everything else is surface
 * development summarized by the night-side lights and the status chip. */
export function orbitalItems(structures: Record<string, number>, seed: number): OrbitalItem[] {
  const rnd = seeded(seed ^ 0x0b17);
  const items: OrbitalItem[] = [];
  const add = (key: string, kind: OrbitalKind, tier: number, orbit: number, px: number) => {
    items.push({ key, kind, tier, orbit, phase: rnd() * Math.PI * 2, speed: (0.008 + rnd() * 0.006) * (rnd() < 0.5 ? 1 : 1.2), px });
  };
  const yard = structures.naval_drydock ?? 0;
  if (yard > 0) add("naval_drydock", "naval_drydock", yard, 2.05, 22 + yard * 2);
  const slipway = structures.capital_slipway ?? 0;
  if (slipway > 0) add("capital_slipway", "capital_slipway", slipway, 2.45, 25 + slipway * 2);
  const warehouse = structures.orbital_warehouse ?? 0;
  if (warehouse > 0) add("orbital_warehouse", "orbital_warehouse", warehouse, 1.7, 10 + warehouse * 0.5);
  const sensor = structures.sensor_array ?? 0;
  if (sensor > 0) add("sensor_array", "sensor_array", sensor, 1.6, 10 + sensor * 0.5);
  // §orbital-defense: the system's station, one sprite that grows with its level.
  const station = Math.min(5, structures.orbital_defense ?? 0);
  if (station > 0) add("orbital_defense", "orbital_defense", station, 1.5 + 0.12 * station, STATION_PX[station - 1]);
  const platforms = Math.min(4, structures.defense_platform ?? 0);
  for (let i = 0; i < platforms; i++) add(`defense_platform:${i}`, "defense_platform", structures.defense_platform, 1.38 + 0.06 * i, 8);
  // Platforms share one ring, evenly spaced, so the defense reads as a picket.
  const ring = items.filter((item) => item.kind === "defense_platform");
  ring.forEach((item, i) => { item.phase = (i / Math.max(1, ring.length)) * Math.PI * 2 + 0.3; item.speed = 0.012; });
  return items;
}

/** On-screen size of a station sprite by level (Defense Platform … Star Fortress). */
const STATION_PX = [16, 20, 26, 34, 48];

export { stationArtUrl } from "./core/derive/station";

/** Native orbital yard art (the planet stage's station sprites). */
export function orbitalYardUrl(kind: "naval_drydock" | "capital_slipway", tier: number, occupied: boolean): string {
  const level = Math.max(1, Math.min(6, Math.floor(tier)));
  return `/art/derived/planet-shipyards/v1/${kind}/tier-${level}${kind === "naval_drydock" && occupied ? "-occupied" : ""}-128.webp`;
}

/** Vector stand-ins for orbital structures without native orbital art. Drawn
 * centred at 0,0 in screen px; the caller scales/positions the Graphics. */
export function drawOrbitalGlyph(g: Graphics, kind: OrbitalKind, px: number): void {
  g.clear();
  const r = px / 2;
  switch (kind) {
    case "orbital_defense":
    case "defense_platform": {
      // A hexagonal gun platform with a red sensor eye (stations load art).
      const pts: number[] = [];
      for (let i = 0; i < 6; i++) {
        const a = Math.PI / 6 + (i * Math.PI) / 3;
        pts.push(Math.cos(a) * r, Math.sin(a) * r);
      }
      g.poly(pts).fill({ color: 0x2a313d, alpha: 0.95 }).stroke({ width: 1, color: 0xff7a6b, alpha: 0.9 });
      g.moveTo(0, 0).lineTo(r * 1.35, -r * 0.35).stroke({ width: 1.2, color: 0xc7ced8, alpha: 0.9 });
      g.circle(0, 0, r * 0.32).fill({ color: 0xff7a6b, alpha: 0.95 });
      break;
    }
    case "orbital_warehouse": {
      // A cargo ring with docked containers.
      g.circle(0, 0, r).stroke({ width: Math.max(1.2, r * 0.22), color: 0xc99bff, alpha: 0.9 });
      for (let i = 0; i < 4; i++) {
        const a = (i * Math.PI) / 2 + 0.4;
        g.rect(Math.cos(a) * r - r * 0.22, Math.sin(a) * r - r * 0.22, r * 0.44, r * 0.44).fill({ color: 0xe3b66a, alpha: 0.95 });
      }
      g.circle(0, 0, r * 0.28).fill({ color: 0x9fb0c8, alpha: 0.9 });
      break;
    }
    case "sensor_array": {
      // A dish on a truss, aimed outward.
      g.moveTo(-r * 0.2, r * 0.7).lineTo(0, 0).lineTo(r * 0.2, r * 0.7).stroke({ width: 1, color: 0x9fb0c8, alpha: 0.9 });
      g.arc(0, -r * 0.1, r * 0.85, Math.PI * 1.1, Math.PI * 1.9).stroke({ width: Math.max(1.4, r * 0.25), color: 0x62d6c3, alpha: 0.95 });
      g.circle(0, -r * 0.55, r * 0.16).fill({ color: 0xbff7ee, alpha: 0.95 });
      break;
    }
    default:
      // Yards use their art; this is only their loading fallback.
      g.rect(-r, -r * 0.3, r * 2, r * 0.6).stroke({ width: 1.2, color: 0x4fc3ff, alpha: 0.9 });
      g.moveTo(-r * 0.6, -r * 0.3).lineTo(-r * 0.6, r * 0.3).moveTo(0, -r * 0.3).lineTo(0, r * 0.3)
        .moveTo(r * 0.6, -r * 0.3).lineTo(r * 0.6, r * 0.3).stroke({ width: 1, color: 0x4fc3ff, alpha: 0.7 });
  }
}

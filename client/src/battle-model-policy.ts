import type { ShipKind } from "./protocol";
import { SHIP_MODELS } from "./ship-models.generated";

/** Screen-space inputs only. The renderer cannot inspect or advance combat. */
export interface BattleModelShip {
  id: number;
  kind: ShipKind;
  x: number;
  y: number;
  angle: number;
  canvasPx: number;
  alpha: number;
  eligible: boolean;
}

/** A wreck section for the 3D renderer: the dead hull's pose (as for a ship)
 * plus this section's separation and tumble in model space (the hull is 2
 * units nose to tail, nose toward −z). Sections are cut per hull type
 * (battlewrecks.ts), so the flat-art slices break in the same places. */
export interface BattleWreckPiece {
  id: string;
  kind: ShipKind;
  chunk: number;
  x: number;
  y: number;
  angle: number;
  canvasPx: number;
  alpha: number;
  /** 0 intact … 0.8 a cold black hulk. */
  char: number;
  /** Separation [x, y, z], model units. */
  offset: [number, number, number];
  /** Tumble [pitch, yaw, roll], radians. */
  tumble: [number, number, number];
}

export interface BattleBurst {
  id: string;
  seed: number;
  type: "death" | "impact" | "muzzle" | "flak";
  x: number;
  y: number;
  radius: number;
  age: number;
  life: number;
  color: number;
  cls: number;
}

export interface BattleModelFrame {
  width: number;
  height: number;
  ships: readonly BattleModelShip[];
  bursts: readonly BattleBurst[];
  tier: number;
  reducedMotion: boolean;
  /** Hull pitch (rad). Negative leans hulls as seen from the battlefield's
   * lower (near) edge, as every angled view in the game is; −inclination lays
   * them in an angled battlefield. Defaults to the top-down lean. */
  pitch?: number;
  /** Tilt (rad) of shock rings lying in the battlefield plane, the same way. */
  ringTilt?: number;
  /** Depth toward the camera per canvas pixel of descent: on a tilted
   * battlefield lower hulls are nearer, so they occlude higher ones. */
  depthSlope?: number;
  /** Sections of hulls that have died (the flat art covers any not shown). */
  wrecks?: readonly BattleWreckPiece[];
}

export const BATTLE_MODEL_MAX_SHIPS = 80;
/** Top-down, hulls still lean slightly toward the viewer at the screen's
 * bottom edge, so a hull flying east shows its near (starboard) side. */
export const BATTLE_MODEL_TILT = -Math.PI / 10;

export function battleModelBudget(narrow: boolean, tier: number) {
  return {
    triangles: (narrow ? 130_000 : 360_000) * (tier >= 2 ? .6 : 1),
    kinds: narrow ? 5 : 8,
    minPixels: narrow ? 14 : 9,
    bursts: tier >= 2 ? (narrow ? 4 : 8) : (narrow ? 8 : 16),
    textureSize: narrow ? 512 : 1024,
  };
}

function onScreen(x: number, y: number, radius: number, width: number, height: number): boolean {
  return Number.isFinite(x + y + radius) && radius > 0
    && x + radius >= 0 && x - radius <= width && y + radius >= 0 && y - radius <= height;
}

/** Large, nearby hulls get the budget first; wire order cannot change admission.
 * Every omitted ship keeps its normal artwork, labels, count and hit feedback. */
export function pickBattleModels(frame: BattleModelFrame, narrow: boolean): BattleModelShip[] {
  const budget = battleModelBudget(narrow, frame.tier);
  const candidates = frame.ships.filter(ship => ship.eligible && ship.alpha > .01
    && Number.isFinite(ship.angle) && ship.canvasPx >= budget.minPixels
    && onScreen(ship.x, ship.y, ship.canvasPx * .75, frame.width, frame.height));
  candidates.sort((a, b) => b.canvasPx - a.canvasPx
    || Math.hypot(a.x - frame.width / 2, a.y - frame.height / 2) - Math.hypot(b.x - frame.width / 2, b.y - frame.height / 2)
    || a.id - b.id);
  let triangles = 0;
  const kinds = new Set<ShipKind>(), picked: BattleModelShip[] = [];
  for (const ship of candidates) {
    const cost = SHIP_MODELS[ship.kind].triangles;
    if (triangles + cost > budget.triangles || (!kinds.has(ship.kind) && kinds.size >= budget.kinds)) continue;
    picked.push(ship); kinds.add(ship.kind); triangles += cost;
    if (picked.length === BATTLE_MODEL_MAX_SHIPS) break;
  }
  return picked;
}

/** The largest on-screen wreck sections, within a fixed per-frame count. */
export function pickBattleWrecks(frame: BattleModelFrame, narrow: boolean): BattleWreckPiece[] {
  return (frame.wrecks ?? []).filter(p => p.alpha > .01 && Number.isFinite(p.angle) && p.canvasPx >= 4
    && onScreen(p.x, p.y, p.canvasPx * 1.5, frame.width, frame.height))
    .sort((a, b) => b.canvasPx - a.canvasPx || a.id.localeCompare(b.id))
    .slice(0, narrow ? 24 : 48);
}

export function pickBattleBursts(frame: BattleModelFrame, narrow: boolean): BattleBurst[] {
  return frame.bursts.filter(b => b.age >= 0 && b.age < b.life && b.life > 0
    && onScreen(b.x, b.y, b.radius * 4, frame.width, frame.height))
    .sort((a, b) => Number(b.type === "death") - Number(a.type === "death") || b.radius - a.radius || a.id.localeCompare(b.id))
    .slice(0, battleModelBudget(narrow, frame.tier).bursts);
}

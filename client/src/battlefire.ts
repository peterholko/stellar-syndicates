import type { BattleRecordView, KeyframeView, ShipKind } from "./protocol";

type FrameShip = KeyframeView["ships"][number];
export interface FireTrack {
  cid?: number;
  side: number;
  kind: ShipKind;
  plat: boolean;
  hpStart: number;
  hpEnd: number;
}
export interface ShownGunfire {
  from: number;
  to: number;
  weapon: "beam" | "driver";
  damage: number;
}

// Stable battle-local ids prevent two crossing, same-class hulls from swapping
// their damage meters or receiving each other's shots. Old frames lack ids;
// only those retain the historical nearest-neighbour reconstruction.
export function matchBattleFrames(a: FrameShip[], b: FrameShip[]): Array<{ from: FrameShip | null; to: FrameShip | null }> {
  const unmatched = new Set(b);
  const out: Array<{ from: FrameShip | null; to: FrameShip | null }> = [];
  for (const from of a) {
    let to: FrameShip | null = null;
    let distance = Infinity;
    for (const candidate of unmatched) {
      if (candidate.side !== from.side || candidate.kind !== from.kind || !!candidate.plat !== !!from.plat) continue;
      if (from.cid !== undefined && candidate.cid !== undefined) {
        if (from.cid === candidate.cid) { to = candidate; break; }
        continue;
      }
      const d = Math.hypot(candidate.x - from.x, candidate.y - from.y);
      if (d < distance) { distance = d; to = candidate; }
    }
    if (to) unmatched.delete(to);
    out.push({ from, to });
  }
  for (const to of unmatched) out.push({ from: null, to });
  return out;
}

export const hasGuns = (kind: ShipKind): boolean =>
  ["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"].includes(kind);

// A window animates frame N -> N+1. The latter contains the resolved shots
// AND the resulting hull values. Never use N's damage, a battle-wide peak, or
// a future unserved round. No next arrived frame means no gunfire to replay.
export function arrivedGunfire(rec: BattleRecordView, round: number, tracks: FireTrack[]): ShownGunfire[] {
  const next = rec.rounds[round + 1];
  if (!next?.frame) return [];
  if (next.frame.gunfire != null) {
    const indices = new Map(tracks.flatMap((s, i) => s.cid === undefined ? [] : [[s.cid, i] as const]));
    return next.frame.gunfire.flatMap((shot) => {
      const from = indices.get(shot.from), to = indices.get(shot.to);
      // Large battles sample sprites and shots. Omit an unshown participant;
      // never assign its hit to an unrelated representative instead.
      return from === undefined || to === undefined ? [] : [{ from, to, weapon: shot.weapon, damage: shot.damage }];
    });
  }
  // Legacy verbatim frames have no attempts. Illustrate ONLY evidenced hull
  // losses, with at most one impact per damaged representative. Do not invent
  // misses/cooldowns, fire from civilians, or guess mixed/torpedo attribution.
  const shots: ShownGunfire[] = [];
  for (const side of [0, 1]) {
    const damage = next.dealt?.[side] ?? 0;
    if (damage <= 0) continue;
    const fits = rec.sides[side].loadouts ?? [];
    if (fits.some((fit) => fit.modules.includes("torpedo_rack"))) continue;
    const families = new Set(fits.map((fit) => fit.modules.includes("mass_driver") ? "driver" : "beam"));
    if (families.size > 1) continue;
    const from = tracks.findIndex((s) => s.side === side && (s.plat || hasGuns(s.kind)));
    if (from < 0) continue;
    const damaged = tracks.flatMap((s, i) => s.side !== side && s.hpEnd < s.hpStart - 1e-5 ? [i] : []);
    for (const to of damaged) shots.push({ from, to, weapon: families.has("driver") ? "driver" : "beam", damage: damage / damaged.length });
  }
  return shots;
}

// Perpendicular to the firing line, not a fixed diagonal that can accidentally
// point straight at the target. Clearance is in screen pixels, including hull.
export function missedEndpoint(ax: number, ay: number, tx: number, ty: number, clearance: number, sign: number): [number, number] {
  const dx = tx - ax, dy = ty - ay;
  const distance = Math.hypot(dx, dy) || 1;
  return [tx - dy / distance * clearance * sign, ty + dx / distance * clearance * sign];
}

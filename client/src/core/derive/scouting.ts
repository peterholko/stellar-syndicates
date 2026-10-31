import type { Commodity, SystemStateView } from "../../protocol";

/** §scouting (Peter, Oct 28 2026): what this corporation's Scouts last saw of
 * a rival system, both shells: its defences (`intel`) and Travian's
 * "resources" report (`scouted_stores`: goods in raiders' reach, the Vault). */
export interface ScoutReport {
  /** Sim time of the newest look. */
  at: number;
  /** "Defence ×2 · Shipyard ×1", when the defences were read. */
  defence: string | null;
  /** Whole units in raiders' reach, dearest first. */
  reach: { commodity: Commodity; units: number }[];
  /** The Vault's room (units). */
  vault: number;
  /** The goods in reach at the arrived Hub prices; null without every quote. */
  value: number | null;
}

/** A probe needs more Scouts than this (sim `World::counter_scouting`). */
export const COUNTER_SCOUTING_HINT = "A probe needs more Scouts than the system's counter-scouting: its own Scouts there, plus 2 a Sensor Array level.";

export function scoutReport(dyn: SystemStateView | undefined,
  price: (commodity: Commodity) => number | undefined): ScoutReport | null {
  const intel = dyn?.intel ?? null;
  const stores = dyn?.scouted_stores ?? null;
  if (!intel && !stores) return null;
  const reach = (stores?.reach ?? []).map((slot) => ({ commodity: slot.commodity as Commodity, units: slot.units }));
  const priced = reach.map((slot) => (price(slot.commodity) ?? Number.NaN) * slot.units);
  return {
    at: Math.max(intel?.observed_at ?? Number.NEGATIVE_INFINITY, stores?.observed_at ?? Number.NEGATIVE_INFINITY),
    defence: intel ? `Defence ×${intel.defense_tier} · Shipyard ×${intel.shipyard_tier}` : null,
    reach,
    vault: stores?.vault ?? 0,
    value: reach.length && priced.every(Number.isFinite) ? priced.reduce((sum, n) => sum + n, 0) : null,
  };
}

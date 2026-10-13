// The orbital defense station as every surface reads it (§orbital-defense):
// one station per system, raised level by level from a Defense Platform to a
// Star Fortress, fighting through its hardpoints. Fit rules mirror the sim
// (crates/sim/src/station.rs); the sim checks again when the order lands.

import type { ModuleKind, StationView } from "../../protocol";

/** The ladder's names, level 1 to 5 (the served catalog carries the same). */
export const STATION_NAMES = ["Defense Platform", "Orbital Battery", "Defense Station", "Citadel", "Star Fortress"];

export const stationName = (level: number): string => STATION_NAMES[Math.max(1, Math.min(5, Math.floor(level))) - 1];

/** The station's art at `level` (1–5), from the derived station sprites. */
export function stationArtUrl(level: number, size: 128 | 256 | 512 = 128): string {
  return `/art/derived/stations/v1/level-${Math.max(1, Math.min(5, Math.floor(level)))}-${size}.webp`;
}

/** A hardpoint's budget: three slots, eight fitting points (a destroyer's). */
export const MOUNT_SLOTS = 3;
export const MOUNT_POINTS = 8;

/** The modules a hardpoint takes, with their fitting points. Point defense
 * comes with every hardpoint; utilities and stabilizers are ship equipment. */
const POINTS: Partial<Record<ModuleKind, number>> = {
  mass_driver: 2, torpedo_rack: 3, prismatic_lance: 3, reflective_plating: 2, whipple_armor: 3, stasis_web: 2,
};
export const MOUNT_MODULES = Object.keys(POINTS) as ModuleKind[];

/** Why `fit` can't go on a hardpoint (empty: it can). `lances`: Platform
 * Lances research is complete. */
export function mountFitError(fit: ModuleKind[], lances: boolean): string {
  if (fit.length > MOUNT_SLOTS) return "A hardpoint has three slots.";
  if (fit.some((m) => !(m in POINTS))) return "A hardpoint takes weapons, armour and webs only; point defense comes with it.";
  if (fit.includes("prismatic_lance") && !lances) return "A lance battery needs Platform Lances research.";
  const points = fit.reduce((n, m) => n + (POINTS[m] ?? 0), 0);
  return points > MOUNT_POINTS ? `That fit needs ${points} fitting points; a hardpoint has ${MOUNT_POINTS}.` : "";
}

/** The crates a refit from `from` to `to` takes from the system's ledger. A
 * lance comes with research, not a crate. */
export function refitCrates(from: ModuleKind[], to: ModuleKind[]): Partial<Record<ModuleKind, number>> {
  const need: Partial<Record<ModuleKind, number>> = {};
  for (const m of new Set(to)) {
    if (m === "prismatic_lance") continue;
    const extra = to.filter((x) => x === m).length - from.filter((x) => x === m).length;
    if (extra > 0) need[m] = extra;
  }
  return need;
}

/** "Mass Driver ×2 · Whipple Armor", or the stock battery. */
export function mountLabel(modules: ModuleKind[], name: (m: ModuleKind) => string): string {
  if (!modules.length) return "Beam battery (stock)";
  const counts = new Map<ModuleKind, number>();
  for (const m of modules) counts.set(m, (counts.get(m) ?? 0) + 1);
  return [...counts].map(([m, n]) => (n > 1 ? `${name(m)} ×${n}` : name(m))).join(" · ");
}

/** One hardpoint's state in a word or two. */
export function mountStatus(m: StationView["mounts"][number]): string {
  const pct = Math.round(m.health * 100);
  if (m.offline) return `Knocked out · repairing, ${pct}% (back at 25%)`;
  return m.health >= 0.995 ? "Firing" : `Firing · ${pct}% hull`;
}

/** "Citadel · 3 of 4 hardpoints firing". */
export function stationSummary(station: StationView): string {
  const firing = station.mounts.filter((m) => !m.offline && m.health > 0).length;
  const total = station.mounts.length;
  if (firing === 0) return `${station.name} · silenced: every hardpoint is out`;
  return `${station.name} · ${firing === total ? "all" : firing} of ${total} hardpoint${total === 1 ? "" : "s"} firing`;
}

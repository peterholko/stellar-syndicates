// Shared captains derivations extracted from the desktop shell.

import { fleetExactCount, type CaptainView, type GhostView, type ModuleKind, type ShipKind } from "../../protocol";
import { label } from "../../icons";
import { shipKindLabel } from "./fleet";

// Mirrors sim::captain::command_weight. This is preview math only; the sim
// enforces merge/build authority. Showing the same weighted burden beside the
// served exact composition explains why rank limits both hull type and count.
const CAPTAIN_COMMAND_WEIGHT: Record<ShipKind, number> = {
  tiny_freighter: 1, small_freighter: 1, large_freighter: 2, heavy_freighter: 4, bulk_freighter: 8,
  scout: 1,
  convoy: 1,
  builder: 1,
  freighter: 1,
  raider: 2,
  corvette: 4,
  colony: 4,
  transport: 4,
  destroyer: 8,
  cruiser: 16,
  battleship: 32,
  dreadnought: 64,
  titan: 128,
};


export function captainXpFloor(level: number): number {
  const n = Math.max(0, level - 1);
  return 100 * n * (n + 1) / 2;
}


export function fleetCommandLoad(g: GhostView): number {
  return (g.composition ?? []).reduce(
    (total, stack) => total + CAPTAIN_COMMAND_WEIGHT[stack.kind] * stack.count,
    0,
  );
}


export function captainTitle(title: CaptainView["title"]): string {
  switch (title) {
    case "lieutenant": return "Lieutenant";
    case "lieutenant_commander": return "Lieutenant-Commander";
    case "commander": return "Commander";
    case "captain": return "Captain";
    case "rear_admiral": return "Rear Admiral";
    case "vice_admiral": return "Vice Admiral";
    case "admiral": return "Admiral";
    case "fleet_admiral": return "Fleet Admiral";
  }
}


export function officerFleetName(g: GhostView): string {
  const exact = fleetExactCount(g);
  return `${shipKindLabel(g.kind)} fleet${exact === null ? "" : ` · ${exact} ship${exact === 1 ? "" : "s"}`}`;
}

// §fitting: the hull-AFFINITY factor line for a (kind, fit) — the named
// multiplier the sim applies (mirrors ship::hull_affinity); null when none.
export function affinityLine(kind: string, mods: ModuleKind[]): string | null {
  const hasWeapon = mods.some((m) => m === "torpedo_rack" || m === "mass_driver" || m === "point_defense_screen");
  if (kind === "raider" && mods.includes("torpedo_rack")) return "Interceptor torpedo affinity ×1.25";
  if (kind === "corvette" && mods.includes("point_defense_screen")) return "Corvette interception affinity ×1.25";
  // §ladder: each capital's one named factor.
  if (kind === "destroyer" && !mods.includes("torpedo_rack") && !mods.includes("mass_driver")) return "Destroyer beam affinity ×1.20";
  if (kind === "cruiser" && (mods.includes("reflective_plating") || mods.includes("whipple_armor"))) return "Cruiser protection affinity ×1.20";
  if (kind === "battleship" && mods.includes("mass_driver")) return "Battleship driver affinity ×1.20";
  if (kind === "dreadnought" && mods.includes("point_defense_screen")) return "Dreadnought interception affinity ×1.30";
  if (kind === "titan" && (hasWeapon || mods.length === 0)) return "Titan weapon affinity ×1.10";
  return null;
}

// (buildOptionRow removed — the inline structure/ship rows it drew are gone; the
//  dedicated build panels now own that gating via structOption / shipOption.)

// §explore Part 3: the trait line (name + one-line effect) for the OWNER's
// system panel. Warn-tinted for the lemon. Slug "bonus_vein:<commodity>" carries
// the vein's commodity.
export function traitLine(slug: string): { title: string; desc: string; warn: boolean } {
  if (slug.startsWith("bonus_vein:")) {
    const c = slug.split(":")[1];
    return { title: "Bonus Vein", desc: `Its ${label(c)} deposit gains ×1.5 natural yield, within the ×3 site cap.`, warn: false };
  }
  switch (slug) {
    case "deep_deposits":
      return { title: "Deep Deposits", desc: "Natural yield gains ×1.5 (within the ×3 cap), but the FIRST Extractor tier is wasted breaking through.", warn: false };
    case "unstable_geology":
      return { title: "Unstable Geology", desc: "Development costs ×1.25 here — survey before committing.", warn: true };
    case "volatile_pockets":
      return { title: "Volatile Pockets", desc: "Refinery output ×1.3 here.", warn: false };
    case "precursor_cache":
      return { title: "Precursor Cache", desc: "A one-time 40 Alloys was deposited to the stockpile at claim.", warn: false };
    default:
      return { title: label(slug), desc: "", warn: false };
  }
}

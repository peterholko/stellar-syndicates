// §research-panel (Nov 1 2026): presentation data both research surfaces share.
// Names, art and unlock captions only. Availability, gates, costs and benefits
// always come from the served research view.

import { stationName } from "./core/derive/station";
import { MODULES } from "./core/derive/equipment";
import { icon, label, structureImage } from "./icons";
import type { ModuleKind, ShipKind } from "./protocol";
import { SHIP_MODELS } from "./ship-models.generated";

export const FIELD_ORDER = ["propulsion", "materials", "computation", "weapons", "hulls", "life"] as const;
export type ResearchField = typeof FIELD_ORDER[number];
export const FIELD_TITLE: Record<string, string> = {
  propulsion: "Propulsion", materials: "Materials", computation: "Computation",
  weapons: "Weapons", hulls: "Hulls", life: "Life",
};
export const SCHOOL_TITLE: Record<string, string> = {
  line_haul: "Line Haul", expedition: "Expedition", deep_crust: "Deep Crust", foundry: "Foundry",
  watch: "Watch", shadow: "Shadow", strike: "Strike", countermeasures: "Countermeasures",
  line: "Line", corsair: "Corsair", growth: "Growth", talent: "Talent",
};
export const ROMAN = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII"];
export const roman = (tier: number): string => ROMAN[tier] ?? String(tier);
export const schoolTitle = (school: string | null): string => school ? SCHOOL_TITLE[school] ?? label(school) : "";

/** One catalogue unlock, exactly as the programme's effect states it. */
export type ResearchUnlock =
  | { structure: string; tier: number }
  | { hull: ShipKind }
  | { module: ModuleKind };

/** The catalogue's object unlocks (Effect::UnlockStructureTier / UnlockHull /
 * UnlockModule in crates/sim/src/research.rs), every effect kept. The research
 * art test checks this table against the catalogue. Programmes missing here
 * grant modifiers or capabilities, which their description covers. */
export const RESEARCH_UNLOCKS: Readonly<Record<string, readonly ResearchUnlock[]>> = {
  prop_bunkerage: [{ structure: "volatile_harvester", tier: 1 }, { structure: "fuel_refinery", tier: 1 }, { structure: "power_plant", tier: 3 }],
  prop_freight_frames: [{ hull: "small_freighter" }],
  prop_heavy_lifters: [{ hull: "convoy" }],
  prop_line_express_charters: [{ hull: "large_freighter" }],
  prop_line_bulk_charters: [{ hull: "heavy_freighter" }],
  prop_line_autonomous_freight: [{ hull: "bulk_freighter" }],
  prop_expedition_iii_extended_tanks: [{ module: "extended_tanks" }],
  prop_expedition_iv_fleet_tenders: [{ module: "fuel_transfer_rig" }],
  // §smelt-from-landing (Nov 6 2026): the Smelter needs no research.
  mat_enrichment: [{ structure: "chemical_works", tier: 1 }],
  mat_prefab_construction: [{ structure: "command_center", tier: 3 }, { structure: "composite_works", tier: 1 }, { structure: "hull_fabricator", tier: 1 }],
  mat_autoforges: [{ structure: "machine_works", tier: 1 }, { structure: "precision_works", tier: 1 }, { structure: "naval_drydock", tier: 1 }, { structure: "drive_works", tier: 1 }],
  mat_belt_stations: [{ hull: "mining_belt_station" }],
  mat_deepcrust_iii_tier4_extraction: [{ structure: "mining_complex", tier: 4 }],
  mat_foundry_iii_tier4_processors: [{ structure: "smelter", tier: 4 }],
  mat_foundry_iv_orbital_yards: [{ structure: "shipyard", tier: 4 }, { structure: "naval_drydock", tier: 4 }, { structure: "orbital_warehouse", tier: 1 }],
  mat_foundry_iv_arcology_frames: [{ structure: "habitat", tier: 4 }, { structure: "warehouse", tier: 4 }],
  // §reactors-drives: Helium-3 Cycle (rung 3) and Antimatter Synthesis (rung 4) at the Drive Works.
  mat_foundry_iv_fusion_reactors: [{ structure: "power_plant", tier: 4 }],
  mat_foundry_vi_antimatter_synthesis: [{ structure: "antimatter_plant", tier: 1 }],
  comp_sensor_gain: [{ structure: "sensor_array", tier: 1 }],
  comp_signal_libraries: [{ structure: "electronics_fabricator", tier: 1 }],
  comp_deep_space_arrays: [{ structure: "sensor_array", tier: 4 }],
  comp_shadow_iv_recon_suite: [{ module: "recon_suite" }],
  weap_fire_control: [{ structure: "orbital_defense", tier: 2 }, { structure: "garrison", tier: 1 }],
  weap_orbital_batteries: [{ structure: "orbital_defense", tier: 3 }],
  weap_munitions_lines: [{ structure: "armaments_complex", tier: 1 }],
  weap_strike_iii_lance_array: [{ module: "lance_array" }],
  weap_cm_iii_defense_stations: [{ structure: "orbital_defense", tier: 4 }],
  weap_cm_iii_ionospheric_veil: [{ structure: "veil_projector", tier: 1 }],
  weap_cm_iv_deep_veil: [{ structure: "veil_projector", tier: 3 }],
  weap_cm_v_grand_batteries: [{ structure: "orbital_defense", tier: 5 }],
  weap_cm_v_star_fortress: [{ structure: "orbital_defense", tier: 6 }],
  hull_line_iii_escort_datalink: [{ hull: "corvette" }, { module: "escort_datalink" }],
  hull_cargo_pods: [{ module: "cargo_pods" }],
  hull_drydock_efficiency: [{ structure: "ordnance_foundry", tier: 1 }],
  hull_modular_berths: [{ structure: "naval_drydock", tier: 1 }],
  hull_line_iv_destroyer: [{ hull: "destroyer" }],
  hull_line_v_cruiser: [{ hull: "cruiser" }],
  hull_line_vi_battleship: [{ hull: "battleship" }],
  hull_line_vii_dreadnought: [{ hull: "dreadnought" }, { structure: "capital_slipway", tier: 1 }, { structure: "shipyard", tier: 5 }],
  hull_line_viii_titan: [{ hull: "titan" }, { structure: "shipyard", tier: 6 }],
  life_growth_v_arcologies: [{ structure: "habitat", tier: 5 }],
  life_talent_iv_academy_iv: [{ structure: "academy", tier: 4 }],
};

/** The sim's ordinary ceiling (build.rs BASE_MAX_STRUCTURE_TIER): an unlock
 * marker at or above it opens tiers V–VI (RESEARCHED_MAX_STRUCTURE_TIER). */
const BASE_MAX_STRUCTURE_TIER = 4;

export interface UnlockPicture {
  /** The object's name, e.g. "Smelter". */
  name: string;
  /** What opens: "Building", "Tier V–VI", "Level 2", "Hull", "Module". */
  opens: string;
  /** The tier the art depicts (structures only). */
  tier?: number;
  art: string;
}

/** Caption and art for one unlock, following build.rs `max_buildable_tier`:
 * a station marker is that station level; a Command Center marker is its
 * Tier III; any other marker of 4+ opens the prize tiers V–VI; below that it
 * is the building's first unlock. */
export function unlockPicture(unlock: ResearchUnlock, px = 96): UnlockPicture {
  const sizes = `${px}px`;
  if ("hull" in unlock) {
    // The hull's own model render (models-v1 previews), as the Shipyard shows it.
    const model = (SHIP_MODELS as Record<string, { title: string; poster: string } | undefined>)[unlock.hull];
    const name = model?.title ?? label(unlock.hull);
    const art = model ? `<img class="research-unlock__img" src="${model.poster}" width="${px}" height="${px}" alt="" loading="lazy" decoding="async">` : icon("fleet", "lg", name);
    return { name, opens: "Hull", art };
  }
  if ("module" in unlock) {
    const entry = MODULES.find(m => m.kind === unlock.module);
    const name = entry?.name ?? label(unlock.module);
    return { name, opens: "Module", art: icon(entry?.icon ?? "build", "lg", name) };
  }
  const { structure: key, tier } = unlock;
  if (key === "orbital_defense") {
    const name = stationName(tier);
    return { name, opens: `Level ${tier}`, tier, art: structureImage(key, tier, "lg", name, "research-unlock__img", sizes) };
  }
  const name = label(key);
  if (key === "command_center" || key === "veil_projector") {
    const depicted = Math.min(3, Math.max(1, tier));
    const opens = key === "veil_projector" && depicted === 1 ? "Building" : `Tier ${roman(depicted)}`;
    return { name, opens, tier: depicted, art: structureImage(key, depicted, "lg", name, "research-unlock__img", sizes) };
  }
  if (tier >= BASE_MAX_STRUCTURE_TIER) {
    return { name, opens: "Tier V–VI", tier: 5, art: structureImage(key, 5, "lg", name, "research-unlock__img", sizes) };
  }
  return { name, opens: "Building", tier: 1, art: structureImage(key, 1, "lg", name, "research-unlock__img", sizes) };
}

export const researchUnlocks = (id: string): readonly ResearchUnlock[] => RESEARCH_UNLOCKS[id] ?? [];

// Shared fleet derivations extracted from the desktop shell.

import {
  cargoUnitsPerHull,
  isLootOnly,
  isPlayerFreighter,
  fleetCargoManifest,
  type BattleRecordView,
  type BattleReportView,
  type Commodity,
  type ReactorGrade,
  type Drive,
  type DriveView,
  type EntityId,
  type GhostView,
  type ShipKind,
  type SideRecordView,
  type SystemInfo,
  type SystemStateView,
  type TransitMode,
  type Vec2,
} from "../../protocol";
import type { Net } from "../../net";
import { state } from "../../state";
import { recordForBattleReport, reportForBattleRecord } from "../../battlehistory";
import { HYPERLIMIT_SU } from "./geo";
import { cargoMultiplier, isBlueprintOnly, isModuleGood, sensorMultiplier, tankMultiplier, utilityProgramme } from "./equipment";

// Mirror of the sim's commodity value-rank (also in render.ts) — for flavor
// text and the observed fuel fallback. Client-only; no server data.
export const COMMODITY_VALUE: Record<Commodity, number> = {
  biomass: 5, silicates: 9, metallic_ore: 8, volatiles: 9, rare_elements: 100,
  cuprite_ore: 24, titanium_ore: 110, crystalline_ore: 6, rare_metal_ore: 1700,
  olivine_ore: 10, bornite_ore: 21, ilmenite_ore: 19,
  iridium_ore: 630, helium3_ice: 260, void_crystal: 1680,
  conductive_metals: 30, titanium: 60, iridium: 90, helium3: 75, exotic_matter: 120,
  provisions: 9, deuterium: 14, polymers: 16, alloys: 26, electronics: 34,
  machinery: 62, armaments: 56,
  composites: 50, hull_sections: 128, precision_components: 92,
  uranium: 5.6, antimatter: 120,
  // §hull-ladder: a light engine pair is 1,000 (reactor 650 + drive 350), the rungs 1.3/2.0/2.8× that; a heavy part
  // is 25 light and the assembly's 2%, a capital part 25 heavy and 2% again (sim fuel::EngineSize::price_factor).
  fission_reactor: 650, fusion_reactor: 1300, advanced_fusion_reactor: 1960, antimatter_reactor: 2720,
  zephyr_drive: 350, gale_drive: 700, tempest_drive: 1060, maelstrom_drive: 1470,
  heavy_fission_reactor: 16575, heavy_fusion_reactor: 33150, heavy_advanced_fusion_reactor: 49980, heavy_antimatter_reactor: 69360, heavy_zephyr_drive: 8925, heavy_gale_drive: 17850, heavy_tempest_drive: 27030, heavy_maelstrom_drive: 37485, heavy_hull_sections: 3264,
  capital_fission_reactor: 422662, capital_fusion_reactor: 845325, capital_advanced_fusion_reactor: 1.27449e+06, capital_antimatter_reactor: 1.76868e+06, capital_zephyr_drive: 227588, capital_gale_drive: 455175, capital_tempest_drive: 689265, capital_maelstrom_drive: 955868, capital_hull_sections: 83232,
  nav_computer: 215, cargo_bay: 150, pulse_laser: 110, beam_battery: 520,
  // §shipbuilding-overhaul Phase 5: the munitions.
  torpedoes: 72, slugs: 30, beam_crystals: 22,
  // §modules-as-goods (Nov 8 2026): a fifth over each module's recipe at base prices (sim market::base_price).
  mass_driver: 701, torpedo_rack: 917, point_defense_screen: 730, reflective_plating: 386, whipple_armor: 638,
  extended_tanks: 826, recon_suite: 874, cargo_pods: 1238, escort_datalink: 1474, fuel_transfer_rig: 1646,
  survey_drive: 1934, nebula_spectrometer: 931, prismatic_lance: 1332, lance_array: 1584, stasis_web: 758,
  warp_stabilizer: 797, interdiction_field: 1270,
};

/** §shipbuilding-overhaul Phase 5: the magazines a fleet carries (owner-only),
 * one row per munition something aboard takes: Torpedoes for the racks, Slugs
 * for the drivers, Beam Crystals in the beam guns. A dry rack or driver falls
 * back to the kit beams; worn crystals fire softer, never nothing. */
export const MUNITIONS: readonly Commodity[] = ["torpedoes", "slugs", "beam_crystals"];
export function magazines(g: GhostView): { good: Commodity; have: number; capacity: number; dry: boolean }[] {
  const rows: { good: Commodity; have: number; capacity: number; dry: boolean }[] = [];
  const add = (good: Commodity, have: number | null | undefined, capacity: number | null | undefined, exhausts: boolean) => {
    if (capacity == null || capacity <= 0) return;
    rows.push({ good, have: have ?? 0, capacity, dry: exhausts && (have ?? 0) < 1 });
  };
  add("torpedoes", g.torpedoes, g.torpedoes_capacity, true);
  add("slugs", g.slugs, g.slugs_capacity, true);
  add("beam_crystals", g.crystals, g.crystals_capacity, false);
  return rows;
}

// §reactors-drives: mirrors crates/sim/src/fuel.rs ReactorGrade and Drive.
export const REACTOR_FUEL: Record<ReactorGrade, Commodity> = { fission: "uranium", fusion: "deuterium", helium: "helium3", antimatter: "antimatter" };
export const REACTOR_GOOD: Record<ReactorGrade, Commodity> = { fission: "fission_reactor", fusion: "fusion_reactor", helium: "advanced_fusion_reactor", antimatter: "antimatter_reactor" };
/** Energy a unit of each rung's fuel holds, in Uranium. */
export const REACTOR_ENERGY: Record<ReactorGrade, number> = { fission: 1, fusion: 4, helium: 24, antimatter: 96 };
/** Energy aboard per unit of hull mass, relative to Fission: doubles a rung. */
export const REACTOR_ENERGY_ABOARD: Record<ReactorGrade, number> = { fission: 1, fusion: 2, helium: 4, antimatter: 8 };
export const REACTOR_LABEL: Record<ReactorGrade, string> = { fission: "Fission", fusion: "Fusion", helium: "Adv. Fusion", antimatter: "Antimatter" };
export const REACTOR_GRADES: readonly ReactorGrade[] = ["fission", "fusion", "helium", "antimatter"];
export const DRIVE_GOOD: Record<Drive, Commodity> = { zephyr: "zephyr_drive", gale: "gale_drive", tempest: "tempest_drive", maelstrom: "maelstrom_drive" };
export const DRIVE_LABEL: Record<Drive, string> = { zephyr: "Zephyr", gale: "Gale", tempest: "Tempest", maelstrom: "Maelstrom" };
/** The drive's multiplier on rated speed; energy a distance is its square. */
export const DRIVE_SPEED: Record<Drive, number> = { zephyr: 1, gale: 1.2, tempest: 1.4, maelstrom: 1.6 };
export const DRIVES: readonly Drive[] = ["zephyr", "gale", "tempest", "maelstrom"];
export const reactorRung = (r: ReactorGrade): number => REACTOR_GRADES.indexOf(r) + 1;
export const driveRung = (d: Drive): number => DRIVES.indexOf(d) + 1;
/** The power rule: a reactor runs a drive of its rung or below. */
export const reactorPowers = (r: ReactorGrade, d: Drive): boolean => reactorRung(r) >= driveRung(d);
/** The rung whose reactor burns `fuel`. */
export const gradeBurning = (fuel: Commodity): ReactorGrade | undefined => REACTOR_GRADES.find((g) => REACTOR_FUEL[g] === fuel);
/** §reactors-drives: the rung a good belongs to — a reactor, a drive or a fuel
 * — else 0. Rung 1 is known from the start; rungs 2–4 arrive with one
 * programme each (sim fuel::rung_programme). */
export function goodRung(c: Commodity): number {
  const r = REACTOR_GRADES.find((g) => REACTOR_GOOD[g] === c || REACTOR_FUEL[g] === c);
  if (r) return reactorRung(r);
  const d = DRIVES.find((k) => DRIVE_GOOD[k] === c);
  return d ? driveRung(d) : 0;
}
export const RUNG_PROGRAMME: Record<number, string> = { 2: "prop_bunkerage", 3: "mat_foundry_iv_fusion_reactors", 4: "mat_foundry_vi_antimatter_synthesis" };
/** §hull-ladder: engines and frames come in three sizes (sim fuel::EngineSize);
 * a heavy part is 25 light ones, a capital part 25 heavy. The sized goods are
 * the light slug with a size prefix. */
export type EngineSize = "light" | "heavy" | "capital";
export const SIZE_PROGRAMME: Record<Exclude<EngineSize, "light">, string> = { heavy: "hull_line_v_cruiser", capital: "hull_line_vii_dreadnought" };
export const SIZE_FACTOR: Record<EngineSize, number> = { light: 1, heavy: 25, capital: 625 };
export function goodSize(c: Commodity): EngineSize {
  return c.startsWith("heavy_") ? "heavy" : c.startsWith("capital_") ? "capital" : "light";
}
/** The light good a sized part is made of (itself for a light good). */
export const lightBase = (c: Commodity): Commodity => c.replace(/^(heavy|capital)_/, "") as Commodity;
export const sizedGood = (base: Commodity, size: EngineSize): Commodity => (size === "light" ? base : `${size}_${base}`) as Commodity;
/** Peter (Oct 28 2026): nothing research has not unlocked is rendered
 * outside the Research screen. A good of rung 2–4 is unknown until its
 * programme is complete, a heavy or capital part until its size's line is;
 * everything else is always known. */
export function goodKnown(c: Commodity): boolean {
  // §modules-as-goods (sim World::good_known): a module is known once research opens it, an arrived blueprint
  // counts; the military modules need none.
  if (isModuleGood(c)) {
    if (state.research?.blueprints?.includes(c)) return true;
    if (isBlueprintOnly(c)) return false;
    const programme = utilityProgramme(c);
    return !programme || researched(programme);
  }
  const size = goodSize(c);
  if (size !== "light" && !researched(SIZE_PROGRAMME[size])) return false;
  const rung = goodRung(lightBase(c));
  return rung <= 1 || researched(RUNG_PROGRAMME[rung]);
}
/** §reactors-drives: a recipe names rung 1 (Fission Reactor, Zephyr Drive) in the hull's size; the yard fits the
 * reactor and drive the build chose. */
export function engineGoodFor(c: Commodity, reactor: ReactorGrade, drive: Drive): Commodity {
  const base = lightBase(c);
  if (base === "fission_reactor") return sizedGood(REACTOR_GOOD[reactor], goodSize(c));
  if (base === "zephyr_drive") return sizedGood(DRIVE_GOOD[drive], goodSize(c));
  return c;
}
export const reactorKnown = (r: ReactorGrade): boolean => goodKnown(REACTOR_GOOD[r]);
export const driveKnown = (d: Drive): boolean => goodKnown(DRIVE_GOOD[d]);

// Mirrors the sim's fuel-cost model (crates/sim/src/fuel.rs + ship.rs).
export const FUEL_PER_MASS_DISTANCE = 1.0e-6;
const FUEL_PER_HULL_MASS = 0.035;
export const WARP_FACTOR = 5;
export const AAA_FUEL_PRICE_MULT = 3;
// §fuel-overhaul: mirrors crates/sim/src/fuel.rs and transit.rs.
export const SLOW_SPEED_FRACTION = 0.5;
export const SLOW_FUEL_MULT = 0.25;
export const FUSION_FREIGHT_MULT = 0.1;
export const HELIUM3_FUEL_EQUIV = 6;
export const JUMP_FUEL_FACTOR = 10;
export const AAA_SERVICE_FEE = 1_000;
const AAA_RESERVE_FRAC = 0.10;
export const HULL_MASS: Record<ShipKind, number> = {
  tiny_freighter: 1500, small_freighter: 2500, large_freighter: 8000, heavy_freighter: 14000, bulk_freighter: 24000,
  convoy: 4500, raider: 200, corvette: 800, colony: 6000, scout: 80, survey_probe: 40, mining_barge: 3500,
  mining_belt_station: 12000, combat_freighter: 2000,
  destroyer: 2000, cruiser: 4000, battleship: 8000, dreadnought: 16000, titan: 32000,
  transport: 7000, freighter: 6000, builder: 2500,
};
// Mirrors ShipKind::max_speed. Command previews use the slowest hull in the
// served composition and label the result as an estimate; the sim remains the
// authority for drive transitions, wells, fuel, and future route changes.
export const HULL_BASE_SPEED: Record<ShipKind, number> = {
  tiny_freighter: 40, small_freighter: 40, large_freighter: 38, heavy_freighter: 34, bulk_freighter: 30,
  convoy: 40, builder: 35, raider: 100, corvette: 65, colony: 33, scout: 115, survey_probe: 120, mining_barge: 30,
  mining_belt_station: 15, combat_freighter: 70,
  destroyer: 55, cruiser: 45, battleship: 36, dreadnought: 29, titan: 23,
  transport: 30, freighter: 32,
};
// Mirrors Commodity::mass_per_unit (cargo.rs): what a unit weighs in a hold.
// Raw ore, ice and harvest 28; refined bulk 14; precision parts, cores and
// modules 7; fuels and munitions 4. Hold room is its own rule (commodityRoom).
const MASS_RAW = 28, MASS_BULK = 14, MASS_PARTS = 7, MASS_SUPPLY = 4;
const RAW_GOODS: ReadonlySet<Commodity> = new Set<Commodity>([
  "metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore",
  "olivine_ore", "bornite_ore", "ilmenite_ore", "iridium_ore", "void_crystal",
  "silicates", "volatiles", "biomass", "helium3_ice",
]);
const PART_GOODS: ReadonlySet<Commodity> = new Set<Commodity>([
  "electronics", "precision_components", "nav_computer", "pulse_laser", "beam_battery", "exotic_matter",
  "fission_reactor", "fusion_reactor", "advanced_fusion_reactor", "antimatter_reactor",
  "zephyr_drive", "gale_drive", "tempest_drive", "maelstrom_drive",
]);
const FUEL_GOODS: ReadonlySet<Commodity> = new Set<Commodity>(["uranium", "deuterium", "helium3", "antimatter"]);
const MUNITION_GOODS: ReadonlySet<Commodity> = new Set<Commodity>(["torpedoes", "slugs", "beam_crystals"]);
/** Commodity::is_compact: precision parts, cores and modules (light base). */
const compactGood = (base: Commodity): boolean => PART_GOODS.has(base) || isModuleGood(base);
export const commodityMass = (c: Commodity): number => {
  // §hull-ladder: a sized part weighs its parts (no fee on mass).
  const base = lightBase(c);
  return SIZE_FACTOR[goodSize(c)]
    * (FUEL_GOODS.has(base) || MUNITION_GOODS.has(base) ? MASS_SUPPLY : RAW_GOODS.has(base) ? MASS_RAW
      : compactGood(base) ? MASS_PARTS : MASS_BULK);
};
/** §hold-room (Nov 10 2026): mirrors Commodity::room_quarters — the room one
 * unit fills, in quarters of a room unit. Ore, ice and harvest 4 (a whole
 * unit); refined bulk, Provisions and fuel 2; parts, cores, modules and
 * munitions 1. A heavy part packs as its light part; a capital part takes
 * ten room units (ROOM_CAPITAL). */
export const ROOM_SCALE = 4;
export const ROOM_CAPITAL = 10 * ROOM_SCALE;
export const commodityRoom = (c: Commodity): number => {
  if (goodSize(c) === "capital") return ROOM_CAPITAL;
  const base = lightBase(c);
  return MUNITION_GOODS.has(base) || compactGood(base) ? 1 : FUEL_GOODS.has(base) ? 2 : RAW_GOODS.has(base) ? 4 : 2;
};
/** Quarters a manifest fills. */
export const manifestRoom = (goods: Iterable<{ commodity: Commodity; units: number }>): number => {
  let q = 0;
  for (const g of goods) q += g.units * commodityRoom(g.commodity);
  return q;
};
/** Quarters of a {good: units} basket. */
export const basketRoom = (basket: Partial<Record<Commodity, number>>): number =>
  Object.entries(basket).reduce((q, [c, n]) => q + (n ?? 0) * commodityRoom(c as Commodity), 0);
/** Room the goods aboard fill, in quarters (Fleet::hold_used). */
export const fleetHoldUsed = (g: GhostView): number => manifestRoom(fleetCargoManifest(g));
/** Units of `c` that fit in `capacity` room units with `used` quarters filled (HoldRoom::fits). */
export const roomFits = (c: Commodity, capacity: number, used: number): number =>
  Math.max(0, Math.floor((capacity * ROOM_SCALE - used) / commodityRoom(c)));
/** Units of `c` a dock, market or route may still load (Fleet::trade_room). */
export const tradeRoomFits = (g: GhostView, c: Commodity): number => roomFits(c, fleetTradeCapacity(g), fleetHoldUsed(g));
/** The hold's rule, for tooltips. */
export const HOLD_ROOM_TIP = "Room a unit: ore 1 · refined goods, fuel 0.5 · parts, modules, munitions 0.25 · capital parts 10";
/** Room in quarters as "43.5" (Peter, Nov 10 2026: "use decimals not fractions"). */
export const fmtRoom = (q: number): string =>
  (Math.max(0, q) / ROOM_SCALE).toLocaleString(undefined, { maximumFractionDigits: 2 });
/** The hold's mass, each good at its own weight (Fleet::cargo_mass). */
export const fleetCargoMass = (g: GhostView): number =>
  fleetCargoManifest(g).reduce((mass, c) => mass + c.units * commodityMass(c.commodity), 0);
export const CARGO_UNITS_PER_FREIGHTER = 400; // legacy Medium hull; use cargoUnitsPerHull for a family member

export const fleetHullMass = (g: GhostView): number => g.composition
  ? g.composition.reduce((mass, ship) => mass + HULL_MASS[ship.kind] * ship.count, 0)
  : HULL_MASS[g.kind];
export const shipMass = (g: GhostView): number =>
  fleetHullMass(g) + (g.own ? fleetCargoMass(g) : 0);
export const fleetBaseSpeed = (g: GhostView): number => driveSpeedMult(g) * Math.min(...(g.composition?.length
  ? g.composition : [{ kind: g.kind, count: 1 }]).filter(s => s.count > 0).map(stack => {
    // Drive Tuning speeds every hull; Military Drives and Torch Regime
    // warships; Express Charters freighters.
    const r = driveResearch(g);
    const base = HULL_BASE_SPEED[stack.kind] * r.speed * (WARSHIPS.includes(stack.kind) ? r.warshipSpeed : 1)
      * (isPlayerFreighter(stack.kind) ? r.freighterSpeed : 1);
    const fits = (g.loadouts ?? []).filter(f => f.kind === stack.kind && f.n > 0);
    const unfitted = fits.reduce((n, f) => n + f.n, 0) < stack.count ? base : Infinity;
    return Math.min(unfitted, ...fits.map(f => base * (f.modules.includes("survey_drive") ? 1.25 : 1)));
  }));
/** The Deuterium tank (the Fusion Reactors'), as served. §reactors-drives:
 * one tank per fuel aboard; an unreported tank is sized from the hulls on
 * the fleet's primary reactor, at that rung's energy a unit. */
export const fleetFuelCapacity = (g: GhostView): number => g.fuel_capacity
  ?? (primaryGrade(g) === "fusion" ? estimatedTank(g, "fusion") : 0);
/** A tank sized from every hull as if all rode `grade`: FUEL_PER_HULL_MASS of
 * energy per unit of hull mass, doubling a rung, in the fuel's units (ship.rs
 * `ReactorGrade::tank_per_hull_mass`); an Antimatter tank is at least its bottle. */
export function estimatedTank(g: GhostView, grade: ReactorGrade): number {
  const mass = fleetHullMass(g) + (g.loadouts ?? []).reduce((extra, s) => extra + HULL_MASS[s.kind] * s.n * (tankMultiplier(s.modules) - 1), 0);
  const cruise = FUEL_PER_HULL_MASS * REACTOR_ENERGY_ABOARD[grade] / REACTOR_ENERGY[grade];
  return mass * (grade === "antimatter" ? Math.max(cruise, 0.01) : cruise);
}
/** §reactors-drives: the hulls aboard by (reactor, drive) (owner-only), else
 * the flagship's default (the dark hulls on Antimatter + Maelstrom, everything
 * else on Fusion + Zephyr). */
export const drivesOf = (g: GhostView): DriveView[] => g.drives?.length ? g.drives
  : [g.kind === "raider" || g.kind === "scout"
    ? { reactor: "antimatter", drive: "maelstrom", count: stacksOf(g).reduce((n, s) => n + s.count, 0) }
    : { reactor: "fusion", drive: "zephyr", count: stacksOf(g).reduce((n, s) => n + s.count, 0) }];
/** The reactor most of the fleet rides. */
export const primaryGrade = (g: GhostView): ReactorGrade => drivesOf(g).reduce((best, d) => d.count > best.count ? d : best).reactor;
/** The slowest drive aboard: the formation flies on it. */
export const slowestDrive = (g: GhostView): Drive => drivesOf(g).reduce((slow, d) => driveRung(d.drive) < driveRung(slow) ? d.drive : slow, "maelstrom" as Drive);
/** Its multiplier on rated speed, and on energy a distance (the square). */
export const driveSpeedMult = (g: GhostView): number => DRIVE_SPEED[slowestDrive(g)];
export const driveEnergyMult = (g: GhostView): number => DRIVE_SPEED[slowestDrive(g)] ** 2;
/** One tank per fuel aboard, as served: `served` is null when the owner's
 * report hasn't arrived. */
export function tanks(g: GhostView): { fuel: Commodity; grade: ReactorGrade; served: number | null; have: number; capacity: number }[] {
  const out: { fuel: Commodity; grade: ReactorGrade; served: number | null; have: number; capacity: number }[] = [];
  const push = (grade: ReactorGrade, served: number | null | undefined, capacity: number | null | undefined) => {
    const cap = capacity ?? 0;
    if (cap > 0 || (served ?? 0) > 0) out.push({ fuel: REACTOR_FUEL[grade], grade, served: served ?? null, have: served ?? 0, capacity: cap });
  };
  push("fission", g.uranium, g.uranium_capacity);
  push("fusion", g.fuel, g.fuel_capacity);
  push("helium", g.helium3, g.helium3_capacity);
  push("antimatter", g.antimatter, g.antimatter_capacity);
  if (!out.length) {
    // No report yet: size the primary reactor's tank from the hulls.
    const grade = primaryGrade(g);
    out.push({ fuel: REACTOR_FUEL[grade], grade, served: null, have: 0, capacity: estimatedTank(g, grade) });
  }
  return out;
}

// §shipbuilding-overhaul: mirrors ship.rs. Slow flies half speed on any core
// at a quarter of the energy per distance. Savings are per distance, in
// addition to the throttle; do not multiply trip fuel by speed a second time.
const WARSHIPS: ShipKind[] = ["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
const stacksOf = (g: GhostView) => g.composition?.length ? g.composition : [{ kind: g.kind, count: 1 }];
const researched = (id: string): boolean => !!state.research?.programmes.some(p => p.id === id && p.state === "completed");
/** The owner's research on its drives (ship.rs FuelResearch). Own fleets
 * only: a rival's research is unknown, so its figures stay stock. */
function driveResearch(g: GhostView) {
  if (!g.own) return { freight: 1, torch: 1, consumption: 1, speed: 1, warshipSpeed: 1, freighterSpeed: 1 };
  const torch = researched("prop_expedition_iii_torch_regime");
  return {
    freight: researched("prop_line_fusion_freight") ? FUSION_FREIGHT_MULT : 1,
    // Torch Regime's thirst, eased by Long Patrol.
    torch: (torch ? 1.4 : 1) * (researched("prop_expedition_iv_long_patrol") ? 0.7 : 1),
    freighterSpeed: researched("prop_line_express_charters") ? 1.25 : 1,
    consumption: researched("prop_efficient_burns") ? 0.75 : 1,
    speed: researched("prop_drive_tuning") ? 1.1 : 1,
    warshipSpeed: (researched("prop_military_drives") ? 1.2 : 1) * (torch ? 1.3 : 1),
  };
}
/** §shipbuilding-overhaul: any formation throttles back to Slow. */
export const slowCapable = (g: GhostView): boolean => !g.tca && stacksOf(g).some(s => s.count > 0);
/** Stealth needs a Gale Drive or better on every hull. */
export const stealthCapable = (g: GhostView): boolean => !g.tca && drivesOf(g).every((d) => d.drive !== "zephyr");
/** Flying Slow: half speed, a quarter of the energy per distance. */
export const fliesSlow = (g: GhostView): boolean => g.transit === "slow" && slowCapable(g);
/** @deprecated the old name: Slow was the Uranium regime. */
export const onUranium = fliesSlow;
export const fusionFreightResearched = (): boolean => researched("prop_line_fusion_freight");
export const transitLabel = (mode: TransitMode = "full"): string =>
  mode === "slow" ? "Slow" : mode === "stealth" ? "Stealth" : "Full speed";
/** The fuel the fleet's reactors burn (its primary rung's). */
export const regimeFuel = (g: GhostView): Commodity => REACTOR_FUEL[primaryGrade(g)];
/** The multiplier on energy per distance on one store, weighed hull by hull
 * by mass (ship.rs cruise_mult_for): a quarter on Uranium; Fusion Freight's cut
 * for player freighters and their cargo; Torch Regime on warships. */
export function cruiseMultFor(g: GhostView, uranium: boolean): number {
  const r = driveResearch(g);
  const regime = (freighter: boolean) => uranium ? SLOW_FUEL_MULT : freighter ? r.freight : 1;
  const stacks = stacksOf(g).filter(s => s.count > 0);
  const cargo = g.own ? fleetCargoMass(g) : 0;
  const total = fleetHullMass(g) + cargo;
  // §reactors-drives: the slowest drive's square on every hull.
  const drive = driveEnergyMult(g);
  if (total <= 1e-9) return drive * regime(false);
  const hulls = stacks.reduce((sum, s) => sum + HULL_MASS[s.kind] * s.count
    * regime(isPlayerFreighter(s.kind)) * (WARSHIPS.includes(s.kind) ? r.torch : 1), 0);
  return drive * (hulls + cargo * regime(stacks.some(s => isPlayerFreighter(s.kind)))) / total;
}
export const cruiseFuelMultiplier = (g: GhostView): number => cruiseMultFor(g, onUranium(g));
/** Efficient Burns' cut on every burn, jumps included. */
export const fuelConsumption = (g: GhostView): number => driveResearch(g).consumption;
export const transitSpeedMultiplier = (g: GhostView): number =>
  fliesSlow(g) ? SLOW_SPEED_FRACTION : g.transit === "stealth" && stealthCapable(g) ? 0.5 : 1;
/** §fuel-overhaul: the Uranium tank (the Fission Reactors'), as served. */
export const fleetUraniumCapacity = (g: GhostView): number => g.uranium_capacity
  ?? (primaryGrade(g) === "fission" ? estimatedTank(g, "fission") : 0);
/** The fleet's primary tank — the fuel most of its hulls burn — as served.
 * `served` is null before the owner's report; `have` reads 0 then. */
export function regimeStore(g: GhostView): { fuel: Commodity; grade: ReactorGrade; served: number | null; have: number; capacity: number } {
  const grade = primaryGrade(g);
  return tanks(g).find((t) => t.grade === grade) ?? tanks(g)[0];
}
/** Antimatter one jump of `distance` burns from the bottle (mass × distance × 1e-7). */
export const jumpAntimatter = (g: GhostView, distance: number): number =>
  FUEL_PER_MASS_DISTANCE * distance * shipMass(g) / JUMP_FUEL_FACTOR * fuelConsumption(g);
/** §shipbuilding-overhaul: the Deuterium tank (Helium-3 has its own tank now). */
export function fusionTank(g: GhostView): { deuterium: number; helium3: number; used: number; capacity: number } {
  const deuterium = Math.max(0, g.fuel ?? 0);
  return { deuterium, helium3: g.helium3 ?? 0, used: deuterium, capacity: fleetFuelCapacity(g) };
}

const SHIP_KIND_LABEL: Record<ShipKind, string> = {
  tiny_freighter: "Tiny Freighter", small_freighter: "Small Freighter", large_freighter: "Large Freighter", heavy_freighter: "Heavy Freighter", bulk_freighter: "Bulk Freighter",
  convoy: "Medium Freighter", raider: "Interceptor", corvette: "Corvette", colony: "Colony Ship", scout: "Scout", survey_probe: "Survey Probe", mining_barge: "Mining Barge",
  destroyer: "Destroyer", cruiser: "Cruiser", battleship: "Battleship", dreadnought: "Dreadnought", titan: "Titan",
  transport: "Troop Transport", builder: "Construction Ship", freighter: "Authority Freighter",
  mining_belt_station: "Mining Belt Station", combat_freighter: "Combat Freighter",
};
export const shipKindLabel = (kind: ShipKind): string => SHIP_KIND_LABEL[kind] ?? kind;

const MERGE_COLOCATE_RADIUS = 80;
export type SalvoFamily = "beam" | "driver" | "torpedo";

export const battleViewerTimers: { close: number | null; aftermath: number | null } = {
  close: null,
  aftermath: null,
};

let netSource: () => Net | null = () => null;
export function bindFleetNet(source: () => Net | null): void {
  netSource = source;
}

/** Every hold aboard, loot-only included (mirrors `Fleet::cargo_capacity`). */
export function fleetCargoCapacity(g: GhostView): number {
  return holdCapacity(g, () => true);
}

/** §holds: the room a dock, a market or a route may fill — every hold but a
 * Combat Freighter's (mirrors `Fleet::trade_capacity`). */
export function fleetTradeCapacity(g: GhostView): number {
  return holdCapacity(g, (kind) => !isLootOnly(kind));
}

function holdCapacity(g: GhostView, counts: (kind: ShipKind) => boolean): number {
  // Capacity follows only the arrived fit, never a refit preview or its ETA.
  const podded = (kind: ShipKind) => (g.loadouts ?? []).filter((s) => s.kind === kind)
    .reduce((sum, s) => sum + s.n * (cargoMultiplier(s.modules) - 1), 0);
  // §syndicate-projects (R6): the Freight bonus widens player freighter holds,
  // §holds: Prize Holds a Combat Freighter's, rounded down per hull kind as
  // the sim does.
  const mult = (kind: ShipKind) => isPlayerFreighter(kind) ? g.cargo_mult ?? 1 : isLootOnly(kind) ? g.prize_mult ?? 1 : 1;
  return (g.composition?.length ? g.composition : [{ kind: g.kind, count: 1 }])
    .filter((ship) => counts(ship.kind))
    .reduce((sum, ship) => {
      const units = cargoUnitsPerHull(ship.kind) * (ship.count + podded(ship.kind));
      return sum + (mult(ship.kind) > 1 ? Math.floor(units * mult(ship.kind) + 1e-9) : units);
    }, 0);
}


export function estimatedFuelForLeg(g: GhostView, dest: Vec2): number {
  const distance = Math.hypot(dest.x - g.pos.x, dest.y - g.pos.y);
  const logisticsMult = g.captain
    ? 1 - Math.min(0.10, Math.max(0, g.captain.attributes.logistics - 1) * 0.02)
    : 1;
  const warp = FUEL_PER_MASS_DISTANCE * distance * shipMass(g) / WARP_FACTOR;
  const wellDistance = Math.min(distance, HYPERLIMIT_SU * 2);
  const wellSurcharge = FUEL_PER_MASS_DISTANCE * wellDistance * shipMass(g) * (1 - 1 / WARP_FACTOR);
  return (warp + wellSurcharge) * logisticsMult * cruiseFuelMultiplier(g) * fuelConsumption(g);
}


export function aaaEstimate(g: GhostView): { fuel: number; cost: number } {
  // §fuel-overhaul: the rescue delivers the fuel the fleet's regime burns.
  // §shipbuilding-overhaul: the leg estimate is energy; the tank holds units.
  const store = regimeStore(g);
  const room = Math.max(0, store.capacity - store.have);
  const destination = g.path?.at(-1)?.pos;
  const packageFuel = destination
    ? Math.min(room, estimatedFuelForLeg(g, destination) / REACTOR_ENERGY[store.grade] + store.capacity * AAA_RESERVE_FRAC)
    : room;
  const marketFuel = state.market?.prices.find((price) => price.commodity === store.fuel)?.price ?? COMMODITY_VALUE[store.fuel];
  return { fuel: packageFuel, cost: AAA_SERVICE_FEE + packageFuel * marketFuel * AAA_FUEL_PRICE_MULT };
}


/** §charting: can this fleet survey a system? A Scout surveys and returns; a
 * Survey Probe surveys once and is spent. */
export function surveyCapable(g: Pick<GhostView, "kind" | "composition">): boolean {
  const surveyor = (kind: ShipKind) => kind === "scout" || kind === "survey_probe";
  return g.composition ? g.composition.some((c) => surveyor(c.kind) && c.count > 0) : surveyor(g.kind);
}

/** §belts: does this fleet carry a Mining Barge (and so can work a belt)? */
export function mineCapable(g: Pick<GhostView, "kind" | "composition">): boolean {
  return g.composition ? g.composition.some((c) => c.kind === "mining_barge" && c.count > 0) : g.kind === "mining_barge";
}

/** §pockets: mirror of the sim's PROSPECT_SECS (a prospecting pass's dwell). */
export const PROSPECT_SECS = 60;

/** §pockets: a fleet with a Scout or a Mining Barge can prospect a belt. */
export function prospectCapable(g: Pick<GhostView, "kind" | "composition">): boolean {
  const prospector = (kind: ShipKind) => kind === "scout" || kind === "mining_barge";
  return g.composition ? g.composition.some((c) => prospector(c.kind) && c.count > 0) : prospector(g.kind);
}

type BeltDeposit = { resource: Commodity; richness: number; reserves?: number | null };
/** §belts: where a barge would work in a charted system: the belt and ore
 * with the most value per second (richness × price), counting the pockets
 * you have found that still hold ore. Null with no ore belt. */
export function beltMiningTarget(system: { bodies: { id: number; kind: string; deposits?: BeltDeposit[] | null; pockets?: BeltDeposit[] }[] } | undefined): { body: number; ore: Commodity; pocket: boolean } | null {
  let best: { body: number; ore: Commodity; pocket: boolean; value: number } | null = null;
  for (const body of system?.bodies ?? []) {
    if (body.kind !== "asteroid_belt") continue;
    const rows = [...(body.deposits ?? []).map(d => ({ d, pocket: false })),
      ...(body.pockets ?? []).filter(d => (d.reserves ?? 1) > 0).map(d => ({ d, pocket: true }))];
    for (const { d, pocket } of rows) {
      const value = d.richness * (COMMODITY_VALUE[d.resource] ?? 0);
      if (!best || value > best.value) best = { body: body.id, ore: d.resource, pocket, value };
    }
  }
  return best && { body: best.body, ore: best.ore, pocket: best.pocket };
}

/** A fleet that would SPEND a probe on its survey (no Scout aboard). */
export function surveySpendsProbe(g: Pick<GhostView, "kind" | "composition">): boolean {
  const has = (kind: ShipKind) => g.composition ? g.composition.some((c) => c.kind === kind && c.count > 0) : g.kind === kind;
  return !has("scout") && has("survey_probe");
}

/** §reactors-drives: a fleet jumps when every hull turns a Maelstrom Drive;
 * mirrors ship.rs can_jump. */
export function jumpCapable(g: GhostView): boolean {
  const drives = drivesOf(g);
  return g.own && drives.length > 0 && drives.every((d) => d.drive === "maelstrom");
}

/** Peter, Nov 12 2026: no Jump command until a hull aboard turns a Maelstrom
 * Drive. With one aboard but not on every hull it shows, disabled. */
export function hasJumpDrive(g: GhostView): boolean {
  return g.own && drivesOf(g).some((d) => d.drive === "maelstrom");
}

/** Why a fleet with a Maelstrom aboard can't jump as formed. */
export const JUMP_MIXED_REASON = "Every hull needs a Maelstrom Drive.";


export function guardCapable(g: GhostView): boolean {
  const combatants: ShipKind[] = ["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
  return g.own && (combatants.includes(g.kind)
    || !!g.composition?.some(s => s.count > 0 && combatants.includes(s.kind)));
}
 // matches COLONY_CLAIM_RADIUS on the server
export function coLocatedOwnFleet(g: GhostView): GhostView | null {
  let best: GhostView | null = null;
  let bestD = MERGE_COLOCATE_RADIUS;
  for (const o of state.ghosts) {
    if (!o.own || o.id === g.id) continue;
    const d = Math.hypot(o.pos.x - g.pos.x, o.pos.y - g.pos.y);
    if (d <= bestD) {
      best = o;
      bestD = d;
    }
  }
  return best;
}


export function shipRoleLore(g: GhostView): string {
  if (g.kind === "colony") {
    return "Colonists + infrastructure. Buy the Authority's claim on an unowned system, then send it: on arrival the system becomes yours and the ship is consumed (it becomes the colony). Without the claim it holds there. It broadcasts its voyage — slow, visible, raidable — so escort it. If someone settles the target first, it holds there intact; redirect it.";
  }
  if (g.kind === "corvette") {
    return "A dedicated defender: any raid contact on one of your freighters within its protect radius must fight through this corvette first. Park it beside a freighter as an escort or at an owned system as a garrison; it cannot raid.";
  }
  if (g.kind === "scout") {
    const sensors = sensorMultiplier(g) * (state.galaxy?.sensor_range ?? 80_000);
    return `Discovers sites and surveys worlds. ${sensors > 0 ? `Recon sensors: ${Math.round(sensors).toLocaleString()} su.` : "Fit a Recon Suite for mobile sensors."} Retreats from expedition threats.`;
  }
  return "";
}


export function hubDockedFleets(): GhostView[] {
  // Docking is intentionally the SERVED report. A newly arrived fleet does not
  // appear early, and a departed one remains listed until its departure light
  // reaches the command center.
  return state.ghosts
    .filter((g) => g.own && g.docked === "hub")
    .sort((a, b) => shipKindLabel(a.kind).localeCompare(shipKindLabel(b.kind)) || a.id.localeCompare(b.id));
}

// Sum a set of own ghosts' EXACT compositions into a per-kind tally.
export function sumOwnComposition(ghosts: GhostView[]): Map<ShipKind, number> {
  const m = new Map<ShipKind, number>();
  for (const g of ghosts) {
    const comp = g.composition ?? [{ kind: g.kind, count: 1 }];
    for (const c of comp) m.set(c.kind, (m.get(c.kind) ?? 0) + c.count);
  }
  return m;
}

/// The report counter and engagement id are separate spaces. The server carries
/// the explicit link; position cannot distinguish repeated/co-located battles.
export function recordForReport(r: BattleReportView): BattleRecordView | undefined {
  return recordForBattleReport(r, state.battleRecords);
}


export function clearBattleAftermathTimer(): void {
  if (battleViewerTimers.aftermath !== null) {
    clearTimeout(battleViewerTimers.aftermath);
    battleViewerTimers.aftermath = null;
  }
}


export function clearBattleCloseTimer(): void {
  if (battleViewerTimers.close !== null) {
    clearTimeout(battleViewerTimers.close);
    battleViewerTimers.close = null;
  }
}


export function battleReportForRecord(rec: BattleRecordView): BattleReportView | undefined {
  return reportForBattleRecord(rec, state.battleReports);
}

export function sideFamily(sv: SideRecordView): SalvoFamily {
  const mods = (sv.loadouts ?? []).flatMap((st) => st.modules);
  if (mods.includes("torpedo_rack")) return "torpedo";
  if (mods.includes("mass_driver")) return "driver";
  return "beam";
}


export function dockedFreighterStock(systemId: EntityId): Map<Commodity, number> {
  const cargo = new Map<Commodity, number>();
  for (const fleet of state.ghosts) {
    if (!fleet.own || !hauls(fleet) || !dockedAtSystem(fleet, systemId)) continue;
    for (const stack of fleetCargoManifest(fleet)) {
      cargo.set(stack.commodity, (cargo.get(stack.commodity) ?? 0) + stack.units);
    }
  }
  return cargo;
}


export function constructionStock(dyn: SystemStateView, projectKey?: string): {
  stockpile: Map<Commodity, number>;
  freighters: Map<Commodity, number>;
  available: Map<Commodity, number>;
} {
  const stockpile = new Map((dyn.stockpile ?? []).map((slot) => [slot.commodity, slot.units]));
  const freighters = dockedFreighterStock(dyn.id);
  const available = new Map(stockpile);
  for (const r of dyn.industry?.reservations ?? []) {
    // A matching recipe may spend its own reservation; every other build uses
    // unreserved stock plus the same arrived docked-Freighter cargo as before.
    if (r.target.kind === projectKey) continue;
    for (const [c,n] of Object.entries(r.goods)) available.set(c as Commodity, Math.max(0,(available.get(c as Commodity) ?? 0)-(n ?? 0)));
  }
  for (const [commodity, units] of freighters) {
    available.set(commodity, (available.get(commodity) ?? 0) + units);
  }
  return { stockpile, freighters, available };
}


// §economy Part 6 / §bodies: worker assignment control → SetAssignment. `spec` is
// "bodyId:slug:workers" — the line lives ON a body now; posted specialists are
// preserved server-side only if re-sent, so we send the current line's along.
export function sendCrew(systemId: EntityId, spec: string): void {
  const net = netSource();
  if (!net) return;
  const [bid, slug, n] = spec.split(":");
  const body_id = Number(bid);
  const dyn = state.systems.find((s) => s.id === systemId);
  const line = dyn?.assignments?.find((a) => a.body_id === body_id && a.structure === slug);
  net.send({ type: "SetAssignment", system_id: systemId, structure: slug, workers: Math.max(0, Number(n) || 0), specialists: line?.specialists ?? {}, body_id });
}


/// Own fleets the player's SERVED picture places at this system. Deliberately
/// never corrected from server truth or client projection: a departed fleet
/// remains listed until its departure light reaches the command center.
export function dockedAtSystem(g: GhostView, systemId: string): boolean {
  // DockSite currently reaches the client through Display ("E29") while
  // SystemInfo uses the EntityId wire form ("29"). Keep that transport quirk at
  // this seam so every system-facing client view still compares one identity.
  return g.docked === systemId || g.docked === `E${systemId}`;
}


export function systemFleetsAt(sys: SystemInfo): GhostView[] {
  return state.ghosts
    .filter((g) => g.own && (
      dockedAtSystem(g, sys.id)
      || (g.docked == null && Math.hypot(g.pos.x - sys.pos.x, g.pos.y - sys.pos.y) <= HYPERLIMIT_SU)
    ))
    .sort((a, b) => {
      const docked = Number(dockedAtSystem(b, sys.id)) - Number(dockedAtSystem(a, sys.id));
      if (docked !== 0) return docked;
      return shipKindLabel(a.kind).localeCompare(shipKindLabel(b.kind)) || a.id.localeCompare(b.id);
    });
}


export function fleetRosterDockName(g: GhostView): string | null {
  if (g.docked === "hub") return "Market Hub";
  if (!g.docked) return null;
  return state.galaxy?.systems.find((system) => dockedAtSystem(g, system.id))?.name ?? "known berth";
}


/// §dock: the hulls BERTHED at a dock, from the ghosts this viewer already has.
///
/// Derived client-side on purpose: the ghost list is already fog-filtered, so
/// counting it can neither invent intel nor lose any. The galaxy map stops
/// drawing these sprites and this count takes their place — the same
/// information, in a form you can actually read when six convoys are stacked on
/// one star. `site` is a system id, or "hub" for the Market Hub.
export function berthed(site: string): GhostView[] {
  return state.ghosts.filter((g) => dockedAtSystem(g, site));
}



/// §TCA Part 5: dockside LOGISTICS for one of the player's own convoys — load and
/// unload across the Market Warehouse or an owned system's stockpile, and
/// the haul order that sends a loaded hull to the Market Hub. Only offered when
/// the fleet is actually alongside a dock and idle; the sim soft-rejects anything
/// else, but there is no point showing a button that will only ever be refused.
/// Does this fleet lift cargo? UI mirror of the sim's `Fleet::cargo_capacity()`:
/// convoy HULLS carry, and it never consults the flagship. An escorted lot whose
/// flagship is a warship still hauls.
export function hauls(g: GhostView): boolean {
  // §belts: a Mining Barge's ore hopper is a hold too, unloaded the same way.
  const holds = (kind: ShipKind) => isPlayerFreighter(kind) || cargoUnitsPerHull(kind) > 0;
  return holds(g.kind) || !!g.composition?.some((c) => holds(c.kind) && c.count > 0);
}

export function dockLoadStock(g: GhostView): [Commodity, number][] {
  if (g.docked === "hub") {
    return (state.wallet?.warehouse ?? []).map((w) => [w.commodity, w.units]);
  }
  if (!g.docked) return [];
  const system = (state.galaxy?.systems ?? []).find((candidate) =>
    dockedAtSystem(g, candidate.id)
    && state.systems.find((served) => served.id === candidate.id)?.owner === state.playerId);
  if (!system) return [];
  const report = state.systems.find(entry => entry.id === system.id);
  return (report?.stockpile ?? []).map(slot => [slot.commodity,
    Math.max(0, slot.units - (report?.industry?.reservations ?? []).reduce((n,r) => n+(r.goods[slot.commodity] ?? 0),0))]);
}

/** §research-wiring: what intel research read off a rival contact, as short
 * lines — Traffic Analysis' cargo kind, Wake Analysis' tank and heading. */
export function contactIntel(g: GhostView): { carrying: string | null; wake: string | null } {
  const points = ["E", "ENE", "NE", "NNE", "N", "NNW", "NW", "WNW", "W", "WSW", "SW", "SSW", "S", "SSE", "SE", "ESE"];
  // The map draws the sim's +y downward, so +y reads as south.
  const from = (radians: number) => points[((Math.round(-radians / (Math.PI / 8)) % 16) + 16) % 16];
  return {
    carrying: g.manifest_class ? g.manifest_class.replace(/^\w/, (c) => c.toUpperCase()) : null,
    wake: g.wake ? `${Math.round(g.wake.fuel * 100)}% fuel · ${g.wake.from === null ? "holding still" : `from the ${from(g.wake.from)}`}` : null,
  };
}

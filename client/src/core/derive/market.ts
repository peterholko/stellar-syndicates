// Shared market derivations extracted from the desktop shell.

import { goodKnown } from "./fleet";
import type { Net } from "../../net";
import {
  cargoUnitsPerHull,
  type ShipKind,
  type BodyView,
  type BuildOption,
  type Commodity,
  type ConversionRecipe,
  type Deposit,
  type EntityId,
  type GhostView,
  type ModuleKind,
  type Side,
  type StockSlot,
  type SystemInfo,
  type SystemStateView,
  type TradeEvent, type ReactorGrade, type Drive, type LicenceView } from "../../protocol";
import { label } from "../../icons";
import { liveSimTime, state } from "../../state";
import { COMMODITY_VALUE, constructionStock, dockedAtSystem, fleetCargoCapacity, fleetFuelCapacity, fleetHoldUsed, fmtRoom, HULL_MASS, ROOM_SCALE } from "./fleet";
import { cargoMultiplier, hullUtilitySummary, isUtility, isBlueprintOnly, isModuleGood, MODULES, moduleFitsHull, tankMultiplier, utilityProgramme } from "./equipment";

const EMPLACE_KITS: Record<string, [Commodity, number][]> = {
  deep_space_sensor: [["alloys", 60], ["electronics", 120]],
};

export const MODULE_SLOTS: Record<string, number> = {
  tiny_freighter: 1, small_freighter: 1, large_freighter: 1, heavy_freighter: 1, bulk_freighter: 1,
  corvette: 2, raider: 2, scout: 1, convoy: 1, colony: 0,
  destroyer: 3, cruiser: 4, battleship: 4, dreadnought: 5, titan: 6,
};
const MODULE_FIT_COST: Record<ModuleKind, number> = {
  mass_driver: 2, torpedo_rack: 3, point_defense_screen: 2, reflective_plating: 2, whipple_armor: 3,
  extended_tanks: 2, recon_suite: 2, cargo_pods: 2, escort_datalink: 2, fuel_transfer_rig: 2,
  survey_drive: 2, nebula_spectrometer: 2, prismatic_lance: 3,
  stasis_web: 2, warp_stabilizer: 2, interdiction_field: 3, lance_array: 3,
};
export const FITTING_POINTS: Record<string, number> = {
  tiny_freighter: 2, small_freighter: 2, large_freighter: 2, heavy_freighter: 2, bulk_freighter: 2,
  corvette: 5, raider: 4, scout: 2, convoy: 2, colony: 2,
  destroyer: 8, cruiser: 12, battleship: 18, dreadnought: 28, titan: 45,
};
const fitCost = (mods: ModuleKind[]): number => mods.reduce((sum, module) => sum + (MODULE_FIT_COST[module] ?? 0), 0);

export const SHIP_YARD: Record<string, { yard: string; tier: number }> = {
  tiny_freighter: { yard: "shipyard", tier: 1 }, small_freighter: { yard: "shipyard", tier: 1 },
  large_freighter: { yard: "naval_drydock", tier: 1 }, heavy_freighter: { yard: "naval_drydock", tier: 2 }, bulk_freighter: { yard: "naval_drydock", tier: 3 },
  convoy: { yard: "shipyard", tier: 2 }, scout: { yard: "shipyard", tier: 1 }, colony: { yard: "shipyard", tier: 1 },
  survey_probe: { yard: "shipyard", tier: 1 }, mining_barge: { yard: "shipyard", tier: 1 },
  mining_belt_station: { yard: "shipyard", tier: 2 }, combat_freighter: { yard: "shipyard", tier: 2 },
  raider: { yard: "shipyard", tier: 1 }, corvette: { yard: "shipyard", tier: 2 },
  destroyer: { yard: "naval_drydock", tier: 1 }, cruiser: { yard: "naval_drydock", tier: 2 }, battleship: { yard: "naval_drydock", tier: 3 },
  dreadnought: { yard: "capital_slipway", tier: 1 }, titan: { yard: "capital_slipway", tier: 2 }, transport: { yard: "garrison", tier: 1 },
};
export const YARD_PREREQ: Readonly<Record<string, { yard: string; tier: number }>> = {
  naval_drydock: { yard: "shipyard", tier: 2 },
  capital_slipway: { yard: "naval_drydock", tier: 3 },
  ordnance_foundry: { yard: "shipyard", tier: 1 },
};
export const YARD_TITLE: Record<string, string> = {
  shipyard: "Shipyard", naval_drydock: "Orbital Shipyard", capital_slipway: "Capital Slipway",
  ordnance_foundry: "Ordnance Foundry", garrison: "Garrison",
};
export const slipsFor = (tier: number): number => Math.max(0, tier);
const HULL_PROGRAMME: Record<string, string> = {
  // §research-locked (Nov 11 2026): the Corvette's card, Escort Datalink.
  corvette: "hull_line_iii_escort_datalink",
  small_freighter: "prop_freight_frames", convoy: "prop_heavy_lifters", large_freighter: "prop_line_express_charters",
  heavy_freighter: "prop_line_bulk_charters", bulk_freighter: "prop_line_autonomous_freight",
  destroyer: "hull_line_iv_destroyer", cruiser: "hull_line_v_cruiser",
  battleship: "hull_line_vi_battleship", dreadnought: "hull_line_vii_dreadnought", titan: "hull_line_viii_titan",
  mining_belt_station: "mat_belt_stations",
};

export const POOL_OF: Record<string, "resource" | "industrial" | "infrastructure"> = {
  mining_complex: "resource", volatile_harvester: "resource", bioharvester: "resource", asteroid_quarry: "resource",
  smelter: "industrial", electronics_fabricator: "industrial", chemical_works: "industrial",
  fuel_refinery: "industrial", machine_works: "industrial", armaments_complex: "industrial",
  composite_works: "industrial", hull_fabricator: "industrial", precision_works: "industrial", drive_works: "industrial",
  uranium_mill: "industrial", antimatter_plant: "industrial",
  shipyard: "industrial", naval_drydock: "industrial", capital_slipway: "industrial", ordnance_foundry: "industrial",
  agroplex: "infrastructure", habitat: "infrastructure", living_terraces: "infrastructure", orbital_warehouse: "infrastructure",
  warehouse: "infrastructure", service_berth: "infrastructure", command_center: "infrastructure",
  sensor_array: "infrastructure", defense_platform: "infrastructure", orbital_defense: "infrastructure", academy: "infrastructure", garrison: "infrastructure",
  // §power: grouped with the civic utilities; it takes no slot (one per planet).
  power_plant: "infrastructure",
  // §vault: under the Command Center; no slot.
  vault: "infrastructure",
  // §season: at the Command Center; no slot.
  assembly_hall: "infrastructure",
  // §veil: off the Fusion Plant; no slot.
  veil_projector: "infrastructure",
};
const EXTRACTION_OF: Record<string, Commodity[]> = {
  mining_complex: ["metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "olivine_ore", "bornite_ore", "ilmenite_ore", "iridium_ore", "void_crystal", "silicates", "rare_elements"],
  volatile_harvester: ["volatiles", "helium3_ice"],
  bioharvester: ["biomass"],
};
// §asteroid-quarry: a belt's quarry works every deposit on it, so any deposit
// will do (sim `StructureKind::suits`).
EXTRACTION_OF.asteroid_quarry = [...EXTRACTION_OF.mining_complex, ...EXTRACTION_OF.volatile_harvester];
/** §asteroid-quarry: a belt takes its quarry and nothing else; the quarry
 * stands only on a belt (sim `StructureKind::suits`). */
export const suitsBody = (key: string, body: Pick<BodyView, "kind" | "environment">): boolean =>
  // §veil: an ionised sky needs a sky — no belt, gas giant or airless rock.
  key === "veil_projector" ? body.kind !== "asteroid_belt" && body.kind !== "gas_giant" && body.environment !== "uninhabitable"
    : (body.kind === "asteroid_belt") === (key === "asteroid_quarry");
export type Pool = "resource" | "industrial" | "infrastructure";
export type PoolUse = Record<Pool, { used: number; total: number }>;
export const POOL_LABEL: Record<Pool, string> = { resource: "Resource", industrial: "Industrial", infrastructure: "Infrastructure" };

export type BuildOpt = BuildOption;
// §gun-platform: the station stands in orbit and takes no site (sim `uses_slot`).
export const structureConsumesSlot = (key: string): boolean => !["service_berth", "command_center", "power_plant", "vault", "assembly_hall",
  "orbital_defense", "veil_projector"].includes(key);

/** §tier-curve: mirror of the sim's structure tier curve
 * (`build::TIER_COST_MULT` / `TIER_TIME_MULT`, index 0 = tier I). Past tier II
 * a structure's next tier costs more and takes longer. */
export const TIER_CURVE = {
  cost: [1, 1, 1.6, 2.56, 4.096, 6.5536],
  time: [1, 1, 20, 180, 900, 2700],
} as const;
/** §progression: an income structure's level costs what it returns
 * (`build::PRODUCTION_TIER_COST_MULT`); build times follow `TIER_CURVE.time`. */
export const PRODUCTION_TIER_CURVE = [1, 3.5, 9, 24, 44, 65] as const;
/** The three with explicit level ladders (`build::follows_tier_curve`). */
const TIER_CURVE_EXEMPT: ReadonlySet<string> = new Set(["command_center", "power_plant", "orbital_defense"]);

/** Explicit level recipes come from Welcome; never display/debit Level I's
 * price for an upgrade. Pending jobs count when selecting the proposed level.
 * Any other structure's level is its base recipe on the tier curve, rounded as
 * the sim rounds. */
export function structureLevel(option: BuildOpt, tier: number): BuildOpt {
  const levels = option.tiers;
  const level = levels?.find(entry => entry.tier === tier) ?? levels?.at(-1);
  if (level) return { ...option, ...level, research_prerequisite: level.research_prerequisite ?? undefined };
  const i = Math.min(Math.max(tier, 1), TIER_CURVE.cost.length) - 1;
  if (i === 0 || (!option.earns && i < 2) || !(option.key in POOL_OF) || TIER_CURVE_EXEMPT.has(option.key)) return option;
  const mult = option.earns ? PRODUCTION_TIER_CURVE[i] : TIER_CURVE.cost[i];
  return {
    ...option,
    costs: option.costs.map(cost => ({ ...cost, units: Math.round(cost.units * mult) })),
    build_secs: option.build_secs * TIER_CURVE.time[i],
  };
}
/** §first-session: mirror of the sim's charter opening builds
 * (`founding::OPENING_BUILDS`): while the charter runs, level I of these on the
 * home system's Command Center world rises in OPENING_BUILD_SECS. */
export const OPENING_BUILD_SECS = 10;
const OPENING_BUILDS: ReadonlySet<string> = new Set(["mining_complex", "bioharvester", "habitat", "agroplex", "power_plant"]);
export function openingBuild(key: string, tier: number, systemId: string, body: BodyView): boolean {
  if (tier !== 1 || !OPENING_BUILDS.has(key) || !((body.structures.command_center ?? 0) > 0)) return false;
  const stage = state?.founding?.stage;
  if (!stage || stage === "complete") return false;
  const at = state.galaxy?.systems?.find((system) => system.id === systemId)?.pos;
  const home = state.commandCenter;
  return !!at && !!home && Math.hypot(at.x - home.x, at.y - home.y) < 1;
}
export const structureBuildSeconds = (option: BuildOpt, body: BodyView, opening = false): number =>
  opening ? OPENING_BUILD_SECS
    : option.build_secs * (body.structure_build_time_mult ?? body.construction_time_mult);
export const MINERAL_DEPOSITS: Commodity[] = ["metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "olivine_ore", "bornite_ore", "ilmenite_ore", "iridium_ore", "void_crystal", "rare_elements", "silicates"];

/** §shipbuilding-overhaul: lines that run one of several standing recipes,
 * picked by the assignment's `refining_ore`: the Smelter by its ore, the Drive
 * Works, Hull Fabricator, Semiconductor Fab and Armaments Complex by product. */
export const PRODUCT_PICKERS: readonly string[] = ["drive_works", "hull_fabricator", "electronics_fabricator", "armaments_complex"];
export const recipeSelectable = (key: string): boolean => key === "smelter" || PRODUCT_PICKERS.includes(key);
const DEFAULT_PRODUCT: Record<string, Commodity> = { drive_works: "fission_reactor", hull_fabricator: "hull_sections",
  electronics_fabricator: "electronics", armaments_complex: "armaments" };
export const defaultRecipeKey = (key: string): Commodity => DEFAULT_PRODUCT[key] ?? "metallic_ore";
export const recipeKey = (key: string, recipe: ConversionRecipe): Commodity =>
  PRODUCT_PICKERS.includes(key) ? recipe.output : recipe.inputs[0]?.[0];

/** Peter (Nov 9 2026): "Do not list all the ore types until the player can
 * actually smelt/refine them". A Smelter draws on its system's stores, so it
 * offers an ore once the system can feed it: some in the stores, some made
 * there (mines, quarries, belt stations, barges' hauls), or a deposit under a
 * mine or quarry on one of its worlds. Ferrite, the default, and the line's
 * current recipe always. */
export function smelterOres(report: Pick<SystemStateView, "stockpile" | "flows" | "bodies">, current?: Commodity | null): Set<Commodity> {
  const ores = new Set<Commodity>([defaultRecipeKey("smelter")]);
  if (current) ores.add(current);
  for (const slot of report.stockpile ?? []) if (slot.units > 0) ores.add(slot.commodity);
  for (const flow of report.flows ?? []) if (flow.made > 0) ores.add(flow.commodity);
  for (const body of report.bodies ?? []) {
    if ((body.structures.mining_complex ?? 0) > 0 || (body.structures.asteroid_quarry ?? 0) > 0) {
      for (const deposit of body.deposits ?? []) ores.add(deposit.resource);
    }
  }
  return ores;
}

/** Recipe is chosen from the served assignment, never a pending UI draft. */
export function assignedRecipe(option: BuildOpt | undefined, ore?: Commodity | null): ConversionRecipe | undefined {
  const key = option?.key ?? "smelter";
  return option?.refining_recipes?.find(r => recipeKey(key, r) === (ore ?? defaultRecipeKey(key))) ?? option?.conversion;
}

export function recipeOutputs(recipe: ConversionRecipe): [Commodity, number][] {
  return [[recipe.output, 1], ...(recipe.byproducts ?? [])];
}

export function commodityPurpose(commodity: Commodity, catalog: readonly BuildOpt[]): string {
  const ore = catalog.find(o => o.key === "smelter")?.refining_recipes?.find(r => r.inputs[0]?.[0] === commodity);
  if (ore) return `Sell raw or refine into ${recipeOutputs(ore).map(([c]) => label(c)).join(" + ")}.`;
  if (commodity === "conductive_metals") return "Conductors for Semiconductor production.";
  if (commodity === "titanium") return "Structural metal for heavy Hull Sections.";
  return "";
}

/** Static recipe from Welcome, not a second client economy. Actual production
 * still follows the delayed workforce/inputs/tier report. */
export function conversionSummary(option: BuildOpt): string {
  const recipe = option.conversion;
  if (!recipe) return "";
  const inputs = recipe.inputs.map(([commodity, units]) => `${units} ${label(commodity)}`).join(" + ");
  const outputs = recipeOutputs(recipe).map(([c, n]) => `${n} ${label(c)}`).join(" + ");
  const choice = PRODUCT_PICKERS.includes(option.key) ? ` Choose the line's product on the planet's ${label(option.key)}.` : " Choose ore on the planet's Smelter.";
  return `${inputs || "No inputs"} → ${outputs} · base ${Number((recipe.rate * 60).toFixed(2))}/min at tier I, fully staffed.${option.refining_recipes?.length ? choice : ""}`;
}
export interface StructOpt {
  o: BuildOpt; pool: Pool; currentTier: number; targetTier: number;
  maxed: boolean;
  /** §unlocks: the next level is above the research ceiling; sheets leave the building out. */
  capped: boolean;
  foundsNew: boolean; tierUp: boolean; afford: boolean; poolFull: boolean; noDeposit: boolean;
  yardPrereq: { yard: string; tier: number; have: number } | null;
  buildable: boolean; reason: string;
  /** §build-sheet: the first gate other than goods that refuses this order, as a
   * short label ("No free Industrial site"); empty when only goods are missing. */
  gate: string;
}
export interface ShipOpt {
  o: BuildOpt; needTier: number; yardTier: number; yardShort: boolean; afford: boolean;
  maxAff: number; buildable: boolean; reason: string; slipsFull: boolean; slips: number; foundingLocked: boolean;
  buildRate: number; yardBody?: BodyView;
  /** §licences: the Authority's licence for this hull, as heard (none for a
   * ground-Shipyard hull), and whether its absence stops the keel. */
  licence?: LicenceView; unlicensed: boolean;
}

export const buildOption = (key: string): BuildOpt | undefined => state.galaxy?.build_options.find((option) => option.key === key);

export const COMMODITIES: Commodity[] = [
  "metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "volatiles", "biomass",
  "olivine_ore", "bornite_ore", "ilmenite_ore", "iridium_ore", "helium3_ice", "void_crystal",
  "rare_elements", "silicates", "conductive_metals", "titanium", "iridium", "helium3", "exotic_matter",
  "alloys", "electronics", "polymers", "uranium", "deuterium", "antimatter", "provisions", "machinery", "armaments",
  "composites", "hull_sections", "precision_components",
  // §reactors-drives: the reactor and drive ladders and the kit parts.
  "fission_reactor", "fusion_reactor", "advanced_fusion_reactor", "antimatter_reactor",
  "zephyr_drive", "gale_drive", "tempest_drive", "maelstrom_drive",
  "nav_computer", "cargo_bay", "pulse_laser", "beam_battery",
  // §shipbuilding-overhaul Phase 5: the munitions.
  "torpedoes", "slugs", "beam_crystals",
  // §hull-ladder: heavy and capital engines and frames (hidden until the
  // Cruiser's and the Dreadnought's lines).
  "heavy_fission_reactor", "heavy_fusion_reactor", "heavy_advanced_fusion_reactor", "heavy_antimatter_reactor", "heavy_zephyr_drive", "heavy_gale_drive", "heavy_tempest_drive", "heavy_maelstrom_drive", "heavy_hull_sections",
  "capital_fission_reactor", "capital_fusion_reactor", "capital_advanced_fusion_reactor", "capital_antimatter_reactor", "capital_zephyr_drive", "capital_gale_drive", "capital_tempest_drive", "capital_maelstrom_drive", "capital_hull_sections",
  // §modules-as-goods (Nov 8 2026): the modules (equipment.ts MODULE_KINDS).
  "mass_driver", "torpedo_rack", "point_defense_screen", "reflective_plating", "whipple_armor", "extended_tanks", "recon_suite",
  "cargo_pods", "escort_datalink", "fuel_transfer_rig", "survey_drive", "nebula_spectrometer", "prismatic_lance", "stasis_web",
  "warp_stabilizer", "interdiction_field", "lance_array",
];
/** §reactors-drives: the goods a catalogue, book or picker may list — every
 * commodity but a reactor, drive or fuel research has not opened yet
 * (Peter, Oct 28 2026: nothing research has not unlocked is rendered). */
export const knownCommodities = (): Commodity[] => COMMODITIES.filter(goodKnown);

export function ownedHaulDestinations(): { id: EntityId; name: string }[] {
  const owned = new Set(
    state.systems
      .filter((system) => system.owner === state.playerId)
      .map((system) => system.id),
  );
  return (state.galaxy?.systems ?? [])
    .filter((system) => owned.has(system.id))
    .map((system) => ({ id: system.id, name: system.name }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

/** An order in flight to the Hub, held off the warehouse count until its
 * receipt. §treasury: no credits: the order paid as it left. */
export type MarketReservation = {
  kind: "market" | "limit"; side: Side; commodity: Commodity; orderUnits: number;
  units: number; limitPrice?: number; issuedAt: number;
};
export type RecentMarketOrder = {
  side: Side; commodity: Commodity; units: number; unitPrice: number; limitFill: boolean; observedAt: number;
};
const RECENT_MARKET_ORDER_LIMIT = 8;
export const recentMarketOrders: RecentMarketOrder[] = [];
export const marketReservations: MarketReservation[] = [];
const PRICE_HISTORY_CAP = 60;
const MARKET_DEPTH = 1600;
const MARKET_HALF_SPREAD = 0.01;
export const freightDraft = new Map<Commodity, number>();

let netSource: () => Net | null = () => null;
let pendingFitSource: () => ModuleKind[] = () => [];
let pendingReactorSource: () => ReactorGrade = () => "fission";
let pendingDriveSource: () => Drive = () => "zephyr";
export function bindMarketDerive(source: () => Net | null, fitSource: () => ModuleKind[], reactorSource: () => ReactorGrade = () => "fission",
  driveSource: () => Drive = () => "zephyr"): void {
  pendingReactorSource = reactorSource;
  pendingDriveSource = driveSource;
  netSource = source;
  pendingFitSource = fitSource;
}


// Pretty "40 Alloys + 60 Semiconductors + 30 Deuterium" for tooltips and refusals.
export function kitCostLabel(kind: string): string {
  return (EMPLACE_KITS[kind] ?? [])
    .map(([c, n]) => `${n} ${label(c)}`)
    .join(" + ");
}


// §emplacements: does ANY system of ours cover the full kit? Mirrors the
// server's charge rule (one system pays for everything; no pooling across
// systems). Stockpiles are owner-only in the view, so `stockpile` is present
// exactly for the systems this rule may draw from.
export function kitAffordable(kind: string): boolean {
  const kit = EMPLACE_KITS[kind];
  if (!kit) return true;
  return state.systems.some(
    (s) => s.stockpile !== null && kit.every(([c, n]) => (s.stockpile!.find((x) => x.commodity === c)?.units ?? 0) >= n),
  );
}


// --- Star System view (SYSTEM tab) — a master→detail workspace (§4, §9) -------
// The galaxy map is the master list (click a system); this tab is the detail:
// header + light-gated ownership + stat strip + geology readout + production
// readout (owner-only) + valid context actions, plus an owned-systems rail when
// you hold several. Fog-safe: ownership/stockpile use exactly the light-gated
// fields the View already provides; a rival's system shows only that it's held.
// One delegated listener (set once) survives the per-render innerHTML rewrites.

// Eyebrow flavor, derived client-side from position + OUR known geology
// (§explore: the exact composition is survey knowledge — unsurveyed systems
// show only the public band).
export function systemFlavor(sys: SystemInfo, deps: Deposit[] | null): string {
  const frac = state.galaxy ? Math.hypot(sys.pos.x, sys.pos.y) / state.galaxy.radius : 0;
  const tier = frac > 0.6 ? "frontier" : frac > 0.33 ? "mid-rim" : "core";
  if (deps === null) return `unsurveyed ${tier}`;
  if (!deps.length) return "barren system";
  const dom = deps.reduce((a, b) =>
    a.richness * COMMODITY_VALUE[a.resource] >= b.richness * COMMODITY_VALUE[b.resource] ? a : b);
  return `${label(dom.resource)}-rich ${tier}`;
}

// §fitting: is (kind, mods) legal — both slots and budget? (mirrors Loadout::validate)
export function fitLegal(kind: string, mods: ModuleKind[]): boolean {
  return mods.length <= (MODULE_SLOTS[kind] ?? 0) && fitCost(mods) <= (FITTING_POINTS[kind] ?? 0)
    && (!mods.includes("escort_datalink") || mods.includes("point_defense_screen"))
    && mods.filter(m => m === "interdiction_field").length <= 1
    && mods.every(m => MODULES.some(entry => entry.kind === m) && moduleFitsHull(m, kind) && (!isUtility(m) || mods.filter(other => other === m).length === 1));
}

export function moduleBuildReason(dyn: SystemStateView, module: ModuleKind): string {
  if (isBlueprintOnly(module) && !state.research?.blueprints?.includes(module)) return "Recover blueprint through exploration";
  if (!moduleResearched(module)) return "Research required";
  const yard = isUtility(module) ? "shipyard" : "armaments_complex";
  if (!(dyn.structures[yard] > 0)) return isUtility(module) ? "Needs Shipyard I" : "Needs Armaments Complex";
  if (isUtility(module) && !dyn.bodies.some(body => shipyardBoost(dyn, body) > 0)) return "Assign Shipyard workforce";
  // Module recipes draw on the local stockpile, not docked commodity holds.
  const stock = new Map((dyn.stockpile ?? []).map(s => [s.commodity, s.units]));
  const recipe = buildOption(`module:${module}`);
  return !recipe || recipe.costs.some(c => (stock.get(c.commodity as Commodity) ?? 0) < c.units) ? "Insufficient stock" : "";
}

export function refitFuelBlocked(g: GhostView, from: ModuleKind[], to: ModuleKind[]): boolean {
  return typeof g.fuel === "number" && g.fuel / Math.max(fleetFuelCapacity(g), 1e-9) * tankMultiplier(from) > tankMultiplier(to) + 1e-9;
}

export function refitCargoCapacity(g: GhostView, hull: string, from: ModuleKind[], to: ModuleKind[], n: number): number {
  return fleetCargoCapacity(g) + cargoUnitsPerHull(hull as ShipKind) * n * (cargoMultiplier(to) - cargoMultiplier(from));
}

export function refitCargoBlocked(g: GhostView, hull: string, from: ModuleKind[], to: ModuleKind[], n: number): boolean {
  return fleetHoldUsed(g) > refitCargoCapacity(g, hull, from, to, n) * ROOM_SCALE; // §hold-room: Fleet::cargo_fits
}

/** The pooled manifest can allow removing one set of pods but not a whole
 * fitted stack. Clamp the selector to the actual arrived free hold space. */
export function maxCargoRefitCount(g: GhostView, hull: string, from: ModuleKind[], to: ModuleKind[], available: number): number {
  const loss = cargoUnitsPerHull(hull as ShipKind) * (cargoMultiplier(from) - cargoMultiplier(to));
  return loss > 0 ? Math.max(0, Math.min(available, Math.floor((fleetCargoCapacity(g) * ROOM_SCALE - fleetHoldUsed(g)) / (loss * ROOM_SCALE)))) : available;
}

export function refitPreview(g: GhostView, hull: import("../../protocol").ShipKind, from: ModuleKind[], to: ModuleKind[], n: number): string {
  const oldCap = fleetFuelCapacity(g);
  const cap = oldCap + HULL_MASS[hull] * .035 * n * (tankMultiplier(to) - tankMultiplier(from));
  const detail = hullUtilitySummary(hull, from, to);
  const fuel = Math.abs(cap - oldCap) > 1e-9 ? ` · Fleet tank ${oldCap.toFixed(1)} → ${cap.toFixed(1)} Fuel · aboard ${g.fuel?.toFixed(1) ?? "unknown"} unchanged` : "";
  const oldCargo = fleetCargoCapacity(g);
  const newCargo = refitCargoCapacity(g, hull, from, to, n);
  const cargo = oldCargo !== newCargo ? `Cargo ${oldCargo} → ${newCargo} · aboard ${fmtRoom(fleetHoldUsed(g))} unchanged` : "";
  return `${detail}${fuel}${cargo ? `${detail || fuel ? " · " : ""}${cargo}` : ""}${detail || fuel || cargo ? " · " : ""}Refit ${3 * n}s after receipt${refitFuelBlocked(g, from, to) ? " · Use fuel before removing tanks" : ""}${refitCargoBlocked(g, hull, from, to, n) ? " · Unload cargo before removing pods" : ""}`;
}

export function utilityRefitChoices(g: GhostView, dock: SystemStateView) {
  if (!g.own || !dockedAtSystem(g, dock.id)) return [];
  const ledger = moduleLedgerAt(dock.id);
  const stacks = (g.composition ?? []).flatMap(stack => {
    const fitted = (g.loadouts ?? []).filter(s => s.kind === stack.kind && s.n > 0);
    const stock = stack.count - fitted.reduce((sum, s) => sum + s.n, 0);
    return [...fitted, ...(stock > 0 ? [{ kind: stack.kind, modules: [] as ModuleKind[], n: stock }] : [])];
  });
  return stacks.flatMap(stack => MODULES.filter(m => isUtility(m.kind)).flatMap(module => {
    if (!utilityWorkshop(dock, stack.kind)) return [];
    const has = stack.modules.includes(module.kind);
    let to = has ? stack.modules.filter(m => m !== module.kind) : [...stack.modules, module.kind];
    if (!has && !fitLegal(stack.kind, to)) to = [...stack.modules.filter(m => !isUtility(m)), module.kind];
    if (!fitLegal(stack.kind, to) || (!has && !(ledger[module.kind] > 0))) return [];
    return [{ ship: stack.kind, from: stack.modules, to, module: module.kind,
      name: `${has ? "Remove" : "Fit"} ${module.name}`, blocked: refitFuelBlocked(g, stack.modules, to) || refitCargoBlocked(g, stack.kind, stack.modules, to, 1),
      preview: refitPreview(g, stack.kind, stack.modules, to, 1) }];
  }));
}

/** A berth can borrow a mining posting, not food crew. Eligibility is a served
 * estimate; actual ownership, crew and crates are checked again on receipt. */
export function utilityWorkshop(dock: SystemStateView, hull: string): boolean {
  return dock.bodies.some(b => shipyardBoost(dock, b) > 0) ||
    dock.owner === state.playerId && ["scout", "raider", "tiny_freighter", "small_freighter"].includes(hull)
    && dock.bodies.some(b => (b.structures.warehouse ?? 0) > 0 && (b.structures.service_berth ?? 0) > 0)
    && dock.assignments.some(a => ["service_berth", "mining_complex"].includes(a.structure) && a.workers > 0);
}

// A module's goods VALUE = its recipe commodities priced at the observed hub
// market (the same basis the sim uses), or null if the price board isn't in yet.
export function moduleRecipeValue(m: ModuleKind): number | null {
  const o = buildOption(`module:${m}`);
  if (!o || !state.market) return null;
  const price = new Map(state.market.prices.map((p) => [p.commodity, p.price]));
  let v = 0;
  for (const c of o.costs) {
    const p = price.get(c.commodity as Commodity);
    if (p === undefined) return null;
    v += c.units * p;
  }
  return v;
}

// §modules-as-goods (Nov 8 2026): the whole modules in a system's stockpile (owner-only; {} if unseen).
export function moduleLedgerAt(sid: string): Record<string, number> {
  const stock = state.systems.find((s) => s.id === sid)?.stockpile ?? [];
  return Object.fromEntries(stock.filter((line) => isModuleGood(line.commodity) && line.units >= 1)
    .map((line) => [line.commodity, Math.floor(line.units)]));
}

export function bodyPoolTotals(b: BodyView): Record<"resource" | "industrial" | "infrastructure", number> {
  return {
    resource: b.resource_slots ?? 0,
    industrial: b.industrial_slots ?? 0,
    infrastructure: b.infrastructure_slots ?? 0,
  };
}

/// THIS body's pools, counting built structures AND pending NEW-structure jobs
/// (mirrors pool_slots_built + pool_slots_pending — tier-ups are exempt).
export function bodyPoolUsage(b: BodyView, dyn: SystemStateView | undefined): PoolUse {
  const totals = bodyPoolTotals(b);
  const used = { resource: 0, industrial: 0, infrastructure: 0 };
  for (const [slug, t] of Object.entries(b.structures ?? {})) {
    if (t > 0 && POOL_OF[slug] && structureConsumesSlot(slug)) used[POOL_OF[slug]] += 1;
  }
  const pending = new Set((dyn?.builds ?? [])
    .filter((j) => j.body_id === b.id && structureConsumesSlot(j.key) && POOL_OF[j.key] && ((b.structures ?? {})[j.key] ?? 0) === 0)
    .map((j) => j.key));
  for (const slug of pending) used[POOL_OF[slug]] += 1;
  return {
    resource: { used: used.resource, total: totals.resource },
    industrial: { used: used.industrial, total: totals.industrial },
    infrastructure: { used: used.infrastructure, total: totals.infrastructure },
  };
}

/// The summary strip: pools SUMMED across the roster (a system fact).
export function poolUsage(dyn: SystemStateView | undefined): PoolUse {
  const sum = { resource: { used: 0, total: 0 }, industrial: { used: 0, total: 0 }, infrastructure: { used: 0, total: 0 } };
  for (const b of dyn?.bodies ?? []) {
    const totals = bodyPoolTotals(b);
    for (const k of ["resource", "industrial", "infrastructure"] as const) sum[k].total += totals[k];
    for (const [slug, t] of Object.entries(b.structures ?? {})) {
      if (t > 0 && POOL_OF[slug] && structureConsumesSlot(slug)) sum[POOL_OF[slug]].used += 1;
    }
  }
  return sum;
}

// Shared by catalogue visibility and build validation. Only a served completion
// unlocks a structure; queued/active research (or its estimated end) never does.
export function structureResearched(o: BuildOpt): boolean {
  return !o.research_prerequisite || state.research?.programmes.some(
    (entry) => (entry.id === o.research_prerequisite || o.research_alternatives?.includes(entry.id)) && entry.state === "completed",
  ) === true;
}

/** §unlocks (Oct 28 2026, Peter: "Anything not available to the player until
 * a research unlocks it should not be rendered"): the highest level of `key`
 * this corporation may build now (sim `max_buildable_tier`, served v72).
 * Unknown until research arrives. */
export const structureCeiling = (key: string): number => state?.research?.structure_tiers?.[key] ?? Infinity;
/** A structure the catalogues may show: researched and unlocked. */
export const structureUnlocked = (o: BuildOpt): boolean => structureResearched(o) && structureCeiling(o.key) >= 1
  // §vault: opens with the finished charter (sim `founding::charter_step`); unseen until then.
  && !(o.key === "vault" && state?.founding?.charter_gates?.vault)
  // §season: the Assembly Hall too, and only once the levies give the colonists a grievance.
  && !(o.key === "assembly_hall" && (state?.founding?.charter_gates?.assembly_hall || (state?.season?.grip ?? "charter") === "charter"));
/** A module whose research, if any, is done; the forges list no other. */
export function moduleResearched(module: ModuleKind): boolean {
  const programme = utilityProgramme(module);
  return !programme || !!state?.research?.programmes.some(p => p.id === programme && p.state === "completed");
}

/// The sim-mirroring state for building `o` on `body` — current/target tier,
/// whether it founds a NEW slot vs. deepens in place, and every precondition
/// (afford / pool-full / matching-deposit). `buildable` = all pass (Queue is
/// live); `reason` is the first failing gate. Lifted out of the old inline build
/// rows so the list, the detail, and the button can never disagree.
export function structOption(o: BuildOpt, dyn: SystemStateView, body: BodyView, pools: PoolUse): StructOpt {
  const pool = POOL_OF[o.key];
  const currentTier = (body.structures ?? {})[o.key] ?? 0;
  const pendingAhead = (dyn.builds ?? []).filter((j) => j.body_id === body.id && j.key === o.key).length;
  const foundsNew = currentTier === 0 && pendingAhead === 0;
  const targetTier = currentTier + pendingAhead + 1;
  const maxed = !!o.tiers?.length && targetTier > Math.max(...o.tiers.map(level => level.tier));
  const capped = !maxed && targetTier > structureCeiling(o.key);
  o = structureLevel(o, targetTier);
  const have = constructionStock(dyn, o.key).available;
  const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
  const attachment = o.key === "service_berth";
  const poolFull = (o.consumes_slot ?? structureConsumesSlot(o.key)) && foundsNew && !!pool && pools[pool].used >= pools[pool].total;
  const extractsFrom = EXTRACTION_OF[o.key];
  const noDeposit = foundsNew && !!extractsFrom && !(body.deposits ?? []).some((d) => extractsFrom.includes(d.resource as Commodity));
  // §yards: a yard needs the one below it standing on the same SYSTEM (not this
  // body — the ladder belongs to a shipbuilding world, so it may spread across
  // bodies). Checked on every tier, so a Drydock can't outgrow its Shipyard.
  // §orbital-defense: a station level names its own yard.
  const pre = o.requires ?? YARD_PREREQ[o.key];
  const preHave = pre ? (dyn.structures?.[pre.yard] ?? 0) : 0;
  const yardPrereq = pre && preHave < pre.tier ? { ...pre, have: preHave } : null;
  const stationReason = o.key === "orbital_defense" ? stationBlock(dyn, body, targetTier) : "";
  const research = o.research_prerequisite;
  const researchLocked = !structureResearched(o);
  // The Welcome catalog names even still-locked research. Only the arrived
  // completion state unlocks construction; an active/queued estimate never does.
  const researchTitle = [research, ...(o.research_alternatives ?? [])].filter((id): id is string => !!id)
    .map(id => state.research?.programmes.find(entry => entry.id === id)?.name
      ?? state.researchCatalog?.find(entry => entry.id === id)?.name ?? label(id)).join(" or ");
  const attachmentReason = attachment && targetTier > 1 ? "Service Berth has one tier." : attachment && !(body.structures.warehouse > 0) ? "Requires a Warehouse on this world. Uses no extra slot." : "";
  // §vault: dug under the world's Command Center.
  const vaultReason = (o.key === "vault" || o.key === "assembly_hall") && !((body.structures.command_center ?? 0) > 0)
    ? o.key === "vault" ? "Dug under a Command Center." : "Stands at a Command Center." : "";
  const populationReason = o.min_population && (body.population * 1e6 + 1e-6 < o.min_population || body.kind === "gas_giant")
    ? `Requires ${o.min_population.toLocaleString()} residents on a solid-surface world.` : "";
  // §gaia: Living Terraces grow only on a Gaia world.
  const environmentReason = o.requires_environment && body.environment !== o.requires_environment
    ? `Grows only on ${label(o.requires_environment)} worlds.` : "";
  // §asteroid-quarry: a belt takes its quarry and nothing else.
  const groundReason = suitsBody(o.key, body) ? ""
    : o.key === "asteroid_quarry" ? "An Asteroid Quarry stands on an asteroid belt."
      : o.key === "veil_projector" ? "A Veil needs an atmosphere to ionise." : "A belt takes only an Asteroid Quarry.";
  // §charter-v4: the charter's land grant covers only the landing world until
  // "Expand mining operations" (sim World::apply_build).
  const landing = (dyn.bodies ?? []).find((b) => (b.structures?.command_center ?? 0) > 0);
  const grantReason = state.founding?.land_grant_open === false && dyn.owner === state.playerId && landing && body.id !== landing.id
    ? `Charter land grant: ${landing.name} only, until you expand mining.` : "";
  // §charter-gates: the charter opens its structures step by step (sim
  // `founding::charter_step`, served as `charter_gates`).
  const charterStep = dyn.owner === state.playerId ? state.founding?.charter_gates?.[o.key] : undefined;
  const charterReason = charterStep ? `The charter opens this at step ${charterStep}.` : "";
  const buildable = !maxed && !capped && !charterReason && !grantReason && !groundReason && !populationReason && !environmentReason && !attachmentReason && !vaultReason && !stationReason && !researchLocked && !poolFull && !noDeposit && !yardPrereq && afford;
  const reason = (maxed ? (o.key === "orbital_defense" ? "The Star Fortress is the station's top level."
    : o.key === "command_center" ? "Maximum Command Center level reached (III)." : "Top level reached.") : "")
    || charterReason || grantReason || groundReason || populationReason || environmentReason || attachmentReason || vaultReason || stationReason || (researchLocked ? `Requires ${researchTitle} research.`
    : noDeposit ? "No matching deposit on this body — a mine only works its own rock."
    : yardPrereq ? `Needs ${YARD_TITLE[yardPrereq.yard] ?? yardPrereq.yard} tier ${yardPrereq.tier} somewhere in this system (have ${yardPrereq.have}).`
    : poolFull ? `This body's ${POOL_LABEL[pool]} slots are full (${pools[pool].used}/${pools[pool].total}).`
      : !afford ? "Not enough goods available at this system." : "");
  // The same gates, in the same order, as the build sheet's short labels.
  const gate = (maxed ? "Top level" : "")
    || (charterReason ? `Charter step ${charterStep}` : "")
    || (grantReason ? `Land grant: ${landing!.name} only` : "")
    || (groundReason ? o.key === "asteroid_quarry" ? "Belts only" : "Belt: quarry only" : "")
    || (populationReason ? `Needs ${o.min_population!.toLocaleString()} residents` : "")
    || (environmentReason ? `${label(o.requires_environment!)} worlds only` : "")
    || (attachmentReason ? targetTier > 1 ? "Top level" : "Needs a Warehouse" : "")
    || (vaultReason ? "Needs a Command Center" : "")
    || (stationReason ? stationReason.includes("another world") ? "Station orbits another world" : "One Star Fortress per corporation" : "")
    || (researchLocked ? `Needs ${researchTitle} research` : "")
    || (noDeposit ? "No matching deposit" : "")
    || (yardPrereq ? `Needs ${YARD_TITLE[yardPrereq.yard] ?? label(yardPrereq.yard)} ${["", "I", "II", "III", "IV", "V", "VI"][yardPrereq.tier] ?? yardPrereq.tier}` : "")
    || (poolFull ? `No free ${POOL_LABEL[pool]} site` : "");
  return { o, pool, currentTier, targetTier, maxed, capped, foundsNew, tierUp: !foundsNew, afford, poolFull, noDeposit, yardPrereq, buildable, reason, gate };
}

/** §orbital-defense: why the station can't rise to `targetTier` on `body`
 * (empty: it can). Mirrors the sim: one station per system, and one Star
 * Fortress per corporation, standing or rising. */
export function stationBlock(dyn: SystemStateView, body: BodyView, targetTier: number): string {
  const elsewhere = (dyn.bodies ?? []).some((b) => b.id !== body.id && (b.structures?.orbital_defense ?? 0) > 0)
    || (dyn.builds ?? []).some((j) => j.key === "orbital_defense" && j.body_id !== body.id);
  if (elsewhere) return "The system's station orbits another world; raise that one.";
  // §gun-platform: the Star Fortress is level 6.
  if (targetTier >= 6) {
    const fortress = state.systems.some((s) => s.id !== dyn.id && s.owner === state.playerId
      && ((s.station?.level ?? 0) >= 6 || ((s.station?.level ?? 0) === 5 && (s.builds ?? []).some((j) => j.key === "orbital_defense"))));
    if (fortress) return "Your corporation already has its Star Fortress.";
  }
  return "";
}

/// Ship gating mirrored from the sim: shipyard-tier gate (SHIP_REQ vs the system's
/// shipyard tier — the same field the old inline rows read) + afford, plus the
/// max affordable count for the quantity stepper. `buildable` = tier ok + affords 1.
// Capitals and every freighter above Tiny need their own completed programme.
// Queued/active research is not an unlock. Mirrors the sim's NeedsResearch gate.
export function hullResearched(key: string): boolean {
  const prog = HULL_PROGRAMME[key];
  if (!prog) return true;
  return state.research?.programmes.find((p) => p.id === prog)?.state === "completed";
}

/** §licences (docs/blueprint-licences.md): research is the skill to build a
 * hull, the Authority's licence the right to. Every Orbital Shipyard and
 * Capital Slipway hull needs one; this is its licence as heard, or undefined
 * for a hull that needs none. */
export function licenceFor(key: string): LicenceView | undefined {
  return state.charter?.licences?.find((l) => l.hull === key);
}
/** A licence that stops a keel: needed, not held, and not at war (a
 * syndicate at war with the Authority needs none). */
export const licenceMissing = (l: LicenceView | undefined): boolean => !!l && !l.held && l.refusal?.reason !== "at_war";
/** Can the Authority sell this licence now? */
export const licenceForSale = (l: LicenceView | undefined): boolean => !!l && !l.held && !l.refusal;
const credits = (n: number) => Math.round(n).toLocaleString("en-US");
/** One short line for a licence: held, its price, or why not. Empty for a
 * hull research hasn't unlocked (nothing it hasn't unlocked is shown). */
export function licenceNote(l: LicenceView | undefined): string {
  if (!l || l.refusal?.reason === "needs_research") return "";
  if (l.held) return "Licensed";
  switch (l.refusal?.reason) {
    case "at_war": return "None needed · at war";
    case "charter_closed": return "Suspended · charter revoked";
    case "needs_commissioned": return `Commissioned only · ${credits(l.price)} Cr`;
    default: return `${credits(l.price)} Cr`;
  }
}
/** Why a missing licence stops the keel, for the build card. */
function licenceReason(l: LicenceView): string {
  if (l.refusal?.reason === "charter_closed") return "Licences are suspended while your charter is revoked.";
  if (l.refusal?.reason === "needs_commissioned") return "The Authority licenses capitals only at Commissioned standing.";
  return "Needs the Authority's licence.";
}

export function shipOption(o: BuildOpt, dyn: SystemStateView): ShipOpt {
  const have = constructionStock(dyn, o.key).available;
  // §yards: read the gating yard's tier from the owner-only structures map
  // (`shipyard_tier` only ever knew about the Shipyard).
  const gate = SHIP_YARD[o.key] ?? { yard: "shipyard", tier: 1 };
  const yardTier = dyn.structures?.[gate.yard] ?? 0;
  const needTier = gate.tier;
  const yardShort = yardTier < needTier;
  const unresearched = !hullResearched(o.key);
  const programme = HULL_PROGRAMME[o.key];
  const researchTitle = state.research?.programmes.find(p => p.id === programme)?.name
    ?? state.researchCatalog?.find(p => p.id === programme)?.name
    ?? label(programme ?? "");
  const foundingLocked = o.key === "colony" && !!state.founding && !state.founding.expansion_unlocked;
  const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
  const maxAff = o.costs.length
    ? Math.max(0, Math.min(...o.costs.map((c) => Math.floor((have.get(c.commodity as Commodity) ?? 0) / c.units))))
    : 0;
  // §yards M1: SLIPWAYS. Hulls in progress at this system that this same yard
  // gates occupy its slips; a full yard can't lay another keel until one frees.
  const slips = slipsFor(yardTier);
  const occupied = (dyn.builds ?? []).filter((j) => SHIP_YARD[j.key] && (j.yard ?? SHIP_YARD[j.key]?.yard) === gate.yard).length;
  const slipsFull = !yardShort && occupied >= slips;
  // Match the server's actual construction site (highest gating-yard tier,
  // last body on ties), not whichever planet the workbench happens to show.
  const yardBody = dyn.bodies?.reduce<BodyView | undefined>((best, body) =>
    !best || (body.structures?.[gate.yard] ?? 0) >= (best.structures?.[gate.yard] ?? 0) ? body : best, undefined);
  const buildRate = yardBody ? shipyardBoost(dyn, yardBody, gate.yard) : 0;
  const licence = licenceFor(o.key);
  const unlicensed = !unresearched && licenceMissing(licence);
  const buildable = !foundingLocked && !yardShort && !unresearched && !unlicensed && !slipsFull && afford;
  const reason = foundingLocked
    ? "Complete the Founding Programme to unlock expansion."
    : unresearched
    ? `Requires ${researchTitle} research.`
    : unlicensed ? licenceReason(licence!)
    : yardShort ? `Needs ${YARD_TITLE[gate.yard] ?? gate.yard} tier ${needTier} (have ${yardTier}).`
    : slipsFull ? `All ${slips} slipway${slips === 1 ? "" : "s"} busy — raise the ${YARD_TITLE[gate.yard] ?? gate.yard} or wait for a hull to launch.`
    : !afford ? "Not enough goods available at this system."
    : buildRate <= 0 ? `Queued hulls wait for workforce at ${YARD_TITLE[gate.yard] ?? gate.yard}${yardBody ? ` · ${yardBody.name}` : ""}.` : "";
  return { o, needTier, yardTier, yardShort: yardShort || unresearched, afford, maxAff, buildable, reason, slipsFull, slips, foundingLocked, buildRate, yardBody, licence, unlicensed };
}

/// Mirrors sim::production::shipyard_work_rate. These are served factors, used
/// only for a prospective quote; in-flight progress uses its own arrived segment.
export function shipyardBoost(dyn: SystemStateView, body: BodyView, yard = "shipyard"): number {
  const a = (dyn.assignments ?? []).find((x) => x.body_id === body.id && x.structure === yard);
  // §power: a yard works at its planet's grid.
  return (body.structures?.[yard] ?? 0) > 0 && a && a.staffing > 0 ? (1 + 0.25 * a.staffing * a.skill) * (body.power?.power ?? 1) : 0;
}


// §step1 build sink, shared by every build UI (the System View management
// column, its contextual body offers, and the rail's remaining paths):
// Every SHIP_YARD hull except the ground-only transport → BuildShip;
// developments → DevelopSystem. This sink is shared by Deck AND mobile, so
// capital hulls must stay on the ship command path in both shells. Same
// system-level commands as always — no UI adds a new gameplay verb.
export function dispatchBuildKey(k: string, sid: string, bodyId?: number): void {
  const net = netSource();
  if (!net) return;
  if (k in SHIP_YARD && k !== "transport") {
    // §modules Part B4: a warship build carries the composed FIT, clamped to this
    // hull's module slots (so a 2-module fit on a 1-slot scout sends just 1, not a
    // silent server reject). The ledger is debited server-side.
    const fit = pendingFitSource().filter((m) => (moduleLedgerAt(sid)[m] ?? 0) > 0 && moduleFitsHull(m, k)).slice(0, MODULE_SLOTS[k] ?? 0);
    if (!fitLegal(k, fit)) return;
    // §reactors-drives: and the reactor and drive the card chose.
    const reactor = pendingReactorSource();
    const drive = pendingDriveSource();
    net.send({ type: "BuildShip", system_id: sid, ship_kind: k as import("../../protocol").ShipKind, loadout: fit.length ? fit : undefined,
      reactor: reactor === "fission" ? undefined : reactor, drive: drive === "zephyr" ? undefined : drive });
  }
  // §modules Part B3: "module:<slug>" → manufacture into the system's stockpile (§modules-as-goods).
  else if (k.startsWith("module:")) net.send({ type: "BuildModule", system_id: sid, module: k.slice(7) as ModuleKind });
  // §bodies: the body panel names its body; omitted → the sim auto-sites.
  else net.send({ type: "DevelopSystem", system_id: sid, upgrade: k, body_id: bodyId }); // §economy: any structure slug
}


// What "Ship production → hub" will ACTUALLY dispatch: the system's NON-FUEL
// stock in whole units. MIRRORS the sim's apply_ship_production rule — the
// ship fuels (Uranium, Deuterium, Antimatter) are retained as the system's
// operating reserve (they power movement; sell them via the Market), so they
// must neither light the button nor be promised in feedback.
// The View's stockpile is already owner-only whole units, so this is exact.
export const SHIP_FUELS: readonly Commodity[] = ["uranium", "deuterium", "helium3", "antimatter"];
export function shippableStock(dyn: SystemStateView | undefined): StockSlot[] {
  return (dyn?.stockpile ?? []).filter((s) => !SHIP_FUELS.includes(s.commodity) && s.units >= 1);
}

export function pruneMarketReservations(): void {
  // Until the receipt: the order's light out and the report's light back.
  const ttl = Math.max(5, 2 * (state.market?.staleness ?? 0) + 5);
  const now = liveSimTime();
  for (let i = marketReservations.length - 1; i >= 0; i--) {
    if (now - marketReservations[i].issuedAt > ttl) marketReservations.splice(i, 1);
  }
}

/** §treasury: the credits at your command center. An order to the Hub pays
 * as it leaves, so nothing in flight is held back here. */
export function spendableMarketCredits(): number {
  return state.credits ?? 0;
}

/** §treasury: what an order for market-priced goods (modules, freight, a
 * rescue) carries above the price, as the sim's `PAYMENT_MARGIN`. The change
 * comes back with the receipt. */
export const PAYMENT_MARGIN = 0.10;

export function reserveMarketOrder(reservation: Omit<MarketReservation, "issuedAt">): void {
  marketReservations.push({ ...reservation, issuedAt: liveSimTime() });
}

export function settleMarketReservation(trade: TradeEvent): void {
  let kind: MarketReservation["kind"] | null = null;
  let side: Side | null = null;
  let commodity: Commodity | null = null;
  if (trade.event === "Bought") [kind, side, commodity] = ["market", "buy", trade.commodity];
  else if (trade.event === "Sold") [kind, side, commodity] = ["market", "sell", trade.commodity];
  else if (trade.event === "LimitPlaced") [kind, side, commodity] = ["limit", trade.side, trade.commodity];
  else if (trade.event === "Rejected") commodity = trade.commodity;
  else return;
  const index = marketReservations.findIndex((reservation) =>
    reservation.commodity === commodity
      && (kind === null || reservation.kind === kind)
      && (side === null || reservation.side === side));
  if (index >= 0) marketReservations.splice(index, 1);
}


// Recent means the execution receipt has reached the corporation—not that a
// client-side estimate expired. Open limit orders remain in the served book;
// only completed market trades and observed limit fills enter this history.
export function recordRecentMarketOrder(trade: TradeEvent): void {
  let row: RecentMarketOrder | null = null;
  if (trade.event === "Bought") {
    row = { side: "buy", commodity: trade.commodity, units: trade.units, unitPrice: trade.unit_price, limitFill: false, observedAt: liveSimTime() };
  } else if (trade.event === "Sold") {
    row = { side: "sell", commodity: trade.commodity, units: trade.units, unitPrice: trade.unit_price, limitFill: false, observedAt: liveSimTime() };
  } else if (trade.event === "LimitFilled") {
    row = { side: trade.side, commodity: trade.commodity, units: trade.units, unitPrice: trade.unit_price, limitFill: true, observedAt: liveSimTime() };
  }
  if (!row) return;
  recentMarketOrders.unshift(row);
  recentMarketOrders.length = Math.min(recentMarketOrders.length, RECENT_MARKET_ORDER_LIMIT);
}
 // ~1 minute at 1 Hz sampling
export function recordPriceHistory(st = state): void {
  if (!st.market) return;
  if (st.simTime - st.lastPriceSampleAt < 0.9) return; // throttle
  st.lastPriceSampleAt = st.simTime;
  for (const p of st.market.prices) {
    const series = (st.priceHistory[p.commodity] ??= []);
    series.push(p.price);
    if (series.length > PRICE_HISTORY_CAP) series.shift();
  }
}

/** Average unit price of walking `units` along the exchange curve. `depth` is the
 *  good's own book depth from the ticker (thin books for the rare goods); the bulk
 *  default covers a ticker row that predates the field. */
export function marketAverageQuote(mid: number, units: number, side: Side, depth: number = MARKET_DEPTH): number {
  const x = units / depth;
  if (side === "buy") {
    return mid * depth * Math.expm1(x) / units * (1 + MARKET_HALF_SPREAD);
  }
  return mid * depth * (1 - Math.exp(-x)) / units * (1 - MARKET_HALF_SPREAD);
}

export function freightDraftEntries(): { commodity: Commodity; units: number }[] {
  return COMMODITIES
    .filter((commodity) => (freightDraft.get(commodity) ?? 0) > 0)
    .map((commodity) => ({ commodity, units: freightDraft.get(commodity)! }));
}

/// Units of `c` in the player's Market Warehouse.
export function warehouseUnits(c: Commodity): number {
  const reported = state.wallet?.warehouse?.find((w) => w.commodity === c)?.units ?? 0;
  const reserved = marketReservations
    .filter((reservation) => reservation.commodity === c)
    .reduce((sum, reservation) => sum + reservation.units, 0);
  return Math.max(0, reported - reserved);
}

// Shared market derivations extracted from the desktop shell.

import type { Net } from "../../net";
import {
  cargoUnitsPerHull,
  type ShipKind,
  fleetCargoUnits,
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
  type TradeEvent,
} from "../../protocol";
import { label } from "../../icons";
import { liveSimTime, state } from "../../state";
import { COMMODITY_VALUE, constructionStock, dockedAtSystem, fleetCargoCapacity, fleetFuelCapacity, HULL_MASS } from "./fleet";
import { cargoMultiplier, hullUtilitySummary, isUtility, isBlueprintOnly, MODULES, moduleFitsHull, tankMultiplier, utilityProgramme } from "./equipment";

const EMPLACE_KITS: Record<string, [Commodity, number][]> = {
  deep_space_sensor: [["alloys", 60], ["electronics", 120], ["fuel", 40]],
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
};
export const FITTING_POINTS: Record<string, number> = {
  tiny_freighter: 2, small_freighter: 2, large_freighter: 2, heavy_freighter: 2, bulk_freighter: 2,
  corvette: 5, raider: 4, scout: 2, convoy: 2, colony: 2,
  destroyer: 8, cruiser: 12, battleship: 18, dreadnought: 28, titan: 45,
};
const fitCost = (mods: ModuleKind[]): number => mods.reduce((sum, module) => sum + (MODULE_FIT_COST[module] ?? 0), 0);

export const SHIP_YARD: Record<string, { yard: string; tier: number }> = {
  tiny_freighter: { yard: "shipyard", tier: 1 }, small_freighter: { yard: "shipyard", tier: 1 },
  large_freighter: { yard: "shipyard", tier: 3 }, heavy_freighter: { yard: "shipyard", tier: 4 }, bulk_freighter: { yard: "shipyard", tier: 5 },
  convoy: { yard: "shipyard", tier: 2 }, scout: { yard: "shipyard", tier: 1 }, colony: { yard: "shipyard", tier: 1 },
  raider: { yard: "shipyard", tier: 2 }, corvette: { yard: "shipyard", tier: 2 },
  destroyer: { yard: "naval_drydock", tier: 1 }, cruiser: { yard: "naval_drydock", tier: 2 }, battleship: { yard: "naval_drydock", tier: 3 },
  dreadnought: { yard: "capital_slipway", tier: 1 }, titan: { yard: "capital_slipway", tier: 2 }, transport: { yard: "garrison", tier: 1 },
};
const YARD_PREREQ: Record<string, { yard: string; tier: number }> = {
  naval_drydock: { yard: "shipyard", tier: 2 },
  capital_slipway: { yard: "naval_drydock", tier: 3 },
  ordnance_foundry: { yard: "shipyard", tier: 1 },
};
export const YARD_TITLE: Record<string, string> = {
  shipyard: "Shipyard", naval_drydock: "Naval Drydock", capital_slipway: "Capital Slipway",
  ordnance_foundry: "Ordnance Foundry", garrison: "Garrison",
};
export const slipsFor = (tier: number): number => Math.max(0, tier);
const HULL_PROGRAMME: Record<string, string> = {
  small_freighter: "prop_freight_frames", convoy: "prop_heavy_lifters", large_freighter: "prop_line_express_charters",
  heavy_freighter: "prop_line_bulk_charters", bulk_freighter: "prop_line_autonomous_freight",
  destroyer: "hull_line_iv_destroyer", cruiser: "hull_line_v_cruiser",
  battleship: "hull_line_vi_battleship", dreadnought: "hull_line_vii_dreadnought", titan: "hull_line_viii_titan",
};

export const POOL_OF: Record<string, "resource" | "industrial" | "infrastructure"> = {
  mining_complex: "resource", volatile_harvester: "resource", bioharvester: "resource",
  smelter: "industrial", electronics_fabricator: "industrial", chemical_works: "industrial",
  fuel_refinery: "industrial", machine_works: "industrial", armaments_complex: "industrial",
  composite_works: "industrial", hull_fabricator: "industrial", precision_works: "industrial", drive_works: "industrial",
  shipyard: "industrial", naval_drydock: "industrial", capital_slipway: "industrial", ordnance_foundry: "industrial",
  agroplex: "infrastructure", habitat: "infrastructure", orbital_warehouse: "infrastructure",
  warehouse: "infrastructure",
  sensor_array: "infrastructure", defense_platform: "infrastructure", academy: "infrastructure", garrison: "infrastructure",
};
const EXTRACTION_OF: Record<string, Commodity[]> = {
  mining_complex: ["metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "silicates", "rare_elements"],
  volatile_harvester: ["volatiles"],
  bioharvester: ["biomass"],
};
export type Pool = "resource" | "industrial" | "infrastructure";
export type PoolUse = Record<Pool, { used: number; total: number }>;
export const POOL_LABEL: Record<Pool, string> = { resource: "Resource", industrial: "Industrial", infrastructure: "Infrastructure" };

export type BuildOpt = BuildOption;
export const MINERAL_DEPOSITS: Commodity[] = ["metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "rare_elements", "silicates"];

/** Recipe is chosen from the served assignment, never a pending UI draft. */
export function assignedRecipe(option: BuildOpt | undefined, ore?: Commodity | null): ConversionRecipe | undefined {
  return option?.refining_recipes?.find(r => r.inputs[0]?.[0] === (ore ?? "metallic_ore")) ?? option?.conversion;
}

export function recipeOutputs(recipe: ConversionRecipe): [Commodity, number][] {
  return [[recipe.output, 1], ...(recipe.byproducts ?? [])];
}

export function commodityPurpose(commodity: Commodity, catalog: readonly BuildOpt[]): string {
  const ore = catalog.find(o => o.key === "smelter")?.refining_recipes?.find(r => r.inputs[0]?.[0] === commodity);
  if (ore) return `Sell raw or refine into ${recipeOutputs(ore).map(([c]) => label(c)).join(" + ")}.`;
  if (commodity === "conductive_metals") return "Conductors for Electronics production.";
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
  return `${inputs} → ${outputs} · base ${Number((recipe.rate * 60).toFixed(2))}/min at tier I, fully staffed.${option.refining_recipes?.length ? " Choose ore on the planet's Smelter." : ""}`;
}
export interface StructOpt {
  o: BuildOpt; pool: Pool; currentTier: number; targetTier: number;
  foundsNew: boolean; tierUp: boolean; afford: boolean; poolFull: boolean; noDeposit: boolean;
  yardPrereq: { yard: string; tier: number; have: number } | null;
  buildable: boolean; reason: string;
}
export interface ShipOpt {
  o: BuildOpt; needTier: number; yardTier: number; yardShort: boolean; afford: boolean;
  maxAff: number; buildable: boolean; reason: string; slipsFull: boolean; slips: number; foundingLocked: boolean;
  buildRate: number; yardBody?: BodyView;
}

export const buildOption = (key: string): BuildOpt | undefined => state.galaxy?.build_options.find((option) => option.key === key);

export const COMMODITIES: Commodity[] = [
  "metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "volatiles", "biomass",
  "rare_elements", "silicates", "conductive_metals", "titanium",
  "alloys", "electronics", "polymers", "fuel", "provisions", "machinery", "armaments",
  "composites", "hull_sections", "precision_components", "drive_assemblies",
];

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

export type MarketReservation = {
  kind: "market" | "limit"; side: Side; commodity: Commodity; orderUnits: number;
  units: number; credits: number; limitPrice?: number; issuedAt: number;
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
export function bindMarketDerive(source: () => Net | null, fitSource: () => ModuleKind[]): void {
  netSource = source;
  pendingFitSource = fitSource;
}


// Pretty "40 Alloys + 60 Electronics + 30 Fuel" for tooltips and refusals.
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
    && mods.every(m => MODULES.some(entry => entry.kind === m) && moduleFitsHull(m, kind) && (!isUtility(m) || mods.filter(other => other === m).length === 1));
}

export function moduleBuildReason(dyn: SystemStateView, module: ModuleKind): string {
  if (isBlueprintOnly(module) && !state.research?.blueprints?.includes(module)) return "Recover blueprint through exploration";
  const programme = utilityProgramme(module);
  if (programme && !state.research?.programmes.some(p => p.id === programme && p.state === "completed")) return "Research required";
  const yard = isUtility(module) ? "shipyard" : "armaments_complex";
  if (!(dyn.structures[yard] > 0)) return isUtility(module) ? "Needs Shipyard I" : "Needs Armaments Complex";
  if (isUtility(module) && !dyn.bodies.some(body => shipyardBoost(dyn, body) > 0)) return "Assign Shipyard workforce";
  // Module recipes draw on the local ledger, not docked commodity holds.
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
  return fleetCargoUnits(g) > refitCargoCapacity(g, hull, from, to, n);
}

/** The pooled manifest can allow removing one set of pods but not a whole
 * fitted stack. Clamp the selector to the actual arrived free hold space. */
export function maxCargoRefitCount(g: GhostView, hull: string, from: ModuleKind[], to: ModuleKind[], available: number): number {
  const loss = cargoUnitsPerHull(hull as ShipKind) * (cargoMultiplier(from) - cargoMultiplier(to));
  return loss > 0 ? Math.max(0, Math.min(available, Math.floor((fleetCargoCapacity(g) - fleetCargoUnits(g)) / loss))) : available;
}

export function refitPreview(g: GhostView, hull: import("../../protocol").ShipKind, from: ModuleKind[], to: ModuleKind[], n: number): string {
  const oldCap = fleetFuelCapacity(g);
  const cap = oldCap + HULL_MASS[hull] * .035 * n * (tankMultiplier(to) - tankMultiplier(from));
  const detail = hullUtilitySummary(hull, from, to);
  const fuel = Math.abs(cap - oldCap) > 1e-9 ? ` · Fleet tank ${oldCap.toFixed(1)} → ${cap.toFixed(1)} Fuel · aboard ${g.fuel?.toFixed(1) ?? "unknown"} unchanged` : "";
  const oldCargo = fleetCargoCapacity(g);
  const newCargo = refitCargoCapacity(g, hull, from, to, n);
  const cargo = oldCargo !== newCargo ? `Cargo ${oldCargo} → ${newCargo} · aboard ${fleetCargoUnits(g)} unchanged` : "";
  return `${detail}${fuel}${cargo ? `${detail || fuel ? " · " : ""}${cargo}` : ""}${detail || fuel || cargo ? " · " : ""}Refit ${3 * n}s after receipt${refitFuelBlocked(g, from, to) ? " · Use fuel before removing tanks" : ""}${refitCargoBlocked(g, hull, from, to, n) ? " · Unload cargo before removing pods" : ""}`;
}

export function utilityRefitChoices(g: GhostView, dock: SystemStateView) {
  if (!g.own || !dockedAtSystem(g, dock.id) || !dock.bodies.some(b => shipyardBoost(dock, b) > 0)) return [];
  const ledger = moduleLedgerAt(dock.id);
  const stacks = (g.composition ?? []).flatMap(stack => {
    const fitted = (g.loadouts ?? []).filter(s => s.kind === stack.kind && s.n > 0);
    const stock = stack.count - fitted.reduce((sum, s) => sum + s.n, 0);
    return [...fitted, ...(stock > 0 ? [{ kind: stack.kind, modules: [] as ModuleKind[], n: stock }] : [])];
  });
  return stacks.flatMap(stack => MODULES.filter(m => isUtility(m.kind)).flatMap(module => {
    const has = stack.modules.includes(module.kind);
    let to = has ? stack.modules.filter(m => m !== module.kind) : [...stack.modules, module.kind];
    if (!has && !fitLegal(stack.kind, to)) to = [...stack.modules.filter(m => !isUtility(m)), module.kind];
    if (!fitLegal(stack.kind, to) || (!has && !(ledger[module.kind] > 0))) return [];
    return [{ ship: stack.kind, from: stack.modules, to, module: module.kind,
      name: `${has ? "Remove" : "Fit"} ${module.name}`, blocked: refitFuelBlocked(g, stack.modules, to) || refitCargoBlocked(g, stack.kind, stack.modules, to, 1),
      preview: refitPreview(g, stack.kind, stack.modules, to, 1) }];
  }));
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

// The module ledger at a system (owner-only; {} if unseen).
export function moduleLedgerAt(sid: string): Record<string, number> {
  return state.systems.find((s) => s.id === sid)?.modules ?? {};
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
    if (t > 0 && POOL_OF[slug]) used[POOL_OF[slug]] += 1;
  }
  const pending = new Set((dyn?.builds ?? [])
    .filter((j) => j.body_id === b.id && POOL_OF[j.key] && ((b.structures ?? {})[j.key] ?? 0) === 0)
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
      if (t > 0 && POOL_OF[slug]) sum[POOL_OF[slug]].used += 1;
    }
  }
  return sum;
}

// Shared by catalogue visibility and build validation. Only a served completion
// unlocks a structure; queued/active research (or its estimated end) never does.
export function structureResearched(o: BuildOpt): boolean {
  return !o.research_prerequisite || state.research?.programmes.some(
    (entry) => entry.id === o.research_prerequisite && entry.state === "completed",
  ) === true;
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
  const have = constructionStock(dyn, o.key).available;
  const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
  const poolFull = foundsNew && !!pool && pools[pool].used >= pools[pool].total;
  const extractsFrom = EXTRACTION_OF[o.key];
  const noDeposit = foundsNew && !!extractsFrom && !(body.deposits ?? []).some((d) => extractsFrom.includes(d.resource as Commodity));
  // §yards: a yard needs the one below it standing on the same SYSTEM (not this
  // body — the ladder belongs to a shipbuilding world, so it may spread across
  // bodies). Checked on every tier, so a Drydock can't outgrow its Shipyard.
  const pre = YARD_PREREQ[o.key];
  const preHave = pre ? (dyn.structures?.[pre.yard] ?? 0) : 0;
  const yardPrereq = pre && preHave < pre.tier ? { ...pre, have: preHave } : null;
  const research = o.research_prerequisite;
  const programme = research ? state.research?.programmes.find((entry) => entry.id === research) : undefined;
  const researchLocked = !structureResearched(o);
  // The Welcome catalog names even still-locked research. Only the arrived
  // completion state unlocks construction; an active/queued estimate never does.
  const researchTitle = programme?.name
    ?? state.researchCatalog?.find((entry) => entry.id === research)?.name
    ?? label(research ?? "");
  const buildable = !researchLocked && !poolFull && !noDeposit && !yardPrereq && afford;
  const reason = researchLocked ? `Requires ${researchTitle} research.`
    : noDeposit ? "No matching deposit on this body — a mine only works its own rock."
    : yardPrereq ? `Needs ${YARD_TITLE[yardPrereq.yard] ?? yardPrereq.yard} tier ${yardPrereq.tier} somewhere in this system (have ${yardPrereq.have}).`
    : poolFull ? `This body's ${POOL_LABEL[pool]} slots are full (${pools[pool].used}/${pools[pool].total}).`
      : !afford ? "Not enough goods available at this system." : "";
  return { o, pool, currentTier, targetTier, foundsNew, tierUp: !foundsNew, afford, poolFull, noDeposit, yardPrereq, buildable, reason };
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
  const occupied = (dyn.builds ?? []).filter((j) => SHIP_YARD[j.key]?.yard === gate.yard).length;
  const slipsFull = !yardShort && occupied >= slips;
  // Match the server's actual construction site (highest gating-yard tier,
  // last body on ties), not whichever planet the workbench happens to show.
  const yardBody = dyn.bodies?.reduce<BodyView | undefined>((best, body) =>
    !best || (body.structures?.[gate.yard] ?? 0) >= (best.structures?.[gate.yard] ?? 0) ? body : best, undefined);
  const buildRate = yardBody ? shipyardBoost(dyn, yardBody, gate.yard) : 0;
  const buildable = !foundingLocked && !yardShort && !unresearched && !slipsFull && afford;
  const reason = foundingLocked
    ? "Complete the Founding Programme to unlock expansion."
    : unresearched
    ? `Requires ${researchTitle} research.`
    : yardShort ? `Needs ${YARD_TITLE[gate.yard] ?? gate.yard} tier ${needTier} (have ${yardTier}).`
    : slipsFull ? `All ${slips} slipway${slips === 1 ? "" : "s"} busy — raise the ${YARD_TITLE[gate.yard] ?? gate.yard} or wait for a hull to launch.`
    : !afford ? "Not enough goods available at this system."
    : buildRate <= 0 ? `Queued hulls wait for workforce at ${YARD_TITLE[gate.yard] ?? gate.yard}${yardBody ? ` · ${yardBody.name}` : ""}.` : "";
  return { o, needTier, yardTier, yardShort: yardShort || unresearched, afford, maxAff, buildable, reason, slipsFull, slips, foundingLocked, buildRate, yardBody };
}

/// Mirrors sim::production::shipyard_work_rate. These are served factors, used
/// only for a prospective quote; in-flight progress uses its own arrived segment.
export function shipyardBoost(dyn: SystemStateView, body: BodyView, yard = "shipyard"): number {
  const a = (dyn.assignments ?? []).find((x) => x.body_id === body.id && x.structure === yard);
  return (body.structures?.[yard] ?? 0) > 0 && a && a.staffing > 0 ? 1 + 0.25 * a.staffing * a.skill : 0;
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
    net.send({ type: "BuildShip", system_id: sid, ship_kind: k as import("../../protocol").ShipKind, loadout: fit.length ? fit : undefined });
  }
  // §modules Part B3: "module:<slug>" → manufacture into the system ledger.
  else if (k.startsWith("module:")) net.send({ type: "BuildModule", system_id: sid, module: k.slice(7) as ModuleKind });
  // §bodies: the body panel names its body; omitted → the sim auto-sites.
  else net.send({ type: "DevelopSystem", system_id: sid, upgrade: k, body_id: bodyId }); // §economy: any structure slug
}


// What "Ship production → hub" will ACTUALLY dispatch: the system's NON-FUEL
// stock in whole units. MIRRORS the sim's apply_ship_production rule — Fuel is
// retained as the system's operating reserve (it powers movement; sell it via
// the Market), so it must neither light the button nor be promised in feedback.
// The View's stockpile is already owner-only whole units, so this is exact.
export function shippableStock(dyn: SystemStateView | undefined): StockSlot[] {
  return (dyn?.stockpile ?? []).filter((s) => s.commodity !== "fuel" && s.units >= 1);
}

export function pruneMarketReservations(): void {
  const ttl = Math.max(5, (state.market?.staleness ?? 0) + 2);
  const now = liveSimTime();
  for (let i = marketReservations.length - 1; i >= 0; i--) {
    if (now - marketReservations[i].issuedAt > ttl) marketReservations.splice(i, 1);
  }
}

export function reservedMarketCredits(): number {
  pruneMarketReservations();
  return marketReservations.reduce((sum, reservation) => sum + reservation.credits, 0);
}

export function spendableMarketCredits(): number {
  return Math.max(0, (state.wallet?.credits ?? 0) - reservedMarketCredits());
}

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

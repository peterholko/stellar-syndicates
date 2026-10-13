// Shared fleet derivations extracted from the desktop shell.

import {
  cargoUnitsPerHull,
  isPlayerFreighter,
  fleetCargoManifest,
  fleetCargoUnits,
  type BattleRecordView,
  type BattleReportView,
  type Commodity,
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
import { cargoMultiplier, sensorMultiplier, tankMultiplier } from "./equipment";

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
  composites: 50, hull_sections: 128, precision_components: 92, warp_core: 215,
  uranium: 5.6, antimatter: 120, fusion_core: 300,
};

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
  mining_belt_station: 12000,
  destroyer: 2000, cruiser: 4000, battleship: 8000, dreadnought: 16000, titan: 32000,
  transport: 7000, freighter: 6000, builder: 2500,
};
// Mirrors ShipKind::max_speed. Command previews use the slowest hull in the
// served composition and label the result as an estimate; the sim remains the
// authority for drive transitions, wells, fuel, and future route changes.
export const HULL_BASE_SPEED: Record<ShipKind, number> = {
  tiny_freighter: 40, small_freighter: 40, large_freighter: 38, heavy_freighter: 34, bulk_freighter: 30,
  convoy: 40, builder: 35, raider: 100, corvette: 65, colony: 33, scout: 115, survey_probe: 120, mining_barge: 30,
  mining_belt_station: 15,
  destroyer: 55, cruiser: 45, battleship: 36, dreadnought: 29, titan: 23,
  transport: 30, freighter: 32,
};
const CARGO_MASS_PER_UNIT = 28;
export const CARGO_UNITS_PER_FREIGHTER = 400; // legacy Medium hull; use cargoUnitsPerHull for a family member

export const fleetHullMass = (g: GhostView): number => g.composition
  ? g.composition.reduce((mass, ship) => mass + HULL_MASS[ship.kind] * ship.count, 0)
  : HULL_MASS[g.kind];
export const shipMass = (g: GhostView): number =>
  fleetHullMass(g) + (g.own ? fleetCargoUnits(g) * CARGO_MASS_PER_UNIT : 0);
export const fleetBaseSpeed = (g: GhostView): number => Math.min(...(g.composition?.length
  ? g.composition : [{ kind: g.kind, count: 1 }]).filter(s => s.count > 0).map(stack => {
    // Drive Tuning speeds every hull; Military Drives and Torch Regime warships.
    const r = driveResearch(g);
    const base = HULL_BASE_SPEED[stack.kind] * r.speed * (WARSHIPS.includes(stack.kind) ? r.warshipSpeed : 1);
    const fits = (g.loadouts ?? []).filter(f => f.kind === stack.kind && f.n > 0);
    const unfitted = fits.reduce((n, f) => n + f.n, 0) < stack.count ? base : Infinity;
    return Math.min(unfitted, ...fits.map(f => base * (f.modules.includes("survey_drive") ? 1.25 : 1)));
  }));
export const fleetFuelCapacity = (g: GhostView): number => g.fuel_capacity ?? (fleetHullMass(g)
  + (g.loadouts ?? []).reduce((extra, s) => extra + HULL_MASS[s.kind] * s.n * (tankMultiplier(s.modules) - 1), 0)) * FUEL_PER_HULL_MASS;

// §fuel-overhaul: mirrors ship.rs. Slow flies half speed on the fission store
// (Uranium) at a quarter of the energy per distance; every formation without
// a ship of the line can fly it. Savings are per distance, in addition to the
// throttle; do not multiply trip fuel by speed a second time.
const SHIPS_OF_THE_LINE: ShipKind[] = ["battleship", "dreadnought", "titan"];
const WARSHIPS: ShipKind[] = ["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
const stacksOf = (g: GhostView) => g.composition?.length ? g.composition : [{ kind: g.kind, count: 1 }];
const researched = (id: string): boolean => !!state.research?.programmes.some(p => p.id === id && p.state === "completed");
/** The owner's research on its drives (ship.rs FuelResearch). Own fleets
 * only: a rival's research is unknown, so its figures stay stock. */
function driveResearch(g: GhostView) {
  if (!g.own) return { freight: 1, torch: 1, consumption: 1, speed: 1, warshipSpeed: 1 };
  const torch = researched("prop_expedition_iii_torch_regime");
  return {
    freight: researched("prop_line_fusion_freight") ? FUSION_FREIGHT_MULT : 1,
    torch: torch ? 1.4 : 1,
    consumption: researched("prop_efficient_burns") ? 0.75 : 1,
    speed: researched("prop_drive_tuning") ? 1.1 : 1,
    warshipSpeed: (researched("prop_military_drives") ? 1.2 : 1) * (torch ? 1.3 : 1),
  };
}
export const slowCapable = (g: GhostView): boolean => !g.tca
  && stacksOf(g).some(s => s.count > 0) && stacksOf(g).every(s => s.count === 0 || !SHIPS_OF_THE_LINE.includes(s.kind));
export const onUranium = (g: GhostView): boolean => g.transit === "slow" && slowCapable(g);
export const fusionFreightResearched = (): boolean => researched("prop_line_fusion_freight");
export const transitLabel = (mode: TransitMode = "full"): string =>
  mode === "slow" ? "Slow" : mode === "stealth" ? "Stealth" : "Full speed";
/** The fuel a regime burns: Uranium on Slow, Deuterium otherwise. */
export const regimeFuel = (g: GhostView): Commodity => onUranium(g) ? "uranium" : "deuterium";
/** The multiplier on energy per distance on one store, weighed hull by hull
 * by mass (ship.rs cruise_mult_for): a quarter on Uranium; Fusion Freight's cut
 * for player freighters and their cargo; Torch Regime on warships. */
export function cruiseMultFor(g: GhostView, uranium: boolean): number {
  const r = driveResearch(g);
  const regime = (freighter: boolean) => uranium ? SLOW_FUEL_MULT : freighter ? r.freight : 1;
  const stacks = stacksOf(g).filter(s => s.count > 0);
  const cargo = g.own ? fleetCargoUnits(g) * CARGO_MASS_PER_UNIT : 0;
  const total = fleetHullMass(g) + cargo;
  if (total <= 1e-9) return regime(false);
  const hulls = stacks.reduce((sum, s) => sum + HULL_MASS[s.kind] * s.count
    * regime(isPlayerFreighter(s.kind)) * (WARSHIPS.includes(s.kind) ? r.torch : 1), 0);
  return (hulls + cargo * regime(stacks.some(s => isPlayerFreighter(s.kind)))) / total;
}
export const cruiseFuelMultiplier = (g: GhostView): number => cruiseMultFor(g, onUranium(g));
/** Efficient Burns' cut on every burn, jumps included. */
export const fuelConsumption = (g: GhostView): number => driveResearch(g).consumption;
export const transitSpeedMultiplier = (g: GhostView): number =>
  onUranium(g) ? SLOW_SPEED_FRACTION : g.transit === "stealth" ? 0.5 : 1;
/** §fuel-overhaul: the fission store's size (served, else every hull bar the ships of the line). */
export const fleetUraniumCapacity = (g: GhostView): number => g.uranium_capacity ?? (stacksOf(g)
  .filter(s => !SHIPS_OF_THE_LINE.includes(s.kind)).reduce((m, s) => m + HULL_MASS[s.kind] * s.count, 0)
  + (g.loadouts ?? []).filter(s => !SHIPS_OF_THE_LINE.includes(s.kind))
    .reduce((extra, s) => extra + HULL_MASS[s.kind] * s.n * (tankMultiplier(s.modules) - 1), 0)) * FUEL_PER_HULL_MASS;
/** The store a fleet's regime burns, as served. */
export const regimeStore = (g: GhostView): { fuel: Commodity; have: number; capacity: number } => onUranium(g)
  ? { fuel: "uranium", have: g.uranium ?? 0, capacity: fleetUraniumCapacity(g) }
  : { fuel: "deuterium", have: g.fuel ?? 0, capacity: fleetFuelCapacity(g) };
/** Antimatter one jump of `distance` burns from the bottle (mass × distance × 1e-7). */
export const jumpAntimatter = (g: GhostView, distance: number): number =>
  FUEL_PER_MASS_DISTANCE * distance * shipMass(g) / JUMP_FUEL_FACTOR * fuelConsumption(g);
/** §fuel-overhaul: the fusion tank as SPACE, not energy: Deuterium units plus
 * Helium-3 units (each Helium-3 holds six Deuterium of energy). */
export function fusionTank(g: GhostView): { deuterium: number; helium3: number; used: number; capacity: number } {
  const helium3 = g.helium3 ?? 0;
  const deuterium = Math.max(0, (g.fuel ?? 0) - HELIUM3_FUEL_EQUIV * helium3);
  return { deuterium, helium3, used: deuterium + helium3, capacity: fleetFuelCapacity(g) };
}

const SHIP_KIND_LABEL: Record<ShipKind, string> = {
  tiny_freighter: "Tiny Freighter", small_freighter: "Small Freighter", large_freighter: "Large Freighter", heavy_freighter: "Heavy Freighter", bulk_freighter: "Bulk Freighter",
  convoy: "Medium Freighter", raider: "Interceptor", corvette: "Corvette", colony: "Colony Ship", scout: "Scout", survey_probe: "Survey Probe", mining_barge: "Mining Barge",
  destroyer: "Destroyer", cruiser: "Cruiser", battleship: "Battleship", dreadnought: "Dreadnought", titan: "Titan",
  transport: "Troop Transport", builder: "Construction Ship", freighter: "Authority Freighter",
  mining_belt_station: "Mining Belt Station",
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

export function fleetCargoCapacity(g: GhostView): number {
  const capacity = g.composition?.length
    ? g.composition.reduce((units, ship) => units + cargoUnitsPerHull(ship.kind) * ship.count, 0)
    : cargoUnitsPerHull(g.kind);
  // Capacity follows only the arrived fit, never a refit preview or its ETA.
  const extra = (g.loadouts ?? []).reduce((sum, s) => sum
    + cargoUnitsPerHull(s.kind) * s.n * (cargoMultiplier(s.modules) - 1), 0);
  return capacity + extra;
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
  const store = regimeStore(g);
  const room = Math.max(0, store.capacity - store.have);
  const destination = g.path?.at(-1)?.pos;
  const packageFuel = destination
    ? Math.min(room, estimatedFuelForLeg(g, destination) + store.capacity * AAA_RESERVE_FRAC)
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

/** §fuel-overhaul: every hull either has built-in drives (Interceptor, Scout)
 * or a fitted Jump Drive; mirrors ship.rs can_jump. */
export function jumpCapable(g: GhostView): boolean {
  const composition = g.composition ?? [];
  return g.own && composition.length > 0
    && composition.every((c) => c.count === 0 || c.kind === "raider" || c.kind === "scout"
      || (g.loadouts ?? []).filter(s => s.kind === c.kind && s.modules.includes("jump_drive")).reduce((n, s) => n + s.n, 0) >= c.count);
}


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
    return "Colonists + infrastructure. Send it to an unclaimed system: on arrival the system becomes yours and the ship is consumed (it becomes the colony). It broadcasts its voyage — slow, visible, raidable — so escort it. If someone claims the target first, it holds there intact; redirect it.";
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

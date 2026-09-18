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
  type Vec2,
} from "../../protocol";
import type { Net } from "../../net";
import { state } from "../../state";
import { recordForBattleReport, reportForBattleRecord } from "../../battlehistory";
import { HYPERLIMIT_SU } from "./geo";
import { cargoMultiplier, sensorMultiplier, tankMultiplier } from "./equipment";

// Mirror of the sim's commodity value-rank (also in render.ts) — for flavor
// text and the observed Fuel fallback. Client-only; no server data.
export const COMMODITY_VALUE: Record<Commodity, number> = {
  biomass: 5, silicates: 9, metallic_ore: 8, volatiles: 9, rare_elements: 100,
  cuprite_ore: 24, titanium_ore: 110, crystalline_ore: 6, rare_metal_ore: 1700,
  conductive_metals: 30, titanium: 60,
  provisions: 9, fuel: 14, polymers: 16, alloys: 26, electronics: 34,
  machinery: 62, armaments: 56,
  composites: 50, hull_sections: 128, precision_components: 92, drive_assemblies: 215,
};

// Mirrors the sim's fuel-cost model (crates/sim/src/fuel.rs + ship.rs).
export const FUEL_PER_MASS_DISTANCE = 1.0e-6;
const FUEL_PER_HULL_MASS = 0.035;
export const WARP_FACTOR = 5;
export const AAA_FUEL_PRICE_MULT = 3;
export const AAA_SERVICE_FEE = 1_000;
const AAA_RESERVE_FRAC = 0.10;
export const HULL_MASS: Record<ShipKind, number> = {
  tiny_freighter: 1500, small_freighter: 2500, large_freighter: 8000, heavy_freighter: 14000, bulk_freighter: 24000,
  convoy: 4500, raider: 200, corvette: 800, colony: 6000, scout: 80,
  destroyer: 2000, cruiser: 4000, battleship: 8000, dreadnought: 16000, titan: 32000,
  transport: 7000, freighter: 6000, builder: 2500,
};
// Mirrors ShipKind::max_speed. Command previews use the slowest hull in the
// served composition and label the result as an estimate; the sim remains the
// authority for drive transitions, wells, fuel, and future route changes.
export const HULL_BASE_SPEED: Record<ShipKind, number> = {
  tiny_freighter: 40, small_freighter: 40, large_freighter: 38, heavy_freighter: 34, bulk_freighter: 30,
  convoy: 40, builder: 35, raider: 100, corvette: 65, colony: 33, scout: 115,
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
    const fits = (g.loadouts ?? []).filter(f => f.kind === stack.kind && f.n > 0);
    const unfitted = fits.reduce((n, f) => n + f.n, 0) < stack.count ? HULL_BASE_SPEED[stack.kind] : Infinity;
    return Math.min(unfitted, ...fits.map(f => HULL_BASE_SPEED[stack.kind] * (f.modules.includes("survey_drive") ? 1.25 : 1)));
  }));
export const fleetFuelCapacity = (g: GhostView): number => g.fuel_capacity ?? (fleetHullMass(g)
  + (g.loadouts ?? []).reduce((extra, s) => extra + HULL_MASS[s.kind] * s.n * (tankMultiplier(s.modules) - 1), 0)) * FUEL_PER_HULL_MASS;

const SHIP_KIND_LABEL: Record<ShipKind, string> = {
  tiny_freighter: "Tiny Freighter", small_freighter: "Small Freighter", large_freighter: "Large Freighter", heavy_freighter: "Heavy Freighter", bulk_freighter: "Bulk Freighter",
  convoy: "Medium Freighter", raider: "Interceptor", corvette: "Corvette", colony: "Colony Ship", scout: "Scout",
  destroyer: "Destroyer", cruiser: "Cruiser", battleship: "Battleship", dreadnought: "Dreadnought", titan: "Titan",
  transport: "Troop Transport", builder: "Construction Ship", freighter: "Authority Freighter",
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
  return (warp + wellSurcharge) * logisticsMult;
}


export function aaaEstimate(g: GhostView): { fuel: number; cost: number } {
  const capacity = fleetFuelCapacity(g);
  const room = Math.max(0, capacity - (g.fuel ?? 0));
  const destination = g.path?.at(-1)?.pos;
  const packageFuel = destination
    ? Math.min(room, estimatedFuelForLeg(g, destination) + capacity * AAA_RESERVE_FRAC)
    : room;
  const marketFuel = state.market?.prices.find((price) => price.commodity === "fuel")?.price ?? COMMODITY_VALUE.fuel;
  return { fuel: packageFuel, cost: AAA_SERVICE_FEE + packageFuel * marketFuel * AAA_FUEL_PRICE_MULT };
}


export function jumpCapable(g: GhostView): boolean {
  const composition = g.composition ?? [];
  return g.own && composition.length > 0
    && composition.every((c) => c.kind === "raider" || c.kind === "scout");
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
  return isPlayerFreighter(g.kind) || !!g.composition?.some((c) => isPlayerFreighter(c.kind) && c.count > 0);
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

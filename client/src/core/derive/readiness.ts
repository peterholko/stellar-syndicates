import { fleetCargoUnits, type GhostView, type Vec2 } from "../../protocol";
import type { PendingIntent, ViewState } from "../../state";
import { aaaEstimate, estimatedFuelForLeg, fleetCargoCapacity, jumpAntimatter, regimeStore, shipKindLabel } from "./fleet";

/** Advisory tunable, not a combat prohibition. Retreating damaged ships must
 * remain commandable. Everything here reads received telemetry, never truth. */
export const BADLY_DAMAGED_HULL = 0.50;
/** The two warp stores by name (kept local: this module has no icon deps). */
const storeName = (fuel: "uranium" | "deuterium" | string): string => fuel === "uranium" ? "Uranium" : "Deuterium";

export function fleetReadiness(g: GhostView) {
  const hull = g.damage == null ? null : Math.max(0, Math.min(1, 1 - g.damage));
  const cargoCapacity = fleetCargoCapacity(g);
  const cargoUsed = fleetCargoUnits(g);
  // §fuel-overhaul: the store the fleet's throttle burns (Uranium on Slow).
  const store = regimeStore(g);
  const served = store.fuel === "uranium" ? g.uranium : g.fuel;
  return { hull, fuel: served ?? null, fuelCapacity: store.capacity, fuelKind: store.fuel,
    antimatter: g.antimatter ?? null, antimatterCapacity: g.antimatter_capacity ?? 0, cargoUsed,
    cargoCapacity, cargoFree: Math.max(0, cargoCapacity - cargoUsed), captain: g.captain ?? null };
}

export function dispatchWarnings(g: GhostView, destination?: Vec2, jump = false): string[] {
  if (!g.own) return []; // never invent rival fuel, cargo or officer knowledge
  const r = fleetReadiness(g);
  const warnings: string[] = [];
  if (r.hull !== null && r.hull < BADLY_DAMAGED_HULL) {
    warnings.push(`Badly damaged · ${Math.round(r.hull * 100)}% hull. Repair before combat.`);
  }
  if (g.supplied === false) warnings.push("Out of Provisions · movement may be blocked.");
  // §fuel-overhaul: a jump burns Antimatter from the bottle, never the warp
  // stores; a short bottle cancels the jump after the spool.
  if (jump) {
    if (destination && r.antimatter !== null) {
      const needed = jumpAntimatter(g, Math.hypot(destination.x - g.pos.x, destination.y - g.pos.y));
      if (needed > r.antimatter + 1e-6) warnings.push(
        `Antimatter short · ~${needed.toFixed(1)} needed / ${r.antimatter.toFixed(1)} aboard. The jump cancels; refill at a dock or from a tender.`);
    }
    return warnings;
  }
  const store = storeName(r.fuelKind);
  // A dry Uranium store falls back to the fusion tank (full speed, full burn).
  const fallback = r.fuelKind === "uranium" && (g.fuel ?? 0) > 0 ? "Switches to Deuterium." : "May stop en route.";
  if (destination) {
    const needed = estimatedFuelForLeg(g, destination);
    if (r.fuel === null) warnings.push("Fuel report unavailable · range unverified.");
    else if (needed > r.fuel + 1e-6) warnings.push(
      `${store} short · ~${Math.ceil(needed)} needed / ${Math.floor(r.fuel)} reported. ${fallback}`);
  } else if (r.fuel !== null && r.fuel <= 0) warnings.push(`${store} empty.`);
  return warnings;
}

export function intentReadinessWarnings(st: Pick<ViewState, "ghosts" | "galaxy" | "emplacements">,
  intent: PendingIntent): string[] {
  if (intent.verb === "command") return (intent.commands ?? []).flatMap(command => {
    const id = "fleet_id" in command ? command.fleet_id : "ship_id" in command ? command.ship_id : intent.shipId;
    const fleet = st.ghosts.find(g => g.own && g.id === id);
    if (command.type === "SetEngageFreight" && command.on) return ["Attacking Authority freighters risks citations and higher market costs."];
    if (command.type === "SetRunBlockades" && command.on) return ["A held blockade's ships can catch this fleet and raid its cargo."];
    if (!fleet) return [];
    if (command.type === "ExploreSite") return dispatchWarnings(fleet, intent.dest);
    if (command.type === "RefuelFleet") {
      const target = st.ghosts.find(g => g.id === command.target_id);
      return [...dispatchWarnings(fleet, target?.pos), "Target must hold for refueling. Tender stops near combat."];
    }
    if (command.type === "RequestFuelRescue") return [`Estimated charge ~${Math.ceil(aaaEstimate(fleet).cost)} Cr · includes 3× market ${storeName(regimeStore(fleet).fuel)} and a non-refundable callout.`];
    if (command.type === "HaulToMarketHub" || command.type === "HaulToSystem" || command.type === "MoveShip" || command.type === "DefendSystem") {
      const destination = command.type === "HaulToMarketHub" ? st.galaxy?.hub
        : command.type === "MoveShip" ? command.dest : st.galaxy?.systems.find(s => s.id === (command.type === "DefendSystem" ? command.system_id : command.system))?.pos;
      return dispatchWarnings(fleet, destination);
    }
    return [];
  });
  const destination = intent.dest ?? st.ghosts.find(g => g.id === intent.targetId)?.pos
    ?? st.galaxy?.systems.find(s => s.id === intent.targetId)?.pos
    ?? st.emplacements.find(e => e.id === intent.targetId)?.pos;
  const ids = intent.shipIds?.length ? intent.shipIds : [intent.shipId];
  return ids.flatMap(id => {
    const fleet = st.ghosts.find(g => g.own && g.id === id);
    if (!fleet) return [];
    return dispatchWarnings(fleet, destination, intent.verb === "jump")
      .map(w => ids.length > 1 ? `${shipKindLabel(fleet.kind)} ${id}: ${w}` : w);
  });
}

import { fleetCargoUnits, type GhostView, type Vec2 } from "../../protocol";
import type { PendingIntent, ViewState } from "../../state";
import { aaaEstimate, estimatedFuelForLeg, fleetCargoCapacity, fleetFuelCapacity, shipKindLabel } from "./fleet";

/** Advisory tunable, not a combat prohibition. Retreating damaged ships must
 * remain commandable. Everything here reads received telemetry, never truth. */
export const BADLY_DAMAGED_HULL = 0.50;

export function fleetReadiness(g: GhostView) {
  const hull = g.damage == null ? null : Math.max(0, Math.min(1, 1 - g.damage));
  const cargoCapacity = fleetCargoCapacity(g);
  const cargoUsed = fleetCargoUnits(g);
  return { hull, fuel: g.fuel ?? null, fuelCapacity: fleetFuelCapacity(g), cargoUsed,
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
  // Jump fuel is explicitly unlimited in the current playtest. Do not price an
  // instantaneous jump as a warp cruise and warn about a fictional fuel bill.
  if (!jump && destination) {
    const needed = estimatedFuelForLeg(g, destination);
    if (r.fuel === null) warnings.push("Fuel report unavailable · range unverified.");
    else if (needed > r.fuel + 1e-6) warnings.push(
      `Fuel short · ~${Math.ceil(needed)} needed / ${Math.floor(r.fuel)} reported. May stop en route.`);
  } else if (!jump && r.fuel !== null && r.fuel <= 0) warnings.push("Empty fuel tank.");
  return warnings;
}

export function intentReadinessWarnings(st: Pick<ViewState, "ghosts" | "galaxy" | "emplacements">,
  intent: PendingIntent): string[] {
  if (intent.verb === "command") return (intent.commands ?? []).flatMap(command => {
    const id = "fleet_id" in command ? command.fleet_id : "ship_id" in command ? command.ship_id : intent.shipId;
    const fleet = st.ghosts.find(g => g.own && g.id === id);
    if (command.type === "SetEngageFreight" && command.on) return ["Attacking Authority freighters risks citations and higher market costs."];
    if (!fleet) return [];
    if (command.type === "RequestFuelRescue") return [`Estimated charge ~${Math.ceil(aaaEstimate(fleet).cost)} Cr · includes 3× market Fuel and a non-refundable callout.`];
    if (command.type === "HaulToMarketHub" || command.type === "HaulToSystem" || command.type === "MoveShip") {
      const destination = command.type === "HaulToMarketHub" ? st.galaxy?.hub
        : command.type === "MoveShip" ? command.dest : st.galaxy?.systems.find(s => s.id === command.system)?.pos;
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

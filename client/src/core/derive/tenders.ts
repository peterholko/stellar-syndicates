import { fleetCargoManifest, isPlayerFreighter, type GhostView } from "../../protocol";
import { fleetFuelCapacity } from "./fleet";

/** Fitting, demand and progress are received facts; no prediction can transfer
 * cargo, fill a tank or complete the service on screen before its light. */
export const tenderCapable = (g: GhostView): boolean => g.own && !g.tca
  && !!g.loadouts?.some(s => isPlayerFreighter(s.kind) && s.n > 0 && s.modules.includes("fuel_transfer_rig"));
export const tenderFuel = (g: GhostView): number =>
  fleetCargoManifest(g).filter(c => c.commodity === "fuel").reduce((n, c) => n + c.units, 0);
export const tenderDemand = (g: GhostView): number | null => g.fuel == null ? null
  : Math.ceil(Math.max(0, fleetFuelCapacity(g) - g.fuel));
export function tenderStatus(g: GhostView): string {
  switch (g.fuel_transfer?.phase) {
    case "rendezvous": return "Rendezvous";
    case "waiting": return "Waiting for target to hold";
    case "transferring": return "Refueling";
    case "complete": return "Refueling complete";
    case "empty": return "Fuel cargo exhausted";
    case "unsafe": return "Stopped · combat nearby";
    case "unavailable": return "Stopped · target or rig unavailable";
    default: return "";
  }
}

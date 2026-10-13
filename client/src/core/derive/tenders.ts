import { fleetCargoManifest, isPlayerFreighter, type GhostView } from "../../protocol";
import { fleetUraniumCapacity, fusionTank } from "./fleet";

/** Fitting, demand and progress are received facts; no prediction can transfer
 * cargo, fill a tank or complete the service on screen before its light. */
export const tenderCapable = (g: GhostView): boolean => g.own && !g.tca
  && !!g.loadouts?.some(s => isPlayerFreighter(s.kind) && s.n > 0 && s.modules.includes("fuel_transfer_rig"));
/** §fuel-overhaul: a tender pumps any fuel it carries into the matching
 * store: Helium-3 and Deuterium into the fusion tank, Uranium into the fission
 * store, Antimatter into the bottle (the one refill away from a dock). */
const TENDER_FUELS = ["helium3", "deuterium", "uranium", "antimatter"];
export const tenderFuel = (g: GhostView): number =>
  fleetCargoManifest(g).filter(c => TENDER_FUELS.includes(c.commodity)).reduce((n, c) => n + c.units, 0);
export const tenderDemand = (g: GhostView): number | null => g.fuel == null ? null
  : Math.ceil(Math.max(0, fusionTank(g).capacity - fusionTank(g).used)
    + (g.uranium == null ? 0 : Math.max(0, fleetUraniumCapacity(g) - g.uranium))
    + (g.antimatter == null ? 0 : Math.max(0, (g.antimatter_capacity ?? 0) - g.antimatter)));
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

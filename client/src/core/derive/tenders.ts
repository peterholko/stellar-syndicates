import { fleetCargoManifest, isPlayerFreighter, type GhostView } from "../../protocol";
import { magazines, tanks } from "./fleet";

/** Fitting, demand and progress are received facts; no prediction can transfer
 * cargo, fill a tank or complete the service on screen before its light. */
export const tenderCapable = (g: GhostView): boolean => g.own && !g.tca
  && !!g.loadouts?.some(s => isPlayerFreighter(s.kind) && s.n > 0 && s.modules.includes("fuel_transfer_rig"));
/** §fuel-overhaul: a tender pumps any fuel it carries into the matching
 * store: Helium-3 and Deuterium into the fusion tank, Uranium into the fission
 * store, Antimatter into the bottle (the one refill away from a dock).
 * §shipbuilding-overhaul Phase 5: and munitions into the magazines. */
const TENDER_FUELS = ["helium3", "deuterium", "uranium", "antimatter", "torpedoes", "slugs", "beam_crystals"];
export const tenderFuel = (g: GhostView): number =>
  fleetCargoManifest(g).filter(c => TENDER_FUELS.includes(c.commodity)).reduce((n, c) => n + c.units, 0);
export const tenderDemand = (g: GhostView): number | null => g.fuel == null ? null
  // §shipbuilding-overhaul: the room in every tank and magazine aboard.
  : Math.ceil(tanks(g).reduce((room, t) => room + Math.max(0, t.capacity - t.have), 0)
    + magazines(g).reduce((room, m) => room + Math.max(0, m.capacity - m.have), 0));
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

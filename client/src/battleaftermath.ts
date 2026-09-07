import type { BattleSurvivor, GhostView, SystemInfo } from "./protocol";
import type { ViewState } from "./state";

type AftermathState = Pick<ViewState, "playerId" | "galaxy" | "systems" | "ghosts" | "battles" | "pendingOrders">;

/** Historical hull/progression stays in the report. Only action availability
 * reads the current SERVED picture; losing a fleet later must not rewrite it. */
export function currentSurvivor(st: AftermathState, survivor: BattleSurvivor): GhostView | undefined {
  return st.ghosts.find((g) => g.own && g.id === survivor.fleet_id);
}

export function survivorOrderBlock(st: AftermathState, fleet: GhostView | undefined): string {
  if (!fleet) return "Fleet is no longer in your current reports.";
  if (st.battles.some((b) => b.participants.includes(fleet.id))) return "Fleet is in a reported battle.";
  if ((st.pendingOrders.get(fleet.id) ?? []).some((o) => !o.lost)) return "An order is already awaiting a response.";
  return "";
}

/** A normal berth/Shipyard is NOT a repair yard. Mirror the sim's staffed
 * Ordnance Foundry requirement using arrived owned-system reports only.
 * Supplies can change before arrival; this is eligibility, never a guarantee. */
export function nearestRepairYard(st: AftermathState, fleet: GhostView): SystemInfo | undefined {
  const candidates = st.galaxy?.systems.filter((system) => {
    const report = st.systems.find((s) => s.id === system.id && s.owner === st.playerId);
    if (!report || !(report.structures.ordnance_foundry > 0)) return false;
    // repair_docked_fleets uses site_for(OrdnanceFoundry), the primary body.
    const primary = report.bodies.find((b) => b.parent === null) ?? report.bodies[0];
    const line = report.assignments.find((a) => a.structure === "ordnance_foundry" && a.body_id === primary?.id);
    return !!line && line.staffing > 0 && line.skill > 0
      && ["alloys", "machinery"].every((kind) => report.stockpile?.some((s) => s.commodity === kind && s.units > 0));
  }) ?? [];
  return candidates.sort((a, b) => Math.hypot(a.pos.x - fleet.pos.x, a.pos.y - fleet.pos.y)
    - Math.hypot(b.pos.x - fleet.pos.x, b.pos.y - fleet.pos.y) || a.id.localeCompare(b.id))[0];
}

export function survivingGuardTarget(st: AftermathState, survivor: BattleSurvivor): GhostView | undefined {
  return st.ghosts.find((g) => g.own && g.id === survivor.guard_target && g.id !== survivor.fleet_id);
}

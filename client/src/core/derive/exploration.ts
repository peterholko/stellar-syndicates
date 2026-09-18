import type { ExpeditionTask, ExplorationSiteView, GhostView } from "../../protocol";
import { isPlayerFreighter } from "../../protocol";
import type { ViewState } from "../../state";
import { fleetCargoCapacity } from "./fleet";

export const EXPLORATION_MARKER_PX = 48;
export function siteTitle(site: ExplorationSiteView): string { return site.details?.name ?? `Unknown contact ${site.id.replace(/^E/, "")}`; }
export function siteArt(site: ExplorationSiteView): string | null {
  return site.details ? `/art/exploration/${site.details.kind}.png` : null;
}
export function siteStatus(site: ExplorationSiteView): string {
  if (!site.details) return "Unidentified · Scout needed";
  if (site.details.guarded) return "Guarded · escorts recommended";
  if (site.details.lead) return "New coordinates recovered";
  if (site.details.opportunity && (!site.details.studied || site.details.opportunity.task === "extract"
      && Object.values(site.details.opportunity.cargo).some(n => n! > 0))) return "Deeper opportunity";
  if (site.details.restored_by) return "Restored sensor outpost";
  if (Object.values(site.details.cargo).some(n => n! > 0) || Object.values(site.details.modules).some(n => n! > 0)) return "Recovery available";
  return site.details.kind === "station" ? "Empty · can be restored" : "Depleted";
}
export function expeditionCapable(fleet: GhostView, task: ExpeditionTask): boolean {
  const kind = task === "investigate" || task === "study" ? "scout" : "convoy";
  const eligible = (k: GhostView["kind"]) => kind === "convoy" ? isPlayerFreighter(k) : k === kind;
  return fleet.own && (fleet.composition?.some(c => eligible(c.kind) && c.count > 0) ?? eligible(fleet.kind));
}
export function explorationOrder(st: ViewState, site: ExplorationSiteView, fleet: GhostView, task: ExpeditionTask) {
  if (!st.explorationSites.some(s => s.id === site.id) || !expeditionCapable(fleet, task)
      || (task !== "investigate" && !site.details)) return null;
  if ((task === "study" || task === "extract")
      && (task !== site.details?.opportunity?.task || deepExpeditionReason(site, fleet))) return null;
  // Ordinary visits keep their automatic arrival behavior; costly/specialized
  // work is explicitly selected and follows the normal Confirm/Cancel path.
  return task === "restore" || task === "study" || task === "extract"
    ? { type: "ExploreSite" as const, fleet_id: fleet.id, site_id: site.id, task }
    : { type: "MoveShip" as const, ship_id: fleet.id, dest: site.pos };
}

/** Mirrors completion requirements using received facts only. The sim rechecks
 * them on-site; a stale report can never promise that a cache remains intact. */
export function deepExpeditionReason(site: ExplorationSiteView, fleet: GhostView): string {
  const d = site.details, o = d?.opportunity;
  if (!o) return "Investigate first";
  if (!expeditionCapable(fleet, o.task)) return o.task === "study" ? "Scout required" : "Freighter required";
  if (d?.guarded) return "Clear the reported guardians first";
  if (o.task === "study" && d?.studied) return "Already studied";
  if (o.requirement === "research_team" && !fleet.loadouts?.some(f => f.n > 0
      && (f.modules.includes("recon_suite") || f.modules.includes("nebula_spectrometer")))
      && !(fleet.captain && fleet.captain.attributes.fieldcraft >= 2)) return "Recon Suite, Nebula Spectrometer or Fieldcraft 2 Captain";
  const manifest = fleet.cargo_manifest ?? (fleet.cargo ? [fleet.cargo] : []);
  if (Object.entries(o.costs).some(([kind, n]) => manifest.filter(c => c.commodity === kind).reduce((sum, c) => sum + c.units, 0) < n!)) return "Load expedition supplies first";
  if (o.task === "extract" && !Object.values(o.cargo).some(n => n! > 0)) return "Deep cache exhausted";
  if (o.task === "extract" && manifest.reduce((sum, c) => sum + c.units, 0)
      - Object.values(o.costs).reduce((sum, n) => sum + (n ?? 0), 0) >= fleetCargoCapacity(fleet)) return "Make room in the hold";
  return "";
}

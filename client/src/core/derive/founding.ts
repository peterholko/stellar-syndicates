import type { FoundingStage } from "../../protocol";
import type { SystemStateView } from "../../protocol";
import type { ViewState } from "../../state";

/** The growth chapter accepts freight OR refining; neither locks the other.
 * Legacy freight/colony stages are not extra steps. */
export const FOUNDING_TOTAL = 9;
export const FOUNDING_STEP: Record<FoundingStage, number> = {
  build_mine: 1, build_convoy: 2, export_production: 2,
  defeat_privateer: 3, complete_export: 4, build_shipyard: 5,
  build_second_freighter: 5, grow_business: 5, build_academy: 6, first_research: 7,
  build_scout: 8, survey_candidates: 9, build_colony: 9,
  establish_colony: 9, complete: 9,
  local_recovery: 2, prepare_export: 4,
};

/** Next actions, not a class-selection command. Derived solely from served
 * home/research reports; queued builds never pretend to satisfy a prerequisite. */
export function foundingBusinessGoals(st: ViewState, home?: SystemStateView) {
  const built = (kind: string) => home?.bodies.some(b => (b.structures[kind] ?? 0) > 0);
  const staffed = (kind: string) => home?.assignments.some(a => a.structure === kind
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  const enriched = st.research?.programmes.some(p => p.id === "mat_enrichment" && p.state === "completed");
  const freight = !built("shipyard") ? ["build-shipyard", "Build Shipyard"]
    : !staffed("shipyard") ? ["shipyard", "Staff Shipyard"] : ["build-convoy", "Build Tiny Freighter"];
  const refine = !built("academy") && !enriched ? ["build-academy", "Build Academy"]
    : !staffed("academy") && !enriched ? ["academy", "Staff Academy"]
    : !enriched ? ["research-enrichment", "Research Enrichment"]
    : !built("smelter") ? ["build-smelter", "Build Smelter"] : ["smelter", "Staff & supply Smelter"];
  return [
    { title: "Expand exports", copy: "Shipyard → second Freighter", action: freight[0], label: freight[1] },
    { title: "Start refining", copy: "Enrichment → operating Smelter", action: refine[0], label: refine[1] },
  ];
}

/** Parallel, actionable goals from received reports. The guide does not send
 * fleet orders or infer remote cache contents from a timer. */
export function foundingLocalGoals(st: ViewState, home?: SystemStateView) {
  const f = st.founding;
  if (!f?.local_opening || !["build_mine", "local_recovery", "prepare_export"].includes(f.stage)) return null;
  const mine = home?.bodies.some(b => (b.structures.mining_complex ?? 0) > 0);
  const staffed = home?.assignments.some(a => a.structure === "mining_complex" && a.workers > 0);
  const site = st.explorationSites.find(s => s.id === f.recovery_site);
  const fitted = st.ghosts.some(g => g.own && g.loadouts?.some(l => l.kind === "scout" && l.modules.includes("recon_suite")));
  const berth = home?.bodies.some(b => (b.structures.service_berth ?? 0) > 0);
  const goals = [];
  const camp = home?.bodies.find(b => b.founding_camp);
  if (camp && camp.founding_camp!.completed.length < 4) goals.push({ title: "Grow your Founding Camp", copy: "Prospect your world, improve extraction or storage, and prepare housing for arriving workers. Optional projects share your planet's construction queue.", action: "home-development", label: "Develop your settlement" });
  if (!mine || !staffed) goals.push({ title: "Develop home", copy: "Build the mine and assign its workforce while your Scout investigates.", action: mine ? "mine" : "build-mine", label: mine ? "Staff Mining Complex" : "Build Mining Complex" });
  if (!f.recovered) goals.push({ title: "Nearby recovery", copy: site?.details ? "Assign the Interceptor to guard your Freighter, then recover this finite cache. Bring its goods and Recon Suite home." : "Investigate the nearby beacon with your starting Scout. It will avoid threats.", action: "recovery", label: site?.details ? "Open guarded recovery" : site ? "Investigate local beacon" : "Waiting for beacon report" });
  if (f.privateer && !f.bounty_received) goals.push({ title: "Protect the recovery", copy: "The privateer threatens your civilian hull. Use the Interceptor; keep the Scout clear.", action: "privateer", label: "Inspect Privateer" });
  if (!fitted) goals.push({ title: "Upgrade your Scout", copy: !berth ? "Attach a Service Berth to the Warehouse: 5 Alloys, 3 Machinery; 2 minutes. It uses no extra slot." : "Return the recovered Recon Suite home, dock the Scout and fit it. An unstaffed berth temporarily borrows a mining crew.", action: berth ? "scout" : "build-service-berth", label: berth ? "Fit recovered Recon Suite" : "Build Service Berth" });
  goals.push({ title: f.stage === "prepare_export" ? "Last action: send the Hub shipment" : "Plan your next investment", copy: "Choose return supplies and a spending cap. The ship sells, buys what it can afford and returns; you need not wait online.", action: "opening-imports", label: "Plan guarded export & return" });
  return goals;
}

/** Refiners may reach exploration without having built the export-route yard. */
export function foundingScoutGoal(home?: SystemStateView) {
  const yard = home?.bodies.find(b => (b.structures.shipyard ?? 0) > 0);
  const staffed = yard && home?.assignments.some(a => a.body_id === yard.id && a.structure === "shipyard"
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  return !yard ? { title: "Prepare for exploration", copy: "Build a Shipyard for your Scout.", action: "build-shipyard", label: "Build Shipyard" }
    : !staffed ? { title: "Prepare for exploration", copy: "Assign workforce to build your Scout.", action: "shipyard", label: "Staff Shipyard" }
    : { title: "Build a Scout", copy: "Discover the resources in nearby systems.", action: "build-scout", label: "Build Scout" };
}

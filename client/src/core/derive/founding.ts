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

/** Refiners may reach exploration without having built the export-route yard. */
export function foundingScoutGoal(home?: SystemStateView) {
  const yard = home?.bodies.find(b => (b.structures.shipyard ?? 0) > 0);
  const staffed = yard && home?.assignments.some(a => a.body_id === yard.id && a.structure === "shipyard"
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  return !yard ? { title: "Prepare for exploration", copy: "Build a Shipyard for your Scout.", action: "build-shipyard", label: "Build Shipyard" }
    : !staffed ? { title: "Prepare for exploration", copy: "Assign workforce to build your Scout.", action: "shipyard", label: "Staff Shipyard" }
    : { title: "Build a Scout", copy: "Discover the resources in nearby systems.", action: "build-scout", label: "Build Scout" };
}

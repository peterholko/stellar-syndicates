// Funnel order of the founding charter, as the server's `tutorial::Stage`
// reports it: arriving, then the guide's checklist. Keys only; the server
// sends every label, so wording stays in one place.
export const STAGE_KEYS = [
  "signed_up", "entered",
  "founded",
  "mine", "shipment", "privateer",
  "shipyard", "research", "survey",
  "barge", "belt_mining", "haul",
] as const;

/** How far along a player is, for sorting: the next stage's place in the
 * funnel, past the end once the charter is fulfilled, -1 with no record. */
export function tutorialRank(tutorial: { next: string | null } | null | undefined): number {
  if (!tutorial) return -1;
  if (tutorial.next === null) return STAGE_KEYS.length;
  const index = (STAGE_KEYS as readonly string[]).indexOf(tutorial.next);
  return index < 0 ? -1 : index;
}

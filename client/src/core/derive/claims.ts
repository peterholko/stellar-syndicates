// §claims (v90; docs/scarcity-staples-plan.md step 1.7): a Colony Ship founds
// a colony only on a system its corporation holds the Authority's claim to.
// A claim is bought at the Hub for one system, has one holder, and lapses
// unused after 7 days. At war with the Authority none is needed, and a found
// sealed world is never sold (landing there is war).
import type { CharterView, EntityId, SeasonView, SystemInfo, SystemStateView } from "../../protocol";

export const CLAIM_TIP = "The Authority's leave to found a colony here. Bought at the Hub, one holder at a time; lapses after 7 days unused. Each colony and claim you hold makes the next ×1.6.";

export type ClaimOffer =
  | { kind: "held"; lapsesAt: number }
  | { kind: "price"; cost: number }
  | { kind: "war" }
  | { kind: "closed" };

type ClaimState = { charter?: CharterView | null; season?: SeasonView | null };

/** The claim on an unowned system, as heard; null where none applies (owned,
 * a home site held for a new charter, a sealed world, no charter yet). */
export function claimOffer(st: ClaimState, system: SystemInfo, dynamic: SystemStateView | undefined): ClaimOffer | null {
  if (dynamic?.owner != null || system.reserved) return null;
  if (st.season?.precursors.some((p) => p.system === system.id)) return null;
  const claims = st.charter?.claims;
  if (!claims) return null;
  const held = claims.held.find((claim) => claim.system === system.id);
  if (held) return { kind: "held", lapsesAt: held.lapses_at };
  if (claims.at_war) return { kind: "war" };
  if (claims.closed) return { kind: "closed" };
  return { kind: "price", cost: Math.round(system.claim_cost * claims.mult) };
}

/** Would a Colony Ship sent to `system` hold there for want of a claim? */
export function claimMissing(st: ClaimState, system: SystemInfo, dynamic: SystemStateView | undefined): boolean {
  const offer = claimOffer(st, system, dynamic);
  return offer?.kind === "price" || offer?.kind === "closed";
}

/** The warning on a Colony Ship sent without its claim. */
export const NO_CLAIM_WARNING = "No claim: buy one first. The ship holds there until it's held.";

/** The claims held, soonest lapse first. */
export function claimsHeld(charter: CharterView | null | undefined): { system: EntityId; lapses_at: number }[] {
  return [...(charter?.claims?.held ?? [])].sort((a, b) => a.lapses_at - b.lapses_at);
}

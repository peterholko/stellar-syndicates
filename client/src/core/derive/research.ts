// Shared research derivations extracted from the desktop shell.

import type { ProgrammeView, ResearchDynView, ResearchView } from "../../protocol";
import type { Net } from "../../net";
import { state } from "../../state";

let netSource: () => Net | null = () => null;

export function bindResearchNet(source: () => Net | null): void {
  netSource = source;
}


// The full ordered queue the player controls = [active, ...queue-ahead]. Sending
// it back as SetResearchQueue re-promotes the front to active (the sim's rule).
export function researchQueueIds(): string[] {
  const r = state.research;
  if (!r) return [];
  return r.active ? [r.active.id, ...r.queue] : [...r.queue];
}

export function sendResearchQueue(ids: string[]): void {
  const net = netSource();
  if (net) net.send({ type: "SetResearchQueue", queue: ids });
}


// §node: one-line description of what a node's bonus does (by slug). Used in the
// system panel + inbox so the tactical payoff is always legible.
export function nodeBonusDesc(slug: string): string {
  switch (slug) {
    case "relay_anchor":
      return "Halves your command delay to targets in its region — orders and their echoes land twice as fast nearby.";
    case "veil":
      return "Your dark fleets in its region run quieter — detected only at half the usual range.";
    case "deep_scan":
      return "Your sensors resolve EXACT composition on anything already visible in its region (bucket → exact).";
    default:
      return "A tactical edge to whoever holds it.";
  }
}


/// §TCA Phase 2: the projected band after `loss` more standing — the client-side
/// forecast behind the "this will be cited" confirmations. A PREVIEW, not a
/// promise: the citation only lands when its light reaches the Market Hub, and
/// standing regenerates in the meantime.
export function projectedBand(loss: number): string {
  const ch = state.charter;
  if (!ch) return "unknown";
  const after = ch.standing - loss;
  // Walk the server-supplied ladder (static, from Welcome): the first row whose
  // threshold we are at or below names the band (row 1, Sanctioned, is a strict
  // "below").
  const ladder = state.charterLadder;
  if (!ladder.length) return "unknown";
  let title = ladder[0][0];
  for (let i = 1; i < ladder.length; i++) {
    const [name, at] = ladder[i];
    const hit = i === 1 ? after < at : after <= at;
    if (hit) title = name;
  }
  return title;
}


// §perf Part B: join the View's DYNAMIC research slice onto the static Welcome
// catalog, rebuilding the full per-node shape the research panel reads. Joined
// by id (both lists come from the same visible_ids order, but the join never
// relies on that). A dyn entry without a catalog row is dropped — it cannot be
// rendered without name/board metadata (and can only mean a server/client
// catalog drift, which the protocol-version check already warns about).
export function mergeResearch(dyn: ResearchDynView, st = state): ResearchView {
  const cat = new Map(st.researchCatalog.map((p) => [p.id, p]));
  const programmes: ProgrammeView[] = [];
  for (const d of dyn.programmes) {
    const p = cat.get(d.id);
    if (!p) continue;
    programmes.push({ ...p, state: d.state, gate: d.gate ?? null });
  }
  return { active: dyn.active, queue: dyn.queue, rate: dyn.rate, stalled: dyn.stalled, academies: dyn.academies, programmes };
}

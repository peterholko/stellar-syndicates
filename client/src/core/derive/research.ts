// Shared research derivations extracted from the desktop shell.

import type { Commodity, NodeStateView, ProgrammeView, ResearchDynView, ResearchView } from "../../protocol";
import type { Net } from "../../net";
import { state } from "../../state";

let netSource: () => Net | null = () => null;

export function bindResearchNet(source: () => Net | null): void {
  netSource = source;
}


// The rows the player sees = [active, ...waiting], each id once. The active row
// is a pinned readout: only the waiting rows go back on the wire (below).
export function researchQueueIds(): string[] {
  const r = state.research;
  if (!r) return [];
  return [...new Set(r.active ? [r.active.id, ...r.queue] : r.queue)];
}

/** Has the player completed programme `id`? */
export function researched(id: string): boolean {
  return state.research?.programmes.some(p => p.id === id && p.state === "completed") ?? false;
}

/** SetResearchQueue's payload for a set of displayed rows: the waiting ids only.
 * The sim (ResearchState::set_queue) keeps an active programme and replaces
 * the queue behind it; with nothing active it promotes the first id. Echoing
 * the active id would queue it a second time behind itself. */
export function researchQueueCommand(rows: string[]): string[] {
  const active = state.research?.active?.id;
  return [...new Set(rows.filter(id => id !== active))];
}

export function sendResearchQueue(rows: string[]): void {
  const net = netSource();
  if (net) net.send({ type: "SetResearchQueue", queue: researchQueueCommand(rows) });
}

/** Only a programme the last report calls available, not already a row, may be
 * added. Sending never marks anything queued: the next report does. */
export function canQueueResearch(id: string): boolean {
  const programme = state.research?.programmes.find(p => p.id === id);
  return programme?.state === "available" && !researchQueueIds().includes(id);
}

export type ResearchQueueEdit = { add: string } | { remove: number } | { move: number; by: -1 | 1 };

/** The displayed rows after one edit, or null when it is refused: an
 * unavailable or duplicate add, a stale index, or anything that would move or
 * remove the pinned active row (row 0 while work is active). Both shells use
 * this, so neither can send what the other forbids. */
export function editResearchQueue(edit: ResearchQueueEdit): string[] | null {
  const rows = researchQueueIds();
  const firstMovable = state.research?.active ? 1 : 0;
  if ("add" in edit) return canQueueResearch(edit.add) ? [...rows, edit.add] : null;
  const index = "remove" in edit ? edit.remove : edit.move;
  if (!Number.isInteger(index) || index < firstMovable || index >= rows.length) return null;
  if ("remove" in edit) return rows.filter((_, i) => i !== index);
  const to = index + edit.by;
  if (to < firstMovable || to >= rows.length) return null;
  [rows[index], rows[to]] = [rows[to], rows[index]];
  return rows;
}

/** The sim's ladder rule (research.rs `is_predecessor`): Tiers II–III follow
 * the field's shared (school-less) programmes, Tier IV and up the same school. */
export function isPredecessor(q: Pick<ProgrammeView, "field" | "school" | "tier">, p: Pick<ProgrammeView, "field" | "school" | "tier">): boolean {
  if (q.tier + 1 !== p.tier || q.field !== p.field) return false;
  return p.tier <= 3 ? q.school === null : q.school === p.school;
}

/** A programme's state in words, readable without its colour. A programme
 * sent to the queue reads Queued once it is a row. */
export function researchStatus(programme: Pick<ProgrammeView, "id" | "state">, queue: string[]): string {
  if (programme.state === "completed") return "Completed";
  if (programme.state === "active") return "In progress";
  if (programme.state === "queued" || queue.includes(programme.id)) return "Queued";
  if (programme.state === "available") return "Available";
  return "Locked";
}

const REACHED = new Set(["completed", "active", "queued", "available"]);

/** §research-panel (Peter, Nov 1 2026: "hide what research is available past
 * 1 level deep"): what the research panels render. Everything researched or
 * researchable, plus the next step after each of those; nothing deeper. */
export function visibleProgrammes(programmes: ProgrammeView[]): ProgrammeView[] {
  const reached = programmes.filter(p => REACHED.has(p.state));
  return programmes.filter(p => REACHED.has(p.state) || p.tier === 1 || reached.some(q => isPredecessor(q, p)));
}


// §rare-materials: what a programme draws, per hour at research rate 1 (one
// staffed Tier I Academy), with a good listed twice merged into one row.
export function researchSuppliesPerHour(programme: Pick<ProgrammeView, "supplies">): [Commodity, number][] {
  const merged = new Map<Commodity, number>();
  for (const [good, perSecond] of programme.supplies ?? []) merged.set(good, (merged.get(good) ?? 0) + perSecond * 3600);
  return [...merged];
}

export const supplyAmount = (perHour: number): string => perHour >= 10 ? Math.round(perHour).toLocaleString() : perHour.toFixed(1);

/** Research work in research-hours at rate 1 (one staffed Tier I Academy). */
export const researchHours = (seconds: number): string => `${(seconds / 3600).toLocaleString("en-US", { maximumFractionDigits: 1 })} h`;


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

/** §node: a node in a few words for the map hover and the system panels; the
 * map itself draws only a dim ◈ unless the viewer holds the node. */
export function nodeLabel(node: NodeStateView, mine: boolean): string {
  return `◈ ${node.title}${!node.awakened ? " · dormant" : mine && !node.fed ? " · unfed" : ""}`;
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
    programmes.push({ ...p, state: d.state, gate: d.gate ?? null, recovered_data: d.recovered_data });
  }
  return { active: dyn.active, queue: dyn.queue, rate: dyn.rate, stalled: dyn.stalled, academies: dyn.academies, programmes, blueprints: dyn.blueprints ?? [], grant: dyn.grant, designations: dyn.designations ?? [], structure_tiers: dyn.structure_tiers };
}

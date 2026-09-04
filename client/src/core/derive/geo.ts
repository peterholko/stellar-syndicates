// Shared geo derivations extracted from the desktop shell.

import { formatId, type Deposit, type GhostView, type StandingEndpoint, type SystemInfo, type Vec2 } from "../../protocol";
import { renderer } from "../../render";
import { state } from "../../state";
import { label } from "../../icons";

export const HYPERLIMIT_SU = 900;
export const REPORT_RECENT_S = 300;

export type DockTarget = { key: string; name: string; pos: Vec2; distance: number };

let knownGeologyIds: Set<string> | null = null;
export const freshSurveyReports = new Map<string, number>();


export function gravityWellAt(pos: Vec2, st = state): string | null {
  const galaxy = st.galaxy;
  if (!galaxy) return null;
  if (Math.hypot(pos.x - galaxy.hub.x, pos.y - galaxy.hub.y) < galaxy.hyperlimit) {
    return "the Market Hub's gravity well";
  }
  const system = galaxy.systems.find((s) =>
    Math.hypot(pos.x - s.pos.x, pos.y - s.pos.y) < galaxy.hyperlimit,
  );
  return system ? `${system.name}'s gravity well` : null;
}


/// The nearest berth in the PLAYER'S SERVED picture. Choosing it here keeps the
/// button epistemically identical to clicking that known map object yourself;
/// the resulting MoveShip still travels through the ordinary delayed-order
/// pipeline and the sim alone decides whether the fleet is docked on arrival.
/// Extend this candidate list when another object becomes a real DockSite.
export function nearestKnownDock(g: GhostView): DockTarget | null {
  const candidates: DockTarget[] = [];
  const add = (key: string, name: string, pos: Vec2) => candidates.push({
    key, name, pos, distance: Math.hypot(pos.x - g.pos.x, pos.y - g.pos.y),
  });
  if (state.galaxy) {
    add("hub", "Market Hub", state.galaxy.hub);
    for (const system of state.galaxy.systems) {
      const served = state.systems.find((entry) => entry.id === system.id);
      if (served?.owner === state.playerId || served?.ally) {
        add(`system:${system.id}`, system.name, system.pos);
      }
    }
  }
  candidates.sort((a, b) => a.distance - b.distance || a.key.localeCompare(b.key));
  return candidates[0] ?? null;
}


export function operationSystemName(id: string): string {
  return state.galaxy?.systems.find((s) => s.id === id)?.name ?? formatId(id);
}


// The star system under a screen point (for double-click / deep-zoom enter).
// Each star counts within its OWN rendered disk (so a deep-zoom giant's rim is
// enterable) or within `slack` of its center (so small stars stay easy to hit);
// the NEAREST CENTER wins among qualifiers — aiming at a small star always
// beats a visually larger neighbor whose disk merely blankets the same pixel.
export function systemUnderCursor(sx: number, sy: number, slack = 22): SystemInfo | null {
  if (!state.galaxy) return null;
  let best: SystemInfo | null = null;
  let bestD = Infinity;
  for (const sys of state.galaxy.systems) {
    const s = renderer.worldToScreen(sys.pos);
    const d = Math.hypot(s.x - sx, s.y - sy);
    if (d < Math.max(slack, renderer.systemHitRadius(sys)) && d < bestD) { bestD = d; best = sys; }
  }
  return best;
}


/// §explore R2: OUR known geology for a system — the exact deposit table iff we
/// surveyed it or own it (from the light-gated view), else null (band only).
export function knownDeposits(sysId: string, st = state): Deposit[] | null {
  return st.systems.find((s) => s.id === sysId)?.deposits ?? null;
}

/// The currently-viewed system id, or null when not in the System View.
export function viewedSystemId(): string | null {
  const m = renderer.viewMode;
  return m.type === "system" ? m.systemId : null;
}

/// §bodies: feed the scene its per-body dynamic layer straight from the wire —
/// the roster (public geography; a rival's bodies carry no structures, so fog
/// needs no client gate), the build queue (owner-only on the wire), the food.
export function pushSystemDynamic(sid: string): void {
  const dyn = state.systems.find((s) => s.id === sid);
  renderer.setSystemDynamic(
    dyn?.bodies ?? [],
    (dyn?.builds ?? []).map((j) => ({ key: j.key, body_id: j.body_id })),
    dyn?.habitat_fed ?? true,
  );
}

/// The nearest system's name, as a human-readable "where" for a battle site.
export function nearestSystemName(p: Vec2): string {
  let best = "";
  let bestD = Infinity;
  for (const s of state.galaxy?.systems ?? []) {
    const d = Math.hypot(s.pos.x - p.x, s.pos.y - p.y);
    if (d < bestD) { bestD = d; best = s.name; }
  }
  return bestD < 200 ? `at ${best}` : best ? `near ${best} (${bestD.toFixed(0)} su out)` : `at (${p.x.toFixed(0)}, ${p.y.toFixed(0)})`;
}


// §emplacements: wire slug → display name. `label()` already title-cases the
// slug, so the vocabulary stays owned by the sim rather than re-typed here.
export function emplacementLabel(kind: string): string {
  return label(kind);
}


// --- Standing orders panel (§15) — constrained logistics automation ----------
export function systemName(id: string): string {
  return state.galaxy?.systems.find((x) => x.id === id)?.name ?? id;
}

export function ownedSystems(): { id: string; name: string }[] {
  if (state.playerId === null) return [];
  return state.systems
    .filter((s) => s.owner === state.playerId)
    .map((s) => ({ id: s.id, name: systemName(s.id) }));
}

// §syndicates Part 3: SYNDICATE-ally systems (per the viewer's known membership)
// are valid AID destinations for standing orders / convoys — deliveries credit the
// ally's stockpile (blockades still interdict the run).
export function allySystems(): { id: string; name: string }[] {
  return state.systems
    .filter((s) => s.ally)
    .map((s) => ({ id: s.id, name: systemName(s.id) }));
}

export function endpointLabel(e: StandingEndpoint): string {
  return e.kind === "hub" ? "hub" : e.kind === "home" ? "home" : systemName(e.id);
}
 // system id → sim-time noticed

export function noteSurveyReports(simTime: number, st = state): void {
  const cur = new Set(st.systems.filter((x) => x.deposits != null).map((x) => x.id));
  if (knownGeologyIds === null) {
    knownGeologyIds = cur; // first View: seed silently
    return;
  }
  for (const id of cur) {
    if (!knownGeologyIds.has(id)) {
      knownGeologyIds.add(id);
      const dyn = st.systems.find((x) => x.id === id);
      if (dyn?.owner !== st.playerId) freshSurveyReports.set(id, simTime);
    }
  }
  // Age out stale reports (they remain in the log/panel; the CARD is for the
  // decision window).
  for (const [id, t] of freshSurveyReports) {
    if (simTime - t > REPORT_RECENT_S) freshSurveyReports.delete(id);
  }
}


// One-way command delay (cc → pos) — the SAME echo math the order lifecycle uses;
// null before the galaxy/CC arrive.
export function commandDelayTo(pos: Vec2): number | null {
  if (!state.commandCenter || !state.galaxy) return null;
  return Math.hypot(pos.x - state.commandCenter.x, pos.y - state.commandCenter.y) / state.galaxy.c;
}

// Nearest KNOWN system name to a point (for naming a battle/report location).
export function locName(pos: Vec2): string {
  if (!state.galaxy) return `(${Math.round(pos.x)}, ${Math.round(pos.y)})`;
  let best: { name: string; d: number } | null = null;
  for (const s of state.galaxy.systems) {
    const d = Math.hypot(s.pos.x - pos.x, s.pos.y - pos.y);
    if (!best || d < best.d) best = { name: s.name, d };
  }
  return best ? best.name : `(${Math.round(pos.x)}, ${Math.round(pos.y)})`;
}


export function foundingHomeSystemId(): string | null {
  if (!state.playerId || !state.galaxy || !state.commandCenter) return null;
  const pos = new Map(state.galaxy.systems.map((s) => [s.id, s.pos]));
  return state.systems
    .filter((s) => s.owner === state.playerId && pos.has(s.id))
    .sort((a, b) => {
      const ap = pos.get(a.id)!;
      const bp = pos.get(b.id)!;
      return Math.hypot(ap.x - state.commandCenter!.x, ap.y - state.commandCenter!.y)
        - Math.hypot(bp.x - state.commandCenter!.x, bp.y - state.commandCenter!.y);
    })[0]?.id ?? null;
}

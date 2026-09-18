import type { GhostView, SystemInfo, SystemStateView } from "../../protocol";
import type { ViewState } from "../../state";
import { dockedAtSystem, guardCapable } from "./fleet";

// Mirrors sim ship.rs::SYSTEM_DEFENSE_*; these are assignment choices, not sensors.
export const DEFENSE_RADII = [5_000, 10_000, 20_000] as const;
export const DEFAULT_DEFENSE_RADIUS = 10_000;

export function defenseOrderBlock(st: ViewState, fleet: GhostView): string {
  if (st.battles.some(b => b.participants.includes(fleet.id))) return "In a reported battle";
  if (fleet.job) return "Work in progress";
  if ((st.pendingOrders.get(fleet.id) ?? []).some(o => !o.lost)) return "Awaiting order response";
  return "";
}

/** The post comes from the served ghost, NOT pending orders or predicted motion.
 * A departure/death remains here until its light arrives; a new assignment is
 * listed separately as pending until compliance-era telemetry shows the post.
 * Contacts are reports, never a promise that a system is currently safe. */
export function systemDefense(st: ViewState, system: SystemInfo, report: SystemStateView) {
  const distance = (g: GhostView) => Math.hypot(g.pos.x - system.pos.x, g.pos.y - system.pos.y);
  const armed = st.ghosts.filter(guardCapable);
  const assigned = armed.filter(g => g.defend_system?.system === system.id);
  const local = armed.filter(g => dockedAtSystem(g, system.id) || (!g.docked && distance(g) <= (st.galaxy?.hyperlimit ?? 900)));
  const defenders = [...new Map([...assigned, ...local].map(g => [g.id, g])).values()];
  const pending = [...st.pendingOrders.values()].flat().filter(o => !o.lost && o.kind === "defend" && o.target_id === system.id);
  const contacts = st.ghosts.filter(g => !g.own && !g.ally && !g.tca
    && guardCapable({ ...g, own: true }) && distance(g) <= 20_000);
  return { assigned, local, defenders, pending, contacts, platforms: report.defense_tier,
    candidates: armed.filter(g => g.defend_system?.system !== system.id),
    // A historical warning is explicitly labeled a report; it never creates a
    // live fleet, an ETA, or an active-threat timer from hidden server state.
    warning: [...st.timeline].reverse().find(e => e.text.startsWith(`Pirate raid incoming: ${system.name} —`)),
  };
}

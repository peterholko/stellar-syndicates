import type { GhostView, MissionProfile, PendingOrderView } from "../protocol";
import type { FleetCommand } from "../core/fleetorders";

export const DEFAULT_MISSION: MissionProfile = { priority: "balanced", screening: "automatic", withdrawal: "never" };
export const MISSION_FIELDS: { key: keyof MissionProfile; label: string; choices: [string, string][] }[] = [
  { key: "priority", label: "Target priority", choices: [["balanced", "Balanced"], ["missile_ships", "Missile ships"], ["installations", "Installations"], ["transports", "Transports"]] },
  { key: "screening", label: "Formation role", choices: [["automatic", "Automatic"], ["protect_transports", "Screen transports"]] },
  { key: "withdrawal", label: "Withdraw if a hull falls below", choices: [["never", "Never"], ["hull30", "30%"], ["hull50", "50%"], ["hull70", "70%"]] },
];

export function missionCommand(g: GhostView, field: string | undefined, value: string | undefined): FleetCommand | null {
  if (!g.own || !MISSION_FIELDS.some(f => f.key === field && f.choices.some(([v]) => v === value))) return null;
  const mission = { ...(g.mission_profile ?? DEFAULT_MISSION), [field!]: value } as MissionProfile;
  return { type: "SetFleetMission", fleet_id: g.id, mission };
}

export function missionHtml(g: GhostView, orders: { lost?: boolean; configuration?: PendingOrderView["configuration"] }[], mobile = false): string {
  if (!g.own) return "";
  const pending = [...orders].reverse().find(o => !o.lost && o.configuration?.kind === "mission")?.configuration;
  const current = pending?.kind === "mission" ? pending.mission : g.mission_profile ?? DEFAULT_MISSION;
  const action = mobile ? 'data-mobile-act="fleet-mission"' : 'data-deck-act="fleet-mission"';
  return `<section class="${mobile ? "m-section" : "deck-section"}"><h3>Mission profile${pending ? " · signal in flight" : ""}</h3>${MISSION_FIELDS.map(f =>
    `<div class="${mobile ? "m-action-group" : "deck-command-block"}"><b>${f.label}</b><div class="${mobile ? "m-actions" : "deck-segment"}">${f.choices.map(([v, label]) =>
      `<button type="button" ${action} data-id="${g.id}" data-field="${f.key}" data-value="${v}" aria-pressed="${current[f.key] === v}" ${pending ? "disabled" : ""}>${label}</button>`).join("")}</div></div>`).join("")}</section>`;
}

export function pirateFactionHtml(g: GhostView, mobile = false): string {
  const profiles = {
    ashwake: ["Ashwake Corsairs", "Torpedo salvos · counter with point defense and missile-ship priority."],
    ironclad: ["Ironclad Reclaimers", "Slow armored scavengers · torpedoes bypass their plating."],
    rift: ["Rift Stalkers", "Fast driver ambushers · Whipple Armor and transport screens. Retreat below 50% hull."],
  };
  const p = g.pirate_faction ? profiles[g.pirate_faction] : null;
  return p ? `<section class="${mobile ? "m-section" : "deck-section"}"><h3>${p[0]}</h3><p>${p[1]}</p></section>` : "";
}

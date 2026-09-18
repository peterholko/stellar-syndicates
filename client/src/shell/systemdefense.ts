import { nearestRepairYard } from "../battleaftermath";
import { defenseOrderBlock, DEFENSE_RADII, systemDefense } from "../core/derive/defense";
import { shipKindLabel } from "../core/derive/fleet";
import { fleetReadiness } from "../core/derive/readiness";
import { countClassLabel, fleetExactCount, type SystemInfo, type SystemStateView } from "../protocol";
import type { ViewState } from "../state";
import type { CoreContext } from "./types";

const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** Compact shared desktop/mobile section. No polling, estimates or truth state. */
export function systemDefenseHtml(st: ViewState, system: SystemInfo, report: SystemStateView,
  radius: number, theme: "deck" | "mobile", assigning = false): string {
  const d = systemDefense(st, system, report);
  const section = theme === "deck" ? "deck-section" : "m-section";
  const muted = theme === "deck" ? "deck-muted" : "m-muted";
  const act = theme === "deck" ? "data-deck-act" : "data-mobile-act";
  const button = (action: string, label: string, fleet = "", reason = "") =>
    `<button type="button" ${act}="defense-${action}" data-system="${esc(system.id)}" data-fleet="${esc(fleet)}"${reason ? ` disabled title="${esc(reason)}"` : ""}>${esc(label)}</button>`;
  const name = (g: (typeof d.defenders)[number]) => `${shipKindLabel(g.kind)} · ${g.id}`;
  const rows = d.defenders.map(g => {
    const r = fleetReadiness(g), post = g.defend_system?.system === system.id;
    const local = d.local.includes(g), block = defenseOrderBlock(st, g);
    const yard = nearestRepairYard(st, g);
    const status = st.battles.some(b => b.participants.includes(g.id)) ? "In battle"
      : post ? local ? "On station" : "Assigned · under way" : g.defend_system ? "Local · defending elsewhere" : "Local · unassigned";
    const readiness = `${r.hull === null ? "Hull unknown" : `${Math.round(r.hull * 100)}% hull`} · ${r.fuel === null ? "Fuel unknown" : `${Math.floor(r.fuel)} fuel`} · report ${Math.ceil(g.age)}s old`;
    const repairReason = block || (g.damage == null || g.damage <= .0001 ? "No reported damage" : "")
      || (!yard ? "Needs a staffed, supplied Ordnance Foundry" : "")
      || (yard && (g.docked === yard.id || g.docked === `E${yard.id}`) ? "Already at repair yard" : "");
    return `<div class="system-defense-row"><div><b>${esc(name(g))}</b><small>${status}${post ? ` · ${g.defend_system!.radius.toLocaleString()} su limit` : ""}</small><small>${esc(readiness)}</small></div><div class="system-defense-actions">${button("open", "Select", g.id)}${button("repair", "Repair", g.id, repairReason)}${post ? button("release", "Release", g.id, block) : ""}</div>${block ? `<small>${esc(block)}</small>` : ""}</div>`;
  }).join("");
  const pending = d.pending.map(o => `<div class="${muted}">Defense assignment sent · fleet ${esc(o.fleet_id)} · awaiting report</div>`).join("");
  const contacts = d.contacts.map(g => `<div class="system-defense-row"><div><b>${g.pirate ? "Pirate" : "Rival"} ${esc(shipKindLabel(g.kind))}</b><small>${fleetExactCount(g) ?? countClassLabel(g.count_class)} ships · report ${Math.ceil(g.age)}s old</small></div></div>`).join("");
  const choices = d.candidates.map(g => `<div class="system-defense-row"><div><b>${esc(name(g))}</b><small>${g.defend_system ? `Defending another system` : g.guard_target ? "Guarding a fleet" : "Available assignment"}</small></div>${button("assign", "Assign", g.id, defenseOrderBlock(st, g))}</div>`).join("");
  const limits = DEFENSE_RADII.map(r => `<button type="button" ${act}="defense-radius" data-system="${esc(system.id)}" data-radius="${r}" aria-pressed="${r === radius}">${(r / 1000).toFixed(0)},000 su</button>`).join("");
  return `<section class="${section} system-defense"><header><h3>Defense</h3><span>${d.platforms} platform tier${d.platforms === 1 ? "" : "s"} · ${d.local.length} local fleet${d.local.length === 1 ? "" : "s"}</span></header>
    ${rows || `<div class="${muted}">No defenders reported here.</div>`}${pending}
    ${button("toggle", assigning ? "Hide assignments" : "Assign a fleet")}${assigning ? `<div><small>New assignment · pursuit limit from this system</small><div class="system-defense-actions">${limits}</div>${choices || `<div class="${muted}">No other combat fleets.</div>`}</div>` : ""}
    <div class="system-defense-actions">${button("build", "Build defenses")}${button("doctrine", "Retreat policy")}</div>
    <h4>Known threats</h4>${report.blockade ? `<div class="${muted}">Blockade reported</div>` : ""}${contacts || `<div class="${muted}">No nearby armed contacts reported.</div>`}
    ${d.warning ? `<div class="system-defense-actions"><small>Raid warning received at ${Math.floor(d.warning.at_time)}s</small>${button("log", "View report")}</div>` : ""}
    </section>`;
}

/** Actions freeze a normal command preview. Nothing sends before Confirm. */
export function stageDefenseAction(ctx: CoreContext, action: string, system: string, fleetId: string, radius: number): void {
  const fleet = ctx.state.ghosts.find(g => g.own && g.id === fleetId);
  if (!fleet || defenseOrderBlock(ctx.state, fleet)) return;
  if (action === "defense-assign") ctx.intent.beginFleetCommand({ type: "DefendSystem", fleet_id: fleet.id, system_id: system, pursuit_radius: radius });
  if (action === "defense-release") ctx.intent.beginFleetCommand({ type: "HoldFleet", ship_id: fleet.id });
  if (action === "defense-repair") {
    const yard = nearestRepairYard(ctx.state, fleet);
    if (yard && fleet.damage != null && fleet.damage > .0001) ctx.intent.beginFleetCommand({ type: "MoveShip", ship_id: fleet.id, dest: yard.pos });
  }
}

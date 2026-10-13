import { nearestRepairYard } from "../battleaftermath";
import { defenseOrderBlock, DEFENSE_RADII, systemDefense } from "../core/derive/defense";
import { shipKindLabel } from "../core/derive/fleet";
import { blockadeBrief } from "../core/derive/blockade";
import { fleetReadiness } from "../core/derive/readiness";
import { countClassLabel, fleetExactCount, type ClientMsg, type ModuleKind, type SystemInfo, type SystemStateView } from "../protocol";
import { MOUNT_MODULES, mountFitError, mountLabel, mountStatus, refitCrates, stationArtUrl, stationSummary } from "../core/derive/station";
import { label } from "../icons";

/** A hardpoint refit being composed (nothing is sent before Confirm). */
export interface StationDraft {
  mount: number;
  fit: ModuleKind[];
}
import type { ViewState } from "../state";
import type { CoreContext } from "./types";

const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** Compact shared desktop/mobile section. No polling, estimates or truth state. */
export function systemDefenseHtml(st: ViewState, system: SystemInfo, report: SystemStateView,
  radius: number, theme: "deck" | "mobile", assigning = false, draft: StationDraft | null = null): string {
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
    const readiness = `${r.hull === null ? "Hull unknown" : `${Math.round(r.hull * 100)}% hull`} · ${r.fuel === null ? "Fuel unknown" : `${Math.floor(r.fuel)} ${label(r.fuelKind)}`} · report ${Math.ceil(g.age)}s old`;
    const repairReason = block || (g.damage == null || g.damage <= .0001 ? "No reported damage" : "")
      || (!yard ? "Needs a staffed, supplied Ordnance Foundry" : "")
      || (yard && (g.docked === yard.id || g.docked === `E${yard.id}`) ? "Already at repair yard" : "");
    return `<div class="system-defense-row"><div><b>${esc(name(g))}</b><small>${status}${post ? ` · ${g.defend_system!.radius.toLocaleString()} su limit` : ""}</small><small>${esc(readiness)}</small></div><div class="system-defense-actions">${button("open", "Select", g.id)}${button("repair", "Repair", g.id, repairReason)}${post ? button("release", "Release", g.id, block) : ""}</div>${block ? `<small>${esc(block)}</small>` : ""}</div>`;
  }).join("");
  const pending = d.pending.map(o => `<div class="${muted}">Defense assignment sent · fleet ${esc(o.fleet_id)} · awaiting report</div>`).join("");
  const contacts = d.contacts.map(g => `<div class="system-defense-row"><div><b>${g.pirate ? "Pirate" : "Rival"} ${esc(shipKindLabel(g.kind))}</b><small>${fleetExactCount(g) ?? countClassLabel(g.count_class)} ships · report ${Math.ceil(g.age)}s old</small></div></div>`).join("");
  const choices = d.candidates.map(g => `<div class="system-defense-row"><div><b>${esc(name(g))}</b><small>${g.defend_system ? `Defending another system` : g.guard_target ? "Guarding a fleet" : "Available assignment"}</small></div>${button("assign", "Assign", g.id, defenseOrderBlock(st, g))}</div>`).join("");
  const limits = DEFENSE_RADII.map(r => `<button type="button" ${act}="defense-radius" data-system="${esc(system.id)}" data-radius="${r}" aria-pressed="${r === radius}">${(r / 1000).toFixed(0)},000 su</button>`).join("");
  const fixed = report.station ? stationSummary(report.station)
    : d.platforms ? `${d.platforms} platform tier${d.platforms === 1 ? "" : "s"}` : "No station";
  return `<section class="${section} system-defense"><header><h3>Defense</h3><span>${esc(fixed)} · ${d.local.length} local fleet${d.local.length === 1 ? "" : "s"}</span></header>
    ${stationHtml(st, system, report, act, muted, draft)}
    ${rows || `<div class="${muted}">No defenders reported here.</div>`}${pending}
    ${button("toggle", assigning ? "Hide assignments" : "Assign a fleet")}${assigning ? `<div><small>New assignment · pursuit limit from this system</small><div class="system-defense-actions">${limits}</div>${choices || `<div class="${muted}">No other combat fleets.</div>`}</div>` : ""}
    <div class="system-defense-actions">${button("build", "Build defenses")}${button("doctrine", "Retreat policy")}</div>
    <h4>Known threats</h4>${report.blockade && st.galaxy ? `<div class="${muted}">${esc(blockadeBrief(report.blockade, st.simTime, st.galaxy.siege_secs, report.ground?.marines_needed).title)} · reported</div>` : ""}${contacts || `<div class="${muted}">No nearby armed contacts reported.</div>`}
    ${d.warning ? `<div class="system-defense-actions"><small>Raid warning received at ${Math.floor(d.warning.at_time)}s</small>${button("log", "View report")}</div>` : ""}
    </section>`;
}

/** §orbital-defense: the station's hardpoints (fit, state, refit) and its
 * next level. Owner-only data from the arrived site report. */
function stationHtml(st: ViewState, system: SystemInfo, report: SystemStateView, act: string, muted: string,
  draft: StationDraft | null): string {
  const station = report.station;
  const option = st.galaxy?.build_options?.find((o) => o.key === "orbital_defense");
  const done = (id: string | null | undefined): boolean => !id || !!st.research?.programmes.some((p) => p.id === id && p.state === "completed");
  const researchName = (id: string) => st.research?.programmes.find((p) => p.id === id)?.name ?? label(id);
  const button = (action: string, text: string, extra = "", reason = "") =>
    `<button type="button" ${act}="defense-${action}" data-system="${esc(system.id)}"${extra}${reason ? ` disabled title="${esc(reason)}"` : ""}>${esc(text)}</button>`;
  const next = option?.tiers?.find((t) => t.tier === (station?.level ?? 0) + 1);
  const nextNeeds = next ? [
    !done(next.research_prerequisite) ? `${researchName(next.research_prerequisite!)} research` : "",
    next.requires && (report.structures?.[next.requires.yard] ?? 0) < next.requires.tier ? `${label(next.requires.yard)} tier ${next.requires.tier} in the system` : "",
  ].filter(Boolean) : [];
  const nextHtml = next
    ? `<div class="system-station-next"><small>Next: <b>${esc(next.label)}</b> · ${esc(next.description)}${nextNeeds.length ? ` Needs ${esc(nextNeeds.join(" and "))}.` : ""}</small>${button("build", station ? `Raise to ${next.label}` : `Build a ${next.label}`)}</div>`
    : station ? `<small class="${muted}">The Star Fortress is the top of the ladder.</small>` : "";
  if (!station) return nextHtml;
  const lances = done("weap_strike_iv_platform_lances");
  const foundry = (report.structures?.ordnance_foundry ?? 0) > 0;
  const name = (m: ModuleKind) => label(m);
  const rows = station.mounts.map((m, i) => {
    const editing = draft?.mount === i ? draft : null;
    const head = `<div><b>Hardpoint ${i + 1}</b><small>${esc(mountLabel(m.modules, name))} · ${esc(mountStatus(m))}</small></div>`;
    if (!editing) {
      return `<div class="system-defense-row system-station-mount">${head}<div class="system-defense-actions">${button("refit", "Refit", ` data-mount="${i}"`,
        foundry ? "" : "Refitting needs an Ordnance Foundry in the system")}</div></div>`;
    }
    const error = mountFitError(editing.fit, lances);
    const crates = refitCrates(m.modules, editing.fit);
    const short = Object.entries(crates).filter(([k, n]) => (report.modules?.[k] ?? 0) < (n ?? 0))
      .map(([k, n]) => `${n}× ${name(k as ModuleKind)} (have ${report.modules?.[k] ?? 0})`);
    const chips = editing.fit.map((mod, k) => button("refit-remove", `${name(mod)} ✕`, ` data-mount="${i}" data-index="${k}"`)).join("")
      || `<span class="${muted}">Stock beam battery</span>`;
    const palette = MOUNT_MODULES.filter((mod) => mod !== "prismatic_lance" || lances).map((mod) =>
      button("refit-add", `+ ${name(mod)}${mod === "prismatic_lance" ? "" : ` · ${report.modules?.[mod] ?? 0} crates`}`,
        ` data-mount="${i}" data-module="${mod}"`, editing.fit.length >= 3 ? "Three slots" : "")).join("");
    const blocked = error || (short.length ? `Not enough crates here: ${short.join(", ")}.` : "");
    return `<div class="system-defense-row system-station-mount is-editing">${head}
      <div class="system-defense-actions">${chips}</div><div class="system-defense-actions">${palette}</div>
      ${blocked ? `<small>${esc(blocked)}</small>` : ""}
      <div class="system-defense-actions">${button("refit-confirm", `Refit hardpoint ${i + 1}`, ` data-mount="${i}"`, blocked)}${button("refit-cancel", "Cancel")}</div></div>`;
  }).join("");
  return `<div class="system-station"><img src="${stationArtUrl(station.level, 256)}" alt="${esc(station.name)}" width="72" height="72"><div><b>${esc(station.name)}</b><small>Level ${station.level} of 5 · ${esc(stationSummary(station))}</small></div></div>${rows}${nextHtml}`;
}

/** The refit editor's actions, shared by both shells: the next draft (null
 * closes it) and, on Confirm, the order to send. `undefined` = not a refit
 * action. The sim re-checks crates, foundry and fit when the order lands. */
export function stationDraftAction(action: string, data: DOMStringMap, draft: StationDraft | null,
  report: SystemStateView | undefined): { draft: StationDraft | null; send?: ClientMsg } | undefined {
  const mount = Number(data.mount);
  const current = report?.station?.mounts[mount]?.modules;
  switch (action) {
    case "defense-refit":
      return current ? { draft: { mount, fit: [...current] } } : { draft };
    case "defense-refit-add":
      return draft && draft.mount === mount && data.module && draft.fit.length < 3
        ? { draft: { mount, fit: [...draft.fit, data.module as ModuleKind] } } : { draft };
    case "defense-refit-remove":
      return draft && draft.mount === mount
        ? { draft: { mount, fit: draft.fit.filter((_, k) => k !== Number(data.index)) } } : { draft };
    case "defense-refit-cancel":
      return { draft: null };
    case "defense-refit-confirm":
      return draft && report && draft.mount === mount
        ? { draft: null, send: { type: "FitStation", system_id: report.id, mount, fit: draft.fit } } : { draft };
    default:
      return undefined;
  }
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

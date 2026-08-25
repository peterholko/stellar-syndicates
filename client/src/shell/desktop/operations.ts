import { fmtEta, operationCopy, operationHullArt, operationIcon, operationReward, operationTitle } from "../../core/derive/format";
import { liveSimTime, state } from "../../state";
import { $, esc } from "./mapchrome";


// --- Operations: one board for contracts, objectives, and shared projects ---
export let lastOperationsSig = "";

export const MIDGAME_COPY: Record<import("../../protocol").MidgameStage, [string, string]> = {
  home_development: ["Home development", "Build a reliable industrial base and finish the founding programme."],
  exploration: ["Exploration", "Use scouts and expedition offers to turn nearby darkness into choices."],
  specialization: ["Specialization", "Compare surveyed strengths and choose what this corporation will do unusually well."],
  first_colony: ["First colony", "Commit the colony ship and establish a second physical holding."],
  trade_network: ["Trade network", "Connect specialized holdings through contracts, freight, and escorted freighters."],
  contested_expansion: ["Contested expansion", "Public objectives and scarce sites now put your plans against rival corporations."],
  regional_power: ["Regional power", "Hold strategic nodes and organize multi-stage syndicate operations."],
};


export function openOperations(): void {
  $("operations-panel").classList.add("is-open");
  $("nav-operations").classList.add("is-active");
  lastOperationsSig = "";
  updateOperationsPanel();
}

export function closeOperations(): void {
  $("operations-panel").classList.remove("is-open");
  $("nav-operations").classList.remove("is-active");
}

export function toggleOperations(): void {
  if ($("operations-panel").classList.contains("is-open")) closeOperations();
  else openOperations();
}


export function operationCard(o: import("../../protocol").OperationView): string {
  const pct = Math.max(0, Math.min(100, o.goal > 0 ? o.progress / o.goal * 100 : 0));
  const selected = state.selectedShipId && state.ghosts.some((g) => g.id === state.selectedShipId && g.own);
  const actions: string[] = [];
  if ((o.state === "offered" || (o.state === "active" && !o.joined)) && !o.joined) {
    actions.push(`<button class="act" data-op="accept" data-id="${esc(o.id)}">Accept</button>`);
  }
  if (o.state === "active" && o.joined) {
    if (selected) actions.push(`<button class="act" data-op="assign" data-id="${esc(o.id)}">${o.assigned_fleet ? "Reassign selected fleet" : "Assign selected fleet"}</button>`);
    if (o.kind.kind === "syndicate_megaproject") {
      const projectSystem = o.kind.system;
      const hostOwned = state.systems.find((system) => system.id === projectSystem)?.owner === state.playerId;
      if (hostOwned) actions.push(`<input id="op-units-${esc(o.id)}" type="number" min="1" step="1" value="25" aria-label="Contribution units" /><button class="act" data-op="contribute" data-id="${esc(o.id)}">Commit local goods</button>`);
    }
    if (o.kind.kind !== "syndicate_megaproject") actions.push(`<button class="act" data-op="abandon" data-id="${esc(o.id)}">Abandon</button>`);
  }
  const until = Math.max(0, o.expires_at - liveSimTime());
  const reportAge = Math.max(0, liveSimTime() - o.reported_at);
  const hullArt = operationHullArt(o);
  const hull = hullArt
    ? `<img class="op-hull" src="${esc(hullArt)}" alt="" aria-hidden="true" />`
    : "";
  return `<article class="op-card${hullArt ? " has-hull" : ""} is-${o.state}">${hull}<div class="op-card-body"><div class="op-top"><span class="op-title">${operationIcon(o)}${esc(operationTitle(o))}</span><span class="op-state">${esc(o.state.replaceAll("_", " "))}</span></div>` +
    `<div class="op-copy">${esc(operationCopy(o))}</div><div class="op-progress"><i style="width:${pct.toFixed(1)}%"></i></div>` +
    `<div class="op-meta"><span>${o.progress}/${o.goal}</span><span>${esc(operationReward(o))}</span><span>${o.state === "completed" ? "complete" : `${fmtEta(until)} remaining`}</span><span>report ${fmtEta(reportAge)} old</span></div>` +
    (actions.length ? `<div class="op-actions">${actions.join("")}</div>` : "") + `</div></article>`;
}


export function updateOperationsPanel(): void {
  const root = $("operations-panel");
  if (!root.classList.contains("is-open")) return;
  const sig = JSON.stringify([state.operations, state.midgameStage, state.selectedShipId]);
  if (sig === lastOperationsSig && root.innerHTML) return;
  lastOperationsSig = sig;
  const [stage, copy] = MIDGAME_COPY[state.midgameStage];
  const available = state.operations.filter((o) => o.state === "offered" || (o.state === "active" && !o.joined));
  const active = state.operations.filter((o) => o.state === "active" && o.joined);
  const history = state.operations.filter((o) => !["offered", "active"].includes(o.state)).sort((a, b) => b.reported_at - a.reported_at).slice(0, 12);
  const group = (title: string, rows: typeof state.operations) => rows.length
    ? `<div class="op-sec">${esc(title)}</div>${rows.map(operationCard).join("")}`
    : title === "Available" ? `<div class="op-sec">Available</div><div class="op-empty">No arrived offers right now. The board refreshes as new reports reach your command center.</div>` : "";
  root.innerHTML = `<div class="op-head"><b>OPERATIONS</b><button class="op-close" data-op="close">✕</button></div><div class="op-body">` +
    `<div class="op-chapter"><b>${esc(stage)}</b><small>${esc(copy)}</small></div>` +
    group("Active", active) + group("Available", available) + group("History", history) + `</div>`;
}


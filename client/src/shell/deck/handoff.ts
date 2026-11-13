import { postVictoryHandoff, type HandoffGoal } from "../../core/derive/handoff";
import { operationReward, operationTitle } from "../../core/derive/format";
import { label } from "../../icons";
import { state } from "../../state";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

export function handoffHtml(): string {
  const goals = postVictoryHandoff();
  if (!goals.length) return "";
  return `<section class="deck-section deck-handoff"><header><div><h3>Your next chapter</h3></div><b>${goals.filter(g => g.done).length}/${goals.length}</b></header>`
    + `<div class="deck-handoff__goals">${goals.map(goalRow).join("")}</div></section>`;
}

/** §ops-compact (Peter, Oct 31 2026): one row per goal. Its checklist, hull
 * cost and reward ride the tooltip; a shortfall shows while the next step is
 * building the hull. */
function goalRow(g: HandoffGoal): string {
  const tip = [
    ...g.requirements.map(r => `${r.met ? "✓" : "○"} ${r.text}`),
    ...(g.costs.length ? [`Hull cost: ${g.costs.map(c => `${c.units} ${label(c.commodity)}`).join(" · ")}`] : []),
    `${g.id === "colony" ? "Reward" : "Gain"}: ${g.payoff}`,
  ].join("\n");
  const short = g.short.length
    ? ` · <span class="is-short" title="Local stock, docked Freighters included.">Short ${g.short.map(c => `${c.units} ${esc(label(c.commodity))}`).join(" · ")}</span>` : "";
  const more = [
    !g.done && g.prospect ? `<button type="button" data-deck-act="handoff-prospect" data-goal="${g.id}">Inspect ${esc(state.galaxy?.systems.find(s => s.id === g.prospect)?.name ?? "prospect")}</button>` : "",
    !g.done && g.funding ? `<button type="button" data-deck-act="handoff-contract" data-goal="${g.id}" title="${esc(`${operationReward(g.funding)} · ${g.funding.joined ? "active contract" : "view requirements"}`)}">Fund this · ${esc(operationTitle(g.funding))}</button>` : "",
  ].join("");
  return `<article class="deck-handoff__goal${g.done ? " is-done" : ""}" data-handoff-goal="${g.id}" title="${esc(tip)}">`
    + `<div><b>${esc(g.title)}</b><small>${esc(g.status)}${short}</small></div>`
    + `<button type="button" data-deck-act="handoff-open" data-goal="${g.id}" ${g.action ? "" : "disabled"}>${esc(g.actionLabel)}</button>`
    + (more ? `<div class="deck-handoff__more">${more}</div>` : "")
    + `</article>`;
}

interface HandoffHooks {
  go(route: DeckRoute): void;
  selectFleet(id: string): void;
  openWorld(system: string, body: number): void;
}

/** Navigation only: selecting a goal never accepts a contract, buys goods,
 * dispatches a fleet or substitutes a local reward/progress clock. */
export function handleHandoffAction(button: HTMLButtonElement, ctx: CoreContext, hooks: HandoffHooks): boolean {
  const act = button.dataset.deckAct;
  if (!act?.startsWith("handoff-")) return false;
  if (act === "handoff-details") { hooks.go({ name: "operations" }); return true; }
  const goal = postVictoryHandoff().find(g => g.id === button.dataset.goal);
  if (!goal) return true;
  if (act === "handoff-contract") {
    if (goal.funding) hooks.go({ name: "operations", query: { contract: goal.funding.id } });
    return true;
  }
  const a = act === "handoff-prospect" && goal.prospect ? { kind: "system" as const, id: goal.prospect }
    : act === "handoff-open" ? goal.action : null;
  if (!a) return true;
  if (a.kind === "fleet") { hooks.selectFleet(a.id); return true; }
  if (a.kind === "operations" || a.kind === "research") { hooks.go({ name: a.kind }); return true; }
  if (a.kind === "warehouse") { hooks.go({ name: "market", query: { tab: "warehouse" } }); return true; }
  const id = a.kind === "system" ? a.id : a.system;
  const system = ctx.state.galaxy?.systems.find(s => s.id === id);
  if (!system) return true;
  ctx.state.selectedSystemId = id;
  ctx.renderer.centerOnWorld(system.pos);
  ctx.renderer.stateVersion++;
  if (a.kind === "world") hooks.openWorld(id, a.body);
  else if (a.kind === "system") hooks.go({ name: "system", params: { id, systemLabel: system.name } });
  else hooks.go({ name: "build", params: { systemId: id, systemLabel: system.name },
    query: { body: String(a.body), mode: a.mode, select: a.select } });
  return true;
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

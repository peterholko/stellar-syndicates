import { postVictoryHandoff } from "../../core/derive/handoff";
import { operationReward, operationTitle } from "../../core/derive/format";
import { icon, label } from "../../icons";
import { state } from "../../state";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

export function handoffHtml(compact = false): string {
  const goals = postVictoryHandoff();
  if (!goals.length) return "";
  return `<section class="deck-section deck-handoff"><header><div><h3>Your next chapter</h3><p>Explore and strengthen your fleet.</p></div></header>
    <div class="deck-handoff__goals${compact ? " is-compact" : ""}">${goals.map(g => `<article class="deck-handoff__goal${g.done ? " is-done" : ""}" data-handoff-goal="${g.id}">
      <header>${icon(g.id === "explore" ? "scout" : "corvette", "lg")}<div><h4>${esc(g.title)}</h4><small>${esc(g.status)}</small></div></header>
      ${compact ? "" : `<p>${esc(g.summary)}</p><ul aria-label="Requirements">${g.requirements.map(r => `<li class="${r.met ? "is-met" : ""}"><span aria-label="${r.met ? "Met" : "Needed"}">${r.met ? "✓" : "○"}</span>${esc(r.text)}</li>`).join("")}</ul>
      ${g.costs.length ? `<table><caption>Hull costs · local stock</caption><thead><tr><th>Resource</th><th>Cost</th><th>Stock</th></tr></thead><tbody>${g.costs.map(c => `<tr><th>${esc(label(c.commodity))}</th><td>${c.units}</td><td class="${c.stock < c.units ? "is-short" : ""}">${c.stock}</td></tr>`).join("")}</tbody></table>` : ""}
      <p class="deck-handoff__payoff"><b>${g.id === "explore" ? "Reward" : "Gain"}</b> ${esc(g.payoff)}</p>`}
      <button type="button" data-deck-act="handoff-open" data-goal="${g.id}" ${g.action ? "" : "disabled"}>${esc(g.actionLabel)}</button>
      ${!compact && !g.done && g.prospect ? `<button type="button" data-deck-act="handoff-prospect" data-goal="${g.id}">Inspect ${esc(state.galaxy?.systems.find(s => s.id === g.prospect)?.name ?? "prospect")}</button>` : ""}
      ${!compact && !g.done && g.funding ? `<button type="button" class="deck-handoff__funding" data-deck-act="handoff-contract" data-goal="${g.id}"><span>Fund this · ${esc(operationTitle(g.funding))}</span><small>${esc(operationReward(g.funding))} · ${g.funding.joined ? "active contract" : "view requirements"}</small></button>` : ""}
    </article>`).join("")}</div>${compact ? `<button type="button" class="deck-section-link" data-deck-act="handoff-details">Requirements &amp; contract rewards</button>` : `<p class="deck-handoff__note">Contract rewards arrive with completion reports. Build costs use received local stock, including docked Freighters.</p>`}</section>`;
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

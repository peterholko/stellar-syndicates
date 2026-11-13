import { fmtHours, foundingPlan, type FoundingStep } from "../../core/derive/founding";
import { foundingHomeSystemId } from "../../core/derive/geo";
import type { WelcomeBackView, WelcomeGo, WelcomeLine } from "../../protocol";
import { requestTransactions } from "../../core/derive/transactions";
import { WELCOME_MIN_AWAY_S, welcomeReport, type WelcomeReport } from "../../core/derive/welcome";
import { setHtml } from "../dom";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

/** Receipts are fetched on demand; don't hold the report for a lost reply. */
const WAIT_FOR_RECEIPTS_MS = 4_000;

interface DeckWelcomeHooks {
  runFounding(): void;
  openLog(): void;
  go(route: DeckRoute): void;
}

/** §founding R11: the charter report on return, shown once per session when
 * the corporation was away long enough for its fleets to have done something. */
export class DeckWelcome {
  private phase: "waiting" | "loading" | "shown" | "done" = "waiting";
  private loadingSince = 0;

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: DeckWelcomeHooks,
    signal: AbortSignal,
  ) {
    root.addEventListener("click", (event) => {
      const act = (event.target as Element).closest<HTMLElement>("[data-deck-act]")?.dataset.deckAct;
      const button = (event.target as Element).closest<HTMLElement>("[data-deck-act]");
      if (act === "welcome-next") { this.close(); this.hooks.runFounding(); }
      else if (act === "welcome-go" && button) {
        const route = this.routeFor(button.dataset);
        this.close();
        if (route) this.hooks.go(route);
      } else if (act === "welcome-log") { this.close(); this.hooks.openLog(); }
      else if (act === "welcome-close") this.close();
    }, { signal });
  }

  render(): void {
    if (this.phase === "shown" || this.phase === "done") return;
    const st = this.ctx.state;
    if (!st.awaySet || st.playerId === null || !st.systems.length) return;
    if (st.welcome) {
      // §welcome (R9): the server's report; the founding guide still leads.
      const plan = foundingPlan(st, st.systems.find((system) => system.id === foundingHomeSystemId()));
      this.phase = "shown";
      setHtml(this.root, serverWelcomeHtml(st.welcome, plan?.progress ?? [], plan?.primary ?? null));
      this.root.hidden = false;
      return;
    }
    if (st.awaySince <= 0 || st.simTime - st.awaySince < WELCOME_MIN_AWAY_S) {
      this.phase = "done";
      return;
    }
    if (this.phase === "waiting") {
      requestTransactions(this.ctx, "initial");
      this.phase = "loading";
      this.loadingSince = Date.now();
    }
    if (!st.transactions.loaded && Date.now() - this.loadingSince < WAIT_FOR_RECEIPTS_MS) return;
    const homeId = foundingHomeSystemId();
    const report = welcomeReport(st, st.systems.find((system) => system.id === homeId));
    if (!report) {
      this.phase = "done";
      return;
    }
    this.phase = "shown";
    setHtml(this.root, welcomeHtml(report));
    this.root.hidden = false;
  }

  teardown(): void {
    this.root.hidden = true;
    this.root.replaceChildren();
  }

  private close(): void {
    this.phase = "done";
    this.teardown();
  }

  private routeFor(data: DOMStringMap): DeckRoute | null {
    const label = (id: string) => this.ctx.state.galaxy?.systems.find((entry) => entry.id === id)?.name ?? "System";
    switch (data.go) {
      case "system": return data.id ? { name: "system", params: { id: data.id, systemLabel: label(data.id) } } : null;
      case "shipyard": return data.id ? { name: "shipyard", params: { id: data.id, systemLabel: label(data.id) } } : null;
      case "signals": case "operations": case "research": case "comms": case "market": case "rankings": case "syndicate": case "log":
        return { name: data.go, ...(data.go === "comms" ? { query: { tab: "mail" } } : {}) };
      default: return null;
    }
  }
}

/** The data attributes that carry a `WelcomeGo` to the click handler. */
function goAttrs(go: WelcomeGo): string {
  const id = go.kind === "system" ? go.id : go.kind === "shipyard" ? go.system : "";
  return `data-deck-act="welcome-go" data-go="${go.kind}"${id ? ` data-id="${esc(id)}"` : ""}`;
}

function lineHtml(line: WelcomeLine): string {
  const text = esc(line.text);
  return `<li class="is-${line.severity}">${line.go ? `<button type="button" class="deck-welcome__line" ${goAttrs(line.go)}>${text}</button>` : text}</li>`;
}

/** §welcome (R9): the server's report, with the founding guide's progress
 * and step while the charter runs. */
export function serverWelcomeHtml(report: WelcomeBackView, underway: string[], step: FoundingStep | null): string {
  const lines = (items: WelcomeLine[]) => `<ul>${items.map(lineHtml).join("")}</ul>`;
  const actions = report.next.slice(0, step ? 2 : 3).map((action, i) =>
    `<button type="button"${!step && i === 0 ? ` class="is-primary"` : ""} ${goAttrs(action.go)}>${esc(action.label)}</button>`).join("");
  const stepButton = step ? `<button type="button" class="is-primary" data-deck-act="welcome-next">${esc(step.alternatives?.length ? "Choose in the guide" : step.label)}</button>` : "";
  return `<header><span>Welcome back · away ${esc(fmtHours(report.away))}</span><button type="button" data-deck-act="welcome-close" aria-label="Close report">✕</button></header>` +
    `<div class="deck-welcome__body">` +
    (report.happened.length ? `<section><small>While you were away</small>${lines(report.happened)}</section>` : "") +
    (underway.length ? `<section><small>Under way</small><ul>${underway.map((item) => `<li>${esc(item)}</li>`).join("")}</ul></section>` : "") +
    (report.waiting.length ? `<section><small>Waiting</small>${lines(report.waiting)}</section>` : "") +
    (step ? `<section class="deck-welcome__next"><small>Next</small><b>${esc(step.status ?? step.title)}</b>${step.reason || step.why || step.copy ? `<p>${esc(step.reason || step.why || step.copy)}</p>` : ""}</section>` : "") +
    `<footer>${stepButton}${actions}<button type="button" data-deck-act="welcome-log">Open Log</button></footer></div>`;
}

function welcomeHtml(report: WelcomeReport): string {
  const list = (items: string[]) => `<ul>${items.map((item) => `<li>${esc(item)}</li>`).join("")}</ul>`;
  const next = report.next;
  return `<header><span>Welcome back · away ${esc(fmtHours(report.away))}</span><button type="button" data-deck-act="welcome-close" aria-label="Close report">✕</button></header>` +
    `<div class="deck-welcome__body">` +
    (report.happened.length ? `<section><small>While you were away</small>${list(report.happened)}${report.partial ? `<p>Older receipts are in Market Hub › Transactions.</p>` : ""}</section>` : "") +
    (report.underway.length ? `<section><small>Under way</small>${list(report.underway)}</section>` : "") +
    (next ? `<section class="deck-welcome__next"><small>Next</small><b>${esc(next.status ?? next.title)}</b>${next.reason || next.why || next.copy ? `<p>${esc(next.reason || next.why || next.copy)}</p>` : ""}</section>` : "") +
    `<footer>${next ? `<button type="button" class="is-primary" data-deck-act="welcome-next">${esc(next.alternatives?.length ? "Choose in the guide" : next.label)}</button>` : ""}` +
    `<button type="button" data-deck-act="welcome-log">Open Log</button></footer></div>`;
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

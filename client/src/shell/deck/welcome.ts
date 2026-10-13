import { fmtHours } from "../../core/derive/founding";
import { foundingHomeSystemId } from "../../core/derive/geo";
import { requestTransactions } from "../../core/derive/transactions";
import { WELCOME_MIN_AWAY_S, welcomeReport, type WelcomeReport } from "../../core/derive/welcome";
import { setHtml } from "../dom";
import type { CoreContext } from "../types";

/** Receipts are fetched on demand; don't hold the report for a lost reply. */
const WAIT_FOR_RECEIPTS_MS = 4_000;

interface DeckWelcomeHooks {
  runFounding(): void;
  openLog(): void;
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
      if (act === "welcome-next") { this.close(); this.hooks.runFounding(); }
      else if (act === "welcome-log") { this.close(); this.hooks.openLog(); }
      else if (act === "welcome-close") this.close();
    }, { signal });
  }

  render(): void {
    if (this.phase === "shown" || this.phase === "done") return;
    const st = this.ctx.state;
    if (!st.awaySet || st.playerId === null || !st.systems.length) return;
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
}

function welcomeHtml(report: WelcomeReport): string {
  const list = (items: string[]) => `<ul>${items.map((item) => `<li>${esc(item)}</li>`).join("")}</ul>`;
  const next = report.next;
  return `<header><span>Welcome back · away ${esc(fmtHours(report.away))}</span><button type="button" data-deck-act="welcome-close" aria-label="Close report">✕</button></header>` +
    `<div class="deck-welcome__body">` +
    (report.happened.length ? `<section><small>While you were away</small>${list(report.happened)}${report.partial ? `<p>Older receipts are in Market Hub › Transactions.</p>` : ""}</section>` : "") +
    (report.underway.length ? `<section><small>Under way</small>${list(report.underway)}</section>` : "") +
    (next ? `<section class="deck-welcome__next"><small>Next</small><b>${esc(next.title)}</b><p>${esc(next.reason ?? next.copy)}</p></section>` : "") +
    `<footer>${next ? `<button type="button" class="is-primary" data-deck-act="welcome-next">${esc(next.alternatives?.length ? "Choose in the guide" : next.label)}</button>` : ""}` +
    `<button type="button" data-deck-act="welcome-log">Open Log</button></footer></div>`;
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

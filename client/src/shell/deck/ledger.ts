import type { ViewState } from "../../state";
import { serviceReward } from "../../core/derive/board";

/** §founding R10/R12: the charter ledger on Operations. Three Authority goals
 * at a time, each with real progress from served state and its one-time stipend.
 * The syndicate step leads the ledger and lists waiting invitations. */
export function charterLedgerHtml(st: ViewState): string {
  const ledger = st.charterLedger;
  if (!ledger || !ledger.goals.length) return "";
  const invites = st.syndicateInvites.length;
  // §ops-compact (Oct 31 2026): one row per goal; its sentence is the tooltip.
  const goals = ledger.goals.map((goal) => {
    const pct = Math.min(100, goal.target > 0 ? (goal.progress / goal.target) * 100 : 0);
    const count = (n: number) => Math.floor(n).toLocaleString();
    const open = goal.metric === "syndicate" ? ["syndicate", "Open Syndicate"]
      : goal.metric === "limit_orders" ? ["market", "Open Market Hub"]
      : goal.metric === "research" ? ["research", "Open Research"] : null;
    const waiting = goal.metric === "syndicate" && invites ? ` · <b>${invites} invitation${invites === 1 ? "" : "s"} waiting</b>` : "";
    return `<article class="deck-charter-ledger__goal" title="${esc(goal.detail)}"><div><b>${esc(goal.title)}</b>` +
      `<small>${count(goal.progress)}/${count(goal.target)}<em>${esc(serviceReward(goal.standing, goal.research))}</em>${waiting}</small></div>` +
      (open ? `<button type="button" data-deck-act="ledger-open" data-route="${open[0]}">${open[1]}</button>` : "") +
      `<div class="deck-charter-ledger__bar" role="progressbar" aria-label="${esc(goal.title)}" aria-valuemin="0" aria-valuemax="${goal.target}" aria-valuenow="${Math.floor(goal.progress)}"><i style="width:${pct.toFixed(1)}%"></i></div></article>`;
  }).join("");
  return `<section class="deck-section deck-charter-ledger"><header><div><h3 title="Authority goals. Each pays once, in any order.">Charter ledger</h3></div><b>${ledger.done}/${ledger.total}</b></header><div class="deck-charter-ledger__goals">${goals}</div></section>`;
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

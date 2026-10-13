import type { ViewState } from "../../state";

/** §founding R10/R12: the charter ledger on Command. Three Authority goals at
 * a time, each with real progress from served state and its one-time stipend.
 * The syndicate step leads the ledger and lists waiting invitations. */
export function charterLedgerHtml(st: ViewState): string {
  const ledger = st.charterLedger;
  if (!ledger || !ledger.goals.length) return "";
  const invites = st.syndicateInvites.length;
  const goals = ledger.goals.map((goal) => {
    const pct = Math.min(100, goal.target > 0 ? (goal.progress / goal.target) * 100 : 0);
    const count = (n: number) => Math.floor(n).toLocaleString();
    const action = goal.metric === "syndicate"
      ? `<button type="button" data-deck-act="ledger-open" data-route="syndicate">${invites ? `${invites} invitation${invites === 1 ? "" : "s"} waiting · Open Syndicate` : "Open Syndicate"}</button>`
      : goal.metric === "limit_orders" ? `<button type="button" data-deck-act="ledger-open" data-route="market">Open Market Hub</button>`
      : goal.metric === "contracts" ? `<button type="button" data-deck-act="ledger-open" data-route="operations">Open Operations</button>`
      : goal.metric === "research" ? `<button type="button" data-deck-act="ledger-open" data-route="research">Open Research</button>`
      : "";
    return `<article class="deck-charter-ledger__goal"><header><b>${esc(goal.title)}</b><em>+${goal.reward.toLocaleString()} Cr</em></header>` +
      `<p>${esc(goal.detail)}</p>` +
      `<div class="deck-charter-ledger__bar" role="progressbar" aria-label="${esc(goal.title)}" aria-valuemin="0" aria-valuemax="${goal.target}" aria-valuenow="${Math.floor(goal.progress)}"><i style="width:${pct.toFixed(1)}%"></i></div>` +
      `<small>${count(goal.progress)} / ${count(goal.target)}</small>${action}</article>`;
  }).join("");
  return `<section class="deck-section deck-charter-ledger"><header><div><h3>Charter ledger</h3><p>Authority goals for a growing charter. Each pays once, in any order.</p></div><b>${ledger.done}/${ledger.total}</b></header><div class="deck-charter-ledger__goals">${goals}</div></section>`;
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

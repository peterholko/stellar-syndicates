import { battleCommandDelay, latestPendingOrder } from "../core/derive/orders";
import { shipKindLabel } from "../core/derive/fleet";
import { liveSimTime } from "../state";
import type { CoreContext } from "./types";

/** Shared desktop/mobile preflight. This is local confirmation state only:
 * no order exists until Confirm is pressed, and no timer grants withdrawal.
 * Revalidate the arrived battle/owner/participant picture on every press. */
export class BattleWithdrawPrompt {
  private pending: { battle: string; fleet: string } | null = null;

  clear(): void { this.pending = null; }

  handle(action: string, battle: string, fleet: string, ctx: CoreContext): string | null {
    if (action === "cancel") { this.clear(); return null; }
    const option = this.options(battle, ctx).find(o => o.fleet.id === fleet);
    if (!option || option.ordered) { this.clear(); return null; }
    if (action === "ask") { this.pending = { battle, fleet }; return null; }
    if (action !== "confirm" || this.pending?.battle !== battle || this.pending.fleet !== fleet) return null;
    // Clear before dispatch so a repeated click on the removed Confirm button
    // cannot send a second command. This still uses the normal delayed queue.
    this.clear();
    ctx.send({ type: "Withdraw", fleet_id: fleet });
    return `Withdraw order sent · ${this.delayLabel(option.delay)} command delay.`;
  }

  html(battle: string, ctx: CoreContext, attribute: "data-deck-theater-act" | "data-deck-act" | "data-mobile-act", prefix: string): string {
    const options = this.options(battle, ctx);
    if (this.pending?.battle === battle && !options.some(o => o.fleet.id === this.pending?.fleet && !o.ordered)) this.clear();
    const button = (action: string, fleet: string, text: string, extra = "") =>
      `<button type="button" ${attribute}="${prefix}-${action}" data-battle="${esc(battle)}" data-fleet="${esc(fleet)}" ${extra}>${esc(text)}</button>`;
    return options.map(o => {
      const name = shipKindLabel(o.fleet.kind);
      const delay = this.delayLabel(o.delay);
      if (this.pending?.battle === battle && this.pending.fleet === o.fleet.id) {
        return `<div class="battle-withdraw-confirm" role="group" aria-label="Confirm withdrawal for ${esc(name)}">
          <span>${o.delay === null ? "Command delay unavailable." : `The order will take about ${Math.ceil(o.delay)} seconds to reach this fleet.`}</span>
          <div>${button("confirm", o.fleet.id, `Confirm withdraw ${name} · ${delay}`, 'class="is-primary m-primary"')}${button("cancel", o.fleet.id, "Cancel")}</div></div>`;
      }
      const timing = o.ordered ? liveSimTime() < o.ordered.arrives_at
        ? `Withdraw sent · ~${Math.ceil(o.ordered.arrives_at - liveSimTime())}s remaining`
        : "Withdraw sent · awaiting response" : `Withdraw ${name} · ${delay}`;
      return button("ask", o.fleet.id, timing, o.ordered ? "disabled" : "");
    }).join("");
  }

  private options(id: string, ctx: CoreContext) {
    const battle = ctx.state.battles.find(b => b.id === id && b.own);
    const record = ctx.state.battleRecords.find(r => r.id === id);
    // Final evidence can arrive before the last live animation finishes, or
    // before the battle View is removed. Neither permits a new withdrawal.
    if (!battle || (record && record.outcome !== null)) return [];
    const delay = battleCommandDelay(battle);
    return ctx.state.ghosts.filter(g => g.own && battle.participants.includes(g.id)).map(fleet => {
      const pending = latestPendingOrder(fleet.id);
      return { fleet, delay, ordered: pending?.kind === "withdraw" ? pending : null };
    });
  }

  private delayLabel(delay: number | null): string { return delay === null ? "delay unknown" : `~${Math.ceil(delay)}s`; }
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

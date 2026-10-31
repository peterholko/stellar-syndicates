// §merchant: the traveling merchant's stall, a Market-style pop-out
// (docs/traveling-merchant.md). One exchange per deal: buys from its stock,
// Ferrite Ore to it, settled at your home with no light delay. §persona (Oct
// 16 2026): a negotiation with the trader (§traders: one of three, dealt per
// corporation), who talks about the offer on the table; the prices are the
// sim's.
import { fmt, fmtDur } from "../../core/derive/format";
import { spendableMarketCredits } from "../../core/derive/market";
import {
  dockedMerchant, isMerchantErrand, isMerchantTarget, merchantErrand, merchantLeavesIn, merchantQuote, merchantShortfall,
  oreAtHome, type MerchantGoods,
} from "../../core/derive/merchant";
import { fuelRoom } from "../../core/derive/founding";
import { dealLine, refusalLine, traderLine, traderOf, traderPortrait, traderResult, type TraderLine } from "../../core/derive/trader";
import { commodityIcon, label } from "../../icons";
import type { Commodity, SystemStateView } from "../../protocol";
import { liveSimTime } from "../../state";
import { lastHtmlWritten, renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

interface MerchantHooks {
  go(route: DeckRoute): void;
  notice(html: string): void;
}


export class DeckMerchantRoutes {
  private buys: MerchantGoods = {};
  private sells: MerchantGoods = {};
  /** When the last deal was sent (sim time), until the amounts change. */
  private sentAt: number | null = null;
  private appliedTarget = "";
  private signature = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: MerchantHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "merchant") return false;
    this.applyRouteTarget(route);
    const st = this.ctx.state;
    const home = this.home();
    const signature = sheetFingerprint([
      route, this.buys, this.sells, this.sentAt, Math.floor(liveSimTime()), st.merchant, st.wallet, st.timeline.length,
      home ? [home.stockpile, home.storage_used, home.storage_cap] : null,
    ]);
    if (!force && signature === this.signature) return true;
    const active = document.activeElement;
    if (!force && active && this.root.contains(active) && active instanceof HTMLInputElement) {
      this.patchOreAtHome();
      return true;
    }
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, this.html());
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "merchant") return false;
    const act = button.dataset.deckAct;
    if (act === "merchant-max") {
      // All the ore you hold, up to what the trader takes.
      const merchant = dockedMerchant(this.ctx.state);
      const units = merchant ? Math.min(merchant.ore_room, oreAtHome(this.home())) : 0;
      if (units > 0) this.sells.metallic_ore = units; else delete this.sells.metallic_ore;
      this.sentAt = null;
      this.signature = "";
      this.render(route, true);
      // A box you typed in keeps its own value through a redraw: put Max in it.
      const box = this.root.querySelector<HTMLInputElement>('[data-merchant-input="sell:metallic_ore"]');
      if (box) box.value = units > 0 ? String(units) : "";
      return true;
    }
    if (act !== "merchant-submit") return false;
    this.submit();
    this.signature = "";
    this.render(route, true);
    return true;
  }

  handleInput(target: HTMLInputElement | HTMLSelectElement, route: DeckRoute | null): boolean {
    if (route?.name !== "merchant") return false;
    const [side, commodity] = (target.dataset.merchantInput ?? "").split(":") as [string, Commodity];
    if ((side !== "buy" && side !== "sell") || !commodity) return false;
    const units = Math.max(0, Math.floor(Number(target.value) || 0));
    const book = side === "buy" ? this.buys : this.sells;
    if (units > 0) book[commodity] = units; else delete book[commodity];
    this.sentAt = null;
    this.signature = "";
    this.render(route, true);
    return true;
  }

  invalidate(): void {
    this.signature = "";
  }

  /** The table isn't redrawn while an amount box has the cursor; the ore at
   * home still ticks up as the mines work. */
  private patchOreAtHome(): void {
    const merchant = this.ctx.state.merchant;
    const held = oreAtHome(this.home());
    const count = this.root.querySelector(".deck-merchant-row__held b");
    if (!merchant || !count || count.textContent === fmt(held)) return;
    count.textContent = fmt(held);
    const box = this.root.querySelector<HTMLInputElement>('[data-merchant-input="sell:metallic_ore"]');
    if (box) box.max = String(Math.min(merchant.ore_room, held));
    const most = this.root.querySelector<HTMLButtonElement>('[data-deck-act="merchant-max"]');
    if (most) most.disabled = merchant.phase !== "docked" || Math.min(merchant.ore_room, held) <= 0;
    // The DOM no longer matches the last html written: the next redraw must
    // reconcile, even if the count is back where it was.
    lastHtmlWritten.delete(this.root);
  }

  private home(): SystemStateView | undefined {
    const system = this.ctx.state.merchant?.system;
    return system ? this.ctx.state.systems.find((s) => s.id === system) : undefined;
  }

  private credits(): number | undefined {
    // §treasury: the credits sit at home, known even before the Hub's report.
    return this.ctx.state.credits !== null ? spendableMarketCredits() : undefined;
  }

  /** A guide shortcut opens the table with that build's shortfall, or the
   * errand's goods (§charter-v5 "Sell ore", "Buy Deuterium"), on it. */
  private applyRouteTarget(route: DeckRoute): void {
    const target = route.query?.for ?? "";
    if (target === this.appliedTarget) return;
    this.appliedTarget = target;
    const merchant = dockedMerchant(this.ctx.state);
    if (merchant && isMerchantErrand(target)) {
      ({ buys: this.buys, sells: this.sells } = merchantErrand(target, this.home(), merchant, fuelRoom(this.ctx.state)));
      this.sentAt = null;
    } else if (merchant && isMerchantTarget(target)) {
      this.buys = merchantShortfall(this.ctx.state, this.home(), target, merchant);
      this.sells = {};
      this.sentAt = null;
    }
  }

  private submit(): void {
    const merchant = dockedMerchant(this.ctx.state);
    if (!merchant) return;
    const quote = merchantQuote(merchant, this.buys, this.sells, this.home(), this.credits());
    if (quote.error) return;
    this.ctx.send({ type: "MerchantTrade", buys: { ...this.buys }, sells: { ...this.sells } });
    const net = quote.cost - quote.revenue;
    this.hooks.notice(`<b>Deal sent</b> · ${net >= 0 ? `you pay ${fmt(net)} Cr` : `you receive ${fmt(-net)} Cr`}.`);
    this.sentAt = this.ctx.state.simTime;
    this.buys = {};
    this.sells = {};
  }

  /** What the trader says: about the deal just sent (a parting shot, or why
   * the sim refused it), else about the offer on the table. */
  private line(dealt: boolean): TraderLine {
    const merchant = this.ctx.state.merchant!;
    const trader = traderOf(this.ctx.state.founding);
    if (this.sentAt !== null) {
      const result = traderResult(this.ctx.state.timeline, this.sentAt);
      return result && !result.ok ? refusalLine(trader, result.reason) : dealLine(trader);
    }
    const quote = merchantQuote(merchant, this.buys, this.sells, this.home(), this.credits());
    return traderLine(trader, merchant, this.buys, this.sells, quote, dealt, !!this.ctx.state.founding?.bounty_received);
  }

  private html(): string {
    const st = this.ctx.state;
    const merchant = st.merchant;
    if (!merchant) {
      return `<section class="deck-page deck-merchant"><div class="deck-empty-inline">No merchant at your home.</div></section>`;
    }
    const docked = merchant.phase === "docked";
    const home = this.home();
    // Its markup, and how its prices compare with the Hub's, are the trader's
    // own business: the table shows only its prices.
    const quote = merchantQuote(merchant, this.buys, this.sells, home, this.credits());
    const dealt = st.timeline.some((e) => e.text.startsWith("Free trader: ") && !e.text.startsWith("Free trader: trade refused"));
    const says = this.line(dealt);
    const who = traderOf(st.founding);
    const portrait = traderPortrait(who, says.mood, () => { this.signature = ""; });
    const status = merchant.phase === "arriving" ? "Arriving" : merchant.phase === "leaving" ? "Leaving"
      : `Leaves in ${fmtDur(merchantLeavesIn(merchant, liveSimTime()))}`;
    const trader = `<header class="deck-merchant__trader" data-mood="${says.mood}">`
      + `<img class="deck-merchant__portrait${portrait.painted ? "" : " is-interim"}" src="${esc(portrait.src)}" alt="${esc(portrait.alt)}" width="210" height="210">`
      + `<div class="deck-merchant__who"><b>${esc(who.name)}</b><small>the ${esc(who.ship)} · ${esc(status)}</small></div>`
      + `<p class="deck-merchant__says" role="status">“${esc(says.text)}”</p></header>`;
    const held = oreAtHome(home);
    const give = row("metallic_ore", `${merchant.ore_price.toFixed(1)} Cr · takes ${merchant.ore_room} more`,
      "sell", this.sells.metallic_ore ?? 0, Math.min(merchant.ore_room, held), docked, held);
    const get = merchant.goods.map((good) => row(good.commodity, `${good.price.toFixed(1)} Cr · ${good.stock} left`,
      "buy", this.buys[good.commodity] ?? 0, good.stock, docked)).join("");
    const table = `<div class="deck-merchant__table">`
      + `<section class="deck-merchant__side" aria-label="You give"><h3>You give</h3>${give}</section>`
      + `<section class="deck-merchant__side" aria-label="You get"><h3>You get</h3>${get}</section></div>`;
    const net = quote.cost - quote.revenue;
    const empty = !quote.cost && !quote.revenue;
    const terms = empty ? "Name your terms."
      : `${net > 0 ? `You pay <b>${fmt(net)} Cr</b>` : net < 0 ? `You receive <b>${fmt(-net)} Cr</b>` : "Even trade"}`
        + `${quote.netUnits ? ` · storage ${quote.netUnits > 0 ? "+" : ""}${quote.netUnits}` : ""}`;
    const canSubmit = docked && !empty && !quote.error;
    // The terms and the handshake stay pinned to the panel's foot.
    const dock = `<div class="deck-merchant__dock"><div class="deck-ticket-quote">${terms}</div>` +
      `<button type="button" class="is-primary" data-deck-act="merchant-submit" title="Settles at your home at once" ${canSubmit ? "" : "disabled"}>Trade</button></div>`;
    return `<section class="deck-page deck-merchant">${trader}${table}${dock}</section>`;
  }
}

/** `held`: your side's stock at home, on a line of its own, with Max (all
 * of it the trader takes) under the amount. */
function row(commodity: Commodity, detail: string, side: "buy" | "sell", value: number, max: number, enabled: boolean, held?: number): string {
  const home = held === undefined ? "" : `<small class="deck-merchant-row__held"><b>${fmt(held)}</b> at home</small>`;
  const amount = `<input type="number" min="0" max="${max}" step="1" value="${value || ""}" placeholder="0" aria-label="${esc(`${side === "buy" ? "Buy" : "Sell"} ${label(commodity)}`)}" data-merchant-input="${side}:${commodity}" ${enabled ? "" : "disabled"}>`;
  const most = held === undefined ? amount : `<span class="deck-merchant-row__amount">${amount}`
    + `<button type="button" data-deck-act="merchant-max" title="All you hold, up to what it takes" ${enabled && max > 0 ? "" : "disabled"}>Max</button></span>`;
  return `<div class="deck-merchant-row">${commodityIcon(commodity)}<span><b>${esc(label(commodity))}</b><small>${esc(detail)}</small>${home}</span>${most}</div>`;
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);
}

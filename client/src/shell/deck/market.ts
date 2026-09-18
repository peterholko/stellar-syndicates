import { hubDockedFleets, shipKindLabel } from "../../core/derive/fleet";
import { agoLabel, fmt, fmtDur, informationDelay, rejectText, trend } from "../../core/derive/format";
import { systemName } from "../../core/derive/geo";
import {
  COMMODITIES,
  commodityPurpose,
  freightDraft,
  freightDraftEntries,
  marketAverageQuote,
  marketReservations,
  moduleRecipeValue,
  pruneMarketReservations,
  recentMarketOrders,
  recordRecentMarketOrder,
  reservedMarketCredits,
  reserveMarketOrder,
  settleMarketReservation,
  spendableMarketCredits,
  warehouseUnits,
} from "../../core/derive/market";
import { projectedBand } from "../../core/derive/research";
import { requestTransactions } from "../../core/derive/transactions";
import type { CoreEvent } from "../../core/events";
import { commodityIcon, icon, label, type IconKey } from "../../icons";
import {
  countClassLabel,
  fleetCargoManifest,
  fleetExactCount,
  freightFee,
  type Commodity,
  type EntityId,
  type ShipmentDir,
  type Side,
  type TradeEvent,
} from "../../protocol";
import { liveSimTime, state } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";
import { transactionsHtml } from "../transactions";

export type DeckMarketTab = "exchange" | "warehouse" | "specialists" | "modules" | "transactions";

export interface DeckTradeNotice {
  title: string;
  message: string;
  tone: "quiet" | "good" | "warn" | "bad";
  destination: DeckRoute;
}

interface MarketHooks {
  go(route: DeckRoute): void;
  notice(html: string): void;
}

const MARKET_PROTECTION_FRAC = 0.10;
const MODULE_BUY_MULT = 2.0;
const MODULE_SELL_MULT = 0.5;

const SPECIALISTS: { slug: string; icon: IconKey; blurb: string }[] = [
  { slug: "geologist", icon: "extractor", blurb: "mineral extraction" },
  { slug: "petrochemical_engineer", icon: "refinery", blurb: "volatiles, fuel and chemicals" },
  { slug: "xenobiologist", icon: "provisions", blurb: "biomass and agroplex production" },
  { slug: "industrial_engineer", icon: "build", blurb: "heavy industry" },
  { slug: "naval_architect", icon: "shipyard", blurb: "shipyards, hulls, drives and armaments" },
];

import { MODULES, isBlueprintOnly } from "../../core/derive/equipment";

/** The routed Market is one served picture: the board, ticket, warehouse,
 * contracts and freight desk all read the same delayed View. Local reservations
 * pessimistically cover commands whose Market Hub receipt has not returned yet;
 * they never pretend that a client estimate is an execution. */
export class DeckMarketRoutes {
  private tab: DeckMarketTab = "exchange";
  private side: Side = "buy";
  private commodity: Commodity = "fuel";
  private quantity = 50;
  private limitOn = false;
  private limitPrice = 0;
  private freightDirection: ShipmentDir = "outbound";
  private freightSystem = "";
  private freightCommodity: Commodity = "fuel";
  private freightQuantity = 100;
  private freightSellOnArrival = false;
  private feedback = "";
  private appliedRouteTab = "";
  private signature = "";
  private reportReadiness = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: MarketHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "market") return false;
    this.applyRouteTab(route);
    if (this.tab === "transactions") requestTransactions(this.ctx);
    this.repairFreightSystem();
    const state = this.ctx.state;
    const readiness = `${!!this.accountReport()}:${!!state.market?.prices.length}`;
    const signature = sheetFingerprint([
      route, this.tab, this.side, this.commodity, this.quantity, this.limitOn, this.limitPrice,
      this.freightDirection, this.freightSystem, this.freightCommodity, this.freightQuantity,
      this.freightSellOnArrival, freightDraftEntries(), this.feedback, Math.floor(liveSimTime()),
      state.market, state.wallet, state.charter, state.freight,
      state.priceHistory, state.systems.map((system) => [system.id, system.owner, system.stockpile, system.storage_used, system.storage_cap, system.modules]),
      hubDockedFleets().map((fleet) => [fleet.id, fleet.kind, fleet.count_class, fleet.composition, fleet.cargo, fleet.cargo_manifest, state.pendingOrders.get(fleet.id)]),
      marketReservations, recentMarketOrders, state.transactions,
    ]);
    if (!force && signature === this.signature) return true;
    const active = document.activeElement;
    // Receiving the first report must unlock the panel even while the player
    // prepares a quantity. setHtml reconciles the input without losing the edit.
    if (!force && readiness === this.reportReadiness && active && this.root.contains(active) && (active instanceof HTMLInputElement || active instanceof HTMLSelectElement)) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    this.reportReadiness = readiness;
    setHtml(this.root, this.marketHtml());
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "market") return false;
    const action = button.dataset.deckAct;
    if (!this.accountReport() && ["market-submit", "market-cancel", "market-hire", "market-module-buy", "market-module-sell", "freight-submit"].includes(action ?? "")) {
      this.render(route, true);
      return true;
    }
    if (action === "market-tab") {
      const tab = button.dataset.tab;
      if (isMarketTab(tab)) {
        this.tab = tab;
        // A receipt may have opened a specific route tab. Honour this local
        // click rather than immediately reapplying that older route parameter.
        this.appliedRouteTab = route.query?.tab ?? "";
      }
    } else if (action === "transactions-older" || action === "transactions-newer" || action === "transactions-latest") {
      requestTransactions(this.ctx, action === "transactions-older" ? "older" : action === "transactions-newer" ? "newer" : "latest");
    } else if (action === "market-commodity") {
      if (isCommodity(button.dataset.commodity)) this.commodity = button.dataset.commodity;
    } else if (action === "market-side") {
      if (button.dataset.side === "buy" || button.dataset.side === "sell") this.side = button.dataset.side;
    } else if (action === "market-submit") {
      this.submitTrade();
    } else if (action === "market-cancel") {
      const id = Number(button.dataset.order);
      if (Number.isFinite(id)) {
        this.ctx.send({ type: "CancelLimitOrder", order_id: id });
        this.feedback = "Cancellation sent · awaiting the Market Hub account report.";
        this.hooks.notice(`<b>Cancellation sent</b> · awaiting delayed Market Hub light.`);
      }
    } else if (action === "market-hire") {
      this.hireSpecialist(button.dataset.specialist);
    } else if (action === "market-module-buy" || action === "market-module-sell") {
      this.tradeModule(button.dataset.module, action === "market-module-buy");
    } else if (action === "market-hub-fleet") {
      const id = button.dataset.fleet;
      const fleet = id ? hubDockedFleets().find((candidate) => candidate.id === id) : undefined;
      if (fleet) this.hooks.go({ name: "fleet", params: { id: fleet.id, fleetLabel: shipKindLabel(fleet.kind) } });
    } else if (action === "market-hub-unload") {
      const id = button.dataset.fleet;
      if (id && hubDockedFleets().some((fleet) => fleet.id === id)) {
        this.ctx.intent.beginFleetCommand({ type: "HubUnload", fleet_id: id });
      }
    } else if (action === "freight-direction") {
      const direction = button.dataset.direction;
      if ((direction === "outbound" || direction === "inbound") && direction !== this.freightDirection) {
        this.freightDirection = direction;
        freightDraft.clear();
      }
    } else if (action === "freight-add") {
      freightDraft.set(this.freightCommodity, Math.max(1, Math.floor(this.freightQuantity)));
    } else if (action === "freight-remove") {
      if (isCommodity(button.dataset.commodity)) freightDraft.delete(button.dataset.commodity);
    } else if (action === "freight-submit") {
      this.submitFreight();
    } else {
      return false;
    }
    this.signature = "";
    this.render(route, true);
    return true;
  }

  handleInput(target: HTMLInputElement | HTMLSelectElement, route: DeckRoute | null): boolean {
    if (route?.name !== "market") return false;
    const field = target.dataset.marketInput;
    if (!field) return false;
    if (field === "trade-quantity") this.quantity = positiveInteger(target.value, this.quantity);
    else if (field === "trade-limit") this.limitPrice = Math.max(0, Number(target.value) || 0);
    else if (field === "trade-limit-on" && target instanceof HTMLInputElement) this.limitOn = target.checked;
    else if (field === "freight-system") {
      this.freightSystem = target.value;
      freightDraft.clear();
    } else if (field === "freight-commodity" && isCommodity(target.value)) this.freightCommodity = target.value;
    else if (field === "freight-quantity") this.freightQuantity = positiveInteger(target.value, this.freightQuantity);
    else if (field === "freight-sell" && target instanceof HTMLInputElement) this.freightSellOnArrival = target.checked;
    else return false;
    this.signature = "";
    this.render(route, true);
    return true;
  }

  onCore(events: readonly CoreEvent[], route: DeckRoute | null): void {
    let changed = false;
    for (const event of events) {
      if (event.kind === "TransactionsApplied") { changed = true; continue; }
      if (event.kind !== "TradeSettled") continue;
      // The delayed Trade event is the sole receipt: release exactly one matching
      // local reservation and only then promote an execution into Recent Orders.
      settleMarketReservation(event.trade);
      recordRecentMarketOrder(event.trade);
      const notice = deckTradeNotice(event.trade);
      this.feedback = notice.message;
      changed = true;
    }
    if (!changed) return;
    this.signature = "";
    if (route?.name === "market") this.render(route, true);
  }

  invalidate(): void {
    this.signature = "";
  }

  private accountReport() {
    const wallet = this.ctx.state.wallet;
    return wallet && !wallet.report_pending ? wallet : null;
  }

  private marketHtml(): string {
    const state = this.ctx.state;
    const account = this.accountReport();
    const stale = state.market?.staleness ?? 0;
    const reserved = reservedMarketCredits();
    const tabs = (["exchange", "warehouse", "specialists", "modules", "transactions"] as DeckMarketTab[])
      .map((tab) => `<button type="button" data-deck-act="market-tab" data-tab="${tab}" aria-selected="${this.tab === tab}">${esc(label(tab))}</button>`).join("");
    // Ticker, account and fleet reports arrive independently. A missing account
    // hides only its values and spending actions, never the entire Market. No
    // current truth or zero-filled pending wallet is used to fill the gaps.
    const body = this.tab === "transactions" ? transactionsHtml(state)
      : this.tab === "exchange" ? this.exchangeHtml()
      : this.tab === "warehouse" ? this.warehouseHtml()
        : this.tab === "specialists" ? this.specialistsHtml()
          : this.modulesHtml();
    return `<section class="deck-page deck-market">` +
      `<div class="deck-stat-grid deck-market__stats">${stat("Spendable credits", account ? `${reserved > 0 ? "~" : ""}${fmt(spendableMarketCredits())} Cr` : "—")}${stat("Reserved in flight", `${fmt(reserved)} Cr`, reserved > 0 ? "warn" : "")}${stat("Equity", account ? `${fmt(account.valuation)} Cr` : "—")}${stat("Market report", state.market ? informationDelay(stale) : "—", stale > 0.5 ? "stale" : "")}</div>` +
      `<nav class="deck-tabs" aria-label="Market sections">${tabs}</nav>${!account ? `<div class="deck-market-feedback" role="status">Account report pending · balances and trading unlock when its light arrives.</div>` : ""}${this.feedback ? `<div class="deck-market-feedback">${esc(this.feedback)}</div>` : ""}${body}</section>`;
  }

  private exchangeHtml(): string {
    const market = this.ctx.state.market;
    const account = this.accountReport();
    const stale = (market?.staleness ?? 0) > 0.5;
    const board = COMMODITIES.map((commodity) => {
      const quote = market?.prices.find((entry) => entry.commodity === commodity);
      const history = this.ctx.state.priceHistory[commodity] ?? [];
      const price = quote?.price;
      const movement = trend(history);
      return `<button type="button" class="deck-market-row${this.commodity === commodity ? " is-selected" : ""}" data-deck-act="market-commodity" data-commodity="${commodity}">` +
        `<span>${commodityIcon(commodity)}<span><b>${esc(label(commodity))}</b><small>warehouse ${account ? warehouseUnits(commodity) : "—"}</small></span></span>` +
        spark(history.length ? history : price === undefined ? [0, 0] : [price, price]) +
        `<em class="is-${movement.tone}">${price === undefined ? "—" : `${stale ? "~" : ""}${price.toFixed(2)}`} ${movement.glyph}</em>` +
        `<small>${quote ? quote.available_buy === quote.available_sell ? `depth ${quote.available_buy}` : `buy depth ${quote.available_buy} · sell depth ${quote.available_sell}` : "no arrived quote"}</small></button>`;
    }).join("");
    return `<div class="deck-market-layout"><section class="deck-section deck-market-board"><header><div><h3>Observed prices</h3></div><b class="deck-stale-value">${market ? esc(informationDelay(market.staleness)) : "Awaiting quotes"}</b></header><div class="deck-market-board__head"><span>Commodity</span><span>History</span><span>Price</span><span>Liquidity</span></div>${board}</section>` +
      `${this.ticketHtml()}</div><div class="deck-market-orders">${this.ordersHtml()}</div>`;
  }

  private ticketHtml(): string {
    const account = this.accountReport();
    const quote = this.ctx.state.market?.prices.find((entry) => entry.commodity === this.commodity);
    const average = quote ? marketAverageQuote(quote.price, this.quantity, this.side, quote.depth) : null;
    const penalty = !account || average === null ? 0 : this.quantity * average * (this.ctx.state.charter?.market_penalty_frac ?? 0);
    const held = warehouseUnits(this.commodity);
    const liquidity = quote ? (this.side === "buy" ? quote.available_buy : quote.available_sell) : 0;
    const warnings: string[] = [];
    if (!quote) warnings.push("No observed quote yet.");
    if (quote && this.quantity > liquidity) warnings.push(`Observed ${this.side === "buy" ? "supply" : "demand"} is ${liquidity}.`);
    if (account && this.side === "sell" && held < this.quantity) warnings.push(`Warehouse holds ${held} — ${this.quantity - held} short.`);
    if (penalty > 0.005) warnings.push(`${fmt(penalty)} Cr charter penalty ${this.side === "buy" ? "included" : "deducted"}.`);
    const limitReady = !this.limitOn || this.limitPrice > 0;
    const stockReady = this.side === "buy" || held >= this.quantity;
    const canSubmit = !!account && !!quote && limitReady && stockReady;
    const total = !account || average === null ? null : this.quantity * average * (1 + (this.side === "buy" ? 1 : -1) * (this.ctx.state.charter?.market_penalty_frac ?? 0));
    const button = this.limitOn
      ? `Place limit ${this.side}`
      : `${label(this.side)} ${this.quantity} ${label(this.commodity)}${total === null ? "" : ` · ~${fmt(total)} Cr`}`;
    return `<section class="deck-section deck-trade-ticket"><header><h3>Trade ticket</h3></header>` +
      `<div class="deck-segment"><button type="button" data-deck-act="market-side" data-side="buy" aria-pressed="${this.side === "buy"}">Buy</button><button type="button" data-deck-act="market-side" data-side="sell" aria-pressed="${this.side === "sell"}">Sell</button></div>` +
      `<div class="deck-trade-ticket__commodity">${commodityIcon(this.commodity)}<span><small>Selected commodity</small><b>${esc(label(this.commodity))}</b></span></div>` +
      `${commodityPurpose(this.commodity, this.ctx.state.galaxy?.build_options ?? []) ? `<small>${esc(commodityPurpose(this.commodity, this.ctx.state.galaxy?.build_options ?? []))}</small>` : ""}` +
      `<div class="deck-form-grid"><label>Quantity<input id="deck-market-quantity" data-market-input="trade-quantity" type="number" min="1" step="1" value="${this.quantity}"></label><label class="deck-check"><input data-market-input="trade-limit-on" type="checkbox" ${this.limitOn ? "checked" : ""}> Limit order</label>${this.limitOn ? `<label>Limit price<input data-market-input="trade-limit" type="number" min="0" step="0.1" value="${this.limitPrice || ""}"></label>` : ""}</div>` +
      `<div class="deck-ticket-quote">${average === null ? "Waiting for a delayed quote." : `Est. ${average.toFixed(2)} Cr/u · protection ±${Math.round(MARKET_PROTECTION_FRAC * 100)}%`}</div>` +
      `${warnings.length ? `<div class="deck-ticket-warnings">${warnings.map((warning) => `<span>${esc(warning)}</span>`).join("")}</div>` : ""}` +
      `<button type="button" class="is-primary" data-deck-act="market-submit" ${canSubmit ? "" : "disabled"}>${esc(button)}</button>` +
      `<small class="deck-muted">${this.limitOn ? "Limit orders fill in batches. " : ""}Receipts arrive after the information delay.</small></section>`;
  }

  private ordersHtml(): string {
    pruneMarketReservations();
    const account = this.accountReport();
    const open = account?.orders ?? [];
    const openRows = open.map((order) => `<div class="deck-order-row"><span><b>${esc(label(order.side))} ${order.units} ${esc(label(order.commodity))}</b><small>resting @ ${order.limit_price.toFixed(2)}</small></span><button type="button" data-deck-act="market-cancel" data-order="${order.id}">Cancel</button></div>`).join("");
    const incoming = marketReservations.map((order) => `<div class="deck-order-row"><span>${icon("inTransit", "sm")}<span><b>${esc(label(order.side))} ${order.orderUnits} ${esc(label(order.commodity))}${order.limitPrice === undefined ? "" : ` @ ${order.limitPrice.toFixed(2)}`}</b><small>sent · awaiting Market Hub</small></span></span></div>`).join("");
    const recent = recentMarketOrders.map((order) => `<div class="deck-order-row"><span>${icon("confirmed", "sm")}<span><b>${order.side === "buy" ? "Bought" : "Sold"} ${order.units} ${esc(label(order.commodity))} @ ${order.unitPrice.toFixed(2)}</b><small>${order.limitFill ? "limit fill · " : ""}${esc(agoLabel(order.observedAt))}</small></span></span></div>`).join("");
    return `<section class="deck-section"><header><h3>Open orders</h3><b>${account ? open.length : "—"}</b></header>${openRows || `<div class="deck-empty-inline">${account ? "No resting limit orders." : "Awaiting account report."}</div>`}</section>` +
      `<section class="deck-section"><header><h3>Incoming orders</h3><b>${marketReservations.length}</b></header>${incoming || `<div class="deck-empty-inline">No orders in flight.</div>`}</section>` +
      `<section class="deck-section"><header><h3>Recent orders</h3><b>${recentMarketOrders.length}</b></header>${recent || `<div class="deck-empty-inline">No execution receipts yet.</div>`}</section>`;
  }

  private warehouseHtml(): string {
    const account = this.accountReport();
    const holdings = account ? COMMODITIES.map((commodity) => ({ commodity, units: warehouseUnits(commodity) })).filter((row) => row.units > 0) : [];
    const ledger = holdings.map((row) => `<span>${commodityIcon(row.commodity)}<small>${esc(label(row.commodity))}</small><b>${row.units}</b></span>`).join("");
    const berths = hubDockedFleets().map((fleet) => {
      const manifest = fleetCargoManifest(fleet);
      const unloadQueued = (this.ctx.state.pendingOrders.get(fleet.id) ?? []).some((order) => !order.lost && order.kind === "unload");
      const cargo = manifest.length ? manifest.map((stack) => `${fmt(stack.units)} ${label(stack.commodity)}`).join(" · ") : "hold empty";
      const exact = fleetExactCount(fleet);
      const count = exact === null ? `est. ${countClassLabel(fleet.count_class)} ships` : `${exact} ship${exact === 1 ? "" : "s"}`;
      return `<article class="deck-berth"><button type="button" data-deck-act="market-hub-fleet" data-fleet="${esc(fleet.id)}">${icon("manifest", "md")}<span><b>${esc(shipKindLabel(fleet.kind))} fleet</b><small>${esc(count)} · ${esc(cargo)}</small></span><em>›</em></button>${manifest.length ? `<button type="button" data-deck-act="market-hub-unload" data-fleet="${esc(fleet.id)}" ${unloadQueued ? "disabled" : ""}>${icon("unload", "sm")} ${unloadQueued ? "Unload queued" : "Unload all"}</button>` : ""}</article>`;
    }).join("");
    const shipments = (this.ctx.state.freight?.shipments ?? []).map((shipment) => `<div class="deck-order-row"><span>${icon("authorityFreighter", "sm")}<span><b>${shipment.units} ${esc(label(shipment.commodity))} ${shipment.direction === "outbound" ? "→" : "←"} ${esc(systemName(shipment.system))}</b><small>${shipment.aboard ? "aboard" : "awaiting departure"}${shipment.direction === "inbound" && shipment.sell_on_arrival ? " · sells on arrival" : ""}</small></span></span></div>`).join("");
    return `<section class="deck-section deck-market-hub"><img src="/art/wormhole_hub_concept.png" alt=""/><div><h3>Authority market station</h3><p>Your goods and docked fleets at the wormhole terminus.</p></div></section>` +
      `<div class="deck-market-warehouse"><div class="deck-market-column"><section class="deck-section"><header><h3>Market Warehouse</h3><b>${account ? holdings.reduce((sum, row) => sum + row.units, 0) : "—"}</b></header><div class="deck-ledger">${ledger || (account ? `<span><small>Warehouse</small><b>empty</b></span>` : `<span>Awaiting account report.</span>`)}</div></section>` +
      `<section class="deck-section"><header><h3>Shipments</h3><b>${this.ctx.state.freight?.shipments.length ?? 0}</b></header>${shipments || `<div class="deck-empty-inline">No freight booked.</div>`}</section></div>` +
      `<div class="deck-market-column"><section class="deck-section"><header><h3>Docked fleets</h3><b>${hubDockedFleets().length}</b></header><div class="deck-berth-list">${berths || `<div class="deck-empty-inline">No fleets reported docked.</div>`}</div></section>` +
      `${this.freightHtml()}</div></div>`;
  }

  private freightHtml(): string {
    if (!this.accountReport()) return emptyState("Book Authority freight", "Awaiting account report.");
    const freight = this.ctx.state.freight;
    if (!freight) return emptyState("Freight desk unavailable", "Waiting for the Authority timetable.");
    const terms = freight.terms;
    const term = terms.find((entry) => entry.system === this.freightSystem);
    const entries = freightDraftEntries();
    const priced = entries.length ? entries : [{ commodity: this.freightCommodity, units: this.freightQuantity }];
    const totalUnits = priced.reduce((sum, entry) => sum + entry.units, 0);
    const destination = this.ctx.state.systems.find((system) => system.id === this.freightSystem);
    const available = (commodity: Commodity): number => this.freightDirection === "outbound"
      ? warehouseUnits(commodity)
      : Math.floor(destination?.stockpile?.find((slot) => slot.commodity === commodity)?.units ?? 0);
    const shortages = priced.filter((entry) => entry.units > available(entry.commodity)).map((entry) => `${label(entry.commodity)} ${available(entry.commodity)}/${entry.units}`);
    const headroom = destination ? Math.max(0, destination.storage_cap - destination.storage_used) : 0;
    const storageWarning = this.freightDirection === "outbound" && destination && totalUnits > headroom
      ? `Only ${Math.floor(headroom)} storage is free in the latest served destination report; excess returns to the warehouse.` : "";
    const tariff = this.ctx.state.charter?.tariff_mult ?? 1;
    const fee = term ? priced.reduce((sum, entry) => {
      const price = this.ctx.state.market?.prices.find((point) => point.commodity === entry.commodity)?.price ?? 0;
      return sum + freightFee(freight, term, price, entry.units) * tariff;
    }, 0) : 0;
    const wait = Math.max(0, freight.next_departure - this.ctx.state.simTime);
    const flight = term ? (this.freightDirection === "outbound" ? term.secs_out : term.secs_round) : 0;
    const departures = term ? Math.max(1, Math.ceil(totalUnits / term.cap)) : 1;
    const finalWait = wait + (departures - 1) * freight.period;
    const arrival = departures > 1 ? `${fmtDur(wait + flight)}–${fmtDur(finalWait + flight)}` : fmtDur(wait + flight);
    const manifest = entries.map((entry) => `<div class="deck-order-row"><span>${commodityIcon(entry.commodity)}<span><b>${entry.units} ${esc(label(entry.commodity))}</b><small>mixed manifest line</small></span></span><button type="button" data-deck-act="freight-remove" data-commodity="${entry.commodity}">Remove</button></div>`).join("");
    const systems = terms.map((entry) => `<option value="${esc(entry.system)}" ${entry.system === this.freightSystem ? "selected" : ""}>${esc(systemName(entry.system))}</option>`).join("");
    const commodities = COMMODITIES.map((commodity) => `<option value="${commodity}" ${commodity === this.freightCommodity ? "selected" : ""}>${esc(label(commodity))}</option>`).join("");
    return `<section class="deck-section deck-freight"><header><div><h3>Book Authority freight</h3><p>One scheduled physical carrier may combine several manifest lines within its shared departure capacity.</p></div>${icon("authorityFreighter", "md")}</header>` +
      `<div class="deck-segment"><button type="button" data-deck-act="freight-direction" data-direction="outbound" aria-pressed="${this.freightDirection === "outbound"}">Warehouse → system</button><button type="button" data-deck-act="freight-direction" data-direction="inbound" aria-pressed="${this.freightDirection === "inbound"}">System → warehouse</button></div>` +
      `<div class="deck-form-grid deck-form-grid--freight"><label>System<select data-market-input="freight-system">${systems}</select></label><label>Goods<select data-market-input="freight-commodity">${commodities}</select></label><label>Quantity<input data-market-input="freight-quantity" type="number" min="1" step="1" value="${this.freightQuantity}"></label><button type="button" data-deck-act="freight-add">Add cargo</button></div>` +
      `<div class="deck-freight-manifest">${manifest || `<div class="deck-empty-inline">Add several goods here, or book the selected line directly.</div>`}</div>` +
      (this.freightDirection === "inbound" ? `<label class="deck-check"><input data-market-input="freight-sell" type="checkbox" ${this.freightSellOnArrival ? "checked" : ""}> Sell at the Exchange when the lot reaches the warehouse</label>` : "") +
      `<div class="deck-freight-quote">${term ? `<b>Fee ${fmt(fee)} Cr${tariff > 1.0001 ? ` · ×${tariff.toFixed(2)} charter tariff` : ""}</b><span>departs in ${fmtDur(wait)} · arrives ~${arrival} · ${term.cap} total/departure${departures > 1 ? ` · rides ${departures} departures` : ""}</span>` : `<span>You hold no system the Authority can serve.</span>`}${shortages.length ? `<em>Short: ${esc(shortages.join(", "))}. Lines soft-reject independently.</em>` : ""}${storageWarning ? `<em>${esc(storageWarning)}</em>` : ""}</div>` +
      `<button type="button" class="is-primary" data-deck-act="freight-submit" ${term ? "" : "disabled"}>${entries.length ? `Book mixed manifest · ${entries.length} goods` : "Book freight"}</button></section>`;
  }

  private specialistsHtml(): string {
    const cost = this.ctx.state.galaxy?.specialist_hire_cost ?? 800;
    const home = this.accountReport() ? this.homeSystemId() : undefined;
    const rows = SPECIALISTS.map((specialist) => `<article class="deck-service-row"><span>${icon(specialist.icon, "md")}<span><b>${esc(label(specialist.slug))}</b><small>${esc(specialist.blurb)}</small></span></span><button type="button" data-deck-act="market-hire" data-specialist="${specialist.slug}" ${home && spendableMarketCredits() >= cost ? "" : "disabled"}>Hire · ${fmt(cost)} Cr</button></article>`).join("");
    return `<section class="deck-section"><header><div><h3>Available specialists</h3><p>Standing Sol contracts ship to your home on a sub-light, raidable personnel liner.</p></div></header><div class="deck-service-list deck-market-services">${rows}</div><div class="deck-ticket-quote">A posted specialist multiplies matching production lines ×1.75.</div></section>`;
  }

  private modulesHtml(): string {
    const account = this.accountReport();
    const home = this.homeSystem();
    const held = home?.modules ?? {};
    const rows = MODULES.map((module) => {
      const value = moduleRecipeValue(module.kind);
      const buy = value === null || isBlueprintOnly(module.kind) ? null : value * MODULE_BUY_MULT;
      const sell = value === null ? null : value * MODULE_SELL_MULT;
      return `<article class="deck-service-row"><span>${icon(module.icon, "md")}<span><b>${esc(module.name)}</b><small>${esc(module.role)} · held ${held[module.kind] ?? 0}</small></span></span><div><button type="button" data-deck-act="market-module-buy" data-module="${module.kind}" ${account && home && buy !== null && spendableMarketCredits() >= buy ? "" : "disabled"}>${isBlueprintOnly(module.kind) ? "Blueprint only" : `Buy ${buy === null ? "—" : `~${fmt(buy)} Cr`}`}</button><button type="button" data-deck-act="market-module-sell" data-module="${module.kind}" ${account && (held[module.kind] ?? 0) > 0 ? "" : "disabled"}>Sell ${sell === null ? "—" : `~${fmt(sell)} Cr`}</button></div></article>`;
    }).join("");
    return `<section class="deck-section"><header><div><h3>Combat modules</h3><p>Sol modules ship as physical crates. Local manufacture at an Armaments Complex remains cheaper.</p></div></header><div class="deck-service-list deck-market-services">${rows}</div></section>`;
  }

  private submitTrade(): void {
    const quote = this.ctx.state.market?.prices.find((entry) => entry.commodity === this.commodity);
    if (!quote) return;
    const quantity = Math.max(1, Math.floor(this.quantity));
    if (this.limitOn && this.limitPrice > 0) {
      this.ctx.send({ type: "PlaceLimitOrder", side: this.side, commodity: this.commodity, units: quantity, limit_price: this.limitPrice });
      reserveMarketOrder({
        kind: "limit", side: this.side, commodity: this.commodity, orderUnits: quantity,
        units: this.side === "sell" ? quantity : 0,
        credits: this.side === "buy" ? quantity * this.limitPrice : 0,
        limitPrice: this.limitPrice,
      });
    } else {
      const average = marketAverageQuote(quote.price, quantity, this.side, quote.depth);
      const protection = average * (this.side === "buy" ? 1 + MARKET_PROTECTION_FRAC : 1 - MARKET_PROTECTION_FRAC);
      this.ctx.send(this.side === "buy"
        ? { type: "MarketBuy", commodity: this.commodity, units: quantity, max_unit_price: protection }
        : { type: "MarketSell", commodity: this.commodity, units: quantity, min_unit_price: protection });
      reserveMarketOrder({
        kind: "market", side: this.side, commodity: this.commodity, orderUnits: quantity,
        units: this.side === "sell" ? quantity : 0,
        credits: this.side === "buy" ? quantity * protection * (1 + (this.ctx.state.charter?.market_penalty_frac ?? 0)) : 0,
      });
    }
    this.feedback = `${label(this.side)} order sent · awaiting the Market Hub account report.`;
    this.hooks.notice(`<b>${esc(label(this.side))} order sent</b> · ${quantity} ${esc(label(this.commodity))} · awaiting delayed Market Hub light.`);
  }

  private submitFreight(): void {
    const system = this.freightSystem as EntityId;
    if (!system) return;
    const entries = freightDraftEntries();
    if (!entries.length) entries.push({ commodity: this.freightCommodity, units: Math.max(1, Math.floor(this.freightQuantity)) });
    for (const entry of entries) {
      this.ctx.send(this.freightDirection === "outbound"
        ? { type: "BookFreightOut", system, commodity: entry.commodity, units: entry.units }
        : { type: "BookFreightIn", system, commodity: entry.commodity, units: entry.units, sell_on_arrival: this.freightSellOnArrival });
    }
    const cargo = entries.map((entry) => `${entry.units} ${label(entry.commodity)}`).join(" + ");
    this.feedback = `Freight booking sent: ${cargo} ${this.freightDirection === "outbound" ? "→" : "←"} ${systemName(system)}.`;
    this.hooks.notice(`<b>Freight booking sent</b> · ${esc(cargo)} · awaiting Authority receipt.`);
    freightDraft.clear();
  }

  private hireSpecialist(specialist?: string): void {
    const home = this.homeSystemId();
    if (!specialist || !home) return;
    this.ctx.send({ type: "HireSpecialist", specialist, dest_system: home });
    this.feedback = `${label(specialist)} contract dispatched to ${systemName(home)}.`;
    this.hooks.notice(`<b>Specialist contract sent</b> · ${esc(label(specialist))} → ${esc(systemName(home))}.`);
  }

  private tradeModule(value: string | undefined, buy: boolean): void {
    const module = MODULES.find((entry) => entry.kind === value);
    const home = this.homeSystemId();
    if (!module || !home) return;
    if (buy && isBlueprintOnly(module.kind)) return;
    this.ctx.send(buy
      ? { type: "BuyModule", module: module.kind, n: 1, dest_system: home }
      : { type: "SellModule", module: module.kind, n: 1, from_system: home });
    this.feedback = `${buy ? "Buy" : "Sell"} order dispatched for ${module.name}.`;
    this.hooks.notice(`<b>Module order sent</b> · ${buy ? "Buy" : "Sell"} ${esc(module.name)}.`);
  }

  private applyRouteTab(route: DeckRoute): void {
    const requested = route.query?.tab;
    const key = requested ?? "";
    if (key === this.appliedRouteTab) return;
    this.appliedRouteTab = key;
    if (isMarketTab(requested)) this.tab = requested;
  }

  private repairFreightSystem(): void {
    const terms = this.ctx.state.freight?.terms ?? [];
    if (!terms.some((entry) => entry.system === this.freightSystem)) this.freightSystem = terms[0]?.system ?? "";
  }

  private homeSystem() {
    return this.ctx.state.systems.find((system) => system.owner === this.ctx.state.playerId);
  }

  private homeSystemId(): EntityId | undefined {
    return this.homeSystem()?.id;
  }
}

/** Translate the arriving economy event—not the command-time estimate—into one
 * shared Deck receipt. Its route makes transient news recoverable as a click
 * into the relevant Market account surface. */
export function deckTradeNotice(trade: TradeEvent): DeckTradeNotice {
  const exchange = { name: "market", query: { tab: "exchange" } } as DeckRoute;
  const warehouse = { name: "market", query: { tab: "warehouse" } } as DeckRoute;
  let title = "Market receipt";
  let message = "Market account updated.";
  let tone: DeckTradeNotice["tone"] = "quiet";
  let destination = exchange;
  switch (trade.event) {
    case "Bought":
      title = "Purchase settled";
      message = `Bought ${trade.units} ${label(trade.commodity)} @ ${trade.unit_price.toFixed(2)} Cr/u — held in your Market Warehouse.${trade.penalty ? ` Charter penalty ${fmt(trade.penalty)} Cr.` : ""}`;
      tone = "good";
      break;
    case "Delivered":
      title = "Delivery arrived";
      message = trade.system
        ? `+${trade.units} ${label(trade.commodity)} stocked at ${systemName(trade.system)}.`
        : `+${trade.units} ${label(trade.commodity)} entered your Market Warehouse.`;
      tone = "good";
      destination = warehouse;
      break;
    case "StockDispatched":
      title = "Supply freighter away";
      message = `${trade.units} ${label(trade.commodity)} → ${systemName(trade.system)} · raidable.`;
      destination = warehouse;
      break;
    case "SellDispatched":
      title = "Sell freighter away";
      message = `${trade.units} ${label(trade.commodity)} crossing to the Market Hub.`;
      destination = warehouse;
      break;
    case "Sold":
      title = "Sale settled";
      message = `Sold ${trade.units} ${label(trade.commodity)} @ ${trade.unit_price.toFixed(2)} Cr/u.${trade.penalty ? ` Charter penalty ${fmt(trade.penalty)} Cr.` : ""}`;
      tone = "good";
      break;
    case "LimitPlaced":
      title = "Limit order resting";
      message = `${label(trade.side)} ${trade.units} ${label(trade.commodity)} @ ${trade.limit_price.toFixed(2)} Cr/u.`;
      break;
    case "LimitFilled":
      title = "Limit order filled";
      message = `${label(trade.side)} ${trade.units} ${label(trade.commodity)} @ ${trade.unit_price.toFixed(2)} Cr/u in the batch.${trade.penalty ? ` Charter penalty ${fmt(trade.penalty)} Cr.` : ""}`;
      tone = "good";
      break;
    case "LimitCancelled":
      title = "Limit order cancelled";
      message = `${label(trade.side)} ${trade.units} ${label(trade.commodity)} @ ${trade.limit_price.toFixed(2)} Cr/u · escrow returned.`;
      break;
    case "AutoDispatched":
      title = `Logistics rule ${trade.rule_id}`;
      message = `${trade.units} ${label(trade.commodity)} shipped automatically · raidable.`;
      destination = warehouse;
      break;
    case "SupplyDiverted": {
      const action = trade.action === "lost" ? "lost; cargo dropped"
        : trade.action === "returned_home" ? "rerouted home · raidable"
          : "rerouted to sell at the Market Hub · raidable";
      title = "Supply diverted";
      message = `${systemName(trade.system)} is no longer held: ${trade.units} ${label(trade.commodity)} ${action}.`;
      tone = trade.action === "lost" ? "bad" : "warn";
      destination = warehouse;
      break;
    }
    case "StorageOverflow":
      title = "Destination storage full";
      message = `${trade.units} ${label(trade.commodity)} could not unload at ${systemName(trade.system)} and continues to the Market Hub.`;
      tone = "warn";
      destination = warehouse;
      break;
    case "Rejected":
      title = "Order rejected";
      message = rejectText(trade);
      tone = "warn";
      destination = trade.system ? warehouse : exchange;
      break;
    case "FreightBooked":
      title = "Authority freight booked";
      message = `${trade.units} ${label(trade.commodity)} ${trade.direction === "outbound" ? "→" : "←"} ${systemName(trade.system)} · ${fmt(trade.fee)} Cr · departs in ${fmtDur(Math.max(0, trade.depart_at - state.simTime))}.`;
      tone = "good";
      destination = warehouse;
      break;
    case "FreightMoved": {
      const cargo = `${trade.units} ${label(trade.commodity)}`;
      const place = systemName(trade.system);
      const remaining = trade.remaining ?? 0;
      title = "Authority freight update";
      message = trade.stage === "departed" ? `Freighter away with ${cargo} · ${place}.`
        : trade.stage === "collected_for_pickup" ? `Freighter collected ${cargo} at ${place}.`
          : trade.stage === "delivered_to_system" && remaining > 0 ? `${cargo} delivered to ${place}; ${remaining} remains aboard because storage is full.`
            : trade.stage === "delivered_to_system" ? `${cargo} delivered to ${place}.`
              : trade.stage === "arrived_at_warehouse" ? `${cargo} from ${place} landed in your Market Warehouse.`
                : trade.stage === "returned_undeliverable" ? `${cargo} could not unload at ${place}; returned to your warehouse.`
                  : trade.stage === "forfeited_on_capture" ? `${cargo} awaiting pickup at ${place} was forfeited when the system fell.`
                    : `${cargo} was destroyed with its Authority freighter near ${place}.`;
      tone = trade.stage === "forfeited_on_capture" || trade.stage === "lost_with_freighter" ? "bad"
        : trade.stage === "returned_undeliverable" || remaining > 0 ? "warn" : "good";
      destination = warehouse;
      break;
    }
    case "CharterReinstated": {
      const from = projectedBand(state.charter ? state.charter.standing - trade.before : 0);
      const to = projectedBand(state.charter ? state.charter.standing - trade.after : 0);
      title = "Authority standing restored";
      message = `Paid ${fmt(trade.cost)} Cr for ${trade.points.toFixed(0)} standing (${trade.before.toFixed(0)} → ${trade.after.toFixed(0)}).${to !== from ? ` Restored to ${to}.` : ""}`;
      tone = "good";
      break;
    }
    case "Loaded":
      title = "Cargo loaded";
      message = `${trade.units} ${label(trade.commodity)} loaded at ${trade.system ? systemName(trade.system) : "the Market Hub"}.`;
      tone = "good";
      destination = warehouse;
      break;
    case "Unloaded":
      title = "Cargo unloaded";
      message = `${trade.units} ${label(trade.commodity)} unloaded at ${trade.system ? systemName(trade.system) : "the Market Hub"}.`;
      tone = "good";
      destination = warehouse;
      break;
  }
  return { title, message, tone, destination };
}

function isMarketTab(value?: string): value is DeckMarketTab {
  return value === "exchange" || value === "warehouse" || value === "specialists" || value === "modules" || value === "transactions";
}

function isCommodity(value?: string): value is Commodity {
  return !!value && COMMODITIES.includes(value as Commodity);
}

function positiveInteger(value: string, fallback: number): number {
  const parsed = Math.floor(Number(value));
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function stat(name: string, value: string, tone: "" | "warn" | "stale" = ""): string {
  return `<span class="deck-stat${tone ? ` is-${tone}` : ""}"><small>${esc(name)}</small><b>${esc(value)}</b></span>`;
}

function spark(data: number[]): string {
  const points = data.length >= 2 ? data : [data[0] ?? 0, data[0] ?? 0];
  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = max - min || 1;
  const path = points.map((value, index) => `${((index / (points.length - 1)) * 60).toFixed(1)},${(18 - ((value - min) / span) * 16 - 1).toFixed(1)}`).join(" ");
  const stroke = points.at(-1)! >= points[0] ? "var(--positive)" : "var(--negative)";
  return `<svg class="deck-spark" viewBox="0 0 60 18" preserveAspectRatio="none" aria-hidden="true"><polyline fill="none" stroke="${stroke}" stroke-width="1.5" vector-effect="non-scaling-stroke" points="${path}"/></svg>`;
}

function emptyState(title: string, copy: string): string {
  return `<div class="deck-empty"><b>${esc(title)}</b><span>${esc(copy)}</span></div>`;
}

function esc(value: string): string {
  return value.replace(/[&<>\"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!);
}

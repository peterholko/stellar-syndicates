import {
  COMMODITIES,
  marketAverageQuote,
  marketReservations,
  moduleRecipeValue,
  recentMarketOrders,
  recordRecentMarketOrder,
  reserveMarketOrder,
  settleMarketReservation,
  spendableMarketCredits,
  warehouseUnits,
} from "../../core/derive/market";
import { fleetCargoCapacity, guardCapable, jumpCapable, shipKindLabel } from "../../core/derive/fleet";
import { foundingHomeSystemId } from "../../core/derive/geo";
import {
  countClassLabel,
  fleetCargoManifest,
  fleetCargoUnits,
  fleetExactCount,
  type Commodity,
  type GhostView,
  type ModuleKind,
  type Side,
  type TradeEvent,
} from "../../protocol";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import type { CoreContext } from "../types";
import type { SheetEntry, SheetView } from "./sheets";
import { SheetStack } from "./sheets";

type MarketTab = "exchange" | "warehouse" | "specialists" | "modules";
type FreightDirection = "outbound" | "inbound";

interface SurfaceHooks {
  openSheet(entry: SheetEntry): void;
  focusFleet(id: string): void;
  focusSystem(id: string): void;
  armMove(id: string): void;
  enterSystem(id: string): void;
  exitSemantic(): void;
  notice(html: string): void;
}

const MARKET_PROTECTION_FRAC = 0.10;
const MODULE_BUY_MULT = 2;
const MODULE_SELL_MULT = 0.5;
const FOUNDING_MINIMIZED_KEY = "stellar-syndicates:founding-guide-mobile-minimized";

const MODULES: { kind: ModuleKind; name: string; role: string }[] = [
  { kind: "mass_driver", name: "Mass Driver", role: "Kinetic weapon" },
  { kind: "torpedo_rack", name: "Torpedo Rack", role: "Heavy strike weapon" },
  { kind: "point_defense_screen", name: "Point-Defense", role: "Torpedo interception" },
  { kind: "reflective_plating", name: "Reflective Plating", role: "Beam protection" },
  { kind: "whipple_armor", name: "Whipple Armor", role: "Kinetic protection" },
];

const SPECIALISTS = [
  ["geologist", "Geologist", "mineral extraction"],
  ["petrochemical_engineer", "Petrochemical Engineer", "volatiles and fuel"],
  ["xenobiologist", "Xenobiologist", "biomass and provisions"],
  ["industrial_engineer", "Industrial Engineer", "heavy industry"],
  ["naval_architect", "Naval Architect", "shipbuilding and armaments"],
] as const;

const esc = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);
const human = (value: string): string => value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
const fmt = (value: number, digits = 0): string => Number.isFinite(value)
  ? value.toLocaleString(undefined, { maximumFractionDigits: digits, minimumFractionDigits: digits })
  : "—";
const element = <T extends HTMLElement>(id: string): T | null => document.getElementById(id) as T | null;
const propsOf = <T extends object>(entry: SheetEntry): Partial<T> => (entry.props && typeof entry.props === "object" ? entry.props : {}) as Partial<T>;

export class MobileSurfaces {
  private marketTab: MarketTab = "exchange";
  private marketCommodity: Commodity = "fuel";
  private marketSide: Side = "buy";
  private freightDirection: FreightDirection = "outbound";
  private readonly dismissedDecisions = new Set<string>();
  private foundingMinimized = false;

  constructor(
    private readonly ctx: CoreContext,
    private readonly sheets: SheetStack,
    private readonly hooks: SurfaceHooks,
  ) {
    try { this.foundingMinimized = localStorage.getItem(FOUNDING_MINIMIZED_KEY) === "1"; } catch { /* optional */ }
  }

  render(entry: SheetEntry): SheetView | null {
    switch (entry.id) {
      case "fleets": return this.renderFleets();
      case "ship": return this.renderShip(entry);
      case "system": return this.renderSystem(entry);
      case "market": return this.renderMarket();
      case "log": return this.renderCheckin();
      case "battle": return this.renderBattle(entry);
      default: return null;
    }
  }

  onTrade(trade: TradeEvent): void {
    settleMarketReservation(trade);
    recordRecentMarketOrder(trade);
  }

  refreshFounding(): void {
    const root = document.getElementById("m-founding");
    if (!root) return;
    if (renderDeferred("m-founding", () => this.refreshFounding())) return;
    const founding = this.ctx.state.founding;
    if (!founding || (founding.stage === "complete" && !founding.protected)) {
      root.hidden = true;
      return;
    }
    root.hidden = false;
    root.classList.toggle("is-minimized", this.foundingMinimized);
    const step = FOUNDING_STEPS[founding.stage];
    const shield = founding.protected
      ? founding.protection_min_until > this.ctx.state.simTime
        ? `shield ${fmt(founding.protection_min_until - this.ctx.state.simTime)}s`
        : "shield active"
      : "shield ended";
    const content = foundingCopy(founding.stage);
    setHtml(root,
      `<header><span>Founding ${step}/12 · ${shield}</span>` +
      `<button type="button" data-mobile-act="founding-toggle" aria-label="${this.foundingMinimized ? "Expand" : "Minimize"} tutorial">${this.foundingMinimized ? "+" : "−"}</button></header>` +
      `<div class="m-founding__body"><b>${esc(content.title)}</b><p>${esc(content.copy)}</p>` +
      `<button type="button" class="m-primary" data-mobile-act="founding-action" data-kind="${content.action}">${esc(content.label)}</button></div>`,
    );
  }

  handleClick(event: Event): boolean {
    const button = (event.target as Element).closest<HTMLElement>("[data-mobile-act]");
    if (!button) return false;
    const action = button.dataset.mobileAct;
    if (!action || action === "confirm-intent" || action === "cancel-intent") return false;

    switch (action) {
      case "fleet-select":
        if (button.dataset.id) this.hooks.focusFleet(button.dataset.id);
        break;
      case "fleet-move":
        if (button.dataset.id) this.hooks.armMove(button.dataset.id);
        break;
      case "fleet-jump": {
        const fleet = this.ownFleet(button.dataset.id);
        if (fleet) {
          this.ctx.intent.armJumpAiming(fleet);
          this.hooks.notice("<b>Jump drive armed.</b> Pinch or pan as needed, then tap a legal destination.");
        }
        break;
      }
      case "fleet-guard": {
        const fleet = this.ownFleet(button.dataset.id);
        if (fleet) {
          this.ctx.intent.armGuardAiming(fleet);
          this.hooks.notice("<b>Guard armed.</b> Tap the fleet this Interceptor should protect.");
        }
        break;
      }
      case "fleet-recall":
        if (button.dataset.id) {
          this.ctx.send({ type: "RecallRaid", raider_id: button.dataset.id });
          delete this.ctx.state.raids[button.dataset.id];
          this.hooks.notice("Recall order dispatched.");
        }
        break;
      case "fleet-transit":
        if (button.dataset.id && (button.dataset.mode === "full" || button.dataset.mode === "stealth")) {
          this.ctx.send({ type: "SetFleetTransit", fleet_id: button.dataset.id, mode: button.dataset.mode });
          this.hooks.notice(`${human(button.dataset.mode)} transit order dispatched.`);
        }
        break;
      case "fleet-unload":
        this.unloadFleet(button.dataset.id);
        break;
      case "fleet-rescue":
        if (button.dataset.id) this.ctx.send({ type: "RequestFuelRescue", fleet_id: button.dataset.id });
        break;
      case "system-select":
        if (button.dataset.id) this.hooks.focusSystem(button.dataset.id);
        break;
      case "system-enter":
        if (button.dataset.id) this.hooks.enterSystem(button.dataset.id);
        break;
      case "semantic-exit":
        this.hooks.exitSemantic();
        break;
      case "market-tab":
        if (isMarketTab(button.dataset.tab)) {
          this.marketTab = button.dataset.tab;
          this.sheets.refresh();
        }
        break;
      case "market-commodity":
        if (isCommodity(button.dataset.commodity)) {
          this.marketCommodity = button.dataset.commodity;
          this.sheets.refresh();
        }
        break;
      case "market-side":
        if (button.dataset.side === "buy" || button.dataset.side === "sell") {
          this.marketSide = button.dataset.side;
          this.sheets.refresh();
        }
        break;
      case "market-submit":
        this.submitMarketOrder();
        break;
      case "market-cancel": {
        const id = Number(button.dataset.id);
        if (Number.isFinite(id)) this.ctx.send({ type: "CancelLimitOrder", order_id: id });
        break;
      }
      case "freight-direction":
        if (button.dataset.direction === "outbound" || button.dataset.direction === "inbound") {
          this.freightDirection = button.dataset.direction;
          this.sheets.refresh();
        }
        break;
      case "freight-submit":
        this.submitFreight();
        break;
      case "hire-specialist":
        this.hireSpecialist(button.dataset.specialist);
        break;
      case "module-buy":
      case "module-sell":
        this.tradeModule(button.dataset.module, action === "module-buy");
        break;
      case "decision-treaty":
        this.answerTreaty(button.dataset.id, button.dataset.answer === "accept");
        break;
      case "decision-syndicate":
        if (button.dataset.id) this.ctx.send({ type: "AcceptSyndicateInvite", syndicate_id: button.dataset.id });
        break;
      case "decision-operation":
        if (button.dataset.id) this.ctx.send({ type: "AcceptOperation", operation_id: button.dataset.id });
        break;
      case "decision-founding":
        this.runFoundingAction(button.dataset.kind ?? "home");
        break;
      case "decision-dismiss":
        if (button.dataset.key) {
          this.dismissedDecisions.add(button.dataset.key);
          this.sheets.refresh();
        }
        break;
      case "decision-withdraw":
        if (button.dataset.id) this.ctx.send({ type: "Withdraw", fleet_id: button.dataset.id });
        break;
      case "battle-open":
        if (button.dataset.id) this.hooks.openSheet({ id: "battle", props: { id: button.dataset.id } });
        break;
      case "founding-toggle":
        this.foundingMinimized = !this.foundingMinimized;
        try { localStorage.setItem(FOUNDING_MINIMIZED_KEY, this.foundingMinimized ? "1" : "0"); } catch { /* optional */ }
        this.refreshFounding();
        break;
      case "founding-action":
        this.runFoundingAction(button.dataset.kind ?? "home");
        break;
      default:
        return false;
    }
    return true;
  }

  private renderFleets(): SheetView {
    const fleets = this.ctx.state.ghosts
      .filter((ghost) => ghost.own)
      .sort((a, b) => Number(!!b.docked) - Number(!!a.docked) || shipKindLabel(a.kind).localeCompare(shipKindLabel(b.kind)) || a.id.localeCompare(b.id));
    const rows = fleets.map((fleet) => this.fleetRow(fleet)).join("");
    return {
      title: `Fleets${fleets.length ? ` · ${fleets.length}` : ""}`,
      eyebrow: "Corporate roster · served picture",
      html: rows
        ? `<div class="m-list">${rows}</div>`
        : `<div class="m-empty">No owned fleets have reported yet.</div>`,
    };
  }

  private fleetRow(fleet: GhostView): string {
    const count = fleetExactCount(fleet);
    const composition = fleet.composition?.map((stack) => `${stack.count} ${shipKindLabel(stack.kind)}`).join(" · ")
      ?? `est. ${countClassLabel(fleet.count_class)} ships`;
    const cargo = fleetCargoUnits(fleet);
    return `<button type="button" class="m-list-row" data-act="fleet:${esc(fleet.id)}" data-mobile-act="fleet-select" data-id="${esc(fleet.id)}">` +
      `<span class="m-list-row__icon">△</span><span class="m-list-row__main"><b>${esc(shipKindLabel(fleet.kind))}${count && count > 1 ? ` Fleet` : ""}</b>` +
      `<small>${esc(composition)}${cargo ? ` · cargo ${cargo}/${fleetCargoCapacity(fleet)}` : ""}</small></span>` +
      `<span class="m-list-row__meta"><em>${esc(this.fleetStatus(fleet))}</em><small>delay ${fmt(fleet.age)}s</small></span></button>`;
  }

  private renderShip(entry: SheetEntry): SheetView {
    const props = propsOf<{ id: string; kind: string; key: string }>(entry);
    if (props.kind === "jumpDeparture") {
      return { title: "Jump departure", eyebrow: "Historical bookmark", html: `<div class="m-empty">This light-delayed scar marks where a fleet jumped away. Its destination is unknown.</div>` };
    }
    if (props.kind === "emplacement") {
      const emplacement = this.ctx.state.emplacements.find((candidate) => candidate.id === props.id);
      return {
        title: emplacement ? "Deep-Space Sensor" : "Emplacement",
        eyebrow: "Standing infrastructure",
        html: emplacement
          ? `<div class="m-stat-grid"><span><small>Range</small><b>${fmt(emplacement.sensor_range)} su</b></span><span><small>Position</small><b>${fmt(emplacement.pos.x)}, ${fmt(emplacement.pos.y)}</b></span></div>`
          : `<div class="m-empty">This emplacement is no longer in the served picture.</div>`,
      };
    }
    const id = props.id ?? this.ctx.state.selectedShipId ?? undefined;
    const fleet = id ? this.ctx.state.ghosts.find((ghost) => ghost.id === id) : undefined;
    if (!fleet) return { title: "Fleet", eyebrow: "Fleet command", html: `<div class="m-empty">That fleet is no longer in the served picture.</div>` };
    const count = fleetExactCount(fleet);
    const composition = fleet.composition?.map((stack) => `${stack.count} × ${shipKindLabel(stack.kind)}`).join(" · ")
      ?? `Estimated ${countClassLabel(fleet.count_class)} ships`;
    const manifest = fleetCargoManifest(fleet);
    const cargo = manifest.length
      ? manifest.map((slot) => `<span>${esc(human(slot.commodity))}<b>${slot.units}</b></span>`).join("")
      : `<span class="m-muted">Hold empty</span>`;
    const orders = this.ctx.state.pendingOrders.get(fleet.id) ?? [];
    const orderRows = orders.length
      ? orders.map((order) => {
          const phase = liveSimTime() < order.arrives_at ? `signal · ${fmt(order.arrives_at - liveSimTime())}s` : "awaiting response";
          return `<div class="m-order"><b>${esc(human(order.kind))}</b><span>${phase}</span></div>`;
        }).join("")
      : `<div class="m-muted">No commands in flight.</div>`;
    const ownControls = fleet.own ? this.shipControls(fleet) : "";
    return {
      title: `${shipKindLabel(fleet.kind)}${count && count > 1 ? " Fleet" : ""}`,
      eyebrow: fleet.own ? "Your served fleet picture" : fleet.pirate ? "Pirate contact" : "Observed contact",
      html: `<div class="m-stat-grid"><span><small>Status</small><b>${esc(this.fleetStatus(fleet))}</b></span>` +
        `<span><small>Information delay</small><b>${fmt(fleet.age, 1)}s</b></span>` +
        `<span><small>Drive</small><b>${esc(this.driveLabel(fleet))}</b></span>` +
        `<span><small>Fuel</small><b>${fleet.fuel == null ? "—" : `${fmt(fleet.fuel)}/${fmt(fleet.fuel_capacity ?? 0)}`}</b></span></div>` +
        `<section class="m-section"><h3>Formation</h3><p>${esc(composition)}</p></section>` +
        `<section class="m-section"><h3>Cargo</h3><div class="m-ledger">${cargo}</div></section>` +
        ownControls +
        `<section class="m-section"><h3>Orders</h3>${orderRows}</section>`,
    };
  }

  private shipControls(fleet: GhostView): string {
    const disabled = fleet.docked ? "disabled" : "";
    const jump = jumpCapable(fleet)
      ? `<button type="button" data-mobile-act="fleet-jump" data-id="${esc(fleet.id)}" ${disabled}>Jump</button>` : "";
    const guard = guardCapable(fleet)
      ? `<button type="button" data-mobile-act="fleet-guard" data-id="${esc(fleet.id)}" ${disabled}>Guard</button>` : "";
    const recall = this.ctx.state.raids[fleet.id]
      ? `<button type="button" data-mobile-act="fleet-recall" data-id="${esc(fleet.id)}">Recall</button>` : "";
    const unload = fleet.docked && fleetCargoUnits(fleet) > 0
      ? `<button type="button" data-mobile-act="fleet-unload" data-id="${esc(fleet.id)}">Unload</button>` : "";
    const rescue = fleet.stalled && !fleet.rescue_inbound
      ? `<button type="button" data-mobile-act="fleet-rescue" data-id="${esc(fleet.id)}">Call AAA Rescue</button>` : "";
    return `<section class="m-section"><h3>Command</h3><div class="m-action-grid">` +
      `<button type="button" class="m-primary" data-mobile-act="fleet-move" data-id="${esc(fleet.id)}" ${disabled}>Move</button>` +
      jump + guard + recall + unload + rescue +
      `<button type="button" data-mobile-act="fleet-transit" data-id="${esc(fleet.id)}" data-mode="full">Full speed</button>` +
      `<button type="button" data-mobile-act="fleet-transit" data-id="${esc(fleet.id)}" data-mode="stealth">Stealth</button>` +
      `</div><small class="m-hint">Select an Interceptor, then tap a rival to raid. Long-press the rival to destroy.</small></section>`;
  }

  private renderSystem(entry: SheetEntry): SheetView {
    const props = propsOf<{ id: string }>(entry);
    const mode = this.ctx.renderer.viewMode;
    const id = props.id ?? this.ctx.state.selectedSystemId ?? (mode.type === "system" ? mode.systemId : undefined);
    const fixed = id ? this.ctx.state.galaxy?.systems.find((system) => system.id === id) : undefined;
    const dynamic = id ? this.ctx.state.systems.find((system) => system.id === id) : undefined;
    if (!fixed) return { title: "System", eyebrow: "System view", html: `<div class="m-empty">Select a star system on the map.</div>` };
    const mine = dynamic?.owner === this.ctx.state.playerId;
    const semantic = this.ctx.renderer.viewMode.type === "system" && this.ctx.renderer.viewMode.systemId === fixed.id;
    const stock = mine && dynamic?.stockpile?.length
      ? dynamic.stockpile.map((slot) => `<span>${esc(human(slot.commodity))}<b>${slot.units}</b></span>`).join("")
      : `<span class="m-muted">${mine ? "Stockpile empty" : "Owner-only"}</span>`;
    const bodies = (dynamic?.bodies ?? []).map((body) =>
      `<div class="m-body-row"><span><b>${esc(body.name)}</b><small>${esc(human(body.size))} · ${esc(human(body.environment))}</small></span>` +
      `<em>${body.geology ? esc(human(body.geology)) : "unsurveyed"}</em></div>`,
    ).join("");
    const nearby = this.ctx.state.ghosts.filter((fleet) => fleet.own && (
      fleet.docked === fixed.id || (!fleet.docked && Math.hypot(fleet.pos.x - fixed.pos.x, fleet.pos.y - fixed.pos.y) <= (this.ctx.state.galaxy?.hyperlimit ?? 900))
    ));
    const fleetRows = nearby.map((fleet) => this.fleetRow(fleet)).join("");
    return {
      title: fixed.name,
      eyebrow: `${fixed.band.toUpperCase()} band · ${mine ? "your holding" : dynamic?.owner ? "rival holding" : "unclaimed"}`,
      html: `<div class="m-action-grid m-action-grid--top">` +
        (semantic
          ? `<button type="button" class="m-primary" data-mobile-act="semantic-exit">Back to galaxy</button>`
          : `<button type="button" class="m-primary" data-mobile-act="system-enter" data-id="${esc(fixed.id)}">Enter system</button>`) +
        `</div>` +
        (mine && dynamic ? `<div class="m-stat-grid"><span><small>Population</small><b>${fmt(dynamic.population, 2)}M</b></span>` +
          `<span><small>Workforce</small><b>${dynamic.workforce ? `${dynamic.workforce.posted}/${dynamic.workforce.units}` : "—"}</b></span>` +
          `<span><small>Storage</small><b>${dynamic.storage_used}/${dynamic.storage_cap}</b></span>` +
          `<span><small>Slots</small><b>${dynamic.slots_used}/${dynamic.slots_total}</b></span></div>` : "") +
        `<section class="m-section"><h3>Stockpile</h3><div class="m-ledger">${stock}</div></section>` +
        `<section class="m-section"><h3>Worlds</h3>${bodies || `<div class="m-muted">No body report.</div>`}</section>` +
        (fleetRows ? `<section class="m-section"><h3>Fleets here</h3><div class="m-list">${fleetRows}</div></section>` : ""),
    };
  }

  private renderMarket(): SheetView {
    const tabs = (["exchange", "warehouse", "specialists", "modules"] as MarketTab[]).map((tab) =>
      `<button type="button" data-act="mtab:${tab}" data-mobile-act="market-tab" data-tab="${tab}" aria-selected="${this.marketTab === tab}">${human(tab)}</button>`,
    ).join("");
    const body = this.marketTab === "exchange" ? this.renderExchange()
      : this.marketTab === "warehouse" ? this.renderWarehouse()
        : this.marketTab === "specialists" ? this.renderSpecialists()
          : this.renderModules();
    return {
      title: "Market Hub",
      eyebrow: `Observed ${fmt(this.ctx.state.market?.staleness ?? 0, 1)}s delayed`,
      html: `<div class="m-subtabs" role="tablist">${tabs}</div>${body}`,
    };
  }

  private renderExchange(): string {
    const market = this.ctx.state.market;
    if (!market) return `<div class="m-empty">Waiting for a Market Hub report.</div>`;
    const rows = market.prices.map((quote) => {
      const held = warehouseUnits(quote.commodity);
      return `<button type="button" class="m-market-row${this.marketCommodity === quote.commodity ? " is-active" : ""}" data-act="commodity:${quote.commodity}" data-mobile-act="market-commodity" data-commodity="${quote.commodity}">` +
        `<span><b>${esc(human(quote.commodity))}</b><small>warehouse ${held}</small></span><em>${fmt(quote.price, 2)} cr</em></button>`;
    }).join("");
    const selected = market.prices.find((quote) => quote.commodity === this.marketCommodity);
    const open = this.ctx.state.wallet?.orders ?? [];
    const openRows = open.map((order) => `<div class="m-order"><span><b>${human(order.side)} ${order.units} ${human(order.commodity)}</b><small>limit ${fmt(order.limit_price, 2)}</small></span>` +
      `<button type="button" data-mobile-act="market-cancel" data-id="${order.id}">Cancel</button></div>`).join("");
    const incoming = marketReservations.map((order) => `<div class="m-order"><b>${human(order.side)} ${order.orderUnits} ${human(order.commodity)}</b><span>signal in flight</span></div>`).join("");
    const recent = recentMarketOrders.map((order) => `<div class="m-order"><b>${human(order.side)} ${order.units} ${human(order.commodity)}</b><span>${fmt(order.unitPrice, 2)} cr</span></div>`).join("");
    return `<div class="m-market-layout"><div class="m-market-board">${rows}</div>` +
      `<section class="m-trade-card"><h3>${human(this.marketSide)} ${human(this.marketCommodity)}</h3>` +
      `<div class="m-segment"><button type="button" data-mobile-act="market-side" data-side="buy" aria-pressed="${this.marketSide === "buy"}">Buy</button>` +
      `<button type="button" data-mobile-act="market-side" data-side="sell" aria-pressed="${this.marketSide === "sell"}">Sell</button></div>` +
      `<label>Quantity<input id="m-market-qty" type="number" inputmode="numeric" min="1" value="1"></label>` +
      `<p>${selected ? `Observed ${fmt(selected.price, 2)} cr · liquidity ${this.marketSide === "buy" ? selected.available_buy : selected.available_sell}` : "No quote"}</p>` +
      `<button type="button" class="m-primary" data-mobile-act="market-submit">Send ${human(this.marketSide)} order</button></section></div>` +
      `<section class="m-section"><h3>Open orders</h3>${openRows || `<div class="m-muted">None.</div>`}</section>` +
      `<section class="m-section"><h3>Incoming orders</h3>${incoming || `<div class="m-muted">None in flight.</div>`}</section>` +
      `<section class="m-section"><h3>Recent orders</h3>${recent || `<div class="m-muted">No execution receipts yet.</div>`}</section>`;
  }

  private renderWarehouse(): string {
    const holdings = COMMODITIES.map((commodity) => ({ commodity, units: warehouseUnits(commodity) })).filter((row) => row.units > 0);
    const ledger = holdings.length
      ? holdings.map((row) => `<span>${human(row.commodity)}<b>${row.units}</b></span>`).join("")
      : `<span class="m-muted">Warehouse empty</span>`;
    const terms = this.ctx.state.freight?.terms ?? [];
    const systems = terms.map((term) => {
      const name = this.ctx.state.galaxy?.systems.find((system) => system.id === term.system)?.name ?? term.system;
      return `<option value="${esc(term.system)}">${esc(name)} · ${fmt(term.secs_out)}s</option>`;
    }).join("");
    const commodities = COMMODITIES.map((commodity) => `<option value="${commodity}">${human(commodity)}</option>`).join("");
    const shipments = (this.ctx.state.freight?.shipments ?? []).map((shipment) =>
      `<div class="m-order"><b>${shipment.units} ${human(shipment.commodity)}</b><span>${shipment.direction} · ${shipment.aboard ? "aboard" : "queued"}</span></div>`,
    ).join("");
    const docked = this.ctx.state.ghosts.filter((fleet) => fleet.own && fleet.docked === "hub").map((fleet) => this.fleetRow(fleet)).join("");
    return `<section class="m-section"><h3>Market Warehouse</h3><div class="m-ledger">${ledger}</div></section>` +
      `<section class="m-trade-card"><h3>Authority freight</h3><div class="m-segment">` +
      `<button type="button" data-mobile-act="freight-direction" data-direction="outbound" aria-pressed="${this.freightDirection === "outbound"}">Hub → system</button>` +
      `<button type="button" data-mobile-act="freight-direction" data-direction="inbound" aria-pressed="${this.freightDirection === "inbound"}">System → hub</button></div>` +
      `<label>System<select id="m-freight-system">${systems}</select></label>` +
      `<label>Commodity<select id="m-freight-commodity">${commodities}</select></label>` +
      `<label>Units<input id="m-freight-qty" type="number" min="1" inputmode="numeric" value="1"></label>` +
      (this.freightDirection === "inbound" ? `<label class="m-check"><input id="m-freight-sell" type="checkbox"> Sell on arrival</label>` : "") +
      `<button type="button" class="m-primary" data-mobile-act="freight-submit" ${terms.length ? "" : "disabled"}>Book shipment</button></section>` +
      `<section class="m-section"><h3>Shipments</h3>${shipments || `<div class="m-muted">No Authority shipments booked.</div>`}</section>` +
      (docked ? `<section class="m-section"><h3>Hub berths</h3><div class="m-list">${docked}</div></section>` : "");
  }

  private renderSpecialists(): string {
    const cost = this.ctx.state.galaxy?.specialist_hire_cost ?? 800;
    const canHire = !!this.homeSystemId() && spendableMarketCredits() >= cost;
    return `<div class="m-list">${SPECIALISTS.map(([slug, name, role]) =>
      `<div class="m-service-row"><span><b>${name}</b><small>${role}</small></span><button type="button" data-mobile-act="hire-specialist" data-specialist="${slug}" ${canHire ? "" : "disabled"}>Hire · ${fmt(cost)} cr</button></div>`,
    ).join("")}</div><p class="m-hint">Contracts ship to your home system on a raidable personnel liner.</p>`;
  }

  private renderModules(): string {
    const home = this.homeSystem();
    const held = home?.modules ?? {};
    return `<div class="m-list">${MODULES.map((module) => {
      const value = moduleRecipeValue(module.kind);
      const buy = value === null ? null : value * MODULE_BUY_MULT;
      const sell = value === null ? null : value * MODULE_SELL_MULT;
      return `<div class="m-service-row"><span><b>${module.name}</b><small>${module.role} · held ${held[module.kind] ?? 0}</small></span>` +
        `<div><button type="button" data-mobile-act="module-buy" data-module="${module.kind}" ${buy !== null && spendableMarketCredits() >= buy && home ? "" : "disabled"}>Buy ${buy === null ? "—" : `~${fmt(buy)}`}</button>` +
        `<button type="button" data-mobile-act="module-sell" data-module="${module.kind}" ${(held[module.kind] ?? 0) > 0 ? "" : "disabled"}>Sell ${sell === null ? "—" : `~${fmt(sell)}`}</button></div></div>`;
    }).join("")}</div><p class="m-hint">Sol modules ship as physical crates; local manufacture remains cheaper.</p>`;
  }

  private renderCheckin(): SheetView {
    const decisions = this.renderDecisions();
    const timeline = this.ctx.state.timeline.slice().reverse().map((entry) =>
      `<div class="m-log-row is-${entry.severity}"><span>${esc(entry.text)}</span><time>${fmt(Math.max(0, liveSimTime() - entry.at_time))}s ago</time></div>`,
    ).join("");
    return {
      title: "Check-in",
      eyebrow: "Decision inbox · arrived reports",
      html: `<section class="m-section m-section--first"><h3>Decision inbox</h3>${decisions || `<div class="m-empty m-empty--good">No urgent decisions.</div>`}</section>` +
        `<section class="m-section"><h3>Recent log</h3>${timeline || `<div class="m-muted">No reports yet.</div>`}</section>`,
    };
  }

  private renderDecisions(): string {
    const rows: string[] = [];
    const founding = this.ctx.state.founding;
    if (founding && founding.stage !== "complete") {
      const key = `founding:${founding.stage}`;
      if (!this.dismissedDecisions.has(key)) {
        const content = foundingCopy(founding.stage);
        rows.push(`<article class="m-decision"><b>${esc(content.title)}</b><p>${esc(content.copy)}</p><div>` +
          `<button type="button" data-mobile-act="decision-dismiss" data-key="${esc(key)}">Dismiss</button>` +
          `<button type="button" class="m-primary" data-mobile-act="decision-founding" data-kind="${content.action}">${esc(content.label)}</button></div></article>`);
      }
    }
    for (const proposal of this.ctx.state.diplomacy?.incoming ?? []) {
      const key = `treaty:${proposal.id}`;
      if (this.dismissedDecisions.has(key)) continue;
      rows.push(`<article class="m-decision"><b>${esc(proposal.name)} proposes ${esc(human(proposal.treaty))}</b><p>A formal diplomatic change awaits your answer.</p>` +
        `<div><button type="button" data-mobile-act="decision-treaty" data-id="${proposal.id}" data-answer="decline">Decline</button>` +
        `<button type="button" class="m-primary" data-mobile-act="decision-treaty" data-id="${proposal.id}" data-answer="accept">Accept</button></div></article>`);
    }
    for (const invite of this.ctx.state.syndicateInvites) {
      const key = `syndicate:${invite.id}`;
      if (this.dismissedDecisions.has(key)) continue;
      rows.push(`<article class="m-decision"><b>Join ${esc(invite.name)}?</b><p>A syndicate invitation awaits.</p><div>` +
        `<button type="button" data-mobile-act="decision-dismiss" data-key="${esc(key)}">Later</button>` +
        `<button type="button" class="m-primary" data-mobile-act="decision-syndicate" data-id="${esc(invite.id)}">Join</button></div></article>`);
    }
    for (const operation of this.ctx.state.operations.filter((candidate) => candidate.state === "offered" && !candidate.joined).slice(0, 3)) {
      const key = `operation:${operation.id}`;
      if (this.dismissedDecisions.has(key)) continue;
      rows.push(`<article class="m-decision"><b>${esc(human(operation.issuer))} operation offered</b><p>${esc(human(operation.kind.kind))} · reward ${fmt(operation.reward.credits)} credits.</p><div>` +
        `<button type="button" data-mobile-act="decision-dismiss" data-key="${esc(key)}">Later</button>` +
        `<button type="button" class="m-primary" data-mobile-act="decision-operation" data-id="${esc(operation.id)}">Accept</button></div></article>`);
    }
    for (const battle of this.ctx.state.battles.filter((candidate) => candidate.own)) {
      const ownFleet = this.ctx.state.ghosts.find((fleet) => fleet.own && battle.participants.includes(fleet.id));
      const key = `battle:${battle.id}`;
      if (this.dismissedDecisions.has(key)) continue;
      rows.push(`<article class="m-decision is-danger"><b>Your fleet is engaged</b><p>Battle light is arriving from the observed theater.</p><div>` +
        `<button type="button" data-mobile-act="battle-open" data-id="${esc(battle.id)}">Open</button>` +
        (ownFleet ? `<button type="button" data-mobile-act="decision-withdraw" data-id="${esc(ownFleet.id)}">Withdraw</button>` : "") +
        `</div></article>`);
    }
    return rows.join("");
  }

  private renderBattle(entry: SheetEntry): SheetView {
    const id = propsOf<{ id: string }>(entry).id;
    const battle = this.ctx.state.battles.find((candidate) => candidate.id === id);
    if (!battle) return { title: "Battle", eyebrow: "Observed theater", html: `<div class="m-empty">This battle has left the current served picture.</div>` };
    const ownFleet = this.ctx.state.ghosts.find((fleet) => fleet.own && battle.participants.includes(fleet.id));
    const semantic = this.ctx.renderer.viewMode.type === "battle";
    return {
      title: "Battle underway",
      eyebrow: `Started ${fmt(Math.max(0, liveSimTime() - battle.started_at))}s ago · delayed`,
      html: `<div class="m-stat-grid"><span><small>Contacts</small><b>${battle.participants.length}</b></span><span><small>Round light</small><b>arriving</b></span></div>` +
        `<div class="m-action-grid m-action-grid--top">` +
        (ownFleet ? `<button type="button" data-mobile-act="decision-withdraw" data-id="${esc(ownFleet.id)}">Withdraw fleet</button>` : "") +
        (semantic ? `<button type="button" class="m-primary" data-mobile-act="semantic-exit">Back to galaxy</button>` : "") +
        `</div>`,
    };
  }

  private submitMarketOrder(): void {
    const qty = Math.max(1, Math.floor(Number(element<HTMLInputElement>("m-market-qty")?.value) || 0));
    const quote = this.ctx.state.market?.prices.find((row) => row.commodity === this.marketCommodity);
    if (!quote) return;
    const average = marketAverageQuote(quote.price, qty, this.marketSide);
    const protection = average * (this.marketSide === "buy" ? 1 + MARKET_PROTECTION_FRAC : 1 - MARKET_PROTECTION_FRAC);
    if (this.marketSide === "buy") {
      this.ctx.send({ type: "MarketBuy", commodity: this.marketCommodity, units: qty, max_unit_price: protection });
    } else {
      this.ctx.send({ type: "MarketSell", commodity: this.marketCommodity, units: qty, min_unit_price: protection });
    }
    reserveMarketOrder({
      kind: "market",
      side: this.marketSide,
      commodity: this.marketCommodity,
      orderUnits: qty,
      units: this.marketSide === "sell" ? qty : 0,
      credits: this.marketSide === "buy" ? qty * protection * (1 + (this.ctx.state.charter?.market_penalty_frac ?? 0)) : 0,
    });
    this.hooks.notice(`${human(this.marketSide)} order sent to the Market Hub.`);
    this.sheets.refresh();
  }

  private submitFreight(): void {
    const system = element<HTMLSelectElement>("m-freight-system")?.value;
    const commodity = element<HTMLSelectElement>("m-freight-commodity")?.value;
    const units = Math.max(1, Math.floor(Number(element<HTMLInputElement>("m-freight-qty")?.value) || 0));
    if (!system || !isCommodity(commodity)) return;
    if (this.freightDirection === "outbound") {
      this.ctx.send({ type: "BookFreightOut", system, commodity, units });
    } else {
      this.ctx.send({ type: "BookFreightIn", system, commodity, units, sell_on_arrival: !!element<HTMLInputElement>("m-freight-sell")?.checked });
    }
    this.hooks.notice("Authority freight booking dispatched.");
  }

  private hireSpecialist(specialist?: string): void {
    const home = this.homeSystemId();
    if (!specialist || !home) return;
    this.ctx.send({ type: "HireSpecialist", specialist, dest_system: home });
    this.hooks.notice(`${human(specialist)} contract dispatched.`);
  }

  private tradeModule(module: string | undefined, buy: boolean): void {
    if (!MODULES.some((candidate) => candidate.kind === module)) return;
    const kind = module as ModuleKind;
    const home = this.homeSystemId();
    if (!home) return;
    if (buy) this.ctx.send({ type: "BuyModule", module: kind, n: 1, dest_system: home });
    else this.ctx.send({ type: "SellModule", module: kind, n: 1, from_system: home });
    this.hooks.notice(`${buy ? "Buy" : "Sell"} order dispatched for ${human(kind)}.`);
  }

  private answerTreaty(idValue: string | undefined, accept: boolean): void {
    const id = Number(idValue);
    if (!Number.isFinite(id)) return;
    this.ctx.send({ type: "RespondTreaty", proposal_id: id, accept });
    this.hooks.notice(`Treaty ${accept ? "accepted" : "declined"}.`);
  }

  private unloadFleet(id?: string): void {
    const fleet = this.ownFleet(id);
    if (!fleet?.docked) return;
    if (fleet.docked === "hub") this.ctx.send({ type: "HubUnload", fleet_id: fleet.id });
    else this.ctx.send({ type: "SystemUnload", fleet_id: fleet.id, system: fleet.docked });
    this.hooks.notice("Unload order dispatched.");
  }

  private runFoundingAction(action: string): void {
    const founding = this.ctx.state.founding;
    if (!founding) return;
    if (action === "market") {
      this.marketTab = "warehouse";
      this.hooks.openSheet({ id: "market" });
      return;
    }
    if (action === "research") {
      this.hooks.openSheet({ id: "research" });
      return;
    }
    if (action === "candidate") {
      const id = founding.survey_candidates[0];
      if (id) this.hooks.focusSystem(id);
      return;
    }
    if (action === "interceptor" && founding.interceptor) {
      this.hooks.focusFleet(founding.interceptor);
      return;
    }
    if (action === "privateer" && founding.privateer) {
      this.hooks.focusFleet(founding.privateer);
      return;
    }
    if (["freighter", "scout", "colony"].includes(action)) {
      const kind = action === "freighter" ? "convoy" : action;
      const fleet = this.ctx.state.ghosts.find((ghost) => ghost.own && ghost.composition?.some((stack) => stack.kind === kind));
      if (fleet) this.hooks.focusFleet(fleet.id);
      return;
    }
    const home = foundingHomeSystemId() ?? this.homeSystemId();
    if (home) this.hooks.focusSystem(home);
  }

  private fleetStatus(fleet: GhostView): string {
    if (fleet.docked === "hub") return "docked · Market Hub";
    if (fleet.docked) {
      const name = this.ctx.state.galaxy?.systems.find((system) => system.id === fleet.docked)?.name ?? fleet.docked;
      return `docked · ${name}`;
    }
    if (fleet.rescue_inbound) return "AAA rescue inbound";
    if (fleet.stalled) return "out of fuel";
    if (fleet.jump_spool) return fleet.jump_spool.waiting_for_fuel ? "jump waiting for fuel" : `jump spooling ${fmt(fleet.jump_spool.remaining)}s`;
    if (fleet.guard_target) return "guarding";
    return Math.hypot(fleet.vel.x, fleet.vel.y) < 0.5 ? "holding" : "under way";
  }

  private driveLabel(fleet: GhostView): string {
    if (fleet.jump_spool) return `Jump Drive Spooling · ${fmt(fleet.jump_spool.remaining)}s`;
    if (!fleet.drive) return Math.hypot(fleet.vel.x, fleet.vel.y) < 0.5 ? "Impulse" : "Warp";
    if (fleet.drive === "thrusters") return "Impulse";
    if ("cruising" in fleet.drive) return fleet.drive.cruising === "warp" ? "Warp" : "Impulse";
    if ("spooling" in fleet.drive) return `Spooling ${human(fleet.drive.spooling.to)} · ${fmt(fleet.drive.spooling.left)}s`;
    return `Dropping from ${human(fleet.drive.dropping.from)} · ${fmt(fleet.drive.dropping.left)}s`;
  }

  private ownFleet(id?: string): GhostView | undefined {
    return id ? this.ctx.state.ghosts.find((fleet) => fleet.id === id && fleet.own) : undefined;
  }

  private homeSystemId(): string | undefined {
    return foundingHomeSystemId() ?? this.ctx.state.systems.find((system) => system.owner === this.ctx.state.playerId)?.id;
  }

  private homeSystem() {
    const id = this.homeSystemId();
    return id ? this.ctx.state.systems.find((system) => system.id === id) : undefined;
  }
}

const FOUNDING_STEPS = {
  build_shipyard: 1, build_mine: 2, build_convoy: 3, export_production: 4,
  defeat_privateer: 5, complete_export: 6, build_academy: 7, first_research: 8,
  build_scout: 9, survey_candidates: 10, build_colony: 11, establish_colony: 12,
  complete: 12,
} as const;

function foundingCopy(stage: keyof typeof FOUNDING_STEPS): { title: string; copy: string; action: string; label: string } {
  switch (stage) {
    case "build_shipyard": return { title: "Build Shipyard I", copy: "Open your home system and establish orbital shipbuilding.", action: "home", label: "Open home" };
    case "build_mine": return { title: "Build and staff a Mining Complex", copy: "Mine Metallic Ore on the designated world, then assign workforce.", action: "home", label: "Open home" };
    case "build_convoy": return { title: "Build your first Freighter", copy: "Use the founding kit at your Shipyard.", action: "home", label: "Open home" };
    case "export_production": return { title: "Dispatch the opening export", copy: "Load Provisions and Metallic Ore, then send the Freighter.", action: "freighter", label: "Select Freighter" };
    case "defeat_privateer": return { title: "Guard the Freighter", copy: "Intercept the Rogue Privateer before it reaches the civilian hull.", action: "privateer", label: "Select Privateer" };
    case "complete_export": return { title: "Complete the export", copy: "Deliver and sell both opening goods at the Market Hub.", action: "freighter", label: "Select Freighter" };
    case "build_academy": return { title: "Establish and staff Academy I", copy: "Import its kit, build it at home, and assign workforce.", action: "market", label: "Open Warehouse" };
    case "first_research": return { title: "Choose a first programme", copy: "Complete any Tier I corporate research programme.", action: "research", label: "Open Research" };
    case "build_scout": return { title: "Build a Scout", copy: "Import its kit and construct the exploration hull.", action: "market", label: "Open Warehouse" };
    case "survey_candidates": return { title: "Survey both prospects", copy: "Use the Scout to reveal their economic specialities.", action: "scout", label: "Select Scout" };
    case "build_colony": return { title: "Build a Colony Ship", copy: "Compare the reports, import the kit, and construct the hull.", action: "market", label: "Open Warehouse" };
    case "establish_colony": return { title: "Establish your second holding", copy: "Choose a prospect and send the Colony Ship.", action: "candidate", label: "Compare prospects" };
    case "complete": return { title: "Founding complete", copy: "Your corporation is ready for independent expansion.", action: "home", label: "Open home" };
  }
}

function isCommodity(value: string | undefined): value is Commodity {
  return !!value && COMMODITIES.includes(value as Commodity);
}

function isMarketTab(value: string | undefined): value is MarketTab {
  return value === "exchange" || value === "warehouse" || value === "specialists" || value === "modules";
}

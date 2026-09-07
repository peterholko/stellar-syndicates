import { triggerLabel } from "../../core/derive/format";
import { allySystems, endpointLabel, ownedSystems } from "../../core/derive/geo";
import { COMMODITIES } from "../../core/derive/market";
import { commodityIcon, icon, label } from "../../icons";
import type {
  Commodity,
  FleetDoctrine,
  StandingEndpoint,
  StandingOrder,
  StandingTrigger,
} from "../../protocol";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

interface PolicyHooks {
  notice(html: string): void;
}

type DoctrineField = {
  key: keyof FleetDoctrine;
  title: string;
  copy: string;
  options: readonly (readonly [string, string])[];
};

const DOCTRINE_FIELDS: readonly DoctrineField[] = [
  {
    key: "engagement",
    title: "Engagement",
    copy: "When an autonomous combat fleet may initiate contact.",
    options: [
      ["avoid", "Avoid — never engage"],
      ["defensive_only", "Defensive only"],
      ["engage_weaker", "Engage weaker fleets"],
      ["engage_any", "Engage any sensed hostile"],
    ],
  },
  {
    key: "retreat",
    title: "Retreat threshold",
    copy: "The strength comparison at which an autonomous fleet breaks away.",
    options: [
      ["quarter", "Retreat if outnumbered about 3:1"],
      ["half", "Retreat if outnumbered"],
      ["three_quarter", "Hold only with a clear edge"],
      ["never", "Never retreat"],
    ],
  },
  {
    key: "escort",
    title: "Escort priority",
    copy: "Which eligible freighter an unassigned escort protects.",
    options: [
      ["guard_nearest", "Guard nearest freighter"],
      ["guard_richest", "Guard richest freighter"],
      ["hold_station", "Hold station"],
    ],
  },
  {
    key: "destination_invalid",
    title: "Lost destination",
    copy: "What a supply fleet does if its destination becomes invalid.",
    options: [
      ["drop", "Drop cargo"],
      ["return_home", "Return cargo home"],
      ["sell_at_hub", "Sell cargo at the Market Hub"],
    ],
  },
];

/** Corporate policies are local administrative commands, but their execution
 * remains physical: standing logistics still requires an idle cargo fleet and
 * doctrine only governs autonomous decisions. Direct fleet orders win. */
export class DeckPolicyRoutes {
  private source = "";
  private destination = "hub";
  private commodity: Commodity = "provisions";
  private triggerKind: StandingTrigger["kind"] = "above_threshold";
  private amount = 100;
  private floor = 50;
  private sellOnArrival = true;
  private logisticsFeedback = "";
  private doctrineFeedback = "";
  private signature = "";
  private appliedPreset = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: PolicyHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "logistics" && route?.name !== "doctrine") return false;
    if (route.name === "logistics") this.applyRoutePreset(route);
    this.repairForm();
    const state = this.ctx.state;
    const signature = sheetFingerprint([
      route, this.source, this.destination, this.commodity, this.triggerKind,
      this.amount, this.floor, this.sellOnArrival, this.logisticsFeedback, this.doctrineFeedback,
      state.standingOrders, state.doctrine,
      state.systems.map((system) => [system.id, system.owner, system.ally]),
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, route.name === "logistics" ? this.logisticsHtml() : this.doctrineHtml());
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "logistics" && route?.name !== "doctrine") return false;
    const action = button.dataset.deckAct;
    if (route.name === "logistics" && action === "standing-clear") {
      const id = Number(button.dataset.order);
      if (!Number.isFinite(id)) return true;
      this.ctx.send({ type: "ClearStandingOrder", order_id: id });
      this.logisticsFeedback = `Removal sent for standing order #${id}.`;
      this.hooks.notice(`<b>Standing order removal sent</b> · #${id}.`);
    } else if (route.name === "logistics" && action === "standing-add") {
      this.addStandingOrder();
    } else {
      return false;
    }
    this.signature = "";
    this.render(route, true);
    return true;
  }

  handleInput(target: HTMLInputElement | HTMLSelectElement, route: DeckRoute | null): boolean {
    if (route?.name !== "logistics" && route?.name !== "doctrine") return false;
    const field = target.dataset.policyInput;
    if (!field) return false;
    if (route.name === "doctrine" && target instanceof HTMLSelectElement && isDoctrineKey(field)) {
      const doctrine = { ...this.ctx.state.doctrine } as FleetDoctrine;
      (doctrine as unknown as Record<string, string>)[field] = target.value;
      this.ctx.intent.beginFleetCommand({ type: "SetFleetDoctrine", doctrine });
      // Keep the select on the served setting until the player confirms and
      // the normal policy report returns; Cancel leaves it unchanged.
      target.value = String(this.ctx.state.doctrine[field]);
      this.doctrineFeedback = "";
    } else if (route.name === "logistics") {
      if (field === "standing-source") this.source = target.value;
      else if (field === "standing-destination") this.destination = target.value;
      else if (field === "standing-commodity" && isCommodity(target.value)) this.commodity = target.value;
      else if (field === "standing-trigger" && isTriggerKind(target.value)) this.triggerKind = target.value;
      else if (field === "standing-amount") this.amount = nonNegative(target.value, this.amount);
      else if (field === "standing-floor") this.floor = nonNegative(target.value, this.floor);
      else if (field === "standing-sell" && target instanceof HTMLInputElement) this.sellOnArrival = target.checked;
      else return false;
    } else {
      return false;
    }
    this.signature = "";
    this.render(route, true);
    return true;
  }

  invalidate(): void {
    this.signature = "";
  }

  private logisticsHtml(): string {
    const owned = ownedSystems();
    const allies = allySystems();
    const sourceOptions = owned.length
      ? owned.map((system) => option(system.id, system.name, system.id === this.source)).join("")
      : `<option value="">Claim a system first</option>`;
    const destinationOptions = option("hub", "Market Hub", this.destination === "hub")
      + option("home", "Home system", this.destination === "home")
      + owned.map((system) => option(system.id, `${system.name} · colony`, system.id === this.destination)).join("")
      + allies.map((system) => option(system.id, `${system.name} · ally aid`, system.id === this.destination)).join("");
    const commodityOptions = COMMODITIES.map((commodity) => option(commodity, label(commodity), commodity === this.commodity)).join("");
    const rows = this.ctx.state.standingOrders.map((order) => this.standingOrderHtml(order)).join("");
    const amountLabel = this.triggerKind === "above_threshold" ? "Source threshold"
      : this.triggerKind === "percent_surplus" ? "Surplus percent"
        : "Destination target";
    return `<section class="deck-page deck-logistics"><header class="deck-page__lead"><span>autonomous physical logistics</span><h2>Standing supply</h2><p>Rules run on the server while you are away, but each dispatch still needs an idle cargo fleet and travels through raidable space.</p></header>`
      + `${this.logisticsFeedback ? `<div class="deck-policy-feedback">${esc(this.logisticsFeedback)}</div>` : ""}`
      + `<section class="deck-section"><header><div><h3>Active rules</h3><p>Every route below is based on the corporation's served systems and alliances.</p></div><b>${this.ctx.state.standingOrders.length}</b></header><div class="deck-standing-list">${rows || `<div class="deck-empty-inline">No standing orders yet.</div>`}</div></section>`
      + `<section class="deck-section deck-standing-builder"><header><div><h3>New rule</h3><p>Choose a source condition and a physical destination.</p></div>${icon("freightRoute", "md")}</header>`
      + `<div class="deck-standing-route"><label>Source<select data-policy-input="standing-source">${sourceOptions}</select></label><span>→</span><label>Destination<select data-policy-input="standing-destination">${destinationOptions}</select></label></div>`
      + `<div class="deck-form-grid"><label>Commodity<select data-policy-input="standing-commodity">${commodityOptions}</select></label><label>Trigger<select data-policy-input="standing-trigger"><option value="above_threshold" ${this.triggerKind === "above_threshold" ? "selected" : ""}>Above source threshold</option><option value="percent_surplus" ${this.triggerKind === "percent_surplus" ? "selected" : ""}>Percent of surplus</option><option value="maintain_at_dest" ${this.triggerKind === "maintain_at_dest" ? "selected" : ""}>Maintain at destination</option></select></label><label>${amountLabel}<input data-policy-input="standing-amount" type="number" min="0" ${this.triggerKind === "percent_surplus" ? `max="100"` : ""} value="${this.amount}"></label>${this.triggerKind === "percent_surplus" ? `<label>Surplus floor<input data-policy-input="standing-floor" type="number" min="0" value="${this.floor}"></label>` : ""}</div>`
      + `${this.destination === "hub" ? `<label class="deck-check"><input data-policy-input="standing-sell" type="checkbox" ${this.sellOnArrival ? "checked" : ""}> Sell cargo when it reaches the Market Warehouse</label>` : ""}`
      + `<div class="deck-section__action"><button type="button" class="is-primary" data-deck-act="standing-add" ${owned.length ? "" : "disabled"}>Create standing order</button><span>Evaluation is periodic; an in-flight run must finish before the same rule dispatches again.</span></div></section></section>`;
  }

  private standingOrderHtml(order: StandingOrder): string {
    const state = order.status === "paused" ? "paused" : order.in_flight ? "freighter en route" : "idle · waiting for trigger";
    return `<article class="deck-standing-row${order.status === "paused" ? " is-paused" : ""}"><span>${commodityIcon(order.commodity)}<span><b>#${order.id} · ${esc(label(order.commodity))}</b><small>${esc(endpointLabel(order.source))} → ${esc(endpointLabel(order.dest))}</small><em>${esc(triggerLabel(order.trigger))} · ${esc(state)}</em></span></span><button type="button" data-deck-act="standing-clear" data-order="${order.id}">Remove</button></article>`;
  }

  private doctrineHtml(): string {
    const fields = DOCTRINE_FIELDS.map((field) => {
      const current = String(this.ctx.state.doctrine[field.key]);
      const options = field.options.map(([value, name]) => option(value, name, value === current)).join("");
      return `<label class="deck-doctrine-field"><span><b>${esc(field.title)}</b><small>${esc(field.copy)}</small></span><select data-policy-input="${field.key}">${options}</select></label>`;
    }).join("");
    return `<section class="deck-page deck-doctrine"><header class="deck-page__lead"><span>corporate standing policy</span><h2>Fleet doctrine</h2><p>Doctrine governs autonomous combat and logistics decisions. A direct fleet order always takes precedence.</p></header>${this.doctrineFeedback ? `<div class="deck-policy-feedback">${esc(this.doctrineFeedback)}</div>` : ""}<section class="deck-section"><header><div><h3>Default behavior</h3><p>Each selection sends the complete policy, preserving the other three decisions.</p></div>${icon("doctrine", "md")}</header><div class="deck-doctrine-grid">${fields}</div></section></section>`;
  }

  private addStandingOrder(): void {
    if (!this.source) return;
    const destination: StandingEndpoint = this.destination === "hub" ? { kind: "hub" }
      : this.destination === "home" ? { kind: "home" }
        : { kind: "system", id: this.destination };
    const trigger: StandingTrigger = this.triggerKind === "percent_surplus"
      ? { kind: "percent_surplus", percent: Math.max(1, Math.min(100, Math.round(this.amount))), floor: this.floor }
      : this.triggerKind === "maintain_at_dest"
        ? { kind: "maintain_at_dest", target: this.amount }
        : { kind: "above_threshold", threshold: this.amount };
    const order: StandingOrder = {
      id: 0,
      source: { kind: "system", id: this.source },
      dest: destination,
      commodity: this.commodity,
      trigger,
      status: "active",
      next_eval_tick: 0,
      in_flight: null,
      sell_on_arrival: this.destination === "hub" && this.sellOnArrival,
    };
    this.ctx.send({ type: "SetStandingOrder", order });
    this.logisticsFeedback = `Standing ${label(this.commodity)} route sent: ${endpointLabel(order.source)} → ${endpointLabel(order.dest)}.`;
    this.hooks.notice(`<b>Standing order sent</b> · ${esc(label(this.commodity))} · ${esc(triggerLabel(trigger))}.`);
  }

  private repairForm(): void {
    const owned = ownedSystems();
    if (!owned.some((system) => system.id === this.source)) this.source = owned[0]?.id ?? "";
    const validDestination = this.destination === "hub" || this.destination === "home"
      || owned.some((system) => system.id === this.destination)
      || allySystems().some((system) => system.id === this.destination);
    if (!validDestination) this.destination = "hub";
  }

  private applyRoutePreset(route: DeckRoute): void {
    const preset = JSON.stringify(route.query ?? {});
    if (!route.query || preset === this.appliedPreset) return;
    if (route.query.source) this.source = route.query.source;
    if (route.query.destination) this.destination = route.query.destination;
    if (isCommodity(route.query.commodity)) this.commodity = route.query.commodity;
    this.appliedPreset = preset;
  }
}

function isDoctrineKey(value: string): value is keyof FleetDoctrine {
  return DOCTRINE_FIELDS.some((field) => field.key === value);
}

function isCommodity(value: string): value is Commodity {
  return COMMODITIES.includes(value as Commodity);
}

function isTriggerKind(value: string): value is StandingTrigger["kind"] {
  return value === "above_threshold" || value === "percent_surplus" || value === "maintain_at_dest";
}

function nonNegative(value: string, fallback: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback;
}

function option(value: string, text: string, selected = false): string {
  return `<option value="${esc(value)}" ${selected ? "selected" : ""}>${esc(text)}</option>`;
}

function esc(value: string): string {
  return value.replace(/[&<>\"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!);
}

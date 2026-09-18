import { isPlayerFreighter } from "../../protocol";
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
  kitAffordable,
  kitCostLabel,
  warehouseUnits,
  utilityRefitChoices,
} from "../../core/derive/market";
import { dockedAtSystem, fleetCargoCapacity, guardCapable, jumpCapable, shipKindLabel, shipRoleLore } from "../../core/derive/fleet";
import { tenderStatus } from "../../core/derive/tenders";
import { fuelTransferHtml } from "../fueltransfer";
import { missionCommand, missionHtml, pirateFactionHtml } from "../mission";
import { fleetIndustryHtml } from "../industry";
import { fleetReadiness, dispatchWarnings } from "../../core/derive/readiness";
import { pirateSite } from "../../core/derive/pirates";
import { foundingHomeSystemId } from "../../core/derive/geo";
import { postVictoryHandoff } from "../../core/derive/handoff";
import { FOUNDING_STEP, FOUNDING_TOTAL, foundingBusinessGoals, foundingScoutGoal } from "../../core/derive/founding";
import { jumpRangeAt } from "../../core/derive/nebula";
import {
  countClassLabel,
  fleetCargoManifest,
  fleetCargoUnits,
  fleetExactCount,
  type Commodity,
  type EngagementEstimate,
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
import { sheetFingerprint } from "../signature";
import { requestTransactions } from "../../core/derive/transactions";
import { transactionsHtml } from "../transactions";
import { DEFAULT_DEFENSE_RADIUS, DEFENSE_RADII } from "../../core/derive/defense";
import { stageDefenseAction, systemDefenseHtml } from "../systemdefense";
import "../../styles/system-defense.css";

type MarketTab = "exchange" | "warehouse" | "specialists" | "modules" | "transactions";

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

import { MODULES, isBlueprintOnly } from "../../core/derive/equipment";

const SPECIALISTS = [
  ["geologist", "Geologist", "mineral extraction"],
  ["petrochemical_engineer", "Petrochemical Engineer", "volatiles and fuel"],
  ["xenobiologist", "Xenobiologist", "biomass and provisions"],
  ["industrial_engineer", "Industrial Engineer", "heavy industry"],
  ["naval_architect", "Naval Architect", "shipyards, hulls, drives and armaments"],
] as const;

const esc = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);
const human = (value: string): string => value === "metallic_ore" ? "Ferrite Ore" : value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
const safeId = (value: string): string => value.replace(/[^a-zA-Z0-9_-]/g, "_");
const fmt = (value: number, digits = 0): string => Number.isFinite(value)
  ? value.toLocaleString(undefined, { maximumFractionDigits: digits, minimumFractionDigits: digits })
  : "—";
const element = <T extends HTMLElement>(id: string): T | null => document.getElementById(id) as T | null;
const propsOf = <T extends object>(entry: SheetEntry): Partial<T> => (entry.props && typeof entry.props === "object" ? entry.props : {}) as Partial<T>;

export class MobileSurfaces {
  private readonly defenseRadius = new Map<string, number>();
  private readonly defenseAssigning = new Set<string>();
  private readonly renderSignatures = new Map<SheetEntry["id"], string>();
  private marketTab: MarketTab = "exchange";
  private marketCommodity: Commodity = "fuel";
  private marketSide: Side = "buy";
  private readonly dismissedDecisions = new Set<string>();
  private readonly engagementEstimates = new Map<string, EngagementEstimate>();
  private readonly estimateAttackerByTarget = new Map<string, string>();
  private readonly arrivedReports: { text: string; tone: "good" | "bad" | "warn"; at: number }[] = [];
  private foundingMinimized = false;

  constructor(
    private readonly ctx: CoreContext,
    private readonly sheets: SheetStack,
    private readonly hooks: SurfaceHooks,
  ) {
    try {
      const stored = localStorage.getItem(FOUNDING_MINIMIZED_KEY);
      this.foundingMinimized = stored === null || stored === "1";
    } catch {
      // Locked-down storage still gets the unobtrusive phone default.
      this.foundingMinimized = true;
    }
  }

  render(entry: SheetEntry): SheetView | null {
    let view: SheetView | null;
    switch (entry.id) {
      case "fleets": view = this.renderFleets(); break;
      case "ship": view = this.renderShip(entry); break;
      case "system": view = this.renderSystem(entry); break;
      case "market": view = this.renderMarket(); break;
      case "log": view = this.renderCheckin(); break;
      case "battle": view = this.renderBattle(entry); break;
      default: return null;
    }
    this.rememberSignature(entry);
    return view;
  }

  refreshNeeded(entry: SheetEntry): boolean | null {
    const signature = this.signature(entry);
    return signature === null ? null : this.renderSignatures.get(entry.id) !== signature;
  }

  onTrade(trade: TradeEvent): void {
    settleMarketReservation(trade);
    recordRecentMarketOrder(trade);
  }

  onEstimate(estimate: EngagementEstimate): void {
    this.engagementEstimates.set(this.estimateKey(estimate.attacker, estimate.target), estimate);
    this.estimateAttackerByTarget.set(estimate.target, estimate.attacker);
  }

  recordReport(text: string, tone: "good" | "bad" | "warn"): void {
    this.arrivedReports.push({ text, tone, at: liveSimTime() });
    if (this.arrivedReports.length > 20) this.arrivedReports.splice(0, this.arrivedReports.length - 20);
  }

  /** The unread badge keys the same live, served decisions as the Log sheet.
   * Keys are stable for the lifetime of a decision and carry no extra truth. */
  decisionKeys(): string[] {
    const keys: string[] = [];
    const founding = this.ctx.state.founding;
    if (founding && founding.stage !== "complete") keys.push(`founding:${founding.stage}`);
    for (const proposal of this.ctx.state.diplomacy?.incoming ?? []) keys.push(`treaty:${proposal.id}`);
    for (const invite of this.ctx.state.syndicateInvites) keys.push(`syndicate:${invite.id}`);
    for (const operation of this.ctx.state.operations.filter((candidate) => candidate.state === "offered" && !candidate.joined).slice(0, 3)) {
      keys.push(`operation:${operation.id}`);
    }
    for (const battle of this.ctx.state.battles.filter((candidate) => candidate.own)) keys.push(`battle:${battle.id}`);
    return keys.filter((key) => !this.dismissedDecisions.has(key));
  }

  refreshFounding(): void {
    const root = document.getElementById("m-founding");
    if (!root) return;
    if (renderDeferred("m-founding", () => this.refreshFounding())) return;
    const founding = this.ctx.state.founding;
    if (!founding || (founding.stage === "complete" && !founding.protected && !postVictoryHandoff().some(g => !g.done))) {
      root.hidden = true;
      return;
    }
    root.hidden = false;
    root.classList.toggle("is-minimized", this.foundingMinimized);
    const step = FOUNDING_STEP[founding.stage];
    const shield = founding.protected
      ? founding.protection_min_until > this.ctx.state.simTime
        ? `shield ${fmt(founding.protection_min_until - this.ctx.state.simTime)}s`
        : "shield active"
      : "shield ended";
    const content = founding.stage === "build_scout" ? foundingScoutGoal(this.homeSystem()) : foundingCopy(founding.stage);
    const home = this.ctx.state.systems.find(s => s.id === this.homeSystemId());
    const goals = founding.stage === "grow_business" ? foundingBusinessGoals(this.ctx.state, home) : [content];
    setHtml(root,
      `<header><span>Founding ${step}/${FOUNDING_TOTAL} · ${shield}</span>` +
      `<button type="button" data-mobile-act="founding-toggle" aria-label="${this.foundingMinimized ? "Expand" : "Minimize"} tutorial">${this.foundingMinimized ? "+" : "−"}</button></header>` +
      `<div class="m-founding__body"><b>${esc(content.title)}</b><p>${esc(content.copy)}</p>` +
      goals.map(goal => `<button type="button" class="m-primary" data-mobile-act="founding-action" data-kind="${goal.action}">${founding.stage === "grow_business" ? `${esc(goal.title)} · ` : ""}${esc(goal.label)}</button>`).join("") + `${founding.bounty_received ? `<button type="button" data-mobile-act="next-objectives">Next objectives &amp; rewards</button>` : ""}</div>`,
    );
  }

  handleClick(event: Event): boolean {
    const button = (event.target as Element).closest<HTMLElement>("[data-mobile-act]");
    if (!button) return false;
    const action = button.dataset.mobileAct;
    if (!action || action === "confirm-intent" || action === "cancel-intent") return false;
    if (action.startsWith("defense-")) {
      const system = button.dataset.system ?? "";
      if (!this.ctx.state.systems.some(s => s.id === system && s.owner === this.ctx.state.playerId)) return true;
      if (action === "defense-toggle") {
        if (this.defenseAssigning.has(system)) this.defenseAssigning.delete(system);
        else this.defenseAssigning.add(system);
        this.sheets.refresh();
      } else if (action === "defense-radius") {
        const radius = Number(button.dataset.radius);
        if (DEFENSE_RADII.some(r => r === radius)) this.defenseRadius.set(system, radius);
        this.sheets.refresh();
      } else if (action === "defense-open" && button.dataset.fleet) this.hooks.focusFleet(button.dataset.fleet);
      else if (action === "defense-log") this.hooks.openSheet({ id: "log" });
      else if (action === "defense-build") this.hooks.openSheet({ id: "build", props: { systemId: system } });
      else if (action === "defense-doctrine") this.hooks.openSheet({ id: "doctrine" });
      else stageDefenseAction(this.ctx, action, system, button.dataset.fleet ?? "", this.defenseRadius.get(system) ?? DEFAULT_DEFENSE_RADIUS);
      return true;
    }

    switch (action) {
      case "fleet-utility-refit": {
        const fleet = this.ctx.state.ghosts.find(g => g.id === button.dataset.id && g.own);
        const dock = fleet && this.ctx.state.systems.find(s => dockedAtSystem(fleet, s.id) && (s.owner === this.ctx.state.playerId || s.ally));
        const choice = fleet && dock ? utilityRefitChoices(fleet, dock).find(c =>
          c.ship === button.dataset.ship && c.from.join(",") === button.dataset.from && c.to.join(",") === button.dataset.to) : undefined;
        if (fleet && choice && !choice.blocked && !(this.ctx.state.pendingOrders.get(fleet.id)?.length)) {
          this.ctx.intent.beginFleetCommand({ type: "RefitShips", fleet_id: fleet.id,
            ship: choice.ship, from: choice.from, to: choice.to, n: 1 });
        }
        break;
      }
      case "fleet-select":
        if (button.dataset.id) this.hooks.focusFleet(button.dataset.id);
        break;
      case "fleet-mission": {
        const fleet = this.ownFleet(button.dataset.id);
        const command = fleet ? missionCommand(fleet, button.dataset.field, button.dataset.value) : null;
        if (command) this.ctx.intent.beginFleetCommand(command);
        break;
      }
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
          this.ctx.intent.beginFleetCommand({ type: "RecallRaid", raider_id: button.dataset.id });
        }
        break;
      case "fleet-transit":
        if (button.dataset.id && (button.dataset.mode === "full" || button.dataset.mode === "stealth")) {
          this.ctx.intent.beginFleetCommand({ type: "SetFleetTransit", fleet_id: button.dataset.id, mode: button.dataset.mode });
        }
        break;
      case "fleet-unload":
        this.unloadFleet(button.dataset.id);
        break;
      case "fleet-rescue":
        if (button.dataset.id) this.ctx.intent.beginFleetCommand({ type: "RequestFuelRescue", fleet_id: button.dataset.id });
        break;
      case "fleet-refuel": {
        const target = button.closest("[data-fuel-transfer]")?.querySelector<HTMLSelectElement>("[data-tender-target]")?.value;
        if (button.dataset.id && target) this.ctx.intent.beginFleetCommand({ type: "RefuelFleet", fleet_id: button.dataset.id, target_id: target });
        break;
      }
      case "fleet-estimate": {
        const target = button.dataset.target;
        const attacker = target ? element<HTMLSelectElement>(`m-estimate-attacker-${safeId(target)}`)?.value : undefined;
        if (attacker && target) {
          this.estimateAttackerByTarget.set(target, attacker);
          this.ctx.send({ type: "EstimateEngagement", attacker, target });
          this.hooks.notice("Engagement projection requested.");
        }
        break;
      }
      case "fleet-emplace": {
        const fleet = this.ownFleet(button.dataset.id);
        if (!fleet || fleet.kind !== "builder") break;
        const busy = !!fleet.job || this.ctx.state.pendingOrders.has(fleet.id) || Math.hypot(fleet.vel.x, fleet.vel.y) >= 0.5;
        const siteError = this.ctx.renderer.siteError("deep_space_sensor", fleet.pos, this.ctx.state);
        if (busy) this.hooks.notice("The Construction Ship must be idle before it can build.");
        else if (siteError) this.hooks.notice(`<span class="warn">Can't build here: ${esc(siteError)}</span>`);
        else if (!kitAffordable("deep_space_sensor")) this.hooks.notice(`<span class="warn">No single owned system can supply ${esc(kitCostLabel("deep_space_sensor"))}.</span>`);
        else {
          this.ctx.intent.beginFleetCommand({ type: "BuildEmplacement", builder: fleet.id, emplacement: "deep_space_sensor" });
        }
        break;
      }
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
      case "transactions-older":
      case "transactions-newer":
      case "transactions-latest":
        requestTransactions(this.ctx, button.dataset.mobileAct === "transactions-older" ? "older" : button.dataset.mobileAct === "transactions-newer" ? "newer" : "latest");
        this.sheets.refresh();
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
        if (button.dataset.id) this.ctx.intent.beginFleetCommand({ type: "Withdraw", fleet_id: button.dataset.id });
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
    const crates = Object.entries(fleet.modules ?? {}).filter(([, n]) => n > 0);
    const cargo = (manifest.length
      ? manifest.map((slot) => `<span>${esc(human(slot.commodity))}<b>${slot.units}</b></span>`).join("")
      : `<span class="m-muted">Hold empty</span>`) + (crates.length
        ? `<span>Module crates · store at an owned dock</span>${crates.map(([kind, n]) => `<span>${esc(human(kind))}<b>${n}</b></span>`).join("")}` : "");
    const orders = this.ctx.state.pendingOrders.get(fleet.id) ?? [];
    const orderRows = orders.length
      ? orders.map((order) => {
          const phase = liveSimTime() < order.arrives_at ? `signal · ${fmt(order.arrives_at - liveSimTime())}s` : "awaiting response";
          return `<div class="m-order"><b>${esc(human(order.kind))}</b><span>${phase}</span></div>`;
        }).join("")
      : `<div class="m-muted">No commands in flight.</div>`;
    const ownControls = fleet.own ? this.shipControls(fleet) : "";
    const readiness = fleetReadiness(fleet);
    const ready = fleet.own ? `<section class="m-section"><h3>Readiness</h3><div class="m-stat-grid">
      <span><small>Hull</small><b>${readiness.hull == null ? "Unknown" : `${Math.round(readiness.hull * 100)}%`}</b></span>
      <span><small>Cargo space</small><b>${readiness.cargoCapacity ? `${readiness.cargoFree} free / ${readiness.cargoCapacity}` : "No hold"}</b></span>
      <span><small>Captain</small><b>${esc(readiness.captain?.name ?? "Unassigned")}</b></span>
      <span><small>Assignment</small><b>${esc(this.fleetStatus(fleet))}</b></span></div>
      ${dispatchWarnings(fleet, fleet.path?.at(-1)?.pos).map(w => `<div class="m-warning">${esc(w)}</div>`).join("")}</section>` : "";
    const engagement = fleet.own ? "" : this.engagementSection(fleet);
    const roleLore = fleet.own ? shipRoleLore(fleet) : "";
    const supply = fleet.own && fleet.supplied === false
      ? `<div class="m-warning"><b>Out of provisions.</b> This fleet keeps its guns and current order, but cannot depart again until supplied.</div>`
      : "";
    return {
      title: `${shipKindLabel(fleet.kind)}${count && count > 1 ? " Fleet" : ""}`,
      eyebrow: fleet.own ? "Your served fleet picture" : fleet.pirate ? "Pirate contact" : "Observed contact",
      html: `<div class="m-stat-grid"><span><small>Status</small><b>${esc(this.fleetStatus(fleet))}</b></span>` +
        `<span><small>Information delay</small><b>${fmt(fleet.age, 1)}s</b></span>` +
        `<span><small>Drive</small><b>${esc(this.driveLabel(fleet))}</b></span>` +
        `<span><small>Fuel</small><b>${fleet.fuel == null ? "—" : `${fmt(fleet.fuel)}/${fmt(fleet.fuel_capacity ?? 0)}`}</b></span></div>` +
        `<section class="m-section"><h3>Formation</h3><p>${esc(composition)}</p>${roleLore ? `<details class="m-details m-help"><summary>Fleet role</summary><div><p>${esc(roleLore)}</p></div></details>` : ""}</section>` +
        ready + supply + fleetIndustryHtml(fleet, this.ctx.state) + this.utilityEquipment(fleet) + pirateFactionHtml(fleet, true) + missionHtml(fleet, orders, true) +
        `<section class="m-section"><h3>Cargo</h3><div class="m-ledger">${cargo}</div></section>` +
        engagement +
        ownControls + (fleet.own ? fuelTransferHtml(fleet, this.ctx.state, "", true) : "") +
        `<section class="m-section"><h3>Orders</h3>${orderRows}</section>`,
    };
  }

  private utilityEquipment(fleet: GhostView): string {
    if (!fleet.own) return "";
    const dock = this.ctx.state.systems.find(s => dockedAtSystem(fleet, s.id) && (s.owner === this.ctx.state.playerId || s.ally));
    const choices = dock ? utilityRefitChoices(fleet, dock) : [];
    const busy = (this.ctx.state.pendingOrders.get(fleet.id)?.length ?? 0) > 0 || !!fleet.path?.length || Math.hypot(fleet.vel.x, fleet.vel.y) >= .5;
    const installed = (fleet.loadouts ?? []).map(s => `${s.n}× ${shipKindLabel(s.kind)} · ${s.modules.map(human).join(" + ")}`).join(" · ");
    return `<section class="m-section"><h3>Equipment</h3>${installed ? `<p>${esc(installed)}</p>` : ""}${choices.map(c =>
      `<div class="m-service-row"><span><b>${shipKindLabel(c.ship)} · ${esc(c.name)}</b><small>${esc(c.preview)}</small></span><button type="button" data-mobile-act="fleet-utility-refit" data-id="${fleet.id}" data-ship="${c.ship}" data-from="${c.from.join(",")}" data-to="${c.to.join(",")}" ${busy || c.blocked ? "disabled" : ""}>Refit 1</button></div>`
    ).join("") || `<small class="m-muted">Utilities need a stocked, staffed Shipyard I.</small>`}</section>`;
  }

  private engagementSection(target: GhostView): string {
    const attackers = this.ctx.state.ghosts.filter((fleet) => fleet.own && !fleet.docked
      && (fleet.kind === "raider" || fleet.composition?.some((stack) => stack.kind === "raider" && stack.count > 0)));
    if (!attackers.length) return "";
    const selected = this.estimateAttackerByTarget.get(target.id) ?? attackers[0]!.id;
    const options = attackers.map((fleet) => {
      const count = fleetExactCount(fleet);
      return `<option value="${esc(fleet.id)}" ${fleet.id === selected ? "selected" : ""}>${esc(shipKindLabel(fleet.kind))}${count && count > 1 ? ` fleet · ${count} hulls` : ""}</option>`;
    }).join("");
    const estimate = this.engagementEstimates.get(this.estimateKey(selected, target.id))
      ?? [...this.engagementEstimates.values()].find((candidate) => candidate.target === target.id);
    return `<section class="m-section"><h3>Engagement projection</h3>` +
      `<div class="m-inline-form"><select id="m-estimate-attacker-${safeId(target.id)}" aria-label="Attacking fleet">${options}</select>` +
      `<button type="button" data-mobile-act="fleet-estimate" data-target="${esc(target.id)}">Estimate</button></div>` +
      (estimate ? this.renderEngagementEstimate(estimate) : `<small class="m-hint">Uses the served composition and defense picture; results are estimates, not combat truth.</small>`) +
      `</section>`;
  }

  private renderEngagementEstimate(estimate: EngagementEstimate): string {
    const losses = estimate.own_loss_bands?.filter((row) => row.hi > 0).map((row) =>
      `${row.lo === row.hi ? row.lo : `${row.lo}–${row.hi}`} ${shipKindLabel(row.kind)}`,
    ).join(", ") || estimate.own_losses.filter((row) => row.count > 0).map((row) => `${row.count} ${shipKindLabel(row.kind)}`).join(", ") || "none";
    const verdict = estimate.win_pct == null
      ? "Projected engagement"
      : `${Math.round(estimate.win_pct)}% ${estimate.win_pct >= 55 ? "favorable" : estimate.win_pct >= 45 ? "even" : "unfavorable"}`;
    const picture = estimate.target_known
      ? "exact reported composition"
      : `typical hulls for an estimated ${countClassLabel(estimate.target_count_class)}-ship contact`;
    return `<article class="m-feature-card"><small>Arrived projection</small><b>${esc(verdict)}</b>` +
      `<span>Expected losses ${esc(losses)} · ${esc(picture)} · composition ${fmt(estimate.composition_age)}s old</span></article>`;
  }

  private estimateKey(attacker: string, target: string): string {
    return `${attacker}:${target}`;
  }

  private rememberSignature(entry: SheetEntry): void {
    const signature = this.signature(entry);
    if (signature !== null) this.renderSignatures.set(entry.id, signature);
  }

  /** Cheap served-slice fingerprints replace 5 Hz HTML reconstruction. The
   * one-second render clock keeps human-facing ages/countdowns alive. */
  private signature(entry: SheetEntry): string | null {
    const state = this.ctx.state;
    const clock = Math.floor(liveSimTime());
    const props = entry.props ?? null;
    switch (entry.id) {
      case "fleets":
        return sheetFingerprint([clock, state.ghosts.filter((fleet) => fleet.own)]);
      case "ship": {
        const id = propsOf<{ id: string }>(entry).id ?? state.selectedShipId ?? "";
        return sheetFingerprint([
          clock, props, state.ghosts.find((fleet) => fleet.id === id), state.pendingOrders.get(id) ?? [], state.raids[id],
          state.galaxy?.jump_range, [...this.engagementEstimates.values()], this.estimateAttackerByTarget.get(id),
        ]);
      }
      case "system": {
        const id = propsOf<{ id: string }>(entry).id ?? state.selectedSystemId ?? "";
        return sheetFingerprint([clock, props, state.systems.find((system) => system.id === id), state.ghosts, state.pendingOrders, state.battles, this.defenseRadius.get(id)]);
      }
      case "market":
        return sheetFingerprint([
          clock, this.marketTab, this.marketCommodity, this.marketSide,
          state.market, state.wallet, state.freight, state.systems, state.ghosts.filter((fleet) => fleet.own && fleet.docked === "hub"),
          marketReservations, recentMarketOrders, state.transactions,
        ]);
      case "log":
        return sheetFingerprint([
          clock, state.timeline, state.founding, state.diplomacy, state.syndicateInvites, state.operations, state.battles,
          [...this.dismissedDecisions], this.arrivedReports,
        ]);
      case "battle": {
        const id = propsOf<{ id: string }>(entry).id ?? "";
        return sheetFingerprint([clock, props, state.battles.find((battle) => battle.id === id), state.ghosts]);
      }
      default:
        return null;
    }
  }

  private shipControls(fleet: GhostView): string {
    const unloadQueued = (this.ctx.state.pendingOrders.get(fleet.id) ?? [])
      .some((order) => !order.lost && order.kind === "unload");
    const jump = jumpCapable(fleet)
      ? `<button type="button" data-mobile-act="fleet-jump" data-id="${esc(fleet.id)}">Jump</button>` : "";
    const guard = guardCapable(fleet)
      ? `<button type="button" data-mobile-act="fleet-guard" data-id="${esc(fleet.id)}">Guard</button>` : "";
    const recall = this.ctx.state.raids[fleet.id]
      ? `<button type="button" data-mobile-act="fleet-recall" data-id="${esc(fleet.id)}">Recall</button>` : "";
    const unload = fleet.docked && fleetCargoUnits(fleet) > 0
      ? `<button type="button" data-mobile-act="fleet-unload" data-id="${esc(fleet.id)}"${unloadQueued ? " disabled" : ""}>${unloadQueued ? "Unload queued" : "Unload"}</button>` : "";
    const rescue = fleet.stalled && !fleet.rescue_inbound
      ? `<button type="button" data-mobile-act="fleet-rescue" data-id="${esc(fleet.id)}">Call AAA Rescue</button>` : "";
    const range = jumpRangeAt(this.ctx.state.galaxy, fleet.pos);
    const commandHelp = `<details class="m-details m-help"><summary>Command rules</summary><div>` +
      `<p>A movement order automatically undocks a berthed fleet. Full speed is fastest but makes dark hulls easier to detect; Stealth is quieter and takes about twice as long.</p>` +
      (jump ? `<p>Jump destinations must be within ${fmt(range)} su of the served sighting, with both ends clear of gravity wells. The true fleet is checked when the delayed order arrives.</p>` : "") +
      (guard ? `<p>A Guard assignment is light-delayed. Once it arrives, defensive reactions happen locally without another command-center round trip.</p>` : "") +
      `</div></details>`;
    return `<section class="m-section"><h3>Command</h3><div class="m-action-grid">` +
      `<button type="button" class="m-primary" data-mobile-act="fleet-move" data-id="${esc(fleet.id)}">Move</button>` +
      jump + guard + recall + unload + rescue +
      `<button type="button" data-mobile-act="fleet-transit" data-id="${esc(fleet.id)}" data-mode="full">Full speed</button>` +
      `<button type="button" data-mobile-act="fleet-transit" data-id="${esc(fleet.id)}" data-mode="stealth">Stealth</button>` +
      `</div><small class="m-hint">Select an Interceptor, then tap a rival to raid. Long-press the rival to destroy.</small>${commandHelp}</section>` +
      this.emplacementControls(fleet);
  }

  private emplacementControls(fleet: GhostView): string {
    if (fleet.kind !== "builder") return "";
    const cost = kitCostLabel("deep_space_sensor");
    const busy = !!fleet.job || this.ctx.state.pendingOrders.has(fleet.id) || Math.hypot(fleet.vel.x, fleet.vel.y) >= 0.5;
    const siteError = this.ctx.renderer.siteError("deep_space_sensor", fleet.pos, this.ctx.state);
    const affordable = kitAffordable("deep_space_sensor");
    const reason = busy ? "Stop here and finish any current job first."
      : siteError ? siteError
        : !affordable ? "No single owned system can supply the full kit."
          : "Open space is legal at this served position.";
    return `<section class="m-section"><h3>Construction</h3><p>A Deep-Space Sensor is a stationary picket raised where this ship is parked.</p>` +
      `<div class="m-order"><b>Kit</b><span>${esc(cost)}</span></div><small class="m-hint">One owned system pays the entire kit; stockpiles are not pooled.</small>` +
      `<div class="${busy || siteError || !affordable ? "m-warning" : "m-muted"}">${esc(reason)}</div>` +
      `<button type="button" class="m-wide-button m-primary" data-mobile-act="fleet-emplace" data-id="${esc(fleet.id)}" ${busy || siteError || !affordable ? "disabled" : ""}>Build Deep-Space Sensor here</button></section>`;
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
    const pirate = pirateSite(dynamic?.intel?.enclave_tier ?? 0);
    return {
      title: fixed.name,
      eyebrow: `${fixed.band.toUpperCase()} band · ${mine ? "your holding" : dynamic?.owner ? "rival holding" : "unclaimed"}`,
      html: (pirate ? `<article class="m-trade-card">${pirate.art ? `<img src="${pirate.art}" alt="" width="64" height="64">` : ""}<b>${pirate.title}</b><p>${pirate.goal}</p></article>` : "") + `<div class="m-action-grid m-action-grid--top">` +
        (semantic
          ? `<button type="button" class="m-primary" data-mobile-act="semantic-exit">Back to galaxy</button>`
          : `<button type="button" class="m-primary" data-mobile-act="system-enter" data-id="${esc(fixed.id)}">Enter system</button>`) +
        `</div>` +
        (mine && dynamic ? `<div class="m-stat-grid"><span><small>Population</small><b>${fmt(dynamic.population, 2)}M</b></span>` +
          `<span><small>Workforce</small><b>${dynamic.workforce ? `${dynamic.workforce.posted}/${dynamic.workforce.units}` : "—"}</b></span>` +
          `<span><small>Storage</small><b>${dynamic.storage_used}/${dynamic.storage_cap}</b></span>` +
          `<span><small>Slots</small><b>${dynamic.slots_used}/${dynamic.slots_total}</b></span></div>` : "") +
        (mine && dynamic ? systemDefenseHtml(this.ctx.state, fixed, dynamic, this.defenseRadius.get(fixed.id) ?? DEFAULT_DEFENSE_RADIUS, "mobile", this.defenseAssigning.has(fixed.id)) : "") +
        `<section class="m-section"><h3>Stockpile</h3><div class="m-ledger">${stock}</div></section>` +
        `<section class="m-section"><h3>Worlds</h3>${bodies || `<div class="m-muted">No body report.</div>`}</section>` +
        (fleetRows ? `<section class="m-section"><h3>Fleets here</h3><div class="m-list">${fleetRows}</div></section>` : ""),
    };
  }

  private renderMarket(): SheetView {
    if (this.marketTab === "transactions") requestTransactions(this.ctx);
    const tabs = (["exchange", "warehouse", "specialists", "modules", "transactions"] as MarketTab[]).map((tab) =>
      `<button type="button" data-act="mtab:${tab}" data-mobile-act="market-tab" data-tab="${tab}" aria-selected="${this.marketTab === tab}">${human(tab)}</button>`,
    ).join("");
    const body = this.marketTab === "transactions" ? transactionsHtml(this.ctx.state)
      : this.marketTab === "exchange" ? this.renderExchange()
      : this.marketTab === "warehouse" ? this.renderWarehouse()
        : this.marketTab === "specialists" ? this.renderSpecialists()
          : this.renderModules();
    return {
      title: "Market Hub",
      eyebrow: `Observed ${fmt(this.ctx.state.market?.staleness ?? 0, 1)}s delayed`,
      html: `<div class="m-subtabs m-market-tabs" role="tablist">${tabs}</div>${body}`,
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
      `<button type="button" class="m-primary" data-mobile-act="market-submit">Send ${human(this.marketSide)} order</button>` +
      `<details class="m-details m-help"><summary>How Exchange orders clear</summary><div><p>Orders rest for the next uniform-price batch, so reacting fastest gives no advantage. Buys cancel above 10% over their estimated average; sells cancel below 10% under it. Quantity impact and the light-delayed liquidity picture are included.</p></div></details></section></div>` +
      `<section class="m-section"><h3>Open orders</h3>${openRows || `<div class="m-muted">None.</div>`}</section>` +
      `<section class="m-section"><h3>Incoming orders</h3>${incoming || `<div class="m-muted">None in flight.</div>`}</section>` +
      `<section class="m-section"><h3>Recent orders</h3>${recent || `<div class="m-muted">No execution receipts yet.</div>`}</section>`;
  }

  private renderWarehouse(): string {
    const holdings = COMMODITIES.map((commodity) => ({ commodity, units: warehouseUnits(commodity) })).filter((row) => row.units > 0);
    const ledger = holdings.length
      ? holdings.map((row) => `<span>${human(row.commodity)}<b>${row.units}</b></span>`).join("")
      : `<span class="m-muted">Warehouse empty</span>`;
    const shipments = (this.ctx.state.freight?.shipments ?? []).map((shipment) =>
      `<div class="m-order"><b>${shipment.units} ${human(shipment.commodity)}</b><span>${shipment.direction} · ${shipment.aboard ? "aboard" : "queued"}</span></div>`,
    ).join("");
    const docked = this.ctx.state.ghosts.filter((fleet) => fleet.own && fleet.docked === "hub").map((fleet) => this.fleetRow(fleet)).join("");
    return `<section class="m-section"><h3>Market Warehouse</h3><div class="m-ledger">${ledger}</div></section>` +
      `<section class="m-section"><h3>Shipments</h3>${shipments || `<div class="m-muted">No shipments in hand.</div>`}</section>` +
      (docked ? `<section class="m-section"><h3>Hub berths</h3><div class="m-list">${docked}</div></section>` : "");
  }

  private renderSpecialists(): string {
    const cost = this.ctx.state.galaxy?.specialist_hire_cost ?? 800;
    const canHire = !!this.homeSystemId() && spendableMarketCredits() >= cost;
    return `<div class="m-list">${SPECIALISTS.map(([slug, name, role]) =>
      `<div class="m-service-row"><span><b>${name}</b><small>${role}</small></span><button type="button" data-mobile-act="hire-specialist" data-specialist="${slug}" ${canHire ? "" : "disabled"}>Hire · ${fmt(cost)} cr</button></div>`,
    ).join("")}</div><p class="m-hint">A posted specialist multiplies matching production lines ×1.75. Contracts ship to your home system on a sub-light, raidable personnel liner.</p>`;
  }

  private renderModules(): string {
    const home = this.homeSystem();
    const held = home?.modules ?? {};
    return `<div class="m-list">${MODULES.map((module) => {
      const value = moduleRecipeValue(module.kind);
      const buy = value === null || isBlueprintOnly(module.kind) ? null : value * MODULE_BUY_MULT;
      const sell = value === null ? null : value * MODULE_SELL_MULT;
      return `<div class="m-service-row"><span><b>${module.name}</b><small>${module.role} · held ${held[module.kind] ?? 0}</small></span>` +
        `<div><button type="button" data-mobile-act="module-buy" data-module="${module.kind}" ${buy !== null && spendableMarketCredits() >= buy && home ? "" : "disabled"}>${isBlueprintOnly(module.kind) ? "Blueprint only" : `Buy ${buy === null ? "—" : `~${fmt(buy)}`}`}</button>` +
        `<button type="button" data-mobile-act="module-sell" data-module="${module.kind}" ${(held[module.kind] ?? 0) > 0 ? "" : "disabled"}>Sell ${sell === null ? "—" : `~${fmt(sell)}`}</button></div></div>`;
    }).join("")}</div><p class="m-hint">Sol modules ship as physical crates; local manufacture remains cheaper.</p>`;
  }

  private renderCheckin(): SheetView {
    const decisions = this.renderDecisions();
    const reports = this.arrivedReports.slice().reverse().map((entry) =>
      `<div class="m-log-row is-${entry.tone}"><span>${esc(entry.text)}</span><time>arrived ${fmt(Math.max(0, liveSimTime() - entry.at))}s ago</time></div>`,
    ).join("");
    const timeline = this.ctx.state.timeline.slice().reverse().map((entry) =>
      `<div class="m-log-row is-${entry.severity}"><span>${esc(entry.text)}</span><time>${fmt(Math.max(0, liveSimTime() - entry.at_time))}s ago</time></div>`,
    ).join("");
    return {
      title: "Check-in",
      eyebrow: "Decision inbox · arrived reports",
      html: `<section class="m-section m-section--first"><h3>Decision inbox</h3>${decisions || `<div class="m-empty m-empty--good">No urgent decisions.</div>`}</section>` +
        (reports ? `<section class="m-section"><h3>Arrived reports</h3>${reports}</section>` : "") +
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
    const average = marketAverageQuote(quote.price, qty, this.marketSide, quote.depth);
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

  private hireSpecialist(specialist?: string): void {
    const home = this.homeSystemId();
    if (!specialist || !home) return;
    this.ctx.send({ type: "HireSpecialist", specialist, dest_system: home });
    this.hooks.notice(`${human(specialist)} contract dispatched.`);
  }

  private tradeModule(module: string | undefined, buy: boolean): void {
    if (buy && module && isBlueprintOnly(module as ModuleKind)) return;
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
    if (fleet.docked === "hub") this.ctx.intent.beginFleetCommand({ type: "HubUnload", fleet_id: fleet.id });
    else this.ctx.intent.beginFleetCommand({ type: "SystemUnload", fleet_id: fleet.id, system: fleet.docked });
  }

  private runFoundingAction(action: string): void {
    const founding = this.ctx.state.founding;
    if (!founding) return;
    if (action === "market") {
      this.marketTab = "warehouse";
      this.hooks.openSheet({ id: "market" });
      return;
    }
    if (action === "research" || action === "research-enrichment") {
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
      const fleet = this.ctx.state.ghosts.find((ghost) => ghost.own && ghost.composition?.some((stack) => (kind === "convoy" ? isPlayerFreighter(stack.kind) : stack.kind === kind)));
      if (fleet) this.hooks.focusFleet(fleet.id);
      return;
    }
    const home = this.homeSystem();
    if (!home) return;
    if (action.startsWith("build-")) {
      const structure = action.slice(6);
      const ships = ["convoy", "scout"].includes(structure);
      const body = ships ? home.bodies.find(b => (b.structures.shipyard ?? 0) > 0)
        : [...home.bodies].sort((a,b) => (structure === "academy" ? (b.infrastructure_slots ?? 0) - (a.infrastructure_slots ?? 0) : (b.industrial_slots ?? 0) - (a.industrial_slots ?? 0)))[0];
      if (body) this.hooks.openSheet({ id: ships ? "shipyard" : "build", props: { systemId: home.id, bodyId: body.id } });
    } else if (["academy", "shipyard", "smelter"].includes(action)) {
      const body = home.bodies.find(b => (b.structures[action] ?? 0) > 0);
      if (body) this.hooks.openSheet({ id: "planet", props: { systemId: home.id, bodyId: body.id } });
    } else this.hooks.focusSystem(home.id);
  }

  private fleetStatus(fleet: GhostView): string {
    if (fleet.docked === "hub") return "docked · Market Hub";
    if (fleet.docked) {
      const name = this.ctx.state.galaxy?.systems.find((system) => system.id === fleet.docked)?.name ?? fleet.docked;
      return `${fleet.defend_system ? "defending · " : ""}docked · ${name}`;
    }
    if (fleet.rescue_inbound) return "AAA rescue inbound";
    if (fleet.stalled) return "out of fuel";
    if (fleet.fuel_transfer) return tenderStatus(fleet);
    if (fleet.jump_spool) return fleet.jump_spool.waiting_for_fuel ? "jump waiting for fuel" : `jump spooling ${fmt(fleet.jump_spool.remaining)}s`;
    if (fleet.guard_target) return "guarding";
    if (fleet.defend_system) return "defending system";
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

function foundingCopy(stage: keyof typeof FOUNDING_STEP): { title: string; copy: string; action: string; label: string } {
  switch (stage) {
    case "grow_business": return { title: "Grow your business", copy: "Expand exports or start refining. Either advances the tutorial; both remain open.", action: "home", label: "Open home" };
    case "build_shipyard": return { title: "Build Shipyard I", copy: "Import Alloys, Machinery and Electronics with your export earnings.", action: "home", label: "Open home" };
    case "build_mine": return { title: "Build and staff a Mining Complex", copy: "Mine Ferrite Ore on the designated world, then assign workforce.", action: "home", label: "Open home" };
    case "build_convoy": return { title: "Prepare a Freighter", copy: "Assign workforce to the Shipyard, then build a Tiny Freighter.", action: "home", label: "Open home" };
    case "build_second_freighter": return { title: "Build a second Tiny Freighter", copy: "Import its materials and staff the Shipyard to expand your trade capacity.", action: "home", label: "Open home" };
    case "export_production": return { title: "Dispatch the opening export", copy: "Load Ferrite Ore, then send the Freighter.", action: "freighter", label: "Select Freighter" };
    case "defeat_privateer": return { title: "Guard the Freighter", copy: "Intercept the Rogue Privateer before it reaches the civilian hull.", action: "privateer", label: "Select Privateer" };
    case "complete_export": return { title: "Complete the export", copy: "Sell Ferrite Ore at the Market Hub.", action: "freighter", label: "Select Freighter" };
    case "build_academy": return { title: "Establish and staff Academy I", copy: "Import its kit, build it at home, and assign workforce.", action: "market", label: "Open Warehouse" };
    case "first_research": return { title: "Choose a first programme", copy: "Complete any Tier I corporate research programme.", action: "research", label: "Open Research" };
    case "build_scout": return { title: "Build a Scout", copy: "Import its kit and construct the exploration hull.", action: "market", label: "Open Warehouse" };
    case "survey_candidates": return { title: "Survey two nearby systems", copy: "Receive both reports to complete the tutorial.", action: "scout", label: "Select Scout" };
    case "build_colony":
    case "establish_colony":
    case "complete": return { title: "Founding complete", copy: "Develop home and grow your trade routes.", action: "home", label: "Open home" };
  }
}

function isCommodity(value: string | undefined): value is Commodity {
  return !!value && COMMODITIES.includes(value as Commodity);
}

function isMarketTab(value: string | undefined): value is MarketTab {
  return value === "exchange" || value === "warehouse" || value === "specialists" || value === "modules" || value === "transactions";
}

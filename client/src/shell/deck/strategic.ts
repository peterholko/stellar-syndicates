import { isPlayerFreighter } from "../../protocol";
import {
  commodityRoom,
  recordForReport,
  guardCapable,
  shipKindLabel,
  sumOwnComposition,
  surveyCapable,
} from "../../core/derive/fleet";
import {
  arrivalLocal,
  fmt,
  fmtBuildDur,
  fmtDur,
  informationDelay,
  operationAbandonable,
  isFounderContract,
  operationCopy,
  operationHullArt,
  operationIcon,
  operationLeft,
  operationReward,
  operationTitle,
} from "../../core/derive/format";
import { nearestSystemName, systemName } from "../../core/derive/geo";
import { licenceForSale, licenceNote } from "../../core/derive/market";
import { CLAIM_TIP, claimsHeld } from "../../core/derive/claims";
import {
  battleCommandDelay,
} from "../../core/derive/orders";
import { BattleWithdrawPrompt } from "../battlewithdraw";
import { battleRecordMarkKey, reportForBattleRecord, reportMarkKey, saveBattleMarks } from "../../battlehistory";
import { currentSurvivor, nearestRepairYard, survivingGuardTarget, survivorOrderBlock } from "../../battleaftermath";
import { captainTitle } from "../../core/derive/captains";
import { dispatchWarnings, fleetReadiness } from "../../core/derive/readiness";
import { captainPortrait } from "../art";
import { corpCard, fmtDays } from "../../core/derive/medals";
import { boardLine, boardSummary } from "../../core/derive/board";
import { signalsSummary } from "../../core/derive/signals";
import { corpCardHtml, medalChipsHtml, medalCountsHtml } from "../corpcard";
import { bonusLine, currentRung, givingWorlds, roman } from "../../core/derive/projects";
import { icon, label, type IconKey } from "../../icons";
import type {
  BattleReportView,
  BattleSurvivor,
  CaptureReportView,
  CharterView,
  Commodity,
  CompCount,
  GhostView,
  OperationView,
  RankingRow,
  ShipKind,
  SyndicateBonus,
  SyndicateProjectsView,
  SyndicateRole,
} from "../../protocol";
import { liveSimTime } from "../../state";
import { crackdownTitle, DECREE_TEXT, demandText, gripNumeral, gripTitle, keystoneFeed, RESOLVE_BAND, RESOLVE_TIP, resolveBand } from "../../core/derive/season";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";
import { handoffHtml, handleHandoffAction } from "./handoff";
import { charterLedgerHtml } from "./ledger";
import { commsAction, commsHtml } from "./comms";
import { INVITES_MAX, inviteLink, presenceText } from "../../core/derive/social";
import { explorationHtml, explorationLinkHtml, handleExplorationAction } from "../exploration";

interface StrategicHooks {
  go(route: DeckRoute): void;
  replace?(route: DeckRoute): void;
  back(): void;
  notice(html: string): void;
  openBattleViewer(id: string): void;
  selectFleets(ids: string[]): void;
  openWorld(system: string, body: number): void;
}

const MIDGAME_COPY: Record<import("../../protocol").MidgameStage, [string, string]> = {
  home_development: ["Home development", "Build a reliable industrial base and finish the founding programme."],
  exploration: ["Exploration", "Use scouts and expedition offers to turn nearby darkness into choices."],
  specialization: ["Specialization", "Compare surveyed strengths and choose what this corporation will do unusually well."],
  first_colony: ["First colony", "Commit the colony ship and establish a second physical holding."],
  trade_network: ["Trade network", "Connect specialized holdings through contracts, freight, and escorted freighters."],
  contested_expansion: ["Contested expansion", "Public objectives and scarce sites now put your plans against rival corporations."],
  regional_power: ["Regional power", "Hold strategic nodes and organize multi-stage syndicate operations."],
};

type OperationTab = "active" | "available" | "history";
const OPERATION_TABS: Record<OperationTab, [string, string]> = {
  active: ["Active", "No active contracts."],
  available: ["Available", "No offers yet."],
  history: ["History", "No finished contracts."],
};

type RankCat = {
  slug: string;
  label: string;
  short: string;
  copy: string;
  format(row: RankingRow): string;
  value(row: RankingRow): number;
};

const RANK_CATS: RankCat[] = [
  { slug: "valuation", label: "Valuation", short: "Val", copy: "Credits plus holdings at the published market close.", format: (r) => `${fmt(r.valuation)} Cr`, value: (r) => r.valuation },
  { slug: "trade_throughput", label: "Trade throughput", short: "Trade", copy: "Cargo delivered by your freighters to any valid destination.", format: (r) => fmt(r.trade_throughput), value: (r) => r.trade_throughput },
  { slug: "market_profit", label: "Net market profit", short: "Profit", copy: "Lifetime Exchange proceeds minus purchase spend.", format: (r) => `${fmt(r.market_profit)} Cr`, value: (r) => r.market_profit },
  { slug: "cargo_captured", label: "Cargo captured", short: "Seized", copy: "Freighter cargo and system plunder seized by force.", format: (r) => fmt(r.cargo_captured), value: (r) => r.cargo_captured },
  { slug: "cargo_protected", label: "Cargo protected", short: "Guard", copy: "Cargo delivered after its freighter survived battle en route.", format: (r) => fmt(r.cargo_protected), value: (r) => r.cargo_protected },
  { slug: "battle_efficiency", label: "Battle efficiency", short: "Kill/Loss", copy: "Enemy hull destroyed divided by own hull lost; provisional rows have too few engagements.", format: (r) => r.battle_ranked ? `×${r.battle_efficiency.toFixed(2)}` : `provisional · ${r.battle_engagements} engagements`, value: (r) => r.battle_ranked ? r.battle_efficiency : -Infinity },
  { slug: "systems_developed", label: "Systems developed", short: "Built", copy: "Total development tiers constructed across your holdings.", format: (r) => fmt(r.systems_developed), value: (r) => r.systems_developed },
  { slug: "intel_gathered", label: "Intel gathered", short: "Intel", copy: "Scout snapshots captured by the corporation.", format: (r) => fmt(r.intel_gathered), value: (r) => r.intel_gathered },
  { slug: "recovery", label: "Recovery", short: "Comeback", copy: "Valuation regained since the last captured-system loss.", format: (r) => `${fmt(r.recovery)} Cr`, value: (r) => r.recovery },
];

/** The strategic controller owns every late-game workspace route. All values
 * come from the served View; commands leave through CoreContext and therefore
 * retain the same delay, fog and authority rules as the simulation. */
export class DeckStrategicRoutes {
  private routeSignature = "";
  private rankCategory = "valuation";
  private operationTab: OperationTab | null = null;
  private operationFleets = new Map<string, string>();
  private operationCharges = new Map<string, string>();
  private readonly withdrawal = new BattleWithdrawPrompt();

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: StrategicHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "battle" || route.params?.report) this.withdrawal.clear();
    if (!route || !["operations", "syndicate", "faction", "rankings", "battle", "comms"].includes(route.name)) return false;
    const active = document.activeElement;
    if (!force && (active instanceof HTMLInputElement || active instanceof HTMLSelectElement || active instanceof HTMLTextAreaElement)
      && this.root.contains(active)) return true;
    const signature = sheetFingerprint([
      route,
      this.rankCategory,
      this.operationTab,
      this.ctx.state.simTime,
      this.ctx.state.operations, this.ctx.state.board,
      this.ctx.state.explorationSites, this.ctx.state.selectedExplorationSiteId,
      this.ctx.state.explorationJournal,
      this.ctx.state.midgameStage,
      this.ctx.state.founding, this.ctx.state.systems, this.ctx.state.research, this.ctx.state.charterLedger,
      this.ctx.state.selectedShipId,
      this.ctx.state.syndicate,
      this.ctx.state.syndicateInvites,
      this.ctx.state.diplomacy,
      this.ctx.state.selectedSystemId,
      this.ctx.state.charter,
      this.ctx.state.charterLadder,
      this.ctx.state.rankings,
      this.ctx.state.battles,
      this.ctx.state.battleReports,
      this.ctx.state.captureReports,
      this.ctx.state.battleRecords,
      this.ctx.state.ghosts,
      this.ctx.state.orders,
      [...this.ctx.state.battleViewed],
      [...this.ctx.state.battleDismissed],
      this.ctx.state.social,
    ]);
    if (!force && signature === this.routeSignature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.routeSignature = signature;
    if (route.name === "operations") setHtml(this.root, this.exploring(route)
      ? explorationHtml(this.ctx.state) : this.operationsHtml(route.query?.contract));
    else if (route.name === "syndicate") setHtml(this.root, this.syndicateHtml());
    else if (route.name === "comms") setHtml(this.root, commsHtml(this.ctx.state, route.query));
    else if (route.name === "faction") setHtml(this.root, this.factionHtml());
    else if (route.name === "rankings") setHtml(this.root, route.query?.corp ? this.corpCardHtml(route.query.corp) : this.rankingsHtml());
    else setHtml(this.root, this.battleHtml(route));
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (!route || !["operations", "syndicate", "faction", "rankings", "battle", "comms"].includes(route.name)) return false;
    if (route.name === "comms" && button.dataset.deckAct?.startsWith("strategic-comms-")) {
      if (commsAction(button, this.ctx, route, this.hooks.go)) { this.invalidate(); return true; }
    }
    if (route.name === "operations" && handleExplorationAction(button, this.ctx)) {
      // §ops-compact: a tab, or ← Contacts, opens the exploration view in place of the page.
      const act = button.dataset.exploreAct ?? "";
      if ((act.startsWith("tab-") || act === "back") && route.query?.view !== "exploration") {
        const to: DeckRoute = { name: "operations", query: { view: "exploration" } };
        if (route.query?.site) (this.hooks.replace ?? this.hooks.go)(to);
        else this.hooks.go(to);
        return true;
      }
      this.invalidate(); this.render(route, true); return true;
    }
    if (route.name === "operations" && handleHandoffAction(button, this.ctx, {
      go: this.hooks.go, openWorld: this.hooks.openWorld, selectFleet: id => this.hooks.selectFleets([id]),
    })) return true;
    const action = button.dataset.deckAct;
    if (action === "open-signals") { this.hooks.go({ name: "signals" }); return true; }
    if (action === "operations-tab") {
      const tab = button.dataset.tab;
      if (tab === "active" || tab === "available" || tab === "history") this.operationTab = tab;
      this.invalidate(); this.render(route, true); return true;
    }
    // §charter-ledger: a goal's button opens the screen that advances it.
    if (action === "ledger-open") {
      const to = button.dataset.route;
      if (to === "syndicate" || to === "market" || to === "research") this.hooks.go({ name: to });
      return true;
    }
    if (!action?.startsWith("strategic-")) return false;
    if (action === "strategic-rank") {
      if (button.dataset.category) this.rankCategory = button.dataset.category;
      this.invalidate();
      this.render(route, true);
      return true;
    }
    // §medals: a corporation's card, from the published table.
    if (action === "strategic-corp" || action === "strategic-corp-back") {
      this.hooks.go(action === "strategic-corp" && button.dataset.corp
        ? { name: "rankings", query: { corp: button.dataset.corp } } : { name: "rankings" });
      return true;
    }
    if (action.startsWith("strategic-operation-")) this.operationAction(action, button);
    else if (action.startsWith("strategic-syndicate-")) this.syndicateAction(action, button);
    else if (action === "strategic-faction-pay") this.payReinstatement();
    // §licences: the order travels to the Hub with its payment.
    else if (action === "strategic-buy-licence" && button.dataset.hull) {
      this.ctx.send({ type: "BuyLicence", hull: button.dataset.hull as ShipKind });
      this.hooks.notice(`<b>Licence order sent</b> · ${esc(shipKindLabel(button.dataset.hull as ShipKind))} → the Authority at the Hub.`);
    }
    // §season: answer the Authority's demands from the Faction page.
    else if (action === "strategic-levy") {
      this.ctx.send({ type: "AnswerLevy", accept: button.dataset.accept === "1" });
      this.hooks.notice(`<b>Levy ${button.dataset.accept === "1" ? "accepted" : "refused"}</b> · answer on its way to the Hub.`);
    } else if (action === "strategic-feed" && button.dataset.system && button.dataset.commodity) {
      this.ctx.send({ type: "FeedKeystone", system_id: button.dataset.system, commodity: button.dataset.commodity as Commodity, units: Number(button.dataset.units ?? 0) });
      this.hooks.notice(`<b>Keystone fed</b> · ${esc(button.dataset.units ?? "")} ${esc(label(button.dataset.commodity as Commodity))} · on its way.`);
    } else if (action === "strategic-gate-step" && button.dataset.fleet && button.dataset.system) {
      this.ctx.send({ type: "GateStep", fleet_id: button.dataset.fleet, system_id: button.dataset.system });
      this.hooks.notice(`<b>Gate Step</b> · to ${esc(systemName(button.dataset.system))}.`);
    } else if (action === "strategic-arrears" && button.dataset.commodity) {
      this.ctx.send({ type: "PayArrears", commodity: button.dataset.commodity as Commodity });
      this.hooks.notice("<b>Arrears paid</b> · on their way to the Hub.");
    } else if (action === "strategic-requisition" && button.dataset.id) {
      const pay = button.dataset.pay === "1";
      this.ctx.send({ type: "AnswerRequisition", id: Number(button.dataset.id), pay });
      this.hooks.notice(`<b>Requisition ${pay ? "paid" : "refused"}</b> · answer on its way to the Hub.`);
    }
    else if (action.startsWith("strategic-battle-withdraw-") && button.dataset.battle && button.dataset.fleet && route.name === "battle" && route.params?.id === button.dataset.battle) {
      const notice = this.withdrawal.handle(action.slice("strategic-battle-withdraw-".length), button.dataset.battle, button.dataset.fleet, this.ctx);
      if (notice) this.hooks.notice(notice);
      this.invalidate();
      this.render(route, true);
    } else if (action === "strategic-battle-doctrine") {
      this.hooks.go({ name: "doctrine" });
    } else if (action === "strategic-battle-view" && button.dataset.record) {
      this.hooks.openBattleViewer(button.dataset.record);
    } else if (action.startsWith("strategic-survivor-")) {
      this.survivorAction(action, button);
    } else if (action === "strategic-battle-dismiss") {
      const id = Number(button.dataset.report);
      const report = button.dataset.reportKind === "capture"
        ? this.ctx.state.captureReports.find((r) => r.id === id)
        : this.ctx.state.battleReports.find((r) => r.id === id && r.battle_id);
      if (report) {
        this.ctx.state.battleDismissed.add(reportMarkKey(report));
        if (this.ctx.renderer.selectedBattleMarkerId === id) this.ctx.renderer.selectedBattleMarkerId = null;
        saveBattleMarks(this.ctx.state);
        this.hooks.back();
      }
    } else if (action === "strategic-battle-dismiss-record") {
      const record = this.ctx.state.battleRecords.find((rec) => rec.id === button.dataset.record && rec.outcome !== null);
      if (record) {
        this.ctx.state.battleDismissed.add(battleRecordMarkKey(record.id));
        saveBattleMarks(this.ctx.state);
        this.hooks.back();
      }
    }
    this.invalidate();
    return true;
  }

  handleInput(input: HTMLInputElement | HTMLSelectElement, route: DeckRoute | null): boolean {
    if (route?.name === "operations" && input instanceof HTMLSelectElement && input.dataset.operation) {
      const choices = input.dataset.deckInput === "operation-charge" ? this.operationCharges : this.operationFleets;
      choices.set(input.dataset.operation, input.value);
      this.invalidate();
      this.render(route, true);
      return true;
    }
    if (!(input instanceof HTMLInputElement)) return false;
    if (route?.name !== "faction" || input.dataset.deckInput !== "reinstatement") return false;
    this.syncReinstatement(input);
    return true;
  }

  invalidate(): void { this.routeSignature = ""; }

  /** Exploration opens from its row on the page, or from a contact. */
  private exploring(route: DeckRoute): boolean {
    return route.query?.view === "exploration" || !!this.ctx.state.selectedExplorationSiteId;
  }

  private operationsHtml(contract?: string): string {
    const [stage, stageCopy] = MIDGAME_COPY[this.ctx.state.midgameStage];
    const selected = this.ctx.state.operations.find(o => o.id === contract);
    const counterRaids = this.ctx.state.operations.filter(o => o !== selected
      && o.briefing?.follow_up.startsWith("counter_raid_") && ["offered", "active"].includes(o.state));
    // §privateer-contract: the charter's own contracts, first while open.
    const charter = this.ctx.state.operations.filter(o => o !== selected && isFounderContract(o)
      && ["offered", "active"].includes(o.state));
    // §contract-board: today's contracts have their own section.
    const board = boardSummary(this.ctx.state, liveSimTime());
    // §signals: their own pop-out; one line here leads to it.
    const signals = this.ctx.state.signals
      ? `<button type="button" class="deck-signals-link" data-deck-act="open-signals"><b>Signals</b><span>${esc(signalsSummary(this.ctx.state.signals, liveSimTime()))}</span><span aria-hidden="true">›</span></button>` : "";
    const others = this.ctx.state.operations.filter(o => o !== selected && !counterRaids.includes(o)
      && !charter.includes(o) && !board?.contracts.includes(o));
    const available = others.filter((o) => o.state === "offered" || (o.state === "active" && !o.joined));
    const active = others.filter((o) => o.state === "active" && o.joined);
    const history = others.filter((o) => !["offered", "active"].includes(o.state)).sort((a, b) => b.reported_at - a.reported_at).slice(0, 12);
    const group = (title: string, rows: OperationView[]) => `<section class="deck-section"><header><div><h3>${esc(title)}</h3>${rows.length ? `<p>${rows.length} arrived record${rows.length === 1 ? "" : "s"}.</p>` : ""}</div><b>${rows.length}</b></header>${rows.map((o) => this.operationCard(o)).join("")}</section>`;
    // §ops-compact (Peter, Oct 31 2026): one contract list at a time, as on
    // mobile; offers show first when nothing is active.
    const lists: Record<OperationTab, OperationView[]> = { active, available, history };
    const tab = this.operationTab ?? (active.length || !available.length ? "active" : "available");
    const tabs = `<nav class="deck-tabs deck-operations__tabs" aria-label="Contracts">${(Object.keys(OPERATION_TABS) as OperationTab[]).map((t) =>
      `<button type="button" data-deck-act="operations-tab" data-tab="${t}" aria-selected="${t === tab}">${OPERATION_TABS[t][0]}<em>${lists[t].length || ""}</em></button>`).join("")}</nav>`;
    const contracts = `<section class="deck-section deck-contracts">${tabs}${lists[tab].map((o) => this.operationCard(o)).join("") || `<div class="deck-empty-inline">${OPERATION_TABS[tab][1]}</div>`}</section>`;
    return `<section class="deck-page deck-operations"><header class="deck-page__lead"><h2 title="${escAttr(stageCopy)}">${esc(stage)}</h2></header>${selected ? group(selected.briefing?.follow_up.startsWith("counter_raid_") ? "Counterattack" : "Funding contract", [selected]) : ""}${charter.length ? group("Authority contract", charter) : ""}${counterRaids.length ? group("Defend home · strike back", counterRaids) : ""}${handoffHtml()}${charterLedgerHtml(this.ctx.state)}${signals}${explorationLinkHtml(this.ctx.state)}${board ? `<section class="deck-section deck-board"><header><div><h3>Contract board</h3><p>${esc(boardLine(board))}</p></div><b>${board.contracts.length}</b></header>${board.contracts.map((o) => this.operationCard(o)).join("") || `<div class="deck-empty-inline">Today's contracts are on their way from the Market Hub.</div>`}</section>` : ""}${contracts}</section>`;
  }

  private operationCard(operation: OperationView): string {
    const pct = Math.max(0, Math.min(100, operation.goal > 0 ? operation.progress / operation.goal * 100 : 0));
    const selected = this.selectedOwnFleet();
    const actions: string[] = [];
    if ((operation.state === "offered" || (operation.state === "active" && !operation.joined)) && !operation.joined) {
      const accept = actionButton("strategic-operation-accept", "Accept", operation.id, "is-primary");
      // §privateer-contract: the guide rings the charter's contract's Accept.
      actions.push(isFounderContract(operation) ? accept.replace("<button ", `<button data-coach="contract-accept" `) : accept);
    }
    if (operation.state === "active" && operation.joined) {
      if (operation.briefing) actions.push(this.followUpControls(operation));
      else {
        if (selected) actions.push(actionButton("strategic-operation-assign", operation.assigned_fleet ? "Reassign selected fleet" : "Assign selected fleet", operation.id));
        if (["rescue_salvage", "prize_recovery"].includes(operation.kind.kind) && selected) actions.push(actionButton("strategic-operation-recover", "Recover with selected fleet", operation.id, "is-primary"));
      }
      if (operation.kind.kind === "syndicate_megaproject") {
        const hostSystem = operation.kind.system;
        const hostOwned = this.ctx.state.systems.find((system) => system.id === hostSystem)?.owner === this.ctx.state.playerId;
        if (hostOwned) actions.push(`<label class="deck-operation-contribution"><span>Local contribution</span><input id="deck-op-units-${escAttr(operation.id)}" type="number" min="1" step="1" value="25"><button type="button" data-deck-act="strategic-operation-contribute" data-operation="${escAttr(operation.id)}">Commit goods</button></label>`);
      } else if (operationAbandonable(operation)) actions.push(actionButton("strategic-operation-abandon", "Abandon", operation.id, "is-danger"));
    }
    const hullArt = operationHullArt(operation);
    const left = operationLeft(operation, liveSimTime());
    const time = operation.state === "completed" ? "complete" : left == null ? "no deadline"
      : `${left >= 3_600 ? fmtDays(left) : fmtDur(left)} remaining`;
    const brief = operation.briefing;
    const readiness = brief ? `<div class="deck-operation__brief"><b>${esc(brief.difficulty)}</b><span>${esc(brief.suitable_fleets)}</span></div>` : "";
    return `<article class="deck-operation is-${operation.state}">${hullArt ? `<img src="${escAttr(hullArt)}" alt="">` : ""}<div><header><span>${operationIcon(operation)}<b>${esc(operationTitle(operation))}</b></span><em>${esc(human(operation.state))}</em></header>${readiness}<p>${esc(operationCopy(operation))}</p><div class="deck-meter"><i style="width:${pct.toFixed(1)}%"></i></div><footer><span>${operation.progress}/${operation.goal}</span><span>${esc(operationReward(operation))}</span><span>${esc(time)}</span><span class="deck-stale-value">${esc(informationDelay(Math.max(0, liveSimTime() - operation.reported_at)))}</span></footer>${actions.length ? `<div class="deck-operation__actions">${actions.join("")}</div>` : ""}</div></article>`;
  }

  private followUpControls(o: OperationView): string {
    const escort = o.kind.kind === "freight_escort";
    const combat = escort || o.kind.kind === "privateer_patrol" || o.kind.kind === "pirate_bounty" || o.kind.kind === "combat_objective";
    const survey = o.kind.kind === "survey_expedition";
    const recovery = o.kind.kind === "rescue_salvage" || o.kind.kind === "prize_recovery";
    const own = this.ctx.state.ghosts.filter(g => g.own);
    const fleets = own.filter(g => survey ? surveyCapable(g)
      : combat ? guardCapable(g) : fleetReadiness(g).cargoCapacity > 0);
    const chosen = this.operationFleets.get(o.id) ?? o.assigned_fleet ?? "";
    const fleet = fleets.find(g => g.id === chosen);
    const charges = own.filter(g => fleetReadiness(g).cargoCapacity > 0 && g.id !== chosen);
    const chargeId = this.operationCharges.get(o.id) ?? "";
    const select = (rows: GhostView[], value: string, input: string, title: string) =>
      `<label>${title}<select data-deck-input="${input}" data-operation="${escAttr(o.id)}"><option value="">Choose fleet</option>${rows.map(g => `<option value="${escAttr(g.id)}" ${g.id === value ? "selected" : ""}>${esc(shipKindLabel(g.kind))} · ${esc(g.id)}${g.docked ? " · docked" : ""}</option>`).join("")}</select></label>`;
    const warnings = fleet ? dispatchWarnings(fleet, escort && o.kind.kind === "freight_escort" ? o.kind.origin : o.target_pos) : [];
    const charge = charges.find(g => g.id === chargeId);
    if (charge) warnings.push(...dispatchWarnings(charge, o.target_pos).map(w => `Freighter: ${w}`));
    // §hold-room: the room each good fills, not a count of units.
    if (fleet && o.kind.kind === "rescue_salvage" && fleetReadiness(fleet).holdFree < o.kind.units * commodityRoom(o.kind.commodity))
      warnings.push(`Recovery needs room for ${o.kind.units}. Unload first.`);
    if (fleet && o.kind.kind === "prize_recovery") {
      // §modules-as-goods: a cache's modules ride in the hold like any cargo.
      const needed = o.kind.prize === "breacher_cache" ? 4 : 8; // modules, a quarter room each
      if (fleetReadiness(fleet).holdFree < needed) warnings.push(`Needs room for ${needed} modules. Unload first.`);
    }
    return `<div class="deck-operation-dispatch">${select(fleets, chosen, "operation-fleet", survey ? "Scout" : combat ? "Combat fleet" : "Freighter")}${escort ? select(charges, chargeId, "operation-charge", "Protect") : ""}
      ${warnings.length ? `<div class="deck-dispatch-warning" role="status">${warnings.map(esc).join("<br>")}</div>` : ""}
      <div><button type="button" data-deck-act="strategic-operation-dispatch" data-operation="${escAttr(o.id)}" ${!fleet || (escort && !charge) ? "disabled" : ""}>${escort ? "Assign guard" : survey ? "Plot survey" : combat ? "Plot assault" : recovery ? "Plot recovery" : "Select Freighter"}</button>
      <button type="button" data-deck-act="strategic-operation-locate" data-operation="${escAttr(o.id)}">${escort ? "Show home departure" : "Show destination"}</button></div>
      ${escort ? "<small>Meet at home, then dispatch the guarded Freighter to Market.</small>" : ""}</div>`;
  }

  private operationAction(action: string, button: HTMLButtonElement): void {
    const operation_id = button.dataset.operation;
    if (!operation_id) return;
    const op = this.ctx.state.operations.find(o => o.id === operation_id);
    if (action === "strategic-operation-locate" && op) {
      this.ctx.renderer.centerOnWorld(op.kind.kind === "freight_escort" ? op.kind.origin : op.target_pos);
      return;
    }
    if (action === "strategic-operation-dispatch" && op) {
      const fleet_id = this.operationFleets.get(op.id) ?? op.assigned_fleet;
      const fleet = this.ctx.state.ghosts.find(g => g.own && g.id === fleet_id);
      if (!fleet) return;
      if (op.kind.kind === "freight_escort") {
        const protected_fleet = this.operationCharges.get(op.id);
        if (!protected_fleet || !guardCapable(fleet) || !this.ctx.state.ghosts.some(g => g.own && g.id === protected_fleet && fleetReadiness(g).cargoCapacity > 0)) return;
        this.ctx.intent.beginFleetCommand([
          { type: "AssignOperationFleet", operation_id, fleet_id: fleet.id, protected_fleet },
          { type: "GuardFleet", interceptor_id: fleet.id, target_id: protected_fleet },
        ]);
      } else {
        this.hooks.selectFleets([fleet.id]);
        if (op.kind.kind === "survey_expedition")
          this.ctx.intent.beginFleetCommand([
            { type: "AssignOperationFleet", operation_id, fleet_id: fleet.id },
            { type: "SurveySystem", fleet_id: fleet.id, system_id: op.kind.system },
          ]);
        else if (["rescue_salvage", "prize_recovery", "privateer_patrol", "pirate_bounty", "combat_objective"].includes(op.kind.kind))
          this.ctx.intent.beginFleetCommand([
            { type: "AssignOperationFleet", operation_id, fleet_id: fleet.id },
            { type: "MoveShip", ship_id: fleet.id, dest: op.target_pos },
          ]);
        else this.ctx.intent.beginFleetCommand({ type: "AssignOperationFleet", operation_id, fleet_id: fleet.id });
      }
      return;
    }
    if (action === "strategic-operation-accept") this.ctx.send({ type: "AcceptOperation", operation_id });
    else if (action === "strategic-operation-abandon") this.ctx.send({ type: "AbandonOperation", operation_id });
    else if (action === "strategic-operation-assign" && this.ctx.state.selectedShipId) this.ctx.intent.beginFleetCommand({ type: "AssignOperationFleet", operation_id, fleet_id: this.ctx.state.selectedShipId });
    else if (action === "strategic-operation-recover" && this.ctx.state.selectedShipId) this.ctx.intent.beginFleetCommand({ type: "RecoverOperation", operation_id, fleet_id: this.ctx.state.selectedShipId });
    else if (action === "strategic-operation-contribute") {
      const operation = this.ctx.state.operations.find((o) => o.id === operation_id);
      if (operation?.kind.kind !== "syndicate_megaproject") return;
      const commodity: Commodity = operation.kind.stage === 0 ? "alloys" : operation.kind.stage === 1 ? "electronics" : "machinery";
      const units = Math.max(1, Math.floor(Number(this.root.querySelector<HTMLInputElement>(`#deck-op-units-${cssEscape(operation_id)}`)?.value) || 0));
      this.ctx.send({ type: "ContributeOperationCargo", operation_id, commodity, units });
    }
    const operation = this.ctx.state.operations.find((candidate) => candidate.id === operation_id);
    this.hooks.notice(`<b>Operation command sent</b>${operation ? ` · ${esc(operationTitle(operation))}` : ""}.`);
  }

  private syndicateHtml(): string {
    const syndicate = this.ctx.state.syndicate;
    const invites = this.ctx.state.syndicateInvites;
    let membership = "";
    if (syndicate) {
      const members = syndicate.members.map((member) => {
        const own = member.id === this.ctx.state.playerId;
        const maySet = syndicate.is_founder && member.id !== syndicate.founder && !own;
        const controls = maySet ? `<div class="deck-role-actions">${(["member", "quartermaster", "officer"] as SyndicateRole[]).map((role) => `<button type="button" data-deck-act="strategic-syndicate-role" data-member="${escAttr(member.id)}" data-role="${role}" aria-pressed="${member.role === role}">${esc(human(role))}</button>`).join("")}</div>` : "";
        const presence = this.ctx.state.social?.members.find((m) => m.player === member.id);
        return `<div class="deck-member-row${presence?.online ? " is-online" : presence ? " is-offline" : ""}"><span><i></i><span><b>${esc(member.name)}</b><small>${own ? "You · " : ""}${esc(human(member.role))}${presence && !own ? ` · ${esc(presenceText(presence))}` : ""}</small></span></span>${controls}</div>`;
      }).join("");
      const invite = ["founder", "officer"].includes(syndicate.my_role) ? `<div class="deck-inline-form"><input id="deck-syndicate-invite" maxlength="32" placeholder="Corporation name"><button type="button" data-deck-act="strategic-syndicate-invite">Invite</button></div>${syndicate.invited.length ? `<p class="deck-muted">Invited · ${syndicate.invited.map(esc).join(" · ")}</p>` : ""}` : "";
      membership = `<section class="deck-section"><header><div><h3>${esc(syndicate.name)}</h3><p>Mutual non-engagement pact · your role ${esc(human(syndicate.my_role))}.</p></div><b>${syndicate.members.length}</b></header><div class="deck-member-list">${members}</div><div class="deck-section__action"><button type="button" data-deck-act="strategic-syndicate-channel">Open channel</button></div>${invite}<div class="deck-danger-row"><button type="button" class="is-danger" data-deck-act="strategic-syndicate-leave">Leave syndicate</button>${syndicate.is_founder ? `<button type="button" class="is-danger" data-deck-act="strategic-syndicate-dissolve">Dissolve syndicate</button>` : ""}</div></section>`;
    } else {
      const invitationRows = invites.map((invite) => `<div class="deck-member-row"><span>${icon("syndicate", "sm")}<span><b>${esc(invite.name)}</b><small>Invitation received</small></span></span><button type="button" data-deck-act="strategic-syndicate-accept" data-syndicate="${escAttr(invite.id)}">Accept</button></div>`).join("");
      membership = `<section class="deck-section"><header><div><h3>Form a syndicate</h3><p>Members cannot raid, attack or blockade one another; allied holdings tint green only as membership light arrives.</p></div></header><div class="deck-inline-form"><input id="deck-syndicate-create" maxlength="32" placeholder="Syndicate name"><button type="button" class="is-primary" data-deck-act="strategic-syndicate-create">Create</button></div>${invitationRows ? `<div class="deck-subhead"><b>Invitations</b><span>${invites.length}</span></div><div class="deck-member-list">${invitationRows}</div>` : `<div class="deck-empty-inline">No pending invitations.</div>`}</section>`;
    }
    return `<section class="deck-page deck-syndicate"><header class="deck-page__lead"><span>Alliance · permissions · formal conflict</span><h2>Syndicate & diplomacy</h2><p>Relationship changes are light-delayed and include a no-surprise separation window.</p></header>${membership}${syndicate?.projects ? this.projectsHtml(syndicate.projects) : ""}${this.friendInvitesHtml()}${this.diplomacyHtml()}</section>`;
  }

  /** §syndicate-projects (R6): the week's ladder, points and bonuses. */
  private projectsHtml(p: SyndicateProjectsView): string {
    const st = this.ctx.state;
    const rung = currentRung(p);
    const steps = p.rungs.map((r, i) => `<li class="${i < p.rung ? "is-done" : i === p.rung ? "is-current" : ""}" title="${escAttr(`${r.goal.toLocaleString()} ${label(r.goods)} · +${r.points} pts`)}">${i < p.rung ? "✓" : i + 1}</li>`).join("");
    let give = `<div class="deck-empty-inline">Ladder done · next week ${fmtDays(p.week_ends_in)}</div>`;
    if (rung) {
      const worlds = givingWorlds(st.systems, st.playerId, rung.goods);
      const need = rung.goal - p.progress;
      const pct = Math.min(100, p.progress / rung.goal * 100);
      give = `<div class="deck-project-rung"><b>${esc(label(rung.goods))} ${p.progress.toLocaleString()} / ${rung.goal.toLocaleString()}</b><small>+${rung.points} pts</small></div><div class="deck-meter"><i style="width:${pct.toFixed(1)}%"></i></div>` +
        (worlds.length ? `<div class="deck-inline-form"><select id="deck-project-system">${worlds.map(([s, n]) => `<option value="${escAttr(s.id)}">${esc(systemName(s.id))} · ${n.toLocaleString()}</option>`).join("")}</select><input id="deck-project-units" type="number" min="1" max="${need}" value="${Math.min(need, worlds[0][1])}"><button type="button" class="is-primary" data-deck-act="strategic-syndicate-project-give">Give</button></div>`
          : `<div class="deck-empty-inline">No ${esc(label(rung.goods))} on your worlds.</div>`);
    }
    const bonuses = p.bonuses.map((b) => {
      const wait = b.next_in !== null ? ` · yours ${roman(b.mine)}, ${roman(b.mine + 1)} in ${fmtDays(b.next_in)}` : "";
      const raise = p.may_raise && b.next_cost !== null ? `<button type="button" data-deck-act="strategic-syndicate-bonus-raise" data-bonus="${b.bonus}" ${p.points >= b.next_cost ? "" : "disabled"}>${roman(b.level + 1)} · ${b.next_cost} pts</button>` : "";
      return `<div class="deck-member-row"><span><span><b>${esc(bonusLine(b))}</b>${wait ? `<small>${esc(wait.slice(3))}</small>` : ""}</span></span>${raise}</div>`;
    }).join("");
    const given = p.given.length ? `<p class="deck-muted">This week · ${p.given.map(([name, n]) => `${esc(name)} ${n.toLocaleString()}`).join(" · ")}</p>` : "";
    return `<section class="deck-section deck-projects"><header><div><h3>Projects · week ${p.week}</h3><p>Closes in ${esc(fmtDays(p.week_ends_in))}${medalCountsHtml(p.medals)}</p></div><b title="Project points to spend">${p.points} pts</b></header><ol class="deck-ladder">${steps}</ol>${give}<div class="deck-member-list">${bonuses}</div>${given}</section>`;
  }

  /** §settle-near-friends (R7): links that hold a home near this corporation. */
  private friendInvitesHtml(): string {
    const open = this.ctx.state.social?.invites ?? [];
    const rows = open.map((invite) => `<div class="deck-member-row deck-invite-row"><input readonly value="${escAttr(inviteLink(invite.code))}" aria-label="Invite link"><button type="button" data-deck-act="strategic-syndicate-link-copy" data-code="${escAttr(invite.code)}">Copy</button><small>${esc(invite.near)} · ${fmtDays(Math.max(0, invite.until - liveSimTime()))}</small><button type="button" class="is-danger" data-deck-act="strategic-syndicate-link-revoke" data-code="${escAttr(invite.code)}">Revoke</button></div>`).join("");
    const hint = "A friend who lands through the link gets the held home and joins your syndicate if it has a seat.";
    return `<section class="deck-section"><header><div><h3>Invite a friend</h3></div><b>${open.length}/${INVITES_MAX}</b></header>${rows ? `<div class="deck-member-list">${rows}</div>` : ""}<div class="deck-section__action"><button type="button" data-deck-act="strategic-syndicate-link-new" title="${escAttr(hint)}" ${open.length >= INVITES_MAX ? "disabled" : ""}>New invite link</button></div></section>`;
  }

  private diplomacyHtml(): string {
    const diplomacy = this.ctx.state.diplomacy;
    const incoming = diplomacy?.incoming.map((proposal) => `<div class="deck-member-row"><span><span><b>${esc(proposal.name)}</b><small>Proposes ${esc(human(proposal.treaty))} · expires in ${fmtDur(Math.max(0, proposal.expires_at - liveSimTime()))}</small></span></span><div class="deck-role-actions"><button type="button" data-deck-act="strategic-syndicate-treaty-response" data-proposal="${proposal.id}" data-accept="1">Accept</button><button type="button" data-deck-act="strategic-syndicate-treaty-response" data-proposal="${proposal.id}" data-accept="0">Decline</button></div></div>`).join("") ?? "";
    const relations = diplomacy?.relations.map((relation) => {
      const status = relation.war_activates_at ? `War activates in ${fmtDur(Math.max(0, relation.war_activates_at - liveSimTime()))}` : relation.reprisal_until && relation.reprisal_until > liveSimTime() ? `Reprisal · ${fmtDur(relation.reprisal_until - liveSimTime())}` : human(relation.state);
      const endable = relation.state === "non_aggression" || relation.state === "ceasefire";
      return `<div class="deck-member-row"><span><span><b>${esc(relation.name)}</b><small>${esc(status)}</small></span></span>${endable ? `<button type="button" data-deck-act="strategic-syndicate-cancel-treaty" data-target="${escAttr(relation.other)}">End agreement</button>` : ""}</div>`;
    }).join("") ?? "";
    return `<section class="deck-section"><header><div><h3>Diplomacy</h3><p>Declarations, proposals and cancellations take effect only after their notice window.</p></div><b>${(diplomacy?.incoming.length ?? 0) + (diplomacy?.relations.length ?? 0)}</b></header>${incoming ? `<div class="deck-subhead"><b>Incoming proposals</b><span>${diplomacy!.incoming.length}</span></div><div class="deck-member-list">${incoming}</div>` : ""}<div class="deck-member-list">${relations || `<div class="deck-empty-inline">No arrived agreements or declarations.</div>`}</div><div class="deck-diplomacy-form"><input id="deck-diplomacy-name" maxlength="32" placeholder="Corporation name"><button type="button" data-deck-act="strategic-syndicate-propose-nap">Offer pact</button><button type="button" data-deck-act="strategic-syndicate-propose-ceasefire">Offer ceasefire</button><button type="button" class="is-danger" data-deck-act="strategic-syndicate-declare-war">Declare war</button></div></section>`;
  }

  private syndicateAction(action: string, button: HTMLButtonElement): void {
    if (action === "strategic-syndicate-channel") {
      this.hooks.go({ name: "comms" });
      return;
    }
    if (action === "strategic-syndicate-link-new") { this.ctx.send({ type: "CreateInvite" }); return; }
    if (action === "strategic-syndicate-project-give") {
      const rung = this.ctx.state.syndicate?.projects && currentRung(this.ctx.state.syndicate.projects);
      const system_id = this.root.querySelector<HTMLSelectElement>("#deck-project-system")?.value;
      const units = Math.floor(Number(this.root.querySelector<HTMLInputElement>("#deck-project-units")?.value));
      if (rung && system_id && units > 0) this.ctx.send({ type: "ContributeSyndicateProject", system_id, commodity: rung.goods, units });
      return;
    }
    if (action === "strategic-syndicate-bonus-raise" && button.dataset.bonus) {
      this.ctx.send({ type: "RaiseSyndicateBonus", bonus: button.dataset.bonus as SyndicateBonus });
      return;
    }
    if (action === "strategic-syndicate-link-revoke" && button.dataset.code) { this.ctx.send({ type: "RevokeInvite", code: button.dataset.code }); return; }
    if (action === "strategic-syndicate-link-copy" && button.dataset.code) {
      const link = inviteLink(button.dataset.code);
      void navigator.clipboard?.writeText(link).then(() => { button.textContent = "Copied"; }, () => {
        button.closest(".deck-invite-row")?.querySelector<HTMLInputElement>("input")?.select();
      });
      return;
    }
    const value = (id: string) => this.root.querySelector<HTMLInputElement>(`#${id}`)?.value.trim() ?? "";
    if (action === "strategic-syndicate-create") this.ctx.send({ type: "CreateSyndicate", name: value("deck-syndicate-create") || "Syndicate" });
    else if (action === "strategic-syndicate-invite") {
      const name = value("deck-syndicate-invite");
      if (name) this.ctx.send({ type: "InviteToSyndicate", name });
    } else if (action === "strategic-syndicate-accept" && button.dataset.syndicate) this.ctx.send({ type: "AcceptSyndicateInvite", syndicate_id: button.dataset.syndicate });
    else if (action === "strategic-syndicate-leave") this.ctx.send({ type: "LeaveSyndicate" });
    else if (action === "strategic-syndicate-dissolve") this.ctx.send({ type: "DissolveSyndicate" });
    else if (action === "strategic-syndicate-role" && button.dataset.member && button.dataset.role) this.ctx.send({ type: "SetSyndicateRole", member: button.dataset.member, role: button.dataset.role as SyndicateRole });
    else if (action === "strategic-syndicate-treaty-response") {
      const proposal_id = Number(button.dataset.proposal);
      if (Number.isFinite(proposal_id)) this.ctx.send({ type: "RespondTreaty", proposal_id, accept: button.dataset.accept === "1" });
    } else if (action === "strategic-syndicate-cancel-treaty" && button.dataset.target) this.ctx.send({ type: "CancelTreaty", target: button.dataset.target });
    else {
      const target_name = value("deck-diplomacy-name");
      if (!target_name) return;
      if (action === "strategic-syndicate-declare-war") this.ctx.send({ type: "DeclareWar", target_name });
      else if (action === "strategic-syndicate-propose-nap") this.ctx.send({ type: "ProposeTreaty", target_name, treaty: "non_aggression" });
      else if (action === "strategic-syndicate-propose-ceasefire") this.ctx.send({ type: "ProposeTreaty", target_name, treaty: "ceasefire" });
    }
    this.hooks.notice("<b>Diplomatic command sent</b> · its legal effect waits for arrived notice.");
  }

  private factionHtml(): string {
    const charter = this.ctx.state.charter;
    if (!charter) return `<section class="deck-page"><header class="deck-page__lead"><span>Terran Charter Authority</span><h2>Faction</h2><p>No charter is present in the served account picture.</p></header></section>`;
    const rows = this.ctx.state.charterLadder.map(([title, at], index) => `<div class="deck-charter-row${title === charter.title ? " is-active" : ""}"><span>${title === charter.title ? "▸" : "·"} ${esc(title)}</span><small>${index === 0 ? "at" : index === 1 ? "below" : "at"} ${at.toFixed(0)}</small></div>`).join("");
    const bites = charter.tariff_mult > 1 || charter.market_penalty_frac > 0;
    // §standing-ladder (R3): buying back stops at full standing; the grades
    // above it are earned.
    const full = charter.full_standing ?? charter.max_standing;
    const shortfall = Math.max(0, full - charter.standing);
    const points = Math.max(1, Math.ceil(Math.min(shortfall, 20)));
    const grades = this.ctx.state.gradeLadder.map(([title, at]) => `<div class="deck-charter-row${title === charter.grade_title ? " is-active" : ""}"><span>${title === charter.grade_title ? "▸" : "·"} ${esc(title)}</span><small>at ${at.toFixed(0)}</small></div>`).join("");
    const next = charter.next_grade ? `${esc(charter.next_grade.title)} at ${charter.next_grade.at.toFixed(0)}` : "Top grade";
    const today = `${fmt(Math.min(charter.earned_today, charter.earn_per_day))}/${fmt(charter.earn_per_day)}`;
    const gradeHtml = charter.grade_title ? `<section class="deck-section"><header><div><h3>Grade · ${esc(charter.grade_title)}</h3><p title="Contracts, Signals and the ledger earn standing past ${full.toFixed(0)}, up to ${fmt(charter.earn_per_day)} a day. Credits can't buy it.">Next: ${next} · today ${today}</p></div></header><div class="deck-stat-grid"><dl class="deck-stat"><dt>Freight fee</dt><dd>×${charter.tariff_mult.toFixed(2)}</dd></dl><dl class="deck-stat"><dt>Freight lift</dt><dd>${fmt(charter.shipment_cap)} / departure</dd></dl></div><div class="deck-charter-ladder">${grades}</div></section>` : "";
    return `<section class="deck-page deck-faction"><header class="deck-page__lead"><span>Legal status · served Authority ledger</span><h2>Terran Charter Authority</h2><p>Standing is priced law, not reputation. Citations land only when their light reaches the Market Hub.</p></header>${this.gripHtml(charter)}<div class="deck-stat-grid"><dl class="deck-stat${charter.status === "good_standing" ? "" : " is-warn"}"><dt>Charter</dt><dd>${esc(charter.title)}</dd></dl><dl class="deck-stat"><dt>Standing</dt><dd>${charter.standing.toFixed(0)} / ${charter.max_standing.toFixed(0)}</dd></dl><dl class="deck-stat${bites ? " is-warn" : ""}"><dt>Freight tariff</dt><dd>×${charter.tariff_mult.toFixed(2)}</dd></dl><dl class="deck-stat${bites ? " is-warn" : ""}"><dt>Exchange penalty</dt><dd>${(charter.market_penalty_frac * 100).toFixed(1)}%</dd></dl></div><section class="deck-section"><header><div><h3>Charter ladder</h3><p>Each band names the tariff on Authority freight and the cut of Exchange trades.</p></div></header><div class="deck-charter-ladder">${rows}</div></section>${gradeHtml}${this.licencesHtml(charter)}${this.claimsHtml(charter)}${shortfall > 0 ? `<section class="deck-section"><header><div><h3>Reinstatement</h3><p>Credits are burned; only standing actually restored is charged.</p></div></header><div class="deck-reinstatement"><input data-deck-input="reinstatement" type="number" min="1" step="1" value="${points}"><span data-reinstatement-cost>${fmt(points * charter.reinstate_cost_per_point)} Cr for ${points} pts</span><button type="button" class="is-primary" data-deck-act="strategic-faction-pay">Pay</button></div></section>` : ""}</section>`;
  }

  /** §licences: the Authority's hull licences, held or for sale. Only hulls
   * research has unlocked show (licenceNote is empty for the rest). */
  private licencesHtml(charter: CharterView): string {
    const rows = (charter.licences ?? []).filter((l) => licenceNote(l)).map((l) =>
      `<div class="deck-demand"><span><b>${esc(shipKindLabel(l.hull))}</b><small>${esc(licenceNote(l))}</small></span>`
      + (licenceForSale(l) ? `<button type="button" class="is-primary" data-deck-act="strategic-buy-licence" data-hull="${l.hull}">Buy</button>` : "")
      + `</div>`).join("");
    return rows ? `<section class="deck-section"><header><div><h3>Licences</h3><p title="Research is the skill to build a hull; the Authority's licence is the right to. Orbital Shipyard and Capital Slipway hulls need one. Bought once, kept for good; standing sets the price.">Orbital Shipyard and Slipway hulls · bought once</p></div></header>${rows}</section>` : "";
  }

  /** §claims: the Authority's claims held, soonest lapse first. */
  private claimsHtml(charter: CharterView): string {
    const held = claimsHeld(charter);
    if (!held.length) return "";
    const now = liveSimTime();
    const rows = held.map((claim) => `<div class="deck-demand"><span><b>${esc(systemName(claim.system))}</b>`
      + `<small>Lapses in ${esc(fmtBuildDur(Math.max(0, claim.lapses_at - now)))}</small></span></div>`).join("");
    return `<section class="deck-section"><header><div><h3>Claims</h3><p title="${esc(CLAIM_TIP)}">New colonies · used on landing</p></div></header>${rows}</section>`;
  }

  /** §season: the grip in force, the levy, Resolve, the Authority's demands
   * and its crackdowns. */
  private gripHtml(charter: CharterView): string {
    const season = this.ctx.state.season;
    const sq = charter.squeeze;
    const now = liveSimTime();
    const grip = season?.grip ?? "charter";
    const levy = charter.levy_frac ?? 0;
    const own = sq ? Math.max(0, levy - sq.base) : 0;
    const pct = (f: number) => `${(f * 100).toFixed(f * 100 % 1 ? 1 : 0)}%`;
    const stats = `<div class="deck-stat-grid"><dl class="deck-stat${levy > 0 ? " is-warn" : ""}"><dt>Levy</dt><dd title="On every Hub sale and purchase">${levy > 0 ? pct(levy) : "None"}${own > 0.0005 ? ` <small>+${pct(own)} yours</small>` : ""}</dd></dl>`
      + `<dl class="deck-stat"><dt>Galaxy maturity</dt><dd>${season ? `${Math.round(season.maturity * 100)}%` : "—"}</dd></dl>`
      + `<dl class="deck-stat"><dt>Resolve</dt><dd title="${esc(RESOLVE_TIP)}">${Math.round((sq?.resolve ?? 0) * 100)}% <small>${esc(RESOLVE_BAND[resolveBand(sq?.resolve ?? 0)])}</small></dd></dl>`
      + `<dl class="deck-stat${sq?.refusals ? " is-warn" : ""}"><dt>Refusals</dt><dd title="Three bring an Authority squadron">${sq?.refusals ?? 0} / 3</dd></dl>`
      + `<dl class="deck-stat"><dt>Levy paid</dt><dd>${fmt(sq?.levy_paid ?? 0)} Cr</dd></dl>`
      + `<dl class="deck-stat"><dt>Authority war fleet</dt><dd title="Grows with maturity and the galaxy's Resolve (${Math.round((season?.galaxy_resolve ?? 0) * 100)}%)">${season?.war_fleet ?? 0} hulls</dd></dl></div>`;
    const rows: string[] = [];
    if (sq?.rise) rows.push(`<div class="deck-demand"><span><b>Levy rise · +${pct(sq.rise.points)}</b><small>${esc(label(sq.rise.commodity))} · ${fmt(sq.rise.trade)} Cr trade · ${esc(fmtBuildDur(Math.max(0, sq.rise.due - now)))} to answer</small></span>`
      + `<button type="button" class="is-primary" data-deck-act="strategic-levy" data-accept="1">Accept</button>`
      + `<button type="button" class="is-danger" data-deck-act="strategic-levy" data-accept="0" title="Embargoes ${esc(label(sq.rise.commodity))} until ${fmt(sq.rise.arrears)} Cr arrears are paid">Refuse</button></div>`);
    for (const req of sq?.requisitions ?? []) rows.push(`<div class="deck-demand"><span><b>Requisition · ${esc(demandText(req.demand, label))}</b><small>${req.demand.kind === "goods" ? "From your Hub warehouse · " : ""}${esc(fmtBuildDur(Math.max(0, req.due - now)))} left</small></span>`
      + `<button type="button" class="is-primary" data-deck-act="strategic-requisition" data-id="${req.id}" data-pay="1">Pay</button>`
      + `<button type="button" class="is-danger" data-deck-act="strategic-requisition" data-id="${req.id}" data-pay="0" title="Standing −8, levy +1%">Refuse</button></div>`);
    for (const embargo of sq?.embargoes ?? []) rows.push(`<div class="deck-demand"><span><b>Embargo · ${esc(label(embargo.commodity))}</b><small>Arrears ${fmt(embargo.arrears)} Cr</small></span>`
      + `<button type="button" class="is-primary" data-deck-act="strategic-arrears" data-commodity="${esc(embargo.commodity)}">Pay arrears</button></div>`);
    // §season phase 4: the spy's clock and the war.
    if (season?.rebel_since != null) rows.unshift(`<div class="deck-demand is-war"><span><b>At war with the Authority</b><small>No Hub, no Authority freight · bounties on your hulls · war fleet ${season.war_fleet} hulls</small></span></div>`);
    else if (season?.war_at != null && season.war_at > now) rows.unshift(`<div class="deck-demand is-war"><span><b>An Authority spy</b><small>War in ${esc(fmtBuildDur(season.war_at - now))} · bring freight home</small></span></div>`);
    for (const c of season?.crackdowns ?? []) {
      const live = c.until > now && !c.exposed;
      const evidence = c.evidence_until > now && !c.exposed ? (c.read ? "evidence read" : `evidence · Scout within ${fmtBuildDur(c.evidence_until - now)}`) : "";
      rows.push(`<div class="deck-demand is-crackdown"><span><b>${esc(crackdownTitle(c.kind))}${c.exposed ? " · exposed" : ""}</b>`
        + `<small>${[c.system ? esc(systemName(c.system)) : "", live ? `${esc(fmtBuildDur(c.until - now))} left` : "", esc(evidence)].filter(Boolean).join(" · ")}</small></span></div>`);
    }
    return `<section class="deck-section deck-grip"><header><div><h3>Grip ${gripNumeral(grip)} · ${esc(gripTitle(grip))}</h3><p>${esc(DECREE_TEXT[grip])}</p></div></header>${stats}${rows.join("")}</section>${this.keystoneHtml()}`;
  }

  /** §season phase 5: your Keystone, every other one, Gate Step and the clocks. */
  private keystoneHtml(): string {
    const st = this.ctx.state;
    const season = st.season;
    if (!season) return "";
    const now = liveSimTime();
    const parts: string[] = [];
    const mine = season.keystone;
    if (mine) {
      const world = st.systems.find((system) => system.id === mine.system);
      const feed = keystoneFeed(season, (world?.stockpile ?? {}) as Partial<Record<Commodity, number>>);
      const rows = feed.map(([good, need, paid, have]) => {
        const units = Math.max(0, Math.min(need - paid, have));
        return `<div class="deck-demand"><span><b>${esc(label(good))} · ${fmt(paid)} / ${fmt(need)}</b><small>${fmt(have)} at ${esc(systemName(mine.system))}</small></span>`
          + `<button type="button" class="is-primary" data-deck-act="strategic-feed" data-system="${esc(mine.system)}" data-commodity="${esc(good)}" data-units="${Math.floor(units)}" ${units >= 1 ? "" : "disabled"}>Feed ${fmt(units)}</button></div>`;
      }).join("");
      const relics = mine.relics_needed ? `<p>Relics held ${mine.relics_held} / ${mine.relics_needed}</p>` : "";
      const gates = mine.powers ? this.gateStepHtml(mine.gates) : "";
      parts.push(`<section class="deck-section deck-keystone"><header><div><h3>Keystone · ${esc(systemName(mine.system))} · level ${mine.level}/10</h3>`
        + `<p>${mine.powers ? "Network Sight · Instant Command · Gate Step · Precursor Fleet" : "Feeding it is war with the Authority. No powers until level 10."}</p></div></header>${relics}${rows}${gates}</section>`);
    }
    const others = season.keystones.filter((k) => k.system !== mine?.system);
    const clock = season.ending ? "" : season.ends_at ? `<p>The season ends in ${esc(fmtBuildDur(Math.max(0, season.ends_at - now)))} if no Seal fires.</p>` : "";
    if (others.length || clock) {
      parts.push(`<section class="deck-section"><header><div><h3>The race</h3>${clock}</div></header>${others.map((k) =>
        `<div class="deck-demand is-crackdown"><span><b>${esc(systemName(k.system))} · level ${k.level}/10</b><small>${esc(k.holder_name ?? "unheld")}${k.seal_at ? ` · Seal in ${esc(fmtBuildDur(Math.max(0, k.seal_at - now)))}` : ""}</small></span></div>`).join("")}</section>`);
    }
    return parts.join("");
  }

  /** Gate Step: your fleets at a gate, and where each can step. */
  private gateStepHtml(gates: string[]): string {
    const st = this.ctx.state;
    const pos = (id: string) => st.galaxy?.systems.find((system) => system.id === id)?.pos;
    const rows = st.ghosts.filter((ghost) => ghost.own).flatMap((ghost) => {
      const at = gates.find((gate) => { const p = pos(gate); return !!p && Math.hypot(p.x - ghost.pos.x, p.y - ghost.pos.y) <= 900; });
      if (!at) return [];
      const buttons = gates.filter((gate) => gate !== at).map((gate) =>
        `<button type="button" data-deck-act="strategic-gate-step" data-fleet="${esc(ghost.id)}" data-system="${esc(gate)}">→ ${esc(systemName(gate))}</button>`).join("");
      return [`<div class="deck-demand"><span><b>Gate Step · ${esc(shipKindLabel(ghost.kind))}</b><small>at ${esc(systemName(at))}</small></span>${buttons}</div>`];
    });
    return rows.join("");
  }

  private syncReinstatement(input: HTMLInputElement): void {
    const charter = this.ctx.state.charter;
    const output = this.root.querySelector<HTMLElement>("[data-reinstatement-cost]");
    if (!charter || !output) return;
    const wanted = Math.max(0, Math.floor(Number(input.value) || 0));
    const points = Math.min(wanted, Math.max(0, (charter.full_standing ?? charter.max_standing) - charter.standing));
    output.textContent = `${fmt(points * charter.reinstate_cost_per_point)} Cr for ${points.toFixed(0)} pts`;
  }

  private payReinstatement(): void {
    const input = this.root.querySelector<HTMLInputElement>("[data-deck-input=reinstatement]");
    const points = Math.max(1, Math.floor(Number(input?.value) || 0));
    this.ctx.send({ type: "PayReinstatement", points });
    this.hooks.notice(`<b>Reinstatement payment sent</b> · ${points} standing requested.`);
  }

  private rankingsHtml(): string {
    const category = RANK_CATS.find((candidate) => candidate.slug === this.rankCategory) ?? RANK_CATS[0];
    const rows = [...this.ctx.state.rankings].sort((a, b) => category.value(b) - category.value(a));
    const categories = RANK_CATS.map((candidate) => `<button type="button" data-deck-act="strategic-rank" data-category="${candidate.slug}" aria-pressed="${candidate.slug === category.slug}">${esc(candidate.short)}</button>`).join("");
    const rankingRows = rows.map((row, index) => `<button type="button" class="deck-ranking-row${row.player_id === this.ctx.state.playerId ? " is-me" : ""}" data-deck-act="strategic-corp" data-corp="${escAttr(row.player_id)}"><b>${index + 1}</b><span><strong>${esc(row.name)}</strong>${row.player_id === this.ctx.state.playerId ? `<small>you</small>` : ""}${row.titles.map((title) => `<em>${esc(title)}</em>`).join("")}${medalChipsHtml(row)}</span><output>${esc(category.format(row))}</output></button>`).join("");
    return `<section class="deck-page deck-rankings"><header class="deck-page__lead"><span>Published campaign ledger</span><h2>Rankings</h2><p>The same light-delayed close is published to every corporation.</p></header><section class="deck-section"><header><div><h3>${esc(category.label)}</h3><p>${esc(category.copy)}</p></div><b>${rows.length}</b></header><div class="deck-rank-categories">${categories}</div><div class="deck-ranking-table">${rankingRows || `<div class="deck-empty-inline">No ledger published yet; the first close lands shortly after campaign start.</div>`}</div></section>${this.syndicateRankingsHtml()}</section>`;
  }

  /** §syndicate-projects (R6): the syndicates' table and their project medals. */
  private syndicateRankingsHtml(): string {
    const rows = this.ctx.state.syndicateRankings;
    if (!rows.length) return "";
    const mine = this.ctx.state.syndicate?.id;
    const body = rows.map((row, index) => `<div class="deck-ranking-row${row.id === mine ? " is-me" : ""}"><b>${index + 1}</b><span><strong>${esc(row.name)}</strong><small>${row.members} members · ${row.week_points} pts this week</small>${medalCountsHtml(row.medals)}</span><output>${esc(fmt(row.valuation))} Cr</output></div>`).join("");
    return `<section class="deck-section"><header><div><h3>Syndicates</h3><p>Members' combined valuation · weekly project medals</p></div><b>${rows.length}</b></header><div class="deck-ranking-table">${body}</div></section>`;
  }

  private corpCardHtml(id: string): string {
    const st = this.ctx.state;
    const card = corpCard(st.rankings, id, st.playerId === null ? null : String(st.playerId), st.simTime);
    const back = `<button type="button" data-deck-act="strategic-corp-back">All rankings</button>`;
    return `<section class="deck-page deck-rankings">${card ? corpCardHtml(card, back)
      : `<div class="deck-empty-inline">Not in the published ledger yet.</div>${back}`}</section>`;
  }

  private battleHtml(route: DeckRoute): string {
    const id = route.params?.id ?? "";
    const reportKind = route.params?.report;
    // Route kind identifies the id space. Report counters can numerically
    // equal unrelated engagement ids, so never infer one from the other.
    const capture = reportKind === "capture" ? this.ctx.state.captureReports.find((report) => String(report.id) === id) : undefined;
    if (capture) return this.captureReportHtml(capture);
    const battleReport = reportKind === "battle" ? this.ctx.state.battleReports.find((report) => String(report.id) === id) : undefined;
    if (battleReport) return this.battleReportHtml(battleReport);
    const ongoing = !reportKind ? this.ctx.state.battles.find((battle) => battle.id === id) : undefined;
    if (ongoing) return this.ongoingBattleHtml(ongoing);
    const record = !reportKind ? this.ctx.state.battleRecords.find((entry) => entry.id === id) : undefined;
    const result = record?.outcome ? reportForBattleRecord(record, this.ctx.state.battleReports) : undefined;
    if (result) return this.battleReportHtml(result);
    if (record) return `<section class="deck-page deck-battle-route"><header class="deck-page__lead"><span>Delayed battle record</span><h2>Engagement ${esc(nearestSystemName(record.pos))}</h2><p>${record.outcome ? `Concluded · ${esc(human(record.outcome))}` : "Still arriving at the light frontier."}</p></header><section class="deck-section"><header><div><h3>Round record</h3><p>${record.rounds.length} arrived round${record.rounds.length === 1 ? "" : "s"} · ${esc(human(record.fidelity))} fidelity.</p></div></header><button type="button" class="is-primary" data-deck-act="strategic-battle-view" data-record="${escAttr(record.id)}">${record.outcome ? "View replay" : "Follow battle · delayed"}</button>${record.outcome ? `<button type="button" data-deck-act="strategic-battle-dismiss-record" data-record="${escAttr(record.id)}">Dismiss map marker</button>` : ""}</section></section>`;
    return `<section class="deck-page"><header class="deck-page__lead"><span>Battle report</span><h2>Report unavailable</h2><p>This report is no longer retained in the served picture. Back returns to the Log.</p></header></section>`;
  }

  private ongoingBattleHtml(battle: import("../../protocol").BattleView): string {
    const participants = new Set(battle.participants);
    const fleets = this.ctx.state.ghosts.filter((fleet) => participants.has(fleet.id));
    const own = fleets.filter((fleet) => fleet.own);
    const rival = fleets.filter((fleet) => !fleet.own);
    const ownComposition = sumOwnComposition(own);
    const ownChips = [...ownComposition.entries()].map(([kind, count]) => forceChip(kind, String(count))).join("");
    const rivalChips = rival.map((fleet) => forceChip(fleet.kind, `~${label(fleet.count_class)}`)).join("");
    const elapsed = Math.max(0, liveSimTime() - battle.age - battle.started_at);
    const delay = battleCommandDelay(battle);
    const record = this.ctx.state.battleRecords.find((candidate) => candidate.id === battle.id);
    const withdraw = this.withdrawal.html(battle.id, this.ctx, "data-deck-act", "strategic-battle-withdraw");
    return `<section class="deck-page deck-battle-route"><header class="deck-page__lead"><span>Battle raging · ${esc(informationDelay(battle.age))}</span><h2>Engagement ${esc(nearestSystemName(battle.pos))}</h2><p>Raging ${fmtDur(elapsed)} · forces remaining by your arrived light.</p></header><section class="deck-section deck-force-strip"><header><div><h3>Observed forces</h3><p>${battle.own ? "Exact own composition; rival strength remains bucketed." : "Weapons-fire light reveals only the observed rivals."}</p></div></header>${battle.own ? forceSide("You", ownChips) : ""}${forceSide(battle.own ? "Enemy" : "Forces", rivalChips)}</section>${delay !== null ? `<div class="deck-alert${delay > 20 ? " deck-alert--bad" : ""}"><b>${esc(informationDelay(delay))}</b><span>A command issued now arrives around ${arrivalLocal(delay)}${delay > 20 ? " · too far to steer closely" : " · still in reach"}.</span></div>` : ""}<div class="deck-battle-actions">${withdraw}${record ? `<button type="button" class="is-primary" data-deck-act="strategic-battle-view" data-record="${escAttr(record.id)}">Follow battle · delayed</button>` : ""}${battle.own ? `<button type="button" data-deck-act="strategic-battle-doctrine">Fleet doctrine</button>` : ""}</div></section>`;
  }

  private battleReportHtml(report: BattleReportView): string {
    this.markViewed(report);
    const attacking = report.you === "attacker";
    const ownLost = attacking ? report.attacker_losses : report.target_losses;
    const rivalLost = attacking ? report.target_losses : report.attacker_losses;
    const ownDestroyed = report.outcome === "both_destroyed" || (attacking ? report.outcome === "attacker_destroyed" : report.outcome === "target_destroyed");
    const rivalDestroyed = report.outcome === "both_destroyed" || (attacking ? report.outcome === "target_destroyed" : report.outcome === "attacker_destroyed");
    const verdict = ownDestroyed && rivalDestroyed ? "Mutual destruction" : ownDestroyed ? "Defeat" : rivalDestroyed ? "Victory" : "Both sides survived";
    const record = recordForReport(report);
    const aftermath = report.aftermath;
    const survivors = aftermath?.survivors ?? [];
    const available = survivors.filter((s) => currentSurvivor(this.ctx.state, s));
    const bounty = aftermath ? `${aftermath.bounty_credits > 0 ? "+" : ""}${fmt(aftermath.bounty_credits)} Cr` : "Not recorded";
    const xp = survivors.reduce((n, f) => n + (f.captain ? Math.max(0, f.captain.after.xp - f.captain.before.xp) : 0), 0);
    const select = available.length ? `<button type="button" data-deck-act="strategic-survivor-select-all" data-report="${report.id}">Select survivors</button>` : "";
    return `<section class="deck-page deck-battle-route deck-aftermath">
      <header class="deck-page__lead"><span>Battle aftermath · ${esc(nearestSystemName(report.pos))}</span><h2>${esc(verdict)}</h2><p>${esc(informationDelay(Math.max(0, report.learned_at - report.at_time)))} · report arrived ${fmtDur(Math.max(0, liveSimTime() - report.learned_at))} ago</p></header>
      <div class="deck-stat-grid">
        <dl class="deck-stat"><dt>Your side lost</dt><dd>${esc(losses(ownLost))}</dd></dl>
        <dl class="deck-stat"><dt>Enemy lost</dt><dd>${esc(losses(rivalLost))}</dd></dl>
        <dl class="deck-stat" title="Encounter bounty only. Contract rewards are listed under Operations."><dt>Battle bounty</dt><dd>${esc(bounty)}</dd></dl>
        <dl class="deck-stat"><dt>Captain experience</dt><dd>${aftermath ? `+${fmt(xp)} XP` : "Not recorded"}</dd></dl>
      </div>
      <section class="deck-section"><header><div><h3>Survivors</h3><p>Hull at battle end${survivors.some((s) => s.withdrew) ? " / withdrawal" : ""}</p></div>${select}</header>
        ${survivors.map((s) => this.survivorHtml(report, s)).join("") || `<div class="deck-empty-inline">${aftermath ? "No surviving fleets." : "Survivor details were not recorded for this battle."}</div>`}
      </section>
      <div class="deck-battle-actions">${record ? `<button type="button" class="is-primary" data-deck-act="strategic-battle-view" data-record="${escAttr(record.id)}">View replay</button>` : ""}
        ${report.battle_id ? `<button type="button" data-deck-act="strategic-battle-dismiss" data-report-kind="battle" data-report="${report.id}" title="Hide this map marker. The report stays in the Log.">Dismiss map marker</button>` : ""}
      </div></section>`;
  }

  private survivorBlock(fleet: GhostView | undefined): string {
    return survivorOrderBlock(this.ctx.state, fleet);
  }

  private survivorHtml(report: BattleReportView, survivor: BattleSurvivor): string {
    const st = this.ctx.state;
    const fleet = currentSurvivor(st, survivor);
    const block = this.survivorBlock(fleet);
    const yard = fleet ? nearestRepairYard(st, fleet) : undefined;
    const guard = survivingGuardTarget(st, survivor);
    const hull = Math.max(0, Math.min(1, survivor.hull)) * 100;
    const count = Object.values(survivor.composition).reduce((n, value) => n + (value ?? 0), 0);
    const composition = Object.entries(survivor.composition).map(([kind, n]) => `${n} ${shipKindLabel(kind as ShipKind)}`).join(" · ");
    const iconKey = count === 1 ? survivorIcon(survivor.kind) : "fleet";
    const button = (action: string, text: string, reason = "", hint = "") =>
      `<span title="${escAttr(reason || hint)}"><button type="button" data-deck-act="strategic-survivor-${action}" data-report="${report.id}" data-fleet="${escAttr(survivor.fleet_id)}" title="${escAttr(reason || hint)}" ${reason ? "disabled" : ""}>${esc(text)}</button></span>`;
    const repairReason = block || (fleet?.damage != null && fleet.damage <= .0001 ? "Fleet's latest report shows full hull." : "")
      || (!yard ? "Repairs need a staffed Shipyard or Ordnance Foundry with Alloys and Machinery." : "")
      || (fleet?.docked === yard?.id ? "Already docked at the repair yard." : "");
    const guarding = !!guard && fleet?.guard_target === guard.id;
    const guardReason = block || (!fleet || !guardCapable(fleet) ? "Requires an Interceptor fleet." : "")
      || (!guard ? "The guarded fleet is no longer in your reports." : "") || (guarding ? "This guard order is already active." : "");
    const captain = survivor.captain;
    const gain = captain ? Math.max(0, captain.after.xp - captain.before.xp) : 0;
    const promoted = captain && captain.after.title !== captain.before.title;
    const officer = captain ? `<div class="deck-aftermath__officer">
      ${captainPortrait(captain.portrait, captain.after.portrait_age, captain.name, "deck-aftermath__portrait", true)}
      <div><b>${esc(captain.name)}</b><span>${promoted ? `Promoted · ${esc(captainTitle(captain.after.title))}` : esc(captainTitle(captain.after.title))}</span></div>
      <strong>+${fmt(gain)} XP</strong></div>` : "";
    return `<article class="deck-aftermath__fleet" data-key="survivor-${escAttr(survivor.fleet_id)}">
      <header>${icon(iconKey, "md")}<div><b>${esc(composition)}</b><small>Fleet ${esc(survivor.fleet_id)}${survivor.withdrew ? " · withdrew" : ""}${!fleet ? " · no current contact" : ""}</small></div><strong>${hull.toFixed(hull > 0 && hull < 1 ? 1 : 0)}% hull</strong></header>
      <div class="deck-meter deck-aftermath__hull" role="progressbar" aria-label="Hull remaining at battle end" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${hull.toFixed(1)}" title="HP-weighted hull of the surviving ships at battle end."><i style="width:${hull.toFixed(2)}%"></i></div>
      ${officer}
      <div class="deck-aftermath__actions">
        ${button("select", "Select", fleet ? "" : "Fleet is no longer in your current reports.")}
        ${button("repair", "Return for repairs", repairReason, yard ? `${yard.name} · ordinary delayed move order; repair supplies may change before arrival.` : "")}
        ${survivor.guard_target ? button("guard", guarding ? "Guarding" : "Continue guarding", guardReason, "Send a delayed guard order to this fleet.") : ""}
      </div>
      ${fleet && !yard && survivor.hull < .9999 ? `<small class="deck-muted">Repairs need a staffed, supplied Shipyard or Ordnance Foundry.</small>` : ""}
    </article>`;
  }

  private survivorAction(action: string, button: HTMLButtonElement): void {
    const report = this.ctx.state.battleReports.find((r) => r.id === Number(button.dataset.report));
    if (!report?.aftermath) return; // forged/stale clicks cannot read unarrived results
    const survivors = report.aftermath.survivors;
    if (action === "strategic-survivor-select-all") {
      this.hooks.selectFleets(survivors.filter((s) => currentSurvivor(this.ctx.state, s)).map((s) => s.fleet_id));
      return;
    }
    const survivor = survivors.find((s) => s.fleet_id === button.dataset.fleet);
    if (!survivor) return;
    const fleet = currentSurvivor(this.ctx.state, survivor);
    if (!fleet) return;
    if (action === "strategic-survivor-select") {
      this.hooks.selectFleets([fleet.id]);
      return;
    }
    if (this.survivorBlock(fleet)) return;
    if (action === "strategic-survivor-repair") {
      const yard = nearestRepairYard(this.ctx.state, fleet);
      if (!yard || fleet.docked === yard.id || (fleet.damage != null && fleet.damage <= .0001)) return;
      this.ctx.intent.beginPendingIntent({ verb: "move", shipId: fleet.id, dest: yard.pos, targetId: yard.id });
    } else if (action === "strategic-survivor-guard") {
      const target = survivingGuardTarget(this.ctx.state, survivor);
      if (!target || !guardCapable(fleet) || fleet.guard_target === target.id) return;
      this.ctx.intent.beginPendingIntent({ verb: "guard", shipId: fleet.id, targetId: target.id, dest: target.pos });
    } else return;
    // A preview is not a dispatch. Pending receipts, not this first click,
    // disable these controls; Cancel leaves the survivor's assignment alone.
  }

  private captureReportHtml(report: import("../../protocol").CaptureReportView): string {
    this.markViewed(report);
    const plunder = report.plunder.length ? report.plunder.map((slot) => `${fmt(slot.units)} ${label(slot.commodity)}`).join(" · ") : "Empty stockpile";
    return `<section class="deck-page deck-battle-route"><header class="deck-page__lead"><span>Capture · delayed report</span><h2>${report.captor ? "Captured" : "Lost"} ${esc(nearestSystemName(report.pos))}</h2><p>${report.captor ? "Your marines took the ground; the old owner keeps surviving fleets." : "Only the territory changed hands; surviving fleets remain in play."}</p></header><div class="deck-stat-grid"><dl class="deck-stat"><dt>System fell</dt><dd>${fmtDur(Math.max(0, liveSimTime() - report.at_time))} ago</dd></dl><dl class="deck-stat"><dt>Learned</dt><dd>${fmtDur(Math.max(0, liveSimTime() - report.learned_at))} ago</dd></dl><dl class="deck-stat is-stale"><dt>Capture report</dt><dd>${esc(informationDelay(Math.max(0, report.learned_at - report.at_time)))}</dd></dl></div><section class="deck-section"><header><div><h3>${report.captor ? "Plunder seized" : "Plunder lost"}</h3><p>The besieged stockpile changed hands; development tiers transfer at half strength.</p></div></header><div class="deck-alert"><b>${esc(plunder)}</b><span>One Troop Transport was consumed by the landing.</span></div></section><button type="button" data-deck-act="strategic-battle-dismiss" data-report-kind="capture" data-report="${report.id}">Dismiss map marker</button><p class="deck-muted">Dismissing removes only the map marker; this report remains in the Log.</p></section>`;
  }

  private markViewed(report: BattleReportView | CaptureReportView): void {
    const key = reportMarkKey(report);
    if (this.ctx.state.battleViewed.has(key)) return;
    this.ctx.state.battleViewed.add(key);
    saveBattleMarks(this.ctx.state);
  }

  private selectedOwnFleet(): GhostView | undefined {
    return this.ctx.state.selectedShipId ? this.ctx.state.ghosts.find((fleet) => fleet.id === this.ctx.state.selectedShipId && fleet.own) : undefined;
  }
}

function actionButton(action: string, text: string, operation: string, cls = ""): string {
  return `<button type="button" class="${cls}" data-deck-act="${action}" data-operation="${escAttr(operation)}">${esc(text)}</button>`;
}

function forceChip(kind: ShipKind, count: string): string {
  return `<span class="deck-force-chip">${icon(isPlayerFreighter(kind) || kind === "freighter" ? "convoy" : "fleet", "sm")}<span>${esc(shipKindLabel(kind))}</span><b>${esc(count)}</b></span>`;
}

function forceSide(side: string, chips: string): string {
  return `<div class="deck-force-side"><b>${esc(side)}</b><div>${chips || `<span class="deck-muted">No arrived contacts</span>`}</div></div>`;
}

function losses(rows: CompCount[]): string {
  return rows.length ? rows.map((row) => `${row.count} ${shipKindLabel(row.kind)}`).join(" · ") : "nothing";
}

function survivorIcon(kind: ShipKind): IconKey {
  if (kind === "freighter") return "authorityFreighter";
  if (kind === "survey_probe") return "scout";
  return kind;
}

function human(value: string): string {
  return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function esc(value: unknown): string {
  return String(value ?? "").replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]!);
}

function escAttr(value: unknown): string { return esc(value); }

function cssEscape(value: string): string {
  return typeof CSS !== "undefined" && CSS.escape ? CSS.escape(value) : value.replace(/[^a-zA-Z0-9_-]/g, "\\$&");
}

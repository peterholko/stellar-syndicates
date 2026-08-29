import { traitLine } from "../../core/derive/captains";
import { agoLabel, arrivalLocal, fmtDur } from "../../core/derive/format";
import { commandDelayTo, freshSurveyReports, locName, REPORT_RECENT_S, systemName } from "../../core/derive/geo";
import { nextDecisionLabel, siegeProgress } from "../../core/derive/orders";
import { nodeBonusDesc } from "../../core/derive/research";
import type { CoreEvent } from "../../core/events";
import { icon, label, type IconKey } from "../../icons";
import { countClassLabel, type TimelineEntry, type Vec2 } from "../../protocol";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

// The priority vocabulary is ported wholesale from the legacy decision inbox.
// It is presentation over the served View only: no item consults sim truth.
export const INBOX_W = {
  siege: 100, battle: 92, hostile: 85, captureLost: 82, blockade: 80,
  garrisonUnfed: 70, nodeUnfed: 68, enclave: 58, storageFull: 55, unfedHabitat: 50, idleStockpile: 48,
  brokenOrder: 46, surveyReport: 45, nodeAwakening: 44, dryRefinery: 42, nodeOpportunity: 41, myGarrisonUnfed: 40,
  surveyOpportunity: 36, emptyQueue: 34,
  captureWon: 28, battleReport: 26, noAutomation: 20,
};

const HOSTILE_CONCERN_MULT = 1.6;
const IDLE_UNITS = 30;
const MAX_HOSTILE_ITEMS = 4;

export type DeckInboxTone = "negative" | "warn" | "info" | "neutral";

export interface DeckInboxAction {
  label: string;
  icon?: IconKey;
  run(): void;
  deliveryPos?: Vec2;
  primary?: boolean;
  danger?: boolean;
}

export interface DeckInboxItem {
  key: string;
  weight: number;
  tone: DeckInboxTone;
  icon: IconKey;
  headline: string;
  stakes?: string;
  age?: number;
  confidence?: string;
  actions: DeckInboxAction[];
}

interface DeckLogHooks {
  go(route: DeckRoute): void;
  focusSystem(id: string): void;
  focusFleet(id: string): void;
}

interface ArrivedReport {
  key: string;
  at: number;
  tone: "good" | "bad" | "warn" | "info";
  text: string;
}

/** Full decision inbox + arrived command news. Stable decision keys make the
 * Log badge an unread-awareness signal instead of a permanently red item count.
 * Opening Log marks the currently served decisions read; a newly appearing key
 * lights it again. */
export class DeckLogRoutes {
  private readonly dismissedInbox = new Set<string>();
  private readonly readDecisionKeys = new Set<string>();
  private readonly arrivedReports: ArrivedReport[] = [];
  private currentInbox: DeckInboxItem[] = [];
  private signature = "";
  private unreadReports = 0;
  private nextReport = 1;

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: DeckLogHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "log") return false;
    this.currentInbox = this.computeInbox();
    this.markRead();
    const signature = sheetFingerprint([
      Math.floor(this.ctx.state.simTime), this.ctx.state.systems, this.ctx.state.ghosts,
      this.ctx.state.battles, this.ctx.state.battleReports, this.ctx.state.captureReports,
      this.ctx.state.standingOrders, this.ctx.state.timeline, this.arrivedReports,
      [...this.dismissedInbox].sort(),
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, this.logHtml());
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "log" || button.dataset.deckAct !== "inbox-action") return false;
    const item = this.currentInbox.find((entry) => entry.key === button.dataset.key);
    item?.actions[Number(button.dataset.actionIndex)]?.run();
    this.invalidate();
    return true;
  }

  onCore(events: CoreEvent[], route: DeckRoute | null): void {
    for (const event of events) {
      const report = this.reportFor(event);
      if (!report) continue;
      this.arrivedReports.unshift({ ...report, key: `local:${this.nextReport++}`, at: liveSimTime() });
      if (this.arrivedReports.length > 30) this.arrivedReports.length = 30;
      if (route?.name !== "log") this.unreadReports++;
    }
    if (route?.name === "log") this.markRead();
    this.invalidate();
  }

  /** The same full served inbox feeds Command's top-four digest. */
  items(): DeckInboxItem[] {
    this.currentInbox = this.computeInbox();
    return this.currentInbox;
  }

  runPrimary(key: string): void {
    const item = this.items().find((entry) => entry.key === key);
    const action = item?.actions.find((entry) => entry.primary) ?? item?.actions[0];
    action?.run();
    this.invalidate();
  }

  decisionKeys(): string[] {
    return this.computeInbox().map((entry) => entry.key);
  }

  badgeCount(route: DeckRoute | null): number {
    if (route?.name === "log") this.markRead();
    const unreadDecisions = this.decisionKeys().filter((key) => !this.readDecisionKeys.has(key)).length;
    return unreadDecisions + this.unreadReports;
  }

  invalidate(): void {
    this.signature = "";
  }

  private markRead(): void {
    for (const key of this.decisionKeys()) this.readDecisionKeys.add(key);
    this.unreadReports = 0;
  }

  // Deterministic, priority-sorted derivation from owner-gated served state.
  // This is the legacy computeInbox vocabulary with only its navigation hooks
  // adapted to the Deck router.
  private computeInbox(): DeckInboxItem[] {
    const state = this.ctx.state;
    const out: DeckInboxItem[] = [];
    if (state.playerId === null || !state.galaxy) return out;
    const galaxy = state.galaxy;
    const owned = state.systems.filter((system) => system.owner === state.playerId);
    const ownedIds = new Set(owned.map((system) => system.id));
    const active = state.standingOrders.filter((order) => order.status === "active");
    const now = liveSimTime();
    const push = (item: DeckInboxItem): void => {
      if (!this.dismissedInbox.has(item.key)) out.push(item);
    };
    const dismiss = (key: string): DeckInboxAction => ({
      label: "Dismiss",
      run: () => {
        this.dismissedInbox.add(key);
        this.invalidate();
      },
    });
    const focusSystem = (id: string): DeckInboxAction => ({ label: "Focus", primary: true, run: () => this.hooks.focusSystem(id) });
    const focusFleet = (id: string, labelText = "Inspect"): DeckInboxAction => ({ label: labelText, primary: true, run: () => this.hooks.focusFleet(id) });
    const openLogistics = (): void => this.hooks.go({ name: "logistics" });
    const sysPos = (id: string): Vec2 | null => galaxy.systems.find((system) => system.id === id)?.pos ?? null;

    for (const system of owned) {
      if (!system.blockade) continue;
      const siege = siegeProgress(system);
      if (siege) {
        const key = `siege:${system.id}`;
        push({ key, weight: INBOX_W.siege, tone: "negative", icon: "siege",
          headline: `${systemName(system.id)} — SIEGE in progress`,
          stakes: siege.ripe ? "CRITICAL — rival marines landing now TAKE it." : `Falls in ${fmtDur(siege.left)} unless you break the blockade or rebuild a Defense Platform.`,
          age: system.blockade.since, actions: [focusSystem(system.id), dismiss(key)] });
      } else {
        const key = `blockade:${system.id}`;
        push({ key, weight: INBOX_W.blockade, tone: "negative", icon: "blockade",
          headline: `${systemName(system.id)} — under BLOCKADE`,
          stakes: "Freighters held in & out; production idles. Break it with relief, or build a Defense Platform tier.",
          age: system.blockade.since, actions: [focusSystem(system.id), dismiss(key)] });
      }
    }

    for (const battle of state.battles) {
      if (!battle.own) continue;
      const key = `battle:${battle.id}`;
      const ownFleet = state.ghosts.find((fleet) => fleet.own && battle.participants.includes(fleet.id));
      const actions: DeckInboxAction[] = [{ label: "Open battle", primary: true, run: () => this.hooks.go({ name: "battle", params: { id: battle.id, label: "Ongoing battle" } }) }];
      if (ownFleet) actions.push({ label: "Withdraw", danger: true, deliveryPos: battle.pos, run: () => this.ctx.send({ type: "Withdraw", fleet_id: ownFleet.id }) });
      actions.push(dismiss(key));
      push({ key, weight: INBOX_W.battle, tone: "negative", icon: "battle",
        headline: `Your fleet is ENGAGED near ${locName(battle.pos)}`,
        stakes: "A battle is underway — reinforce, or Withdraw to break off (light-delayed).",
        age: battle.started_at, actions });
    }

    const threatRadius = galaxy.sensor_range * HOSTILE_CONCERN_MULT;
    const hostiles: DeckInboxItem[] = [];
    for (const fleet of state.ghosts) {
      if (fleet.own || fleet.ally || fleet.kind !== "raider") continue;
      let near: { id: string; distance: number; pos: Vec2 } | null = null;
      for (const system of owned) {
        const pos = sysPos(system.id);
        if (!pos) continue;
        const distance = Math.hypot(fleet.pos.x - pos.x, fleet.pos.y - pos.y);
        if (distance <= threatRadius && (!near || distance < near.distance)) near = { id: system.id, distance, pos };
      }
      if (!near) continue;
      const speed = Math.hypot(fleet.vel.x, fleet.vel.y);
      const closing = speed > 1 && fleet.vel.x * (near.pos.x - fleet.pos.x) + fleet.vel.y * (near.pos.y - fleet.pos.y) > 0;
      const size = fleet.composition ? `${fleet.composition.reduce((sum, stack) => sum + stack.count, 0)}-ship` : `~${countClassLabel(fleet.count_class)}`;
      const foe = fleet.pirate ? "PIRATE" : "Hostile";
      const key = `hostile:${fleet.id}:${near.id}`;
      hostiles.push({ key, weight: INBOX_W.hostile + (fleet.pirate ? 3 : 0), tone: "warn", icon: "warning",
        headline: `${foe} ${size} raider near ${systemName(near.id)}`,
        stakes: closing ? `Closing on ${systemName(near.id)} — ~${fmtDur(near.distance / speed)} out at its shown speed (a delayed sighting).` : `${Math.round(near.distance)} su out, holding — watch it (delayed sighting).`,
        age: fleet.age,
        confidence: fleet.composition ? undefined : "size estimate only — the contact is outside your sensor coverage",
        actions: [focusSystem(near.id), dismiss(key)] });
    }
    hostiles.sort((a, b) => (b.age ?? 0) - (a.age ?? 0)).slice(0, MAX_HOSTILE_ITEMS).forEach(push);

    for (const system of state.systems) {
      const tier = system.intel?.enclave_tier ?? 0;
      if (tier <= 0) continue;
      const key = `enclave:${system.id}`;
      push({ key, weight: INBOX_W.enclave, tone: "warn", icon: "raider",
        headline: `Pirate enclave at ${systemName(system.id)} — tier ${tier}`,
        stakes: "It raids careless trade nearby and grows if ignored. Station a raider fleet on it to destroy the base (yields its plunder).",
        age: system.intel?.observed_at, actions: [focusSystem(system.id), dismiss(key)] });
    }

    for (const report of state.captureReports) {
      if (now - report.learned_at > REPORT_RECENT_S) continue;
      const key = `capture:${report.id}`;
      push({ key, weight: report.captor ? INBOX_W.captureWon : INBOX_W.captureLost,
        tone: report.captor ? "info" : "negative", icon: report.captor ? "captured" : "lost",
        headline: report.captor ? `You CAPTURED ${locName(report.pos)}` : `You LOST ${locName(report.pos)}`,
        stakes: report.captor ? "Territory taken — plunder seized." : "Rival marines landed at full siege and took the system.",
        age: report.learned_at,
        actions: [{ label: "Open report", primary: true, run: () => this.hooks.go({ name: "log", params: { marker: String(report.id), label: "Capture report" } }) }, dismiss(key)] });
    }

    for (const system of owned) {
      if ((system.ally_garrison_ships ?? 0) > 0 && system.ally_garrison_fed === false) {
        const key = `garrison:${system.id}`;
        push({ key, weight: INBOX_W.garrisonUnfed, tone: "warn", icon: "garrison",
          headline: `Ally garrison at ${systemName(system.id)} is UNFED`,
          stakes: `${system.ally_garrison_ships} allied ship(s) here — their defense is SUSPENDED until you supply Provisions.`,
          actions: [{ label: "Auto-supply", icon: "doctrine", primary: true, run: openLogistics }, focusSystem(system.id), dismiss(key)] });
      }
    }
    for (const fleet of state.ghosts) {
      if (!fleet.own || !fleet.garrison_host || fleet.garrison_fed !== false) continue;
      const key = `mygarr:${fleet.id}`;
      push({ key, weight: INBOX_W.myGarrisonUnfed, tone: "warn", icon: "garrison",
        headline: `Your garrison at ${systemName(fleet.garrison_host)} is UNFED`,
        stakes: "The host is out of Provisions — this garrison isn't defending. Recall it, or wait for the host to resupply.",
        actions: [focusFleet(fleet.id), dismiss(key)] });
    }

    for (const system of owned) {
      if (system.storage_cap > 0 && system.storage_used >= system.storage_cap) {
        const key = `storage:${system.id}`;
        push({ key, weight: INBOX_W.storageFull, tone: "warn", icon: "storage",
          headline: `${systemName(system.id)} — storage FULL (${system.storage_used}/${system.storage_cap})`,
          stakes: "Production idles at the cap. Ship goods out, automate it, or build an Orbital Warehouse (nothing is lost).",
          actions: [
            { label: "Book pickup", icon: "cargo", run: () => this.ctx.send({ type: "ShipProduction", system_id: system.id }) },
            { label: "Auto-supply", icon: "doctrine", run: openLogistics }, focusSystem(system.id), dismiss(key),
          ] });
      }
      if (system.population > 0 && !system.habitat_fed) {
        const key = `habitat:${system.id}`;
        push({ key, weight: INBOX_W.unfedHabitat, tone: "warn", icon: "habitat",
          headline: `${systemName(system.id)} — food ${label(system.food_state ?? "rationing").toUpperCase()}`,
          stakes: "Workforce slowed, growth paused. Ship Provisions here or set a standing order (nothing is lost, nobody dies).",
          actions: [{ label: "Auto-supply", icon: "doctrine", primary: true, run: openLogistics }, focusSystem(system.id), dismiss(key)] });
      }
      if (system.node?.awakened && !system.node.fed) {
        const key = `node:${system.id}`;
        push({ key, weight: INBOX_W.nodeUnfed, tone: "warn", icon: "unfed",
          headline: `${systemName(system.id)} — ${system.node.title} node UNFED`,
          stakes: `Its bonus is SUSPENDED. ${nodeBonusDesc(system.node.bonus)} Ship its upkeep here or automate it (nothing is lost).`,
          actions: [{ label: "Auto-supply", icon: "doctrine", primary: true, run: openLogistics }, focusSystem(system.id), dismiss(key)] });
      }
      const volatiles = (system.stockpile ?? []).find((stack) => stack.commodity === "volatiles")?.units ?? 0;
      if (system.refinery_tier >= 1 && volatiles === 0) {
        const key = `refinery:${system.id}`;
        push({ key, weight: INBOX_W.dryRefinery, tone: "info", icon: "refinery",
          headline: `${systemName(system.id)} — Refinery idle`, stakes: "No Volatiles — Fuel production stopped. Haul some in or automate it.",
          actions: [{ label: "Auto-supply", icon: "doctrine", primary: true, run: openLogistics }, focusSystem(system.id), dismiss(key)] });
      }
      const total = (system.stockpile ?? []).reduce((sum, stack) => sum + stack.units, 0);
      const covered = active.some((order) => order.source.kind === "system" && order.source.id === system.id);
      if (total >= IDLE_UNITS && !covered && !(system.storage_cap > 0 && system.storage_used >= system.storage_cap)) {
        const key = `idle:${system.id}`;
        push({ key, weight: INBOX_W.idleStockpile, tone: "info", icon: "market",
          headline: `${systemName(system.id)} — ${total} units idle`,
          stakes: "No standing order ships from here — automate it so it works while you're away.",
          actions: [
            { label: "Auto-supply", icon: "doctrine", primary: true, run: openLogistics },
            { label: "Book pickup", icon: "cargo", run: () => this.ctx.send({ type: "ShipProduction", system_id: system.id }) }, dismiss(key),
          ] });
      }
      if (system.slots_total > 0 && system.slots_used === 0 && system.builds.length === 0) {
        const key = `queue:${system.id}`;
        push({ key, weight: INBOX_W.emptyQueue, tone: "info", icon: "build",
          headline: `${systemName(system.id)} — nothing built yet`,
          stakes: `${system.slots_total} development slot(s) free and idle — develop it (Mining Complex, Orbital Warehouse, Sensor…).`,
          actions: [focusSystem(system.id), dismiss(key)] });
      }
    }

    const nodeSystems = state.systems.filter((system) => system.node);
    const awakenAt = galaxy.node_awakening_time ?? 0;
    const secondsLeft = awakenAt - now;
    if (nodeSystems.length && nodeSystems.some((system) => !system.node!.awakened) && secondsLeft > 0) {
      const key = "nodes:awakening";
      push({ key, weight: INBOX_W.nodeAwakening, tone: "info", icon: "intel",
        headline: `Exotic nodes awaken in ${fmtDur(secondsLeft)}`,
        stakes: `${nodeSystems.length} exotic system(s) become capturable tactical prizes. Stage colony ships + fleets now — first arrival claims an unowned node.`,
        actions: [dismiss(key)] });
    }
    for (const system of nodeSystems) {
      if (!system.node!.awakened || system.owner) continue;
      const key = `nodeopen:${system.id}`;
      push({ key, weight: INBOX_W.nodeOpportunity, tone: "info", icon: "claim",
        headline: `${systemName(system.id)} — ${system.node!.title} node UNCLAIMED`,
        stakes: `A capturable tactical prize. ${nodeBonusDesc(system.node!.bonus)} Send a colony ship — first arrival claims it.`,
        actions: [focusSystem(system.id), dismiss(key)] });
    }

    for (const [systemId, at] of freshSurveyReports) {
      const key = `surveyrep:${systemId}`;
      const dynamic = state.systems.find((system) => system.id === systemId);
      const info = galaxy.systems.find((system) => system.id === systemId);
      if (!dynamic?.deposits || !info) continue;
      const summary = dynamic.deposits.map((deposit) => `${label(deposit.resource)} ~${deposit.richness.toFixed(1)}/s`).join(" · ");
      const roles = (dynamic.opportunities ?? []).slice(0, 3).map((opportunity) => `${opportunity.tier === "jackpot" ? "JACKPOT " : ""}${opportunity.title}${opportunity.body_name ? ` — ${opportunity.body_name}` : ""} ×${opportunity.score.toFixed(2)}`);
      const garden = [...dynamic.bodies].sort((a, b) => b.habitat_capacity_mult - a.habitat_capacity_mult)[0];
      const minerals = [...dynamic.bodies].filter((body) => body.geology !== null).sort((a, b) => (b.mineral_extraction_mult ?? 1) - (a.mineral_extraction_mult ?? 1))[0];
      const discovered = roles.length ? roles.join(" · ") : [
        garden ? `Best settlement: ${garden.name} (${label(garden.size)} ${label(garden.environment)})` : "",
        minerals ? `Best minerals: ${minerals.name} (${label(minerals.geology!)})` : "",
        dynamic.trait ? traitLine(dynamic.trait).title : "",
      ].filter(Boolean).join(" · ");
      push({ key, weight: INBOX_W.surveyReport, tone: "info", icon: "intel",
        headline: `Survey report: ${systemName(systemId)} (${info.band.toUpperCase()} band)`, age: at,
        stakes: `${summary || "barren"}.${discovered ? ` ${discovered}.` : ""}${dynamic.owner === null ? " Unclaimed: send a colony ship if it's worth holding." : ""}`,
        actions: [focusSystem(systemId), dismiss(key)] });
    }

    const nearSu = 3000;
    const ownedPositions = owned.map((system) => sysPos(system.id)).filter((pos): pos is Vec2 => pos !== null);
    const richUnsurveyed = state.systems.filter((system) => {
      if (system.deposits !== null) return false;
      const info = galaxy.systems.find((candidate) => candidate.id === system.id);
      return !!info && info.band === "rich" && ownedPositions.some((pos) => Math.hypot(pos.x - info.pos.x, pos.y - info.pos.y) <= nearSu);
    });
    if (richUnsurveyed.length) {
      const key = "surveyops";
      push({ key, weight: INBOX_W.surveyOpportunity, tone: "info", icon: "sensor",
        headline: `${richUnsurveyed.length} RICH-band system(s) unsurveyed within ${nearSu} su`,
        stakes: "The spectral read says rich, but its deposits, mineral grades and rare features are unknown. Survey before committing a colony ship — or claim blind.",
        actions: [{ label: "Focus nearest", primary: true, run: () => this.hooks.focusSystem(richUnsurveyed[0].id) }, dismiss(key)] });
    }

    const allyIds = new Set(state.systems.filter((system) => system.ally).map((system) => system.id));
    for (const order of active) {
      const refs: string[] = [];
      if (order.source.kind === "system" && !ownedIds.has(order.source.id)) refs.push(systemName(order.source.id));
      if (order.dest.kind === "system" && !ownedIds.has(order.dest.id) && !allyIds.has(order.dest.id)) refs.push(systemName(order.dest.id));
      if (!refs.length) continue;
      const key = `order:${order.id}`;
      push({ key, weight: INBOX_W.brokenOrder, tone: "warn", icon: "doctrine",
        headline: `Standing order #${order.id} targets a system you don't hold`, stakes: `Points at ${refs.join(" & ")} — update or clear it.`,
        actions: [{ label: "Open logistics", primary: true, run: openLogistics }, dismiss(key)] });
    }

    for (const report of state.battleReports) {
      if (state.battleViewed.has(report.id) || now - report.learned_at > REPORT_RECENT_S) continue;
      const key = `report:${report.id}`;
      push({ key, weight: INBOX_W.battleReport, tone: "info", icon: "aftermath",
        headline: `A battle you were in concluded near ${locName(report.pos)}`,
        stakes: "Open the report for losses and the outcome.", age: report.learned_at,
        actions: [{ label: "Open results", primary: true, run: () => this.hooks.go({ name: "battle", params: { id: String(report.id), label: "Battle report" } }) }, dismiss(key)] });
    }

    if (owned.length > 0 && active.length === 0 && out.length === 0) {
      push({ key: "noauto", weight: INBOX_W.noAutomation, tone: "info", icon: "doctrine",
        headline: "No standing orders running",
        stakes: `You hold ${owned.length} system${owned.length > 1 ? "s" : ""} — automate supply so it works while you're away.`,
        actions: [{ label: "Open logistics", primary: true, run: openLogistics }] });
    }

    out.sort((a, b) => b.weight - a.weight || a.key.localeCompare(b.key));
    return out;
  }

  private logHtml(): string {
    const inbox = this.currentInbox.length
      ? this.currentInbox.map((item) => this.inboxCardHtml(item)).join("")
      : `<div class="deck-log-clear">${icon("success", "sm")} <span>${esc(nextDecisionLabel())}</span></div>`;
    const reports = this.arrivedReports.length
      ? `<section class="deck-section"><header><div><h3>Arrived reports</h3><p>The transient toast lane mirrored here for recall.</p></div><b>${this.arrivedReports.length}</b></header><div class="deck-timeline">${this.arrivedReports.map((report) => `<div class="deck-timeline__row is-${report.tone}">${statusIcon(report.tone)}<span>${esc(report.text)}</span><time>${agoLabel(report.at)}</time></div>`).join("")}</div></section>`
      : "";
    const away = this.ctx.state.timeline.filter((entry) => entry.at_time > this.ctx.state.awaySince);
    const earlier = this.ctx.state.timeline.filter((entry) => entry.at_time <= this.ctx.state.awaySince);
    return `<section class="deck-page deck-log"><header class="deck-page__lead"><span>Served awareness</span><h2>Decision inbox</h2><p>Priorities and reports derived only from light that has reached command.</p></header>` +
      `<section class="deck-section"><header><div><h3>Needs a decision</h3><p>Threats first, then constrained capacity and new information.</p></div><b>${this.currentInbox.length}</b></header><div class="deck-inbox">${inbox}</div></section>` +
      reports +
      `<section class="deck-section"><header><div><h3>Light-delayed log</h3><p>What became observable while you were away, newest first.</p></div><b>${away.length} new</b></header>${this.timelineGroup("Since last command", away)}${this.timelineGroup("Earlier", earlier)}</section></section>`;
  }

  private inboxCardHtml(item: DeckInboxItem): string {
    const age = item.age !== undefined ? `<span class="deck-inbox__age">Information age · ${esc(agoLabel(item.age))}</span>` : "";
    const confidence = item.confidence ? `<div class="deck-inbox__confidence">${icon("uncertainty", "sm")} ${esc(item.confidence)}</div>` : "";
    const actions = item.actions.map((action, index) => {
      const delay = action.deliveryPos ? commandDelayTo(action.deliveryPos) : null;
      const eta = delay !== null ? `<small>Order signal arrives ~${esc(arrivalLocal(delay))}</small>` : "";
      return `<button type="button" class="${action.primary ? "is-primary" : ""}${action.danger ? " is-danger" : ""}" data-deck-act="inbox-action" data-key="${esc(item.key)}" data-action-index="${index}">${action.icon ? icon(action.icon, "sm") : ""}<span>${esc(action.label)}${eta}</span></button>`;
    }).join("");
    return `<article class="deck-inbox-card is-${item.tone}"><header>${icon(item.icon, "sm")}<div><b>${esc(item.headline)}</b>${age}</div></header>${item.stakes ? `<p>${esc(item.stakes)}</p>` : ""}${confidence}<div class="deck-inbox__actions">${actions}</div></article>`;
  }

  private timelineGroup(title: string, entries: TimelineEntry[]): string {
    if (!entries.length) return title === "Since last command" ? `<div class="deck-log-clear"><span>Nothing new since you were last here.</span></div>` : "";
    return `<h4 class="deck-timeline__heading">${title}</h4><div class="deck-timeline">${[...entries].reverse().map((entry) => `<div class="deck-timeline__row is-${entry.severity}">${statusIcon(entry.severity)}<span>${esc(entry.text)}</span><time>${agoLabel(entry.at_time)}</time></div>`).join("")}</div>`;
  }

  private reportFor(event: CoreEvent): Omit<ArrivedReport, "key" | "at"> | null {
    if (event.kind === "OrderConfirmed") return { tone: "good", text: `Order confirmed — ${human(event.orderKind)} response light arrived` };
    if (event.kind === "ReportArrived") return { tone: event.report.outcome === "target_destroyed" && event.report.you === "attacker" ? "good" : "warn", text: `Combat report — ${human(event.report.outcome)} · delayed ${Math.round(event.report.age)}s` };
    if (event.kind === "BattleConcluded") return { tone: event.outcome === "target_destroyed" ? "good" : "warn", text: `Battle concluded — ${human(event.outcome)}` };
    if (event.kind === "EstimateReady") return { tone: "info", text: event.estimate.win_pct == null ? "Engagement projection ready" : `Engagement projection — ${Math.round(event.estimate.win_pct)}% win chance` };
    if (event.kind === "TradeSettled") return { tone: event.trade.event === "Rejected" ? "bad" : "info", text: `${human(event.trade.event)} — ${"units" in event.trade ? event.trade.units : ""} ${"commodity" in event.trade ? label(event.trade.commodity) : ""}`.trim() };
    if (event.kind === "ServerError") return { tone: "bad", text: `Command refused — ${event.message}` };
    return null;
  }
}

function statusIcon(tone: ArrivedReport["tone"] | TimelineEntry["severity"]): string {
  return icon(tone === "good" ? "success" : tone === "info" ? "intel" : "warning", "sm");
}

function human(value: string): string {
  return value.replace(/_/g, " ").replace(/([a-z])([A-Z])/g, "$1 $2").replace(/^./, (letter) => letter.toUpperCase());
}

function esc(value: string): string {
  return value.replace(/[&<>\"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!);
}

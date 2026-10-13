import { traitLine } from "../../core/derive/captains";
import { reportMarkKey } from "../../battlehistory";
import { agoLabel, arrivalLocal, fmtDur, informationDelay, rejectText } from "../../core/derive/format";
import { commandDelayTo, foundingHomeSystemId, freshSurveyReports, locName, REPORT_RECENT_S, systemName } from "../../core/derive/geo";
import { foundingPlan } from "../../core/derive/founding";
import { nextDecisionLabel } from "../../core/derive/orders";
import { blockadeBrief, blockadeStage, siegeClock } from "../../core/derive/blockade";
import { nodeBonusDesc } from "../../core/derive/research";
import type { CoreEvent } from "../../core/events";
import { icon, label, type IconKey } from "../../icons";
import { countClassLabel, fleetCargoUnits, type FoundingStage, type TimelineEntry, type Vec2, type GhostView } from "../../protocol";
import { regimeStore, shipKindLabel } from "../../core/derive/fleet";
import { pirateSite } from "../../core/derive/pirates";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

// The priority vocabulary is ported wholesale from the legacy decision inbox.
// It is presentation over the served View only: no item consults sim truth.
export const INBOX_W = {
  founding: 120,
  siege: 100, siegeRipe: 96, battle: 92, hostile: 85, captureLost: 82, blockade: 80, contested: 76, contesting: 60,
  garrisonUnfed: 70, nodeUnfed: 68, enclave: 58, brownout: 57, storageFull: 55, unfedHabitat: 50,
  strandedFleet: 78, unsuppliedFleet: 72, lowFuel: 66, overdueResponse: 64, loadedFleet: 52,
  brokenOrder: 46, surveyReport: 45, nodeAwakening: 44, dryRefinery: 42, nodeOpportunity: 41, myGarrisonUnfed: 40,
  operationOffer: 39, syndicateInvite: 38, idleFleet: 32,
  surveyOpportunity: 36, emptyQueue: 34,
  idleStockpile: 30, captureWon: 28, battleReport: 26, noAutomation: 20,
};

const HOSTILE_CONCERN_MULT = 1.6;
/** §founding R8: until the first guarded export leaves, these decisions fold
 * into one "Also happening" line so the guide and real threats stay on top. */
const FOCUS_FOLD = ["nodeopen:", "nodes:", "operations:", "surveyops", "surveyrep:", "enclave:", "idle:", "queue:",
  "invites:", "idlefleet:", "loaded:", "storage:", "refinery:", "noauto"];
const FOCUS_KEY = "focus:also";
const IDLE_UNITS = 30;
const MAX_HOSTILE_ITEMS = 4;
const FOUNDING_DIRECTIVE: Record<FoundingStage, string> = {
  build_mine: "Build and staff the Mining Complex",
  local_recovery: "Send your first shipment",
  prepare_export: "Send your first shipment",
  export_production: "Send your first shipment",
  build_convoy: "Replace your Freighter, then ship your ore",
  defeat_privateer: "Protect the shipment from the Rogue Privateer",
  build_shipyard: "Build and staff a Shipyard",
  complete_export: "Build and staff a Shipyard",
  grow_business: "Build and staff a Shipyard",
  build_second_freighter: "Build and staff a Shipyard",
  build_academy: "Start research at an Academy",
  first_research: "Start research at an Academy",
  build_scout: "Chart a nearby system with a Survey Probe",
  survey_candidates: "Chart a nearby system with a Survey Probe",
  build_barge: "Build a Mining Barge",
  mine_belt: "Send the Mining Barge to the charted belt",
  haul_ore: "Bring the barge's ore home",
  build_colony: "Founding complete",
  establish_colony: "Founding complete",
  complete: "Founding complete",
};

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
  runFounding(): void;
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
  private focusOpen = false;
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
    const openLogistics = (source?: string, destination?: string, commodity?: string): void => this.hooks.go({
      name: "logistics",
      query: { ...(source ? { source } : {}), ...(destination ? { destination } : {}), ...(commodity ? { commodity } : {}) },
    });
    const openProduction = (id: string): void => {
      const system = galaxy.systems.find((entry) => entry.id === id);
      this.hooks.go({ name: "system", params: { id, systemLabel: system?.name ?? "System" }, query: { tab: "production" } });
    };
    const openBuild = (id: string): void => {
      const system = galaxy.systems.find((entry) => entry.id === id);
      this.hooks.go({ name: "build", params: { systemId: id, systemLabel: system?.name ?? "System" } });
    };
    const sysPos = (id: string): Vec2 | null => galaxy.systems.find((system) => system.id === id)?.pos ?? null;

    // §founding R4: the directive is the guide's own next step: same title,
    // same reason, and the same action as the floating card's button.
    const homeId = foundingHomeSystemId();
    const plan = state.founding && state.founding.stage !== "complete"
      ? foundingPlan(state, state.systems.find((system) => system.id === homeId)) : null;
    const focus = plan?.chapter.name === "Landfall" && !state.founding?.export_dispatched && !this.focusOpen;
    if (state.founding && state.founding.stage !== "complete") {
      const step = plan?.primary;
      const chapter = plan && plan.chapter.name !== "Founding" ? `${plan.chapter.name} ${plan.chapter.done}/${plan.chapter.total}` : "Founding directive";
      push({
        key: `founding:${state.founding.stage}:${step?.action ?? ""}`,
        weight: INBOX_W.founding,
        tone: plan?.blocked ? "warn" : "info",
        icon: "build",
        headline: `${chapter} · ${step?.title ?? FOUNDING_DIRECTIVE[state.founding.stage]}`,
        stakes: step ? [step.reason, step.copy].filter(Boolean).join(" ") : "This is the corporation's current guided objective. The same action appears in the Founding guide.",
        actions: [{ label: step?.alternatives?.length ? "Choose in the guide" : step?.label ?? "Continue", primary: true, run: () => this.hooks.runFounding() }],
      });
    }

    // Blockades, by stage (core/derive/blockade.ts): the owner hears every
    // stage with what to do; the besieger hears the fight and the ripe siege.
    for (const system of state.systems) {
      const blockade = system.blockade;
      if (!blockade) continue;
      const stage = blockadeStage(blockade);
      const brief = blockadeBrief(blockade, now, galaxy.siege_secs, system.ground?.marines_needed);
      const name = systemName(system.id);
      if (system.owner === state.playerId) {
        const key = `blockade-${stage}:${system.id}`;
        push({ key, weight: stage === "siege" ? INBOX_W.siege : stage === "held" ? INBOX_W.blockade : INBOX_W.contested,
          tone: stage === "contested" ? "warn" : "negative", icon: stage === "siege" ? "siege" : "blockade",
          headline: `${name} — ${brief.title}`, stakes: brief.detail, age: blockade.since,
          actions: [{ label: "Send defenders", primary: true, run: () => this.hooks.focusSystem(system.id) }, dismiss(key)] });
      } else if (blockade.by_me) {
        const ripe = siegeClock(blockade, now, galaxy.siege_secs)?.ripe ?? false;
        if (stage !== "contested" && !ripe) continue;
        const key = ripe ? `siege-ripe:${system.id}` : `blockade-contesting:${system.id}`;
        push({ key, weight: ripe ? INBOX_W.siegeRipe : INBOX_W.contesting, tone: ripe ? "info" : "warn",
          icon: ripe ? "siege" : "blockade", headline: `${name} — ${brief.title}`, stakes: brief.detail,
          age: blockade.since, actions: [focusSystem(system.id), dismiss(key)] });
      }
    }

    for (const battle of state.battles) {
      if (!battle.own) continue;
      const key = `battle:${battle.id}`;
      const ownFleet = state.ghosts.find((fleet) => fleet.own && battle.participants.includes(fleet.id));
      const actions: DeckInboxAction[] = [{ label: "Open battle", primary: true, run: () => this.hooks.go({ name: "battle", params: { id: battle.id, label: "Ongoing battle" } }) }];
      if (ownFleet) actions.push({ label: "Withdraw", danger: true, deliveryPos: battle.pos, run: () => this.ctx.intent.beginFleetCommand({ type: "Withdraw", fleet_id: ownFleet.id }) });
      actions.push(dismiss(key));
      push({ key, weight: INBOX_W.battle, tone: "negative", icon: "battle",
        headline: `Your fleet is engaged near ${locName(battle.pos)}`,
        stakes: "A battle is underway — reinforce, or Withdraw to break off (light-delayed).",
        age: battle.started_at, actions });
    }

    const threatRadius = galaxy.sensor_range * HOSTILE_CONCERN_MULT;
    const hostiles: DeckInboxItem[] = [];
    const holding = new Set<string>();
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
      const foe = fleet.pirate ? "Pirate" : "Hostile";
      const key = `hostile:${fleet.id}:${near.id}`;
      if (!closing) holding.add(key);
      hostiles.push({ key, weight: INBOX_W.hostile + (fleet.pirate ? 3 : 0), tone: "warn", icon: "warning",
        headline: `${foe} ${size} Interceptor near ${systemName(near.id)}`,
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
        headline: `${pirateSite(tier)!.title} · ${systemName(system.id)}`,
        stakes: pirateSite(tier)!.goal,
        age: system.intel?.observed_at, actions: [focusSystem(system.id), dismiss(key)] });
    }

    for (const report of state.captureReports) {
      if (now - report.learned_at > REPORT_RECENT_S) continue;
      const key = `capture:${report.id}`;
      push({ key, weight: report.captor ? INBOX_W.captureWon : INBOX_W.captureLost,
        tone: report.captor ? "info" : "negative", icon: report.captor ? "captured" : "lost",
        headline: report.captor ? `You captured ${locName(report.pos)}` : `You lost ${locName(report.pos)}`,
        stakes: report.captor ? "Territory taken — plunder seized." : "Rival marines landed at full siege and took the system.",
        age: report.learned_at,
        actions: [{ label: "Open report", primary: true, run: () => this.hooks.go({ name: "battle", params: { id: String(report.id), report: "capture", label: "Capture report" } }) }, dismiss(key)] });
    }

    for (const system of owned) {
      if ((system.ally_garrison_ships ?? 0) > 0 && system.ally_garrison_fed === false) {
        const key = `garrison:${system.id}`;
        push({ key, weight: INBOX_W.garrisonUnfed, tone: "warn", icon: "garrison",
          headline: `Ally garrison at ${systemName(system.id)} is unfed`,
          stakes: `${system.ally_garrison_ships} allied ship(s) here — their defense is SUSPENDED until you supply Provisions.`,
          actions: [{ label: "Set supply route", icon: "doctrine", primary: true, run: () => openLogistics(undefined, system.id, "provisions") }, focusSystem(system.id), dismiss(key)] });
      }
    }
    for (const fleet of state.ghosts) {
      if (!fleet.own || !fleet.garrison_host || fleet.garrison_fed !== false) continue;
      const key = `mygarr:${fleet.id}`;
      push({ key, weight: INBOX_W.myGarrisonUnfed, tone: "warn", icon: "garrison",
        headline: `Your garrison at ${systemName(fleet.garrison_host)} is unfed`,
        stakes: "The host is out of Provisions — this garrison isn't defending. Recall it, or wait for the host to resupply.",
        actions: [focusFleet(fleet.id), dismiss(key)] });
    }

    const engagedFleetIds = new Set(state.battles.flatMap((battle) => battle.participants));
    for (const fleet of state.ghosts) {
      if (!fleet.own) continue;
      const name = `${shipKindLabel(fleet.kind)} fleet`;
      const fleetAction = focusFleet(fleet.id, "Open fleet");
      if (fleet.stalled) {
        const key = `stranded:${fleet.id}`;
        push({ key, weight: INBOX_W.strandedFleet, tone: "negative", icon: "fuel",
          headline: `${name} is out of fuel`,
          stakes: "The fleet is holding. Refuel it at a berth or request Authority Astral Assistance from its fleet panel.",
          age: fleet.age, actions: [fleetAction, dismiss(key)] });
      } else if (fleet.supplied === false) {
        const key = `unsupplied:${fleet.id}`;
        push({ key, weight: INBOX_W.unsuppliedFleet, tone: "warn", icon: "provisions",
          headline: `${name} is out of Provisions`,
          stakes: "It keeps its current course and weapons, but cannot accept a new movement or offensive order until supplied.",
          age: fleet.age, actions: [fleetAction, dismiss(key)] });
      } else if (lowStore(fleet)) {
        // §fuel-overhaul: the store the fleet's throttle burns (Uranium on Slow).
        const store = regimeStore(fleet), have = (store.fuel === "uranium" ? fleet.uranium : fleet.fuel) ?? 0;
        const key = `fuel:${fleet.id}`;
        push({ key, weight: INBOX_W.lowFuel, tone: "warn", icon: store.fuel === "uranium" ? "uranium" : "deuterium",
          headline: `${name} has low ${label(store.fuel)}`,
          stakes: `${Math.round(have)} of ${Math.round(store.capacity)} ${label(store.fuel)} remains in the served report.`,
          age: fleet.age, actions: [fleetAction, dismiss(key)] });
      }

      const overdue = (state.pendingOrders.get(fleet.id) ?? []).filter((order) => !order.lost && now > order.response_at);
      if (overdue.length) {
        const key = `overdue:${fleet.id}:${overdue[0].id}`;
        push({ key, weight: INBOX_W.overdueResponse, tone: "warn", icon: "echo",
          headline: `${name} response is overdue`,
          stakes: `${overdue.length} order response${overdue.length === 1 ? " is" : "s are"} still unconfirmed. The estimate expired; only arrived fleet light can confirm delivery.`,
          actions: [fleetAction, dismiss(key)] });
      }

      const cargo = fleetCargoUnits(fleet);
      if (fleet.docked && cargo > 0) {
        const key = `loaded:${fleet.id}:${fleet.docked}`;
        push({ key, weight: INBOX_W.loadedFleet, tone: "info", icon: "cargo",
          headline: `${name} is docked with ${cargo} cargo`,
          stakes: fleet.docked === "hub" ? "Unload or sell its manifest at the Market Hub." : "Unload it, send it to market, or assign another haul from the fleet panel.",
          age: fleet.age, actions: [fleetAction, dismiss(key)] });
      }

      const parked = !fleet.docked
        && !fleet.guard_target
        && !engagedFleetIds.has(fleet.id)
        && !state.orders[fleet.id]
        && !(fleet.path?.length)
        && !(state.pendingOrders.get(fleet.id)?.length)
        && Math.hypot(fleet.vel.x, fleet.vel.y) < 0.5;
      if (parked) {
        const key = `idlefleet:${fleet.id}`;
        push({ key, weight: INBOX_W.idleFleet, tone: "neutral", icon: "fleet",
          headline: `${name} is holding without an assignment`,
          stakes: "It is undocked and has no served course, guard target, or command in flight.",
          age: fleet.age, actions: [fleetAction, dismiss(key)] });
      }
    }

    const offeredOperations = state.operations.filter((operation) => operation.state === "offered" || (operation.state === "active" && !operation.joined));
    if (offeredOperations.length) {
      const key = `operations:${offeredOperations.map((operation) => operation.id).join(",")}`;
      push({ key, weight: INBOX_W.operationOffer, tone: "info", icon: "manifest",
        headline: `${offeredOperations.length} operation${offeredOperations.length === 1 ? "" : "s"} available`,
        stakes: "Review arrived contracts and strategic opportunities before they expire.",
        actions: [{ label: "Open Operations", primary: true, run: () => this.hooks.go({ name: "operations" }) }, dismiss(key)] });
    }
    if (state.syndicateInvites.length) {
      const key = `invites:${state.syndicateInvites.map((invite) => invite.id).join(",")}`;
      push({ key, weight: INBOX_W.syndicateInvite, tone: "info", icon: "syndicate",
        headline: `${state.syndicateInvites.length} syndicate invitation${state.syndicateInvites.length === 1 ? "" : "s"}`,
        stakes: "An invitation is waiting for a decision.",
        actions: [{ label: "Review invitation", primary: true, run: () => this.hooks.go({ name: "syndicate" }) }, dismiss(key)] });
    }

    for (const system of owned) {
      if (system.storage_cap > 0 && system.storage_used >= system.storage_cap * 0.85) {
        const key = `storage:${system.id}`;
        const full = system.storage_used >= system.storage_cap;
        push({ key, weight: INBOX_W.storageFull, tone: "warn", icon: "storage",
          headline: `${systemName(system.id)} — storage ${full ? "full" : "nearly full"} (${system.storage_used}/${system.storage_cap})`,
          stakes: full ? "Production idles at the cap. Ship goods out, automate it, or build an Orbital Warehouse (nothing is lost)." : "Capacity is running low. Plan a shipment or expand storage before production stops.",
          actions: [
            { label: "Open production", icon: "cargo", primary: true, run: () => openProduction(system.id) },
            { label: "Automate export", icon: "doctrine", run: () => openLogistics(system.id, "hub") }, focusSystem(system.id), dismiss(key),
          ] });
      }
      // §power: each browning-out planet, with its fix.
      for (const body of system.bodies ?? []) {
        const grid = body.power;
        if (!grid || grid.power >= 0.999) continue;
        const key = `brownout:${system.id}:${body.id}`;
        const fuel = grid.fuel ? label(grid.fuel) : "fuel";
        const plant = (body.structures.power_plant ?? 0) > 0;
        const world = (): void => {
          const info = galaxy.systems.find((entry) => entry.id === system.id);
          this.hooks.go({ name: "world", params: { systemId: system.id, systemLabel: info?.name ?? "System", bodyId: String(body.id), worldLabel: body.name } });
        };
        push({ key, weight: INBOX_W.brownout, tone: "warn", icon: "refinery",
          headline: `${body.name} — Power ${Math.round(grid.power * 100)}%`,
          stakes: grid.no_fuel ? `Plant out of ${fuel}. Its mines, factories, yards and Academy slow down.`
            : `${plant ? "Raise its Power Plant" : "Build a Power Plant"}. Its mines, factories, yards and Academy slow down.`,
          actions: [grid.no_fuel
            ? { label: "Set supply route", icon: "doctrine", primary: true, run: () => openLogistics(undefined, system.id, grid.fuel ?? undefined) }
            : { label: plant ? "Open world" : "Build plant", icon: "build", primary: true, run: world },
          focusSystem(system.id), dismiss(key)] });
      }
      if (system.population > 0 && !system.habitat_fed) {
        const key = `habitat:${system.id}`;
        push({ key, weight: INBOX_W.unfedHabitat, tone: "warn", icon: "habitat",
          headline: `${systemName(system.id)} — food ${label(system.food_state ?? "rationing")}`,
          stakes: "Workforce slowed, growth paused. Ship Provisions here or set a standing order (nothing is lost, nobody dies).",
          actions: [{ label: "Set supply route", icon: "doctrine", primary: true, run: () => openLogistics(undefined, system.id, "provisions") }, focusSystem(system.id), dismiss(key)] });
      }
      if (system.node?.awakened && !system.node.fed) {
        const key = `node:${system.id}`;
        push({ key, weight: INBOX_W.nodeUnfed, tone: "warn", icon: "unfed",
          headline: `${systemName(system.id)} — ${system.node.title} node unfed`,
          stakes: `Its bonus is SUSPENDED. ${nodeBonusDesc(system.node.bonus)} Ship its upkeep here or automate it (nothing is lost).`,
          actions: [{ label: "Set supply route", icon: "doctrine", primary: true, run: () => openLogistics(undefined, system.id, "provisions") }, focusSystem(system.id), dismiss(key)] });
      }
      const volatiles = (system.stockpile ?? []).find((stack) => stack.commodity === "volatiles")?.units ?? 0;
      if (system.refinery_tier >= 1 && volatiles === 0) {
        const key = `refinery:${system.id}`;
        push({ key, weight: INBOX_W.dryRefinery, tone: "info", icon: "refinery",
          headline: `${systemName(system.id)} — Refinery idle`, stakes: "No Volatiles — Deuterium production stopped. Haul some in or automate it.",
          actions: [{ label: "Set supply route", icon: "doctrine", primary: true, run: () => openLogistics(undefined, system.id, "volatiles") }, focusSystem(system.id), dismiss(key)] });
      }
      const total = (system.stockpile ?? []).reduce((sum, stack) => sum + stack.units, 0);
      const covered = active.some((order) => order.source.kind === "system" && order.source.id === system.id);
      if (total >= IDLE_UNITS && !covered && !(system.storage_cap > 0 && system.storage_used >= system.storage_cap)) {
        const key = `idle:${system.id}`;
        push({ key, weight: INBOX_W.idleStockpile, tone: "info", icon: "market",
          headline: `${systemName(system.id)} — ${total} stored units without a route`,
          stakes: "No standing order ships from here — automate it so it works while you're away.",
          actions: [
            { label: "Automate export", icon: "doctrine", primary: true, run: () => openLogistics(system.id, "hub") },
            { label: "Open production", icon: "cargo", run: () => openProduction(system.id) }, dismiss(key),
          ] });
      }
      if (system.slots_total > 0 && system.slots_used === 0 && system.builds.length === 0) {
        const key = `queue:${system.id}`;
        push({ key, weight: INBOX_W.emptyQueue, tone: "info", icon: "build",
          headline: `${systemName(system.id)} — nothing built yet`,
          stakes: `${system.slots_total} development slot(s) free and idle — develop it (Mining Complex, Orbital Warehouse, Sensor…).`,
          actions: [{ label: "Open Build", primary: true, run: () => openBuild(system.id) }, dismiss(key)] });
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
        headline: `${systemName(system.id)} — ${system.node!.title} node unclaimed`,
        stakes: `A capturable tactical prize. ${nodeBonusDesc(system.node!.bonus)} Send a colony ship — first arrival claims it.`,
        actions: [focusSystem(system.id), dismiss(key)] });
    }

    for (const [systemId, at] of freshSurveyReports) {
      const key = `surveyrep:${systemId}`;
      const dynamic = state.systems.find((system) => system.id === systemId);
      const info = galaxy.systems.find((system) => system.id === systemId);
      if (!dynamic?.deposits || !info) continue;
      const summary = dynamic.deposits.map((deposit) => `${label(deposit.resource)} ×${deposit.richness.toFixed(2)}`).join(" · ");
      const roles = (dynamic.opportunities ?? []).slice(0, 3).map((opportunity) => `${opportunity.tier === "jackpot" ? "Jackpot · " : ""}${opportunity.title}${opportunity.body_name ? ` — ${opportunity.body_name}` : ""} ×${opportunity.score.toFixed(2)}`);
      const garden = [...dynamic.bodies].sort((a, b) => b.habitat_capacity_mult - a.habitat_capacity_mult)[0];
      const minerals = [...dynamic.bodies].filter((body) => body.geology !== null).sort((a, b) => (b.mineral_extraction_mult ?? 1) - (a.mineral_extraction_mult ?? 1))[0];
      const discovered = roles.length ? roles.join(" · ") : [
        garden ? `Best settlement: ${garden.name} (${label(garden.size)} ${label(garden.environment)})` : "",
        minerals ? `Best minerals: ${minerals.name} (${label(minerals.geology!)})` : "",
        dynamic.trait ? traitLine(dynamic.trait).title : "",
      ].filter(Boolean).join(" · ");
      push({ key, weight: INBOX_W.surveyReport, tone: "info", icon: "intel",
        headline: `Survey report: ${systemName(systemId)} (${label(info.band)} band)`, age: at,
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
        headline: `Logistics rule ${order.id} targets a system you don't hold`, stakes: `Points at ${refs.join(" & ")} — update or clear it.`,
        actions: [{ label: "Open logistics", primary: true, run: openLogistics }, dismiss(key)] });
    }

    for (const report of state.battleReports) {
      if (state.battleViewed.has(reportMarkKey(report)) || now - report.learned_at > REPORT_RECENT_S) continue;
      const key = `report:${report.id}`;
      push({ key, weight: INBOX_W.battleReport, tone: "info", icon: "aftermath",
        headline: `A battle you were in concluded near ${locName(report.pos)}`,
        stakes: "Open the report for losses and the outcome.", age: report.learned_at,
        actions: [{ label: "Open results", primary: true, run: () => this.hooks.go({ name: "battle", params: { id: String(report.id), report: "battle", label: "Battle report" } }) }, dismiss(key)] });
    }

    if (owned.length > 0 && active.length === 0 && out.length === 0) {
      push({ key: "noauto", weight: INBOX_W.noAutomation, tone: "info", icon: "doctrine",
        headline: "No standing orders running",
        stakes: `You hold ${owned.length} system${owned.length > 1 ? "s" : ""} — automate supply so it works while you're away.`,
        actions: [{ label: "Open logistics", primary: true, run: openLogistics }] });
    }

    if (focus) {
      const folded = out.filter((item) => holding.has(item.key) || FOCUS_FOLD.some((prefix) => item.key.startsWith(prefix)));
      if (folded.length) {
        const kept = out.filter((item) => !folded.includes(item));
        out.length = 0;
        out.push(...kept);
        const shown = folded.slice(0, 3).map((item) => item.headline).join(" · ");
        push({ key: FOCUS_KEY, weight: 10, tone: "neutral", icon: "intel",
          headline: `Also happening · ${folded.length} update${folded.length === 1 ? "" : "s"}`,
          stakes: `Held back until your first guarded export leaves, so the guide stays on top: ${shown}${folded.length > 3 ? ` · and ${folded.length - 3} more` : ""}.`,
          actions: [{ label: "Show all", primary: true, run: () => { this.focusOpen = true; this.invalidate(); } }, dismiss(FOCUS_KEY)] });
      }
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
    if (event.kind === "OrderConfirmed") return { tone: "good", text: `Order received — ${human(event.orderKind)} compliance light arrived` };
    if (event.kind === "FleetDocked") return { tone: "good", text: `Fleet docked at ${event.berth === "hub" ? "the Market Hub" : systemName(event.berth)}` };
    if (event.kind === "FleetArrived") return { tone: "good", text: "Fleet arrived at its commanded destination" };
    if (event.kind === "BuildCompleted") return { tone: "good", text: `${human(event.buildKey)} completed at ${systemName(event.systemId)}` };
    if (event.kind === "StructureStaffed") return { tone: "good", text: `${event.title} staffed at ${systemName(event.systemId)}` };
    if (event.kind === "ResearchCompleted") return { tone: "good", text: `Research complete — ${event.programmeName}` };
    if (event.kind === "CommandRejected") return { tone: "bad", text: event.message };
    if (event.kind === "ReportArrived") return { tone: event.report.outcome === "target_destroyed" && event.report.you === "attacker" ? "good" : "warn", text: `Combat report — ${human(event.report.outcome)} · ${informationDelay(event.report.age)}` };
    if (event.kind === "BattleConcluded") return { tone: event.outcome === "target_destroyed" ? "good" : "warn", text: `Battle concluded — ${human(event.outcome)}` };
    if (event.kind === "EstimateReady") return { tone: "info", text: event.estimate.win_pct == null ? "Engagement projection ready" : `Engagement projection — ${Math.round(event.estimate.win_pct)}% win chance` };
    if (event.kind === "TradeSettled") return { tone: event.trade.event === "Rejected" ? "bad" : "info", text: event.trade.event === "Rejected" ? rejectText(event.trade) : `${human(event.trade.event)} — ${"units" in event.trade ? event.trade.units : ""} ${"commodity" in event.trade ? label(event.trade.commodity) : ""}`.trim() };
    if (event.kind === "ServerError") return { tone: "bad", text: `Command refused — ${event.message}` };
    return null;
  }
}

/** Below a fifth of the store the fleet's throttle burns. */
function lowStore(fleet: GhostView): boolean {
  const store = regimeStore(fleet);
  const have = store.fuel === "uranium" ? fleet.uranium : fleet.fuel;
  return have != null && store.capacity > 0 && have / store.capacity < 0.2;
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

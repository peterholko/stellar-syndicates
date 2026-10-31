import { captainTitle, captainXpFloor, fleetCommandLoad, officerFleetName } from "../../core/derive/captains";
import { ago, INVITES_MAX, inviteLink, markChannelSeen, presenceText } from "../../core/derive/social";
import { blockadeBrief } from "../../core/derive/blockade";
import { constructionStock, dockedAtSystem, guardCapable, shipKindLabel, systemFleetsAt, surveyCapable } from "../../core/derive/fleet";
import { colonyPurpose } from "../../core/derive/colony";
import { anchorStatus, beltActions, pocketLeft, prospectLine } from "../../core/derive/belts";
import { COURSE, freeSpecialists, isStructureJob, jobTag, jobTitle, SENDING, slotLine, SPECIALIST_KINDS, specialistAffine, structureQueue, WORKFORCE_STRUCTURES, yardStatus, type StructureJob } from "../../core/derive/worldbuild";
import { fleetReadiness } from "../../core/derive/readiness";
import { postVictoryHandoff } from "../../core/derive/handoff";
import { campOpen } from "../../core/derive/founding";
import { explorationHtml, explorationFocused, handleExplorationAction } from "../exploration";
import { focusIndustryFleet, industryHtml, industryServedSignature, industryStandingVisible, industryUiSignature, handleIndustryAction, handleIndustryInput } from "../industry";
import { refiningComparison } from "../refining";
import { allySystems, commandDelayTo, foundingHomeSystemId, ownedSystems, systemName } from "../../core/derive/geo";
import {
  bodyPoolUsage,
  buildOption,
  COMMODITIES,
  conversionSummary,
  recipeOutputs,
  fitLegal,
  FITTING_POINTS,
  hullResearched,
  MODULE_SLOTS,
  moduleLedgerAt,
  moduleBuildReason,
  moduleResearched,
  POOL_LABEL,
  POOL_OF,
  poolUsage,
  SHIP_YARD,
  YARD_TITLE,
  shipOption,
  structOption,
  structureUnlocked,
  structureLevel,
  structureBuildSeconds,
  openingBuild,
  type BuildOpt,
  type Pool,
} from "../../core/derive/market";
import { fmtBuildDur, fmtEta, operationCopy, operationReward, operationTitle } from "../../core/derive/format";
import { commodityIcon, label, structureImage } from "../../icons";
import { bonusLine, currentRung, givingWorlds, roman } from "../../core/derive/projects";
import { nodeBonusDesc, nodeLabel, researched, researchSuppliesPerHour, supplyAmount } from "../../core/derive/research";
import { hullFigures } from "../../core/derive/orders";
import { gridLine } from "../../core/derive/worldbuild";
import type {
  CharterView,
  BodyView,
  CaptainAttribute,
  CaptainRosterView,
  Commodity,
  FleetDoctrine,
  GhostView,
  MigrationPolicy,
  ModuleKind,
  OperationView,
  ProgrammeView,
  RankingRow,
  ShipKind,
  StandingEndpoint,
  StandingOrder,
  StandingTrigger,
  SyndicateBonus,
  SyndicateProjectsView,
  SyndicateRole,
  SystemStateView,
} from "../../protocol";
import { liveSimTime } from "../../state";
import { crackdownTitle, DECREE_TEXT, demandText, gripNumeral, gripTitle, keystoneFeed, RESOLVE_BAND, resolveBand } from "../../core/derive/season";
import "../../ship-construction-view";
import { liveProgressAttr } from "../../core/derive/construction";
import { pickYardJob, yardFallback, yardJobs, type YardJob } from "../../core/derive/shipyard";
import { HULL_YARD } from "../../shipyard-scene";
import type { SystemBodyDetail } from "../../systemview";
import { captainPortrait } from "../art";
import { corpCard, fmtDays } from "../../core/derive/medals";
import { boardLine, boardSummary, serviceReward } from "../../core/derive/board";
import { signalsSummary } from "../../core/derive/signals";
import { corpCardHtml, medalChipsHtml, medalCountsHtml } from "../corpcard";
import type { CoreContext } from "../types";
import type { SheetEntry, SheetView } from "./sheets";
import { SheetStack } from "./sheets";
import { sheetFingerprint } from "../signature";

interface ParityHooks {
  openSheet(entry: SheetEntry): void;
  focusFleet(id: string): void;
  deselectFleets(): void;
  focusSystem(id: string): void;
  enterSystem(id: string): void;
  exitSemantic(): void;
  notice(html: string): void;
}

type SystemTab = "overview" | "worlds" | "production" | "construction";
type PlanetTab = "economy" | "population" | "infrastructure";
type OperationTab = "active" | "available" | "history";

const FIELD_ORDER = ["propulsion", "materials", "computation", "weapons", "hulls", "life"] as const;
const FIELD_TITLE: Record<string, string> = {
  propulsion: "Propulsion", materials: "Materials", computation: "Computation",
  weapons: "Weapons", hulls: "Hulls", life: "Life",
};
const SHIP_ORDER: ShipKind[] = ["scout", "survey_probe", "corvette", "raider", "combat_freighter", "tiny_freighter", "small_freighter", "convoy", "mining_barge", "large_freighter", "heavy_freighter", "bulk_freighter", "colony", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
const SHIP_KEYS = new Set<string>(SHIP_ORDER);
import { MODULES, moduleFitsHull } from "../../core/derive/equipment";
const PRODUCER_STRUCTURES = new Set([
  "mining_complex", "volatile_harvester", "bioharvester", "smelter", "electronics_fabricator",
  "chemical_works", "fuel_refinery", "machine_works", "armaments_complex", "agroplex", "living_terraces", "academy",
  "composite_works", "hull_fabricator", "precision_works", "drive_works", "uranium_mill", "antimatter_plant",
  "shipyard", "naval_drydock", "capital_slipway", "ordnance_foundry",
]);
const DOCTRINE_FIELDS: { key: keyof FleetDoctrine; label: string; options: [string, string][] }[] = [
  { key: "engagement", label: "Engagement", options: [["avoid", "Avoid"], ["defensive_only", "Defensive only"], ["engage_weaker", "Engage weaker"], ["engage_any", "Engage any"]] },
  { key: "retreat", label: "Retreat", options: [["quarter", "At ~3:1 against"], ["half", "When outnumbered"], ["three_quarter", "Without a clear edge"], ["never", "Never"]] },
  { key: "escort", label: "Escort", options: [["guard_nearest", "Guard nearest"], ["guard_richest", "Guard richest"], ["hold_station", "Hold station"]] },
  { key: "destination_invalid", label: "Lost destination", options: [["drop", "Drop cargo"], ["return_home", "Return home"], ["sell_at_hub", "Sell at hub"]] },
];

type RankCat = { slug: string; label: string; value: (row: RankingRow) => number; text: (row: RankingRow) => string };
const RANK_CATS: RankCat[] = [
  { slug: "valuation", label: "Valuation", value: (r) => r.valuation, text: (r) => `${fmt(r.valuation)} Cr` },
  { slug: "trade", label: "Trade", value: (r) => r.trade_throughput, text: (r) => fmt(r.trade_throughput) },
  { slug: "profit", label: "Profit", value: (r) => r.market_profit, text: (r) => `${fmt(r.market_profit)} Cr` },
  { slug: "captured", label: "Captured", value: (r) => r.cargo_captured, text: (r) => fmt(r.cargo_captured) },
  { slug: "protected", label: "Protected", value: (r) => r.cargo_protected, text: (r) => fmt(r.cargo_protected) },
  { slug: "battle", label: "Kill/Loss", value: (r) => r.battle_ranked ? r.battle_efficiency : -Infinity, text: (r) => r.battle_ranked ? `×${r.battle_efficiency.toFixed(2)}` : "provisional" },
  { slug: "built", label: "Developed", value: (r) => r.systems_developed, text: (r) => fmt(r.systems_developed) },
  { slug: "intel", label: "Intel", value: (r) => r.intel_gathered, text: (r) => fmt(r.intel_gathered) },
  { slug: "recovery", label: "Recovery", value: (r) => r.recovery, text: (r) => `${fmt(r.recovery)} Cr` },
];

/** §hull stats (Peter, Oct 27 2026): the hull's figures under its 3D model, in
 * the yard and the ship builder. */
function hullStatsHtml(kind: string): string {
  const figures = hullFigures(kind);
  return figures.length ? `<div class="m-stat-grid m-hull-stats">${figures.map(([name, value]) =>
    `<span><small>${esc(name)}</small><b>${esc(value)}</b></span>`).join("")}</div>` : "";
}

const esc = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);
const human = (value: string): string => (value === "metallic_ore" ? "Ferrite Ore" : value === "electronics" ? "Semiconductors" : value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()));
const fmt = (value: number, digits = 0): string => Number.isFinite(value)
  ? value.toLocaleString(undefined, { maximumFractionDigits: digits, minimumFractionDigits: digits })
  : "—";
const fmtPopulation = (millions: number): string => {
  const people = millions * 1_000_000;
  return people >= 1_000_000 ? `${(people / 1_000_000).toFixed(2)}m` : people >= 1_000 ? `${(people / 1_000).toFixed(1)}k` : fmt(people);
};
const propsOf = <T extends object>(entry: SheetEntry): Partial<T> => (entry.props && typeof entry.props === "object" ? entry.props : {}) as Partial<T>;
const element = <T extends HTMLElement>(id: string): T | null => document.getElementById(id) as T | null;
const option = (value: string, label: string, selected = false): string => `<option value="${esc(value)}"${selected ? " selected" : ""}>${esc(label)}</option>`;

export class MobileParitySurfaces {
  private refiningDraft: { system: string; body: number; ore: Commodity } | null = null;
  /** §fuel-overhaul: a Drive Works product being chosen (Warp or Fusion Core). */
  private coreDraft: { system: string; body: number; product: Commodity } | null = null;
  private readonly renderSignatures = new Map<SheetEntry["id"], string>();
  private researchField = "propulsion";
  private operationTab: OperationTab = "active";
  private handoffContract = "";
  private systemTab: SystemTab = "worlds";
  private planetTab: PlanetTab = "economy";
  private rankingCategory = "valuation";
  // §social (R5): the Comms sheet's tab, open mail and compose draft.
  private commsTab: "channel" | "inbox" | "sent" = "channel";
  private commsMail: number | null = null;
  private commsCompose: { to: string; subject: string } | null = null;
  /** The pushed Comms entry whose `tab` prop was applied (once per push). */
  private commsEntry: SheetEntry | null = null;
  private selectedBuild = "";
  /** §construction: the structure order whose Cancel awaits a Confirm. */
  private cancelArmed: number | null = null;
  private readonly cancelSent = new Set<number>();
  private selectedHull: ShipKind | "" = "";
  /** §shipyard-view: the hull picked in the yard sheet. */
  private yardPicked: string | null = null;
  /** Hulls the yard sheet opened on that a report has shown building. */
  private readonly yardSeen = new Set<string>();
  private pendingFit: ModuleKind[] = [];

  constructor(
    private readonly ctx: CoreContext,
    private readonly sheets: SheetStack,
    private readonly hooks: ParityHooks,
  ) {}

  render(entry: SheetEntry): SheetView | null {
    let view: SheetView | null;
    switch (entry.id) {
      case "research": view = this.renderResearch(); break;
      case "officers": view = this.renderOfficers(); break;
      case "operations": view = this.renderOperations(); break;
      case "syndicate": view = this.renderSyndicate(); break;
      case "comms": {
        const tab = (entry.props as { tab?: string } | undefined)?.tab;
        if (entry !== this.commsEntry) {
          this.commsEntry = entry;
          if (tab === "inbox") { this.commsTab = "inbox"; this.commsMail = null; this.commsCompose = null; }
        }
        view = this.renderComms();
        break;
      }
      case "faction": view = this.renderFaction(); break;
      case "rankings": view = this.renderRankings(entry); break;
      case "logistics": view = this.renderLogistics(); break;
      case "doctrine": view = this.renderDoctrine(); break;
      case "system": view = this.renderSystem(entry); break;
      case "planet": view = this.renderPlanet(entry); break;
      case "build": view = this.renderBuild(entry); break;
      case "shipyard": view = this.renderShipyard(entry); break;
      case "yard": view = this.renderYard(entry); break;
      case "hub": view = this.renderHub(); break;
      default: return null;
    }
    this.rememberSignature(entry);
    return view;
  }

  refreshNeeded(entry: SheetEntry): boolean | null {
    const signature = this.signature(entry);
    return signature === null ? null : this.renderSignatures.get(entry.id) !== signature;
  }

  handleInput(target: HTMLInputElement | HTMLSelectElement): void {
    if (handleIndustryInput(target, this.ctx.state)) this.sheets.refresh();
  }

  handleClick(event: Event): boolean {
    const button = (event.target as Element).closest<HTMLElement>("[data-mobile-act]");
    if (!button) return false;
    if (handleIndustryAction(button, this.ctx)) { this.sheets.refresh(); return true; }
    if (handleExplorationAction(button, this.ctx)) { this.sheets.refresh(); return true; }
    const action = button.dataset.mobileAct;
    if (!action || !this.ownsAction(action)) return false;
    switch (action) {
      case "next-objectives":
        this.hooks.openSheet({ id: "operations" });
        break;
      case "open-signals":
        this.hooks.openSheet({ id: "signals" });
        break;
      case "planet-prospect":
        if (button.dataset.fleet && button.dataset.system && button.dataset.body) {
          this.ctx.intent.beginFleetCommand({ type: "ProspectBelt", fleet_id: button.dataset.fleet,
            system_id: button.dataset.system, body_id: Number(button.dataset.body) });
        }
        break;
      case "planet-anchor":
        if (button.dataset.fleet && button.dataset.system && button.dataset.body) {
          this.ctx.intent.beginFleetCommand({ type: "AnchorStation", fleet_id: button.dataset.fleet,
            system_id: button.dataset.system, body_id: Number(button.dataset.body) });
        }
        break;
      case "planet-mine":
        if (button.dataset.fleet && button.dataset.system && button.dataset.body && button.dataset.ore) {
          this.ctx.intent.beginFleetCommand({ type: "MineBelt", fleet_id: button.dataset.fleet, system_id: button.dataset.system,
            body_id: Number(button.dataset.body), ore: button.dataset.ore as Commodity, ...(button.dataset.pocket ? { pocket: true } : {}) });
        }
        break;
      case "operation-ledger": {
        const sheet = button.dataset.sheet;
        if (sheet === "syndicate" || sheet === "market" || sheet === "research") this.hooks.openSheet({ id: sheet });
        break;
      }
      case "next-goal": case "next-funding": case "next-prospect":
        this.openHandoffGoal(action, button.dataset.goal);
        break;
      case "research-field":
        if (button.dataset.field) this.researchField = button.dataset.field;
        this.sheets.refresh();
        break;
      case "research-add":
        if (button.dataset.id) this.changeResearchQueue("add", button.dataset.id);
        break;
      case "research-grant":
        this.ctx.send({ type: "FundResearch" });
        this.hooks.notice("<b>Research grant sent</b> · paid at command.");
        break;
      case "research-up": case "research-down": case "research-remove":
        this.changeResearchQueue(action.slice(9) as "up" | "down" | "remove", Number(button.dataset.index));
        break;
      case "officer-recruit": {
        const home = foundingHomeSystemId();
        if (home) this.ctx.send({ type: "RecruitCaptain", system_id: home });
        break;
      }
      case "officer-assign": {
        const captain = Number(button.dataset.captain);
        const fleet = element<HTMLSelectElement>(`m-officer-fleet-${captain}`)?.value;
        if (Number.isFinite(captain) && fleet) this.ctx.intent.beginFleetCommand({ type: "AssignCaptain", captain_id: captain, fleet_id: fleet });
        break;
      }
      case "officer-reserve": {
        const captain = Number(button.dataset.captain);
        if (Number.isFinite(captain)) this.ctx.intent.beginFleetCommand({ type: "ReserveCaptain", captain_id: captain });
        break;
      }
      case "officer-train": {
        const captain = Number(button.dataset.captain);
        const attribute = button.dataset.attribute as CaptainAttribute | undefined;
        if (Number.isFinite(captain) && isCaptainAttribute(attribute)) this.ctx.intent.beginFleetCommand({ type: "TrainCaptain", captain_id: captain, attribute });
        break;
      }
      case "officer-fleet":
        if (button.dataset.id) this.hooks.focusFleet(button.dataset.id);
        break;
      case "operation-tab":
        if (isOperationTab(button.dataset.tab)) this.operationTab = button.dataset.tab;
        this.sheets.refresh();
        break;
      case "operation-accept": case "operation-abandon":
        if (button.dataset.id) this.ctx.send(action === "operation-accept"
          ? { type: "AcceptOperation", operation_id: button.dataset.id }
          : { type: "AbandonOperation", operation_id: button.dataset.id });
        break;
      case "operation-assign": case "operation-recover": {
        const id = button.dataset.id;
        const fleet = id ? element<HTMLSelectElement>(`m-op-fleet-${safeId(id)}`)?.value : "";
        const operation = this.ctx.state.operations.find(o => o.id === id);
        if (id && fleet && operation?.kind.kind === "freight_escort") {
          const protected_fleet = element<HTMLSelectElement>(`m-op-charge-${safeId(id)}`)?.value;
          const guard = this.ctx.state.ghosts.find(g => g.own && g.id === fleet);
          if (protected_fleet && guard && guardCapable(guard)) {
            this.ctx.intent.beginFleetCommand([
              { type: "AssignOperationFleet", operation_id: id, fleet_id: fleet, protected_fleet },
              { type: "GuardFleet", interceptor_id: fleet, target_id: protected_fleet },
            ]);
          }
        } else if (id && fleet) {
          const order = action === "operation-assign"
            ? { type: "AssignOperationFleet" as const, operation_id: id, fleet_id: fleet }
            : { type: "RecoverOperation" as const, operation_id: id, fleet_id: fleet };
          this.ctx.intent.beginFleetCommand(action === "operation-assign" && operation?.kind.kind === "survey_expedition"
            ? [order, { type: "SurveySystem", fleet_id: fleet, system_id: operation.kind.system }]
            : action === "operation-assign" && operation?.briefing
            && ["rescue_salvage", "prize_recovery", "privateer_patrol", "pirate_bounty", "combat_objective"].includes(operation.kind.kind)
            ? [order, { type: "MoveShip", ship_id: fleet, dest: operation.target_pos }] : order);
        }
        break;
      }
      case "operation-contribute": {
        const id = button.dataset.id;
        const operation = this.ctx.state.operations.find((candidate) => candidate.id === id);
        if (!id || operation?.kind.kind !== "syndicate_megaproject") break;
        const commodity: Commodity = operation.kind.stage === 0 ? "alloys" : operation.kind.stage === 1 ? "electronics" : "machinery";
        const units = Math.max(1, Math.floor(Number(element<HTMLInputElement>(`m-op-units-${safeId(id)}`)?.value) || 0));
        this.ctx.send({ type: "ContributeOperationCargo", operation_id: id, commodity, units });
        break;
      }
      case "comms-tab":
        this.commsTab = button.dataset.tab === "inbox" || button.dataset.tab === "sent" ? button.dataset.tab : "channel";
        this.commsMail = null;
        this.commsCompose = null;
        break;
      case "comms-say": {
        const input = document.getElementById("m-chat-input") as HTMLInputElement | null;
        const text = input?.value.trim();
        if (text) this.ctx.send({ type: "SyndicateSay", text });
        if (input) input.value = "";
        break;
      }
      case "comms-open": {
        const id = Number(button.dataset.id);
        if (button.dataset.unread) this.ctx.send({ type: "MarkMailRead", id });
        this.commsMail = id;
        break;
      }
      case "comms-back": this.commsMail = null; this.commsCompose = null; break;
      case "comms-compose": this.commsCompose = { to: button.dataset.to ?? "", subject: button.dataset.subject ?? "" }; this.commsMail = null; break;
      case "comms-send": {
        const value = (id: string) => (document.getElementById(id) as HTMLInputElement | HTMLTextAreaElement | null)?.value.trim() ?? "";
        const to = value("m-mail-to"), body = value("m-mail-body");
        if (!to || !body) break;
        this.ctx.send({ type: "SendMail", to, subject: value("m-mail-subject"), body });
        this.commsCompose = null;
        this.commsTab = "sent";
        break;
      }
      case "comms-delete": this.ctx.send({ type: "DeleteMail", id: Number(button.dataset.id) }); this.commsMail = null; break;
      case "comms-mute": if (button.dataset.player) this.ctx.send({ type: "MuteCorp", player: button.dataset.player, mute: button.dataset.mute === "1" }); break;
      case "comms-report":
        if (button.dataset.kind === "chat" || button.dataset.kind === "mail") {
          this.ctx.send({ type: "ReportMessage", kind: button.dataset.kind, id: Number(button.dataset.id), reason: "reported in Comms" });
          if (button instanceof HTMLButtonElement) button.disabled = true;
          button.textContent = "Reported";
        }
        break;
      case "invite-new":
        this.ctx.send({ type: "CreateInvite" });
        break;
      case "invite-revoke":
        if (button.dataset.code) this.ctx.send({ type: "RevokeInvite", code: button.dataset.code });
        break;
      case "invite-copy":
        if (button.dataset.code) {
          void navigator.clipboard?.writeText(inviteLink(button.dataset.code)).then(() => { button.textContent = "Copied"; }, () => {
            button.closest(".m-invite-row")?.querySelector<HTMLInputElement>("input")?.select();
          });
        }
        break;
      case "syndicate-create": {
        const name = element<HTMLInputElement>("m-syndicate-name")?.value.trim();
        if (name) this.ctx.send({ type: "CreateSyndicate", name });
        break;
      }
      case "syndicate-invite": {
        const name = element<HTMLInputElement>("m-syndicate-invite")?.value.trim();
        if (name) this.ctx.send({ type: "InviteToSyndicate", name });
        break;
      }
      case "syndicate-accept":
        if (button.dataset.id) this.ctx.send({ type: "AcceptSyndicateInvite", syndicate_id: button.dataset.id });
        break;
      case "syndicate-leave": this.ctx.send({ type: "LeaveSyndicate" }); break;
      case "syndicate-dissolve": this.ctx.send({ type: "DissolveSyndicate" }); break;
      case "syndicate-role": {
        const member = button.dataset.member;
        const role = button.dataset.role as SyndicateRole | undefined;
        if (member && isSyndicateRole(role)) this.ctx.send({ type: "SetSyndicateRole", member, role });
        break;
      }
      case "project-give": {
        const rung = this.ctx.state.syndicate?.projects && currentRung(this.ctx.state.syndicate.projects);
        const system_id = element<HTMLSelectElement>("m-project-system")?.value;
        const units = Math.floor(Number(element<HTMLInputElement>("m-project-units")?.value));
        if (rung && system_id && units > 0) this.ctx.send({ type: "ContributeSyndicateProject", system_id, commodity: rung.goods, units });
        break;
      }
      case "bonus-raise":
        if (button.dataset.bonus) this.ctx.send({ type: "RaiseSyndicateBonus", bonus: button.dataset.bonus as SyndicateBonus });
        break;
      case "diplomacy-propose": case "diplomacy-ceasefire": case "diplomacy-war": {
        const name = element<HTMLInputElement>("m-diplomacy-name")?.value.trim();
        if (!name) break;
        if (action === "diplomacy-war") this.ctx.send({ type: "DeclareWar", target_name: name });
        else this.ctx.send({ type: "ProposeTreaty", target_name: name, treaty: action === "diplomacy-propose" ? "non_aggression" : "ceasefire" });
        break;
      }
      case "diplomacy-response": {
        const proposal = Number(button.dataset.id);
        if (Number.isFinite(proposal)) this.ctx.send({ type: "RespondTreaty", proposal_id: proposal, accept: button.dataset.answer === "accept" });
        break;
      }
      case "diplomacy-cancel":
        if (button.dataset.target) this.ctx.send({ type: "CancelTreaty", target: button.dataset.target });
        break;
      case "flagship-name": {
        const name = element<HTMLInputElement>("m-flagship-name")?.value.trim() ?? "";
        this.ctx.send({ type: "NameFlagship", name });
        break;
      }
      case "faction-pay": {
        const points = Math.max(1, Math.floor(Number(element<HTMLInputElement>("m-faction-points")?.value) || 0));
        this.ctx.send({ type: "PayReinstatement", points });
        break;
      }
      // §season: the Authority's demands.
      case "faction-levy":
        this.ctx.send({ type: "AnswerLevy", accept: button.dataset.accept === "1" });
        this.hooks.notice(`Levy ${button.dataset.accept === "1" ? "accepted" : "refused"} · answer on its way to the Hub.`);
        break;
      case "faction-arrears":
        if (button.dataset.commodity) this.ctx.send({ type: "PayArrears", commodity: button.dataset.commodity as Commodity });
        this.hooks.notice("Arrears paid · on their way to the Hub.");
        break;
      case "faction-feed":
        if (button.dataset.system && button.dataset.commodity) this.ctx.send({ type: "FeedKeystone", system_id: button.dataset.system,
          commodity: button.dataset.commodity as Commodity, units: Number(button.dataset.units ?? 0) });
        this.hooks.notice("Keystone fed · on its way.");
        break;
      case "faction-requisition":
        if (button.dataset.id) this.ctx.send({ type: "AnswerRequisition", id: Number(button.dataset.id), pay: button.dataset.pay === "1" });
        this.hooks.notice(`Requisition ${button.dataset.pay === "1" ? "paid" : "refused"} · answer on its way to the Hub.`);
        break;
      case "open-rankings": this.hooks.openSheet({ id: "rankings" }); break;
      // §medals: a corporation's card, from the published table.
      case "corp-card": if (button.dataset.id) this.hooks.openSheet({ id: "rankings", props: { corp: button.dataset.id } }); break;
      case "ranking-category":
        if (button.dataset.category) this.rankingCategory = button.dataset.category;
        this.sheets.refresh();
        break;
      case "system-tab":
        if (isSystemTab(button.dataset.tab)) this.systemTab = button.dataset.tab;
        this.sheets.refresh();
        break;
      case "open-ground":
        if (button.dataset.id) this.hooks.openSheet({ id: "ground", props: { id: button.dataset.id } });
        break;
      case "open-planet": {
        const systemId = button.dataset.system;
        const bodyId = Number(button.dataset.body);
        if (systemId && Number.isFinite(bodyId)) this.openPlanet(systemId, bodyId);
        break;
      }
      case "open-build": case "open-shipyard": {
        const systemId = button.dataset.system;
        const bodyId = Number(button.dataset.body);
        if (!systemId || !Number.isFinite(bodyId)) break;
        this.selectedBuild = "";
        this.selectedHull = "";
        this.pendingFit = [];
        this.hooks.openSheet({ id: action === "open-build" ? "build" : "shipyard", props: { systemId, bodyId } });
        break;
      }
      case "system-enter":
        if (button.dataset.id) this.hooks.enterSystem(button.dataset.id);
        break;
      case "system-exit": this.hooks.exitSemantic(); break;
      case "system-focus":
        if (button.dataset.id) this.hooks.focusSystem(button.dataset.id);
        break;
      case "system-fleet":
        if (button.dataset.id) this.hooks.focusFleet(button.dataset.id);
        break;
      case "system-ship-production":
        if (button.dataset.id) this.ctx.send({ type: "ShipProduction", system_id: button.dataset.id });
        break;
      case "open-logistics":
        // A Freighter's route line opens its trip.
        if (button.dataset.fleet) focusIndustryFleet(this.ctx.state, button.dataset.fleet);
        this.hooks.openSheet({ id: "logistics" });
        break;
      case "open-doctrine": this.hooks.openSheet({ id: "doctrine" }); break;
      case "planet-tab":
        if (isPlanetTab(button.dataset.tab)) this.planetTab = button.dataset.tab;
        this.sheets.refresh();
        break;
      case "worker-set": this.setWorkers(button); break;
      // §research-wiring Talent: post or recall a specialist; Academy courses.
      case "specialist-set": this.setSpecialist(button); break;
      case "stream": if (button.dataset.system && button.dataset.to) {
        this.ctx.send({ type: "SetDesignation", cap: "mass_streams", target: { system_pair: [button.dataset.system, button.dataset.to] } });
      } break;
      case "train": if (button.dataset.system && button.dataset.kind) this.ctx.send({ type: "TrainSpecialist", system_id: button.dataset.system, specialist: button.dataset.kind }); break;
      // §designations: a crown on this world.
      case "crown": if (button.dataset.system && (button.dataset.cap === "crown_vein" || button.dataset.cap === "crown_project")) {
        this.ctx.send({ type: "SetDesignation", cap: button.dataset.cap, target: { body: { system: button.dataset.system, body: Number(button.dataset.body) } } });
      } break;
      case "refining-select": {
        const ore = button.dataset.ore as Commodity;
        if (buildOption("smelter")?.refining_recipes?.some(r => r.inputs[0][0] === ore)) {
          this.refiningDraft = { system: button.dataset.system ?? "", body: Number(button.dataset.body), ore };
          this.sheets.refresh();
        }
        break;
      }
      case "refining-cancel": this.refiningDraft = null; this.coreDraft = null; this.sheets.refresh(); break;
      case "refining-core-select": {
        const product = button.dataset.product as Commodity;
        if (buildOption("drive_works")?.refining_recipes?.some(r => r.output === product)) {
          this.coreDraft = { system: button.dataset.system ?? "", body: Number(button.dataset.body), product };
          this.sheets.refresh();
        }
        break;
      }
      case "refining-core-confirm": {
        const draft = this.coreDraft;
        const site = this.ctx.state.systems.find(s => s.id === draft?.system && s.owner === this.ctx.state.playerId);
        if (site && draft && site.bodies.some(b => b.id === draft.body && b.structures.drive_works > 0)) {
          const line = site.assignments.find(a => a.body_id === draft.body && a.structure === "drive_works");
          this.ctx.send({ type: "SetAssignment", system_id: site.id, body_id: draft.body,
            structure: "drive_works", workers: line?.workers ?? 0, specialists: line?.specialists ?? {}, refining_ore: draft.product });
        }
        this.coreDraft = null;
        this.sheets.refresh();
        break;
      }
      case "refining-confirm": {
        const draft = this.refiningDraft;
        const site = this.ctx.state.systems.find(s => s.id === draft?.system && s.owner === this.ctx.state.playerId);
        if (site && draft && site.bodies.some(b => b.id === draft.body && b.structures.smelter > 0)) {
          const line = site.assignments.find(a => a.body_id === draft.body && a.structure === "smelter");
          this.ctx.send({ type: "SetAssignment", system_id: site.id, body_id: draft.body,
            structure: "smelter", workers: line?.workers ?? 0, specialists: line?.specialists ?? {}, refining_ore: draft.ore });
        }
        this.refiningDraft = null;
        this.sheets.refresh();
        break;
      }
      case "migration-policy": {
        const system = button.dataset.system;
        const body = Number(button.dataset.body);
        const policy = button.dataset.policy as MigrationPolicy | undefined;
        if (system && Number.isFinite(body) && isMigrationPolicy(policy)) this.ctx.send({ type: "SetMigrationPolicy", system_id: system, body_id: body, policy });
        break;
      }
      case "migration-relocate": this.relocateMigrants(button); break;
      case "module-build": {
        const system = button.dataset.system;
        const module = button.dataset.module as ModuleKind | undefined;
        if (system && isModule(module)) this.ctx.send({ type: "BuildModule", system_id: system, module });
        break;
      }
      case "build-select":
        this.selectedBuild = button.dataset.key ?? "";
        this.sheets.refresh();
        break;
      case "build-queue": this.queueStructure(button); break;
      case "build-cancel":
        this.cancelArmed = Number(button.dataset.job);
        this.sheets.refresh();
        break;
      case "build-cancel-keep":
        this.cancelArmed = null;
        this.sheets.refresh();
        break;
      case "build-cancel-confirm": {
        const system = button.dataset.system;
        const id = Number(button.dataset.job);
        if (system && Number.isInteger(id)) {
          this.ctx.send({ type: "CancelBuild", system_id: system, job_id: id });
          this.cancelSent.add(id);
          this.hooks.notice("Cancel sent. Its goods return when the order arrives.");
        }
        this.cancelArmed = null;
        this.sheets.refresh();
        break;
      }
      case "ship-select":
        if (isShipKind(button.dataset.kind)) {
          this.selectedHull = button.dataset.kind;
          this.pendingFit = [];
        }
        this.sheets.refresh();
        // After the press guard has applied the new detail, reveal its model
        // instead of leaving a phone at the top of a long hull catalogue.
        window.setTimeout(() => document.getElementById("m-ship-detail")?.scrollIntoView({ block: "start" }), 0);
        break;
      case "ship-fit": {
        const module = button.dataset.module as ModuleKind | undefined;
        if (!isModule(module) || !this.selectedHull) break;
        if (this.pendingFit.includes(module)) this.pendingFit = this.pendingFit.filter((candidate) => candidate !== module);
        else if (this.pendingFit.length < (MODULE_SLOTS[this.selectedHull] ?? 0)) this.pendingFit.push(module);
        this.sheets.refresh();
        break;
      }
      case "ship-fit-pick": {
        const fit = this.ctx.state.syndicate?.fits?.find((candidate) => candidate.name === button.dataset.name);
        if (fit) { this.selectedHull = fit.kind; this.pendingFit = [...fit.modules]; this.sheets.refresh(); }
        break;
      }
      case "ship-fit-delete":
        if (button.dataset.name) this.ctx.send({ type: "DeleteFit", name: button.dataset.name });
        break;
      case "ship-fit-save": this.saveFit(); break;
      case "ship-queue": this.queueShips(button); break;
      case "open-yard":
        if (button.dataset.system) {
          this.yardPicked = null;
          this.hooks.openSheet({ id: "yard", props: { systemId: button.dataset.system, kind: button.dataset.kind } });
        }
        break;
      case "yard-pick":
        this.yardPicked = button.dataset.job ?? null;
        this.sheets.refresh();
        break;
      case "hub-market": this.hooks.openSheet({ id: "market" }); break;
      case "hub-fleet":
        if (button.dataset.id) this.hooks.focusFleet(button.dataset.id);
        break;
      case "logistics-clear": {
        const id = Number(button.dataset.id);
        if (Number.isFinite(id)) this.ctx.send({ type: "ClearStandingOrder", order_id: id });
        break;
      }
      case "logistics-add": this.addStandingOrder(); break;
      case "doctrine-save": this.saveDoctrine(); break;
    }
    return true;
  }

  /// §belts/§pockets: a charted belt's Prospect and Mine buttons, as on desktop.
  private beltWorkHtml(dynamic: SystemStateView, body: BodyView): string {
    const { prospect, mine, anchor, station, note } = beltActions(this.ctx.state.ghosts, this.ctx.state.playerId, dynamic, body);
    const at = `data-system="${esc(dynamic.id)}" data-body="${body.id}"`;
    const buttons = prospect.map((a) => `<button type="button" data-mobile-act="planet-prospect" data-fleet="${esc(a.fleet)}" ${at}>${esc(a.text)}</button>`).join("")
      + mine.map((a) => `<button type="button" data-mobile-act="planet-mine" data-fleet="${esc(a.fleet)}" ${at} data-ore="${esc(a.ore!)}"${a.pocket ? ` data-pocket="1"` : ""}>${esc(a.text)}</button>`).join("")
      + anchor.map((a) => `<button type="button" data-mobile-act="planet-anchor" data-fleet="${esc(a.fleet)}" ${at}>${esc(a.text)}</button>`).join("");
    const depot = station ? `<div class="m-order"><b>Station · ${esc(anchorStatus(station.anchor, liveSimTime()))}</b><span>${fmt(station.held)} / ${fmt(station.capacity)}</span></div>` : "";
    return `<section class="m-section"><h3>Ship mining</h3>${depot}${note ? `<div class="m-muted">${esc(note)}</div>` : ""}${buttons ? `<div class="m-action-grid">${buttons}</div>` : ""}</section>`;
  }

  private ownsAction(action: string): boolean {
    return /^(open-signals|research|officer|operation|syndicate|diplomacy|flagship|faction|open-rankings|ranking|system|open-ground|open-planet|open-build|open-shipyard|open-yard|yard-pick|open-logistics|open-doctrine|planet|worker|refining|migration|module-build|build|ship-|hub-|logistics|doctrine)/.test(action);
  }

  private renderResearch(): SheetView {
    const research = this.ctx.state.research;
    if (!research) return { title: "Research", eyebrow: "Corporate programme boards", html: `<div class="m-empty">Research data has not arrived.</div>` };
    if (!FIELD_ORDER.includes(this.researchField as typeof FIELD_ORDER[number])) this.researchField = FIELD_ORDER[0];
    const queue = this.researchQueue();
    const fieldTabs = FIELD_ORDER.map((field) => `<button type="button" data-mobile-act="research-field" data-field="${field}" aria-selected="${field === this.researchField}">${FIELD_TITLE[field]}</button>`).join("");
    const active = research.active
      ? `<article class="m-feature-card"><small>ACTIVE · ${research.rate.toFixed(2)}/s</small><b>${esc(research.active.name)}</b>` +
        `<div class="m-progress"><i style="width:${Math.min(100, research.active.progress / Math.max(1, research.active.cost) * 100).toFixed(1)}%"></i></div>` +
        `<span>${fmt(research.active.progress)} / ${fmt(research.active.cost)} · ${research.active.eta_secs == null ? (research.stalled ? "stalled" : "awaiting supply") : `ETA ${fmtEta(research.active.eta_secs)}`}</span>` +
        // §research-grant (R3): credits buy a research-hour, a few a day.
        (research.grant?.price != null ? `<button type="button" data-mobile-act="research-grant" ${research.grant.left > 0 ? "" : "disabled"}>Fund ${fmtEta(research.grant.secs)} · ${fmt(research.grant.price)} Cr · ${research.grant.left}/${research.grant.per_day} today</button>` : "") + `</article>`
      : `<div class="m-empty">No active programme. Add an available programme below.</div>`;
    const queueRows = queue.map((id, index) => {
      const programme = research.programmes.find((candidate) => candidate.id === id);
      return `<div class="m-queue-row"><span><b>${index + 1}. ${esc(programme?.name ?? id)}</b><small>${esc(FIELD_TITLE[programme?.field ?? ""] ?? "Programme")}</small></span>` +
        `<div><button type="button" data-mobile-act="research-up" data-index="${index}" ${index === 0 ? "disabled" : ""}>↑</button>` +
        `<button type="button" data-mobile-act="research-down" data-index="${index}" ${index === queue.length - 1 ? "disabled" : ""}>↓</button>` +
        `<button type="button" data-mobile-act="research-remove" data-index="${index}">×</button></div></div>`;
    }).join("");
    const programmes = research.programmes
      .filter((programme) => programme.field === this.researchField)
      .sort((a, b) => a.tier - b.tier || (a.school ?? "").localeCompare(b.school ?? "") || a.name.localeCompare(b.name))
      .map((programme) => this.programmeCard(programme)).join("");
    const academies = research.academies.map((academy) => `<div class="m-order"><b>${esc(academy.system)} · Tier ${academy.tier}</b><span>${academy.supplied ? `${academy.rate.toFixed(2)}/s` : "unsupplied"}</span></div>`).join("");
    return {
      title: "Research",
      eyebrow: "One board at a time · full programme tree",
      html: active + `<section class="m-section"><h3>Queue</h3>${queueRows || `<div class="m-muted">Queue empty.</div>`}</section>` +
        `<div class="m-scroll-tabs">${fieldTabs}</div><div class="m-programmes">${programmes || `<div class="m-empty">No programmes on this board.</div>`}</div>` +
        `<details class="m-details"><summary>Academy contribution</summary><div>${academies || `<div class="m-muted">No staffed Academies.</div>`}</div></details>`,
    };
  }

  private programmeCard(programme: ProgrammeView): string {
    const available = programme.state === "available";
    const gate = programme.gate
      ? `<span class="m-warning">${esc(programme.gate.label)} · ${fmt(programme.gate.current)}/${fmt(programme.gate.threshold)}</span>` : "";
    const supplies = programme.state === "completed" ? [] : researchSuppliesPerHour(programme);
    const supplyRow = supplies.length ? `<span class="m-supplies" title="Goods an Academy draws per hour, at research rate 1">Supplies /h ${supplies
      .map(([good, n]) => `${commodityIcon(good)}<b>${supplyAmount(n)}</b>`).join(" ")}</span>` : "";
    return `<article class="m-card is-${esc(programme.state)}"><header><small>Tier ${programme.tier}${programme.school ? ` · ${esc(human(programme.school))}` : ""}</small><em>${esc(programme.state)}</em></header>` +
      `<b>${esc(programme.name)}</b><p>${esc(programme.blurb)}</p>${programme.recovered_data ? `<span>Recovered data · ${Math.round(programme.recovered_data / programme.cost * 100)}% research work banked</span>` : ""}${supplyRow}${gate}` +
      (available ? `<button type="button" class="m-primary" data-mobile-act="research-add" data-id="${esc(programme.id)}">Add to queue · ${fmt(programme.cost)} s</button>` : "") + `</article>`;
  }

  private changeResearchQueue(action: "add" | "up" | "down" | "remove", value: string | number): void {
    const queue = this.researchQueue();
    if (action === "add") {
      const id = String(value);
      if (!queue.includes(id)) queue.push(id);
    } else {
      const index = Number(value);
      if (!Number.isInteger(index) || index < 0 || index >= queue.length) return;
      if (action === "remove") queue.splice(index, 1);
      else if (action === "up" && index > 0) [queue[index - 1], queue[index]] = [queue[index], queue[index - 1]];
      else if (action === "down" && index < queue.length - 1) [queue[index], queue[index + 1]] = [queue[index + 1], queue[index]];
    }
    this.ctx.send({ type: "SetResearchQueue", queue });
  }

  private researchQueue(): string[] {
    const research = this.ctx.state.research;
    if (!research) return [];
    return research.active ? [research.active.id, ...research.queue] : [...research.queue];
  }

  private renderOfficers(): SheetView {
    const home = foundingHomeSystemId();
    const homeState = home ? this.ctx.state.systems.find((system) => system.id === home) : undefined;
    const academy = homeState?.structures?.academy ?? 0;
    const pending = homeState?.builds?.filter((job) => job.key === "officer_commission").length ?? 0;
    const living = this.ctx.state.captains.filter((captain) => captain.loss_fate !== "killed").length;
    const canRecruit = !!home && academy > 0 && living + pending < this.ctx.state.captainCapacity;
    const cards = this.ctx.state.captains.map((captain) => this.officerCard(captain, home)).join("");
    return {
      title: `Officers · ${living}/${this.ctx.state.captainCapacity}`,
      eyebrow: "Personnel · physical command",
      html: `<section class="m-section m-section--first"><div class="m-action-grid"><button type="button" class="m-primary" data-mobile-act="officer-recruit" ${canRecruit ? "" : "disabled"}>Commission Lieutenant</button></div>` +
        `<small class="m-hint">Academy ${academy || "required"} · 60s · 40 Provisions · 20 Semiconductors · 10 Machinery</small></section>` +
        `<div class="m-officer-list">${cards || `<div class="m-empty">No officer reports available.</div>`}</div>`,
    };
  }

  private officerCard(entry: CaptainRosterView, home: string | null): string {
    const report = entry.report;
    const assigned = entry.assigned_fleet ? this.ctx.state.ghosts.find((fleet) => fleet.id === entry.assigned_fleet && fleet.own) : undefined;
    const recovering = entry.recovering_until !== null && entry.recovering_until > this.ctx.state.simTime;
    const killed = entry.loss_fate === "killed";
    const station = entry.assigned_fleet === null
      ? entry.stationed_system
      : assigned && this.ctx.state.systems.find((system) => system.owner === this.ctx.state.playerId && dockedAtSystem(assigned, system.id))?.id;
    const local = !recovering && !killed && !!station && (entry.assigned_fleet === null || (!!assigned && Math.hypot(assigned.vel.x, assigned.vel.y) < 0.5));
    const canTrain = local && station === home;
    const occupied = new Set(this.ctx.state.captains.flatMap((captain) => captain.assigned_fleet ? [captain.assigned_fleet] : []));
    const eligible = report && station ? this.ctx.state.ghosts.filter((fleet) => fleet.own && dockedAtSystem(fleet, station) && Math.hypot(fleet.vel.x, fleet.vel.y) < 0.5 && (!occupied.has(fleet.id) || fleet.id === entry.assigned_fleet) && fleetCommandLoad(fleet) <= report.command_capacity && fleet.id !== entry.assigned_fleet) : [];
    const status = killed ? "Killed in action" : recovering ? `${human(entry.loss_fate ?? "recovery")} · ${fmtEta(entry.recovering_until! - this.ctx.state.simTime)}` : entry.assigned_fleet ? assigned ? `Assigned · ${officerFleetName(assigned)}` : "Assigned · report in transit" : entry.stationed_system ? `Reserve · ${systemName(entry.stationed_system)}` : "Reserve";
    // §signals: hurt on a Hazardous signal — sits signals out, still commands.
    const hurt = entry.hurt_until != null && entry.hurt_until > this.ctx.state.simTime ? ` · Hurt · ${fmtEta(entry.hurt_until - this.ctx.state.simTime)}` : "";
    const age = report?.portrait_age ?? "young";
    const titled = report ? `${captainTitle(report.title)} ${entry.name}` : entry.name;
    const floor = report ? captainXpFloor(report.level) : 0;
    const progress = report ? Math.max(0, Math.min(100, (report.xp - floor) / Math.max(1, report.next_level_xp - floor) * 100)) : 0;
    const assign = eligible.length
      ? `<div class="m-inline-form"><select id="m-officer-fleet-${entry.id}">${eligible.map((fleet) => option(fleet.id, `${officerFleetName(fleet)} · ${fleetCommandLoad(fleet)}/${report!.command_capacity}`)).join("")}</select><button type="button" data-mobile-act="officer-assign" data-captain="${entry.id}">${entry.assigned_fleet ? "Transfer" : "Assign"}</button></div>` : "";
    const training = report && report.unspent > 0
      ? `<div class="m-chip-actions">${(["command", "navigation", "fieldcraft", "logistics"] as CaptainAttribute[]).map((attribute) => `<button type="button" data-mobile-act="officer-train" data-captain="${entry.id}" data-attribute="${attribute}" ${canTrain ? "" : "disabled"}>+ ${human(attribute)}</button>`).join("")}</div>` : "";
    return `<article class="m-officer-card">${captainPortrait(entry.portrait, age, `Portrait of ${titled}`, "m-officer-portrait", true)}<div><header><b>${esc(titled)}</b><em>${esc(status + hurt)}</em></header>` +
      (report ? `<small>Level ${report.level}</small><div class="m-progress"><i style="width:${progress.toFixed(1)}%"></i></div><p>Cmd ${report.attributes.command} · Nav ${report.attributes.navigation} · Field ${report.attributes.fieldcraft} · Log ${report.attributes.logistics}</p>` : `<p>Personnel light has not reached command.</p>`) +
      training + assign + (entry.assigned_fleet && local ? `<button type="button" data-mobile-act="officer-reserve" data-captain="${entry.id}">Return to reserve</button>` : "") +
      (assigned ? `<button type="button" data-mobile-act="officer-fleet" data-id="${esc(assigned.id)}">Select formation</button>` : "") + `</div></article>`;
  }

  private renderOperations(): SheetView {
    if (explorationFocused(this.ctx.state)) return { title: "Exploration", html: explorationHtml(this.ctx.state) };
    const [stageTitle, stageCopy] = MIDGAME_COPY[this.ctx.state.midgameStage] ?? [human(this.ctx.state.midgameStage), ""];
    // §contract-board: today's contracts have their own section.
    const board = boardSummary(this.ctx.state, liveSimTime());
    const listed = this.ctx.state.operations.filter((operation) => !board?.contracts.includes(operation));
    const active = listed.filter((operation) => operation.state === "active" && operation.joined);
    const available = listed.filter((operation) => operation.state === "offered" || (operation.state === "active" && !operation.joined));
    const history = listed.filter((operation) => !["offered", "active"].includes(operation.state)).sort((a, b) => b.reported_at - a.reported_at);
    const selected = this.ctx.state.operations.find(o => o.id === this.handoffContract);
    const counterRaids = [...active, ...available].filter(o => o !== selected && o.briefing?.follow_up.startsWith("counter_raid_"));
    const rows = (this.operationTab === "active" ? active : this.operationTab === "available" ? available : history)
      .filter(o => !counterRaids.includes(o));
    return {
      title: "Operations",
      eyebrow: `${stageTitle} · contracts and objectives`,
      html: explorationHtml(this.ctx.state) + `<article class="m-feature-card"><small>CURRENT ARC</small><b>${esc(stageTitle)}</b><span>${esc(stageCopy)}</span></article>` +
        (selected ? `<section class="m-section"><h3>Funding contract</h3>${this.operationCard(selected)}</section>` : "") +
        (counterRaids.length ? `<section class="m-section"><h3>Defend home · strike back</h3>${counterRaids.map(o => this.operationCard(o)).join("")}</section>` : "") + this.handoffHtml() + this.ledgerHtml() +
        (this.ctx.state.signals ? `<button type="button" class="m-signals-link" data-mobile-act="open-signals"><b>Signals</b><span>${esc(signalsSummary(this.ctx.state.signals, liveSimTime()))}</span><span aria-hidden="true">›</span></button>` : "") +
        (board ? `<section class="m-section"><h3>Contract board</h3><p class="m-section__note">${esc(boardLine(board))}</p>${board.contracts.map(o => this.operationCard(o)).join("") || `<div class="m-empty">Today's contracts are on their way from the Market Hub.</div>`}</section>` : "") +
        `<div class="m-subtabs m-subtabs--3">${(["active", "available", "history"] as OperationTab[]).map((tab) => `<button type="button" data-mobile-act="operation-tab" data-tab="${tab}" aria-selected="${tab === this.operationTab}">${human(tab)} · ${(tab === "active" ? active : tab === "available" ? available : history).length}</button>`).join("")}</div>` +
        `<div class="m-programmes">${rows.filter(o => o !== selected).map((operation) => this.operationCard(operation)).join("") || `<div class="m-empty">No other ${this.operationTab} operations.</div>`}</div>`,
    };
  }

  private handoffHtml(): string {
    const goals = postVictoryHandoff();
    if (!goals.length) return "";
    return `<section class="m-section"><h3>Your next chapter</h3>${goals.map(g => `<article class="m-feature-card"><small>${esc(g.status)}</small><b>${esc(g.title)}</b><span>${esc(g.summary)}</span>
      <ul>${g.requirements.map(r => `<li>${r.met ? "✓" : "○"} ${esc(r.text)}</li>`).join("")}</ul>
      <small>${g.id === "colony" ? "Reward" : "Gain"}: ${esc(g.payoff)}</small>
      <button type="button" data-mobile-act="${g.action?.kind === "warehouse" ? "founding-action" : "next-goal"}" data-kind="market" data-goal="${g.id}" ${g.action ? "" : "disabled"}>${esc(g.actionLabel)}</button>
      ${g.prospect && !g.done ? `<button type="button" data-mobile-act="next-prospect" data-goal="${g.id}">Inspect ${esc(systemName(g.prospect))}</button>` : ""}
      ${g.funding && !g.done ? `<button type="button" data-mobile-act="next-funding" data-goal="${g.id}">Fund this · ${esc(operationTitle(g.funding))}<small>${esc(operationReward(g.funding))}</small></button>` : ""}</article>`).join("")}</section>`;
  }

  /** §founding R10/R12: the charter ledger, three Authority goals at a time,
   * led by the syndicate step and its waiting invitations. */
  private ledgerHtml(): string {
    const ledger = this.ctx.state.charterLedger;
    if (!ledger?.goals.length) return "";
    const invites = this.ctx.state.syndicateInvites.length;
    const sheets: Partial<Record<string, [string, string]>> = {
      syndicate: ["syndicate", invites ? `${invites} invitation${invites === 1 ? "" : "s"} waiting · Open Syndicate` : "Open Syndicate"],
      limit_orders: ["market", "Open Market Hub"],
      research: ["research", "Open Research"],
    };
    return `<section class="m-section"><h3>Charter ledger · ${ledger.done}/${ledger.total}</h3>${ledger.goals.map(goal => {
      const pct = Math.min(100, goal.target > 0 ? (goal.progress / goal.target) * 100 : 0);
      const open = sheets[goal.metric];
      return `<article class="m-feature-card"><small>${esc(serviceReward(goal.standing, goal.research))} · paid once</small><b>${esc(goal.title)}</b><span>${esc(goal.detail)}</span>` +
        `<small>${Math.floor(goal.progress).toLocaleString()} / ${Math.floor(goal.target).toLocaleString()} · ${Math.round(pct)}%</small>` +
        (open ? `<button type="button" data-mobile-act="operation-ledger" data-sheet="${open[0]}">${esc(open[1])}</button>` : "") + `</article>`;
    }).join("")}</section>`;
  }

  private openHandoffGoal(action: string, id?: string): void {
    const goal = postVictoryHandoff().find(g => g.id === id);
    if (!goal) return;
    if (action === "next-funding") {
      if (goal.funding) { this.handoffContract = goal.funding.id; this.sheets.refresh(); }
      return;
    }
    const a = action === "next-prospect" && goal.prospect ? { kind: "system" as const, id: goal.prospect } : goal.action;
    if (!a) return;
    // Like desktop these are navigation, not orders or tutorial rewards.
    if (a.kind === "fleet") this.hooks.focusFleet(a.id);
    else if (a.kind === "system") this.hooks.focusSystem(a.id);
    else if (a.kind === "world") this.openPlanet(a.system, a.body);
    else if (a.kind === "build") {
      this.selectedBuild = a.mode === "structures" ? a.select : "";
      this.selectedHull = a.mode === "ships" ? a.select as ShipKind : "";
      this.pendingFit = [];
      this.hooks.openSheet({ id: a.mode === "ships" ? "shipyard" : "build", props: { systemId: a.system, bodyId: a.body } });
    } else if (a.kind === "research" || a.kind === "operations") this.hooks.openSheet({ id: a.kind });
  }

  private operationCard(operation: OperationView): string {
    const pct = Math.max(0, Math.min(100, operation.goal > 0 ? operation.progress / operation.goal * 100 : 0));
    const fleetOptions = this.ctx.state.ghosts.filter((fleet) => fleet.own
      && (operation.kind.kind === "survey_expedition"
        ? surveyCapable(fleet)
        : ["rescue_salvage", "prize_recovery"].includes(operation.kind.kind) ? fleetReadiness(fleet).cargoCapacity > 0
        : !["freight_escort", "privateer_patrol", "pirate_bounty", "combat_objective"].includes(operation.kind.kind) || guardCapable(fleet)))
      .map((fleet) => option(fleet.id, officerFleetName(fleet), fleet.id === operation.assigned_fleet)).join("");
    const charges = this.ctx.state.ghosts.filter(g => g.own && fleetReadiness(g).cargoCapacity > 0)
      .map(g => option(g.id, officerFleetName(g), false)).join("");
    const id = safeId(operation.id);
    let actions = "";
    if ((operation.state === "offered" || (operation.state === "active" && !operation.joined)) && !operation.joined) actions = `<button type="button" class="m-primary" data-mobile-act="operation-accept" data-id="${esc(operation.id)}">Accept</button>`;
    if (operation.state === "active" && operation.joined) {
      actions += fleetOptions ? `<div class="m-inline-form"><select aria-label="Assigned fleet" id="m-op-fleet-${id}">${fleetOptions}</select>${operation.kind.kind === "freight_escort" ? `<select aria-label="Protected Freighter" id="m-op-charge-${id}"><option value="">Choose Freighter</option>${charges}</select>` : ""}<button type="button" data-mobile-act="operation-assign" data-id="${esc(operation.id)}">${operation.kind.kind === "freight_escort" ? "Assign guard" : operation.kind.kind === "survey_expedition" ? "Plot survey" : ["rescue_salvage", "prize_recovery"].includes(operation.kind.kind) && operation.briefing ? "Plot recovery" : operation.kind.kind === "pirate_bounty" && operation.briefing ? "Plot assault" : "Assign"}</button>` +
        (["rescue_salvage", "prize_recovery"].includes(operation.kind.kind) ? `<button type="button" data-mobile-act="operation-recover" data-id="${esc(operation.id)}">Recover now</button>` : "") + `</div>` : "";
      if (operation.kind.kind === "syndicate_megaproject") actions += `<div class="m-inline-form"><input id="m-op-units-${id}" type="number" min="1" inputmode="numeric" value="25"><button type="button" data-mobile-act="operation-contribute" data-id="${esc(operation.id)}">Commit goods</button></div>`;
      if (operation.kind.kind !== "syndicate_megaproject") actions += `<button type="button" data-mobile-act="operation-abandon" data-id="${esc(operation.id)}">Abandon</button>`;
    }
    return `<article class="m-card"><header><small>${esc(human(operation.issuer))}</small><em>${esc(human(operation.state))}</em></header><b>${esc(operationTitle(operation))}</b><p>${esc(operationCopy(operation))}</p>` +
      (operation.briefing ? `<span>${esc(operation.briefing.difficulty)} · ${esc(operation.briefing.suitable_fleets)}</span>` : "") +
      `<div class="m-progress"><i style="width:${pct.toFixed(1)}%"></i></div><span>${operation.progress}/${operation.goal} · ${esc(operationReward(operation))} · ${operation.state === "completed" ? "complete" : `${fmtEta(Math.max(0, operation.expires_at - liveSimTime()))} left`}</span>${actions}</article>`;
  }

  private renderSyndicate(): SheetView {
    const syndicate = this.ctx.state.syndicate;
    const invites = this.ctx.state.syndicateInvites;
    let membership = "";
    if (!syndicate) {
      membership = `<section class="m-trade-card"><h3>Found a syndicate</h3><label>Name<input id="m-syndicate-name" maxlength="32" placeholder="Syndicate name"></label><button type="button" class="m-primary" data-mobile-act="syndicate-create">Create</button></section>` +
        `<section class="m-section"><h3>Invitations</h3>${invites.map((invite) => `<div class="m-service-row"><span><b>${esc(invite.name)}</b></span><button type="button" data-mobile-act="syndicate-accept" data-id="${esc(invite.id)}">Accept</button></div>`).join("") || `<div class="m-muted">No pending invitations.</div>`}</section>`;
    } else {
      const members = syndicate.members.map((member) => {
        const roles = syndicate.is_founder && member.id !== syndicate.founder && member.id !== this.ctx.state.playerId
          ? `<div class="m-chip-actions">${(["member", "quartermaster", "officer"] as SyndicateRole[]).map((role) => `<button type="button" data-mobile-act="syndicate-role" data-member="${esc(member.id)}" data-role="${role}" ${member.role === role ? "disabled" : ""}>${human(role)}</button>`).join("")}</div>` : "";
        return `<div class="m-member"><span><b>${esc(member.name)}</b><small>${member.id === this.ctx.state.playerId ? "you · " : ""}${human(member.role)}</small></span>${roles}</div>`;
      }).join("");
      const canInvite = ["founder", "officer"].includes(syndicate.my_role);
      const titan = this.ctx.state.ghosts.some((fleet) => fleet.own && fleet.composition?.some((stack) => stack.kind === "titan" && stack.count > 0));
      membership = `<article class="m-feature-card"><small>${esc(human(syndicate.my_role))}</small><b>${esc(syndicate.name)}</b><span>${syndicate.members.length} members · mutual non-engagement</span></article>` +
        `<section class="m-section"><h3>Roster</h3>${members}</section>` +
        (canInvite ? `<section class="m-trade-card"><h3>Invite corporation</h3><label>Corporation name<input id="m-syndicate-invite" maxlength="32"></label><button type="button" data-mobile-act="syndicate-invite">Invite</button></section>` : "") +
        (syndicate.projects ? this.projectsBlock(syndicate.projects) : "") +
        (titan ? `<section class="m-trade-card"><h3>Flagship</h3><label>Name<input id="m-flagship-name" maxlength="32" value="${esc(syndicate.flagship_name ?? "")}"></label><button type="button" data-mobile-act="flagship-name">Set name</button></section>` : "") +
        `<div class="m-action-grid"><button type="button" data-mobile-act="syndicate-leave">Leave</button>${syndicate.is_founder ? `<button type="button" class="m-danger" data-mobile-act="syndicate-dissolve">Dissolve</button>` : ""}</div>`;
    }
    return { title: syndicate?.name ?? "Syndicate", eyebrow: "Alliance network · formal diplomacy", html: membership + this.friendInvitesBlock() + this.diplomacyBlock() };
  }

  /** §syndicate-projects (R6): the week's ladder, points and bonuses. */
  private projectsBlock(p: SyndicateProjectsView): string {
    const st = this.ctx.state;
    const rung = currentRung(p);
    const steps = p.rungs.map((_, i) => `<li class="${i < p.rung ? "is-done" : i === p.rung ? "is-current" : ""}">${i < p.rung ? "✓" : i + 1}</li>`).join("");
    let give = `<div class="m-muted">Ladder done · next week ${esc(fmtDays(p.week_ends_in))}</div>`;
    if (rung) {
      const worlds = givingWorlds(st.systems, st.playerId, rung.goods);
      const need = rung.goal - p.progress;
      give = `<div class="m-order"><b>${esc(label(rung.goods))} ${fmt(p.progress)} / ${fmt(rung.goal)}</b><span>+${rung.points} pts</span></div>` +
        (worlds.length ? `<label>From<select id="m-project-system">${worlds.map(([s, n]) => option(s.id, `${systemName(s.id)} · ${fmt(n)}`)).join("")}</select></label><label>Units<input id="m-project-units" type="number" min="1" inputmode="numeric" max="${need}" value="${Math.min(need, worlds[0][1])}"></label><button type="button" class="m-primary" data-mobile-act="project-give">Give</button>`
          : `<div class="m-muted">No ${esc(label(rung.goods))} on your worlds.</div>`);
    }
    const bonuses = p.bonuses.map((b) => {
      const wait = b.next_in !== null ? `<small>yours ${roman(b.mine)} · ${roman(b.mine + 1)} in ${esc(fmtDays(b.next_in))}</small>` : "";
      const raise = p.may_raise && b.next_cost !== null ? `<button type="button" data-mobile-act="bonus-raise" data-bonus="${b.bonus}" ${p.points >= b.next_cost ? "" : "disabled"}>${roman(b.level + 1)} · ${b.next_cost} pts</button>` : "";
      return `<div class="m-service-row"><span><b>${esc(bonusLine(b))}</b>${wait}</span>${raise}</div>`;
    }).join("");
    const given = p.given.length ? `<div class="m-muted">This week · ${p.given.map(([name, n]) => `${esc(name)} ${fmt(n)}`).join(" · ")}</div>` : "";
    return `<section class="m-trade-card"><h3>Projects · week ${p.week} · ${p.points} pts${medalCountsHtml(p.medals)}</h3><small>Closes in ${esc(fmtDays(p.week_ends_in))}</small><ol class="m-ladder">${steps}</ol>${give}</section>` +
      `<section class="m-section"><h3>Bonuses</h3>${bonuses}${given}</section>`;
  }

  /** §settle-near-friends (R7): links that hold a home near this corporation. */
  private friendInvitesBlock(): string {
    const open = this.ctx.state.social?.invites ?? [];
    const rows = open.map((invite) => `<div class="m-invite-row"><input readonly value="${esc(inviteLink(invite.code))}" aria-label="Invite link"><small>${esc(invite.near)} · ${fmtDays(Math.max(0, invite.until - liveSimTime()))}</small><div class="m-chip-actions"><button type="button" data-mobile-act="invite-copy" data-code="${esc(invite.code)}">Copy</button><button type="button" class="m-danger" data-mobile-act="invite-revoke" data-code="${esc(invite.code)}">Revoke</button></div></div>`).join("");
    return `<section class="m-section"><h3>Invite a friend · ${open.length}/${INVITES_MAX}</h3>${rows}<button type="button" class="m-primary" data-mobile-act="invite-new" ${open.length >= INVITES_MAX ? "disabled" : ""}>New invite link</button></section>`;
  }

  /** §social (R5): the syndicate channel and corporation mail. */
  private renderComms(): SheetView {
    const st = this.ctx.state, social = st.social, me = st.playerId;
    const tabs = `<div class="m-chip-actions">${(["channel", "inbox", "sent"] as const).map((tab) => `<button type="button" data-mobile-act="comms-tab" data-tab="${tab}" aria-pressed="${this.commsTab === tab}">${tab === "channel" ? "Channel" : tab === "inbox" ? `Inbox${social?.unread ? ` · ${social.unread}` : ""}` : "Sent"}</button>`).join("")}<button type="button" class="m-primary" data-mobile-act="comms-compose">New mail</button></div>`;
    let body = "";
    if (this.commsCompose) {
      const names = [...new Set(st.rankings.filter((r) => r.player_id !== me).map((r) => r.name))].sort();
      body = `<section class="m-trade-card"><h3>New mail</h3><label>To<input id="m-mail-to" list="m-mail-corps" maxlength="40" value="${esc(this.commsCompose.to)}"></label><datalist id="m-mail-corps">${names.map((n) => `<option value="${esc(n)}"></option>`).join("")}</datalist><label>Subject<input id="m-mail-subject" maxlength="80" value="${esc(this.commsCompose.subject)}"></label><label>Message<textarea id="m-mail-body" maxlength="2000" rows="6"></textarea></label><div class="m-action-grid"><button type="button" class="m-primary" data-mobile-act="comms-send">Send</button><button type="button" data-mobile-act="comms-back">Cancel</button></div></section>`;
    } else if (this.commsTab === "channel") {
      if (!st.syndicate) body = `<div class="m-muted">Join a syndicate to use its channel.</div>`;
      else {
        markChannelSeen(social);
        const muted = new Set(social?.muted ?? []);
        const members = (social?.members ?? []).map((m) => `<div class="m-service-row"><span><b>${m.online ? "● " : ""}${esc(m.name)}</b><small>${esc(presenceText(m))}</small></span>${m.player !== me ? `<button type="button" data-mobile-act="comms-mute" data-player="${esc(m.player)}" data-mute="${muted.has(m.player) ? "0" : "1"}">${muted.has(m.player) ? "Unmute" : "Mute"}</button>` : ""}</div>`).join("");
        const lines = (social?.channel ?? []).map((line) => `<div class="m-chat-line"><b>${esc(line.author_name)}</b> <small>${ago(line.at)}</small>${line.author !== me ? ` <button type="button" class="m-chat-report" data-mobile-act="comms-report" data-kind="chat" data-id="${line.id}">⚑</button>` : ""}<p>${esc(line.text)}</p></div>`).join("");
        body = `<section class="m-section"><div class="m-chat-log">${lines || `<div class="m-muted">No messages yet.</div>`}</div><div class="m-chat-compose"><input id="m-chat-input" maxlength="400" placeholder="Message ${esc(st.syndicate.name)}"><button type="button" class="m-primary" data-mobile-act="comms-say">Send</button></div></section><section class="m-section"><h3>Members</h3>${members}</section>`;
      }
    } else {
      const mails = (this.commsTab === "sent" ? social?.sent : social?.inbox) ?? [];
      const open = mails.find((m) => m.id === this.commsMail);
      if (open) {
        const reply = this.commsTab === "inbox" ? `<button type="button" class="m-primary" data-mobile-act="comms-compose" data-to="${esc(open.from_name)}" data-subject="${esc(open.subject.startsWith("Re: ") ? open.subject : `Re: ${open.subject}`)}">Reply</button><button type="button" data-mobile-act="comms-report" data-kind="mail" data-id="${open.id}">Report</button>` : "";
        body = `<article class="m-mail-open"><b>${esc(open.subject)}</b><small>${esc(this.commsTab === "sent" ? `To ${open.to_name}` : `From ${open.from_name}`)} · ${ago(open.at)} ago</small><p>${esc(open.body)}</p><div class="m-action-grid">${reply}<button type="button" data-mobile-act="comms-back">Back</button><button type="button" class="m-danger" data-mobile-act="comms-delete" data-id="${open.id}">Delete</button></div></article>`;
      } else {
        body = mails.map((m) => `<button type="button" class="m-list-row${this.commsTab === "inbox" && !m.read ? " is-unread" : ""}" data-mobile-act="comms-open" data-id="${m.id}" data-unread="${this.commsTab === "inbox" && !m.read ? "1" : ""}"><span class="m-list-row__main"><b>${esc(this.commsTab === "sent" ? m.to_name : m.from_name)}</b><small>${esc(m.subject)}</small></span><span class="m-list-row__meta"><small>${ago(m.at)}</small></span></button>`).join("") || `<div class="m-muted">${this.commsTab === "sent" ? "Nothing sent." : "No mail."}</div>`;
      }
    }
    return { title: "Comms", eyebrow: "Authority relay · no light lag", html: tabs + body };
  }

  private diplomacyBlock(): string {
    const diplomacy = this.ctx.state.diplomacy;
    const incoming = diplomacy?.incoming.map((proposal) => `<div class="m-decision"><b>${esc(proposal.name)} · ${esc(human(proposal.treaty))}</b><div><button type="button" data-mobile-act="diplomacy-response" data-id="${proposal.id}" data-answer="decline">Decline</button><button type="button" class="m-primary" data-mobile-act="diplomacy-response" data-id="${proposal.id}" data-answer="accept">Accept</button></div></div>`).join("") ?? "";
    const relations = diplomacy?.relations.map((relation) => `<div class="m-service-row"><span><b>${esc(relation.name)}</b><small>${relation.war_activates_at ? `war in ${fmtEta(relation.war_activates_at - liveSimTime())}` : human(relation.state)}</small></span>${["non_aggression", "ceasefire"].includes(relation.state) ? `<button type="button" data-mobile-act="diplomacy-cancel" data-target="${esc(relation.other)}">End</button>` : ""}</div>`).join("") ?? "";
    return `<section class="m-section"><h3>Diplomacy</h3>${incoming}${relations || `<div class="m-muted">No arrived bilateral agreements or declarations.</div>`}</section>` +
      `<section class="m-trade-card"><h3>Formal action</h3><label>Corporation name<input id="m-diplomacy-name" maxlength="32"></label><div class="m-action-grid"><button type="button" data-mobile-act="diplomacy-propose">Offer pact</button><button type="button" data-mobile-act="diplomacy-ceasefire">Ceasefire</button><button type="button" class="m-danger" data-mobile-act="diplomacy-war">Declare war</button></div><p>War and separation changes respect their no-surprise notice windows.</p></section>`;
  }

  private renderFaction(): SheetView {
    const charter = this.ctx.state.charter;
    if (!charter) return { title: "Faction", eyebrow: "Terran Charter Authority", html: `<div class="m-empty">No charter on file.</div>` };
    const ladder = this.ctx.state.charterLadder.map(([title, threshold]) => `<div class="m-order${title === charter.title ? " is-current" : ""}"><b>${title === charter.title ? "▸ " : ""}${esc(title)}</b><span>${fmt(threshold)} standing</span></div>`).join("");
    // §standing-ladder (R3): buying back stops at full standing.
    const full = charter.full_standing ?? charter.max_standing;
    const shortfall = Math.max(0, full - charter.standing);
    const grades = this.ctx.state.gradeLadder.map(([title, at]) => `<div class="m-order${title === charter.grade_title ? " is-current" : ""}"><b>${title === charter.grade_title ? "▸ " : ""}${esc(title)}</b><span>${fmt(at)} standing</span></div>`).join("");
    const next = charter.next_grade ? `Next ${esc(charter.next_grade.title)} at ${fmt(charter.next_grade.at)}` : "Top grade";
    const gradeHtml = charter.grade_title ? `<section class="m-section"><h3>Grade · ${esc(charter.grade_title)}</h3><p>${next} · today ${fmt(Math.min(charter.earned_today, charter.earn_per_day))}/${fmt(charter.earn_per_day)} · freight ×${charter.tariff_mult.toFixed(2)} · ${fmt(charter.shipment_cap)} per lift</p>${grades}</section>` : "";
    const pay = shortfall > 0 ? `<section class="m-trade-card"><h3>Reinstatement</h3><label>Standing points<input id="m-faction-points" type="number" min="1" max="${Math.ceil(shortfall)}" value="${Math.ceil(Math.min(shortfall, 20))}"></label><p>${fmt(charter.reinstate_cost_per_point)} Cr per point</p><button type="button" data-mobile-act="faction-pay">Pay Authority</button></section>` : "";
    return {
      title: "Terran Charter Authority",
      eyebrow: `${charter.title} · legal standing`,
      html: `<div class="m-stat-grid"><span><small>Standing</small><b>${charter.standing.toFixed(0)}/${charter.max_standing.toFixed(0)}</b></span><span><small>Tariff</small><b>×${charter.tariff_mult.toFixed(2)}</b></span><span><small>Exchange penalty</small><b>${(charter.market_penalty_frac * 100).toFixed(1)}%</b></span><span><small>Status</small><b>${human(charter.status)}</b></span></div>` +
        `${this.gripHtml(charter)}<section class="m-section"><h3>Standing bands</h3>${ladder}</section>${gradeHtml}${pay}<button type="button" class="m-wide-button" data-mobile-act="open-rankings">Open published rankings</button>`,
    };
  }

  /** §season phase 5: your Keystone and the race's clock. */
  private keystoneHtml(): string {
    const season = this.ctx.state.season;
    if (!season) return "";
    const now = liveSimTime();
    const mine = season.keystone;
    const clock = !season.ending && season.ends_at ? `<p>The season ends in ${fmtBuildDur(Math.max(0, season.ends_at - now))} if no Seal fires.</p>` : "";
    if (!mine) return season.keystones.length || clock ? `<section class="m-section"><h3>The race</h3>${clock}${season.keystones.map((k) =>
      `<div class="m-order"><b>${esc(systemName(k.system))} · level ${k.level}/10</b><span>${esc(k.holder_name ?? "unheld")}${k.seal_at ? ` · Seal in ${fmtBuildDur(Math.max(0, k.seal_at - now))}` : ""}</span></div>`).join("")}</section>` : "";
    const world = this.ctx.state.systems.find((system) => system.id === mine.system);
    const rows = keystoneFeed(season, (world?.stockpile ?? {}) as Partial<Record<Commodity, number>>).map(([good, need, paid, have]) => {
      const units = Math.max(0, Math.min(need - paid, have));
      return `<div class="m-order"><b>${esc(label(good))} · ${fmt(paid)} / ${fmt(need)}</b><span>${fmt(have)} here</span>`
        + `<div class="m-chip-actions"><button type="button" class="m-primary" data-mobile-act="faction-feed" data-system="${esc(mine.system)}" data-commodity="${esc(good)}" data-units="${Math.floor(units)}" ${units >= 1 ? "" : "disabled"}>Feed ${fmt(units)}</button></div></div>`;
    }).join("");
    return `<section class="m-section"><h3>Keystone · ${esc(systemName(mine.system))} · level ${mine.level}/10</h3><p>${mine.powers ? "Network Sight · Instant Command · Gate Step · Precursor Fleet" : "Feeding it is war with the Authority."}</p>${clock}`
      + `${mine.relics_needed ? `<p>Relics held ${mine.relics_held} / ${mine.relics_needed}</p>` : ""}${rows}</section>`;
  }

  /** §season: the grip in force, the levy and the Authority's demands. */
  private gripHtml(charter: CharterView): string {
    const season = this.ctx.state.season;
    const sq = charter.squeeze;
    const now = liveSimTime();
    const grip = season?.grip ?? "charter";
    const levy = charter.levy_frac ?? 0;
    const pct = (f: number) => `${(f * 100).toFixed(f * 100 % 1 ? 1 : 0)}%`;
    const rows: string[] = [];
    if (sq?.rise) rows.push(`<div class="m-order"><b>Levy rise · +${pct(sq.rise.points)}</b><span>${esc(label(sq.rise.commodity))} · ${fmtBuildDur(Math.max(0, sq.rise.due - now))} to answer</span>`
      + `<div class="m-chip-actions"><button type="button" class="m-primary" data-mobile-act="faction-levy" data-accept="1">Accept</button><button type="button" data-mobile-act="faction-levy" data-accept="0">Refuse</button></div></div>`);
    for (const req of sq?.requisitions ?? []) rows.push(`<div class="m-order"><b>Requisition · ${esc(demandText(req.demand, label))}</b><span>${fmtBuildDur(Math.max(0, req.due - now))} left</span>`
      + `<div class="m-chip-actions"><button type="button" class="m-primary" data-mobile-act="faction-requisition" data-id="${req.id}" data-pay="1">Pay</button><button type="button" data-mobile-act="faction-requisition" data-id="${req.id}" data-pay="0">Refuse</button></div></div>`);
    for (const embargo of sq?.embargoes ?? []) rows.push(`<div class="m-order"><b>Embargo · ${esc(label(embargo.commodity))}</b><span>Arrears ${fmt(embargo.arrears)} Cr</span>`
      + `<div class="m-chip-actions"><button type="button" class="m-primary" data-mobile-act="faction-arrears" data-commodity="${esc(embargo.commodity)}">Pay arrears</button></div></div>`);
    if (season?.rebel_since != null) rows.unshift(`<div class="m-order"><b>At war with the Authority</b><span>No Hub · no Authority freight · bounties on your hulls</span></div>`);
    else if (season?.war_at != null && season.war_at > now) rows.unshift(`<div class="m-order"><b>An Authority spy</b><span>War in ${fmtBuildDur(season.war_at - now)}</span></div>`);
    for (const c of season?.crackdowns ?? []) {
      const evidence = c.evidence_until > now && !c.exposed ? (c.read ? "evidence read" : `Scout within ${fmtBuildDur(c.evidence_until - now)}`) : "";
      rows.push(`<div class="m-order"><b>${esc(crackdownTitle(c.kind))}${c.exposed ? " · exposed" : ""}</b><span>${[c.until > now && !c.exposed ? `${fmtBuildDur(c.until - now)} left` : "", evidence].filter(Boolean).join(" · ")}</span></div>`);
    }
    return `<section class="m-section"><h3>Grip ${gripNumeral(grip)} · ${esc(gripTitle(grip))}</h3><p>${esc(DECREE_TEXT[grip])}</p>`
      + `<div class="m-stat-grid"><span><small>Levy</small><b>${levy > 0 ? pct(levy) : "None"}</b></span><span><small>Maturity</small><b>${season ? `${Math.round(season.maturity * 100)}%` : "—"}</b></span>`
      + `<span><small>Resolve</small><b>${Math.round((sq?.resolve ?? 0) * 100)}% ${esc(RESOLVE_BAND[resolveBand(sq?.resolve ?? 0)])}</b></span><span><small>Refusals</small><b>${sq?.refusals ?? 0} / 3</b></span>`
      + `<span><small>Levy paid</small><b>${fmt(sq?.levy_paid ?? 0)} Cr</b></span><span><small>War fleet</small><b>${season?.war_fleet ?? 0} hulls</b></span></div>${rows.join("")}</section>${this.keystoneHtml()}`;
  }

  private renderRankings(entry: SheetEntry): SheetView {
    const corp = propsOf<{ corp: string }>(entry).corp;
    if (corp) {
      const st = this.ctx.state;
      const card = corpCard(st.rankings, corp, st.playerId === null ? null : String(st.playerId), st.simTime);
      const back = `<button type="button" class="m-wide-button" data-mobile-act="open-rankings">All rankings</button>`;
      return { title: card?.row.name ?? "Corporation", eyebrow: "Corporation card",
        html: card ? corpCardHtml(card, back) : `<div class="m-empty">Not in the published ledger yet.</div>${back}` };
    }
    const category = RANK_CATS.find((candidate) => candidate.slug === this.rankingCategory) ?? RANK_CATS[0];
    const chips = RANK_CATS.map((candidate) => `<button type="button" data-mobile-act="ranking-category" data-category="${candidate.slug}" aria-selected="${candidate.slug === category.slug}">${candidate.label}</button>`).join("");
    const rows = [...this.ctx.state.rankings].sort((a, b) => category.value(b) - category.value(a)).map((row, index) => `<button type="button" class="m-ranking-row${row.player_id === this.ctx.state.playerId ? " is-me" : ""}" data-mobile-act="corp-card" data-id="${esc(String(row.player_id))}"><strong>${index + 1}</strong><span><b>${esc(row.name)}${medalChipsHtml(row)}</b><small>${row.titles.map(human).join(" · ") || (row.player_id === this.ctx.state.playerId ? "your corporation" : "")}</small></span><em>${esc(category.text(row))}</em></button>`).join("");
    // §syndicate-projects (R6): the syndicates' table below the corporations'.
    const mine = this.ctx.state.syndicate?.id;
    const syndicates = this.ctx.state.syndicateRankings.map((row, index) => `<div class="m-ranking-row${row.id === mine ? " is-me" : ""}"><strong>${index + 1}</strong><span><b>${esc(row.name)}${medalCountsHtml(row.medals)}</b><small>${row.members} members · ${row.week_points} pts this week</small></span><em>${fmt(row.valuation)} Cr</em></div>`).join("");
    return { title: "Rankings", eyebrow: `Published ledger · ${category.label}`, html: `<div class="m-scroll-tabs m-scroll-tabs--compact">${chips}</div>${rows || `<div class="m-empty">No ledger has been published yet.</div>`}${syndicates ? `<section class="m-section"><h3>Syndicates</h3>${syndicates}</section>` : ""}` };
  }

  private renderSystem(entry: SheetEntry): SheetView {
    const props = propsOf<{ id: string }>(entry);
    const mode = this.ctx.renderer.viewMode;
    const id = props.id ?? this.ctx.state.selectedSystemId ?? (mode.type === "system" ? mode.systemId : undefined);
    const fixed = id ? this.ctx.state.galaxy?.systems.find((system) => system.id === id) : undefined;
    const dynamic = id ? this.ctx.state.systems.find((system) => system.id === id) : undefined;
    if (!fixed) return { title: "System", eyebrow: "System management", html: `<div class="m-empty">Select a star system.</div>` };
    const mine = dynamic?.owner === this.ctx.state.playerId;
    const semantic = mode.type === "system" && mode.systemId === id;
    const tabs = (["overview", "worlds", "production", "construction"] as SystemTab[]).map((tab) => `<button type="button" data-mobile-act="system-tab" data-tab="${tab}" aria-selected="${tab === this.systemTab}">${human(tab)}</button>`).join("");
    const fleets = systemFleetsAt(fixed);
    let body = "";
    if (this.systemTab === "overview") body = this.blockadeBanner(dynamic) + this.systemOverview(fixed.name, dynamic, mine, fleets, id!);
    else if (this.systemTab === "worlds") body = this.systemWorlds(id!, dynamic);
    else if (this.systemTab === "production") body = mine ? this.systemProduction(dynamic!) : `<div class="m-empty">Production is private.</div>`;
    else body = mine ? this.systemConstruction(id!, dynamic!) : `<div class="m-empty">Construction is private.</div>`;
    return {
      title: fixed.name,
      eyebrow: mine ? "Your system · served management" : dynamic?.owner ? "Observed rival holding" : "Unclaimed system",
      html: `<div class="m-action-grid m-action-grid--top"><button type="button" class="m-primary" data-mobile-act="${semantic ? "system-exit" : "system-enter"}" data-id="${esc(id!)}">${semantic ? "Back to galaxy" : "Open System View"}</button>` +
        (mine ? `<button type="button" data-mobile-act="open-logistics">Logistics</button><button type="button" data-mobile-act="open-doctrine">Fleet doctrine</button>` : "") + `</div>` +
        `<div class="m-subtabs">${tabs}</div>${body}`,
    };
  }

  /** The blockade here, as its participants read it (stage, who, what to do). */
  private blockadeBanner(dynamic: SystemStateView | undefined): string {
    const galaxy = this.ctx.state.galaxy;
    if (!dynamic?.blockade || !galaxy) return "";
    const brief = blockadeBrief(dynamic.blockade, liveSimTime(), galaxy.siege_secs, dynamic.ground?.marines_needed);
    return `<article class="m-decision${brief.tone === "bad" ? " is-danger" : ""}"><b>${esc(brief.title)}</b><p>${esc(brief.detail)}</p></article>`;
  }

  private systemOverview(name: string, dynamic: SystemStateView | undefined, mine: boolean, fleets: GhostView[], systemId: string): string {
    const pools = poolUsage(dynamic);
    const homeId = foundingHomeSystemId();
    const home = this.ctx.state.systems.find(s => s.id === homeId && s.owner === this.ctx.state.playerId);
    const purpose = dynamic && systemId !== homeId ? colonyPurpose(dynamic, home) : null;
    const purposeHtml = purpose ? `<section class="m-section"><h3>Colony purpose</h3><b>${esc(purpose.headline)}</b>
      <p>${esc(purpose.homeNeed)}</p><span>Supply imports: ${esc(purpose.imports.map(human).join(", ") || "No essential feedstock missing")}</span></section>` : "";
    const fleetRows = fleets.map((fleet) => `<button type="button" class="m-list-row" data-mobile-act="system-fleet" data-id="${esc(fleet.id)}"><span class="m-list-row__icon">△</span><span class="m-list-row__main"><b>${esc(shipKindLabel(fleet.kind))} fleet</b><small>${fleet.docked ? "docked" : Math.hypot(fleet.vel.x, fleet.vel.y) > 1 ? "under way" : "holding"}</small></span><span class="m-list-row__meta"><small>${fleet.age.toFixed(1)}s delay</small></span></button>`).join("");
    // §node: the map shows only a dim ◈; the sheet names it and its bonus.
    const node = dynamic?.node ? `<span title="${esc(nodeBonusDesc(dynamic.node.bonus))}"><small>Exotic node</small><b>${esc(nodeLabel(dynamic.node, mine).replace(/^◈ /, ""))}</b></span>` : "";
    return purposeHtml + `<div class="m-stat-grid"><span><small>Owner</small><b>${mine ? "Your corporation" : dynamic?.owner ? "Rival" : "Unclaimed"}</b></span><span><small>Worlds</small><b>${dynamic && dynamic.deposits == null && !mine ? "Uncharted" : dynamic?.bodies.length ?? 0}</b></span>${node}` +
      (mine ? `<span><small>Population</small><b>${fmtPopulation(dynamic?.population ?? 0)}</b></span><span><small>Storage</small><b>${fmt(dynamic?.storage_used ?? 0)}/${fmt(dynamic?.storage_cap ?? 0)}</b></span>` : "") + `</div>` +
      (mine ? `<section class="m-section"><h3>Development pools</h3><div class="m-ledger">${(["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span>${POOL_LABEL[pool]}<b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("")}</div>${this.developmentPoolHelp()}</section>` : "") +
      (mine ? this.streamHtml(systemId) : "") +
      `<section class="m-section"><h3>Fleets at ${esc(name)}</h3><div class="m-list">${fleetRows || `<div class="m-muted">No own fleets in the served picture.</div>`}</div></section>` +
      this.groundAction(systemId) +
      `<button type="button" class="m-wide-button" data-mobile-act="system-focus" data-id="${esc(systemId)}">Center on map</button>`;
  }

  private systemWorlds(systemId: string, dynamic: SystemStateView | undefined): string {
    return (dynamic?.bodies ?? []).map((body) => `<button type="button" class="m-world-row" data-mobile-act="open-planet" data-system="${esc(systemId)}" data-body="${body.id}"><span><b>${esc(body.name)}</b><small>${human(body.size)} ${human(body.environment)} · ${human(body.kind)}${gridLine(body) ? ` · <i class="m-grid-line${gridLine(body)!.short ? " is-short" : ""}">${esc(gridLine(body)!.text)}</i>` : ""}</small></span><em>${body.population > 0 ? fmtPopulation(body.population) : body.geology == null ? "unsurveyed" : human(body.geology)}</em></button>`).join("") || (dynamic && dynamic.deposits == null && dynamic.owner !== this.ctx.state.playerId
      ? `<div class="m-empty">Uncharted: its planets, moons and belts stay unknown until one of your Scouts or Survey Probes surveys it.</div>`
      : `<div class="m-empty">No body roster has arrived.</div>`);
  }

  private systemProduction(dynamic: SystemStateView): string {
    const stock = dynamic.stockpile?.map((slot) => `<span>${human(slot.commodity)}<b>${fmt(slot.units)}</b></span>`).join("") ?? "";
    const lines = dynamic.assignments.map((line) => `<div class="m-production-row"><span><b>${esc(line.title)} ×${line.tier}</b><small>${esc(dynamic.bodies.find((body) => body.id === line.body_id)?.name ?? "World")} · ${line.workers} workforce${line.suspended ? ` · ${human(line.suspended)}` : ""}${(line.power ?? 1) < 0.999 ? ` · Power ${Math.round((line.power ?? 1) * 100)}%` : ""}</small></span><em>${line.outputs.map(([commodity, rate]) => `+${rate.toFixed(2)} ${human(commodity)}/s`).join(" · ") || "idle"}</em></div>`).join("");
    return `<section class="m-section m-section--first"><h3>Stockpile · ${fmt(dynamic.storage_used)}/${fmt(dynamic.storage_cap)}</h3><div class="m-ledger">${stock || `<span>Empty</span>`}</div></section><section class="m-section"><h3>Production lines</h3>${lines || `<div class="m-muted">No staffed lines.</div>`}</section>`;
  }

  /** §construction: one line per world (the structure in its work slot, its
   * time and how many wait) with its yard work beneath. Bars and Cancel are
   * the world's own, under its Infrastructure tab. */
  private systemConstruction(systemId: string, dynamic: SystemStateView): string {
    const now = liveSimTime();
    const catalog = this.ctx.state.galaxy?.build_options ?? [];
    const worlds = dynamic.bodies.map((body) => {
      const line = structureQueue(dynamic, body, catalog, now);
      const yard = dynamic.builds.filter((job) => job.body_id === body.id && !isStructureJob(job.key))
        .map((job) => `<button type="button" class="m-yard-line" data-mobile-act="open-yard" data-system="${esc(systemId)}" data-kind="${esc(job.key)}">${esc(buildOption(job.key)?.label ?? human(job.key))} · ${yardStatus(job, now)}</button>`).join("");
      return `<div class="m-service-row"><span><b>${esc(body.name)}</b><small>${esc(slotLine(line[0] ?? null, line.filter((entry, index) => index > 0 && !entry.active).length))}</small>${yard}</span><div><button type="button" data-mobile-act="open-build" data-system="${esc(systemId)}" data-body="${body.id}">Structures</button>${(body.structures.shipyard ?? 0) > 0 ? `<button type="button" data-mobile-act="open-shipyard" data-system="${esc(systemId)}" data-body="${body.id}">Ships</button>` : ""}</div></div>`;
    }).join("");
    return `<section class="m-section m-section--first"><h3>Construction by world</h3>${worlds}</section>` +
      `<div class="m-action-grid"><button type="button" data-mobile-act="system-ship-production" data-id="${esc(systemId)}">Ship production to Hub</button><button type="button" data-mobile-act="open-logistics">Auto-supply rules</button></div>`;
  }

  private openPlanet(systemId: string, bodyId: number): void {
    this.hooks.deselectFleets();
    this.ctx.renderer.pulseSystemBody(String(bodyId));
    this.planetTab = "economy";
    this.hooks.openSheet({ id: "planet", props: { systemId, bodyId, detail: this.ctx.renderer.systemBodyDetail(String(bodyId)) } });
  }

  private renderPlanet(entry: SheetEntry): SheetView {
    const props = propsOf<{ systemId: string; bodyId: number; detail: SystemBodyDetail }>(entry);
    const systemId = props.systemId ?? (this.ctx.renderer.viewMode.type === "system" ? this.ctx.renderer.viewMode.systemId : undefined);
    const dynamic = systemId ? this.ctx.state.systems.find((system) => system.id === systemId) : undefined;
    const bodyId = props.bodyId ?? Number(props.detail?.id);
    const body = dynamic?.bodies.find((candidate) => candidate.id === bodyId);
    if (!systemId || !dynamic || !body) return { title: props.detail?.name ?? "World", eyebrow: "Planet panel", html: `<div class="m-empty">This body's served management data is unavailable.</div>` };
    const mine = dynamic.owner === this.ctx.state.playerId;
    const tabs = (["economy", "population", "infrastructure"] as PlanetTab[]).map((tab) => `<button type="button" data-mobile-act="planet-tab" data-tab="${tab}" aria-selected="${tab === this.planetTab}">${tab === "infrastructure" ? "Build" : human(tab)}</button>`).join("");
    const publicData = `<details class="m-details"><summary>World data</summary><div><div class="m-stat-grid"><span><small>Environment</small><b>${human(body.environment)}</b></span><span><small>Size</small><b>${human(body.size)}</b></span><span><small>Geology</small><b>${body.geology ? human(body.geology) : "Unsurveyed"}</b></span><span><small>Construction</small><b>×${body.construction_time_mult.toFixed(2)}</b></span></div>` +
      (body.special ? `<article class="m-feature-card"><small>RARE FEATURE</small><b>${human(body.special)}</b><span>${esc(body.special_effect ?? "Rare planetary feature")}</span></article>` : "") +
      `<section class="m-section"><h3>Deposits</h3>${body.deposits == null ? `<div class="m-muted">Geology unsurveyed.</div>` : body.deposits.map((deposit) => `<div class="m-order"><b>${human(deposit.resource)}</b><span>richness ${deposit.richness.toFixed(2)} · ${deposit.reserves == null ? "renewable" : deposit.reserves <= 0 ? "depleted" : `${fmt(deposit.reserves)} reserves`}</span></div>`).join("") || `<div class="m-muted">No deposits.</div>`}` +
      // §pockets: your finds, and how often you've prospected here.
      `${(body.pockets ?? []).map((pocket) => `<div class="m-order is-pocket"><b>${human(pocket.resource)} pocket</b><span>richness ${pocket.richness.toFixed(2)} · ${pocketLeft(pocket)}</span></div>`).join("")}${prospectLine(body) ? `<div class="m-muted">${esc(prospectLine(body))}</div>` : ""}</section></div></details>`;
    const beltWork = body.kind === "asteroid_belt" ? this.beltWorkHtml(dynamic, body) : "";
    const ground = this.groundAction(systemId);
    if (!mine) return { title: body.name, eyebrow: `${human(body.kind)} · observed world`, html: publicData + beltWork + ground };
    const active = this.planetTab === "economy" ? this.planetEconomy(systemId, dynamic, body) : this.planetTab === "population" ? this.planetPopulation(systemId, body) : this.planetInfrastructure(systemId, dynamic, body);
    const camp = body.founding_camp && campOpen(this.ctx.state) ? `<section class="camp-summary"><b>Founding Camp · ${body.founding_camp.completed.length}/4 developments</b><p>Housing ${Math.round((body.population_capacity ?? .003) * 1e6).toLocaleString()} · System storage ${dynamic.storage_cap}</p><button type="button" data-settlement-open data-coach="settlement-projects" data-system="${esc(systemId)}" data-body="${body.id}">Settlement projects</button></section>` : "";
    // §power: this planet's grid.
    const grid = gridLine(body);
    const gridHtml = grid ? `<div class="m-grid-line${grid.short ? " is-short" : ""}" title="${esc(grid.tip)}">${esc(grid.text)}</div>` : "";
    return { title: body.name, eyebrow: "Owned world · served management", html: `${camp}${gridHtml}<div class="m-subtabs m-subtabs--3">${tabs}</div>${active}${publicData}${beltWork}${ground}` };
  }

  /** A ground record is already player-served and fog-filtered. Surfacing it
   * here mirrors the desktop system/planet affordance without consulting the
   * authoritative fight or inventing rounds beyond the arrived prefix. */
  private groundAction(systemId: string): string {
    const records = this.ctx.state.groundRecords.filter((record) => record.system === systemId);
    const record = records.find((candidate) => candidate.outcome === null)
      ?? records.sort((a, b) => b.started_at - a.started_at)[0];
    if (!record) return "";
    const label = record.outcome === null ? "Watch landing in progress" : "Replay latest landing";
    const detail = record.fidelity === "participant" ? "Participant record" : "Observed from orbit";
    return `<section class="m-section m-ground-link"><h3>Ground action</h3><button type="button" class="m-wide-button" data-mobile-act="open-ground" data-id="${esc(record.id)}">${label}</button><small>${detail} · ${record.rounds.length} arrived round${record.rounds.length === 1 ? "" : "s"}</small></section>`;
  }

  private planetEconomy(systemId: string, dynamic: SystemStateView, body: BodyView): string {
    const assignments = dynamic.assignments.filter((line) => line.body_id === body.id);
    // §workforce cap: + only while the system has a free worker and the line has room.
    const free = dynamic.workforce ? Math.max(0, dynamic.workforce.units - dynamic.workforce.posted) : Infinity;
    const canAdd = (slug: string, workers: number) => free > 0 && workers < (body.structures[slug] ?? 0);
    const rows = assignments.map((line) => this.assignmentRow(systemId, line.body_id, line.structure, line.title, line.workers, line.specialists, line.outputs, line.suspended, canAdd(line.structure, line.workers))).join("");
    const assigned = new Set(assignments.map((line) => line.structure));
    const idle = Object.entries(body.structures).filter(([slug, tier]) => tier > 0 && PRODUCER_STRUCTURES.has(slug) && !assigned.has(slug)).map(([slug]) => this.assignmentRow(systemId, body.id, slug, human(slug), 0, {}, [], "needs_crew", canAdd(slug, 0))).join("");
    const built = Object.entries(body.structures).filter(([, tier]) => tier > 0).map(([slug, tier]) => {
      const option = buildOption(slug);
      const level = option ? structureLevel(option, tier) : undefined;
      return `<span>${structureImage(slug, tier, "sm")}${esc(level?.label ?? human(slug))}<b>Level ${tier}</b>${slug === "command_center" ? `<small>${esc(level?.description ?? "Local corporate administration. No workforce or extra slot.")}</small>` : ""}</span>`;
    }).join("");
    const recipes = body.structures.smelter > 0 ? buildOption("smelter")?.refining_recipes ?? [] : [];
    const currentOre = assignments.find(a => a.structure === "smelter")?.refining_ore ?? "metallic_ore";
    const draft = this.refiningDraft?.system === systemId && this.refiningDraft.body === body.id ? this.refiningDraft : null;
    const refining = recipes.length ? `<section class="m-section"><h3>Smelter recipe</h3><small>Current: ${human(currentOre)}</small><div class="m-chip-actions">${recipes.map(r => `<button type="button" data-mobile-act="refining-select" data-system="${esc(systemId)}" data-body="${body.id}" data-ore="${r.inputs[0][0]}" aria-pressed="${(draft?.ore ?? currentOre) === r.inputs[0][0]}">${human(r.inputs[0][0])}</button>`).join("")}</div>${draft ? `<p>${human(draft.ore)} → ${recipeOutputs(recipes.find(r => r.inputs[0][0] === draft.ore)!).map(([c]) => human(c)).join(" + ")}</p><small>Takes effect after the order arrives.</small><div class="m-chip-actions"><button type="button" data-mobile-act="refining-confirm">Confirm recipe</button><button type="button" data-mobile-act="refining-cancel">Cancel</button></div>` : ""}</section>` : "";
    const ore = draft?.ore ?? (recipes.length ? currentOre : body.deposits?.find(d => buildOption("smelter")?.refining_recipes?.some(r => r.inputs[0][0] === d.resource))?.resource);
    const recipe = buildOption("smelter")?.refining_recipes?.find(r => r.inputs[0][0] === ore);
    const comparison = refiningComparison(this.ctx.state, dynamic, body.id, recipe);
    // §fuel-overhaul: the Drive Works makes Warp Cores or Fusion Cores.
    const cores = body.structures.drive_works > 0 ? buildOption("drive_works")?.refining_recipes ?? [] : [];
    const currentCore = assignments.find(a => a.structure === "drive_works")?.refining_ore ?? "warp_core";
    const coreDraft = this.coreDraft?.system === systemId && this.coreDraft.body === body.id ? this.coreDraft : null;
    const product = cores.length ? `<section class="m-section"><h3>Drive Works product</h3><small>Current: ${human(currentCore)}</small><div class="m-chip-actions">${cores.map(r => `<button type="button" data-mobile-act="refining-core-select" data-system="${esc(systemId)}" data-body="${body.id}" data-product="${r.output}" aria-pressed="${(coreDraft?.product ?? currentCore) === r.output}">${human(r.output)}</button>`).join("")}</div>${coreDraft ? `<small>Takes effect after the order arrives.</small><div class="m-chip-actions"><button type="button" data-mobile-act="refining-core-confirm">Confirm product</button><button type="button" data-mobile-act="refining-cancel">Cancel</button></div>` : ""}</section>` : "";
    return `<section class="m-section m-section--first"><h3>Built here</h3><div class="m-ledger">${built || `<span>Undeveloped</span>`}</div></section>${this.crownsHtml(systemId, body)}${refining}${comparison}${product}<section class="m-section"><h3>${body.belt ? "Production" : "Workforce assignments"}</h3>${rows + idle || `<div class="m-muted">No production structures.</div>`}</section>${(body.structures.academy ?? 0) > 0 ? this.coursesHtml(systemId, dynamic) : ""}`;
  }

  private assignmentRow(systemId: string, bodyId: number, structure: string, title: string, workers: number, specialists: Record<string, number>, outputs: [Commodity, number][], suspended: string | null, canAdd = true): string {
    const output = outputs.map(([commodity, rate]) => `+${rate.toFixed(2)} ${human(commodity)}/s`).join(" · ") || "idle";
    // §asteroid-quarry: a quarry has no crew to post; it eats its upkeep.
    if (!WORKFORCE_STRUCTURES.has(structure)) {
      const tier = this.ctx.state.systems.find((candidate) => candidate.id === systemId)?.bodies.find((body) => body.id === bodyId)?.structures[structure] ?? 0;
      const upkeep = (buildOption(structure)?.upkeep ?? []).map(([commodity, rate]) => `−${Number((rate * tier * 60).toFixed(2))} ${human(commodity)}`).join(" · ");
      return `<div class="m-assignment"><span><b>${esc(title)}</b><small>${upkeep ? `${esc(upkeep)} / min` : "No crew"}${suspended ? ` · ${human(suspended)}` : ""}</small><em>${esc(output)}</em></span></div>`;
    }
    const specialistsText = Object.entries(specialists).filter(([, n]) => n > 0).map(([kind, n]) => `${n} ${human(kind)}`).join(" · ");
    return `<div class="m-assignment"><span><b>${esc(title)}</b><small>${workers} workforce${specialistsText ? ` · ${esc(specialistsText)}` : ""}${suspended ? ` · ${human(suspended)}` : ""}</small><em>${esc(output)}</em></span><div><button type="button" data-mobile-act="worker-set" data-system="${esc(systemId)}" data-body="${bodyId}" data-structure="${esc(structure)}" data-workers="${Math.max(0, workers - 1)}" ${workers <= 0 ? "disabled" : ""}>−</button><button type="button" data-mobile-act="worker-set" data-system="${esc(systemId)}" data-body="${bodyId}" data-structure="${esc(structure)}" data-workers="${workers + 1}" ${canAdd ? "" : "disabled"}>+</button></div></div>${this.specialistChips(systemId, bodyId, structure, specialists)}`;
  }

  /** §research-wiring Talent: one chip per profession the system holds, to
   * post (+) or recall (−) one on this line (★ where its skill serves it). */
  private specialistChips(systemId: string, bodyId: number, structure: string, posted: Record<string, number>): string {
    const dynamic = this.ctx.state.systems.find((candidate) => candidate.id === systemId);
    if (!dynamic) return "";
    const kinds = [...new Set([...Object.keys(dynamic.specialists ?? {}).filter(kind => (dynamic.specialists[kind] ?? 0) > 0), ...Object.keys(posted)])];
    if (!kinds.length) return "";
    return `<div class="m-chip-actions">${kinds.map(kind => {
      const n = posted[kind] ?? 0;
      const free = freeSpecialists(dynamic, kind, bodyId, structure);
      const name = `${human(kind)}${specialistAffine(kind, structure) ? " ★" : ""}`;
      return `<button type="button" data-mobile-act="specialist-set" data-system="${esc(systemId)}" data-body="${bodyId}" data-structure="${esc(structure)}" data-kind="${esc(kind)}" data-n="${n - 1}" ${n <= 0 ? "disabled" : ""}>− ${esc(name)} ${n}</button><button type="button" data-mobile-act="specialist-set" data-system="${esc(systemId)}" data-body="${bodyId}" data-structure="${esc(structure)}" data-kind="${esc(kind)}" data-n="${n + 1}" ${n >= free ? "disabled" : ""}>+</button>`;
    }).join("")}</div>`;
  }

  private setSpecialist(button: HTMLElement): void {
    const system = button.dataset.system;
    const body = Number(button.dataset.body);
    const structure = button.dataset.structure;
    const kind = button.dataset.kind;
    const dynamic = system ? this.ctx.state.systems.find((candidate) => candidate.id === system) : undefined;
    const line = dynamic?.assignments.find((candidate) => candidate.body_id === body && candidate.structure === structure);
    if (!system || !structure || !kind || !line || !Number.isFinite(body)) return;
    const specialists = { ...line.specialists, [kind]: Math.max(0, Number(button.dataset.n) || 0) };
    if (!specialists[kind]) delete specialists[kind];
    this.ctx.send({ type: "SetAssignment", system_id: system, structure, workers: line.workers, specialists, body_id: body });
  }

  /** §research-wiring Talent: the Academy's courses, and raising experts. */
  private coursesHtml(systemId: string, dynamic: SystemStateView): string {
    const expert = researched("life_talent_iii_expert_training");
    const slots = researched("life_talent_iv_broad_curricula") ? 2 : 1;
    const train = SPECIALIST_KINDS.map(kind => `<button type="button" data-mobile-act="train" data-system="${esc(systemId)}" data-kind="${kind}">${esc(human(kind))}</button>`).join("");
    const raise = expert ? SPECIALIST_KINDS.filter(kind => (dynamic.specialists?.[kind] ?? 0) > 0)
      .map(kind => `<button type="button" data-mobile-act="train" data-system="${esc(systemId)}" data-kind="expert_${kind}">${esc(human(`expert_${kind}`))}</button>`).join("") : "";
    const courses = dynamic.builds.filter(job => SPECIALIST_KINDS.some(kind => job.key === kind || job.key === `expert_${kind}`))
      .map(job => `<div class="m-order"><span><b>${esc(human(job.key))}</b></span><span>${job.queued ? "waiting" : yardStatus(job, liveSimTime())}</span></div>`).join("");
    return `<section class="m-section"><h3>Academy courses · ${slots} at a time</h3><small>${COURSE.base.costs} · ${COURSE.base.seconds} s · housed ${dynamic.specialist_housing ?? 0}</small><div class="m-chip-actions">${train}</div>${raise ? `<small>Raise to expert · ${COURSE.expert.costs}</small><div class="m-chip-actions">${raise}</div>` : ""}${courses}</section>`;
  }

  /** §designations: Crown Vein / Crown Project on this world. */
  private crownsHtml(systemId: string, body: BodyView): string {
    if (body.belt) return "";
    const crowns = ([["crown_vein", "Crown Vein", "mat_deepcrust_v_crown_vein"], ["crown_project", "Crown Project", "mat_foundry_v_crown_project"]] as const)
      .filter(([, , programme]) => researched(programme));
    if (!crowns.length) return "";
    return `<section class="m-section"><h3>Crowns</h3><div class="m-chip-actions">${crowns.map(([cap, title]) => {
      const current = this.ctx.state.research?.designations?.find(d => d.cap === cap);
      const here = !!current?.live && "body" in current.target && current.target.body.system === systemId && current.target.body.body === body.id;
      const locked = (current?.locked_until ?? 0) > liveSimTime();
      return here ? `<span>♛ ${esc(title)}</span>` : `<button type="button" data-mobile-act="crown" data-system="${esc(systemId)}" data-body="${body.id}" data-cap="${cap}" ${locked ? "disabled" : ""}>♛ ${esc(title)}</button>`;
    }).join("")}</div></section>`;
  }

  private planetPopulation(systemId: string, body: BodyView): string {
    const policies: MigrationPolicy[] = ["closed", "managed", "open", "priority"];
    const cohort = this.ctx.state.galaxy?.migrant_cohort_people ?? 1_000;
    const destinations = this.ctx.state.systems.filter((system) => system.owner === this.ctx.state.playerId && !system.blockade && system.habitat_fed).flatMap((system) => system.bodies.filter((candidate) => !(system.id === systemId && candidate.id === body.id) && candidate.population > 0).map((candidate) => option(`${system.id}|${candidate.id}`, `${systemName(system.id)} · ${candidate.name}`))).join("");
    return `<div class="m-stat-grid"><span><small>Population</small><b>${fmtPopulation(body.population)}</b></span><span><small>Inbound</small><b>${fmt(body.inbound_migrants)} people</b></span><span><small>Habitat appeal</small><b>×${body.population_growth_mult.toFixed(2)}</b></span><span><small>Food use</small><b>×${body.provisions_mult.toFixed(2)}</b></span></div>` +
      `<section class="m-section"><h3>Immigration policy</h3><div class="m-chip-actions">${policies.map((policy) => `<button type="button" data-mobile-act="migration-policy" data-system="${esc(systemId)}" data-body="${body.id}" data-policy="${policy}" aria-pressed="${body.migration_policy === policy}">${human(policy)}</button>`).join("")}</div></section>` +
      `<section class="m-trade-card"><h3>Internal relocation · ${cohort.toLocaleString()} people</h3><label>Destination<select id="m-relocate-${safeId(systemId)}-${body.id}">${destinations}</select></label><button type="button" data-mobile-act="migration-relocate" data-system="${esc(systemId)}" data-body="${body.id}" ${destinations && body.population * 1_000_000 >= cohort * 2 ? "" : "disabled"}>Dispatch migrant liner</button></section>`;
  }

  private planetInfrastructure(systemId: string, dynamic: SystemStateView, body: BodyView): string {
    const pools = bodyPoolUsage(body, dynamic);
    const now = liveSimTime();
    const line = structureQueue(dynamic, body, this.ctx.state.galaxy?.build_options ?? [], now);
    // §construction: this world's work slot in order; only the order building
    // now has a bar and a time. Hulls and modules follow on their yards.
    const queue = line.map((job) => job.position === 1
      ? `<div class="m-order m-order--head"><span><b>${esc(jobTitle(job))}</b>${job.active && job.progress !== null ? `<div class="m-progress"${liveProgressAttr(job.line)}><i style="width:${job.progress.toFixed(1)}%"></i></div>` : ""}</span><span>${!job.active ? "Starting…" : job.readyIn === null ? "—" : job.readyIn <= 0.5 ? "completing" : fmtBuildDur(job.readyIn)}</span>${this.cancelControl(systemId, job)}</div>`
      : `<div class="m-order"><span><b>#${job.position} ${esc(jobTitle(job))}</b></span><span>${job.startsIn === null ? "after the jobs ahead" : job.startsIn > 0.5 ? `starts in ${fmtBuildDur(job.startsIn)}` : "starts next"}</span>${this.cancelControl(systemId, job)}</div>`).join("")
      + dynamic.builds.filter((job) => job.body_id === body.id && !isStructureJob(job.key))
        .map((job) => `<button type="button" class="m-order m-order--hull" data-mobile-act="open-yard" data-system="${esc(systemId)}" data-kind="${esc(job.key)}"><span><b>${esc(buildOption(job.key)?.label ?? human(job.key))}</b></span><span>${yardStatus(job, now)}</span></button>`).join("");
    const modules = (body.structures.armaments_complex ?? 0) > 0 || (body.structures.shipyard ?? 0) > 0 ? `<section class="m-section"><h3>Equipment</h3>${MODULES.filter((module) => moduleResearched(module.kind)).map((module) => {
      const recipe = buildOption(`module:${module.kind}`);
      const reason = moduleBuildReason(dynamic, module.kind);
      return `<div class="m-service-row"><span><b>${module.name}</b><small>${esc(module.role)}</small><small>Stock ${dynamic.modules?.[module.kind] ?? 0} · ${recipe?.costs.map((cost) => `${cost.units} ${human(cost.commodity)}`).join(" + ") ?? "recipe unavailable"}${reason ? ` · ${reason}` : ""}</small></span><button type="button" data-mobile-act="module-build" data-system="${esc(systemId)}" data-module="${module.kind}" ${reason ? "disabled" : ""}>Build</button></div>`;
    }).join("")}</section>` : "";
    return `<section class="m-section m-section--first"><h3>Body slot pools</h3><div class="m-ledger">${(["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span>${POOL_LABEL[pool]}<b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("")}</div>${this.developmentPoolHelp()}</section>` +
      `<div class="m-action-grid"><button type="button" class="m-primary" data-mobile-act="open-build" data-system="${esc(systemId)}" data-body="${body.id}">Build structure</button>${(body.structures.shipyard ?? 0) > 0 ? `<button type="button" data-mobile-act="open-shipyard" data-system="${esc(systemId)}" data-body="${body.id}">Build ship</button>` : ""}</div>` +
      `<section class="m-section"><h3>Construction</h3>${queue || `<div class="m-muted">Build slot free</div>`}</section>${modules}`;
  }

  /** §designations Mass Streams: the stream through this system, or where it could run. */
  private streamHtml(systemId: string): string {
    if (!researched("prop_line_mass_streams")) return "";
    const current = this.ctx.state.research?.designations?.find(d => d.cap === "mass_streams");
    const pair = current && "system_pair" in current.target ? current.target.system_pair : null;
    const locked = (current?.locked_until ?? 0) > liveSimTime();
    const status = pair ? `<p>${esc(systemName(pair[0]))} ⇄ ${esc(systemName(pair[1]))}${current?.live ? "" : " · broken"}</p>` : "";
    const options = pair?.includes(systemId) || locked ? "" : this.ctx.state.systems
      .filter(s => s.owner === this.ctx.state.playerId && s.id !== systemId)
      .map(s => `<button type="button" data-mobile-act="stream" data-system="${esc(systemId)}" data-to="${esc(s.id)}">⇄ ${esc(systemName(s.id))}</button>`).join("");
    return `<section class="m-section"><h3>Mass Stream</h3>${status}${options ? `<div class="m-chip-actions">${options}</div>` : ""}</section>`;
  }

  private developmentPoolHelp(): string {
    return `<details class="m-details m-help"><summary>How development pools work</summary><div><p>Each pool belongs to one world; system totals add those worlds together. The first tier of a distinct structure uses one slot from its matching pool. Upgrading that same structure uses no additional slot. Ships use shipyard capacity, not these pools.</p></div></details>`;
  }

  private setWorkers(button: HTMLElement): void {
    const system = button.dataset.system;
    const body = Number(button.dataset.body);
    const structure = button.dataset.structure;
    const workers = Math.max(0, Number(button.dataset.workers) || 0);
    const dynamic = system ? this.ctx.state.systems.find((candidate) => candidate.id === system) : undefined;
    const line = dynamic?.assignments.find((candidate) => candidate.body_id === body && candidate.structure === structure);
    if (system && structure && Number.isFinite(body)) this.ctx.send({ type: "SetAssignment", system_id: system, structure, workers, specialists: line?.specialists ?? {}, body_id: body });
  }

  private relocateMigrants(button: HTMLElement): void {
    const fromSystem = button.dataset.system;
    const fromBody = Number(button.dataset.body);
    const value = fromSystem ? element<HTMLSelectElement>(`m-relocate-${safeId(fromSystem)}-${fromBody}`)?.value : "";
    const [toSystem, toBodyText] = value?.split("|") ?? [];
    const toBody = Number(toBodyText);
    if (fromSystem && Number.isFinite(fromBody) && toSystem && Number.isFinite(toBody)) this.ctx.send({ type: "RelocateMigrants", from_system: fromSystem, from_body: fromBody, to_system: toSystem, to_body: toBody });
  }

  private renderBuild(entry: SheetEntry): SheetView {
    const { systemId, dynamic, body } = this.buildContext(entry);
    if (!systemId || !dynamic || !body) return { title: "Build", eyebrow: "Structure construction", html: `<div class="m-empty">Build context is unavailable.</div>` };
    const pools = bodyPoolUsage(body, dynamic);
    const options = (this.ctx.state.galaxy?.build_options ?? []).filter((candidate) => !SHIP_KEYS.has(candidate.key)
      && !candidate.key.startsWith("module:") && !!POOL_OF[candidate.key] && structureUnlocked(candidate)
      && !structOption(candidate as BuildOpt, dynamic, body, pools).capped) as BuildOpt[];
    if (this.selectedBuild && !options.some((candidate) => candidate.key === this.selectedBuild)) this.selectedBuild = "";
    const rows = options.map((candidate) => {
      const state = structOption(candidate, dynamic, body, pools);
      return `<button type="button" class="m-build-row${candidate.key === this.selectedBuild ? " is-active" : ""}" data-mobile-act="build-select" data-key="${esc(candidate.key)}"><span class="m-build-row__identity">${structureImage(candidate.key, state.targetTier, "sm", undefined, "m-structure-icon")}<span><b>${esc(state.o.label)}</b><small>${POOL_LABEL[state.pool]} · ${state.tierUp ? `Tier ${state.currentTier} → ${state.targetTier}` : "new Tier I"}</small></span></span><em>${state.buildable ? fmtEta(structureBuildSeconds(state.o, body, openingBuild(candidate.key, state.targetTier, systemId, body))) : esc(state.reason || "unavailable")}</em></button>`;
    }).join("");
    const selected = options.find((candidate) => candidate.key === this.selectedBuild);
    const detail = selected ? this.structureDetail(systemId, dynamic, body, selected, pools) : `<div class="m-empty">Choose a structure to inspect its recipe and queue it.</div>`;
    return { title: `Build on ${body.name}`, eyebrow: "Structure construction · local supply", html: `<div class="m-ledger m-ledger--pools">${(["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span>${POOL_LABEL[pool]}<b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("")}</div><div class="m-build-list">${rows}</div>${detail}` };
  }

  private structureDetail(systemId: string, dynamic: SystemStateView, body: BodyView, build: BuildOpt, pools: ReturnType<typeof bodyPoolUsage>): string {
    const state = structOption(build, dynamic, body, pools);
    build = state.o;
    if (state.maxed) return `<article class="m-build-detail"><header>${structureImage(build.key, state.targetTier, "md", undefined, "m-structure-icon")}<span><small>Maximum level built or queued</small><h3>${esc(build.label)}</h3></span></header><p>${esc(build.description ?? "")}</p><div class="m-warning">${esc(state.reason)}</div></article>`;
    const attachment = build.key === "service_berth";
    const heading = attachment ? "Warehouse attachment" : `${POOL_LABEL[state.pool]} · ${state.tierUp ? `upgrade to Tier ${state.targetTier}` : "new structure"}`;
    const slot = build.key === "command_center" ? " Dedicated civic footprint; no extra slot." : build.key === "power_plant" ? " Own slot; one per planet." : build.key === "vault" ? " Under the Command Center; no slot." : build.key === "assembly_hall" ? " At the Command Center; no slot." : attachment ? " Attached to Warehouse; no extra slot." : state.foundsNew ? ` Claims one ${POOL_LABEL[state.pool].toLowerCase()} slot.` : " Deepens in place without another slot.";
    const supply = constructionStock(dynamic).available;
    const costs = build.costs.map((cost) => `<div class="m-order"><b>${human(cost.commodity)}</b><span>need ${cost.units} · have ${fmt(supply.get(cost.commodity as Commodity) ?? 0)}</span></div>`).join("");
    // §construction: an order already on this world holds the button's spot,
    // with its Cancel; the next tier is a deliberate, secondary choice.
    const latest = structureQueue(dynamic, body, this.ctx.state.galaxy?.build_options ?? [], liveSimTime())
      .filter((entry) => entry.key === build.key).at(-1);
    const queueButton = `data-mobile-act="build-queue" data-system="${esc(systemId)}" data-body="${body.id}" data-key="${esc(build.key)}"`;
    const action = latest
      ? `<div class="m-order"><b>Tier ${latest.tier} · ${jobTag(latest)}</b>${this.cancelControl(systemId, latest)}</div>${state.buildable ? `<button type="button" class="m-also" ${queueButton}>Also queue Tier ${state.targetTier}</button>` : ""}`
      : `<button type="button" class="m-primary" ${queueButton} data-coach="queue-build" ${state.buildable ? "" : "disabled"}>Queue build</button>`;
    return `<article class="m-build-detail"><header>${structureImage(build.key, state.targetTier, "md", undefined, "m-structure-icon")}<span><small>${heading}</small><h3>${esc(build.label)}</h3></span></header>${build.description ? `<p>${esc(build.description)}</p>` : ""}${build.conversion ? `<p>${esc(conversionSummary(build))}</p>` : ""}<div>${costs}</div><p>${fmtEta(structureBuildSeconds(build, body, openingBuild(build.key, state.targetTier, systemId, body)))} on this world.${slot}</p>${state.reason ? `<div class="m-warning">${esc(state.reason)}</div>` : ""}${action}</article>`;
  }

  /** §construction: Cancel for a structure order, waiting or building, behind a Confirm. */
  private cancelControl(systemId: string, job: StructureJob): string {
    if (job.job.id === undefined) return "";
    if (this.cancelSent.has(job.job.id)) return `<small>Cancel sent</small>`;
    if (!job.cancellable) return "";
    return this.cancelArmed === job.job.id
      ? `<span class="m-chip-actions"><button type="button" class="m-danger" data-mobile-act="build-cancel-confirm" data-system="${esc(systemId)}" data-job="${job.job.id}">Confirm cancel</button><button type="button" data-mobile-act="build-cancel-keep">Keep</button></span>`
      : `<button type="button" data-mobile-act="build-cancel" data-job="${job.job.id}">Cancel</button>`;
  }

  private queueStructure(button: HTMLElement): void {
    const system = button.dataset.system;
    const body = Number(button.dataset.body);
    const key = button.dataset.key;
    if (system && key && Number.isFinite(body)) {
      this.ctx.send({ type: "DevelopSystem", system_id: system, upgrade: key, body_id: body });
      this.hooks.notice(`${human(key)} queued at ${systemName(system)}.`);
      // Queued: the picker closes (Peter, Oct 20 2026); back to the world.
      this.sheets.pop();
    }
  }

  private renderShipyard(entry: SheetEntry): SheetView {
    const { systemId, dynamic, body } = this.buildContext(entry);
    if (!systemId || !dynamic || !body) return { title: "Shipyard", eyebrow: "Hull construction", html: `<div class="m-empty">Shipyard context is unavailable.</div>` };
    const options = SHIP_ORDER.filter(hullResearched).map((kind) => this.ctx.state.galaxy?.build_options.find((candidate) => candidate.key === kind)).filter((candidate) => candidate !== undefined);
    if (this.selectedHull && !options.some((candidate) => candidate.key === this.selectedHull)) this.selectedHull = "";
    const rows = options.map((candidate) => {
      const state = shipOption(candidate, dynamic);
      return `<button type="button" class="m-build-row${candidate.key === this.selectedHull ? " is-active" : ""}" data-mobile-act="ship-select" data-kind="${candidate.key}"><img class="m-ship-hull" src="/art/derived/ships/models-v1/panels/${candidate.key}-128.png" alt="" width="48" height="48" decoding="async"><span><b>${esc(candidate.label)}</b><small>${YARD_TITLE[SHIP_YARD[candidate.key]?.yard] ?? "yard"} · ${state.buildRate > 0 ? fmtEta(candidate.build_secs * (state.yardBody?.ship_build_time_mult ?? 1) / state.buildRate) : "needs workforce"}</small></span><em>${state.buildable ? `max ${state.maxAff}` : esc(state.reason)}</em></button>`;
    }).join("");
    const selected = options.find((candidate) => candidate.key === this.selectedHull);
    const detail = selected ? this.shipDetail(systemId, dynamic, body, selected) : `<div class="m-empty">Choose a hull to inspect its recipe, fitting and batch controls.</div>`;
    const inYard = yardJobs(dynamic, liveSimTime());
    const peek = inYard.length ? `<button type="button" class="m-yard-peek" data-mobile-act="open-yard" data-system="${esc(systemId)}"><b>In the yard</b><span>${inYard.slice(0, 2).map((job) => esc(`${job.title} ${job.progress !== null ? `${Math.round(job.progress * 100)}%` : job.status}`)).join(" · ")}${inYard.length > 2 ? ` · +${inYard.length - 2}` : ""}</span></button>` : "";
    return { title: `Shipyard · ${body.name}`, eyebrow: "Hull construction · local supply", html: `${peek}<div class="m-build-list">${rows}</div>${detail}` };
  }

  /** §shipyard-view: a hull under construction in its yard's interior, as
   * the Deck's Shipyard pop-out shows it. */
  private renderYard(entry: SheetEntry): SheetView {
    const { systemId = "", kind, sent } = propsOf<{ systemId: string; kind?: string; sent?: number }>(entry);
    const dynamic = this.ctx.state.systems.find((system) => system.id === systemId);
    const now = liveSimTime();
    const jobs = yardJobs(dynamic, now);
    const ordered = kind && HULL_YARD[kind as ShipKind] ? kind as ShipKind : null;
    // As the Deck's pop-out: the hull it opened on keeps the bay unless a tab
    // is picked; an order on its way shows as a blueprint; seen building and
    // then gone, it launched.
    const picked = jobs.find((entry) => entry.key === this.yardPicked) ?? null;
    const watch = ordered ? `${systemId}|${ordered}|${sent ?? ""}` : null;
    const inYard = jobs.some((entry) => entry.kind === ordered);
    if (watch && inYard) this.yardSeen.add(watch);
    const pos = this.ctx.state.galaxy?.systems.find((system) => system.id === systemId)?.pos;
    const fallback = !picked && watch && !inYard ? yardFallback(this.yardSeen.has(watch), sent ?? null, pos ? commandDelayTo(pos) ?? 0 : 0, now) : null;
    const job = picked ?? (fallback ? null : pickYardJob(jobs, null, ordered));
    const yardBody = dynamic?.bodies.find((body) => (body.structures.shipyard ?? 0) > 0);
    const build = yardBody ? `<div class="m-action-grid"><button type="button" data-mobile-act="open-shipyard" data-system="${esc(systemId)}" data-body="${yardBody.id}">Build ships</button></div>` : "";
    if (!job && !fallback) return { title: "Shipyard", eyebrow: systemName(systemId), html: `<div class="m-empty">No hulls in the yard.</div>${build}` };
    const hull = job?.kind ?? ordered!;
    const progress = job ? job.progress : fallback?.state === "launched" ? 1 : null;
    const tab = (entry: YardJob) => `<button type="button" data-mobile-act="yard-pick" data-job="${esc(entry.key)}" aria-pressed="${entry === job}"><b>${esc(entry.title)}</b><small>${esc(entry.progress !== null ? `${Math.round(entry.progress * 100)}%` : entry.status)}</small></button>`;
    const tabs = jobs.length > 1 ? `<div class="m-yard-tabs">${jobs.map(tab).join("")}</div>` : "";
    const bar = job?.progress == null ? "" : `<div class="m-progress"${liveProgressAttr(job.line)}><i style="width:${(job.progress * 100).toFixed(1)}%"></i></div>`;
    const yard = job?.yard ?? HULL_YARD[hull] ?? "shipyard";
    const view = `<ship-construction-view id="m-yard-view" kind="${esc(hull)}" yard="${yard}"${progress === null ? "" : ` progress="${progress.toFixed(4)}"`}></ship-construction-view>`;
    const note = fallback?.state === "silent"
      ? `<small class="is-warn">No report yet · it may have been refused</small>`
      : `<small>${esc(job?.where ?? YARD_TITLE[yard] ?? "Shipyard")}</small>`;
    const status = job ? job.status : fallback?.state === "launched" ? "Launched" : fallback?.state === "sending" ? SENDING : "";
    const info = `<div class="m-yard-info"><span><b>${esc(job?.title ?? shipKindLabel(hull))}</b>${note}</span><em>${esc(status)}</em></div>`;
    return { title: "Shipyard", eyebrow: systemName(systemId), html: `${tabs}${view}${info}${bar}${hullStatsHtml(hull)}${build}` };
  }

  private shipDetail(systemId: string, dynamic: SystemStateView, body: BodyView, build: BuildOpt): string {
    const hull = build.key as ShipKind;
    const state = shipOption(build, dynamic);
    const supply = constructionStock(dynamic).available;
    const ledger = moduleLedgerAt(systemId);
    const slots = MODULE_SLOTS[hull] ?? 0;
    const effectiveFit = this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0 && moduleFitsHull(module, hull)).slice(0, slots);
    const fitOkay = !effectiveFit.length || fitLegal(hull, effectiveFit);
    const costs = build.costs.map((cost) => `<div class="m-order"><b>${human(cost.commodity)}</b><span>${cost.units} each · have ${fmt(supply.get(cost.commodity as Commodity) ?? 0)}</span></div>`).join("");
    const modules = slots ? MODULES.filter((module) => (ledger[module.kind] ?? 0) > 0 && moduleFitsHull(module.kind, hull)).map((module) => `<button type="button" data-mobile-act="ship-fit" data-module="${module.kind}" aria-pressed="${this.pendingFit.includes(module.kind)}">${esc(module.name)} · ${ledger[module.kind]}</button>`).join("") : "";
    const fits = (this.ctx.state.syndicate?.fits ?? []).filter((fit) => fit.kind === hull).map((fit) => `<span class="m-saved-fit"><button type="button" data-mobile-act="ship-fit-pick" data-name="${esc(fit.name)}">${esc(fit.name)}</button><button type="button" data-mobile-act="ship-fit-delete" data-name="${esc(fit.name)}">×</button></span>`).join("");
    return `<article id="m-ship-detail" class="m-build-detail"><small>${human(SHIP_YARD[hull]?.yard ?? "shipyard")} · ${slots} module slots · ${FITTING_POINTS[hull] ?? 0} fit points</small><h3>${esc(build.label)}</h3><ship-model-preview kind="${hull}"></ship-model-preview>${hullStatsHtml(hull)}${costs}` +
      `<label>Quantity<input id="m-ship-qty" type="number" min="1" max="${Math.max(1, state.maxAff)}" inputmode="numeric" value="1"></label>` +
      (slots ? `<section class="m-section"><h3>Fit next build</h3><div class="m-chip-actions">${modules || `<span class="m-muted">No modules in this system ledger.</span>`}</div>${!fitOkay ? `<div class="m-warning">This fit exceeds the hull budget.</div>` : ""}</section>` : "") +
      (this.ctx.state.syndicate ? `<section class="m-section"><h3>Syndicate fits</h3><div class="m-saved-fits">${fits || `<span class="m-muted">No saved fits for this hull.</span>`}</div><div class="m-inline-form"><input id="m-fit-name" maxlength="24" placeholder="Fit name"><button type="button" data-mobile-act="ship-fit-save" ${effectiveFit.length && fitOkay ? "" : "disabled"}>Save fit</button></div></section>` : "") +
      (state.reason ? `<div class="m-warning">${esc(state.reason)}</div>` : "") +
      `<button type="button" class="m-primary" data-mobile-act="ship-queue" data-system="${esc(systemId)}" data-body="${body.id}" ${state.buildable && fitOkay ? "" : "disabled"}>Queue hulls</button></article>`;
  }

  private saveFit(): void {
    if (!this.selectedHull) return;
    const name = element<HTMLInputElement>("m-fit-name")?.value.trim();
    if (name && this.pendingFit.length && fitLegal(this.selectedHull, this.pendingFit)) this.ctx.send({ type: "SaveFit", name, ship: this.selectedHull, loadout: [...this.pendingFit] });
  }

  private queueShips(button: HTMLElement): void {
    const system = button.dataset.system;
    if (!system || !this.selectedHull) return;
    const dynamic = this.ctx.state.systems.find((candidate) => candidate.id === system);
    const build = buildOption(this.selectedHull);
    if (!dynamic || !build) return;
    const state = shipOption(build, dynamic);
    const quantity = Math.min(state.maxAff, Math.max(1, Math.floor(Number(element<HTMLInputElement>("m-ship-qty")?.value) || 1)));
    const ledger = moduleLedgerAt(system);
    const fit = this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0 && moduleFitsHull(module, this.selectedHull)).slice(0, MODULE_SLOTS[this.selectedHull] ?? 0);
    if (!state.buildable || quantity < 1 || (fit.length > 0 && !fitLegal(this.selectedHull, fit))) return;
    for (let i = 0; i < quantity; i++) this.ctx.send({ type: "BuildShip", system_id: system, ship_kind: this.selectedHull, loadout: fit.length ? fit : undefined });
    this.hooks.notice(`${quantity}× ${shipKindLabel(this.selectedHull)} queued.`);
    // §shipyard-view: the hull on order appears in its yard at once.
    this.yardPicked = null;
    this.hooks.openSheet({ id: "yard", props: { systemId: system, kind: this.selectedHull, sent: liveSimTime() } });
  }

  private buildContext(entry: SheetEntry): { systemId?: string; bodyId?: number; dynamic?: SystemStateView; body?: BodyView } {
    const props = propsOf<{ systemId: string; bodyId: number }>(entry);
    const dynamic = props.systemId ? this.ctx.state.systems.find((system) => system.id === props.systemId) : undefined;
    const body = dynamic?.bodies.find((candidate) => candidate.id === props.bodyId);
    return { systemId: props.systemId, bodyId: props.bodyId, dynamic, body };
  }

  private renderHub(): SheetView {
    const docked = this.ctx.state.ghosts.filter((fleet) => fleet.own && fleet.docked === "hub");
    const rows = docked.map((fleet) => `<button type="button" class="m-list-row" data-mobile-act="hub-fleet" data-id="${esc(fleet.id)}"><span class="m-list-row__icon">△</span><span class="m-list-row__main"><b>${esc(shipKindLabel(fleet.kind))} fleet</b><small>docked · ${fleet.composition?.reduce((sum, stack) => sum + stack.count, 0) ?? "served count"} ships</small></span><span class="m-list-row__meta"><em>berthed</em></span></button>`).join("");
    return {
      title: "Wormhole Hub",
      eyebrow: "Market gateway · unlimited corporate berths",
      html: `<article class="m-feature-card"><small>GLOBAL MARKET ACCESS</small><b>Wormhole Hub</b><span>Exchange orders, warehouse holdings, Authority freight, specialists and modules.</span></article>` +
        `<button type="button" class="m-wide-button m-primary" data-mobile-act="hub-market">Open Market panel</button>` +
        `<section class="m-section"><h3>Your docked fleets · ${docked.length}</h3><div class="m-list">${rows || `<div class="m-muted">No owned fleet is docked at the Hub.</div>`}</div></section>`,
    };
  }

  private renderLogistics(): SheetView {
    const industry = industryHtml(this.ctx.state);
    if (!industryStandingVisible(this.ctx.state)) return { title: "Logistics", html: industry };
    const sources = ownedSystems();
    const destinations = `<option value="hub">Market Hub</option><option value="home">Home system</option>` + sources.map((system) => option(system.id, system.name)).join("") + allySystems().map((system) => option(system.id, `${system.name} · ally`)).join("");
    const commodities = COMMODITIES.map((commodity) => option(commodity, human(commodity))).join("");
    const orders = this.ctx.state.standingOrders.map((order) => `<div class="m-standing-row"><span><b>#${order.id} · ${human(order.commodity)}</b><small>${endpointText(order.source)} → ${endpointText(order.dest)} · ${triggerText(order.trigger)} · ${order.in_flight ? "freighter en route" : order.status}</small></span><button type="button" data-mobile-act="logistics-clear" data-id="${order.id}">×</button></div>`).join("");
    return {
      title: "Auto-supply",
      eyebrow: "Standing logistics · runs while away",
      html: industry + `<section class="m-section m-section--first"><h3>Standing orders</h3>${orders || `<div class="m-muted">No standing orders.</div>`}</section>` +
        `<section class="m-trade-card"><h3>New rule</h3><label>Source<select id="m-log-source">${sources.map((system) => option(system.id, system.name)).join("")}</select></label><label>Destination<select id="m-log-dest">${destinations}</select></label><label>Commodity<select id="m-log-commodity">${commodities}</select></label><label>Trigger<select id="m-log-trigger"><option value="above_threshold">Above source threshold</option><option value="percent_surplus">Percent surplus</option><option value="maintain_at_dest">Maintain at destination</option></select></label><label>Amount<input id="m-log-amount" type="number" min="0" value="100"></label><label>Surplus floor<input id="m-log-floor" type="number" min="0" value="50"></label><label class="m-check"><input id="m-log-sell" type="checkbox" checked> Sell on Hub arrival</label><button type="button" class="m-primary" data-mobile-act="logistics-add" ${sources.length ? "" : "disabled"}>Add standing order</button></section>`,
    };
  }

  private addStandingOrder(): void {
    const source = element<HTMLSelectElement>("m-log-source")?.value;
    const destination = element<HTMLSelectElement>("m-log-dest")?.value;
    const commodity = element<HTMLSelectElement>("m-log-commodity")?.value as Commodity | undefined;
    const kind = element<HTMLSelectElement>("m-log-trigger")?.value;
    const amount = Math.max(0, Number(element<HTMLInputElement>("m-log-amount")?.value) || 0);
    const floor = Math.max(0, Number(element<HTMLInputElement>("m-log-floor")?.value) || 0);
    if (!source || !destination || !commodity) return;
    const dest: StandingEndpoint = destination === "hub" ? { kind: "hub" } : destination === "home" ? { kind: "home" } : { kind: "system", id: destination };
    const trigger: StandingTrigger = kind === "percent_surplus" ? { kind: "percent_surplus", percent: Math.max(1, Math.min(100, Math.round(amount))), floor } : kind === "maintain_at_dest" ? { kind: "maintain_at_dest", target: amount } : { kind: "above_threshold", threshold: amount };
    const order: StandingOrder = { id: 0, source: { kind: "system", id: source }, dest, commodity, trigger, status: "active", next_eval_tick: 0, in_flight: null, sell_on_arrival: !!element<HTMLInputElement>("m-log-sell")?.checked };
    this.ctx.send({ type: "SetStandingOrder", order });
  }

  private renderDoctrine(): SheetView {
    const fields = DOCTRINE_FIELDS.map((field) => `<label>${field.label}<select id="m-doctrine-${field.key}">${field.options.map(([value, label]) => option(value, label, this.ctx.state.doctrine[field.key] === value)).join("")}</select></label>`).join("");
    return { title: "Fleet Doctrine", eyebrow: "Corporate standing policy", html: `<section class="m-trade-card"><h3>Default fleet behavior</h3>${fields}<button type="button" class="m-primary" data-mobile-act="doctrine-save">Save doctrine</button><p>These policies govern autonomous combat and logistics decisions. Direct orders still take precedence.</p></section>` };
  }

  private saveDoctrine(): void {
    const doctrine = { ...this.ctx.state.doctrine } as FleetDoctrine;
    for (const field of DOCTRINE_FIELDS) (doctrine as unknown as Record<string, string>)[field.key] = element<HTMLSelectElement>(`m-doctrine-${field.key}`)?.value ?? doctrine[field.key];
    this.ctx.intent.beginFleetCommand({ type: "SetFleetDoctrine", doctrine });
  }

  private rememberSignature(entry: SheetEntry): void {
    const signature = this.signature(entry);
    if (signature !== null) this.renderSignatures.set(entry.id, signature);
  }

  /** Fingerprint only the served slice used by the active workspace. Serializing
   * these small owner-facing records is much cheaper than rebuilding its HTML. */
  private signature(entry: SheetEntry): string | null {
    const state = this.ctx.state;
    const clock = Math.floor(liveSimTime());
    const props = entry.props ?? null;
    switch (entry.id) {
      case "research":
        return sheetFingerprint([clock, this.researchField, state.research]);
      case "officers":
        return sheetFingerprint([clock, state.captains, state.captainCapacity, state.systems, state.ghosts.filter((fleet) => fleet.own)]);
      case "operations":
        return sheetFingerprint([clock, this.operationTab, this.handoffContract, state.midgameStage, state.operations, state.board, state.explorationSites, state.explorationJournal, state.selectedExplorationSiteId, state.ghosts.filter((fleet) => fleet.own), state.founding, state.systems, state.research, state.charterLedger, state.syndicateInvites]);
      case "syndicate":
        return sheetFingerprint([clock, state.syndicate, state.syndicateInvites, state.diplomacy, state.systems, state.ghosts.filter((fleet) => fleet.own), state.social?.members, state.social?.invites]);
      case "comms":
        return sheetFingerprint([state.social, state.syndicate, this.commsTab, this.commsMail, this.commsCompose, state.rankings.length]);
      case "faction":
        return sheetFingerprint([clock, state.charter, state.charterLadder]);
      case "rankings":
        return sheetFingerprint([clock, this.rankingCategory, state.rankings, state.syndicateRankings, state.syndicate?.id, state.playerId, props]);
      case "logistics":
        return sheetFingerprint([clock, state.standingOrders, state.systems, state.ghosts.filter(g => g.own), state.research, industryUiSignature(state), industryServedSignature(state)]);
      case "doctrine":
        return sheetFingerprint([clock, state.doctrine]);
      case "system": {
        const mode = this.ctx.renderer.viewMode;
        const id = propsOf<{ id: string }>(entry).id ?? state.selectedSystemId ?? (mode.type === "system" ? mode.systemId : "");
        return sheetFingerprint([
          clock, props, this.systemTab, mode.type, id, state.systems.find((system) => system.id === id),
          state.ghosts.filter((fleet) => fleet.own), state.groundRecords.filter((record) => record.system === id),
        ]);
      }
      case "planet": {
        const { systemId } = propsOf<{ systemId: string }>(entry);
        return sheetFingerprint([
          clock, props, this.planetTab, this.refiningDraft, this.coreDraft, state.market, state.research, state.systems.find((system) => system.id === systemId),
          state.systems.map((system) => [system.id, system.owner, system.blockade, system.habitat_fed, system.bodies.map((body) => [body.id, body.population])]),
          state.groundRecords.filter((record) => record.system === systemId),
        ]);
      }
      case "build": {
        const { systemId } = propsOf<{ systemId: string }>(entry);
        return sheetFingerprint([clock, props, this.selectedBuild, state.galaxy?.build_options,
          state.research?.programmes.map((p) => [p.id, p.state]), state.systems.find((system) => system.id === systemId)]);
      }
      case "shipyard": {
        const { systemId } = propsOf<{ systemId: string }>(entry);
        return sheetFingerprint([
          clock, props, this.selectedHull, this.pendingFit, state.galaxy?.build_options,
          state.research?.programmes.map((p) => [p.id, p.state]),
          state.systems.find((system) => system.id === systemId), state.syndicate?.fits,
        ]);
      }
      case "yard": {
        const { systemId } = propsOf<{ systemId: string }>(entry);
        return sheetFingerprint([clock, props, this.yardPicked, state.systems.find((system) => system.id === systemId)?.builds]);
      }
      case "hub":
        return sheetFingerprint([clock, state.ghosts.filter((fleet) => fleet.own && fleet.docked === "hub")]);
      default:
        return null;
    }
  }
}

const MIDGAME_COPY: Record<string, [string, string]> = {
  home_development: ["Home development", "Build a reliable industrial base and finish the founding programme."],
  exploration: ["Exploration", "Turn nearby darkness into strategic choices."],
  specialization: ["Specialization", "Choose what this corporation will do unusually well."],
  first_colony: ["First colony", "Establish a second physical holding."],
  trade_network: ["Trade network", "Connect specialized holdings with freight and escorts."],
  contested_expansion: ["Contested expansion", "Compete for scarce sites and public objectives."],
  regional_power: ["Regional power", "Hold strategic nodes and organize major operations."],
};

const safeId = (value: string): string => value.replace(/[^a-zA-Z0-9_-]/g, "_");
const endpointText = (endpoint: StandingEndpoint): string => endpoint.kind === "hub" ? "Market Hub" : endpoint.kind === "home" ? "Home" : systemName(endpoint.id);
const triggerText = (trigger: StandingTrigger): string => trigger.kind === "above_threshold" ? `above ${fmt(trigger.threshold)}` : trigger.kind === "maintain_at_dest" ? `maintain ${fmt(trigger.target)}` : `${fmt(trigger.percent)}% surplus above ${fmt(trigger.floor)}`;
const isOperationTab = (value?: string): value is OperationTab => value === "active" || value === "available" || value === "history";
const isSystemTab = (value?: string): value is SystemTab => value === "overview" || value === "worlds" || value === "production" || value === "construction";
const isPlanetTab = (value?: string): value is PlanetTab => value === "economy" || value === "population" || value === "infrastructure";
const isMigrationPolicy = (value?: string): value is MigrationPolicy => value === "closed" || value === "managed" || value === "open" || value === "priority";
const isCaptainAttribute = (value?: string): value is CaptainAttribute => value === "command" || value === "navigation" || value === "fieldcraft" || value === "logistics";
const isSyndicateRole = (value?: string): value is SyndicateRole => value === "member" || value === "quartermaster" || value === "officer" || value === "founder";
const isModule = (value?: string): value is ModuleKind => MODULES.some((module) => module.kind === value);
const isShipKind = (value?: string): value is ShipKind => !!value && SHIP_ORDER.includes(value as ShipKind);

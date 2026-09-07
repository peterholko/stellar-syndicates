import { captainTitle, captainXpFloor, fleetCommandLoad, officerFleetName } from "../../core/derive/captains";
import { constructionStock, dockedAtSystem, guardCapable, shipKindLabel, systemFleetsAt } from "../../core/derive/fleet";
import { colonyPurpose } from "../../core/derive/colony";
import { buildsByPlanet } from "../../core/derive/construction";
import { fleetReadiness } from "../../core/derive/readiness";
import { postVictoryHandoff } from "../../core/derive/handoff";
import { allySystems, foundingHomeSystemId, ownedSystems, systemName } from "../../core/derive/geo";
import {
  bodyPoolUsage,
  buildOption,
  fitLegal,
  FITTING_POINTS,
  MODULE_SLOTS,
  moduleLedgerAt,
  POOL_LABEL,
  POOL_OF,
  poolUsage,
  SHIP_YARD,
  shipOption,
  structOption,
  type BuildOpt,
  type Pool,
} from "../../core/derive/market";
import { fmtEta, operationCopy, operationReward, operationTitle } from "../../core/derive/format";
import { icon, structureIcon } from "../../icons";
import type {
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
  SyndicateRole,
  SystemStateView,
} from "../../protocol";
import { liveSimTime } from "../../state";
import type { SystemBodyDetail } from "../../systemview";
import { captainPortrait } from "../art";
import type { CoreContext } from "../types";
import type { SheetEntry, SheetView } from "./sheets";
import { SheetStack } from "./sheets";
import { sheetFingerprint } from "../signature";

interface ParityHooks {
  openSheet(entry: SheetEntry): void;
  focusFleet(id: string): void;
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
const SHIP_ORDER: ShipKind[] = ["scout", "corvette", "raider", "convoy", "colony", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
const SHIP_KEYS = new Set<string>(SHIP_ORDER);
const MODULES: { kind: ModuleKind; name: string }[] = [
  { kind: "mass_driver", name: "Mass Driver" },
  { kind: "torpedo_rack", name: "Torpedo Rack" },
  { kind: "point_defense_screen", name: "Point-Defense Screen" },
  { kind: "reflective_plating", name: "Reflective Plating" },
  { kind: "whipple_armor", name: "Whipple Armor" },
];
const PRODUCER_STRUCTURES = new Set([
  "mining_complex", "volatile_harvester", "bioharvester", "smelter", "electronics_fabricator",
  "chemical_works", "fuel_refinery", "machine_works", "armaments_complex", "agroplex", "academy",
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

const esc = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);
const human = (value: string): string => value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
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
  private readonly renderSignatures = new Map<SheetEntry["id"], string>();
  private researchField = "propulsion";
  private operationTab: OperationTab = "active";
  private handoffContract = "";
  private systemTab: SystemTab = "worlds";
  private planetTab: PlanetTab = "economy";
  private rankingCategory = "valuation";
  private selectedBuild = "";
  private selectedHull: ShipKind | "" = "";
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
      case "faction": view = this.renderFaction(); break;
      case "rankings": view = this.renderRankings(); break;
      case "logistics": view = this.renderLogistics(); break;
      case "doctrine": view = this.renderDoctrine(); break;
      case "system": view = this.renderSystem(entry); break;
      case "planet": view = this.renderPlanet(entry); break;
      case "build": view = this.renderBuild(entry); break;
      case "shipyard": view = this.renderShipyard(entry); break;
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

  handleClick(event: Event): boolean {
    const button = (event.target as Element).closest<HTMLElement>("[data-mobile-act]");
    if (!button) return false;
    const action = button.dataset.mobileAct;
    if (!action || !this.ownsAction(action)) return false;
    switch (action) {
      case "next-objectives":
        this.hooks.openSheet({ id: "operations" });
        break;
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
          this.ctx.intent.beginFleetCommand(operation?.briefing && operation.kind.kind === "rescue_salvage"
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
      case "syndicate-project": {
        const system = element<HTMLSelectElement>("m-syndicate-project-system")?.value;
        if (system) this.ctx.send({ type: "CreateSyndicateOperation", system_id: system });
        break;
      }
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
      case "open-rankings": this.hooks.openSheet({ id: "rankings" }); break;
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
      case "open-logistics": this.hooks.openSheet({ id: "logistics" }); break;
      case "open-doctrine": this.hooks.openSheet({ id: "doctrine" }); break;
      case "planet-tab":
        if (isPlanetTab(button.dataset.tab)) this.planetTab = button.dataset.tab;
        this.sheets.refresh();
        break;
      case "worker-set": this.setWorkers(button); break;
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
      case "ship-select":
        if (isShipKind(button.dataset.kind)) {
          this.selectedHull = button.dataset.kind;
          this.pendingFit = [];
        }
        this.sheets.refresh();
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

  private ownsAction(action: string): boolean {
    return /^(research|officer|operation|syndicate|diplomacy|flagship|faction|open-rankings|ranking|system|open-ground|open-planet|open-build|open-shipyard|open-logistics|open-doctrine|planet|worker|migration|module-build|build|ship-|hub-|logistics|doctrine)/.test(action);
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
        `<span>${fmt(research.active.progress)} / ${fmt(research.active.cost)} · ${research.active.eta_secs == null ? (research.stalled ? "stalled" : "awaiting supply") : `ETA ${fmtEta(research.active.eta_secs)}`}</span></article>`
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
    return `<article class="m-card is-${esc(programme.state)}"><header><small>Tier ${programme.tier}${programme.school ? ` · ${esc(human(programme.school))}` : ""}</small><em>${esc(programme.state)}</em></header>` +
      `<b>${esc(programme.name)}</b><p>${esc(programme.blurb)}</p>${gate}` +
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
        `<small class="m-hint">Academy ${academy || "required"} · 60s · 40 Provisions · 20 Electronics · 10 Machinery</small></section>` +
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
    const age = report?.portrait_age ?? "young";
    const titled = report ? `${captainTitle(report.title)} ${entry.name}` : entry.name;
    const floor = report ? captainXpFloor(report.level) : 0;
    const progress = report ? Math.max(0, Math.min(100, (report.xp - floor) / Math.max(1, report.next_level_xp - floor) * 100)) : 0;
    const assign = eligible.length
      ? `<div class="m-inline-form"><select id="m-officer-fleet-${entry.id}">${eligible.map((fleet) => option(fleet.id, `${officerFleetName(fleet)} · ${fleetCommandLoad(fleet)}/${report!.command_capacity}`)).join("")}</select><button type="button" data-mobile-act="officer-assign" data-captain="${entry.id}">${entry.assigned_fleet ? "Transfer" : "Assign"}</button></div>` : "";
    const training = report && report.unspent > 0
      ? `<div class="m-chip-actions">${(["command", "navigation", "fieldcraft", "logistics"] as CaptainAttribute[]).map((attribute) => `<button type="button" data-mobile-act="officer-train" data-captain="${entry.id}" data-attribute="${attribute}" ${canTrain ? "" : "disabled"}>+ ${human(attribute)}</button>`).join("")}</div>` : "";
    return `<article class="m-officer-card">${captainPortrait(entry.portrait, age, `Portrait of ${titled}`, "m-officer-portrait", true)}<div><header><b>${esc(titled)}</b><em>${esc(status)}</em></header>` +
      (report ? `<small>Level ${report.level} · authority ${report.command_capacity}</small><div class="m-progress"><i style="width:${progress.toFixed(1)}%"></i></div><p>Cmd ${report.attributes.command} · Nav ${report.attributes.navigation} · Field ${report.attributes.fieldcraft} · Log ${report.attributes.logistics}</p>` : `<p>Personnel light has not reached command.</p>`) +
      training + assign + (entry.assigned_fleet && local ? `<button type="button" data-mobile-act="officer-reserve" data-captain="${entry.id}">Return to reserve</button>` : "") +
      (assigned ? `<button type="button" data-mobile-act="officer-fleet" data-id="${esc(assigned.id)}">Select formation</button>` : "") + `</div></article>`;
  }

  private renderOperations(): SheetView {
    const [stageTitle, stageCopy] = MIDGAME_COPY[this.ctx.state.midgameStage] ?? [human(this.ctx.state.midgameStage), ""];
    const active = this.ctx.state.operations.filter((operation) => operation.state === "active" && operation.joined);
    const available = this.ctx.state.operations.filter((operation) => operation.state === "offered" || (operation.state === "active" && !operation.joined));
    const history = this.ctx.state.operations.filter((operation) => !["offered", "active"].includes(operation.state)).sort((a, b) => b.reported_at - a.reported_at);
    const rows = this.operationTab === "active" ? active : this.operationTab === "available" ? available : history;
    const selected = this.ctx.state.operations.find(o => o.id === this.handoffContract);
    return {
      title: "Operations",
      eyebrow: `${stageTitle} · contracts and objectives`,
      html: `<article class="m-feature-card"><small>CURRENT ARC</small><b>${esc(stageTitle)}</b><span>${esc(stageCopy)}</span></article>` +
        (selected ? `<section class="m-section"><h3>Funding contract</h3>${this.operationCard(selected)}</section>` : "") + this.handoffHtml() +
        `<div class="m-subtabs m-subtabs--3">${(["active", "available", "history"] as OperationTab[]).map((tab) => `<button type="button" data-mobile-act="operation-tab" data-tab="${tab}" aria-selected="${tab === this.operationTab}">${human(tab)} · ${(tab === "active" ? active : tab === "available" ? available : history).length}</button>`).join("")}</div>` +
        `<div class="m-programmes">${rows.filter(o => o !== selected).map((operation) => this.operationCard(operation)).join("") || `<div class="m-empty">No other ${this.operationTab} operations.</div>`}</div>`,
    };
  }

  private handoffHtml(): string {
    const goals = postVictoryHandoff();
    if (!goals.length) return "";
    return `<section class="m-section"><h3>Your next chapter</h3>${goals.map(g => `<article class="m-feature-card"><small>${esc(g.status)}</small><b>${esc(g.title)}</b><span>${esc(g.summary)}</span>
      <ul>${g.requirements.map(r => `<li>${r.met ? "✓" : "○"} ${esc(r.text)}</li>`).join("")}</ul>
      <small>${g.id === "explore" ? "Reward" : "Gain"}: ${esc(g.payoff)}</small>
      <button type="button" data-mobile-act="${g.action?.kind === "warehouse" ? "founding-action" : "next-goal"}" data-kind="market" data-goal="${g.id}" ${g.action ? "" : "disabled"}>${esc(g.actionLabel)}</button>
      ${g.prospect && !g.done ? `<button type="button" data-mobile-act="next-prospect" data-goal="${g.id}">Inspect ${esc(systemName(g.prospect))}</button>` : ""}
      ${g.funding && !g.done ? `<button type="button" data-mobile-act="next-funding" data-goal="${g.id}">Fund this · ${esc(operationTitle(g.funding))}<small>${esc(operationReward(g.funding))}</small></button>` : ""}</article>`).join("")}</section>`;
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
    // Like desktop these are navigation, not orders. The existing founding
    // market action opens the Warehouse tab for the already-earned kit.
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
      && (operation.kind.kind !== "freight_escort" || guardCapable(fleet)))
      .map((fleet) => option(fleet.id, officerFleetName(fleet), fleet.id === operation.assigned_fleet)).join("");
    const charges = this.ctx.state.ghosts.filter(g => g.own && fleetReadiness(g).cargoCapacity > 0)
      .map(g => option(g.id, officerFleetName(g), false)).join("");
    const id = safeId(operation.id);
    let actions = "";
    if ((operation.state === "offered" || (operation.state === "active" && !operation.joined)) && !operation.joined) actions = `<button type="button" class="m-primary" data-mobile-act="operation-accept" data-id="${esc(operation.id)}">Accept</button>`;
    if (operation.state === "active" && operation.joined) {
      actions += fleetOptions ? `<div class="m-inline-form"><select aria-label="Assigned fleet" id="m-op-fleet-${id}">${fleetOptions}</select>${operation.kind.kind === "freight_escort" ? `<select aria-label="Protected Freighter" id="m-op-charge-${id}"><option value="">Choose Freighter</option>${charges}</select>` : ""}<button type="button" data-mobile-act="operation-assign" data-id="${esc(operation.id)}">${operation.kind.kind === "freight_escort" ? "Assign guard" : operation.briefing?.follow_up === "salvage" ? "Plot recovery" : "Assign"}</button>` +
        (operation.kind.kind === "rescue_salvage" ? `<button type="button" data-mobile-act="operation-recover" data-id="${esc(operation.id)}">Recover now</button>` : "") + `</div>` : "";
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
      const canProject = ["founder", "officer", "quartermaster"].includes(syndicate.my_role);
      const projectSystems = this.ctx.state.systems.filter((system) => system.owner === this.ctx.state.playerId || system.ally).map((system) => option(system.id, systemName(system.id))).join("");
      const titan = this.ctx.state.ghosts.some((fleet) => fleet.own && fleet.composition?.some((stack) => stack.kind === "titan" && stack.count > 0));
      membership = `<article class="m-feature-card"><small>${esc(human(syndicate.my_role))}</small><b>${esc(syndicate.name)}</b><span>${syndicate.members.length} members · mutual non-engagement</span></article>` +
        `<section class="m-section"><h3>Roster</h3>${members}</section>` +
        (canInvite ? `<section class="m-trade-card"><h3>Invite corporation</h3><label>Corporation name<input id="m-syndicate-invite" maxlength="32"></label><button type="button" data-mobile-act="syndicate-invite">Invite</button></section>` : "") +
        (canProject ? `<section class="m-trade-card"><h3>Shared operation</h3><label>Host system<select id="m-syndicate-project-system">${projectSystems}</select></label><button type="button" data-mobile-act="syndicate-project" ${projectSystems ? "" : "disabled"}>Start project</button></section>` : "") +
        (titan ? `<section class="m-trade-card"><h3>Flagship</h3><label>Name<input id="m-flagship-name" maxlength="32" value="${esc(syndicate.flagship_name ?? "")}"></label><button type="button" data-mobile-act="flagship-name">Set name</button></section>` : "") +
        `<div class="m-action-grid"><button type="button" data-mobile-act="syndicate-leave">Leave</button>${syndicate.is_founder ? `<button type="button" class="m-danger" data-mobile-act="syndicate-dissolve">Dissolve</button>` : ""}</div>`;
    }
    return { title: syndicate?.name ?? "Syndicate", eyebrow: "Alliance network · formal diplomacy", html: membership + this.diplomacyBlock() };
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
    const shortfall = Math.max(0, charter.max_standing - charter.standing);
    const pay = shortfall > 0 ? `<section class="m-trade-card"><h3>Reinstatement</h3><label>Standing points<input id="m-faction-points" type="number" min="1" max="${Math.ceil(shortfall)}" value="${Math.ceil(Math.min(shortfall, 20))}"></label><p>${fmt(charter.reinstate_cost_per_point)} Cr per point</p><button type="button" data-mobile-act="faction-pay">Pay Authority</button></section>` : "";
    return {
      title: "Terran Charter Authority",
      eyebrow: `${charter.title} · legal standing`,
      html: `<div class="m-stat-grid"><span><small>Standing</small><b>${charter.standing.toFixed(0)}/${charter.max_standing.toFixed(0)}</b></span><span><small>Tariff</small><b>×${charter.tariff_mult.toFixed(2)}</b></span><span><small>Exchange penalty</small><b>${(charter.market_penalty_frac * 100).toFixed(1)}%</b></span><span><small>Status</small><b>${human(charter.status)}</b></span></div>` +
        `<section class="m-section"><h3>Standing bands</h3>${ladder}</section>${pay}<button type="button" class="m-wide-button" data-mobile-act="open-rankings">Open published rankings</button>`,
    };
  }

  private renderRankings(): SheetView {
    const category = RANK_CATS.find((candidate) => candidate.slug === this.rankingCategory) ?? RANK_CATS[0];
    const chips = RANK_CATS.map((candidate) => `<button type="button" data-mobile-act="ranking-category" data-category="${candidate.slug}" aria-selected="${candidate.slug === category.slug}">${candidate.label}</button>`).join("");
    const rows = [...this.ctx.state.rankings].sort((a, b) => category.value(b) - category.value(a)).map((row, index) => `<div class="m-ranking-row${row.player_id === this.ctx.state.playerId ? " is-me" : ""}"><strong>${index + 1}</strong><span><b>${esc(row.name)}</b><small>${row.titles.map(human).join(" · ") || (row.player_id === this.ctx.state.playerId ? "your corporation" : "")}</small></span><em>${esc(category.text(row))}</em></div>`).join("");
    return { title: "Rankings", eyebrow: `Published ledger · ${category.label}`, html: `<div class="m-scroll-tabs m-scroll-tabs--compact">${chips}</div>${rows || `<div class="m-empty">No ledger has been published yet.</div>`}` };
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
    if (this.systemTab === "overview") body = this.systemOverview(fixed.name, dynamic, mine, fleets, id!);
    else if (this.systemTab === "worlds") body = this.systemWorlds(id!, dynamic);
    else if (this.systemTab === "production") body = mine ? this.systemProduction(dynamic!) : `<div class="m-empty">Production is private.</div>`;
    else body = mine ? this.systemConstruction(id!, dynamic!) : `<div class="m-empty">Construction is private.</div>`;
    return {
      title: fixed.name,
      eyebrow: mine ? "Your system · served management" : dynamic?.owner ? "Observed rival holding" : "Unclaimed system",
      html: `<div class="m-action-grid m-action-grid--top"><button type="button" class="m-primary" data-mobile-act="${semantic ? "system-exit" : "system-enter"}" data-id="${esc(id!)}">${semantic ? "Back to galaxy" : "Open System View"}</button>` +
        (mine ? `<button type="button" data-mobile-act="open-logistics">Auto-supply</button><button type="button" data-mobile-act="open-doctrine">Fleet doctrine</button>` : "") + `</div>` +
        `<div class="m-subtabs">${tabs}</div>${body}`,
    };
  }

  private systemOverview(name: string, dynamic: SystemStateView | undefined, mine: boolean, fleets: GhostView[], systemId: string): string {
    const pools = poolUsage(dynamic);
    const homeId = foundingHomeSystemId();
    const home = this.ctx.state.systems.find(s => s.id === homeId && s.owner === this.ctx.state.playerId);
    const purpose = dynamic && systemId !== homeId ? colonyPurpose(dynamic, home) : null;
    const purposeHtml = purpose ? `<section class="m-section"><h3>Colony purpose</h3><b>${esc(purpose.headline)}</b>
      <p>${esc(purpose.homeNeed)}</p><span>Supply imports: ${esc(purpose.imports.map(human).join(", ") || "No essential feedstock missing")}</span></section>` : "";
    const fleetRows = fleets.map((fleet) => `<button type="button" class="m-list-row" data-mobile-act="system-fleet" data-id="${esc(fleet.id)}"><span class="m-list-row__icon">△</span><span class="m-list-row__main"><b>${esc(shipKindLabel(fleet.kind))} fleet</b><small>${fleet.docked ? "docked" : Math.hypot(fleet.vel.x, fleet.vel.y) > 1 ? "under way" : "holding"}</small></span><span class="m-list-row__meta"><small>${fleet.age.toFixed(1)}s delay</small></span></button>`).join("");
    return purposeHtml + `<div class="m-stat-grid"><span><small>Owner</small><b>${mine ? "Your corporation" : dynamic?.owner ? "Rival" : "Unclaimed"}</b></span><span><small>Worlds</small><b>${dynamic?.bodies.length ?? 0}</b></span>` +
      (mine ? `<span><small>Population</small><b>${fmtPopulation(dynamic?.population ?? 0)}</b></span><span><small>Storage</small><b>${fmt(dynamic?.storage_used ?? 0)}/${fmt(dynamic?.storage_cap ?? 0)}</b></span>` : "") + `</div>` +
      (mine ? `<section class="m-section"><h3>Development pools</h3><div class="m-ledger">${(["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span>${POOL_LABEL[pool]}<b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("")}</div>${this.developmentPoolHelp()}</section>` : "") +
      `<section class="m-section"><h3>Fleets at ${esc(name)}</h3><div class="m-list">${fleetRows || `<div class="m-muted">No own fleets in the served picture.</div>`}</div></section>` +
      this.groundAction(systemId) +
      `<button type="button" class="m-wide-button" data-mobile-act="system-focus" data-id="${esc(systemId)}">Center on map</button>`;
  }

  private systemWorlds(systemId: string, dynamic: SystemStateView | undefined): string {
    return (dynamic?.bodies ?? []).map((body) => `<button type="button" class="m-world-row" data-mobile-act="open-planet" data-system="${esc(systemId)}" data-body="${body.id}"><span><b>${esc(body.name)}</b><small>${human(body.size)} ${human(body.environment)} · ${human(body.kind)}</small></span><em>${body.population > 0 ? fmtPopulation(body.population) : body.geology == null ? "unsurveyed" : human(body.geology)}</em></button>`).join("") || `<div class="m-empty">No body roster has arrived.</div>`;
  }

  private systemProduction(dynamic: SystemStateView): string {
    const stock = dynamic.stockpile?.map((slot) => `<span>${human(slot.commodity)}<b>${fmt(slot.units)}</b></span>`).join("") ?? "";
    const lines = dynamic.assignments.map((line) => `<div class="m-production-row"><span><b>${esc(line.title)} ×${line.tier}</b><small>${esc(dynamic.bodies.find((body) => body.id === line.body_id)?.name ?? "World")} · ${line.workers} workforce${line.suspended ? ` · ${human(line.suspended)}` : ""}</small></span><em>${line.outputs.map(([commodity, rate]) => `+${rate.toFixed(2)} ${human(commodity)}/s`).join(" · ") || "idle"}</em></div>`).join("");
    return `<section class="m-section m-section--first"><h3>Stockpile · ${fmt(dynamic.storage_used)}/${fmt(dynamic.storage_cap)}</h3><div class="m-ledger">${stock || `<span>Empty</span>`}</div></section><section class="m-section"><h3>Production lines</h3>${lines || `<div class="m-muted">No staffed lines.</div>`}</section>`;
  }

  private systemConstruction(systemId: string, dynamic: SystemStateView): string {
    const queue = buildsByPlanet(dynamic.builds).map(({ bodyId, jobs }) => `<div class="m-section"><h4>${esc(dynamic.bodies.find((body) => body.id === bodyId)?.name ?? "System yard")}</h4>${jobs.map((job) => `<div class="m-order"><b>${esc(buildOption(job.key)?.label ?? human(job.key))}</b><span>${job.queued ? "Queued" : job.complete_time == null ? "Paused · needs workforce" : `Building · ${fmtEta(Math.max(0, job.complete_time - liveSimTime()))}`}</span></div>`).join("")}</div>`).join("");
    const worlds = dynamic.bodies.map((body) => `<div class="m-service-row"><span><b>${esc(body.name)}</b><small>${Object.values(body.structures).reduce((sum, tier) => sum + tier, 0)} structure tiers</small></span><div><button type="button" data-mobile-act="open-build" data-system="${esc(systemId)}" data-body="${body.id}">Structures</button>${(body.structures.shipyard ?? 0) > 0 ? `<button type="button" data-mobile-act="open-shipyard" data-system="${esc(systemId)}" data-body="${body.id}">Ships</button>` : ""}</div></div>`).join("");
    return `<section class="m-section m-section--first"><h3>Queue</h3>${queue || `<div class="m-muted">Nothing under construction.</div>`}</section>` +
      `<section class="m-section"><h3>Build by world</h3>${worlds}</section>` +
      `<div class="m-action-grid"><button type="button" data-mobile-act="system-ship-production" data-id="${esc(systemId)}">Ship production to Hub</button><button type="button" data-mobile-act="open-logistics">Auto-supply rules</button></div>`;
  }

  private openPlanet(systemId: string, bodyId: number): void {
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
      `<section class="m-section"><h3>Deposits</h3>${body.deposits == null ? `<div class="m-muted">Geology unsurveyed.</div>` : body.deposits.map((deposit) => `<div class="m-order"><b>${human(deposit.resource)}</b><span>richness ${deposit.richness.toFixed(2)} · ${deposit.reserves == null ? "renewable" : `${fmt(deposit.reserves)} reserves`}</span></div>`).join("") || `<div class="m-muted">No deposits.</div>`}</section></div></details>`;
    const ground = this.groundAction(systemId);
    if (!mine) return { title: body.name, eyebrow: `${human(body.kind)} · observed world`, html: publicData + ground };
    const active = this.planetTab === "economy" ? this.planetEconomy(systemId, dynamic, body) : this.planetTab === "population" ? this.planetPopulation(systemId, body) : this.planetInfrastructure(systemId, dynamic, body);
    return { title: body.name, eyebrow: "Owned world · served management", html: `<div class="m-subtabs m-subtabs--3">${tabs}</div>${active}${publicData}${ground}` };
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
    const rows = assignments.map((line) => this.assignmentRow(systemId, line.body_id, line.structure, line.title, line.workers, line.specialists, line.outputs, line.suspended)).join("");
    const assigned = new Set(assignments.map((line) => line.structure));
    const idle = Object.entries(body.structures).filter(([slug, tier]) => tier > 0 && PRODUCER_STRUCTURES.has(slug) && !assigned.has(slug)).map(([slug]) => this.assignmentRow(systemId, body.id, slug, human(slug), 0, {}, [], "needs_crew")).join("");
    const built = Object.entries(body.structures).filter(([, tier]) => tier > 0).map(([slug, tier]) => `<span>${human(slug)}<b>×${tier}</b></span>`).join("");
    return `<section class="m-section m-section--first"><h3>Built here</h3><div class="m-ledger">${built || `<span>Undeveloped</span>`}</div></section><section class="m-section"><h3>Workforce assignments</h3>${rows + idle || `<div class="m-muted">No production structures.</div>`}</section>`;
  }

  private assignmentRow(systemId: string, bodyId: number, structure: string, title: string, workers: number, specialists: Record<string, number>, outputs: [Commodity, number][], suspended: string | null): string {
    const output = outputs.map(([commodity, rate]) => `+${rate.toFixed(2)} ${human(commodity)}/s`).join(" · ") || "idle";
    const specialistsText = Object.entries(specialists).filter(([, n]) => n > 0).map(([kind, n]) => `${n} ${human(kind)}`).join(" · ");
    return `<div class="m-assignment"><span><b>${esc(title)}</b><small>${workers} workforce${specialistsText ? ` · ${esc(specialistsText)}` : ""}${suspended ? ` · ${human(suspended)}` : ""}</small><em>${esc(output)}</em></span><div><button type="button" data-mobile-act="worker-set" data-system="${esc(systemId)}" data-body="${bodyId}" data-structure="${esc(structure)}" data-workers="${Math.max(0, workers - 1)}" ${workers <= 0 ? "disabled" : ""}>−</button><button type="button" data-mobile-act="worker-set" data-system="${esc(systemId)}" data-body="${bodyId}" data-structure="${esc(structure)}" data-workers="${workers + 1}">+</button></div></div>`;
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
    const queue = dynamic.builds.filter((job) => job.body_id === body.id).map((job) => `<div class="m-order"><b>${esc(buildOption(job.key)?.label ?? human(job.key))}</b><span>${job.queued ? "Queued" : job.complete_time == null ? "Paused · needs workforce" : `Building · ${fmtEta(Math.max(0, job.complete_time - liveSimTime()))}`}</span></div>`).join("");
    const modules = (body.structures.armaments_complex ?? 0) > 0 ? `<section class="m-section"><h3>Module forge</h3>${MODULES.map((module) => {
      const recipe = buildOption(`module:${module.kind}`);
      const have = constructionStock(dynamic).available;
      const afford = !!recipe && recipe.costs.every((cost) => (have.get(cost.commodity as Commodity) ?? 0) >= cost.units);
      return `<div class="m-service-row"><span><b>${module.name}</b><small>ledger ${dynamic.modules?.[module.kind] ?? 0} · ${recipe?.costs.map((cost) => `${cost.units} ${human(cost.commodity)}`).join(" + ") ?? "recipe unavailable"}</small></span><button type="button" data-mobile-act="module-build" data-system="${esc(systemId)}" data-module="${module.kind}" ${afford ? "" : "disabled"}>Build</button></div>`;
    }).join("")}</section>` : "";
    return `<section class="m-section m-section--first"><h3>Body slot pools</h3><div class="m-ledger">${(["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span>${POOL_LABEL[pool]}<b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("")}</div>${this.developmentPoolHelp()}</section>` +
      `<div class="m-action-grid"><button type="button" class="m-primary" data-mobile-act="open-build" data-system="${esc(systemId)}" data-body="${body.id}">Build structure</button>${(body.structures.shipyard ?? 0) > 0 ? `<button type="button" data-mobile-act="open-shipyard" data-system="${esc(systemId)}" data-body="${body.id}">Build ship</button>` : ""}</div>` +
      `<section class="m-section"><h3>Construction here</h3>${queue || `<div class="m-muted">Nothing queued.</div>`}</section>${modules}`;
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
    const options = (this.ctx.state.galaxy?.build_options ?? []).filter((candidate) => !SHIP_KEYS.has(candidate.key) && !candidate.key.startsWith("module:") && !!POOL_OF[candidate.key]) as BuildOpt[];
    if (this.selectedBuild && !options.some((candidate) => candidate.key === this.selectedBuild)) this.selectedBuild = "";
    const rows = options.map((candidate) => {
      const state = structOption(candidate, dynamic, body, pools);
      return `<button type="button" class="m-build-row${candidate.key === this.selectedBuild ? " is-active" : ""}" data-mobile-act="build-select" data-key="${esc(candidate.key)}"><span class="m-build-row__identity">${icon(structureIcon(candidate.key), "sm", undefined, "m-structure-icon")}<span><b>${esc(candidate.label)}</b><small>${POOL_LABEL[state.pool]} · ${state.tierUp ? `Tier ${state.currentTier} → ${state.targetTier}` : "new Tier I"}</small></span></span><em>${state.buildable ? fmtEta(candidate.build_secs * body.construction_time_mult) : esc(state.reason || "unavailable")}</em></button>`;
    }).join("");
    const selected = options.find((candidate) => candidate.key === this.selectedBuild);
    const detail = selected ? this.structureDetail(systemId, dynamic, body, selected, pools) : `<div class="m-empty">Choose a structure to inspect its recipe and queue it.</div>`;
    return { title: `Build on ${body.name}`, eyebrow: "Structure construction · local supply", html: `<div class="m-ledger m-ledger--pools">${(["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span>${POOL_LABEL[pool]}<b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("")}</div><div class="m-build-list">${rows}</div>${detail}` };
  }

  private structureDetail(systemId: string, dynamic: SystemStateView, body: BodyView, build: BuildOpt, pools: ReturnType<typeof bodyPoolUsage>): string {
    const state = structOption(build, dynamic, body, pools);
    const supply = constructionStock(dynamic).available;
    const costs = build.costs.map((cost) => `<div class="m-order"><b>${human(cost.commodity)}</b><span>need ${cost.units} · have ${fmt(supply.get(cost.commodity as Commodity) ?? 0)}</span></div>`).join("");
    return `<article class="m-build-detail"><header>${icon(structureIcon(build.key), "md", undefined, "m-structure-icon")}<span><small>${POOL_LABEL[state.pool]} · ${state.tierUp ? `upgrade to Tier ${state.targetTier}` : "new structure"}</small><h3>${esc(build.label)}</h3></span></header><div>${costs}</div><p>${fmtEta(build.build_secs * body.construction_time_mult)} on this world.${state.foundsNew ? ` Claims one ${POOL_LABEL[state.pool].toLowerCase()} slot.` : " Deepens in place without another slot."}</p>${state.reason ? `<div class="m-warning">${esc(state.reason)}</div>` : ""}<button type="button" class="m-primary" data-mobile-act="build-queue" data-system="${esc(systemId)}" data-body="${body.id}" data-key="${esc(build.key)}" ${state.buildable ? "" : "disabled"}>Queue build</button></article>`;
  }

  private queueStructure(button: HTMLElement): void {
    const system = button.dataset.system;
    const body = Number(button.dataset.body);
    const key = button.dataset.key;
    if (system && key && Number.isFinite(body)) {
      this.ctx.send({ type: "DevelopSystem", system_id: system, upgrade: key, body_id: body });
      this.hooks.notice(`${human(key)} queued at ${systemName(system)}.`);
    }
  }

  private renderShipyard(entry: SheetEntry): SheetView {
    const { systemId, dynamic, body } = this.buildContext(entry);
    if (!systemId || !dynamic || !body) return { title: "Shipyard", eyebrow: "Hull construction", html: `<div class="m-empty">Shipyard context is unavailable.</div>` };
    const options = SHIP_ORDER.map((kind) => this.ctx.state.galaxy?.build_options.find((candidate) => candidate.key === kind)).filter((candidate) => candidate !== undefined);
    if (this.selectedHull && !options.some((candidate) => candidate.key === this.selectedHull)) this.selectedHull = "";
    const rows = options.map((candidate) => {
      const state = shipOption(candidate, dynamic);
      return `<button type="button" class="m-build-row${candidate.key === this.selectedHull ? " is-active" : ""}" data-mobile-act="ship-select" data-kind="${candidate.key}"><span><b>${esc(candidate.label)}</b><small>${SHIP_YARD[candidate.key]?.yard ? human(SHIP_YARD[candidate.key].yard) : "yard"} · ${state.buildRate > 0 ? fmtEta(candidate.build_secs * (state.yardBody?.ship_build_time_mult ?? 1) / state.buildRate) : "needs workforce"}</small></span><em>${state.buildable ? `max ${state.maxAff}` : esc(state.reason)}</em></button>`;
    }).join("");
    const selected = options.find((candidate) => candidate.key === this.selectedHull);
    const detail = selected ? this.shipDetail(systemId, dynamic, body, selected) : `<div class="m-empty">Choose a hull to inspect its recipe, fitting and batch controls.</div>`;
    return { title: `Shipyard · ${body.name}`, eyebrow: "Hull construction · local supply", html: `<div class="m-build-list">${rows}</div>${detail}` };
  }

  private shipDetail(systemId: string, dynamic: SystemStateView, body: BodyView, build: BuildOpt): string {
    const hull = build.key as ShipKind;
    const state = shipOption(build, dynamic);
    const supply = constructionStock(dynamic).available;
    const ledger = moduleLedgerAt(systemId);
    const slots = MODULE_SLOTS[hull] ?? 0;
    const effectiveFit = this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0).slice(0, slots);
    const fitOkay = !effectiveFit.length || fitLegal(hull, effectiveFit);
    const costs = build.costs.map((cost) => `<div class="m-order"><b>${human(cost.commodity)}</b><span>${cost.units} each · have ${fmt(supply.get(cost.commodity as Commodity) ?? 0)}</span></div>`).join("");
    const modules = slots ? MODULES.filter((module) => (ledger[module.kind] ?? 0) > 0).map((module) => `<button type="button" data-mobile-act="ship-fit" data-module="${module.kind}" aria-pressed="${this.pendingFit.includes(module.kind)}">${esc(module.name)} · ${ledger[module.kind]}</button>`).join("") : "";
    const fits = (this.ctx.state.syndicate?.fits ?? []).filter((fit) => fit.kind === hull).map((fit) => `<span class="m-saved-fit"><button type="button" data-mobile-act="ship-fit-pick" data-name="${esc(fit.name)}">${esc(fit.name)}</button><button type="button" data-mobile-act="ship-fit-delete" data-name="${esc(fit.name)}">×</button></span>`).join("");
    return `<article class="m-build-detail"><small>${human(SHIP_YARD[hull]?.yard ?? "shipyard")} · ${slots} module slots · ${FITTING_POINTS[hull] ?? 0} fit points</small><h3>${esc(build.label)}</h3>${costs}` +
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
    const fit = this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0).slice(0, MODULE_SLOTS[this.selectedHull] ?? 0);
    if (!state.buildable || quantity < 1 || (fit.length > 0 && !fitLegal(this.selectedHull, fit))) return;
    for (let i = 0; i < quantity; i++) this.ctx.send({ type: "BuildShip", system_id: system, ship_kind: this.selectedHull, loadout: fit.length ? fit : undefined });
    this.hooks.notice(`${quantity}× ${shipKindLabel(this.selectedHull)} queued.`);
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
    const sources = ownedSystems();
    const destinations = `<option value="hub">Market Hub</option><option value="home">Home system</option>` + sources.map((system) => option(system.id, system.name)).join("") + allySystems().map((system) => option(system.id, `${system.name} · ally`)).join("");
    const commodities = COMMODITIES_LOCAL.map((commodity) => option(commodity, human(commodity))).join("");
    const orders = this.ctx.state.standingOrders.map((order) => `<div class="m-standing-row"><span><b>#${order.id} · ${human(order.commodity)}</b><small>${endpointText(order.source)} → ${endpointText(order.dest)} · ${triggerText(order.trigger)} · ${order.in_flight ? "freighter en route" : order.status}</small></span><button type="button" data-mobile-act="logistics-clear" data-id="${order.id}">×</button></div>`).join("");
    return {
      title: "Auto-supply",
      eyebrow: "Standing logistics · runs while away",
      html: `<section class="m-section m-section--first"><h3>Standing orders</h3>${orders || `<div class="m-muted">No standing orders.</div>`}</section>` +
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
        return sheetFingerprint([clock, this.operationTab, this.handoffContract, state.midgameStage, state.operations, state.ghosts.filter((fleet) => fleet.own), state.founding, state.systems, state.research]);
      case "syndicate":
        return sheetFingerprint([clock, state.syndicate, state.syndicateInvites, state.diplomacy, state.systems, state.ghosts.filter((fleet) => fleet.own)]);
      case "faction":
        return sheetFingerprint([clock, state.charter, state.charterLadder]);
      case "rankings":
        return sheetFingerprint([clock, this.rankingCategory, state.rankings, state.playerId]);
      case "logistics":
        return sheetFingerprint([clock, state.standingOrders, state.systems.map((system) => [system.id, system.owner, system.ally])]);
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
          clock, props, this.planetTab, state.systems.find((system) => system.id === systemId),
          state.systems.map((system) => [system.id, system.owner, system.blockade, system.habitat_fed, system.bodies.map((body) => [body.id, body.population])]),
          state.groundRecords.filter((record) => record.system === systemId),
        ]);
      }
      case "build": {
        const { systemId } = propsOf<{ systemId: string }>(entry);
        return sheetFingerprint([clock, props, this.selectedBuild, state.galaxy?.build_options, state.systems.find((system) => system.id === systemId)]);
      }
      case "shipyard": {
        const { systemId } = propsOf<{ systemId: string }>(entry);
        return sheetFingerprint([
          clock, props, this.selectedHull, this.pendingFit, state.galaxy?.build_options,
          state.systems.find((system) => system.id === systemId), state.syndicate?.fits,
        ]);
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

const COMMODITIES_LOCAL: Commodity[] = ["metallic_ore", "rare_elements", "silicates", "volatiles", "biomass", "alloys", "electronics", "polymers", "fuel", "provisions", "machinery", "armaments"];
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

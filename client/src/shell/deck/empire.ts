import { constructionStock, dockedAtSystem, shipKindLabel } from "../../core/derive/fleet";
import { colonyPurpose } from "../../core/derive/colony";
import { buildProgress, buildsByPlanet } from "../../core/derive/construction";
import { fmtBuildDur } from "../../core/derive/format";
import {
  bodyPoolUsage,
  buildOption,
  dispatchBuildKey,
  fitLegal,
  FITTING_POINTS,
  MODULE_SLOTS,
  moduleLedgerAt,
  POOL_LABEL,
  POOL_OF,
  SHIP_YARD,
  shipOption,
  structOption,
  systemFlavor,
  YARD_TITLE,
  type BuildOpt,
  type Pool,
} from "../../core/derive/market";
import { latestGroundRecordFor, SHIP_STATS } from "../../core/derive/orders";
import type { CoreEvent } from "../../core/events";
import { commodityIcon as commodityGlyph, icon, label, structureIcon, type IconKey } from "../../icons";
import type { AssignmentView, BodyView, Commodity, ModuleKind, ShipKind, SystemInfo, SystemStateView } from "../../protocol";
import { liveSimTime } from "../../state";
import { starIconUrl, starTypeFor } from "../../stars";
import { bodyArtUrl } from "../../systemview";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import { fleetListRow } from "./fleet-row";
import type { DeckRoute } from "./router";

export type DeckSystemTab = "overview" | "worlds" | "production" | "fleets" | "build";

interface EmpireHooks {
  go(route: DeckRoute): void;
  openGroundViewer(id: string): void;
  notice(html: string): void;
  toast(title: string, message: string, tone?: "quiet" | "good" | "warn" | "bad", destination?: DeckRoute): void;
}

type BuilderMode = "structures" | "ships" | "modules";
type BuildFeedback = { systemId: string; issuedAt: number; timelineLength: number; text: string; tone: "info" | "good" | "bad" };
type WorldWorkbenchContext = { systemId: string; bodyId: number; origin: string };

const SHIP_ORDER: ShipKind[] = ["scout", "corvette", "raider", "convoy", "colony", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
const SHIP_KEYS = new Set<string>(SHIP_ORDER);
const WORKFORCE_STRUCTURES = new Set([
  "mining_complex", "volatile_harvester", "bioharvester", "smelter", "electronics_fabricator",
  "chemical_works", "fuel_refinery", "machine_works", "armaments_complex", "agroplex", "academy",
  "shipyard", "naval_drydock", "capital_slipway", "ordnance_foundry",
]);
const STRUCTURE_DESCRIPTION: Record<string, string> = {
  mining_complex: "Extracts Metallic Ore, Silicates, or Rare Elements from local deposits.",
  volatile_harvester: "Extracts Volatiles from a local deposit.",
  bioharvester: "Harvests Biomass from a habitable world's biosphere.",
  smelter: "Turns Metallic Ore and Fuel into Alloys.",
  electronics_fabricator: "Turns Rare Elements and Silicates into Electronics.",
  chemical_works: "Turns Volatiles and Biomass into Polymers.",
  fuel_refinery: "Turns Volatiles into ship Fuel.",
  agroplex: "Turns Biomass into Provisions for colonies and fleets.",
  machine_works: "Turns Alloys, Electronics, and Fuel into Machinery.",
  armaments_complex: "Produces Armaments and manufactures fleet modules.",
  shipyard: "Builds civilian hulls and light warships.",
  naval_drydock: "Builds Destroyers, Cruisers, and Battleships.",
  capital_slipway: "Builds Dreadnoughts and Titans.",
  ordnance_foundry: "Repairs battle damage and supports fleet refits.",
  habitat: "Raises this world's population capacity and workforce.",
  orbital_warehouse: "Expands the system's shared stockpile capacity.",
  sensor_array: "Projects a sensor bubble around the system.",
  defense_platform: "Defends the system against hostile fleets.",
  academy: "Contributes research and trains specialists and officers.",
  garrison: "Defends the planet and raises troop transports.",
};
const MODULES: { kind: ModuleKind; label: string; icon: IconKey; fit: number }[] = [
  { kind: "mass_driver", label: "Mass Driver", icon: "moduleMassDriver", fit: 2 },
  { kind: "torpedo_rack", label: "Torpedo Rack", icon: "moduleTorpedoRack", fit: 3 },
  { kind: "point_defense_screen", label: "Point-Defense Screen", icon: "modulePointDefense", fit: 2 },
  { kind: "reflective_plating", label: "Reflective Plating", icon: "moduleReflectivePlating", fit: 2 },
  { kind: "whipple_armor", label: "Whipple Armor", icon: "moduleWhippleArmor", fit: 3 },
];

/** Routed empire management. This reads only the player's served SystemStateView:
 * public astronomy is always visible, survey findings require arrived survey
 * light, and owner-only production never receives a rival rendering path. */
export class DeckEmpireRoutes {
  private systemTab: DeckSystemTab = "overview";
  private builderMode: BuilderMode = "structures";
  private selectedBuild = "";
  private selectedHull: ShipKind | "" = "";
  private shipQuantity = 1;
  private pendingFit: ModuleKind[] = [];
  private buildFeedback: BuildFeedback | null = null;
  private lastBuildPreset = "";
  private lastSystemId = "";
  private buildContext = "";
  private workbenchOpen = false;
  private worldWorkbench: WorldWorkbenchContext | null = null;
  private worldSignature = "";
  private signature = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly workbenchRoot: HTMLElement,
    private readonly worldWorkbenchRoot: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: EmpireHooks,
  ) {}

  get composedFit(): ModuleKind[] {
    return this.pendingFit;
  }

  render(route: DeckRoute | null, force = false): boolean {
    this.renderWorldWorkbench(route, force);
    if (route?.name !== "system" && route?.name !== "build" && route?.name !== "world") {
      this.resetWorkbench();
      return false;
    }
    const context = this.systemContext(route);
    const systemId = context.system?.id ?? "";
    if (systemId && systemId !== this.lastSystemId) {
      this.systemTab = route.name === "build"
        ? "build"
        : isSystemTab(route.query?.tab) ? route.query.tab : "overview";
      this.lastSystemId = systemId;
    } else if (route.name === "build") {
      this.systemTab = "build";
    } else if (isSystemTab(route.query?.tab)) {
      this.systemTab = route.query.tab;
    }
    const buildContext = !this.worldWorkbench && route.name !== "world" && this.systemTab === "build" ? systemId : "";
    if (buildContext !== this.buildContext) {
      this.buildContext = buildContext;
      this.workbenchOpen = !!buildContext;
    }
    const docked = context.system ? this.dockedFleets(context.system) : [];
    const signature = sheetFingerprint([
      route, this.systemTab, this.builderMode, this.selectedBuild, this.selectedHull,
      this.shipQuantity, this.pendingFit, this.buildFeedback, this.workbenchOpen, Math.floor(liveSimTime()),
      context.system, context.dynamic, this.ctx.state.timeline.length, this.ctx.state.syndicate?.fits,
      this.ctx.state.syndicate?.flagship_name,
      this.systemTab === "fleets" ? docked.map((fleet) => [
        fleet.id, fleet.kind, fleet.docked, Math.floor(fleet.age), fleet.composition,
        fleet.cargo_manifest, fleet.cargo,
      ]) : null,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))
      || (this.workbenchOpen && renderDeferred(this.workbenchRoot.id, () => this.render(route, true)))) return true;
    this.signature = signature;
    setHtml(this.root, route.name === "world"
        ? this.worldHtml(route, context.system, context.dynamic)
        : this.systemHtml(context.system, context.dynamic));
    this.renderWorkbench(route, context.system, context.dynamic);
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name === "world") return this.handleWorldAction(button, route);
    if (route?.name !== "system" && route?.name !== "build") return false;
    const action = button.dataset.deckAct;
    const { system, dynamic } = this.systemContext(route);
    if (!system) return false;
    if (action === "build-workbench-close") return this.closeBuildWorkbench(route);
    if (action === "system-tab") {
      const tab = button.dataset.tab;
      if (!isSystemTab(tab)) return true;
      this.systemTab = tab;
      if (tab === "build") this.workbenchOpen = true;
      this.signature = "";
      if (route.name === "build" && tab !== "build") {
        this.hooks.go({ name: "system", params: { id: system.id, systemLabel: system.name } });
      } else {
        this.render(route, true);
      }
      return true;
    }
    if (this.systemTab === "build") return this.handleBuildAction(button, route);
    if (action === "watch-ground" && button.dataset.ground) {
      this.hooks.openGroundViewer(button.dataset.ground);
      return true;
    }
    if (action === "system-fleet-open" && button.dataset.fleet) {
      const fleet = this.ctx.state.ghosts.find((candidate) =>
        candidate.id === button.dataset.fleet && candidate.own && dockedAtSystem(candidate, system.id));
      if (fleet) {
        this.ctx.state.selectedShipId = fleet.id;
        this.ctx.state.selectedShipIds = new Set([fleet.id]);
        this.ctx.state.selectedEmplacementId = null;
        this.ctx.renderer.stateVersion++;
        this.hooks.go({ name: "fleet", params: { id: fleet.id, fleetLabel: `${shipKindLabel(fleet.kind)} fleet` } });
      }
      return true;
    }
    if (action === "open-world") {
      const body = dynamic?.bodies.find((candidate) => String(candidate.id) === button.dataset.body);
      if (body) this.openWorldWorkbench(system.id, body.id, route);
      return true;
    }
    if (action === "ship-production") {
      if (!dynamic || dynamic.owner !== this.ctx.state.playerId || dynamic.blockade) return true;
      const manifest = shippableStock(dynamic);
      if (!manifest.length) return true;
      this.ctx.send({ type: "ShipProduction", system_id: system.id });
      this.hooks.notice(`<b>Authority pickup requested</b> · ${manifest.map((slot) => `${fmt(slot.units)} ${esc(label(slot.commodity))}`).join(" · ")} → Market Hub.`);
      return true;
    }
    if (action === "set-workers") {
      if (!dynamic || dynamic.owner !== this.ctx.state.playerId) return true;
      const bodyId = Number(button.dataset.body);
      const structure = button.dataset.structure;
      const workers = Math.max(0, Number(button.dataset.workers) || 0);
      const assignment = dynamic.assignments.find((entry) => entry.body_id === bodyId && entry.structure === structure);
      if (structure && Number.isFinite(bodyId)) {
        this.ctx.send({
          type: "SetAssignment",
          system_id: system.id,
          body_id: bodyId,
          structure,
          workers,
          specialists: assignment?.specialists ?? {},
        });
        this.hooks.notice(`<b>Workforce directive sent</b> · ${esc(label(structure))} → ${workers}.`);
      }
      return true;
    }
    return false;
  }

  invalidate(): void {
    this.signature = "";
    this.worldSignature = "";
  }

  openWorldWorkbench(systemId: string, bodyId: number, origin: DeckRoute | null): boolean {
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === systemId);
    const dynamic = this.ctx.state.systems.find((entry) => entry.id === systemId);
    const body = dynamic?.bodies.find((entry) => entry.id === bodyId);
    if (!system || !dynamic || !body) return false;
    this.resetWorkbench();
    this.worldWorkbench = { systemId, bodyId, origin: routeIdentity(origin) };
    this.worldSignature = "";
    this.ctx.state.selectedSystemId = systemId;
    this.ctx.renderer.pulseSystemBody(String(bodyId));
    this.renderWorldWorkbench(origin, true);
    requestAnimationFrame(() => this.worldWorkbenchPanel.focus());
    return true;
  }

  handleWorldWorkbenchAction(button: HTMLButtonElement, origin: DeckRoute | null): boolean {
    if (!this.worldWorkbench) return false;
    if (button.dataset.deckAct === "world-workbench-close") return this.closeWorldWorkbench();
    const route = this.worldRoute(this.worldWorkbench);
    const handled = this.handleWorldAction(button, route, this.worldWorkbenchRoot);
    if (handled) {
      this.worldSignature = "";
      this.renderWorldWorkbench(origin, true);
    }
    return handled;
  }

  closeWorldWorkbench(): boolean {
    if (!this.worldWorkbench) return false;
    this.resetWorldWorkbench();
    return true;
  }

  closeBuildWorkbench(route: DeckRoute | null): boolean {
    if (!route) {
      const wasOpen = this.workbenchOpen;
      this.resetWorkbench();
      return wasOpen;
    }
    if (!this.workbenchOpen) return false;
    this.workbenchOpen = false;
    this.signature = "";
    if (route.name === "system" || route.name === "build") this.render(route, true);
    else this.resetWorkbench();
    return true;
  }

  teardown(): void {
    this.buildContext = "";
    this.workbenchOpen = false;
    this.resetWorkbench();
    this.resetWorldWorkbench();
  }

  onCore(events: readonly CoreEvent[], route: DeckRoute | null): void {
    if (!this.buildFeedback || !events.some((event) => event.kind === "TimelineApplied")) return;
    const rejected = this.ctx.state.timeline
      .slice(this.buildFeedback.timelineLength)
      .reverse()
      .find((entry) => entry.at_time + 0.001 >= this.buildFeedback!.issuedAt
        && entry.severity === "warn" && entry.text.startsWith("Can't build"));
    if (!rejected) return;
    this.buildFeedback = { ...this.buildFeedback, text: rejected.text, tone: "bad" };
    this.signature = "";
    const onBuildTab = route?.name === "build" || (route?.name === "system" && this.systemTab === "build");
    this.hooks.toast("Build refused", rejected.text, "bad", onBuildTab ? route ?? undefined : undefined);
    if (onBuildTab && route) this.render(route, true);
  }

  private systemContext(route: DeckRoute): { system?: SystemInfo; dynamic?: SystemStateView } {
    const fallback = this.ctx.state.selectedSystemId
      ?? this.ctx.state.systems.find((entry) => entry.owner === this.ctx.state.playerId)?.id;
    const id = route.params?.id ?? route.params?.systemId ?? fallback;
    return {
      system: this.ctx.state.galaxy?.systems.find((entry) => entry.id === id),
      dynamic: this.ctx.state.systems.find((entry) => entry.id === id),
    };
  }

  private systemHtml(system?: SystemInfo, dynamic?: SystemStateView): string {
    if (!system) return emptyState("System unavailable", "Select a star on the galaxy map to inspect its served report.");
    const mine = dynamic?.owner === this.ctx.state.playerId;
    const held = dynamic?.owner !== null && dynamic?.owner !== undefined;
    const surveyed = dynamic?.deposits !== null && dynamic?.deposits !== undefined;
    const owner = mine ? "Owned system" : held ? "Rival-held system" : "Unclaimed system";
    const survey = surveyed ? "survey report received" : `${label(system.band)} spectral band · geology unsurveyed`;
    const star = starTypeFor(system.id);
    const tabs = [
      ["overview", "Overview"], ["worlds", "Worlds"], ["production", "Production"], ["fleets", "Fleets"], ["build", "Build"],
    ].map(([tab, text]) => `<button type="button" data-deck-act="system-tab" data-tab="${tab}" aria-selected="${tab === this.systemTab}">${text}</button>`).join("");
    const alert = dynamic?.blockade
      ? `<div class="deck-alert deck-alert--bad"><b>${dynamic.blockade.by_me ? "Blockade established" : "Under blockade"}</b><span>Physical shipping is interdicted while this report remains current.</span></div>`
      : "";
    const active = this.systemTab === "overview"
      ? this.systemOverview(system, dynamic, mine, owner, survey)
      : this.systemTab === "worlds"
        ? this.systemWorlds(system, dynamic, mine)
        : this.systemTab === "production"
          ? this.systemProduction(system, dynamic, mine)
          : this.systemTab === "fleets"
            ? this.systemFleets(system)
            : this.buildDashboard(dynamic);
    return `<section class="deck-page deck-system"><header class="deck-page__lead deck-system__lead"><img class="deck-system__star" src="${esc(starIconUrl(star))}" alt="${esc(star.title)}"><h2>${esc(system.name)}</h2><span>${esc(systemFlavor(system, dynamic?.deposits ?? null))}</span></header>${alert}<nav class="deck-tabs" aria-label="System sections">${tabs}</nav>${active}</section>`;
  }

  private systemOverview(system: SystemInfo, dynamic: SystemStateView | undefined, mine: boolean, owner: string, survey: string): string {
    const bodies = dynamic?.bodies ?? [];
    const publicStats = [
      stat("Status", owner),
      stat("Survey", survey),
      stat("Worlds", String(bodies.length || "—")),
      stat("Spectrum", label(system.band)),
    ];
    if (!mine || !dynamic) {
      return `<div class="deck-stat-grid">${publicStats.join("")}</div>` +
        `<section class="deck-section"><h3>Known picture</h3><p class="deck-muted">${dynamic?.owner ? "Ownership light has arrived. Economic and workforce reports remain private to that corporation." : "No corporate claim has reached command."}</p></section>` +
        this.groundActivityHtml(dynamic) + this.opportunityHtml(dynamic);
    }
    const workforce = dynamic.workforce;
    const ownerStats = [
      stat("Population", fmtPopulation(dynamic.population)),
      stat("Food", label(dynamic.food_state ?? "well_supplied"), !dynamic.habitat_fed),
      stat("Workforce", workforce ? `${workforce.posted}/${workforce.units}` : "—", !!workforce && workforce.posted > workforce.units),
      stat("Development", `${dynamic.slots_used}/${dynamic.slots_total}`, dynamic.slots_total > 0 && dynamic.slots_used >= dynamic.slots_total),
      stat("Storage", `${fmt(dynamic.storage_used)}/${fmt(dynamic.storage_cap)}`, dynamic.storage_cap > 0 && dynamic.storage_used >= dynamic.storage_cap),
      stat("Construction", String(dynamic.builds.length)),
    ];
    return `<div class="deck-stat-grid">${ownerStats.join("")}</div>${this.groundActivityHtml(dynamic)}${this.opportunityHtml(dynamic)}${this.queueHtml(dynamic)}`;
  }

  private groundActivityHtml(dynamic: SystemStateView | undefined): string {
    if (!dynamic) return "";
    const record = latestGroundRecordFor(dynamic.id);
    if (!record) return "";
    const state = record.outcome === null ? "Landing in progress" : record.outcome === "taken" ? "Ground taken" : "Landing repulsed";
    return `<section class="deck-section deck-ground-activity"><header><div><h3>${esc(state)}</h3><p>${record.outcome === null ? "Follow each round as its light reaches command." : "The arrived ground record can be replayed round by round."}</p></div><b>${record.rounds.length}</b></header><button type="button" class="is-primary" data-deck-act="watch-ground" data-ground="${esc(record.id)}">${record.outcome === null ? "Follow landing · delayed" : "View landing replay"}</button></section>`;
  }

  private systemWorlds(_system: SystemInfo, dynamic: SystemStateView | undefined, mine: boolean): string {
    const bodies = dynamic?.bodies ?? [];
    if (!bodies.length) return emptyState("No world roster", "This system's body catalogue has not reached the client.");
    const outputByBody = new Map<number, Map<Commodity, number>>();
    for (const assignment of dynamic?.assignments ?? []) {
      const totals = outputByBody.get(assignment.body_id) ?? new Map<Commodity, number>();
      for (const [commodity, rate] of assignment.outputs) totals.set(commodity, (totals.get(commodity) ?? 0) + rate);
      outputByBody.set(assignment.body_id, totals);
    }
    const rows = bodies.map((body) => {
      const deposits = body.deposits === null
        ? `<span class="deck-world__deposits deck-muted">Geology unsurveyed</span>`
        : body.deposits.length
          ? `<span class="deck-world__deposits">${body.deposits.map((deposit) => `${commodityGlyph(deposit.resource)} <span>${esc(label(deposit.resource))} <b>×${deposit.richness.toFixed(2)}</b></span>`).join("")}</span>`
          : `<span class="deck-world__deposits deck-muted">Surveyed · no deposits</span>`;
      const outputs = [...(outputByBody.get(body.id)?.entries() ?? [])]
        .filter(([, rate]) => rate > 0.001)
        .map(([commodity, rate]) => `+${rate.toFixed(2)}/s ${esc(label(commodity))}`).join(" · ");
      return `<button type="button" class="deck-world" data-deck-act="open-world" data-body="${body.id}"><span class="deck-world__identity">${planetIcon(body)}<span><b>${esc(body.name)}</b><small>${esc(label(body.size))} · ${esc(label(body.environment))} · ${esc(label(body.kind))}</small></span></span>${deposits}<span class="deck-world__meta">${body.population > 0 ? fmtPopulation(body.population) : body.geology === null ? "awaiting survey" : mine ? "undeveloped" : "observed"}${outputs ? `<small>${outputs}</small>` : ""}</span></button>`;
    }).join("");
    return `<section class="deck-section"><header><div><h3>World roster</h3><p>${mine ? "Open a world for its served economy and management." : "Public astronomy plus survey knowledge; private development stays hidden."}</p></div><b>${bodies.length}</b></header><div class="deck-world-list">${rows}</div></section>`;
  }

  private systemProduction(_system: SystemInfo, dynamic: SystemStateView | undefined, mine: boolean): string {
    if (!mine || !dynamic) return emptyState("Production unavailable", "Stockpiles, workforce and construction are owner-only reports.");
    const available = constructionStock(dynamic).available;
    const manifest = shippableStock(dynamic);
    const stock = [...available.entries()]
      .filter(([, units]) => units > 0)
      .sort(([a], [b]) => label(a).localeCompare(label(b)))
      .map(([commodity, units]) => `<span>${commodityGlyph(commodity)}<small>${esc(label(commodity))}</small><b>${fmt(units)}</b></span>`).join("");
    const shipmentUnits = manifest.reduce((sum, slot) => sum + slot.units, 0);
    const shipment = `<div class="deck-section__action"><button type="button" class="is-primary" data-deck-act="ship-production" ${dynamic.blockade || !manifest.length ? "disabled" : ""}>Ship ${shipmentUnits ? fmt(shipmentUnits) : "no"} goods to Market</button><span>Fuel remains in the system reserve. Pickup, transit and sale are physical and delayed.</span></div>`;
    const assignments = dynamic.assignments.map((line) => this.assignmentHtml(dynamic, line));
    const assigned = new Set(dynamic.assignments.map((line) => `${line.body_id}:${line.structure}`));
    for (const body of dynamic.bodies) {
      for (const [structure, tier] of Object.entries(body.structures)) {
        if (tier <= 0 || !WORKFORCE_STRUCTURES.has(structure) || assigned.has(`${body.id}:${structure}`)) continue;
        assignments.push(this.unassignedAssignmentHtml(body, structure, tier));
      }
    }
    const productionRows = assignments.join("") || `<div class="deck-empty-inline">No production structures in this system.</div>`;
    return `<section class="deck-section"><header><div><h3>Stockpile</h3><p>Local stock plus cargo on docked owned freighters.</p></div><b>${fmt(dynamic.storage_used)}/${fmt(dynamic.storage_cap)}</b></header><div class="deck-ledger">${stock || `<span><small>Stockpile</small><b>empty</b></span>`}</div>${shipment}</section>` +
      `<section class="deck-section"><header><div><h3>Production lines</h3><p>Output = throughput × staffing × expertise × supply × site.</p></div><b>${assignments.length}</b></header><div class="deck-assignment-list">${productionRows}</div></section>` +
      this.queueHtml(dynamic);
  }

  private systemFleets(system: SystemInfo): string {
    // Membership is deliberately the player's SERVED dock report. A departing
    // fleet remains listed until that light reaches command; no client-side
    // position estimate or authoritative server truth corrects the roster.
    const fleets = this.dockedFleets(system);
    if (!fleets.length) return emptyState("No docked fleets", `No owned fleet is reported berthed at ${system.name}.`);
    const rows = fleets.map((fleet) => fleetListRow(fleet, {
      openAction: "system-fleet-open", status: "Docked",
      flagshipName: this.ctx.state.syndicate?.flagship_name,
    })).join("");
    return `<section class="deck-section"><header><div><h3>Docked fleets</h3></div><b>${fleets.length}</b></header><div class="deck-fleet-list">${rows}</div></section>`;
  }

  private dockedFleets(system: SystemInfo) {
    return this.ctx.state.ghosts
      .filter((fleet) => fleet.own && dockedAtSystem(fleet, system.id))
      .sort((a, b) => shipKindLabel(a.kind).localeCompare(shipKindLabel(b.kind)) || a.id.localeCompare(b.id));
  }

  private assignmentHtml(dynamic: SystemStateView, line: AssignmentView): string {
    const body = dynamic.bodies.find((entry) => entry.id === line.body_id);
    const output = line.outputs.length
      ? line.outputs.map(([commodity, rate]) => `+${rate.toFixed(2)} ${label(commodity)}/s`).join(" · ")
      : "Idle";
    return `<article class="deck-assignment${line.suspended ? " is-warn" : ""}"><div><b>${esc(line.title)} · Tier ${line.tier}</b><span>${esc(body?.name ?? "System")}${line.suspended ? ` · ${esc(label(line.suspended))}` : ""}</span><small>${esc(output)}</small></div><div class="deck-stepper" aria-label="Workforce assigned"><button type="button" data-deck-act="set-workers" data-body="${line.body_id}" data-structure="${esc(line.structure)}" data-workers="${Math.max(0, line.workers - 1)}" ${line.workers <= 0 ? "disabled" : ""}>−</button><b>${line.workers}</b><button type="button" data-deck-act="set-workers" data-body="${line.body_id}" data-structure="${esc(line.structure)}" data-workers="${line.workers + 1}">+</button></div></article>`;
  }

  private unassignedAssignmentHtml(body: BodyView, structure: string, tier: number): string {
    return `<article class="deck-assignment is-warn"><div><b>${esc(label(structure))} · Tier ${tier}</b><span>${esc(body.name)} · needs workforce</span><small>Idle</small></div><div class="deck-stepper" aria-label="Workforce assigned"><button type="button" disabled>−</button><b>0</b><button type="button" data-deck-act="set-workers" data-body="${body.id}" data-structure="${esc(structure)}" data-workers="1">+</button></div></article>`;
  }

  private queueHtml(dynamic: SystemStateView): string {
    const now = liveSimTime();
    const rows = buildsByPlanet(dynamic.builds).map(({ bodyId, jobs }) => {
      const body = dynamic.bodies.find((entry) => entry.id === bodyId);
      const builds = jobs.map((job) => {
        const duration = job.complete_time == null ? null : Math.max(0, job.complete_time - now);
        const progress = buildProgress(job, now);
        const bar = progress == null ? "" : `<span class="deck-queue-progress" role="progressbar" aria-label="${esc(buildName(job.key))} build progression" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${progress.toFixed(0)}"><i style="width:${progress.toFixed(1)}%"></i></span>`;
        const status = job.queued ? "Queued" : duration == null ? "Paused · needs workforce" : duration > 0 ? `Building · ${fmtBuildDur(duration)}` : "completing";
        return `<div class="deck-queue-row${job.queued ? " is-queued" : ""}"><span>${icon("queue", "sm")}<span><b>${esc(buildName(job.key))}</b>${bar}</span></span><em>${status}</em></div>`;
      }).join("");
      return `<div class="deck-planet-queue"><h4>${esc(body?.name ?? "System yard")}</h4>${builds}</div>`;
    }).join("");
    const feedback = this.buildFeedback?.systemId === dynamic.id && this.buildFeedback.text
      ? `<div class="deck-build-feedback is-${this.buildFeedback.tone}">${esc(this.buildFeedback.text)}</div>`
      : "";
    const queueCount = rows ? `<b>${dynamic.builds.length}</b>` : "";
    return `<section class="deck-section"><header><div><h3>Construction queue</h3></div>${queueCount}</header>${feedback}${rows || `<div class="deck-empty-inline">Empty</div>`}</section>`;
  }

  private handleBuildAction(button: HTMLButtonElement, route: DeckRoute): boolean {
    const { system, dynamic } = this.systemContext(route);
    if (!system || !dynamic || dynamic.owner !== this.ctx.state.playerId) return false;
    const action = button.dataset.deckAct;
    if (action === "builder-mode") {
      const mode = button.dataset.mode;
      if (mode === "structures" || mode === "ships" || mode === "modules") {
        this.builderMode = mode;
        this.hooks.go({ ...route, query: { ...(route.query ?? {}), mode } });
      }
      return true;
    } else if (action === "builder-body") {
      const body = dynamic.bodies.find((entry) => String(entry.id) === button.dataset.body);
      if (body) this.hooks.go({ ...route, query: { ...(route.query ?? {}), body: String(body.id) } });
      return true;
    } else if (action === "builder-select") {
      this.selectedBuild = button.dataset.key ?? "";
    } else if (action === "builder-select-hull") {
      const hull = button.dataset.hull as ShipKind | undefined;
      if (hull && SHIP_KEYS.has(hull)) {
        this.selectedHull = hull;
        this.shipQuantity = 1;
      }
    } else if (action === "builder-quantity") {
      this.shipQuantity = Math.max(1, Number(button.dataset.quantity) || 1);
    } else if (action === "builder-fit") {
      const module = button.dataset.module as ModuleKind | undefined;
      if (module && MODULES.some((entry) => entry.kind === module)) {
        const index = this.pendingFit.indexOf(module);
        if (index >= 0) this.pendingFit.splice(index, 1);
        else this.pendingFit.push(module);
      }
    } else if (action === "builder-fit-pick") {
      const fit = (this.ctx.state.syndicate?.fits ?? []).find((entry) => entry.name === button.dataset.name);
      if (fit && (!this.selectedHull || fit.kind === this.selectedHull)) this.pendingFit = [...fit.modules];
    } else if (action === "builder-fit-delete") {
      const name = button.dataset.name;
      if (name) this.ctx.send({ type: "DeleteFit", name });
    } else if (action === "builder-fit-save") {
      if (!this.selectedHull) return true;
      const input = this.root.querySelector<HTMLInputElement>("#deck-fit-name");
      const name = input?.value.trim();
      const fit = this.effectiveFit(dynamic.id, this.selectedHull);
      if (name && fit.length && fitLegal(this.selectedHull, fit)) {
        this.ctx.send({ type: "SaveFit", name, ship: this.selectedHull, loadout: fit });
        this.hooks.notice(`<b>Doctrine fit sent</b> · ${esc(name)}.`);
      }
    } else if (action === "builder-forge") {
      const module = button.dataset.module as ModuleKind | undefined;
      if (module && MODULES.some((entry) => entry.kind === module)) this.dispatchBuild(`module:${module}`, dynamic.id);
    } else if (action === "builder-queue") {
      const body = this.builderBody(route, dynamic);
      if (body && this.selectedBuild) this.dispatchBuild(this.selectedBuild, dynamic.id, body.id);
    } else if (action === "builder-queue-ships") {
      if (!this.selectedHull) return true;
      const option = buildOption(this.selectedHull);
      const state = option ? shipOption(option, dynamic) : null;
      const fit = this.effectiveFit(dynamic.id, this.selectedHull);
      const quantity = Math.min(this.shipQuantity, state?.maxAff ?? 0);
      if (!state?.buildable || quantity < 1 || (fit.length > 0 && !fitLegal(this.selectedHull, fit))) return true;
      for (let index = 0; index < quantity; index++) dispatchBuildKey(this.selectedHull, dynamic.id);
      this.noteBuildDispatch(dynamic.id, `${quantity}× ${buildName(this.selectedHull)} sent to the yard.`);
      this.shipQuantity = 1;
    } else {
      return false;
    }
    this.signature = "";
    this.render(route, true);
    return true;
  }

  private handleWorldAction(button: HTMLButtonElement, route: DeckRoute, actionRoot = this.root): boolean {
    const { system, dynamic } = this.systemContext(route);
    const body = dynamic?.bodies.find((entry) => String(entry.id) === route.params?.bodyId);
    if (!system || !dynamic || !body) return false;
    const action = button.dataset.deckAct;
    if (action === "world-build") {
      this.resetWorldWorkbench();
      this.hooks.go({
        name: "build",
        params: { systemId: system.id, systemLabel: system.name },
        query: { body: String(body.id), mode: button.dataset.mode === "ships" ? "ships" : "structures" },
      });
      return true;
    }
    if (action === "set-workers") {
      if (dynamic.owner !== this.ctx.state.playerId) return true;
      const structure = button.dataset.structure;
      const workers = Math.max(0, Number(button.dataset.workers) || 0);
      const assignment = dynamic.assignments.find((entry) => entry.body_id === body.id && entry.structure === structure);
      if (structure) {
        this.ctx.send({ type: "SetAssignment", system_id: system.id, body_id: body.id, structure, workers, specialists: assignment?.specialists ?? {} });
        this.hooks.notice(`<b>Workforce directive sent</b> · ${esc(label(structure))} → ${workers}.`);
      }
      return true;
    }
    if (action === "migration-policy") {
      const policy = button.dataset.policy;
      if (dynamic.owner === this.ctx.state.playerId && isMigrationPolicy(policy)) {
        this.ctx.send({ type: "SetMigrationPolicy", system_id: system.id, body_id: body.id, policy });
        this.hooks.notice(`<b>Migration policy sent</b> · ${esc(label(policy))}.`);
      }
      return true;
    }
    if (action === "relocate-migrants") {
      const select = actionRoot.querySelector<HTMLSelectElement>("[data-deck-relocation-target]");
      const [toSystem, toBodyText] = select?.value.split("|") ?? [];
      const toBody = Number(toBodyText);
      if (dynamic.owner === this.ctx.state.playerId && toSystem && Number.isFinite(toBody)) {
        this.ctx.send({ type: "RelocateMigrants", from_system: system.id, from_body: body.id, to_system: toSystem, to_body: toBody });
        this.hooks.notice(`<b>Migrant liner requested</b> · ${esc(body.name)} → ${esc(systemName(this.ctx, toSystem))}.`);
      }
      return true;
    }
    return false;
  }

  private buildHtml(route: DeckRoute, system?: SystemInfo, dynamic?: SystemStateView): string {
    if (!system || !dynamic) return emptyState("Build context unavailable", "Open an owned system before entering construction.");
    if (dynamic.owner !== this.ctx.state.playerId) return emptyState("Construction is private", "You can inspect this system, but only its owner receives production and build controls.");
    if (route.query?.mode === "ships" || route.query?.mode === "structures" || route.query?.mode === "modules") this.builderMode = route.query.mode;
    const preset = `${dynamic.id}:${route.query?.body ?? ""}:${route.query?.mode ?? ""}:${route.query?.select ?? ""}`;
    if (route.query?.select && preset !== this.lastBuildPreset) {
      if (SHIP_KEYS.has(route.query.select)) this.selectedHull = route.query.select as ShipKind;
      else this.selectedBuild = route.query.select;
      this.lastBuildPreset = preset;
    }
    const modeTabs = (["structures", "ships", "modules"] as BuilderMode[]).map((mode) => `<button type="button" data-deck-act="builder-mode" data-mode="${mode}" aria-selected="${this.builderMode === mode}">${mode === "structures" ? "Structures" : mode === "ships" ? "Ships" : "Modules"}</button>`).join("");
    if (this.builderMode === "modules") {
      return `<section class="deck-page deck-build"><nav class="deck-tabs" aria-label="Build category">${modeTabs}</nav>${this.moduleForge(dynamic)}</section>`;
    }
    const body = this.builderBody(route, dynamic);
    const sites = this.builderMode === "ships" ? dynamic.bodies.filter(isShipbuildingBody) : dynamic.bodies;
    if (!body) {
      const unavailable = this.builderMode === "ships"
        ? emptyState("No shipyard", "Build a Shipyard on a planet before ordering hulls here.")
        : emptyState("No build site", "No served planet is available in this system.");
      return `<section class="deck-page deck-build"><nav class="deck-tabs" aria-label="Build category">${modeTabs}</nav>${unavailable}</section>`;
    }
    const worlds = sites.map((entry) => `<button type="button" data-deck-act="builder-body" data-body="${entry.id}" aria-selected="${entry.id === body.id}">${esc(entry.name)}</button>`).join("");
    const content = this.builderMode === "structures"
      ? this.structureBuilder(dynamic, body)
      : this.shipBuilder(dynamic, body);
    return `<section class="deck-page deck-build"><nav class="deck-tabs" aria-label="Build category">${modeTabs}</nav><div class="deck-builder-worlds"><span>Build site</span>${worlds}</div>${content}</section>`;
  }

  private buildDashboard(dynamic?: SystemStateView): string {
    if (!dynamic) return emptyState("Construction unavailable", "The served system ledger has not arrived.");
    return this.queueHtml(dynamic);
  }

  private renderWorkbench(route: DeckRoute, system?: SystemInfo, dynamic?: SystemStateView): void {
    const visible = this.workbenchOpen && this.systemTab === "build" && route.name !== "world" && !!system && !!dynamic;
    this.workbenchPanel.hidden = !visible;
    this.workbenchPanel.setAttribute("aria-hidden", String(!visible));
    if (!visible) {
      setHtml(this.workbenchRoot, "");
      return;
    }
    const title = this.workbenchPanel.querySelector<HTMLElement>("#deck-build-workbench-title");
    if (title) title.textContent = `${system.name} Construction`;
    setHtml(this.workbenchRoot, this.buildHtml(route, system, dynamic));
  }

  private renderWorldWorkbench(origin: DeckRoute | null, force: boolean): void {
    const context = this.worldWorkbench;
    if (!context || context.origin !== routeIdentity(origin)) {
      if (context) this.resetWorldWorkbench();
      return;
    }
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === context.systemId);
    const dynamic = this.ctx.state.systems.find((entry) => entry.id === context.systemId);
    const body = dynamic?.bodies.find((entry) => entry.id === context.bodyId);
    const signature = sheetFingerprint([context, system, dynamic, this.ctx.state.systems]);
    if (!force && signature === this.worldSignature) return;
    if (renderDeferred(this.worldWorkbenchRoot.id, () => this.renderWorldWorkbench(origin, true))) return;
    this.worldSignature = signature;
    this.worldWorkbenchPanel.hidden = false;
    this.worldWorkbenchPanel.setAttribute("aria-hidden", "false");
    const title = this.worldWorkbenchPanel.querySelector<HTMLElement>("#deck-world-workbench-title");
    if (title) title.textContent = body && system ? `${body.name} · ${system.name}` : "World details";
    const artwork = this.worldWorkbenchPanel.querySelector<HTMLImageElement>("#deck-world-workbench-art");
    if (artwork) {
      artwork.hidden = !body || !system;
      artwork.alt = body ? `${body.name} world` : "";
      if (body && system) artwork.src = bodyArtUrl(system.id, body);
      else artwork.removeAttribute("src");
    }
    if (!system || !dynamic || !body) {
      setHtml(this.worldWorkbenchRoot, emptyState("World report unavailable", "The served body record is no longer available."));
      return;
    }
    setHtml(this.worldWorkbenchRoot, this.worldHtml(this.worldRoute(context), system, dynamic, false));
  }

  private resetWorkbench(): void {
    this.buildContext = "";
    this.workbenchOpen = false;
    this.workbenchPanel.hidden = true;
    this.workbenchPanel.setAttribute("aria-hidden", "true");
    setHtml(this.workbenchRoot, "");
  }

  private resetWorldWorkbench(): void {
    this.worldWorkbench = null;
    this.worldSignature = "";
    this.worldWorkbenchPanel.hidden = true;
    this.worldWorkbenchPanel.setAttribute("aria-hidden", "true");
    setHtml(this.worldWorkbenchRoot, "");
  }

  private worldRoute(context: WorldWorkbenchContext): DeckRoute {
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === context.systemId);
    const body = this.ctx.state.systems.find((entry) => entry.id === context.systemId)?.bodies.find((entry) => entry.id === context.bodyId);
    return {
      name: "world",
      params: {
        systemId: context.systemId,
        systemLabel: system?.name ?? "System",
        bodyId: String(context.bodyId),
        worldLabel: body?.name ?? "World",
      },
    };
  }

  private get workbenchPanel(): HTMLElement {
    return this.workbenchRoot.closest("#deck-build-workbench") as HTMLElement;
  }

  private get worldWorkbenchPanel(): HTMLElement {
    return this.worldWorkbenchRoot.closest("#deck-world-workbench") as HTMLElement;
  }

  private builderBody(route: DeckRoute, dynamic: SystemStateView): BodyView | undefined {
    const requested = route.query?.body;
    const explicit = dynamic.bodies.find((entry) => String(entry.id) === requested);
    if (explicit && (this.builderMode !== "ships" || isShipbuildingBody(explicit))) return explicit;
    if (this.builderMode === "ships") return bestShipyardBody(dynamic);
    return bestStructureBody(dynamic, this.selectedBuild) ?? dynamic.bodies[0];
  }

  private worldHtml(route: DeckRoute, system?: SystemInfo, dynamic?: SystemStateView, includeLead = true): string {
    const body = dynamic?.bodies.find((entry) => String(entry.id) === route.params?.bodyId);
    if (!system || !dynamic || !body) return emptyState("World report unavailable", "The requested served body record is not in this system report.");
    const mine = dynamic.owner === this.ctx.state.playerId;
    const profile = `<div class="deck-stat-grid">${stat("Environment", label(body.environment))}${stat("Size", label(body.size))}${stat("Geology", body.geology ? label(body.geology) : "Unsurveyed")}${stat("Construction", `×${body.construction_time_mult.toFixed(2)}`)}${stat("Habitat", `×${body.habitat_capacity_mult.toFixed(2)}`)}${stat("Settlement", `×${body.population_growth_mult.toFixed(2)}`)}</div>`;
    const feature = body.special
      ? `<article class="deck-world-feature"><small>Rare planetary feature</small><b>${esc(label(body.special))}</b><span>${esc(body.special_effect ?? "Rare planetary feature")}</span></article>`
      : "";
    const deposits = body.deposits === null
      ? `<div class="deck-empty-inline">Geology has not been surveyed.</div>`
      : body.deposits.length
        ? body.deposits.map((deposit) => `<div class="deck-deposit"><span>${commodityGlyph(deposit.resource)}<span><b>${esc(label(deposit.resource))}</b><small>${deposit.reserves === null ? "Renewable deposit" : `${fmt(deposit.reserves)} reserves`}</small></span></span><em>×${deposit.richness.toFixed(2)}</em></div>`).join("")
        : `<div class="deck-empty-inline">Surveyed · no extractable deposits.</div>`;
    const roles = (dynamic.opportunities ?? []).filter((entry) => entry.body_id === body.id);
    const roleHtml = roles.length ? `<div class="deck-opportunities">${roles.map((entry) => `<article class="deck-opportunity is-${entry.tier}"><small>${esc(label(entry.tier))}</small><b>${esc(entry.title)} · ×${entry.score.toFixed(2)}</b><span>${esc(entry.reason)}</span></article>`).join("")}</div>` : "";
    const publicSections = `${this.colonyPurposeHtml(dynamic, body)}<section class="deck-section"><header><div><h3>World summary</h3><p>Public astronomy; mineral grade and deposits require arrived survey light.</p></div></header>${profile}${feature}${roleHtml}</section><section class="deck-section"><header><div><h3>Deposits</h3><p>Natural site richness before staffing, research and structures.</p></div></header>${deposits}</section>`;
    if (!mine) {
      const lead = includeLead ? `<header class="deck-page__lead"><span>${body.habitable ? "Habitable world" : "Observed world"}</span><h2>${esc(body.name)}</h2><p>${esc(system.name)} · private economy hidden</p></header>` : "";
      return `<section class="deck-page deck-world-page">${lead}${publicSections}</section>`;
    }
    const economy = this.worldEconomy(dynamic, body);
    const population = this.worldPopulation(system.id, body);
    const structures = Object.entries(body.structures).filter(([, tier]) => tier > 0).map(([key, tier]) => `<span>${icon(structureIcon(key), "sm")}<small>${esc(label(key))}</small><b>Tier ${tier}</b></span>`).join("");
    const pools = bodyPoolUsage(body, dynamic);
    const poolLine = (["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span class="deck-stat${pools[pool].used >= pools[pool].total ? " is-warn" : ""}"><small>${POOL_LABEL[pool]} slots</small><b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("");
    const buildActions = `<div class="deck-world-actions"><button type="button" class="is-primary" data-deck-act="world-build" data-mode="structures">Build structure</button>${isShipbuildingBody(body) ? `<button type="button" data-deck-act="world-build" data-mode="ships">Build ship</button>` : ""}</div>`;
    const lead = includeLead ? `<header class="deck-page__lead"><span>Owned world · served management</span><h2>${esc(body.name)}</h2><p>${esc(system.name)} · ${fmtPopulation(body.population)}</p></header>` : "";
    return `<section class="deck-page deck-world-page">${lead}${publicSections}<section class="deck-section"><header><div><h3>Economy</h3><p>Structures, outputs and posted workforce on this world.</p></div></header><div class="deck-ledger">${structures || `<span><small>Development</small><b>none</b></span>`}</div>${economy}</section><section class="deck-section"><header><div><h3>Population</h3><p>Migration is physical; policy directs future Authority allocations.</p></div></header>${population}</section><section class="deck-section"><header><div><h3>Development</h3><p>New structures use a world slot; later tiers deepen in place.</p></div></header><div class="deck-stat-grid">${poolLine}</div>${buildActions}</section></section>`;
  }

  private worldEconomy(dynamic: SystemStateView, body: BodyView): string {
    const assignments = dynamic.assignments.filter((entry) => entry.body_id === body.id);
    const assigned = new Set(assignments.map((entry) => entry.structure));
    const rows = assignments.map((entry) => this.assignmentHtml(dynamic, entry));
    for (const [structure, tier] of Object.entries(body.structures)) {
      if (tier <= 0 || assigned.has(structure) || !WORKFORCE_STRUCTURES.has(structure)) continue;
      rows.push(this.unassignedAssignmentHtml(body, structure, tier));
    }
    return `<div class="deck-assignment-list">${rows.join("") || `<div class="deck-empty-inline">No production structures on this world.</div>`}</div>`;
  }

  private worldPopulation(systemId: string, body: BodyView): string {
    const policyButtons = (["closed", "managed", "open", "priority"] as const).map((policy) => `<button type="button" data-deck-act="migration-policy" data-policy="${policy}" aria-pressed="${body.migration_policy === policy}">${esc(label(policy))}</button>`).join("");
    const cohort = this.ctx.state.galaxy?.migrant_cohort_people ?? 1_000;
    const options = this.ctx.state.systems
      .filter((entry) => entry.owner === this.ctx.state.playerId && !entry.blockade && entry.habitat_fed)
      .flatMap((entry) => entry.bodies.filter((candidate) => !(entry.id === systemId && candidate.id === body.id) && candidate.population > 0).map((candidate) => `<option value="${esc(entry.id)}|${candidate.id}">${esc(systemName(this.ctx, entry.id))} · ${esc(candidate.name)}</option>`)).join("");
    return `<div class="deck-stat-grid">${stat("Population", fmtPopulation(body.population))}${stat("Inbound", `${fmt(body.inbound_migrants)} people`)}${stat("Food use", `×${body.provisions_mult.toFixed(2)}`)}${stat("Settlement appeal", `×${body.population_growth_mult.toFixed(2)}`)}</div><div class="deck-policy"><span>Immigration policy</span>${policyButtons}</div><div class="deck-inline-form"><select data-deck-relocation-target>${options}</select><button type="button" data-deck-act="relocate-migrants" ${options && body.population * 1_000_000 >= cohort * 2 ? "" : "disabled"}>Relocate ${cohort.toLocaleString()}</button></div>`;
  }

  private structureBuilder(dynamic: SystemStateView, body: BodyView): string {
    const pools = bodyPoolUsage(body, dynamic);
    const options = (this.ctx.state.galaxy?.build_options ?? [])
      .filter((entry) => !SHIP_KEYS.has(entry.key) && !entry.key.startsWith("module:") && !!POOL_OF[entry.key]) as BuildOpt[];
    if (!options.some((entry) => entry.key === this.selectedBuild)) this.selectedBuild = options[0]?.key ?? "";
    const poolBars = (["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => {
      const usage = pools[pool];
      const pct = usage.total > 0 ? Math.min(100, usage.used / usage.total * 100) : 100;
      return `<span class="deck-pool${usage.used >= usage.total ? " is-full" : ""}"><small>${POOL_LABEL[pool]}</small><b>${usage.used}/${usage.total}</b><i><span style="width:${pct.toFixed(1)}%"></span></i></span>`;
    }).join("");
    const groups = (["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => {
      // Catalog placement only: Shipyard leads Infrastructure, but still uses
      // its Industrial slot and recipe through structOption / POOL_OF.
      const entries = options.filter((entry) => (entry.key === "shipyard" ? "infrastructure" : POOL_OF[entry.key]) === pool);
      if (pool === "infrastructure") entries.sort((a, b) => Number(b.key === "shipyard") - Number(a.key === "shipyard"));
      const rows = entries.map((entry) => {
        const state = structOption(entry, dynamic, body, pools);
        return `<button type="button" class="deck-builder-row${entry.key === this.selectedBuild ? " is-selected" : ""}${state.buildable ? "" : " is-disabled"}" data-deck-act="builder-select" data-key="${esc(entry.key)}"><span>${icon(structureIcon(entry.key), "sm", undefined, "deck-structure-icon")}<b>${esc(entry.label)}</b></span></button>`;
      }).join("");
      return rows ? `<h3 class="deck-builder-group">${POOL_LABEL[pool]}</h3>${rows}` : "";
    }).join("");
    const selected = options.find((entry) => entry.key === this.selectedBuild);
    const detail = selected ? this.structureDetail(dynamic, body, selected, pools) : emptyState("Choose a structure", "Inspect its recipe, slot and build time.");
    const commit = selected ? this.structureCommit(dynamic, body, selected, pools) : "";
    return `<div class="deck-pools">${poolBars}</div><div class="deck-builder">${commit}<div class="deck-builder__list">${groups}</div><div class="deck-builder__detail">${detail}</div></div>`;
  }

  private structureCommit(dynamic: SystemStateView, body: BodyView, option: BuildOpt, pools: ReturnType<typeof bodyPoolUsage>): string {
    const state = structOption(option, dynamic, body, pools);
    return `<div class="deck-builder__commit"><span><small>Selected structure</small><b>${esc(option.label)}</b></span><button type="button" class="is-primary" data-deck-act="builder-queue" ${state.buildable ? "" : "disabled"}>Queue build</button></div>`;
  }

  private structureDetail(dynamic: SystemStateView, body: BodyView, option: BuildOpt, pools: ReturnType<typeof bodyPoolUsage>): string {
    const state = structOption(option, dynamic, body, pools);
    const supply = constructionStock(dynamic).available;
    const costs = option.costs.map((cost) => {
      const commodity = cost.commodity as Commodity;
      const have = supply.get(commodity) ?? 0;
      return `<tr${have < cost.units ? ` class="is-short"` : ""}><th scope="row"><span>${commodityGlyph(commodity)} ${esc(label(commodity))}</span></th><td>${cost.units}</td><td>${fmt(have)}</td></tr>`;
    }).join("");
    const category = state.foundsNew ? POOL_LABEL[state.pool] : `${POOL_LABEL[state.pool]} · upgrade to tier ${state.targetTier}`;
    const description = STRUCTURE_DESCRIPTION[option.key] ?? "Adds a new capability to this world.";
    return `<article class="deck-build-detail"><header>${icon(structureIcon(option.key), "md", undefined, "deck-structure-icon")}<span><small>${esc(category)}</small><h3>${esc(option.label)}</h3></span></header><p>${esc(description)}</p><table class="deck-cost-table" aria-label="Construction requirements"><thead><tr><th aria-label="Resource"></th><th scope="col">Cost</th><th scope="col">Stock</th></tr></thead><tbody>${costs}</tbody></table><dl><div><dt>Build time</dt><dd>${fmtBuildDur(option.build_secs * body.construction_time_mult)}</dd></div><div><dt>Slot</dt><dd>${state.foundsNew ? `${POOL_LABEL[state.pool]} ${pools[state.pool].used} → ${pools[state.pool].used + 1}/${pools[state.pool].total}` : "Deepens in place"}</dd></div></dl>${state.reason ? `<div class="deck-build-warning">${esc(state.reason)}</div>` : ""}</article>`;
  }

  private shipBuilder(dynamic: SystemStateView, body: BodyView): string {
    const options = SHIP_ORDER.map((kind) => buildOption(kind)).filter((entry): entry is BuildOpt => !!entry);
    if (!this.selectedHull || !options.some((entry) => entry.key === this.selectedHull)) this.selectedHull = options[0]?.key as ShipKind ?? "";
    const rows = options.map((entry) => {
      const state = shipOption(entry, dynamic);
      return `<button type="button" class="deck-builder-row${entry.key === this.selectedHull ? " is-selected" : ""}${state.buildable ? "" : " is-disabled"}" data-deck-act="builder-select-hull" data-hull="${entry.key}"><span>${icon(shipIcon(entry.key), "md", undefined, "deck-hull-icon")}<b>${esc(buildName(entry.key))}</b></span></button>`;
    }).join("");
    const selected = this.selectedHull ? options.find((entry) => entry.key === this.selectedHull) : undefined;
    const detail = selected ? this.shipDetail(dynamic, body, selected) : emptyState("Choose a hull", "Inspect its recipe, capability and fitting budget.");
    const commit = selected ? this.shipCommit(dynamic, selected) : "";
    return `<div class="deck-builder">${commit}<div class="deck-builder__list"><h3 class="deck-builder-group">Hull catalogue</h3>${rows}</div><div class="deck-builder__detail">${detail}</div></div>`;
  }

  private shipCommit(dynamic: SystemStateView, option: BuildOpt): string {
    const hull = option.key as ShipKind;
    const state = shipOption(option, dynamic);
    const max = Math.max(1, state.maxAff);
    const quantity = Math.min(Math.max(1, this.shipQuantity), max);
    const fitOkay = fitLegal(hull, this.effectiveFit(dynamic.id, hull));
    const canQueue = state.buildable && quantity <= state.maxAff && fitOkay;
    return `<div class="deck-builder__commit"><span><small>Selected hull</small><b>${esc(buildName(hull))} ×${quantity}</b></span><button type="button" class="is-primary" data-deck-act="builder-queue-ships" ${canQueue ? "" : "disabled"}>Queue ${quantity} hull${quantity === 1 ? "" : "s"}</button></div>`;
  }

  private shipDetail(dynamic: SystemStateView, body: BodyView, option: BuildOpt): string {
    const hull = option.key as ShipKind;
    const state = shipOption(option, dynamic);
    const stats = SHIP_STATS[hull];
    const max = Math.max(1, state.maxAff);
    const quantity = Math.min(Math.max(1, this.shipQuantity), max);
    this.shipQuantity = quantity;
    const supply = constructionStock(dynamic).available;
    const costs = option.costs.map((cost) => {
      const commodity = cost.commodity as Commodity;
      const need = cost.units * quantity;
      const have = supply.get(commodity) ?? 0;
      return `<div class="deck-cost${have < need ? " is-short" : ""}"><span>${commodityGlyph(commodity)} ${esc(label(commodity))}</span><b>${need} <small>${quantity > 1 ? `${cost.units}×${quantity} · ` : ""}have ${fmt(have)}</small></b></div>`;
    }).join("");
    const quantities = [...new Set([1, 5, 10, max])].filter((value) => value <= max).map((value) => `<button type="button" data-deck-act="builder-quantity" data-quantity="${value}" aria-selected="${quantity === value}">${value === max && max > 10 ? `Max ${value}` : value}</button>`).join("");
    const fit = this.fitPicker(dynamic, hull);
    const fitOkay = fitLegal(hull, this.effectiveFit(dynamic.id, hull));
    const gate = SHIP_YARD[hull] ?? { yard: "shipyard", tier: 1 };
    const siteTime = (state.yardBody ?? body).ship_build_time_mult ?? 1;
    const buildTime = state.buildRate > 0 ? fmtBuildDur(option.build_secs * siteTime / state.buildRate) : "Awaiting workforce";
    return `<article class="deck-build-detail"><header>${icon(shipIcon(hull), "lg", undefined, "deck-hull-icon")}<span><small>${esc(YARD_TITLE[gate.yard] ?? label(gate.yard))} · Tier ${gate.tier}</small><h3>${esc(buildName(hull))}</h3></span></header><p>${esc(stats?.role ?? "Fleet hull")}</p><div class="deck-quantity"><span>Quantity</span>${quantities}</div><div class="deck-costs">${costs}</div><dl><div><dt>Build time</dt><dd>${buildTime}</dd></div><div><dt>Site</dt><dd>${esc((state.yardBody ?? body).name)} · ×${siteTime.toFixed(2)}</dd></div></dl>${stats ? `<div class="deck-hull-stats">${stat("Speed", fmt(stats.speed))}${stat("Hull", fmt(stats.hull))}${stat("Attack", fmt(stats.atk))}${stat("Defense", fmt(stats.def))}</div>` : ""}${fit}${state.reason ? `<div class="deck-build-warning">${esc(state.reason)}</div>` : ""}${!fitOkay ? `<div class="deck-build-warning">The composed fit exceeds this hull's fitting budget or module slots.</div>` : ""}</article>`;
  }

  private fitPicker(dynamic: SystemStateView, hull: ShipKind): string {
    const ledger = moduleLedgerAt(dynamic.id);
    this.pendingFit = this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0);
    const slots = MODULE_SLOTS[hull] ?? 0;
    if (!slots) return `<section class="deck-fit"><h4>Fitting</h4><span class="deck-muted">This hull has no module slots.</span></section>`;
    const available = MODULES.filter((entry) => (ledger[entry.kind] ?? 0) > 0);
    const chips = available.map((entry) => `<button type="button" data-deck-act="builder-fit" data-module="${entry.kind}" aria-pressed="${this.pendingFit.includes(entry.kind)}">${icon(entry.icon, "sm")} ${esc(entry.label)} · ${ledger[entry.kind]}</button>`).join("");
    const effective = this.effectiveFit(dynamic.id, hull);
    const used = effective.reduce((sum, module) => sum + (MODULES.find((entry) => entry.kind === module)?.fit ?? 0), 0);
    const total = FITTING_POINTS[hull] ?? 0;
    const pct = total > 0 ? Math.min(100, used / total * 100) : 0;
    const saved = (this.ctx.state.syndicate?.fits ?? []).filter((entry) => entry.kind === hull).map((entry) => `<span class="deck-saved-fit"><button type="button" data-deck-act="builder-fit-pick" data-name="${esc(entry.name)}">${esc(entry.name)}</button><button type="button" data-deck-act="builder-fit-delete" data-name="${esc(entry.name)}" aria-label="Delete ${esc(entry.name)}">×</button></span>`).join("");
    return `<section class="deck-fit"><h4>Fit next build · ${effective.length}/${slots} slots</h4><div class="deck-fit-bar${used > total ? " is-over" : ""}"><span style="width:${pct.toFixed(1)}%"></span><b>${used}/${total} pts</b></div><div class="deck-fit-chips">${chips || `<span class="deck-muted">No modules in this system ledger.</span>`}</div>${this.ctx.state.syndicate ? `<div class="deck-saved-fits">${saved || `<span class="deck-muted">No saved fits for this hull.</span>`}</div><div class="deck-inline-form"><input id="deck-fit-name" data-deck-enter="builder-fit-save" maxlength="24" placeholder="Doctrine fit name"><button type="button" data-deck-act="builder-fit-save" ${effective.length && fitLegal(hull, effective) ? "" : "disabled"}>Save fit</button></div>` : ""}</section>`;
  }

  private moduleForge(dynamic: SystemStateView): string {
    const hasForge = (dynamic.structures.armaments_complex ?? 0) > 0;
    const supply = constructionStock(dynamic).available;
    const ledger = moduleLedgerAt(dynamic.id);
    const rows = MODULES.map((entry) => {
      const recipe = buildOption(`module:${entry.kind}`);
      const affordable = !!recipe && recipe.costs.every((cost) => (supply.get(cost.commodity as Commodity) ?? 0) >= cost.units);
      const cost = recipe?.costs.map((part) => `${part.units} ${label(part.commodity)}`).join(" · ") ?? "Recipe unavailable";
      return `<div class="deck-forge-row"><span>${icon(entry.icon, "sm")}<span><b>${esc(entry.label)}</b><small>Ledger ${ledger[entry.kind] ?? 0} · ${esc(cost)}</small></span></span><button type="button" data-deck-act="builder-forge" data-module="${entry.kind}" ${hasForge && affordable ? "" : "disabled"}>Forge</button></div>`;
    }).join("");
    return `<section class="deck-module-forge"><h4>Module forge</h4>${hasForge ? "" : `<p class="deck-muted">Build an Armaments Complex to manufacture modules here.</p>`}${rows}</section>`;
  }

  private effectiveFit(systemId: string, hull: ShipKind): ModuleKind[] {
    const ledger = moduleLedgerAt(systemId);
    return this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0).slice(0, MODULE_SLOTS[hull] ?? 0);
  }

  private dispatchBuild(key: string, systemId: string, bodyId?: number): void {
    dispatchBuildKey(key, systemId, bodyId);
    this.noteBuildDispatch(systemId);
  }

  private noteBuildDispatch(systemId: string, text = ""): void {
    this.buildFeedback = {
      systemId,
      issuedAt: this.ctx.state.simTime,
      timelineLength: this.ctx.state.timeline.length,
      text,
      tone: "info",
    };
    if (text) this.hooks.notice(`<b>Build order sent</b> · ${esc(text)}`);
  }

  private opportunityHtml(dynamic?: SystemStateView): string {
    const opportunities = dynamic?.opportunities ?? [];
    if (!opportunities.length) return dynamic ? this.colonyPurposeHtml(dynamic) : "";
    const cards = opportunities.slice(0, 3).map((entry) => `<article class="deck-opportunity is-${entry.tier}"><small>${esc(label(entry.tier))}</small><b>${esc(entry.title)} · ×${entry.score.toFixed(2)}</b><span>${entry.body_name ? `${esc(entry.body_name)} · ` : ""}${esc(entry.reason)}</span></article>`).join("");
    return `${dynamic ? this.colonyPurposeHtml(dynamic) : ""}<section class="deck-section"><header><div><h3>Surveyed opportunities</h3></div></header><div class="deck-opportunities">${cards}</div></section>`;
  }

  private colonyPurposeHtml(candidate: SystemStateView, body?: BodyView): string {
    const st = this.ctx.state;
    const homeId = st.galaxy?.systems.find(s => st.commandCenter && Math.hypot(
      s.pos.x - st.commandCenter.x, s.pos.y - st.commandCenter.y) < 1)?.id;
    if (candidate.id === homeId) return "";
    const home = st.systems.find(s => s.id === homeId && s.owner === st.playerId);
    const purpose = colonyPurpose(candidate, home, body);
    if (!purpose) return "";
    const goods = (items: Commodity[]) => items.map(g => `${commodityGlyph(g)} ${esc(label(g))}`).join(" · ");
    return `<section class="deck-section deck-colony-purpose" aria-label="Colony purpose"><header><h3>Colony purpose</h3></header>
      <b>${esc(purpose.headline)}</b><span>${esc(purpose.homeNeed)}</span>
      ${purpose.exports.length ? `<div><small>Potential exports</small><span>${goods(purpose.exports)}</span></div>` : ""}
      <div><small>Supply imports</small><span>${purpose.imports.length ? goods(purpose.imports) : "No essential feedstock missing from the survey"}</span></div>
      <small>${esc(purpose.advantages || "Potential requires structures, workforce and freight.")}</small></section>`;
  }
}

function isSystemTab(value?: string): value is DeckSystemTab {
  return value === "overview" || value === "worlds" || value === "production" || value === "fleets" || value === "build";
}

function isMigrationPolicy(value?: string): value is BodyView["migration_policy"] {
  return value === "closed" || value === "managed" || value === "open" || value === "priority";
}

function bestStructureBody(system: SystemStateView, structure: string): BodyView | undefined {
  const wanted: Partial<Record<string, Commodity[]>> = {
    mining_complex: ["metallic_ore", "rare_elements", "silicates"],
    volatile_harvester: ["volatiles"],
    bioharvester: ["biomass"],
  };
  const resources = wanted[structure];
  return [...system.bodies].sort((a, b) => {
    const score = (body: BodyView): number => {
      const deposits = body.deposits ?? [];
      const richness = deposits
        .filter((deposit) => !resources || resources.includes(deposit.resource))
        .reduce((sum, deposit) => sum + deposit.richness, 0);
      const pool = structure && POOL_OF[structure];
      const slots = pool === "industrial" ? body.industrial_slots ?? 0
        : pool === "infrastructure" ? body.infrastructure_slots ?? 0
          : body.resource_slots ?? 0;
      return richness * 100 + slots;
    };
    return score(b) - score(a) || a.id - b.id;
  })[0];
}

function bestShipyardBody(system: SystemStateView): BodyView | undefined {
  return system.bodies.filter(isShipbuildingBody).sort((a, b) => {
    const aLowGravity = a.special === "low_gravity" ? 1 : 0;
    const bLowGravity = b.special === "low_gravity" ? 1 : 0;
    return bLowGravity - aLowGravity
      || (b.industrial_slots ?? 0) - (a.industrial_slots ?? 0)
      || a.id - b.id;
  })[0];
}

function isShipbuildingBody(body: BodyView): boolean {
  return (body.structures.shipyard ?? 0) > 0
    || (body.structures.naval_drydock ?? 0) > 0
    || (body.structures.capital_slipway ?? 0) > 0;
}

function systemName(ctx: CoreContext, id: string): string {
  return ctx.state.galaxy?.systems.find((entry) => entry.id === id)?.name ?? id;
}

function planetIcon(body: BodyView): string {
  const key: IconKey = body.environment === "gaia" || body.environment === "terran"
    ? "planetHabitable"
    : body.environment === "uninhabitable" ? "planetUninhabitable" : "planetHostile";
  return icon(key, "md", label(body.environment));
}

function shippableStock(dynamic: SystemStateView) {
  return (dynamic.stockpile ?? []).filter((slot) => slot.commodity !== "fuel" && slot.units >= 1);
}

function stat(name: string, value: string, warn = false): string {
  return `<span class="deck-stat${warn ? " is-warn" : ""}"><small>${esc(name)}</small><b>${esc(value)}</b></span>`;
}

function buildName(key: string): string {
  return label(key === "convoy" ? "freighter" : key === "raider" ? "interceptor" : key);
}

function shipIcon(key: string): IconKey {
  const icons: Record<string, IconKey> = {
    scout: "scout", corvette: "corvette", raider: "raider", convoy: "convoy", colony: "colony",
    destroyer: "destroyer", cruiser: "cruiser", battleship: "battleship",
    dreadnought: "dreadnought", titan: "titan",
  };
  return icons[key] ?? "fleet";
}

function fmt(value: number): string {
  return Number.isFinite(value) ? Math.round(value).toLocaleString() : "—";
}

function fmtPopulation(millions: number): string {
  const people = millions * 1_000_000;
  if (people >= 1_000_000) return `${(people / 1_000_000).toFixed(2)}m`;
  if (people >= 1_000) return `${(people / 1_000).toFixed(1)}k`;
  return fmt(people);
}

function emptyState(title: string, copy: string): string {
  return `<div class="deck-empty"><b>${esc(title)}</b><span>${esc(copy)}</span></div>`;
}

function routeIdentity(route: DeckRoute | null): string {
  if (!route) return "";
  const sorted = (values?: Record<string, string>) => Object.entries(values ?? {}).sort(([a], [b]) => a.localeCompare(b));
  return JSON.stringify([route.name, sorted(route.params), sorted(route.query)]);
}

function esc(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!);
}

undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined
undefined

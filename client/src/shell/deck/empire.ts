import { constructionStock, dockedAtSystem, PROSPECT_SECS, shipKindLabel } from "../../core/derive/fleet";
import { anchorStatus, beltActions, prospectLine } from "../../core/derive/belts";
import { colonyPurpose } from "../../core/derive/colony";
import { commandDelayTo } from "../../core/derive/geo";
import { pirateSite } from "../../core/derive/pirates";
import { buildProgress, buildsByPlanet } from "../../core/derive/construction";
import { gridLine, hullJobs, maxLevel, nextOrderTiming, romanTier as roman, structureFacts, structureQueue, WORKFORCE_STRUCTURES, type StructureJob } from "../../core/derive/worldbuild";
import { isOrbitalYard, planetStructureImage } from "../../planet-structure-art";
import { planetSurface, type PlanetSurface } from "../../planet-surfaces";
import { coach } from "../coach";
import { fmtBuildDur, fmtRatePerMin } from "../../core/derive/format";
import { blockadeBadge, blockadeBrief } from "../../core/derive/blockade";
import {
  bodyPoolUsage,
  buildOption,
  dispatchBuildKey,
  fitLegal,
  FITTING_POINTS,
  hullResearched,
  MODULE_SLOTS,
  moduleLedgerAt,
  moduleBuildReason,
  POOL_LABEL,
  POOL_OF,
  SHIP_FUELS,
  SHIP_YARD,
  shipOption,
  structOption,
  structureLevel,
  structureResearched,
  structureBuildSeconds,
  conversionSummary,
  MINERAL_DEPOSITS,
  systemFlavor,
  YARD_TITLE,
  type BuildOpt,
  type Pool,
  type PoolUse,
  type StructOpt,
} from "../../core/derive/market";
import { latestGroundRecordFor, SHIP_STATS } from "../../core/derive/orders";
import type { CoreEvent } from "../../core/events";
import { commodityIcon as commodityGlyph, icon, label, structureImage, type IconKey } from "../../icons";
import type { AssignmentView, BodyView, Commodity, Deposit, ModuleKind, ShipKind, SystemInfo, SystemStateView } from "../../protocol";
import { fleetExactCount } from "../../protocol";
import { liveSimTime } from "../../state";
import { starTypeFor } from "../../stars";
import { starArtSrcset, starArtUrl } from "../../starart";
import { BELT_META, beltChunkUrl, bodyArtUrl } from "../../systemview";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import { fleetListRow } from "./fleet-row";
import { PlanetPanel, type PlanetDockedFleet, type PlanetPanelModel, type PlanetSiege } from "./planet";
import { berthAnchor, siegeTarget, type AnchorBody } from "../../systemlayout";
import { refiningComparison } from "../refining";
import { isSitePool } from "./planet-sites";
import "../../styles/planet-panel.css";
import { DEFAULT_DEFENSE_RADIUS, DEFENSE_RADII } from "../../core/derive/defense";
import { stageDefenseAction, stationDraftAction, systemDefenseHtml, type StationDraft } from "../systemdefense";
import "../../styles/system-defense.css";
import type { DeckRoute } from "./router";

export type DeckSystemTab = "overview" | "worlds" | "production" | "fleets" | "build";

/** A system report's worlds as the System View's placement rules read them. */
function anchorBodies(dynamic: SystemStateView): AnchorBody[] {
  return dynamic.bodies.map((entry) => ({ id: entry.id, parent: entry.parent, habitable: entry.habitable,
    size: entry.size, kind: entry.kind, population: entry.population ?? 0, structures: entry.structures ?? {} }));
}

interface EmpireHooks {
  go(route: DeckRoute): void;
  /** Swap the current route for a sibling (a world switch) without growing the stack. */
  replace(route: DeckRoute): void;
  openGroundViewer(id: string): void;
  /** §planet-build: open a world with its structure builder over the planet,
   * as if the player had gone there and pressed Build. */
  openWorldBuild(systemId: string, bodyId: number, query: Record<string, string>): void;
  notice(html: string): void;
  toast(title: string, message: string, tone?: "quiet" | "good" | "warn" | "bad", destination?: DeckRoute): void;
}

type BuilderMode = "structures" | "ships" | "modules";
type BuildFeedback = { systemId: string; issuedAt: number; timelineLength: number; text: string; tone: "info" | "good" | "bad" };

const SHIP_ORDER: ShipKind[] = ["scout", "survey_probe", "corvette", "raider", "tiny_freighter", "small_freighter", "convoy", "mining_barge", "large_freighter", "heavy_freighter", "bulk_freighter", "colony", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
const SHIP_KEYS = new Set<string>(SHIP_ORDER);
const STRUCTURE_DESCRIPTION: Record<string, string> = {
  mining_complex: "Mines this world's ore deposits.",
  volatile_harvester: "Extracts Volatiles from a local deposit.",
  bioharvester: "Harvests Biomass from the biosphere.",
  smelter: "Refines ore into metals. Double power.",
  electronics_fabricator: "Silicates and Rare Elements → Electronics.",
  chemical_works: "Volatiles and Biomass → Polymers.",
  fuel_refinery: "Volatiles → Deuterium.",
  agroplex: "Biomass → Provisions.",
  machine_works: "Alloys and Electronics → Machinery. Double power.",
  armaments_complex: "Makes Armaments and fleet modules.",
  composite_works: "Makes Composites for hulls and drives.",
  hull_fabricator: "Makes Hull Sections for Destroyers and larger.",
  precision_works: "Makes Precision Components for advanced hulls.",
  drive_works: "Makes Warp Cores or Fusion Cores. Double power.",
  shipyard: "Builds freighters, scouts, colony ships and corvettes.",
  naval_drydock: "Builds large freighters and warships in orbit. Needs Shipyard II and Autoforges or Modular Berths.",
  capital_slipway: "Builds Dreadnoughts and Titans in orbit. Needs Orbital Shipyard III.",
  ordnance_foundry: "Repairs and refits fleets.",
  habitat: "Houses people, who become workforce.",
  orbital_warehouse: "Storage for the whole system.",
  sensor_array: "Sensor bubble around the system.",
  defense_platform: "Guns against hostile fleets.",
  academy: "Research and officer training.",
  garrison: "Defends the planet and raises troops.",
  warehouse: "Storage for the whole system.",
  service_berth: "Warehouse attachment for small-hull utility fittings. No extra slot; borrows a mining crew when unstaffed.",
  command_center: "Local administration. Lands with a Power Plant I. Upgrades add Infrastructure slots and speed up construction.",
  living_terraces: "Gaia housing that grows its own food.",
  power_plant: "Powers this planet's mines, factories, yards and Academy. Solar I–II; Deuterium III–IV; Helium-3 V–VI.",
  uranium_mill: "Ferrite Ore → Uranium.",
  antimatter_plant: "Deuterium → Antimatter. Ten times a factory's power.",
};
import { MODULES, moduleFitsHull } from "../../core/derive/equipment";

/** §construction: a structure order this client sent that no served report has
 * shown yet. It holds its goods in the builder's arithmetic until the report
 * arrives, and says so when no report comes back. */
interface PendingBuild {
  systemId: string;
  bodyId: number;
  key: string;
  tier: number;
  /** Built tier plus served orders for this structure when it was sent. */
  seen: number;
  sentAt: number;
  /** When the report should have come back (two light legs). */
  expectBy: number;
  slot: number | null;
  costs: { commodity: Commodity; units: number }[];
}

/** §build-sheet: one catalogue entry as the build sheet reads it. */
interface SheetEntry {
  key: string;
  entry: BuildOpt;
  state: StructOpt;
  short: { commodity: Commodity; units: number }[];
  order: PendingBuild | undefined;
  jobs: StructureJob[];
  /** Built, under construction or ordered on this world. */
  existing: boolean;
  /** Why it can't be ordered here, other than goods; empty when it can. */
  block: string;
}


/** Routed empire management. This reads only the player's served SystemStateView:
 * public astronomy is always visible, survey findings require arrived survey
 * light, and owner-only production never receives a rival rendering path. */
export class DeckEmpireRoutes {
  /** §founding R5: guide deep links open a world on the structure they name. */
  focusWorldStructure(structure: string): void {
    // §construction: a building is staffed from the world's management view,
    // so a link to one closes the builder that would otherwise hide it.
    this.worldBuilder = null;
    this.planet.focus(structure);
  }

  private readonly defenseRadius = new Map<string, number>();
  private readonly defenseAssigning = new Set<string>();
  /** §orbital-defense: a hardpoint refit being composed, per system. */
  private readonly stationDraft = new Map<string, StationDraft>();
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
  /** A construction child opened from a world route, keyed to that world. */
  private worldBuildRoute: DeckRoute | null = null;
  private worldBuildFor = "";
  private worldBuildOpener: HTMLElement | null = null;
  private worldBuildSignature = "";
  private planetPanel: PlanetPanel | null = null;
  private signature = "";
  /** §build-sheet: the structure builder open across its world's planet view.
   * `site`: a free site the player chose; `siteOnly`: opened from that site, so
   * only new buildings of its kind are offered; `placing`: the sheet is folded
   * away while the player picks a site on the planet; `blocked`: tabs whose
   * "Can't build here" list is open. */
  private worldBuilder: { world: string; site: { pool: Pool; slot: number } | null; siteOnly: boolean;
    placing: boolean; tab: Pool; blocked: Pool[] } | null = null;
  private pendingBuilds: PendingBuild[] = [];
  /** Worlds whose waiting line is expanded in the construction bar. */
  private readonly queueOpen = new Set<string>();
  private cancelArmed: number | null = null;
  private readonly cancelSent = new Map<number, number>();
  /** A completion toast asked to point at the new building's workforce. */
  private staffCoach = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly workbenchRoot: HTMLElement,
    private readonly stageRoot: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: EmpireHooks,
  ) {}

  get composedFit(): ModuleKind[] {
    return this.pendingFit;
  }

  render(route: DeckRoute | null, force = false): boolean {
    // The construction child belongs to ONE world; any other route dismisses it.
    if (this.worldBuildRoute && (route?.name !== "world" || worldKey(route) !== this.worldBuildFor)) this.resetWorkbench();
    this.renderWorldBuildWorkbench(force);
    if (route?.name !== "system" && route?.name !== "build" && route?.name !== "world") {
      this.hideStage();
      this.resetWorkbench();
      return false;
    }
    // §planet-build: the structure builder always opens over its world. The
    // redirect waits a microtask so the router finishes announcing this route
    // first; it still lands before the next paint.
    const redirect = route.name === "build" ? this.structureBuildTarget(route) : null;
    if (redirect) {
      queueMicrotask(() => this.hooks.openWorldBuild(redirect.systemId, redirect.bodyId, redirect.query));
      return true;
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
    const buildContext = route.name !== "world" && this.systemTab === "build" ? systemId : "";
    if (buildContext !== this.buildContext) {
      this.buildContext = buildContext;
      this.workbenchOpen = !!buildContext;
    }
    const docked = context.system ? this.dockedFleets(context.system) : [];
    const signature = sheetFingerprint([
      route, this.systemTab, this.builderMode, this.selectedBuild, this.selectedHull,
      this.shipQuantity, this.pendingFit, this.buildFeedback, this.workbenchOpen, Math.floor(liveSimTime()),
      this.worldBuilder, this.pendingBuilds, [...this.queueOpen], this.cancelArmed, [...this.cancelSent.keys()],
      context.system, context.dynamic, this.ctx.state.timeline.length, this.ctx.state.syndicate?.fits,
      this.ctx.state.market,
      this.ctx.state.syndicate?.flagship_name,
      route.name === "world" ? this.ctx.state.commandCenter : null,
      this.systemTab === "build" ? this.ctx.state.research?.programmes.map((p) => [p.id, p.state]) : null,
      this.systemTab === "overview" ? [this.ctx.state.ghosts, this.ctx.state.pendingOrders, this.ctx.state.battles, this.defenseRadius.get(systemId)] : null,
      this.systemTab === "fleets" ? docked.map((fleet) => [
        fleet.id, fleet.kind, fleet.docked, Math.floor(fleet.age), fleet.composition,
        fleet.cargo_manifest, fleet.cargo,
      ]) : null,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))
      || (this.workbenchOpen && renderDeferred(this.workbenchRoot.id, () => this.render(route, true)))
      || (route.name === "world" && renderDeferred(this.stageRoot.id, () => this.render(route, true)))) return true;
    this.signature = signature;
    if (route.name === "world") {
      // The planet rung: its scene takes the map area, its management this column.
      const body = context.dynamic?.bodies.find((entry) => String(entry.id) === route.params?.bodyId);
      const world = worldKey(route);
      if (this.worldBuilder && this.worldBuilder.world !== world) this.worldBuilder = null;
      if ((route.query?.build === "structures" || route.query?.structure) && context.system && body
        && context.dynamic?.owner === this.ctx.state.playerId) {
        // A request from elsewhere (the guide, a link, a completion toast):
        // open the planet's own builder, or select the named building, then
        // drop the request once the router has announced this route, so
        // closing the builder stays closed.
        if (route.query.build === "structures") {
          const site = isSitePool(route.query.site_pool) && route.query.site_slot !== undefined
            ? { pool: route.query.site_pool, slot: Number(route.query.site_slot) } : null;
          this.enterWorldBuilder(world, site, route.query.select);
        } else if (route.query.structure) {
          this.worldBuilder = null;
          this.planet.focus(route.query.structure);
          if (route.query.staff) this.staffCoach = route.query.structure;
        }
        const plain = worldRoute(context.system, body);
        queueMicrotask(() => this.hooks.replace(plain));
      }
      if (context.system && context.dynamic && body) {
        this.reconcileBuilds();
        const model = this.planetModel(context.system, context.dynamic, body);
        setHtml(this.stageRoot, this.planet.renderScene(model));
        setHtml(this.root, this.planet.render(model));
        this.showStage(body.name);
        this.pointAtStaffing(model);
      } else {
        this.hideStage();
        setHtml(this.root, emptyState("World report unavailable", "The requested served body record is not in this system report."));
      }
    } else {
      this.hideStage();
      setHtml(this.root, this.systemHtml(context.system, context.dynamic));
    }
    this.renderWorkbench(route, context.system, context.dynamic);
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (this.worldBuildRoute && this.workbenchPanel.contains(button)) {
      if (button.dataset.deckAct === "build-workbench-close") return this.closeBuildWorkbench(route);
      return this.handleBuildAction(button, this.worldBuildRoute);
    }
    if (route?.name === "world") return this.handleWorldAction(button, route);
    if (route?.name !== "system" && route?.name !== "build") return false;
    const action = button.dataset.deckAct;
    const { system, dynamic } = this.systemContext(route);
    if (!system) return false;
    if (action?.startsWith("defense-") && dynamic?.owner === this.ctx.state.playerId) {
      const refit = stationDraftAction(action, button.dataset, this.stationDraft.get(system.id) ?? null, dynamic);
      if (refit) {
        if (refit.draft) this.stationDraft.set(system.id, refit.draft);
        else this.stationDraft.delete(system.id);
        if (refit.send) this.ctx.send(refit.send);
        this.render(route, true);
      } else if (action === "defense-toggle") {
        if (this.defenseAssigning.has(system.id)) this.defenseAssigning.delete(system.id);
        else this.defenseAssigning.add(system.id);
        this.render(route, true);
      } else if (action === "defense-radius") {
        const radius = Number(button.dataset.radius);
        if (DEFENSE_RADII.some(r => r === radius)) this.defenseRadius.set(system.id, radius);
        this.render(route, true);
      } else if (action === "defense-build") {
        this.hooks.go({ name: "build", params: { id: system.id }, query: { mode: "structures", select: "orbital_defense" } });
      } else if (action === "defense-doctrine") this.hooks.go({ name: "doctrine" });
      else if (action === "defense-log") this.hooks.go({ name: "log" });
      else if (action === "defense-open") {
        const fleet = this.ctx.state.ghosts.find(g => g.own && g.id === button.dataset.fleet);
        if (fleet) {
          this.ctx.state.selectedShipId = fleet.id;
          this.ctx.state.selectedShipIds = new Set([fleet.id]);
          this.ctx.state.selectedSystemId = null;
          this.ctx.state.selectedEmplacementId = null;
          this.ctx.renderer.stateVersion++;
          this.hooks.go({ name: "fleet", params: { id: fleet.id } });
        }
      } else stageDefenseAction(this.ctx, action, system.id, button.dataset.fleet ?? "", this.defenseRadius.get(system.id) ?? DEFAULT_DEFENSE_RADIUS);
      return true;
    }
    if (action === "build-workbench-close") return this.closeBuildWorkbench(route);
    if (action === "system-tab") {
      const tab = button.dataset.tab;
      if (!isSystemTab(tab)) return true;
      const target = tab === "build" && this.builderMode === "structures"
        ? this.structureBuildTarget({ name: "build", params: { systemId: system.id, systemLabel: system.name } }) : null;
      if (target) {
        this.hooks.openWorldBuild(target.systemId, target.bodyId, target.query);
        return true;
      }
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
      if (body) {
        // A production line names its structure so the world opens on it.
        if (button.dataset.structure) this.planet.focus(button.dataset.structure);
        this.hooks.go(worldRoute(system, body));
      }
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
    return false;
  }

  invalidate(): void {
    this.signature = "";
  }

  closeBuildWorkbench(route: DeckRoute | null): boolean {
    if (this.worldBuilder && route?.name === "world") {
      // Escape steps back: from placing to the sheet, then out of the sheet.
      if (this.worldBuilder.placing) this.worldBuilder.placing = false;
      else this.worldBuilder = null;
      this.render(route, true);
      return true;
    }
    if (this.worldBuildRoute) {
      const opener = this.worldBuildOpener;
      this.resetWorkbench();
      (opener?.isConnected ? opener : this.root.querySelector<HTMLButtonElement>("button:not(:disabled)"))?.focus();
      return true;
    }
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
    this.planetPanel = null;
    this.buildContext = "";
    this.workbenchOpen = false;
    this.resetWorkbench();
    this.hideStage();
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
    const survey = surveyed ? "survey report received" : "Uncharted · send a Scout or Survey Probe";
    const star = starTypeFor(system.id);
    const tabs = [
      ["overview", "Overview"], ["worlds", "Worlds"], ["production", "Production"], ["fleets", "Fleets"], ["build", "Build"],
    ].map(([tab, text]) => `<button type="button" data-deck-act="system-tab" data-tab="${tab}" aria-selected="${tab === this.systemTab}">${text}</button>`).join("");
    const brief = dynamic?.blockade && this.ctx.state.galaxy
      ? blockadeBrief(dynamic.blockade, liveSimTime(), this.ctx.state.galaxy.siege_secs, dynamic.ground?.marines_needed)
      : null;
    const alert = brief
      ? `<div class="deck-alert deck-alert--${brief.tone === "good" ? "info" : brief.tone}"><b>${esc(brief.title)}</b><span>${esc(brief.detail)}</span></div>`
      : "";
    const pirate = pirateSite(dynamic?.intel?.enclave_tier ?? 0);
    const threat = pirate ? `<article class="deck-operation">${pirate.art ? `<img src="${pirate.art}" alt="">` : ""}<div><b>${pirate.title}</b><p>${pirate.goal}</p><span>Reported defenses: ${dynamic?.intel?.defense_tier ?? "?"} tiers</span></div></article>` : "";
    const active = this.systemTab === "overview"
      ? this.systemOverview(system, dynamic, mine, owner, survey)
      : this.systemTab === "worlds"
        ? this.systemWorlds(system, dynamic, mine)
        : this.systemTab === "production"
          ? this.systemProduction(system, dynamic, mine)
          : this.systemTab === "fleets"
            ? this.systemFleets(system)
            : this.buildDashboard(dynamic);
    return `<section class="deck-page deck-system"><header class="deck-page__lead deck-system__lead"><img class="deck-system__star" src="${esc(starArtUrl("system", star.slug))}" srcset="${esc(starArtSrcset("system", star.slug))}" sizes="72px" alt="${esc(star.title)}"><h2>${esc(system.name)}</h2><span>${esc(systemFlavor(system, dynamic?.deposits ?? null))}</span></header>${alert}${threat}<nav class="deck-tabs" aria-label="System sections">${tabs}</nav>${active}</section>`;
  }

  private systemOverview(system: SystemInfo, dynamic: SystemStateView | undefined, mine: boolean, owner: string, survey: string): string {
    const bodies = dynamic?.bodies ?? [];
    const publicStats = [
      stat("Status", owner),
      stat("Survey", survey),
      stat("Worlds", dynamic?.deposits == null && !mine ? "Uncharted" : String(bodies.length || "—")),
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
    return `<div class="deck-stat-grid">${ownerStats.join("")}</div>${systemDefenseHtml(this.ctx.state, system, dynamic, this.defenseRadius.get(system.id) ?? DEFAULT_DEFENSE_RADIUS, "deck", this.defenseAssigning.has(system.id), this.stationDraft.get(system.id) ?? null)}${this.groundActivityHtml(dynamic)}${this.opportunityHtml(dynamic)}${this.queueHtml(dynamic)}`;
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
    if (!bodies.length && dynamic && dynamic.deposits == null && !mine) {
      return emptyState("Uncharted system", "Its planets, moons and asteroid belts stay unknown until one of your Scouts or Survey Probes surveys it and the report reaches command.");
    }
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
        .map(([commodity, rate]) => `${fmtRatePerMin(rate)} ${esc(label(commodity))}`).join(" · ");
      // §power: each planet's own grid.
      const grid = mine ? gridLine(body) : null;
      // §belts: a belt reads as its type, never as a sized, weathered world.
      const identity = body.belt
        ? BELT_META[body.belt.kind]?.label ?? "Asteroid belt"
        : `${label(body.size)} · ${label(body.environment)} · ${label(body.kind)}`;
      return `<button type="button" class="deck-world" data-deck-act="open-world" data-body="${body.id}"><span class="deck-world__identity">${planetIcon(body)}<span><b>${esc(body.name)}</b><small>${esc(identity)}</small></span></span>${deposits}<span class="deck-world__meta">${body.population > 0 ? fmtPopulation(body.population) : body.geology === null ? "awaiting survey" : mine ? "undeveloped" : "observed"}${outputs ? `<small>${outputs}</small>` : ""}${grid ? `<small class="deck-world__grid${grid.short ? " is-short" : ""}" title="${esc(grid.tip)}">${esc(grid.text)}</small>` : ""}</span></button>`;
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
    const shipment = `<div class="deck-section__action"><button type="button" class="is-primary" data-deck-act="ship-production" ${(dynamic.blockade?.held_since ?? null) !== null || !manifest.length ? "disabled" : ""}>Ship ${shipmentUnits ? fmt(shipmentUnits) : "no"} goods to Market</button><span>Fuels stay in the system reserve. Pickup, transit and sale are physical and delayed.</span></div>`;
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
      `<section class="deck-section"><header><div><h3>Production lines</h3><p>Open a line to assign its workforce on the world. Output = throughput × staffing × expertise × supply × power × site.</p></div><b>${assignments.length}</b></header><div class="deck-assignment-list">${productionRows}</div></section>` +
      this.queueHtml(dynamic);
  }

  private systemFleets(system: SystemInfo): string {
    // Membership is deliberately the player's SERVED dock report. A departing
    // fleet remains listed until that light reaches command; no client-side
    // position estimate or authoritative server truth corrects the roster.
    const fleets = this.dockedFleets(system);
    if (!fleets.length) return emptyState("No docked fleets", `No owned fleet is reported berthed at ${system.name}.`);
    const rows = fleets.map((fleet) => fleetListRow(fleet, {
      openAction: "system-fleet-open", status: fleet.defend_system ? "Defending · docked" : "Docked",
      flagshipName: this.ctx.state.syndicate?.flagship_name,
    })).join("");
    return `<section class="deck-section"><header><div><h3>Docked fleets</h3></div><b>${fleets.length}</b></header><div class="deck-fleet-list">${rows}</div></section>`;
  }

  private dockedFleets(system: SystemInfo) {
    return this.ctx.state.ghosts
      .filter((fleet) => fleet.own && dockedAtSystem(fleet, system.id))
      .sort((a, b) => shipKindLabel(a.kind).localeCompare(shipKindLabel(b.kind)) || a.id.localeCompare(b.id));
  }

  // Production lines are read-only roll-ups: workforce is edited on the world
  // itself, where the draft → review → confirm contract shows the light delay.
  private assignmentHtml(dynamic: SystemStateView, line: AssignmentView): string {
    const body = dynamic.bodies.find((entry) => entry.id === line.body_id);
    const output = line.outputs.length
      ? line.outputs.map(([commodity, rate]) => `${fmtRatePerMin(rate)} ${label(commodity)}`).join(" · ")
      : "Idle";
    // §power: a line on a browned-out planet names its share.
    const power = line.power ?? 1;
    return `<button type="button" class="deck-assignment${line.suspended || power < 0.999 ? " is-warn" : ""}" data-deck-act="open-world" data-body="${line.body_id}" data-structure="${esc(line.structure)}" aria-label="Open ${esc(line.title)} on ${esc(body?.name ?? "its world")}"><div><b>${esc(line.title)} · Tier ${line.tier}</b><span>${esc(body?.name ?? "System")}${line.suspended ? ` · ${esc(label(line.suspended))}` : ""}${power < 0.999 ? ` · Power ${Math.round(power * 100)}%` : ""}</span><small>${esc(output)}</small></div><span class="deck-assignment__workers"><b>${line.workers}</b><small>workforce</small></span></button>`;
  }

  private unassignedAssignmentHtml(body: BodyView, structure: string, tier: number): string {
    return `<button type="button" class="deck-assignment is-warn" data-deck-act="open-world" data-body="${body.id}" data-structure="${esc(structure)}" aria-label="Open ${esc(label(structure))} on ${esc(body.name)}"><div><b>${esc(label(structure))} · Tier ${tier}</b><span>${esc(body.name)} · needs workforce</span><small>Idle</small></div><span class="deck-assignment__workers"><b>0</b><small>workforce</small></span></button>`;
  }

  /** The construction queue: every world's jobs for the system rail, or one
   * world's own jobs (no grouping) for that world's column. */
  private queueHtml(dynamic: SystemStateView, bodyId?: number): string {
    const now = liveSimTime();
    const groups = buildsByPlanet(dynamic.builds).filter((group) => bodyId === undefined || group.bodyId === bodyId);
    const rows = groups.map(({ bodyId: groupBody, jobs }) => {
      const body = dynamic.bodies.find((entry) => entry.id === groupBody);
      const builds = jobs.map((job) => {
        const duration = job.complete_time == null ? null : Math.max(0, job.complete_time - now);
        const progress = buildProgress(job, now);
        const bar = progress == null ? "" : `<span class="deck-queue-progress" role="progressbar" aria-label="${esc(buildName(job.key))} build progression" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${progress.toFixed(0)}"><i style="width:${progress.toFixed(1)}%"></i></span>`;
        const status = job.queued ? "Queued" : duration == null ? "Paused · needs workforce" : duration > 0 ? `Building · ${fmtBuildDur(duration)}` : "completing";
        return `<div class="deck-queue-row${job.queued ? " is-queued" : ""}"><span>${icon("queue", "sm")}<span><b>${esc(buildName(job.key))}</b>${bar}</span></span><em>${status}</em></div>`;
      }).join("");
      return bodyId === undefined ? `<div class="deck-planet-queue"><h4>${esc(body?.name ?? "System yard")}</h4>${builds}</div>` : builds;
    }).join("");
    const feedback = this.buildFeedback?.systemId === dynamic.id && this.buildFeedback.text
      ? `<div class="deck-build-feedback is-${this.buildFeedback.tone}">${esc(this.buildFeedback.text)}</div>`
      : "";
    const count = groups.reduce((sum, group) => sum + group.jobs.length, 0);
    const queueCount = rows ? `<b>${count}</b>` : "";
    return `<section class="deck-section"><header><div><h3>${bodyId === undefined ? "Construction queue" : "Construction here"}</h3></div>${queueCount}</header>${feedback}${rows || `<div class="deck-empty-inline">Empty</div>`}</section>`;
  }

  private handleBuildAction(button: HTMLButtonElement, route: DeckRoute): boolean {
    const { system, dynamic } = this.systemContext(route);
    if (!system || !dynamic || dynamic.owner !== this.ctx.state.playerId) return false;
    const action = button.dataset.deckAct;
    if (action === "builder-all-structures") {
      const query = { ...route.query };
      delete query.site_pool;
      delete query.site_slot;
      this.navigateBuilder({ ...route, query });
      return true;
    } else if (action === "builder-mode") {
      const mode = button.dataset.mode;
      if (mode === "structures" || mode === "ships" || mode === "modules") {
        this.builderMode = mode;
        const query: Record<string, string> = { ...route.query, mode };
        if (mode !== "structures") { delete query.site_pool; delete query.site_slot; }
        this.navigateBuilder({ ...route, query });
      }
      return true;
    } else if (action === "builder-body") {
      const body = dynamic.bodies.find((entry) => String(entry.id) === button.dataset.body);
      if (body) {
        const query: Record<string, string> = { ...route.query, body: String(body.id) };
        delete query.site_pool;
        delete query.site_slot;
        this.navigateBuilder({ ...route, query });
      }
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
      const input = this.workbenchRoot.querySelector<HTMLInputElement>("#deck-fit-name");
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
      if (body && this.selectedBuild) {
        if (isSitePool(route.query?.site_pool)) {
          const model = this.planetModel(system, dynamic, body);
          const slot = Number(route.query?.site_slot);
          const site = this.planet.availableSite(model, slot);
          if (!site || site.pool !== route.query.site_pool || POOL_OF[this.selectedBuild] !== site.pool || body.structures[this.selectedBuild]
            || dynamic.builds.some(job => job.body_id === body.id && job.key === this.selectedBuild)) return true;
          this.planet.preferSite(model, this.selectedBuild, slot);
        }
        this.dispatchBuild(this.selectedBuild, dynamic.id, body.id);
      }
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
    if (this.worldBuildRoute) this.renderWorldBuildWorkbench(true);
    else this.render(route, true);
    return true;
  }

  private navigateBuilder(route: DeckRoute): void {
    if (this.worldBuildRoute) {
      this.worldBuildRoute = route;
      this.renderWorldBuildWorkbench(true);
    } else this.hooks.go(route);
  }

  /// §belts: a charted belt's Ship mining section: a Prospect button for each
  /// Scout or barge fleet, and a Mine button per ore and found pocket for each
  /// barge fleet, staged through the ordinary Confirm preview.
  private beltMiningHtml(dynamic: SystemStateView, body: BodyView): string {
    if (body.kind !== "asteroid_belt") return "";
    const { prospect, mine, anchor, station, note, yours } = beltActions(this.ctx.state.ghosts, this.ctx.state.playerId, dynamic, body);
    const buttons = prospect.map((a) => `<button type="button" data-deck-act="world-prospect" data-fleet="${esc(a.fleet)}" title="${PROSPECT_SECS} s loud pass · 50% to find each hidden pocket">${esc(a.text)}</button>`).join("")
      + mine.map((a) => `<button type="button" data-deck-act="world-mine" data-fleet="${esc(a.fleet)}" data-ore="${esc(a.ore!)}"${a.pocket ? ` data-pocket="1"` : ""}>${esc(a.text)}</button>`).join("")
      + anchor.map((a) => `<button type="button" data-deck-act="world-anchor" data-fleet="${esc(a.fleet)}" title="Barges here unload into it">${esc(a.text)}</button>`).join("");
    // §belts: your station at this belt, with its hold.
    const depot = station ? `<div class="deck-belt-station"><span>${icon("mining_belt_station", "sm")}<b>Station</b><small>${esc(anchorStatus(station.anchor, liveSimTime()))}</small></span>`
      + `<em>${fmt(station.held)} / ${fmt(station.capacity)}</em><button type="button" data-deck-act="world-station-open" data-fleet="${esc(station.fleet)}">Open</button></div>` : "";
    const hint = mine.length ? `<p class="deck-muted">A barge cuts ore into ${station ? "your station" : "its own hold"}${yours ? ", or straight into this system's stockpile" : ""}.${station ? "" : " Bring it home to unload when the hold is full."}</p>` : "";
    return `<section class="deck-section"><h3>Ship mining</h3>${depot}${note ? `<div class="deck-empty-inline">${esc(note)}</div>` : ""}${buttons ? `<div class="deck-world-actions">${buttons}</div>` : ""}${hint}</section>`;
  }

  private handleWorldAction(button: HTMLButtonElement, route: DeckRoute): boolean {
    const { system, dynamic } = this.systemContext(route);
    const body = dynamic?.bodies.find((entry) => String(entry.id) === route.params?.bodyId);
    if (!system || !dynamic || !body) return false;
    const action = button.dataset.deckAct;
    if (action === "world-mine" && button.dataset.fleet && button.dataset.ore) {
      this.ctx.intent.beginFleetCommand({ type: "MineBelt", fleet_id: button.dataset.fleet, system_id: system.id,
        body_id: body.id, ore: button.dataset.ore as Commodity, ...(button.dataset.pocket ? { pocket: true } : {}) });
      return true;
    }
    if (action === "world-prospect" && button.dataset.fleet) {
      this.ctx.intent.beginFleetCommand({ type: "ProspectBelt", fleet_id: button.dataset.fleet, system_id: system.id, body_id: body.id });
      return true;
    }
    if (action === "world-station-open" && button.dataset.fleet) {
      const station = this.ctx.state.ghosts.find((g) => g.own && g.id === button.dataset.fleet);
      if (station) {
        this.ctx.state.selectedShipId = station.id;
        this.ctx.state.selectedShipIds = new Set([station.id]);
        this.ctx.state.selectedEmplacementId = null;
        this.ctx.renderer.stateVersion++;
        this.hooks.go({ name: "fleet", params: { id: station.id, fleetLabel: "Mining Belt Station" } });
      }
      return true;
    }
    if (action === "world-anchor" && button.dataset.fleet) {
      this.ctx.intent.beginFleetCommand({ type: "AnchorStation", fleet_id: button.dataset.fleet, system_id: system.id, body_id: body.id });
      return true;
    }
    if (action === "watch-ground" && button.dataset.ground) {
      this.hooks.openGroundViewer(button.dataset.ground);
      return true;
    }
    if (action === "world-fleet-open" && button.dataset.fleet) {
      // §planet-orbit: a docked hull in the sky opens its fleet, as in the System View.
      const fleet = this.ctx.state.ghosts.find((candidate) => candidate.id === button.dataset.fleet && dockedAtSystem(candidate, system.id));
      if (fleet) {
        if (fleet.own) {
          this.ctx.state.selectedShipId = fleet.id;
          this.ctx.state.selectedShipIds = new Set([fleet.id]);
          this.ctx.state.selectedEmplacementId = null;
          this.ctx.renderer.stateVersion++;
        }
        this.hooks.go({ name: "fleet", params: { id: fleet.id, fleetLabel: `${shipKindLabel(fleet.kind)} fleet` } });
      }
      return true;
    }
    if (action === "world-switch") {
      const next = dynamic.bodies.find(entry => String(entry.id) === button.dataset.body);
      if (!next || next.id === body.id) return true;
      // A sibling world replaces this route entry: Back still returns to the system.
      this.hooks.replace(worldRoute(system, next));
      requestAnimationFrame(() => {
        this.root.scrollTop = 0;
        this.stageRoot.querySelector<HTMLButtonElement>(`#planet-world-${next.id}`)?.focus();
      });
      return true;
    }
    if (action?.startsWith("planet-")) {
      // Clicking a building, on the planet or in the list, shows its management.
      if (action === "planet-select") this.worldBuilder = null;
      const handled = this.planet.handleAction(button.dataset, this.planetModel(system, dynamic, body), command => this.ctx.send(command));
      if (handled) this.render(route, true);
      return handled;
    }
    if (action === "world-build" || action === "world-build-site") {
      if (dynamic.owner !== this.ctx.state.playerId) return true;
      // Hulls keep the central workbench; structures open a sheet across this
      // world's planet view, beside its management column.
      if (action === "world-build" && button.dataset.mode === "ships") {
        this.worldBuilder = null;
        this.openWorldBuilder(route, system, body, "ships", undefined, button.dataset.select, button);
        this.render(route, true);
        return true;
      }
      const site = action === "world-build-site" ? this.planet.availableSite(this.planetModel(system, dynamic, body), Number(button.dataset.slot)) : undefined;
      if (action === "world-build-site" && (!site || site.pool !== button.dataset.pool)) return true;
      // While placing, a lit site is where the sheet's pick goes.
      if (site && this.worldBuilder?.placing && this.worldBuilder.world === worldKey(route)) {
        if (POOL_OF[this.selectedBuild] === site.pool) this.worldBuilder.site = { pool: site.pool, slot: site.slot };
        this.worldBuilder.placing = false;
        this.render(route, true);
        return true;
      }
      this.enterWorldBuilder(worldKey(route), site ? { pool: site.pool, slot: site.slot } : null, button.dataset.select);
      if (site) {
        // A chosen site asks for a new building of its kind: keep a fitting
        // pick, otherwise take the first one this world can place there.
        const fits = (key: string) => this.worldOptions(dynamic, body, site.pool).some((entry) => entry.key === key);
        if (!fits(this.selectedBuild)) this.selectedBuild = this.worldOptions(dynamic, body, site.pool)
          .find((entry) => structOption(entry, dynamic, body, bodyPoolUsage(body, dynamic)).buildable)?.key
          ?? this.worldOptions(dynamic, body, site.pool)[0]?.key ?? this.selectedBuild;
      }
      this.render(route, true);
      return true;
    }
    if (action?.startsWith("world-builder-") || action === "world-queue-toggle" || action?.startsWith("world-cancel")
      || action === "world-pending-dismiss") {
      if (dynamic.owner !== this.ctx.state.playerId) return true;
      this.worldBuilderAction(action, button, route, system, dynamic, body);
      this.render(route, true);
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
      const select = this.root.querySelector<HTMLSelectElement>("[data-deck-relocation-target]");
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
    // A child of a world route builds on that world: the site is fixed, so a
    // switcher strip would only duplicate the planet's own world strip.
    const worlds = this.worldBuildRoute
      ? `<b>${esc(body.name)}</b>`
      : sites.map((entry) => `<button type="button" data-deck-act="builder-body" data-body="${entry.id}" aria-selected="${entry.id === body.id}">${esc(entry.name)}</button>`).join("");
    const content = this.builderMode === "structures"
      ? this.structureBuilder(dynamic, body, isSitePool(route.query?.site_pool) ? route.query.site_pool : undefined)
      : this.shipBuilder(dynamic, body);
    return `<section class="deck-page deck-build"><nav class="deck-tabs" aria-label="Build category">${modeTabs}</nav><div class="deck-builder-worlds"><span>Build site</span>${worlds}</div>${content}</section>`;
  }

  private buildDashboard(dynamic?: SystemStateView): string {
    if (!dynamic) return emptyState("Construction unavailable", "The served system ledger has not arrived.");
    return this.queueHtml(dynamic);
  }

  private renderWorkbench(route: DeckRoute, system?: SystemInfo, dynamic?: SystemStateView): void {
    if (this.worldBuildRoute) return;
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

  private renderWorldBuildWorkbench(force: boolean): void {
    const route = this.worldBuildRoute;
    if (!route) return;
    const { system, dynamic } = this.systemContext(route);
    const signature = sheetFingerprint([
      route, system, dynamic, this.ctx.state.playerId, this.ctx.state.research, this.ctx.state.market,
      this.ctx.state.galaxy?.build_options, this.ctx.state.syndicate?.fits,
      this.builderMode, this.selectedBuild, this.selectedHull, this.shipQuantity,
      this.pendingFit, this.buildFeedback,
    ]);
    if (!force && signature === this.worldBuildSignature) return;
    if (renderDeferred(this.workbenchRoot.id, () => this.renderWorldBuildWorkbench(true))) return;
    this.worldBuildSignature = signature;
    this.workbenchPanel.hidden = false;
    this.workbenchPanel.setAttribute("aria-hidden", "false");
    const title = this.workbenchPanel.querySelector<HTMLElement>("#deck-build-workbench-title");
    if (title) title.textContent = `${system?.name ?? "Planet"} Construction`;
    const feedback = this.buildFeedback?.systemId === system?.id && this.buildFeedback?.text
      ? `<div class="deck-build-feedback is-${this.buildFeedback.tone}" role="status">${esc(this.buildFeedback.text)}</div>` : "";
    setHtml(this.workbenchRoot, `${feedback}${this.buildHtml(route, system, dynamic)}`);
  }

  private resetWorkbench(): void {
    this.worldBuildRoute = null;
    this.worldBuildOpener = null;
    this.worldBuildSignature = "";
    this.worldBuildFor = "";
    this.buildContext = "";
    this.workbenchOpen = false;
    this.workbenchPanel.hidden = true;
    this.workbenchPanel.setAttribute("aria-hidden", "true");
    setHtml(this.workbenchRoot, "");
  }

  private get workbenchPanel(): HTMLElement {
    return this.workbenchRoot.closest("#deck-build-workbench") as HTMLElement;
  }

  private get stagePanel(): HTMLElement {
    return this.stageRoot.closest("#deck-planet-stage") as HTMLElement;
  }

  private showStage(name: string): void {
    const stage = this.stagePanel;
    stage.hidden = false;
    stage.setAttribute("aria-label", `${name} planet view`);
  }

  private hideStage(): void {
    const stage = this.stagePanel;
    if (stage.hidden) return;
    stage.hidden = true;
    setHtml(this.stageRoot, "");
  }

  private builderBody(route: DeckRoute, dynamic: SystemStateView): BodyView | undefined {
    const requested = route.query?.body;
    const explicit = dynamic.bodies.find((entry) => String(entry.id) === requested);
    if (explicit && (this.builderMode !== "ships" || isShipbuildingBody(explicit))) return explicit;
    if (this.builderMode === "ships") return bestShipyardBody(dynamic);
    return bestStructureBody(dynamic, this.selectedBuild) ?? dynamic.bodies[0];
  }

  /// Construction is a child of this world, not a workspace navigation: the
  /// planet's report, selection, drafts and scroll position stay mounted.
  private openWorldBuilder(route: DeckRoute, system: SystemInfo, body: BodyView, mode: "structures" | "ships",
    site: { pool: string; slot: number } | undefined, select: string | undefined, opener: HTMLElement | null): void {
    this.worldBuildRoute = {
      name: "build",
      params: { systemId: system.id, systemLabel: system.name },
      query: { body: String(body.id), mode,
        ...(site ? { site_pool: site.pool, site_slot: String(site.slot) } : {}),
        ...(select ? { select } : {}) },
    };
    this.worldBuildFor = worldKey(route);
    this.worldBuildOpener = opener;
    this.renderWorldBuildWorkbench(true);
    requestAnimationFrame(() => { if (this.worldBuildRoute) this.workbenchPanel.focus(); });
  }

  /// §planet-build: where a structure builder asked for by a `build` route (the
  /// tutorial, handoffs, links) belongs: the requested world, else the best one
  /// for the selected structure. Ships and modules keep the system builder, and
  /// so does a system you don't own (its builder explains why).
  private structureBuildTarget(route: DeckRoute): { systemId: string; bodyId: number; query: Record<string, string> } | null {
    if ((route.query?.mode ?? "structures") !== "structures") return null;
    const { system, dynamic } = this.systemContext(route);
    if (!system || !dynamic || dynamic.owner !== this.ctx.state.playerId) return null;
    const requested = dynamic.bodies.find((entry) => String(entry.id) === route.query?.body);
    const body = requested ?? bestStructureBody(dynamic, route.query?.select ?? this.selectedBuild) ?? dynamic.bodies[0];
    if (!body) return null;
    const query: Record<string, string> = {};
    for (const key of ["select", "site_pool", "site_slot"] as const) {
      const value = route.query?.[key];
      if (value !== undefined) query[key] = value;
    }
    return { systemId: system.id, bodyId: body.id, query };
  }

  private get planet(): PlanetPanel {
    return this.planetPanel ??= new PlanetPanel();
  }

  // ─── §construction: the world's own builder and work slot ──────────────────

  private enterWorldBuilder(world: string, site: { pool: Pool; slot: number } | null, select?: string): void {
    if (select) this.selectedBuild = select;
    // The sheet opens on the site's kind, else on the remembered pick's.
    const tab = site?.pool ?? (POOL_OF[this.selectedBuild] as Pool | undefined) ?? "resource";
    this.worldBuilder = { world, site, siteOnly: !!site, placing: false, tab, blocked: [] };
    this.builderMode = "structures";
    this.cancelArmed = null;
    // One builder at a time: a hull workbench over this world closes.
    if (this.worldBuildRoute) this.resetWorkbench();
  }

  /** Structures this world can be offered, optionally only new ones for a site's pool. */
  private worldOptions(dynamic: SystemStateView, body: BodyView, pool?: Pool | null): BuildOpt[] {
    return (this.ctx.state.galaxy?.build_options ?? []).filter((entry) => !SHIP_KEYS.has(entry.key) && !(entry.key in SHIP_YARD)
      && !entry.key.startsWith("module:") && !!POOL_OF[entry.key] && structureResearched(entry)
      && (!pool || (POOL_OF[entry.key] === pool && !body.structures[entry.key]
        && !dynamic.builds.some((job) => job.body_id === body.id && job.key === entry.key)))) as BuildOpt[];
  }

  /** Goods promised to this system's orders in flight: not spent yet, not free either. */
  private heldFor(systemId: string): Map<Commodity, number> {
    const held = new Map<Commodity, number>();
    for (const order of this.pendingBuilds) {
      if (order.systemId !== systemId) continue;
      for (const cost of order.costs) held.set(cost.commodity, (held.get(cost.commodity) ?? 0) + cost.units);
    }
    return held;
  }

  /** Drop orders a served report has confirmed, and forget long-silent ones. */
  private reconcileBuilds(): void {
    const now = liveSimTime();
    this.pendingBuilds = this.pendingBuilds.filter((order) => {
      const dynamic = this.ctx.state.systems.find((entry) => entry.id === order.systemId);
      const body = dynamic?.bodies.find((entry) => entry.id === order.bodyId);
      if (!dynamic || !body || dynamic.owner !== this.ctx.state.playerId) return false;
      const count = (body.structures[order.key] ?? 0) + dynamic.builds.filter((job) => job.body_id === order.bodyId && job.key === order.key).length;
      return count <= order.seen && now < order.expectBy + 600;
    });
    const served = new Set(this.ctx.state.systems.flatMap((system) => system.builds.map((job) => job.id)));
    for (const [id, at] of this.cancelSent) if (!served.has(id) || now > at + 600) this.cancelSent.delete(id);
  }

  private pointAtStaffing(model: PlanetPanelModel): void {
    const key = this.staffCoach;
    if (!key) return;
    this.staffCoach = "";
    const staffed = model.report.assignments.some((line) => line.body_id === model.body.id && line.structure === key && line.workers > 0);
    if ((model.body.structures[key] ?? 0) > 0 && WORKFORCE_STRUCTURES.has(key) && !staffed) {
      coach("staff-plus", `Assign workforce so the ${buildName(key)} starts work`);
    }
  }

  private worldBuilderAction(action: string, button: HTMLButtonElement, route: DeckRoute, system: SystemInfo, dynamic: SystemStateView, body: BodyView): void {
    const world = worldKey(route);
    const wb = this.worldBuilder;
    if (action === "world-builder-exit") this.worldBuilder = null;
    else if (action === "world-builder-pick" && button.dataset.key) {
      this.selectedBuild = button.dataset.key;
      const pool = POOL_OF[this.selectedBuild] as Pool | undefined;
      if (wb && pool) wb.tab = pool;
      // A chosen site takes only a new building of its kind.
      if (wb?.site && !this.worldOptions(dynamic, body, wb.site.pool).some((entry) => entry.key === this.selectedBuild)) {
        wb.site = null;
        wb.siteOnly = false;
      }
    } else if (action === "world-builder-tab" && wb && isSitePool(button.dataset.pool)) wb.tab = button.dataset.pool;
    else if (action === "world-builder-blocked" && wb) {
      wb.blocked = wb.blocked.includes(wb.tab) ? wb.blocked.filter((pool) => pool !== wb.tab) : [...wb.blocked, wb.tab];
    } else if (action === "world-builder-all" && wb) {
      wb.site = null;
      wb.siteOnly = false;
    } else if (action === "world-builder-place" && wb) wb.placing = true;
    else if (action === "world-builder-place-keep" && wb) {
      // Keep the suggested (or already chosen) site and return to the sheet.
      const pool = POOL_OF[this.selectedBuild] as Pool | undefined;
      const slot = !pool ? null : wb.site?.pool === pool ? wb.site.slot : this.planet.firstFreeSite(this.planetModel(system, dynamic, body), pool);
      if (pool && slot !== null) wb.site = { pool, slot };
      wb.placing = false;
    } else if (action === "world-builder-place-back" && wb) wb.placing = false;
    else if (action === "world-builder-market") this.hooks.go({ name: "market" });
    else if (action === "world-builder-queue" && button.dataset.key) this.queueWorldBuild(system, dynamic, body, button.dataset.key);
    else if (action === "world-queue-toggle") {
      if (this.queueOpen.has(world)) this.queueOpen.delete(world); else this.queueOpen.add(world);
      this.cancelArmed = null;
    } else if (action === "world-cancel") this.cancelArmed = Number(button.dataset.job);
    else if (action === "world-cancel-keep") this.cancelArmed = null;
    else if (action === "world-cancel-confirm") {
      const id = Number(button.dataset.job);
      const job = structureQueue(dynamic, body, this.ctx.state.galaxy?.build_options ?? [], liveSimTime()).find((entry) => entry.job.id === id);
      if (job?.cancellable) {
        this.ctx.send({ type: "CancelBuild", system_id: system.id, job_id: id });
        this.cancelSent.set(id, liveSimTime());
        this.hooks.notice(`<b>Cancel sent</b> · ${esc(this.jobTitle(job))} at ${esc(body.name)}. Its goods return when the order arrives.`);
      }
      this.cancelArmed = null;
    } else if (action === "world-pending-dismiss") {
      this.pendingBuilds = this.pendingBuilds.filter((order) => !(order.systemId === system.id && order.bodyId === body.id
        && order.key === button.dataset.key));
    }
  }

  private queueWorldBuild(system: SystemInfo, dynamic: SystemStateView, body: BodyView, key: string): void {
    const entry = this.worldOptions(dynamic, body).find((option) => option.key === key);
    if (!entry) return;
    const pools = bodyPoolUsage(body, dynamic);
    const state = structOption(entry, dynamic, body, pools);
    const short = this.shortfall(dynamic, system.id, state.o);
    if (!state.buildable || short.length || this.pendingBuilds.some((order) => order.systemId === system.id && order.bodyId === body.id && order.key === key)) return;
    const model = this.planetModel(system, dynamic, body);
    let slot: number | null = null;
    const site = this.worldBuilder?.site;
    if (state.foundsNew && site && site.pool === state.pool && this.planet.availableSite(model, site.slot)) {
      this.planet.preferSite(model, key, site.slot);
      slot = site.slot;
    } else if (state.foundsNew && state.pool) slot = this.planet.firstFreeSite(model, state.pool);
    const delay = commandDelayTo(system.pos) ?? 0;
    const now = liveSimTime();
    const seen = (body.structures[key] ?? 0) + dynamic.builds.filter((job) => job.body_id === body.id && job.key === key).length;
    this.dispatchBuild(key, system.id, body.id);
    this.pendingBuilds.push({ systemId: system.id, bodyId: body.id, key, tier: state.targetTier, seen, sentAt: now,
      expectBy: now + 2 * delay + 3, slot, costs: state.o.costs.map((cost) => ({ commodity: cost.commodity as Commodity, units: cost.units })) });
    // The site is taken: the sheet goes back to the whole tab.
    if (this.worldBuilder) {
      this.worldBuilder.site = null;
      this.worldBuilder.siteOnly = false;
    }
    this.hooks.notice(`<b>Build order sent</b> · ${esc(state.o.label)} ${roman(state.targetTier)} → ${esc(body.name)}.`);
  }

  /** Goods this order still lacks at the system, after orders in flight. */
  private shortfall(dynamic: SystemStateView, systemId: string, option: BuildOpt): { commodity: Commodity; units: number }[] {
    const have = constructionStock(dynamic, option.key).available;
    const held = this.heldFor(systemId);
    return option.costs.flatMap((cost) => {
      const commodity = cost.commodity as Commodity;
      const free = (have.get(commodity) ?? 0) - (held.get(commodity) ?? 0);
      return free + 1e-9 >= cost.units ? [] : [{ commodity, units: Math.ceil(cost.units - Math.max(0, free)) }];
    });
  }

  private jobTitle(job: StructureJob): string {
    return job.project ? label(job.key) : `${buildName(job.key)} ${roman(job.tier)}`;
  }

  /** The construction bar pinned under the world's header. */
  private constructionBarHtml(system: SystemInfo, dynamic: SystemStateView, body: BodyView, line: readonly StructureJob[]): string {
    const now = liveSimTime();
    const world = `${system.id}:${body.id}`;
    const delay = commandDelayTo(system.pos) ?? 0;
    const pending = this.pendingBuilds.filter((order) => order.systemId === system.id && order.bodyId === body.id);
    const head = line[0];
    const waiting = line.slice(1);
    const open = this.queueOpen.has(world) && waiting.length > 0;
    const actions = `<span class="deck-construction__actions"><button type="button" class="is-primary" data-deck-act="world-build" data-mode="structures" data-coach="world-build">Build</button>${isShipbuildingBody(body) ? `<button type="button" data-deck-act="world-build" data-mode="ships">Build ship</button>` : ""}</span>`;
    const stale = delay >= 2 ? ` · as reported ~${Math.ceil(delay)}s ago` : "";
    const active = head
      ? `<div class="deck-construction__job">${structureImage(head.key, head.tier, "md")}<div><b>${esc(this.jobTitle(head))}</b>${progressBar(head, this.jobTitle(head))}<small>${!head.active ? "Queued · starts next" : head.project ? "Founding Camp project" : (body.structures[head.key] ?? 0) > 0 ? `Level ${roman(body.structures[head.key])} → ${roman(head.tier)}` : "New building"}${stale}</small></div><em>${etaText(head.readyIn)}</em></div>`
      : `<p class="deck-construction__idle">Build slot free</p>`;
    const sent = pending.map((order) => {
      const silent = now > order.expectBy + 15;
      return `<div class="deck-construction__job is-sent">${structureImage(order.key, order.tier, "md")}<div><b>${esc(buildName(order.key))} ${roman(order.tier)}</b><small${silent ? ' class="is-warn"' : ""}>${silent ? "No report yet · it may have been refused" : `Order sent · confirms in ${fmtBuildDur(Math.max(1, order.expectBy - now))}`}</small></div>${silent ? `<button type="button" data-deck-act="world-pending-dismiss" data-key="${esc(order.key)}">Dismiss</button>` : "<em>…</em>"}</div>`;
    }).join("");
    const next = waiting[0];
    const more = next ? `<button type="button" class="deck-construction__more" data-deck-act="world-queue-toggle" aria-expanded="${open}"><span>${waiting.length} queued · next <b>${esc(this.jobTitle(next))}</b>${next.startsIn !== null && next.startsIn > 0.5 ? `, starts in ${fmtBuildDur(next.startsIn)}` : ""}</span><span aria-hidden="true">${open ? "▾" : "▸"}</span></button>` : "";
    const rows = open ? `<ol class="deck-construction__queue">${waiting.map((job) => {
      const control = job.job.id !== undefined && this.cancelSent.has(job.job.id) ? `<span class="deck-construction__note-inline">Cancel sent</span>`
        : job.cancellable && this.cancelArmed === job.job.id ? `<span class="deck-construction__confirm"><button type="button" class="is-danger" data-deck-act="world-cancel-confirm" data-job="${job.job.id}">Confirm cancel</button><button type="button" data-deck-act="world-cancel-keep">Keep</button></span>`
        : job.cancellable ? `<button type="button" data-deck-act="world-cancel" data-job="${job.job.id}">Cancel</button>` : "";
      return `<li><span class="deck-construction__pos">#${job.position}</span>${structureImage(job.key, job.tier, "sm")}<div><b>${esc(this.jobTitle(job))}</b><small>${job.startsIn === null ? "starts after the jobs ahead" : job.startsIn > 0.5 ? `starts in ${fmtBuildDur(job.startsIn)}` : "starts next"}${job.readyIn !== null ? ` · ready in ${fmtBuildDur(job.readyIn)}` : ""}</small></div>${control}</li>`;
    }).join("")}</ol>${waiting.some((job) => job.cancellable) ? `<p class="deck-construction__note">Cancel returns the goods.</p>` : ""}` : "";
    const hulls = hullJobs(dynamic, body);
    const yard = hulls.length ? `<div class="deck-construction__hulls"><small>Hulls in the yard</small>${hulls.map((job) => {
      const left = job.complete_time == null ? null : Math.max(0, job.complete_time - now);
      return `<span>${esc(buildName(job.key))} · ${job.queued ? "waiting for a slip" : left === null ? "paused · needs workforce" : etaText(left)}</span>`;
    }).join("")}</div>` : "";
    const sheet = this.worldBuilder?.world === world && !this.worldBuilder.placing;
    return `<section class="deck-construction${sheet ? " is-building" : ""}" aria-label="Construction on ${esc(body.name)}"><header><h3>Construction</h3>${actions}</header>${active}${sent}${more}${rows}${yard}</section>`;
  }

  /** §build-sheet: building opens across the planet view, so the world's
   * column, its construction bar and its queue stay in sight. The tabs are the
   * world's slot types; each shows upgrades of what stands, new buildings that
   * fit, and, folded, what can't be built here and why. The detail column
   * carries the level ladder, the figures, the costs and Queue. */
  private worldSheetHtml(system: SystemInfo, dynamic: SystemStateView, body: BodyView, line: readonly StructureJob[], art: string): string {
    const wb = this.worldBuilder!;
    const pools = bodyPoolUsage(body, dynamic);
    const pending = this.pendingBuilds.filter((order) => order.systemId === system.id && order.bodyId === body.id);
    const surface = planetSurface(art, system.id, body);
    const site = wb.siteOnly ? wb.site : null;
    if (site) wb.tab = site.pool;
    const tab = this.worldOptions(dynamic, body).map((entry) => this.sheetEntry(dynamic, system.id, body, entry, line, pending, pools))
      .filter((item) => item.state.pool === wb.tab);
    const upgrades = site ? [] : tab.filter((item) => item.existing && !item.state.maxed);
    const fresh = tab.filter((item) => !item.existing && !item.block);
    const blocked = tab.filter((item) => item.state.maxed || (!item.existing && !!item.block));
    const shown = [...upgrades, ...fresh, ...blocked];
    if (!shown.some((item) => item.key === this.selectedBuild)) {
      this.selectedBuild = ([...fresh, ...upgrades].find((item) => !item.block && !item.short.length && !item.order)
        ?? fresh[0] ?? upgrades[0] ?? blocked[0])?.key ?? this.selectedBuild;
    }
    const pick = shown.find((item) => item.key === this.selectedBuild);
    // Placing: the sheet folds away while the player picks a lit site.
    if (wb.placing) {
      if (pick && this.placeable(pick)) {
        return `<div class="deck-bsheet-place" role="status"><span>Place <b>${esc(pick.state.o.label)} ${roman(pick.state.targetTier)}</b> on a lit site</span><button type="button" class="is-primary" data-deck-act="world-builder-place-keep">Use this one</button><button type="button" data-deck-act="world-builder-place-back">Back</button></div>`;
      }
      wb.placing = false;
    }
    // Slots in use, counting new buildings whose orders are still in flight.
    const used = (pool: Pool) => pools[pool].used + pending.filter((order) => order.slot !== null && POOL_OF[order.key] === pool
      && !(body.structures[order.key] > 0) && !dynamic.builds.some((job) => job.body_id === body.id && job.key === order.key)).length;
    const tabs = (["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => {
      const taken = used(pool), total = pools[pool].total;
      const pct = total > 0 ? Math.min(100, taken / total * 100) : 100;
      return `<button type="button" class="deck-bsheet__tab" role="tab" aria-selected="${wb.tab === pool}" data-deck-act="world-builder-tab" data-pool="${pool}"${site && pool !== site.pool ? " disabled" : ""}><b>${POOL_LABEL[pool]}<small>${taken}/${total}</small></b><span class="deck-bsheet__meter${taken >= total ? " is-full" : ""}"><i style="width:${pct.toFixed(0)}%"></i></span></button>`;
    }).join("");
    const free = Math.max(0, pools[wb.tab].total - used(wb.tab));
    const cards = (items: readonly SheetEntry[]) => `<div class="deck-bsheet__cards">${items.map((item) => this.sheetCard(item, body, surface)).join("")}</div>`;
    const sections: string[] = [];
    if (upgrades.length) sections.push(`<section class="deck-bsheet__sec"><h3>Upgrade</h3>${cards(upgrades)}</section>`);
    sections.push(`<section class="deck-bsheet__sec"><h3>New <small>${free} free site${free === 1 ? "" : "s"}</small></h3>${fresh.length ? cards(fresh) : free ? "<p>Nothing new fits here.</p>" : ""}</section>`);
    if (blocked.length) {
      const open = wb.blocked.includes(wb.tab);
      sections.push(`<section class="deck-bsheet__sec is-blocked"><button type="button" class="deck-bsheet__more" data-deck-act="world-builder-blocked" aria-expanded="${open}">${open ? "▾" : "▸"} Can't build here (${blocked.length})</button>${open ? cards(blocked) : ""}</section>`);
    }
    const chip = site ? `<div class="deck-bsheet__site"><span>Free ${POOL_LABEL[site.pool]} site</span><button type="button" data-deck-act="world-builder-all">Show all</button></div>` : "";
    return `<section class="deck-bsheet" role="dialog" aria-label="Build on ${esc(body.name)}"><header class="deck-bsheet__head"><h2>Build on ${esc(body.name)}</h2><div class="deck-bsheet__tabs" role="tablist" aria-label="Slot type">${tabs}</div><button type="button" class="is-primary" data-deck-act="world-builder-exit">Done</button></header>${chip}<div class="deck-bsheet__body"><div class="deck-bsheet__list">${sections.join("")}</div>${this.sheetDetailHtml(system, dynamic, body, pick, line, surface)}</div></section>`;
  }

  private worldOptionState(dynamic: SystemStateView, systemId: string, body: BodyView, entry: BuildOpt, line: readonly StructureJob[],
    pending: readonly PendingBuild[], pools: PoolUse) {
    const state = structOption(entry, dynamic, body, pools);
    return { state, short: this.shortfall(dynamic, systemId, state.o), order: pending.find((order) => order.key === entry.key),
      jobs: line.filter((job) => job.key === entry.key) };
  }

  private sheetEntry(dynamic: SystemStateView, systemId: string, body: BodyView, entry: BuildOpt, line: readonly StructureJob[],
    pending: readonly PendingBuild[], pools: PoolUse): SheetEntry {
    const { state, short, order, jobs } = this.worldOptionState(dynamic, systemId, body, entry, line, pending, pools);
    // A new building whose order is still in flight stays with the new ones.
    return { key: entry.key, entry, state, short, order, jobs, existing: state.currentTier > 0 || jobs.length > 0,
      block: state.maxed ? "Top level" : state.gate ?? "" };
  }

  /** A new building that takes a ground site the player may choose. */
  private placeable(item: SheetEntry): boolean {
    return item.state.foundsNew && !item.block && !item.order && !isOrbitalYard(item.key) && item.entry.consumes_slot !== false;
  }

  private sheetCard(item: SheetEntry, body: BodyView, surface: PlanetSurface | null): string {
    const { state } = item;
    const resting = state.maxed || (!!item.block && item.existing);
    const tier = resting ? Math.max(1, state.currentTier) : item.block ? 1 : state.targetTier;
    const level = state.maxed ? "Top" : item.block ? "" : state.foundsNew ? "New" : `${roman(state.targetTier - 1)} → ${roman(state.targetTier)}`;
    const waiting = item.jobs.filter((job) => !job.active).at(-1);
    const active = item.jobs.find((job) => job.active);
    const status = item.order ? `<span class="deck-bsheet__status is-quiet">Order sent</span>`
      : waiting ? `<span class="deck-bsheet__status is-quiet">${roman(waiting.tier)} queued #${waiting.position}</span>`
      : active ? `<span class="deck-bsheet__status">${roman(active.tier)} building · ${etaText(active.readyIn)}</span>` : "";
    const short = new Map(item.short.map((entry) => [entry.commodity, entry.units]));
    const facts = item.block ? `<span class="deck-bsheet__why">${esc(item.block)}</span>`
      : `<span class="deck-bsheet__line">${state.o.costs.map((cost) => {
        const missing = short.get(cost.commodity as Commodity);
        return `<span${missing ? ' class="is-short"' : ""} title="${esc(label(cost.commodity))}${missing ? ` · ${fmt(missing)} short` : ""}">${commodityGlyph(cost.commodity as Commodity)}${cost.units}</span>`;
      }).join("")}<span>${fmtBuildDur(structureBuildSeconds(state.o, body))}</span></span>`;
    return `<button type="button" class="deck-bsheet__card${item.block ? " is-blocked" : ""}" data-deck-act="world-builder-pick" data-key="${esc(item.key)}" aria-pressed="${item.key === this.selectedBuild}"><span class="deck-bsheet__art">${planetStructureImage(item.key, tier, "120px", false, surface)}</span>${level ? `<span class="deck-bsheet__level">${level}</span>` : ""}${status}<b>${esc(state.o.label)}</b>${facts}</button>`;
  }

  private sheetDetailHtml(system: SystemInfo, dynamic: SystemStateView, body: BodyView, item: SheetEntry | undefined,
    line: readonly StructureJob[], surface: PlanetSurface | null): string {
    if (!item) return `<aside class="deck-bsheet__detail">${emptyState("Nothing to build here", "Every building this world can take is built or under way.")}</aside>`;
    const { state, entry, key } = item;
    const resting = state.maxed || (!!item.block && item.existing);
    const shown = resting ? Math.max(1, state.currentTier) : item.block ? 1 : state.targetTier;
    const top = maxLevel(entry);
    const ladder = top > 1 ? `<div class="deck-bsheet__ladder" style="--levels:${top}" aria-label="Levels">${Array.from({ length: top }, (_, index) => index + 1).map((tier) => {
      // Catalogued levels name their research; V and VI of any other
      // building need research only the sim can check.
      const level = entry.tiers?.find((candidate) => candidate.tier === tier);
      const locked = level?.research_prerequisite && !structureResearched(structureLevel(entry, tier)) ? level.research_prerequisite : null;
      const research = level ? !!locked : top === 6 && tier >= 5;
      const tip = locked ? `Needs ${this.programmeName(locked)} research` : research ? "Levels V and VI need research" : "";
      return `<span class="deck-bsheet__rung${tier <= state.currentTier ? " is-have" : ""}${tier === state.targetTier && !item.block && !state.maxed ? " is-next" : ""}${research ? " is-research" : ""}"${tip ? ` title="${esc(tip)}"` : ""}>${planetStructureImage(key, tier, "48px")}<small>${roman(tier)}</small></span>`;
    }).join("")}</div>` : "";
    const step = `Level ${roman(state.targetTier - 1)} → ${roman(state.targetTier)}`;
    const kind = item.block ? `<span class="is-warn">${esc(item.block)}</span>`
      : state.foundsNew ? entry.consumes_slot === false ? "New · no slot used" : `New · ${POOL_LABEL[state.pool]}`
      : entry.consumes_slot === false ? `${step} · no slot used` : step;
    const title = entry.tiers?.length ? state.o.label : `${state.o.label} ${roman(shown)}`;
    const description = STRUCTURE_DESCRIPTION[key] ?? state.o.description ?? "";
    const facts = item.block || state.maxed ? [] : structureFacts(entry, state.targetTier - 1, state.targetTier, body);
    const factHtml = facts.length ? `<dl class="deck-bsheet__facts">${facts.map((fact) => `<dt>${esc(fact.label)}</dt><dd>${fact.from ? `${esc(fact.from)} → ` : ""}<b>${esc(fact.to)}</b></dd>`).join("")}</dl>` : "";
    const have = constructionStock(dynamic, key).available;
    const held = this.heldFor(system.id);
    const costs = item.block || state.maxed || !state.o.costs.length ? "" : `<ul class="deck-bsheet__costs">${state.o.costs.map((cost) => {
      const commodity = cost.commodity as Commodity;
      const free = Math.max(0, (have.get(commodity) ?? 0) - (held.get(commodity) ?? 0));
      const short = free + 1e-9 < cost.units;
      return `<li${short ? ' class="is-short"' : ""}>${commodityGlyph(commodity)}<span>${cost.units} ${esc(label(commodity))}</span><small>${short ? `${fmt(Math.ceil(cost.units - free))} short` : `have ${fmt(free)}`}</small></li>`;
    }).join("")}</ul>`;
    const chosen = this.worldBuilder?.site?.pool === state.pool;
    const where = this.placeable(item) ? `<div class="deck-bsheet__where"><span>${chosen ? "Your site" : "Site picked for you"}</span><button type="button" data-deck-act="world-builder-place">Choose on planet</button></div>` : "";
    const delay = commandDelayTo(system.pos) ?? 0;
    const timing = nextOrderTiming(line, structureBuildSeconds(state.o, body), delay);
    const warn = !item.order && !item.block && item.short.length > 0;
    const when = item.block || state.maxed ? ""
      : item.order ? `Confirms in ${fmtBuildDur(Math.max(1, item.order.expectBy - liveSimTime()))}`
      : item.short.length ? `Short ${item.short.map((entry) => `${fmt(entry.units)} ${label(entry.commodity)}`).join(", ")}`
      : timing.readyIn === null ? "Starts after the jobs ahead"
      : (timing.startsIn ?? 0) >= 1 ? `Starts in ${fmtBuildDur(timing.startsIn!)} · ready in ${fmtBuildDur(timing.readyIn)}`
      : `Ready in ${fmtBuildDur(timing.readyIn)}`;
    const name = state.foundsNew ? `${state.o.label} ${roman(state.targetTier)}` : entry.tiers?.length ? state.o.label : `level ${roman(state.targetTier)}`;
    const actions = item.order ? `<button type="button" class="is-primary" disabled>Order sent · confirming</button>`
      : item.block || state.maxed ? ""
      : item.short.length || !state.buildable ? `<button type="button" class="is-primary" disabled>Queue ${esc(name)}</button><button type="button" data-deck-act="world-builder-market">Buy at Market</button>`
      : `<button type="button" class="is-primary" data-deck-act="world-builder-queue" data-key="${esc(key)}" data-coach="queue-build">Queue ${esc(name)}</button>`;
    return `<aside class="deck-bsheet__detail" aria-label="Selected building"><span class="deck-bsheet__art deck-bsheet__hero">${planetStructureImage(key, shown, "220px", false, surface)}</span>${ladder}<div class="deck-bsheet__title"><h3>${esc(title)}</h3><p>${kind}</p></div>${description ? `<p class="deck-bsheet__desc">${esc(description)}</p>` : ""}${factHtml}${costs}${where}${when ? `<p class="deck-bsheet__when${warn ? " is-warn" : ""}">${when}</p>` : ""}${actions ? `<div class="deck-bsheet__actions">${actions}</div>` : ""}</aside>`;
  }

  private programmeName(id: string): string {
    return this.ctx.state.research?.programmes.find((entry) => entry.id === id)?.name
      ?? this.ctx.state.researchCatalog?.find((entry) => entry.id === id)?.name ?? label(id);
  }

  /** §construction: the world's pinned bar, its builder and the queue facts the
   * scene and rows draw from. Owner-only; a rival world gets none of it. */
  private constructionModel(system: SystemInfo, dynamic: SystemStateView, body: BodyView, art = ""): Pick<PlanetPanelModel, "constructionHtml" | "builderHtml" | "construction"> {
    if (dynamic.owner !== this.ctx.state.playerId) return { constructionHtml: "", builderHtml: "", construction: null };
    const line = structureQueue(dynamic, body, this.ctx.state.galaxy?.build_options ?? [], liveSimTime());
    const building = this.worldBuilder?.world === `${system.id}:${body.id}`;
    // The sheet first: it settles the pick, and may leave placing mode.
    const builderHtml = building ? this.worldSheetHtml(system, dynamic, body, line, art) : "";
    const pick = building ? this.worldOptions(dynamic, body).find((entry) => entry.key === this.selectedBuild) : undefined;
    const pickState = pick ? structOption(pick, dynamic, body, bodyPoolUsage(body, dynamic)) : null;
    const pending = this.pendingBuilds.filter((order) => order.systemId === system.id && order.bodyId === body.id);
    // A new building with no other gate may be placed; one already on its way
    // is not a new site to preview.
    const fresh = !!pickState?.foundsNew && !pickState.gate && !pickState.maxed && !pending.some((order) => order.key === pick?.key);
    return {
      constructionHtml: this.constructionBarHtml(system, dynamic, body, line),
      builderHtml,
      construction: {
        line,
        pending: pending.map((order) => ({ key: order.key, tier: order.tier, slot: order.slot })),
        build: building ? {
          key: pick?.key ?? "", tier: pickState?.targetTier ?? 1, fresh,
          pool: pickState?.pool ?? null, site: this.worldBuilder?.site?.slot ?? null, placing: !!this.worldBuilder?.placing,
        } : null,
      },
    };
  }

  private planetModel(system: SystemInfo, dynamic: SystemStateView, body: BodyView): PlanetPanelModel {
    const mine = dynamic.owner === this.ctx.state.playerId;
    const profile = `<div class="deck-stat-grid">${stat("Environment", label(body.environment))}${stat("Size", label(body.size))}${stat("Geology", body.geology ? label(body.geology) : "Unsurveyed")}${stat("Construction", `×${body.construction_time_mult.toFixed(2)}`)}${stat("Habitat", `×${body.habitat_capacity_mult.toFixed(2)}`)}${stat("Settlement", `×${body.population_growth_mult.toFixed(2)}`)}</div>`;
    const feature = body.special
      ? `<article class="deck-world-feature"><small>Rare planetary feature</small><b>${esc(label(body.special))}</b><span>${esc(body.special_effect ?? "Rare planetary feature")}</span></article>`
      : "";
    const depositRow = (deposit: Deposit, pocket = false) => `<div class="deck-deposit${pocket ? " is-pocket" : ""}"><span>${commodityGlyph(deposit.resource)}<span><b>${esc(label(deposit.resource))}${pocket ? " pocket" : ""}</b><small>${deposit.reserves === null ? "Renewable deposit" : deposit.reserves <= 0 ? "Depleted" : `${fmt(deposit.reserves)} ${pocket ? "left" : "reserves"}`}</small></span></span><em>×${deposit.richness.toFixed(2)}</em></div>`;
    // §pockets: a belt adds the pockets you've found and how often it's been prospected.
    const belt = body.kind === "asteroid_belt" && body.deposits !== null;
    const pockets = belt ? (body.pockets ?? []).map((deposit) => depositRow(deposit, true)).join("") : "";
    const prospected = belt ? `<p class="deck-muted deck-prospected">${esc(prospectLine(body))}</p>` : "";
    const deposits = body.deposits === null
      ? `<div class="deck-empty-inline">Geology has not been surveyed.</div>`
      : body.deposits.length || pockets
        ? body.deposits.map((deposit) => depositRow(deposit)).join("") + pockets + prospected
        : `<div class="deck-empty-inline">Surveyed · no extractable deposits.</div>${prospected}`;
    const roles = (dynamic.opportunities ?? []).filter((entry) => entry.body_id === body.id);
    const roleHtml = roles.length ? `<div class="deck-opportunities">${roles.map((entry) => `<article class="deck-opportunity is-${entry.tier}"><small>${esc(label(entry.tier))}</small><b>${esc(entry.title)} · ×${entry.score.toFixed(2)}</b><span>${esc(entry.reason)}</span></article>`).join("")}</div>` : "";
    const publicSections = `${this.colonyPurposeHtml(dynamic, body)}<section class="deck-section"><h3>Planet profile</h3>${profile}${feature}${roleHtml}</section><section class="deck-section"><h3>Deposits</h3>${deposits}</section>${this.beltMiningHtml(dynamic, body)}`;
    return {
      system, report: dynamic, body, mine, art: bodyArtUrl(system.id, body), delay: commandDelayTo(system.pos),
      catalog: this.ctx.state.galaxy?.build_options ?? [], workforceStructures: WORKFORCE_STRUCTURES,
      descriptions: STRUCTURE_DESCRIPTION, surveyHtml: publicSections,
      populationHtml: mine ? this.worldPopulation(system.id, body) : "",
      ...this.constructionModel(system, dynamic, body, bodyArtUrl(system.id, body)),
      economy: this.ctx.state,
      siege: this.planetSiege(system, dynamic, body),
      docked: this.planetDocked(system, dynamic, body),
    };
  }

  /// A siege of this world, from the participant-only blockade/ground fields
  /// and an arrived landing record — the same world the System View rings.
  private planetSiege(system: SystemInfo, dynamic: SystemStateView, body: BodyView): PlanetSiege | null {
    const landing = this.ctx.state.groundRecords.find((record) => record.system === system.id && record.outcome === null);
    if (!dynamic.blockade && !landing) return null;
    if (siegeTarget(anchorBodies(dynamic)) !== body.id) return null;
    const pct = (value: number) => `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
    const base = this.ctx.state.galaxy?.siege_secs ?? 0;
    const caption = dynamic.blockade ? blockadeBadge(dynamic.blockade, liveSimTime(), base) : "LANDING UNDER WAY";
    const latest = landing?.rounds[landing.rounds.length - 1];
    const suppression = dynamic.ground?.suppression ?? latest?.suppression ?? 0;
    const detail = [
      landing ? `LANDING ${pct(latest?.marines_frac ?? 1)} vs ${pct(latest?.defenders_frac ?? 1)}` : "",
      suppression > 0.02 ? `GARRISON PINNED ${pct(suppression)}` : "",
    ].filter(Boolean).join(" · ");
    return { caption, detail, suppression, landingId: landing?.id ?? null };
  }

  /// §planet-orbit: fleets berthed at this system whose facility is on this
  /// world — the System View's berth rule, from the same light-gated ghosts —
  /// so a hull docked beside a world in the orrery is in that world's sky.
  private planetDocked(system: SystemInfo, dynamic: SystemStateView, body: BodyView): PlanetDockedFleet[] {
    const anchors = anchorBodies(dynamic);
    const state = this.ctx.state;
    return state.ghosts
      .filter((ghost) => dockedAtSystem(ghost, system.id) && berthAnchor(ghost.kind, anchors) === body.id)
      .map((ghost) => ({
        id: ghost.id, kind: ghost.migrant ? "colony" : ghost.kind, own: ghost.own, count: fleetExactCount(ghost),
        name: ghost.migrant ? "Migrant Liner" : ghost.rescue_service ? "AAA Rescue Tender" : shipKindLabel(ghost.kind),
        selected: state.selectedShipId === ghost.id || state.selectedShipIds.has(ghost.id),
      }));
  }

  private worldPopulation(systemId: string, body: BodyView): string {
    const policyButtons = (["closed", "managed", "open", "priority"] as const).map((policy) => `<button type="button" data-deck-act="migration-policy" data-policy="${policy}" aria-pressed="${body.migration_policy === policy}">${esc(label(policy))}</button>`).join("");
    const cohort = this.ctx.state.galaxy?.migrant_cohort_people ?? 1_000;
    const options = this.ctx.state.systems
      .filter((entry) => entry.owner === this.ctx.state.playerId && entry.blockade?.held_since == null && entry.habitat_fed)
      .flatMap((entry) => entry.bodies.filter((candidate) => !(entry.id === systemId && candidate.id === body.id) && candidate.population > 0).map((candidate) => `<option value="${esc(entry.id)}|${candidate.id}">${esc(systemName(this.ctx, entry.id))} · ${esc(candidate.name)}</option>`)).join("");
    return `<div class="deck-stat-grid">${stat("Population", fmtPopulation(body.population))}${stat("Inbound", `${fmt(body.inbound_migrants)} people`)}${stat("Food use", `×${body.provisions_mult.toFixed(2)}`)}${stat("Settlement appeal", `×${body.population_growth_mult.toFixed(2)}`)}</div><div class="deck-policy"><span>Immigration policy</span>${policyButtons}</div><div class="deck-inline-form"><select data-deck-relocation-target>${options}</select><button type="button" data-deck-act="relocate-migrants" ${options && body.population * 1_000_000 >= cohort * 2 ? "" : "disabled"}>Relocate ${cohort.toLocaleString()}</button></div>`;
  }

  private structureBuilder(dynamic: SystemStateView, body: BodyView, poolFilter?: Pool): string {
    const pools = bodyPoolUsage(body, dynamic);
    const options = (this.ctx.state.galaxy?.build_options ?? [])
      .filter((entry) => !SHIP_KEYS.has(entry.key) && !entry.key.startsWith("module:") && !!POOL_OF[entry.key]
        && (!poolFilter || (POOL_OF[entry.key] === poolFilter && !body.structures[entry.key]
          && !dynamic.builds.some(job => job.body_id === body.id && job.key === entry.key)))
        && structureResearched(entry)) as BuildOpt[];
    if (!options.some((entry) => entry.key === this.selectedBuild)) this.selectedBuild = options[0]?.key ?? "";
    const poolBars = (["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => {
      const usage = pools[pool];
      const pct = usage.total > 0 ? Math.min(100, usage.used / usage.total * 100) : 100;
      return `<span class="deck-pool${usage.used >= usage.total ? " is-full" : ""}"><small>${POOL_LABEL[pool]}</small><b>${usage.used}/${usage.total}</b><i><span style="width:${pct.toFixed(1)}%"></span></i></span>`;
    }).join("");
    const groups = (["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => {
      // Catalog placement only: Shipyard leads Infrastructure, but still uses
      // its Industrial slot and recipe through structOption / POOL_OF.
      const entries = options.filter((entry) => (!poolFilter && entry.key === "shipyard" ? "infrastructure" : POOL_OF[entry.key]) === pool);
      if (pool === "infrastructure") entries.sort((a, b) => Number(b.key === "shipyard") - Number(a.key === "shipyard"));
      const rows = entries.map((entry) => {
        const state = structOption(entry, dynamic, body, pools);
        return `<button type="button" class="deck-builder-row${entry.key === this.selectedBuild ? " is-selected" : ""}${state.buildable ? "" : " is-disabled"}" data-deck-act="builder-select" data-key="${esc(entry.key)}"><span>${structureImage(entry.key, state.targetTier, "sm", undefined, "deck-structure-icon")}<b>${esc(state.o.label)}</b></span></button>`;
      }).join("");
      return rows ? `<h3 class="deck-builder-group">${POOL_LABEL[pool]}</h3>${rows}` : "";
    }).join("");
    const selected = options.find((entry) => entry.key === this.selectedBuild);
    const detail = selected ? this.structureDetail(dynamic, body, selected, pools) : poolFilter
      ? emptyState("No new structures available", "Use All structures to view upgrades.")
      : emptyState("Choose a structure", "Inspect its recipe, slot and build time.");
    const commit = selected ? this.structureCommit(dynamic, body, selected, pools) : "";
    const filter = poolFilter ? `<div class="deck-world-actions"><b>${POOL_LABEL[poolFilter]} slot</b><button type="button" data-deck-act="builder-all-structures">All structures</button></div>` : "";
    return `${filter}<div class="deck-pools">${poolBars}</div><div class="deck-builder">${commit}<div class="deck-builder__list">${groups}</div><div class="deck-builder__detail">${detail}</div></div>`;
  }

  private structureCommit(dynamic: SystemStateView, body: BodyView, option: BuildOpt, pools: ReturnType<typeof bodyPoolUsage>): string {
    const state = structOption(option, dynamic, body, pools);
    option = state.o;
    return `<div class="deck-builder__commit"><span><small>Selected structure</small><b>${esc(option.label)}</b></span><button type="button" class="is-primary" data-deck-act="builder-queue" data-coach="queue-build" ${state.buildable ? "" : "disabled"}>Queue build</button></div>`;
  }

  private structureDetail(dynamic: SystemStateView, body: BodyView, option: BuildOpt, pools: ReturnType<typeof bodyPoolUsage>): string {
    const state = structOption(option, dynamic, body, pools);
    option = state.o;
    if (state.maxed) return `<article class="deck-build-detail"><header>${structureImage(option.key, state.targetTier, "md", undefined, "deck-structure-icon")}<span><small>Maximum level built or queued</small><h3>${esc(option.label)}</h3></span></header><p>${esc(option.description ?? "")}</p><div class="deck-build-warning">${esc(state.reason)}</div></article>`;
    const supply = constructionStock(dynamic, option.key).available;
    const costs = option.costs.map((cost) => {
      const commodity = cost.commodity as Commodity;
      const have = supply.get(commodity) ?? 0;
      return `<tr${have < cost.units ? ` class="is-short"` : ""}><th scope="row"><span>${commodityGlyph(commodity)} ${esc(label(commodity))}</span></th><td>${cost.units}</td><td>${fmt(have)}</td></tr>`;
    }).join("");
    const attachment = option.key === "service_berth";
    const category = attachment ? "Warehouse attachment" : state.foundsNew ? POOL_LABEL[state.pool] : `${POOL_LABEL[state.pool]} · upgrade to tier ${state.targetTier}`;
    const slot = option.key === "command_center" ? "Dedicated civic footprint · no extra slot" : option.key === "power_plant" ? "Own slot · one per planet" : attachment ? "Attached to Warehouse · no extra slot" : state.foundsNew ? `${POOL_LABEL[state.pool]} ${pools[state.pool].used} → ${pools[state.pool].used + 1}/${pools[state.pool].total}` : "Deepens in place";
    const recipe = option.key === "smelter" ? option.refining_recipes?.find(r => body.deposits?.some(d => d.resource === r.inputs[0][0])) ?? option.conversion : undefined;
    const comparison = recipe ? refiningComparison(this.ctx.state, dynamic, body.id, recipe) : "";
    const description = option.description ?? STRUCTURE_DESCRIPTION[option.key] ?? "Adds a new capability to this world.";
    return `<article class="deck-build-detail"><header>${structureImage(option.key, state.targetTier, "md", undefined, "deck-structure-icon")}<span><small>${esc(category)}</small><h3>${esc(option.label)}</h3></span></header><p>${esc(description)}</p>${option.conversion ? `<p class="deck-production-recipe">${esc(conversionSummary(option))}</p>` : ""}<table class="deck-cost-table" aria-label="Construction requirements"><thead><tr><th aria-label="Resource"></th><th scope="col">Cost</th><th scope="col">Stock</th></tr></thead><tbody>${costs}</tbody></table><dl><div><dt>Build time</dt><dd>${fmtBuildDur(structureBuildSeconds(option, body))}</dd></div><div><dt>Slot</dt><dd>${slot}</dd></div></dl>${state.reason ? `<div class="deck-build-warning">${esc(state.reason)}</div>` : ""}${comparison}</article>`;
  }

  private shipBuilder(dynamic: SystemStateView, body: BodyView): string {
    const options = SHIP_ORDER.filter(hullResearched).map((kind) => buildOption(kind)).filter((entry): entry is BuildOpt => !!entry);
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
    const supply = constructionStock(dynamic, option.key).available;
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
    return `<article class="deck-build-detail"><header>${icon(shipIcon(hull), "lg", undefined, "deck-hull-icon")}<span><small>${esc(YARD_TITLE[gate.yard] ?? label(gate.yard))} · Tier ${gate.tier}</small><h3>${esc(buildName(hull))}</h3></span></header><p>${esc(stats?.role ?? "Fleet hull")}</p><ship-model-preview kind="${hull}"></ship-model-preview><div class="deck-quantity"><span>Quantity</span>${quantities}</div><div class="deck-costs">${costs}</div><dl><div><dt>Build time</dt><dd>${buildTime}</dd></div><div><dt>Site</dt><dd>${esc((state.yardBody ?? body).name)} · ×${siteTime.toFixed(2)}</dd></div></dl>${stats ? `<div class="deck-hull-stats">${stat("Speed", fmt(stats.speed))}${stat("Hull", fmt(stats.hull))}${stat("Attack", fmt(stats.atk))}${stat("Defense", fmt(stats.def))}</div>` : ""}${fit}${state.reason ? `<div class="deck-build-warning">${esc(state.reason)}</div>` : ""}${!fitOkay ? `<div class="deck-build-warning">The composed fit exceeds this hull's fitting budget or module slots.</div>` : ""}</article>`;
  }

  private fitPicker(dynamic: SystemStateView, hull: ShipKind): string {
    const ledger = moduleLedgerAt(dynamic.id);
    this.pendingFit = this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0);
    const slots = MODULE_SLOTS[hull] ?? 0;
    if (!slots) return `<section class="deck-fit"><h4>Fitting</h4><span class="deck-muted">This hull has no module slots.</span></section>`;
    const available = MODULES.filter((entry) => (ledger[entry.kind] ?? 0) > 0 && moduleFitsHull(entry.kind, hull));
    const chips = available.map((entry) => `<button type="button" data-deck-act="builder-fit" data-module="${entry.kind}" aria-pressed="${this.pendingFit.includes(entry.kind)}">${icon(entry.icon, "sm")} ${esc(entry.label)} · ${ledger[entry.kind]}</button>`).join("");
    const effective = this.effectiveFit(dynamic.id, hull);
    const used = effective.reduce((sum, module) => sum + (MODULES.find((entry) => entry.kind === module)?.fit ?? 0), 0);
    const total = FITTING_POINTS[hull] ?? 0;
    const pct = total > 0 ? Math.min(100, used / total * 100) : 0;
    const saved = (this.ctx.state.syndicate?.fits ?? []).filter((entry) => entry.kind === hull).map((entry) => `<span class="deck-saved-fit"><button type="button" data-deck-act="builder-fit-pick" data-name="${esc(entry.name)}">${esc(entry.name)}</button><button type="button" data-deck-act="builder-fit-delete" data-name="${esc(entry.name)}" aria-label="Delete ${esc(entry.name)}">×</button></span>`).join("");
    return `<section class="deck-fit"><h4>Fit next build · ${effective.length}/${slots} slots</h4><div class="deck-fit-bar${used > total ? " is-over" : ""}"><span style="width:${pct.toFixed(1)}%"></span><b>${used}/${total} pts</b></div><div class="deck-fit-chips">${chips || `<span class="deck-muted">No modules in this system ledger.</span>`}</div>${this.ctx.state.syndicate ? `<div class="deck-saved-fits">${saved || `<span class="deck-muted">No saved fits for this hull.</span>`}</div><div class="deck-inline-form"><input id="deck-fit-name" data-deck-enter="builder-fit-save" maxlength="24" placeholder="Doctrine fit name"><button type="button" data-deck-act="builder-fit-save" ${effective.length && fitLegal(hull, effective) ? "" : "disabled"}>Save fit</button></div>` : ""}</section>`;
  }

  private moduleForge(dynamic: SystemStateView): string {
    const ledger = moduleLedgerAt(dynamic.id);
    const rows = MODULES.map((entry) => {
      const recipe = buildOption(`module:${entry.kind}`);
      const reason = moduleBuildReason(dynamic, entry.kind);
      const cost = recipe?.costs.map((part) => `${part.units} ${label(part.commodity)}`).join(" · ") ?? "Recipe unavailable";
      return `<div class="deck-forge-row"><span>${icon(entry.icon, "sm")}<span><b>${esc(entry.label)}</b><small>${esc(entry.role)}</small><small>Stock ${ledger[entry.kind] ?? 0} · ${esc(cost)}${reason ? ` · ${esc(reason)}` : ""}</small></span></span><button type="button" data-deck-act="builder-forge" data-module="${entry.kind}" ${reason ? "disabled" : ""}>Build</button></div>`;
    }).join("");
    return `<section class="deck-module-forge"><h4>Equipment</h4>${rows}</section>`;
  }

  private effectiveFit(systemId: string, hull: ShipKind): ModuleKind[] {
    const ledger = moduleLedgerAt(systemId);
    return this.pendingFit.filter((module) => (ledger[module] ?? 0) > 0 && moduleFitsHull(module, hull)).slice(0, MODULE_SLOTS[hull] ?? 0);
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
    mining_complex: MINERAL_DEPOSITS,
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
  if (body.kind === "asteroid_belt") {
    const kind = body.belt?.kind ?? "stony";
    return `<img class="icon icon--md" src="${beltChunkUrl(kind)}" alt="" title="${esc(BELT_META[kind]?.label ?? "Asteroid belt")}">`;
  }
  const key: IconKey = body.environment === "gaia" || body.environment === "terran"
    ? "planetHabitable"
    : body.environment === "uninhabitable" ? "planetUninhabitable" : "planetHostile";
  return icon(key, "md", label(body.environment));
}

function shippableStock(dynamic: SystemStateView) {
  return (dynamic.stockpile ?? []).filter((slot) => !SHIP_FUELS.includes(slot.commodity) && slot.units >= 1);
}

function stat(name: string, value: string, warn = false): string {
  return `<span class="deck-stat${warn ? " is-warn" : ""}"><small>${esc(name)}</small><b>${esc(value)}</b></span>`;
}

function buildName(key: string): string {
  return label(key === "convoy" ? "medium_freighter" : key === "raider" ? "interceptor" : key);
}

function shipIcon(key: string): IconKey {
  const icons: Record<string, IconKey> = {
    scout: "scout", survey_probe: "scout", corvette: "corvette", raider: "raider", convoy: "convoy", colony: "colony",
    tiny_freighter: "tiny_freighter", small_freighter: "small_freighter", large_freighter: "large_freighter", heavy_freighter: "heavy_freighter", bulk_freighter: "bulk_freighter",
    builder: "builder", transport: "transport", freighter: "authorityFreighter", mining_barge: "mining_barge",
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

/** Time left in sim seconds, as the queue reads it. */
function etaText(seconds: number | null): string {
  return seconds === null ? "—" : seconds <= 0.5 ? "completing" : fmtBuildDur(seconds);
}

function progressBar(job: StructureJob, title: string): string {
  return job.progress === null ? "" : `<span class="deck-queue-progress" role="progressbar" aria-label="${esc(title)} build progression" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${job.progress.toFixed(0)}"><i style="width:${job.progress.toFixed(1)}%"></i></span>`;
}

function emptyState(title: string, copy: string): string {
  return `<div class="deck-empty"><b>${esc(title)}</b><span>${esc(copy)}</span></div>`;
}

/** The `world` route beneath a system on the zoom ladder. */
function worldRoute(system: SystemInfo, body: BodyView): DeckRoute {
  return { name: "world", params: { systemId: system.id, systemLabel: system.name, bodyId: String(body.id), worldLabel: body.name } };
}

function worldKey(route: DeckRoute): string {
  return `${route.params?.systemId ?? route.params?.id ?? ""}:${route.params?.bodyId ?? ""}`;
}

function esc(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!);
}

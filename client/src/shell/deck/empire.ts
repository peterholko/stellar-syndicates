import { constructionStock } from "../../core/derive/fleet";
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
import { icon, label, type IconKey } from "../../icons";
import type { AssignmentView, BodyView, Commodity, ModuleKind, ShipKind, SystemInfo, SystemStateView } from "../../protocol";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

export type DeckSystemTab = "overview" | "worlds" | "production";

interface EmpireHooks {
  go(route: DeckRoute): void;
  openGroundViewer(id: string): void;
  notice(html: string): void;
  toast(title: string, message: string, tone?: "quiet" | "good" | "warn" | "bad", destination?: DeckRoute): void;
}

type BuilderMode = "structures" | "ships";
type BuildFeedback = { systemId: string; issuedAt: number; timelineLength: number; text: string; tone: "info" | "good" | "bad" };

const SHIP_ORDER: ShipKind[] = ["scout", "corvette", "raider", "convoy", "colony", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
const SHIP_KEYS = new Set<string>(SHIP_ORDER);
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
  private signature = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: EmpireHooks,
  ) {}

  get composedFit(): ModuleKind[] {
    return this.pendingFit;
  }

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "system" && route?.name !== "build" && route?.name !== "world") return false;
    const context = this.systemContext(route);
    const signature = sheetFingerprint([
      route, this.systemTab, this.builderMode, this.selectedBuild, this.selectedHull,
      this.shipQuantity, this.pendingFit, this.buildFeedback, Math.floor(liveSimTime()),
      context.system, context.dynamic, this.ctx.state.timeline.length, this.ctx.state.syndicate?.fits,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, route.name === "build"
      ? this.buildHtml(route, context.system, context.dynamic)
      : route.name === "world"
        ? this.worldHtml(route, context.system, context.dynamic)
        : this.systemHtml(context.system, context.dynamic));
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name === "build") return this.handleBuildAction(button, route);
    if (route?.name === "world") return this.handleWorldAction(button, route);
    if (route?.name !== "system") return false;
    const action = button.dataset.deckAct;
    const { system, dynamic } = this.systemContext(route);
    if (!system) return false;
    if (action === "watch-ground" && button.dataset.ground) {
      this.hooks.openGroundViewer(button.dataset.ground);
      return true;
    }
    if (action === "system-tab") {
      const tab = button.dataset.tab;
      if (tab === "build") {
        this.hooks.go({
          name: "build",
          params: { systemId: system.id, systemLabel: system.name },
          query: dynamic?.bodies[0] ? { body: String(dynamic.bodies[0].id) } : undefined,
        });
      } else if (isSystemTab(tab)) {
        this.systemTab = tab;
        this.signature = "";
        this.render(route, true);
      }
      return true;
    }
    if (action === "open-world") {
      const body = dynamic?.bodies.find((candidate) => String(candidate.id) === button.dataset.body);
      if (body) this.hooks.go({
        name: "world",
        params: {
          systemId: system.id,
          systemLabel: system.name,
          bodyId: String(body.id),
          worldLabel: body.name,
        },
      });
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
    this.hooks.toast("Build refused", rejected.text, "bad", route?.name === "build" ? route : undefined);
    if (route?.name === "build") this.render(route, true);
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
    const tabs = [
      ["overview", "Overview"], ["worlds", "Worlds"], ["production", "Production"], ["build", "Build"],
    ].map(([tab, text]) => `<button type="button" data-deck-act="system-tab" data-tab="${tab}" aria-selected="${tab === this.systemTab}">${text}</button>`).join("");
    const alert = dynamic?.blockade
      ? `<div class="deck-alert deck-alert--bad"><b>${dynamic.blockade.by_me ? "Blockade established" : "Under blockade"}</b><span>Physical shipping is interdicted while this report remains current.</span></div>`
      : "";
    const active = this.systemTab === "overview"
      ? this.systemOverview(system, dynamic, mine, owner, survey)
      : this.systemTab === "worlds"
        ? this.systemWorlds(system, dynamic, mine)
        : this.systemProduction(system, dynamic, mine);
    return `<section class="deck-page deck-system"><header class="deck-page__lead"><span>${esc(systemFlavor(system, dynamic?.deposits ?? null))}</span><h2>${esc(system.name)}</h2><p>${esc(owner)} · ${esc(survey)}</p></header>${alert}<nav class="deck-tabs" aria-label="System sections">${tabs}</nav>${active}</section>`;
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
    const assignments = dynamic.assignments.length
      ? dynamic.assignments.map((line) => this.assignmentHtml(dynamic, line)).join("")
      : `<div class="deck-empty-inline">No staffed production lines.</div>`;
    return `<section class="deck-section"><header><div><h3>Stockpile</h3><p>Local stock plus cargo on docked owned freighters.</p></div><b>${fmt(dynamic.storage_used)}/${fmt(dynamic.storage_cap)}</b></header><div class="deck-ledger">${stock || `<span><small>Stockpile</small><b>empty</b></span>`}</div>${shipment}</section>` +
      `<section class="deck-section"><header><div><h3>Production lines</h3><p>Output = throughput × staffing × expertise × supply × site.</p></div><b>${dynamic.assignments.length}</b></header><div class="deck-assignment-list">${assignments}</div></section>` +
      this.queueHtml(dynamic);
  }

  private assignmentHtml(dynamic: SystemStateView, line: AssignmentView): string {
    const body = dynamic.bodies.find((entry) => entry.id === line.body_id);
    const output = line.outputs.length
      ? line.outputs.map(([commodity, rate]) => `+${rate.toFixed(2)} ${label(commodity)}/s`).join(" · ")
      : "Idle";
    return `<article class="deck-assignment${line.suspended ? " is-warn" : ""}"><div><b>${esc(line.title)} · Tier ${line.tier}</b><span>${esc(body?.name ?? "System")}${line.suspended ? ` · ${esc(label(line.suspended))}` : ""}</span><small>${esc(output)}</small></div><div class="deck-stepper" aria-label="Workforce assigned"><button type="button" data-deck-act="set-workers" data-body="${line.body_id}" data-structure="${esc(line.structure)}" data-workers="${Math.max(0, line.workers - 1)}" ${line.workers <= 0 ? "disabled" : ""}>−</button><b>${line.workers}</b><button type="button" data-deck-act="set-workers" data-body="${line.body_id}" data-structure="${esc(line.structure)}" data-workers="${line.workers + 1}">+</button></div></article>`;
  }

  private queueHtml(dynamic: SystemStateView): string {
    const now = liveSimTime();
    const rows = [...dynamic.builds].sort((a, b) => a.complete_time - b.complete_time).map((job) => {
      const body = dynamic.bodies.find((entry) => entry.id === job.body_id);
      const duration = Math.max(0, job.complete_time - now);
      return `<div class="deck-queue-row"><span>${icon("queue", "sm")}<span><b>${esc(buildName(job.key))}</b><small>${esc(body?.name ?? "System yard")}</small></span></span><em>${duration > 0 ? fmtBuildDur(duration) : "completing"}</em></div>`;
    }).join("");
    const feedback = this.buildFeedback?.systemId === dynamic.id
      ? `<div class="deck-build-feedback is-${this.buildFeedback.tone}">${esc(this.buildFeedback.text)}</div>`
      : "";
    return `<section class="deck-section"><header><div><h3>Construction queue</h3><p>Completion times follow the live simulation clock.</p></div><b>${dynamic.builds.length}</b></header>${feedback}${rows || `<div class="deck-empty-inline">Nothing under construction.</div>`}</section>`;
  }

  private handleBuildAction(button: HTMLButtonElement, route: DeckRoute): boolean {
    const { system, dynamic } = this.systemContext(route);
    if (!system || !dynamic || dynamic.owner !== this.ctx.state.playerId) return false;
    const action = button.dataset.deckAct;
    if (action === "builder-mode") {
      const mode = button.dataset.mode;
      if (mode === "structures" || mode === "ships") {
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

  private handleWorldAction(button: HTMLButtonElement, route: DeckRoute): boolean {
    const { system, dynamic } = this.systemContext(route);
    const body = dynamic?.bodies.find((entry) => String(entry.id) === route.params?.bodyId);
    if (!system || !dynamic || !body) return false;
    const action = button.dataset.deckAct;
    if (action === "world-build") {
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
      const select = this.root.querySelector<HTMLSelectElement>("#deck-relocation-target");
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
    if (route.query?.mode === "ships" || route.query?.mode === "structures") this.builderMode = route.query.mode;
    const preset = `${dynamic.id}:${route.query?.body ?? ""}:${route.query?.mode ?? ""}:${route.query?.select ?? ""}`;
    if (route.query?.select && preset !== this.lastBuildPreset) {
      if (SHIP_KEYS.has(route.query.select)) this.selectedHull = route.query.select as ShipKind;
      else this.selectedBuild = route.query.select;
      this.lastBuildPreset = preset;
    }
    const body = this.builderBody(route, dynamic);
    if (!body) return emptyState("No build site", "No served world is available in this system.");
    const modeTabs = (["structures", "ships"] as BuilderMode[]).map((mode) => `<button type="button" data-deck-act="builder-mode" data-mode="${mode}" aria-selected="${this.builderMode === mode}">${mode === "structures" ? "Structures" : "Ships & Modules"}</button>`).join("");
    const worlds = dynamic.bodies.map((entry) => `<button type="button" data-deck-act="builder-body" data-body="${entry.id}" aria-selected="${entry.id === body.id}">${esc(entry.name)}</button>`).join("");
    const content = this.builderMode === "structures"
      ? this.structureBuilder(dynamic, body)
      : this.shipBuilder(dynamic, body);
    return `<section class="deck-page deck-build"><header class="deck-page__lead"><span>Local construction · served inventory</span><h2>Build at ${esc(system.name)}</h2><p>Recipes may draw from the system stockpile and owned freighters docked here.</p></header><nav class="deck-tabs" aria-label="Build category">${modeTabs}</nav><div class="deck-builder-worlds"><span>Build site</span>${worlds}</div>${content}${this.queueHtml(dynamic)}</section>`;
  }

  private builderBody(route: DeckRoute, dynamic: SystemStateView): BodyView | undefined {
    const requested = route.query?.body;
    return dynamic.bodies.find((entry) => String(entry.id) === requested) ?? dynamic.bodies[0];
  }

  private worldHtml(route: DeckRoute, system?: SystemInfo, dynamic?: SystemStateView): string {
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
    const publicSections = `<section class="deck-section"><header><div><h3>World summary</h3><p>Public astronomy; mineral grade and deposits require arrived survey light.</p></div></header>${profile}${feature}${roleHtml}</section><section class="deck-section"><header><div><h3>Deposits</h3><p>Natural site richness before staffing, research and structures.</p></div></header>${deposits}</section>`;
    if (!mine) {
      return `<section class="deck-page deck-world-page"><header class="deck-page__lead"><span>${body.habitable ? "Habitable world" : "Observed world"}</span><h2>${esc(body.name)}</h2><p>${esc(system.name)} · private economy hidden</p></header>${publicSections}</section>`;
    }
    const economy = this.worldEconomy(dynamic, body);
    const population = this.worldPopulation(system.id, body);
    const structures = Object.entries(body.structures).filter(([, tier]) => tier > 0).map(([key, tier]) => `<span>${icon(structureIcon(key), "sm")}<small>${esc(label(key))}</small><b>Tier ${tier}</b></span>`).join("");
    const pools = bodyPoolUsage(body, dynamic);
    const poolLine = (["resource", "industrial", "infrastructure"] as Pool[]).map((pool) => `<span class="deck-stat${pools[pool].used >= pools[pool].total ? " is-warn" : ""}"><small>${POOL_LABEL[pool]} slots</small><b>${pools[pool].used}/${pools[pool].total}</b></span>`).join("");
    const buildActions = `<div class="deck-world-actions"><button type="button" class="is-primary" data-deck-act="world-build" data-mode="structures">Build structure</button>${(body.structures.shipyard ?? 0) > 0 ? `<button type="button" data-deck-act="world-build" data-mode="ships">Build ship</button>` : ""}</div>`;
    return `<section class="deck-page deck-world-page"><header class="deck-page__lead"><span>Owned world · served management</span><h2>${esc(body.name)}</h2><p>${esc(system.name)} · ${fmtPopulation(body.population)}</p></header>${publicSections}<section class="deck-section"><header><div><h3>Economy</h3><p>Structures, outputs and posted workforce on this world.</p></div></header><div class="deck-ledger">${structures || `<span><small>Development</small><b>none</b></span>`}</div>${economy}</section><section class="deck-section"><header><div><h3>Population</h3><p>Migration is physical; policy directs future Authority allocations.</p></div></header>${population}</section><section class="deck-section"><header><div><h3>Development</h3><p>New structures use a world slot; later tiers deepen in place.</p></div></header><div class="deck-stat-grid">${poolLine}</div>${buildActions}</section></section>`;
  }

  private worldEconomy(dynamic: SystemStateView, body: BodyView): string {
    const assignments = dynamic.assignments.filter((entry) => entry.body_id === body.id);
    const assigned = new Set(assignments.map((entry) => entry.structure));
    const producers = new Set(["mining_complex", "volatile_harvester", "bioharvester", "smelter", "electronics_fabricator", "chemical_works", "fuel_refinery", "machine_works", "armaments_complex", "agroplex", "academy", "shipyard", "naval_drydock", "capital_slipway", "ordnance_foundry"]);
    const rows = assignments.map((entry) => this.assignmentHtml(dynamic, entry));
    for (const [structure, tier] of Object.entries(body.structures)) {
      if (tier <= 0 || assigned.has(structure) || !producers.has(structure)) continue;
      rows.push(`<article class="deck-assignment is-warn"><div><b>${esc(label(structure))} · Tier ${tier}</b><span>${esc(body.name)} · needs workforce</span><small>Idle</small></div><div class="deck-stepper"><button type="button" disabled>−</button><b>0</b><button type="button" data-deck-act="set-workers" data-structure="${esc(structure)}" data-workers="1">+</button></div></article>`);
    }
    return `<div class="deck-assignment-list">${rows.join("") || `<div class="deck-empty-inline">No production structures on this world.</div>`}</div>`;
  }

  private worldPopulation(systemId: string, body: BodyView): string {
    const policyButtons = (["closed", "managed", "open", "priority"] as const).map((policy) => `<button type="button" data-deck-act="migration-policy" data-policy="${policy}" aria-pressed="${body.migration_policy === policy}">${esc(label(policy))}</button>`).join("");
    const cohort = this.ctx.state.galaxy?.migrant_cohort_people ?? 1_000;
    const options = this.ctx.state.systems
      .filter((entry) => entry.owner === this.ctx.state.playerId && !entry.blockade && entry.habitat_fed)
      .flatMap((entry) => entry.bodies.filter((candidate) => !(entry.id === systemId && candidate.id === body.id) && candidate.population > 0).map((candidate) => `<option value="${esc(entry.id)}|${candidate.id}">${esc(systemName(this.ctx, entry.id))} · ${esc(candidate.name)}</option>`)).join("");
    return `<div class="deck-stat-grid">${stat("Population", fmtPopulation(body.population))}${stat("Inbound", `${fmt(body.inbound_migrants)} people`)}${stat("Food use", `×${body.provisions_mult.toFixed(2)}`)}${stat("Settlement appeal", `×${body.population_growth_mult.toFixed(2)}`)}</div><div class="deck-policy"><span>Immigration policy</span>${policyButtons}</div><div class="deck-inline-form"><select id="deck-relocation-target">${options}</select><button type="button" data-deck-act="relocate-migrants" ${options && body.population * 1_000_000 >= cohort * 2 ? "" : "disabled"}>Relocate ${cohort.toLocaleString()}</button></div>`;
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
      const rows = options.filter((entry) => POOL_OF[entry.key] === pool).map((entry) => {
        const state = structOption(entry, dynamic, body, pools);
        return `<button type="button" class="deck-builder-row${entry.key === this.selectedBuild ? " is-selected" : ""}${state.buildable ? "" : " is-disabled"}" data-deck-act="builder-select" data-key="${esc(entry.key)}"><span>${icon(structureIcon(entry.key), "sm")}<span><b>${esc(entry.label)}</b><small>${state.tierUp ? `Tier ${state.currentTier} → ${state.targetTier}` : "New Tier I"}</small></span></span><em>${state.buildable ? fmtBuildDur(entry.build_secs * body.construction_time_mult) : esc(shortReason(state.reason))}</em></button>`;
      }).join("");
      return rows ? `<h3 class="deck-builder-group">${POOL_LABEL[pool]}</h3>${rows}` : "";
    }).join("");
    const selected = options.find((entry) => entry.key === this.selectedBuild);
    const detail = selected ? this.structureDetail(dynamic, body, selected, pools) : emptyState("Choose a structure", "Inspect its recipe, slot and build time.");
    return `<div class="deck-pools">${poolBars}</div><div class="deck-builder"><div class="deck-builder__list">${groups}</div><div class="deck-builder__detail">${detail}</div></div>`;
  }

  private structureDetail(dynamic: SystemStateView, body: BodyView, option: BuildOpt, pools: ReturnType<typeof bodyPoolUsage>): string {
    const state = structOption(option, dynamic, body, pools);
    const supply = constructionStock(dynamic).available;
    const costs = option.costs.map((cost) => {
      const commodity = cost.commodity as Commodity;
      const have = supply.get(commodity) ?? 0;
      return `<div class="deck-cost${have < cost.units ? " is-short" : ""}"><span>${commodityGlyph(commodity)} ${esc(label(commodity))}</span><b>${cost.units} <small>have ${fmt(have)}</small></b></div>`;
    }).join("");
    return `<article class="deck-build-detail"><header>${icon(structureIcon(option.key), "md")}<span><small>${POOL_LABEL[state.pool]} · ${state.foundsNew ? "new structure" : `upgrade to tier ${state.targetTier}`}</small><h3>${esc(option.label)}</h3></span></header><div class="deck-costs">${costs}</div><dl><div><dt>Build time</dt><dd>${fmtBuildDur(option.build_secs * body.construction_time_mult)}</dd></div><div><dt>Slot</dt><dd>${state.foundsNew ? `${POOL_LABEL[state.pool]} ${pools[state.pool].used} → ${pools[state.pool].used + 1}/${pools[state.pool].total}` : "Deepens in place"}</dd></div></dl>${state.reason ? `<div class="deck-build-warning">${esc(state.reason)}</div>` : ""}<button type="button" class="is-primary" data-deck-act="builder-queue" ${state.buildable ? "" : "disabled"}>Queue build</button></article>`;
  }

  private shipBuilder(dynamic: SystemStateView, body: BodyView): string {
    const options = SHIP_ORDER.map((kind) => buildOption(kind)).filter((entry): entry is BuildOpt => !!entry);
    if (!this.selectedHull || !options.some((entry) => entry.key === this.selectedHull)) this.selectedHull = options[0]?.key as ShipKind ?? "";
    const rows = options.map((entry) => {
      const state = shipOption(entry, dynamic);
      const stats = SHIP_STATS[entry.key];
      return `<button type="button" class="deck-builder-row${entry.key === this.selectedHull ? " is-selected" : ""}${state.buildable ? "" : " is-disabled"}" data-deck-act="builder-select-hull" data-hull="${entry.key}"><span>${icon(shipIcon(entry.key), "sm")}<span><b>${esc(buildName(entry.key))}</b><small>${esc(stats?.role ?? "Fleet hull")}</small></span></span><em>${state.buildable ? `max ${state.maxAff}` : esc(shortReason(state.reason))}</em></button>`;
    }).join("");
    const selected = this.selectedHull ? options.find((entry) => entry.key === this.selectedHull) : undefined;
    const detail = selected ? this.shipDetail(dynamic, body, selected) : emptyState("Choose a hull", "Inspect its recipe, capability and fitting budget.");
    return `<div class="deck-builder"><div class="deck-builder__list"><h3 class="deck-builder-group">Hull catalogue</h3>${rows}</div><div class="deck-builder__detail">${detail}${this.moduleForge(dynamic)}</div></div>`;
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
    const canQueue = state.buildable && quantity <= state.maxAff && fitOkay;
    const gate = SHIP_YARD[hull] ?? { yard: "shipyard", tier: 1 };
    const siteTime = body.ship_build_time_mult ?? 1;
    return `<article class="deck-build-detail"><header>${icon(shipIcon(hull), "md")}<span><small>${esc(YARD_TITLE[gate.yard] ?? label(gate.yard))} · Tier ${gate.tier}</small><h3>${esc(buildName(hull))}</h3></span></header><p>${esc(stats?.role ?? "Fleet hull")}</p><div class="deck-quantity"><span>Quantity</span>${quantities}</div><div class="deck-costs">${costs}</div><dl><div><dt>Build time</dt><dd>${fmtBuildDur(option.build_secs * siteTime)}</dd></div><div><dt>Site</dt><dd>${esc(body.name)} · ×${siteTime.toFixed(2)}</dd></div></dl>${stats ? `<div class="deck-hull-stats">${stat("Speed", fmt(stats.speed))}${stat("Hull", fmt(stats.hull))}${stat("Attack", fmt(stats.atk))}${stat("Defense", fmt(stats.def))}</div>` : ""}${fit}${state.reason ? `<div class="deck-build-warning">${esc(state.reason)}</div>` : ""}${!fitOkay ? `<div class="deck-build-warning">The composed fit exceeds this hull's fitting budget or module slots.</div>` : ""}<button type="button" class="is-primary" data-deck-act="builder-queue-ships" ${canQueue ? "" : "disabled"}>Queue ${quantity} hull${quantity === 1 ? "" : "s"}</button></article>`;
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
    this.noteBuildDispatch(systemId, `${buildName(key)} sent to local administration.`);
  }

  private noteBuildDispatch(systemId: string, text: string): void {
    this.buildFeedback = {
      systemId,
      issuedAt: this.ctx.state.simTime,
      timelineLength: this.ctx.state.timeline.length,
      text,
      tone: "info",
    };
    this.hooks.notice(`<b>Build order sent</b> · ${esc(text)}`);
  }

  private opportunityHtml(dynamic?: SystemStateView): string {
    const opportunities = dynamic?.opportunities ?? [];
    if (!opportunities.length) return "";
    const cards = opportunities.slice(0, 3).map((entry) => `<article class="deck-opportunity is-${entry.tier}"><small>${esc(label(entry.tier))}</small><b>${esc(entry.title)} · ×${entry.score.toFixed(2)}</b><span>${entry.body_name ? `${esc(entry.body_name)} · ` : ""}${esc(entry.reason)}</span></article>`).join("");
    return `<section class="deck-section"><header><div><h3>Surveyed opportunities</h3><p>Specialized strengths, not universal multipliers.</p></div></header><div class="deck-opportunities">${cards}</div></section>`;
  }
}

function isSystemTab(value?: string): value is DeckSystemTab {
  return value === "overview" || value === "worlds" || value === "production";
}

function isMigrationPolicy(value?: string): value is BodyView["migration_policy"] {
  return value === "closed" || value === "managed" || value === "open" || value === "priority";
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

function commodityGlyph(commodity: Commodity): string {
  const keys: Partial<Record<Commodity, IconKey>> = {
    metallic_ore: "ore", alloys: "alloys", fuel: "fuel", provisions: "provisions",
    volatiles: "volatiles", biomass: "biomass",
  };
  return icon(keys[commodity] ?? "cargo", "sm", label(commodity));
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

function structureIcon(key: string): IconKey {
  const icons: Record<string, IconKey> = {
    shipyard: "shipyard", sensor_array: "sensor", defense_platform: "defense",
    habitat: "habitat", fuel_refinery: "refinery", orbital_warehouse: "orbital_warehouse",
    mining_complex: "extractor", volatile_harvester: "extractor", bioharvester: "extractor",
    academy: "intel", armaments_complex: "moduleMassDriver",
  };
  return icons[key] ?? "build";
}

function shipIcon(key: string): IconKey {
  const icons: Record<string, IconKey> = {
    scout: "scout", corvette: "corvette", raider: "raider", convoy: "convoy", colony: "colony",
  };
  return icons[key] ?? "fleet";
}

function shortReason(reason: string): string {
  if (!reason) return "Unavailable";
  const cut = reason.indexOf(" — ");
  return cut > 0 ? reason.slice(0, cut) : reason;
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

function esc(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!);
}

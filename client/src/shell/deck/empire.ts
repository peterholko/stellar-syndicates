import { constructionStock } from "../../core/derive/fleet";
import { fmtBuildDur } from "../../core/derive/format";
import { systemFlavor } from "../../core/derive/market";
import { icon, label, type IconKey } from "../../icons";
import type { AssignmentView, BodyView, Commodity, SystemInfo, SystemStateView } from "../../protocol";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

export type DeckSystemTab = "overview" | "worlds" | "production";

interface EmpireHooks {
  go(route: DeckRoute): void;
  notice(html: string): void;
}

/** Routed empire management. This reads only the player's served SystemStateView:
 * public astronomy is always visible, survey findings require arrived survey
 * light, and owner-only production never receives a rival rendering path. */
export class DeckEmpireRoutes {
  private systemTab: DeckSystemTab = "overview";
  private signature = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: EmpireHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "system") return false;
    const context = this.systemContext(route);
    const signature = sheetFingerprint([
      route, this.systemTab, Math.floor(liveSimTime()), context.system, context.dynamic,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, this.systemHtml(context.system, context.dynamic));
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "system") return false;
    const action = button.dataset.deckAct;
    const { system, dynamic } = this.systemContext(route);
    if (!system) return false;
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
        this.opportunityHtml(dynamic);
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
    return `<div class="deck-stat-grid">${ownerStats.join("")}</div>${this.opportunityHtml(dynamic)}${this.queueHtml(dynamic)}`;
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
    return `<section class="deck-section"><header><div><h3>Construction queue</h3><p>Completion times follow the live simulation clock.</p></div><b>${dynamic.builds.length}</b></header>${rows || `<div class="deck-empty-inline">Nothing under construction.</div>`}</section>`;
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

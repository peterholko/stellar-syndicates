import { captainTitle, captainXpFloor, fleetCommandLoad, officerFleetName } from "../../core/derive/captains";
import { fleetRosterDockName, shipKindLabel } from "../../core/derive/fleet";
import { fmt, fmtEta } from "../../core/derive/format";
import { foundingHomeSystemId, systemName } from "../../core/derive/geo";
import { researchQueueIds, sendResearchQueue } from "../../core/derive/research";
import { icon, label } from "../../icons";
import type {
  AcademyRow,
  CaptainAttribute,
  CaptainRosterView,
  GhostView,
  ProgrammeView,
} from "../../protocol";
import { liveSimTime } from "../../state";
import { captainPortrait } from "../art";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import { fleetListRow } from "./fleet-row";
import type { DeckRoute } from "./router";

interface RosterHooks {
  go(route: DeckRoute): void;
  openWorld(systemId: string, bodyId: number): void;
  notice(html: string): void;
}

const FIELD_ORDER = ["propulsion", "materials", "computation", "weapons", "hulls", "life"];
const FIELD_TITLE: Record<string, string> = {
  propulsion: "Propulsion", materials: "Materials", computation: "Computation",
  weapons: "Weapons", hulls: "Hulls", life: "Life",
};
const SCHOOL_TITLE: Record<string, string> = {
  line_haul: "Line Haul", expedition: "Expedition", deep_crust: "Deep Crust", foundry: "Foundry",
  watch: "Watch", shadow: "Shadow", strike: "Strike", countermeasures: "Countermeasures",
  line: "Line", corsair: "Corsair", growth: "Growth", talent: "Talent",
};
const ROMAN = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII"];
type ResearchPage = "catalog" | "queue" | "completed";
interface ResearchSelection { field: string; tier: number; id: string }

/** Corporation-wide fleet, officer and research routes. These surfaces share
 * the same served owner picture; no roster row or personnel status is corrected
 * from true positions between reports. */
export class DeckRosterRoutes {
  private signature = "";
  private researchPreset = "";
  private researchPage: ResearchPage = "catalog";
  private researchSelections: Record<"catalog" | "completed", ResearchSelection> = {
    catalog: { field: "", tier: 0, id: "" },
    completed: { field: "", tier: 0, id: "" },
  };

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: RosterHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    const preset = route?.name === "research" ? route.query?.programme ?? "" : "";
    if (preset && preset !== this.researchPreset) {
      const p = this.ctx.state.research?.programmes.find(p => p.id === preset);
      if (p) {
        this.researchPage = p.state === "completed" ? "completed" : "catalog";
        Object.assign(this.researchSelection(), { field: p.field, tier: p.tier, id: p.id });
        this.researchPreset = preset;
      }
    } else if (!preset) this.researchPreset = "";
    if (route?.name !== "fleets" && route?.name !== "officers" && route?.name !== "research") return false;
    const active = document.activeElement;
    if (!force && active instanceof HTMLElement && this.root.contains(active) && active.matches("select, input")) return true;
    const signature = sheetFingerprint([
      route, Math.floor(liveSimTime()), this.ctx.state.ghosts, this.ctx.state.selectedShipIds,
      this.ctx.state.battles, this.ctx.state.commandSignals, this.ctx.state.orders,
      this.ctx.state.raids, this.ctx.state.captains, this.ctx.state.captainCapacity,
      this.ctx.state.research, this.ctx.state.systems, this.ctx.state.syndicate?.flagship_name,
      this.researchPage, this.researchSelections,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    setHtml(this.root, route.name === "fleets" ? this.fleetsHtml() : route.name === "officers" ? this.officersHtml() : this.researchHtml());
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "fleets" && route?.name !== "officers" && route?.name !== "research") return false;
    const action = button.dataset.deckAct;
    if (!action?.startsWith("roster-") && !action?.startsWith("officer-") && !action?.startsWith("research-")) return false;
    if (action.startsWith("research-") && route.name !== "research") return false;
    if (action === "roster-open" || action === "roster-center") {
      const id = button.dataset.fleet;
      const fleet = this.ctx.state.ghosts.find((entry) => entry.id === id && entry.own);
      if (fleet) {
        this.ctx.state.selectedShipId = fleet.id;
        this.ctx.state.selectedEmplacementId = null;
        if (this.ctx.state.selectedShipIds.size <= 1) this.ctx.state.selectedShipIds = new Set([fleet.id]);
        this.ctx.renderer.centerOnWorld(fleet.pos);
        this.ctx.renderer.stateVersion++;
        if (action === "roster-open") this.hooks.go({ name: "fleet", params: { id: fleet.id, fleetLabel: `${shipKindLabel(fleet.kind)} fleet` } });
      }
    } else if (action === "roster-group") {
      const id = button.dataset.fleet;
      const fleet = this.ctx.state.ghosts.find((entry) => entry.id === id && entry.own);
      if (fleet) this.toggleGroup(fleet);
    } else if (action === "officer-recruit") {
      const home = foundingHomeSystemId();
      if (home) {
        this.ctx.send({ type: "RecruitCaptain", system_id: home });
        this.hooks.notice("<b>Officer commission ordered</b> · physical training begins at the home Academy.");
      }
    } else if (action === "officer-assign") {
      const captain = Number(button.dataset.captain);
      const select = this.root.querySelector<HTMLSelectElement>(`[data-officer-fleet="${captain}"]`);
      if (Number.isFinite(captain) && select?.value) {
        this.ctx.intent.beginFleetCommand({ type: "AssignCaptain", captain_id: captain, fleet_id: select.value });
      }
    } else if (action === "officer-reserve") {
      const captain = Number(button.dataset.captain);
      if (Number.isFinite(captain)) {
        this.ctx.intent.beginFleetCommand({ type: "ReserveCaptain", captain_id: captain });
      }
    } else if (action === "officer-train") {
      const captain = Number(button.dataset.captain);
      const attribute = button.dataset.attribute as CaptainAttribute;
      if (Number.isFinite(captain) && attribute) {
        this.ctx.intent.beginFleetCommand({ type: "TrainCaptain", captain_id: captain, attribute });
      }
    } else if (action === "officer-fleet") {
      const fleet = this.ctx.state.ghosts.find((entry) => entry.id === button.dataset.fleet && entry.own);
      if (fleet) {
        this.ctx.renderer.centerOnWorld(fleet.pos);
        this.hooks.go({ name: "fleet", params: { id: fleet.id, fleetLabel: `${shipKindLabel(fleet.kind)} fleet` } });
      }
    } else if (action === "officer-academy") {
      const home = foundingHomeSystemId();
      const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === home);
      if (home && system) this.hooks.go({ name: "system", params: { id: home, systemLabel: system.name } });
    } else if (action === "research-page") {
      const page = button.dataset.page;
      if (page === "catalog" || page === "queue" || page === "completed") this.researchPage = page;
    } else if (action === "research-field") {
      const field = button.dataset.field;
      if (field && FIELD_ORDER.includes(field)) {
        Object.assign(this.researchSelection(), { field, tier: 0, id: "" });
      }
    } else if (action === "research-tier") {
      const tier = Number(button.dataset.tier);
      const selection = this.researchSelection();
      if (this.researchPool().some((entry) => entry.field === selection.field && entry.tier === tier)) {
        selection.tier = tier;
        selection.id = "";
      }
    } else if (action === "research-select" || action === "research-inspect") {
      const programme = this.ctx.state.research?.programmes.find((entry) => entry.id === button.dataset.programme);
      if (programme) {
        if (action === "research-inspect") this.researchPage = "catalog";
        Object.assign(this.researchSelection(), { field: programme.field, tier: programme.tier, id: programme.id });
      }
    } else if (action === "research-add") {
      // Selection only inspects. This one explicit action sends work; the
      // server's arrived availability (not a client gate calculation) permits it.
      const programme = this.ctx.state.research?.programmes.find((entry) => entry.id === this.researchSelections.catalog.id);
      const queue = deckResearchQueue();
      if (this.researchPage === "catalog" && programme?.state === "available" && !queue.includes(programme.id)) {
        this.sendResearchQueue([...queue, programme.id]);
        this.hooks.notice("<b>Programme queued</b> · awaiting the next served research report.");
      }
    } else if (action === "research-up" || action === "research-down" || action === "research-remove") {
      const queue = deckResearchQueue();
      const index = Number(button.dataset.index);
      const firstMovable = this.ctx.state.research?.active ? 1 : 0;
      if (Number.isInteger(index) && index >= firstMovable && index < queue.length) {
        if (action === "research-remove") queue.splice(index, 1);
        else if (action === "research-up" && index > firstMovable) [queue[index - 1], queue[index]] = [queue[index], queue[index - 1]];
        else if (action === "research-down" && index < queue.length - 1) [queue[index + 1], queue[index]] = [queue[index], queue[index + 1]];
        this.sendResearchQueue(queue);
        this.hooks.notice("<b>Research queue updated</b> · awaiting the next served report.");
      }
    } else if (action === "research-academy") {
      const home = foundingHomeSystemId();
      const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === home);
      const body = home ? this.ctx.state.systems.find((entry) => entry.id === home)?.bodies.find((entry) => (entry.structures.academy ?? 0) > 0) : undefined;
      if (home && system) {
        if (body) this.hooks.openWorld(home, body.id);
        else this.hooks.go({ name: "build", params: { systemId: home, systemLabel: system.name }, query: { mode: "structures", select: "academy" } });
      }
    }
    this.signature = "";
    this.render(route, true);
    return true;
  }

  invalidate(): void { this.signature = ""; }

  private fleetsHtml(): string {
    const fleets = this.ctx.state.ghosts.filter((entry) => entry.own).sort((a, b) => shipKindLabel(a.kind).localeCompare(shipKindLabel(b.kind)) || a.id.localeCompare(b.id));
    const undocked = fleets.filter((entry) => !entry.docked);
    const docked = fleets.filter((entry) => !!entry.docked);
    const group = (name: string, entries: GhostView[]) => entries.length ? `<section class="deck-section"><header><div><h3>${esc(name)}</h3></div><b>${entries.length}</b></header><div class="deck-fleet-list">${entries.map((entry) => this.fleetRow(entry)).join("")}</div></section>` : "";
    const grouped = this.ctx.state.selectedShipIds.size > 1;
    return `<section class="deck-page deck-fleets"><header class="deck-page__lead"><h2>Fleets</h2>${grouped ? `<p>${this.ctx.state.selectedShipIds.size} fleets grouped · choose a map destination.</p>` : ""}</header>${fleets.length ? group("Undocked", undocked) + group("Docked", docked) : empty("No fleet reports", "Build a ship or wait for a fleet report to arrive.")}</section>`;
  }

  private fleetRow(g: GhostView): string {
    const dock = fleetRosterDockName(g);
    const battle = this.ctx.state.battles.some((entry) => entry.participants.includes(g.id));
    const guard = g.guard_target ? this.ctx.state.ghosts.find((entry) => entry.id === g.guard_target && entry.own) : undefined;
    return fleetListRow(g, {
      openAction: "roster-open",
      status: battle ? "In battle" : g.defend_system ? dock ? "Defending · docked" : "Defending" : dock ? "Docked" : guard ? "Guarding" : this.rosterActivity(g),
      location: dock ?? (guard ? `${shipKindLabel(guard.kind)} fleet` : undefined),
      flagshipName: this.ctx.state.syndicate?.flagship_name,
      controls: true, grouped: this.ctx.state.selectedShipIds.has(g.id),
    });
  }

  private officersHtml(): string {
    const home = foundingHomeSystemId();
    const system = home ? this.ctx.state.systems.find((entry) => entry.id === home) : undefined;
    const academyTier = system?.structures.academy ?? 0;
    const pending = system?.builds.filter((entry) => entry.key === "officer_commission").length ?? 0;
    const living = this.ctx.state.captains.filter((entry) => entry.loss_fate !== "killed").length;
    const memorial = this.ctx.state.captains.length - living;
    const used = living + pending;
    const canRecruit = !!home && academyTier > 0 && used < this.ctx.state.captainCapacity;
    const recruitReason = !home ? "Home-system report unavailable." : academyTier === 0 ? "Build an Academy at home first." : used >= this.ctx.state.captainCapacity ? "Officer berths are full." : "";
    return `<section class="deck-page deck-officers"><header class="deck-page__lead"><span>Personnel · physical command</span><h2>Officer Corps</h2><p>Junior officers cover several light formations; senior ranks concentrate authority over capital fleets.</p></header><div class="deck-stat-grid"><dl class="deck-stat"><dt>Active</dt><dd>${living}</dd></dl><dl class="deck-stat"><dt>Commissioning</dt><dd>${pending}</dd></dl><dl class="deck-stat"><dt>Berths</dt><dd>${used} / ${this.ctx.state.captainCapacity}</dd></dl><dl class="deck-stat"><dt>Memorial</dt><dd>${memorial}</dd></dl></div><section class="deck-section"><header><div><h3>Academy commission</h3><p>60s · 40 Provisions · 20 Electronics · 10 Machinery.</p></div><button type="button" data-deck-act="officer-academy">Open Academy</button></header><button type="button" class="is-primary" data-deck-act="officer-recruit" ${canRecruit ? "" : "disabled"}>Commission Lieutenant</button>${recruitReason ? `<small class="deck-disabled-reason">${esc(recruitReason)}</small>` : ""}</section><div class="deck-officer-grid">${this.ctx.state.captains.length ? this.ctx.state.captains.map((entry) => this.officerCard(entry, home)).join("") : empty("No officer reports", "Build and staff an Academy to commission an officer.")}</div><p class="deck-muted">Career XP: combat 60–85 · survey 35 · delivery 20 · jump 12. Loss reports can reveal rescue, injury, capture or death.</p></section>`;
  }

  private officerCard(entry: CaptainRosterView, home: string | null): string {
    const report = entry.report;
    const assigned = entry.assigned_fleet ? this.ctx.state.ghosts.find((fleet) => fleet.id === entry.assigned_fleet && fleet.own) : undefined;
    const recovering = entry.recovering_until !== null && entry.recovering_until > this.ctx.state.simTime;
    const killed = entry.loss_fate === "killed";
    const station = entry.assigned_fleet === null ? entry.stationed_system : assigned && this.ctx.state.systems.find((system) => system.owner === this.ctx.state.playerId && dockedAt(system.id, assigned))?.id;
    const local = !recovering && !killed && !!station && (entry.assigned_fleet === null || (!!assigned && Math.hypot(assigned.vel.x, assigned.vel.y) < .5));
    const canTrain = local && station === home;
    const occupied = new Set(this.ctx.state.captains.flatMap((captain) => captain.assigned_fleet ? [captain.assigned_fleet] : []));
    const eligible = report && station ? this.ctx.state.ghosts.filter((fleet) => fleet.own && dockedAt(station, fleet) && Math.hypot(fleet.vel.x, fleet.vel.y) < .5 && (!occupied.has(fleet.id) || fleet.id === entry.assigned_fleet) && fleetCommandLoad(fleet) <= report.command_capacity && fleet.id !== entry.assigned_fleet) : [];
    const status = killed ? "Killed in action" : recovering ? `${label(entry.loss_fate ?? "recovery")} · ${fmtEta(entry.recovering_until! - this.ctx.state.simTime)}` : entry.assigned_fleet ? assigned ? `Assigned · ${officerFleetName(assigned)}` : "Assigned · report in transit" : entry.stationed_system ? `Reserve · ${systemName(entry.stationed_system)}` : "Reserve";
    const titled = report ? `${captainTitle(report.title)} ${entry.name}` : entry.name;
    const floor = report ? captainXpFloor(report.level) : 0;
    const span = report ? Math.max(1, report.next_level_xp - floor) : 1;
    const progress = report ? report.level >= 10 ? 100 : Math.max(0, Math.min(100, (report.xp - floor) / span * 100)) : 0;
    const options = eligible.map((fleet) => `<option value="${escAttr(fleet.id)}">${esc(officerFleetName(fleet))} · load ${fleetCommandLoad(fleet)}/${report!.command_capacity}</option>`).join("");
    const assignment = report && local && options ? `<div class="deck-inline-form"><select data-officer-fleet="${entry.id}">${options}</select><button type="button" data-deck-act="officer-assign" data-captain="${entry.id}">${entry.assigned_fleet ? "Transfer" : "Assign"}</button></div>` : "";
    const train = report && report.unspent > 0 ? `<div class="deck-officer-train">${(["command", "navigation", "fieldcraft", "logistics"] as CaptainAttribute[]).map((attribute) => `<button type="button" data-deck-act="officer-train" data-captain="${entry.id}" data-attribute="${attribute}" ${canTrain ? "" : "disabled"}>+ ${label(attribute)}</button>`).join("")}</div>` : "";
    const actions = `${entry.assigned_fleet && local ? `<button type="button" data-deck-act="officer-reserve" data-captain="${entry.id}">Return to reserve</button>` : ""}${assigned ? `<button type="button" data-deck-act="officer-fleet" data-fleet="${escAttr(assigned.id)}">Open formation</button>` : ""}`;
    return `<article class="deck-officer-card${killed ? " is-lost" : recovering ? " is-recovering" : ""}">${captainPortrait(entry.portrait, report?.portrait_age ?? "young", `Portrait of ${titled}`, "deck-officer-card__portrait", true)}<div><header><div><span>${esc(status)}</span><h3>${esc(titled)}</h3></div>${report ? `<b>Lv ${report.level}</b>` : ""}</header><p>${report ? `Command ${report.attributes.command} · Navigation ${report.attributes.navigation} · Fieldcraft ${report.attributes.fieldcraft} · Logistics ${report.attributes.logistics}` : "Personnel light has not reached command."}</p><div class="deck-meter"><i style="width:${progress.toFixed(1)}%"></i></div><small>${report ? report.level >= 10 ? "Maximum level" : `${report.xp.toLocaleString()} / ${report.next_level_xp.toLocaleString()} XP · authority ${report.command_capacity}` : "Attributes pending"}</small>${train}${assignment}<div class="deck-officer-actions">${actions}</div></div></article>`;
  }

  private researchHtml(): string {
    const research = this.ctx.state.research;
    if (!research) return empty("Research report pending", "Waiting for the corporation's research report.");
    const queue = deckResearchQueue();
    const completed = research.programmes.filter((entry) => entry.state === "completed").length;
    const tabs: [ResearchPage, string][] = [["catalog", "Research"], ["queue", `Queue · ${queue.length}`], ["completed", `Completed · ${completed}`]];
    const nav = `<nav class="deck-tabs" aria-label="Research pages">${tabs.map(([page, name]) => `<button type="button" data-deck-act="research-page" data-page="${page}" aria-current="${this.researchPage === page ? "page" : "false"}" aria-selected="${this.researchPage === page}">${name}</button>`).join("")}</nav>`;
    const active = research.active ? this.activeResearchHtml(research.active, research.stalled) : `<div class="deck-research-idle">No active research</div>`;
    return `<section class="deck-page deck-research">${nav}${active}${this.researchPage === "queue" ? this.researchQueueHtml() : this.researchBrowserHtml()}</section>`;
  }

  private researchQueueHtml(): string {
    const research = this.ctx.state.research!;
    const queue = deckResearchQueue();
    const activePinned = !!research.active;
    const queueHtml = queue.length ? queue.map((id, index) => {
      const programme = research.programmes.find((entry) => entry.id === id);
      const pinned = activePinned && index === 0;
      const firstQueued = activePinned ? index === 1 : index === 0;
      const name = programme?.name ?? id;
      return `<div class="deck-research-queue-row"><span><b>${index + 1}</b><span><button type="button" class="deck-research-queue-name" data-deck-act="research-inspect" data-programme="${escAttr(id)}">${esc(name)}</button><small>${pinned ? "In progress" : "Queued"}</small></span></span><div><button type="button" data-deck-act="research-up" data-index="${index}" aria-label="Move ${escAttr(name)} up" ${pinned || firstQueued ? "disabled" : ""}>↑</button><button type="button" data-deck-act="research-down" data-index="${index}" aria-label="Move ${escAttr(name)} down" ${pinned || index === queue.length - 1 ? "disabled" : ""}>↓</button><button type="button" data-deck-act="research-remove" data-index="${index}" aria-label="Remove ${escAttr(name)}" ${pinned ? "disabled" : ""}>Remove</button></div></div>`;
    }).join("") : `<div class="deck-empty-inline">Empty</div>`;
    return `<section class="deck-section"><header><h3>Research queue</h3><b>${queue.length}</b></header><div class="deck-research-queue">${queueHtml}</div></section>` + this.researchAcademiesHtml(research.academies, research.rate);
  }

  private activeResearchHtml(active: { id: string; name: string; progress: number; cost: number; eta_secs: number | null }, stalled: boolean): string {
    const pct = Math.max(0, Math.min(100, active.progress / Math.max(1e-9, active.cost) * 100));
    const eta = stalled ? "Paused" : active.eta_secs !== null ? fmtEta(active.eta_secs) : "Awaiting supply";
    return `<section class="deck-section deck-research-active"><header><button type="button" data-deck-act="research-inspect" data-programme="${escAttr(active.id)}"><small>In progress</small><strong>${esc(active.name)}</strong></button><b>${pct.toFixed(0)}% · ${esc(eta)}</b></header><div class="deck-meter" role="progressbar" aria-label="${escAttr(active.name)} progress" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${pct.toFixed(1)}"><i style="width:${pct.toFixed(1)}%"></i></div></section>`;
  }

  private researchAcademiesHtml(academies: AcademyRow[], rate: number): string {
    const rows = academies.map((row) => `<div class="deck-academy-row${row.supplied ? "" : " is-warn"}"><span><b>${esc(row.system)}</b><small>${row.supplied ? "Supplied" : "Unsupplied"}</small></span><em>Tier ${ROMAN[row.tier] ?? row.tier} · ${row.rate.toFixed(2)}/s</em></div>`).join("");
    return `<section class="deck-section"><header><h3>Academies · ${rate.toFixed(2)}/s</h3><button type="button" data-deck-act="research-academy">Open Academy</button></header><div class="deck-academies">${rows || `<div class="deck-empty-inline">No staffed Academy</div>`}</div></section>`;
  }

  private researchSelection(): ResearchSelection {
    return this.researchSelections[this.researchPage === "completed" ? "completed" : "catalog"];
  }

  private researchPool(): ProgrammeView[] {
    const programmes = this.ctx.state.research?.programmes ?? [];
    return this.researchPage === "completed" ? programmes.filter((entry) => entry.state === "completed") : programmes;
  }

  private researchBrowserHtml(): string {
    const programmes = this.researchPool();
    const selection = this.researchSelection();
    const preferred = (entries: ProgrammeView[]) => entries.find((entry) => entry.state === "active")
      ?? entries.find((entry) => entry.state === "available") ?? entries.find((entry) => entry.state === "queued") ?? entries[0];
    // Browser state survives incoming Views. Only repair missing selections;
    // never jump to a new tier just because its requirements became satisfied.
    if (!selection.field) selection.field = preferred(programmes)?.field ?? FIELD_ORDER[0];
    const field = programmes.filter((entry) => entry.field === selection.field);
    const tiers = [...new Set(field.map((entry) => entry.tier))].sort((a, b) => a - b);
    if (!tiers.includes(selection.tier)) selection.tier = preferred(field)?.tier ?? 1;
    const rows = field.filter((entry) => entry.tier === selection.tier);
    if (!rows.some((entry) => entry.id === selection.id)) selection.id = preferred(rows)?.id ?? "";
    const selected = rows.find((entry) => entry.id === selection.id);
    const queue = deckResearchQueue();
    const fields = `<nav class="deck-research-fields" aria-label="Research categories">${FIELD_ORDER.map((key) => {
      const count = programmes.filter((entry) => entry.field === key && (this.researchPage === "completed" || entry.state === "available")).length;
      return `<button type="button" data-deck-act="research-field" data-field="${key}" aria-label="${FIELD_TITLE[key]}, ${count} ${this.researchPage === "completed" ? "completed" : "available"} technologies" aria-pressed="${selection.field === key}">${icon(fieldIcon(key), "sm")}<span>${FIELD_TITLE[key]}</span><small title="${this.researchPage === "completed" ? "Completed technologies" : "Available technologies"}">${count}</small></button>`;
    }).join("")}</nav>`;
    const tierNav = tiers.length ? `<nav class="deck-research-tiers" aria-label="Research tiers"><span>Tier</span>${tiers.map((tier) => `<button type="button" data-deck-act="research-tier" data-tier="${tier}" aria-label="Tier ${ROMAN[tier] ?? tier}" aria-pressed="${selection.tier === tier}">${ROMAN[tier] ?? tier}</button>`).join("")}</nav>` : "";
    const schools = [...new Set(rows.map((entry) => entry.school))];
    const list = schools.map((school) => `${school ? `<h4>${esc(SCHOOL_TITLE[school] ?? label(school))}</h4>` : ""}${rows.filter((entry) => entry.school === school).map((entry) =>
      `<button type="button" class="deck-research-choice is-${escAttr(entry.state)}" data-deck-act="research-select" data-programme="${escAttr(entry.id)}" aria-pressed="${entry.id === selection.id}"><strong>${esc(entry.name)}</strong><small>${esc(researchStatus(entry, queue))}</small></button>`).join("")}`).join("");
    const canAdd = selected?.state === "available" && !queue.includes(selected.id);
    const footer = this.researchPage === "catalog" ? `<footer class="deck-research-action"><span>${selected ? esc(selected.name) : "Select a technology"}</span><button type="button" class="is-primary" data-deck-act="research-add" ${canAdd ? "" : "disabled"}>Add to queue</button></footer>` : "";
    return `${fields}${tierNav}${rows.length ? `<div class="deck-research-browser"><section class="deck-research-choices" aria-label="Technologies in ${escAttr(FIELD_TITLE[selection.field] ?? selection.field)} Tier ${ROMAN[selection.tier] ?? selection.tier}">${list}</section>${selected ? this.researchDetailHtml(selected, queue) : ""}</div>` : empty(this.researchPage === "completed" ? "No completed technologies in this category" : "No technology reports", "")}${footer}`;
  }

  private researchDetailHtml(programme: ProgrammeView, queue: string[]): string {
    const research = this.ctx.state.research!;
    const gate = programme.gate;
    const gateMet = !gate || gate.current >= gate.threshold;
    const hasPredecessor = programme.tier === 1 || research.programmes.some((entry) => entry.state === "completed"
      && entry.field === programme.field && entry.tier + 1 === programme.tier
      && (programme.tier <= 3 ? entry.school === null : entry.school === programme.school));
    const missingPredecessor = programme.state === "locked" && !hasPredecessor;
    const predecessor = `${programme.tier <= 3 ? FIELD_TITLE[programme.field] : SCHOOL_TITLE[programme.school ?? ""]} Tier ${ROMAN[programme.tier - 1] ?? programme.tier - 1}`;
    const gateHtml = gate ? `<div class="deck-research-gate${gateMet ? " is-met" : ""}"><span>${esc(gate.label)} <b>${researchCount(gate.current)} / ${researchCount(gate.threshold)}</b>${gateMet ? " · Met" : ""}</span><div class="deck-meter"><i style="width:${Math.max(0, Math.min(100, gate.current / Math.max(1e-9, gate.threshold) * 100)).toFixed(1)}%"></i></div></div>` : "";
    const requirements = programme.state === "locked" ? `<section class="deck-research-requirements"><h4>To unlock</h4>${missingPredecessor ? `<p>Complete one ${esc(predecessor)} technology.</p>` : ""}${gateHtml}${!missingPredecessor && gateMet ? `<p>Awaiting unlock confirmation.</p>` : ""}</section>` : "";
    const dossier = programme.recovered_data ? `<div class="deck-research-dossier">Recovered data · ${Math.round(programme.recovered_data / Math.max(1, programme.cost) * 100)}% research work banked</div>` : "";
    const needsAcademy = programme.state !== "completed" && !research.academies.some((academy) => academy.supplied);
    return `<article class="deck-section deck-research-detail" aria-label="Selected technology"><header>${icon(fieldIcon(programme.field), "md")}<div><small>${esc(FIELD_TITLE[programme.field])} · Tier ${ROMAN[programme.tier] ?? programme.tier}${programme.school ? ` · ${esc(SCHOOL_TITLE[programme.school] ?? label(programme.school))}` : ""}</small><h3>${esc(programme.name)}</h3></div></header><span class="deck-research-state is-${escAttr(programme.state)}">${esc(researchStatus(programme, queue))}</span><p class="deck-research-benefit">${esc(programme.blurb)}</p><dl class="deck-research-cost"><dt title="Total research-seconds required; completion time depends on staffed, supplied Academies.">Research work</dt><dd>${fmt(programme.cost)}</dd></dl>${dossier}${requirements}${needsAcademy ? `<div class="deck-research-supply"><span>Staff and supply an Academy to progress.</span><button type="button" data-deck-act="research-academy">Open Academy</button></div>` : ""}</article>`;
  }

  private toggleGroup(fleet: GhostView): void {
    const selected = new Set(this.ctx.state.selectedShipIds);
    if (selected.has(fleet.id)) selected.delete(fleet.id);
    else selected.add(fleet.id);
    if (!selected.size) {
      selected.add(fleet.id);
      this.ctx.state.selectedShipId = fleet.id;
    } else if (!this.ctx.state.selectedShipId || !selected.has(this.ctx.state.selectedShipId)) {
      this.ctx.state.selectedShipId = [...selected][0];
    }
    this.ctx.state.selectedShipIds = selected;
    this.ctx.renderer.stateVersion++;
    this.hooks.notice(selected.size > 1 ? `<b>${selected.size} fleets grouped</b> · choose one map destination for a batch move.` : "<b>Single-fleet command</b> · group another fleet to move together.");
  }

  /** The server's SetResearchQueue payload is queue-ahead only while a
   * programme is active. Keep the pinned row in the Deck readout, but do not
   * echo it back onto the wire when the player edits the rows beneath it. */
  private sendResearchQueue(rows: string[]): void {
    const active = this.ctx.state.research?.active?.id;
    sendResearchQueue(rows.filter((id) => id !== active));
  }

  private rosterActivity(g: GhostView): string {
    if (this.ctx.state.commandSignals.some((entry) => entry.shipId === g.id)) return "Signal outbound";
    if (g.rescue_inbound) return "AAA rescue active";
    if (g.stalled) return "Out of fuel · holding";
    if (this.ctx.state.raids[g.id]) return "Raiding";
    if (this.ctx.state.orders[g.id]) return "En route";
    return Math.hypot(g.vel.x, g.vel.y) < .5 ? "Holding station" : "Under way";
  }
}

function deckResearchQueue(): string[] {
  return [...new Set(researchQueueIds())];
}

function researchStatus(programme: ProgrammeView, queue: string[]): string {
  if (programme.state === "completed") return "Completed";
  if (programme.state === "active") return "In progress";
  if (programme.state === "queued" || queue.includes(programme.id)) return "Queued";
  if (programme.state === "available") return "Available";
  return "Locked";
}

function researchCount(value: number): string {
  return value.toLocaleString("en-US", { maximumFractionDigits: 2 });
}

function dockedAt(systemId: string, fleet: GhostView): boolean {
  return fleet.docked === systemId || fleet.docked === `E${systemId}`;
}

function fieldIcon(field: string): "jump" | "moduleWhippleArmor" | "intel" | "attack" | "fleet" | "population" {
  if (field === "propulsion") return "jump";
  if (field === "materials") return "moduleWhippleArmor";
  if (field === "computation") return "intel";
  if (field === "weapons") return "attack";
  if (field === "hulls") return "fleet";
  return "population";
}

function empty(title: string, copy: string): string {
  return `<div class="deck-empty"><b>${esc(title)}</b><span>${esc(copy)}</span></div>`;
}

const esc = (value: string): string => value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]!);
const escAttr = esc;

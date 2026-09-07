import { captainTitle, captainXpFloor, fleetCommandLoad, officerFleetName } from "../../core/derive/captains";
import { fleetRosterDockName, shipKindLabel } from "../../core/derive/fleet";
import { fmt, fmtEta, informationDelay } from "../../core/derive/format";
import { foundingHomeSystemId, systemName } from "../../core/derive/geo";
import { researchQueueIds, sendResearchQueue } from "../../core/derive/research";
import { icon, label } from "../../icons";
import {
  fleetCargoUnits,
  fleetExactCount,
  countClassLabel,
  type AcademyRow,
  type CaptainAttribute,
  type CaptainRosterView,
  type GhostView,
  type ProgrammeView,
} from "../../protocol";
import { liveSimTime } from "../../state";
import { captainPortrait } from "../art";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
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

/** Corporation-wide fleet, officer and research routes. These surfaces share
 * the same served owner picture; no roster row or personnel status is corrected
 * from true positions between reports. */
export class DeckRosterRoutes {
  private signature = "";

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: RosterHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "fleets" && route?.name !== "officers" && route?.name !== "research") return false;
    const active = document.activeElement;
    if (!force && active instanceof HTMLElement && this.root.contains(active) && active.matches("select, input")) return true;
    const signature = sheetFingerprint([
      route, Math.floor(liveSimTime()), this.ctx.state.ghosts, this.ctx.state.selectedShipIds,
      this.ctx.state.battles, this.ctx.state.commandSignals, this.ctx.state.orders,
      this.ctx.state.raids, this.ctx.state.captains, this.ctx.state.captainCapacity,
      this.ctx.state.research, this.ctx.state.systems,
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
    } else if (action === "research-add") {
      const id = button.dataset.programme;
      const queue = deckResearchQueue();
      if (id && !queue.includes(id)) {
        this.sendResearchQueue([...queue, id]);
        this.hooks.notice("<b>Programme queued</b> · awaiting the next served research report.");
      }
    } else if (action === "research-up" || action === "research-down" || action === "research-remove") {
      const queue = deckResearchQueue();
      const index = Number(button.dataset.index);
      if (Number.isFinite(index) && index >= 0 && index < queue.length) {
        if (action === "research-remove") queue.splice(index, 1);
        else if (action === "research-up" && index > 0) [queue[index - 1], queue[index]] = [queue[index], queue[index - 1]];
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
    const group = (name: string, entries: GhostView[]) => entries.length ? `<section class="deck-section"><header><div><h3>${esc(name)}</h3><p>Served status and location.</p></div><b>${entries.length}</b></header><div class="deck-roster">${entries.map((entry) => this.fleetRow(entry)).join("")}</div></section>` : "";
    const grouped = this.ctx.state.selectedShipIds.size > 1;
    return `<section class="deck-page deck-fleets"><header class="deck-page__lead"><span>Corporation-wide served roster</span><h2>${icon("fleet", "md")} Fleets</h2><p>${grouped ? `${this.ctx.state.selectedShipIds.size} fleets grouped. The next map move applies to the whole group.` : "Group fleets here, then choose one map destination for a batch move."}</p></header>${fleets.length ? group("Undocked", undocked) + group("Docked", docked) : empty("No fleet reports", "Build a ship or wait for a fleet report to arrive.")}</section>`;
  }

  private fleetRow(g: GhostView): string {
    const exact = fleetExactCount(g);
    const composition = (g.composition ?? []).filter((entry) => entry.count > 0).map((entry) => `${entry.count}× ${shipKindLabel(entry.kind)}`).join(" · ");
    const cargo = fleetCargoUnits(g);
    const dock = fleetRosterDockName(g);
    const battle = this.ctx.state.battles.some((entry) => entry.participants.includes(g.id));
    const guard = g.guard_target ? this.ctx.state.ghosts.find((entry) => entry.id === g.guard_target && entry.own) : undefined;
    const activity = dock ? `Docked · ${dock}` : battle ? "In battle" : guard ? `Guarding ${shipKindLabel(guard.kind)} fleet` : this.rosterActivity(g);
    const grouped = this.ctx.state.selectedShipIds.has(g.id);
    const name = g.kind === "titan" && this.ctx.state.syndicate?.flagship_name?.trim() || `${shipKindLabel(g.kind)} fleet`;
    const summary = [exact === null ? `estimated ${countClassLabel(g.count_class)} ships` : `${exact} ship${exact === 1 ? "" : "s"}`, composition, cargo ? `${fmt(cargo)} cargo` : ""].filter(Boolean).join(" · ");
    return `<article class="deck-roster-row${grouped ? " is-grouped" : ""}"><button type="button" data-deck-act="roster-open" data-fleet="${escAttr(g.id)}"><span>${icon(g.kind === "convoy" ? "convoy" : "fleet", "md")}<span><b>${esc(name)}</b><small>${esc(summary)}</small><em>${esc(activity)}</em></span></span><span class="deck-stale-value"><b>${esc(informationDelay(g.age))}</b></span></button><div><button type="button" data-deck-act="roster-group" data-fleet="${escAttr(g.id)}" aria-pressed="${grouped}">${grouped ? "✓ Grouped" : "+ Group"}</button><button type="button" data-deck-act="roster-center" data-fleet="${escAttr(g.id)}">Center</button></div></article>`;
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
    if (!research) return `<section class="deck-page"><header class="deck-page__lead"><span>Private programme boards</span><h2>Research unavailable</h2><p>Reconnect to restore the corporation's research picture.</p></header></section>`;
    const queue = deckResearchQueue();
    const active = research.active ? this.activeResearchHtml(research.active, research.rate, research.stalled, research.academies) : `<div class="deck-alert"><b>No active programme</b><span>Choose any available node. The front of the queue begins accruing immediately.</span></div>`;
    const activePinned = !!research.active;
    const queueHtml = queue.length ? queue.map((id, index) => {
      const programme = research.programmes.find((entry) => entry.id === id);
      const pinned = activePinned && index === 0;
      const firstQueued = activePinned ? index === 1 : index === 0;
      return `<div class="deck-research-queue-row"><span><b>${index + 1}</b><span><strong>${esc(programme?.name ?? id)}</strong><small>${pinned ? "active · pinned" : "queued"}</small></span></span><div><button type="button" data-deck-act="research-up" data-index="${index}" ${pinned || firstQueued ? "disabled" : ""}>↑</button><button type="button" data-deck-act="research-down" data-index="${index}" ${pinned || index === queue.length - 1 ? "disabled" : ""}>↓</button><button type="button" data-deck-act="research-remove" data-index="${index}" ${pinned ? "disabled" : ""}>Remove</button></div></div>`;
    }).join("") : `<div class="deck-empty-inline">Queue empty — choose an available programme below.</div>`;
    const academyReady = research.academies.some((academy) => academy.supplied);
    const boards = FIELD_ORDER.map((field) => this.researchBoard(field, research.programmes.filter((entry) => entry.field === field), queue, academyReady)).join("");
    return `<section class="deck-page deck-research"><header class="deck-page__lead"><span>Private corporation programme boards</span><h2>Research</h2><p>Programmes apply corporation-wide when their completion report is served.</p></header>${active}<section class="deck-section"><header><div><h3>Programme queue</h3><p>Reorder or remove work; the first row is active.</p></div><b>${queue.length}</b></header><div class="deck-research-queue">${queueHtml}</div></section><div class="deck-research-boards">${boards}</div></section>`;
  }

  private activeResearchHtml(active: { name: string; progress: number; cost: number; eta_secs: number | null }, rate: number, stalled: boolean, academies: AcademyRow[]): string {
    const pct = Math.max(0, Math.min(100, active.progress / Math.max(1e-9, active.cost) * 100));
    const eta = active.eta_secs !== null ? fmtEta(active.eta_secs) : stalled ? "stalled" : "awaiting supply";
    const academy = academies.length ? academies.map((row) => `<div class="deck-academy-row${row.supplied ? "" : " is-warn"}"><span><b>${esc(row.system)}</b><small>${row.supplied ? "supplied" : "unsupplied"}</small></span><em>Tier ${ROMAN[row.tier] ?? row.tier} · ${row.rate.toFixed(2)}/s</em></div>`).join("") : `<div class="deck-alert"><b>No staffed Academy</b><span>Build and staff an Academy to produce research.</span></div>`;
    return `<section class="deck-section deck-research-active"><header><div><span>Active programme</span><h3>${esc(active.name)}</h3></div><button type="button" data-deck-act="research-academy">Open Academy</button></header><div class="deck-research-progress"><div><span>${fmt(active.progress)} / ${fmt(active.cost)} research-seconds</span><b>${esc(eta)} · ${rate.toFixed(2)}/s</b></div><div class="deck-meter"><i style="width:${pct.toFixed(1)}%"></i></div></div><div class="deck-academies">${academy}</div></section>`;
  }

  private researchBoard(field: string, programmes: ProgrammeView[], queue: string[], academyReady: boolean): string {
    const schools = [...new Set(programmes.filter((entry) => entry.school).map((entry) => entry.school as string))];
    const group = (school: string | null, tier: number) => {
      const rows = programmes.filter((entry) => (entry.school ?? null) === school && entry.tier === tier);
      if (!rows.length) return "";
      const gate = rows.find((entry) => entry.gate)?.gate;
      const gateHtml = gate ? `<div class="deck-research-gate"><span>${esc(gate.label)} · ${Math.floor(gate.current)} / ${Math.round(gate.threshold)}${gate.current >= gate.threshold ? " · gate met" : ""}</span><div class="deck-meter"><i style="width:${Math.max(0, Math.min(100, gate.current / Math.max(1e-9, gate.threshold) * 100)).toFixed(1)}%"></i></div></div>` : "";
      const gateMet = !gate || gate.current >= gate.threshold;
      return `<section class="deck-research-tier"><h4>Tier ${ROMAN[tier]}</h4>${gateHtml}${rows.map((entry) => this.researchNode(entry, queue.indexOf(entry.id), academyReady, gateMet)).join("")}</section>`;
    };
    return `<article class="deck-research-board"><header><span>${icon(fieldIcon(field), "md")}</span><h3>${esc(FIELD_TITLE[field] ?? label(field))}</h3></header>${group(null, 1)}${group(null, 2)}${schools.map((school) => `<div class="deck-research-school">${esc(SCHOOL_TITLE[school] ?? label(school))}</div>${[3, 4, 5, 6, 7, 8].map((tier) => group(school, tier)).join("")}`).join("")}</article>`;
  }

  private researchNode(programme: ProgrammeView, queueIndex: number, academyReady: boolean, gateMet: boolean): string {
    const available = programme.state === "available";
    const queued = queueIndex >= 0;
    const prerequisite = (available || queued) && !academyReady
      ? `<em class="deck-research-prerequisite">Requires a staffed, supplied Academy to progress.</em>`
      : "";
    const stateLabel = programme.state === "locked" && gateMet ? "prerequisites required" : label(programme.state);
    return `<article class="deck-research-node is-${escAttr(programme.state)}"><header><b>${esc(programme.name)}</b>${queued ? `<em>#${queueIndex + 1}</em>` : `<em>${esc(stateLabel)}</em>`}</header><p>${esc(programme.blurb)}</p>${prerequisite}${available ? `<button type="button" data-deck-act="research-add" data-programme="${escAttr(programme.id)}">Add to queue</button>` : ""}</article>`;
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

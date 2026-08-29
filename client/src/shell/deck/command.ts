import { fmtDur } from "../../core/derive/format";
import { foundingHomeSystemId } from "../../core/derive/geo";
import { nextDecisionLabel } from "../../core/derive/orders";
import { icon, label, type IconKey } from "../../icons";
import type { BodyView, FoundingStage, FoundingView, GhostView, SystemInfo, SystemStateView } from "../../protocol";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

const FOUNDING_MINIMIZED_KEY = "stellar-syndicates:founding-guide-minimized";
const FOUNDING_STEP: Record<FoundingStage, number> = {
  build_shipyard: 1, build_mine: 2, build_convoy: 3, export_production: 4,
  defeat_privateer: 5, complete_export: 6, build_academy: 7, first_research: 8,
  build_scout: 9, survey_candidates: 10, build_colony: 11,
  establish_colony: 12, complete: 12,
};

interface CommandHooks {
  go(route: DeckRoute): void;
  focusFleet(id: string): void;
  notice(html: string): void;
}

interface FoundingContent {
  title: string;
  copy: string;
  action: string;
  label: string;
}

interface CommandDecision {
  key: string;
  weight: number;
  tone: "bad" | "warn" | "info" | "good";
  icon: IconKey;
  title: string;
  copy: string;
  route?: DeckRoute;
  systemId?: string;
  fleetId?: string;
}

/** Command is the routed home for the served picture. The short digest is a
 * pure presentation of data already delivered to this client; D5 expands the
 * same slot with the complete decision-inbox vocabulary. */
export class DeckCommandRoutes {
  private minimized = false;
  private signature = "";
  private foundingSignature = "";
  private decisions: CommandDecision[] = [];

  constructor(
    private readonly workspaceRoot: HTMLElement,
    private readonly foundingRoot: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: CommandHooks,
  ) {
    try { this.minimized = localStorage.getItem(FOUNDING_MINIMIZED_KEY) === "1"; } catch { /* session-only */ }
  }

  render(route: DeckRoute | null, force = false): boolean {
    this.renderFounding(force);
    if (route?.name !== "command") return false;
    const signature = sheetFingerprint([
      Math.floor(this.ctx.state.simTime), this.ctx.state.founding, this.ctx.state.systems,
      this.ctx.state.battles, this.ctx.state.operations, this.ctx.state.timeline.slice(-8),
      this.ctx.state.midgameStage,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.workspaceRoot.id, () => this.render(route, true))) return true;
    this.signature = signature;
    this.decisions = this.commandDecisions().slice(0, 4);
    setHtml(this.workspaceRoot, this.commandHtml());
    return true;
  }

  handleWorkspaceAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "command") return false;
    if (button.dataset.deckAct === "command-operations") {
      this.hooks.go({ name: "operations" });
      return true;
    }
    if (button.dataset.deckAct === "command-logistics") {
      this.hooks.go({ name: "logistics" });
      return true;
    }
    if (button.dataset.deckAct === "command-doctrine") {
      this.hooks.go({ name: "doctrine" });
      return true;
    }
    if (button.dataset.deckAct !== "command-decision") return false;
    const decision = this.decisions[Number(button.dataset.index)];
    if (!decision) return true;
    if (decision.fleetId) this.hooks.focusFleet(decision.fleetId);
    else if (decision.systemId) this.focusSystem(decision.systemId);
    if (decision.route) this.hooks.go(decision.route);
    return true;
  }

  handleFoundingAction(button: HTMLButtonElement): boolean {
    const action = button.dataset.deckAct;
    if (action === "founding-toggle") {
      this.minimized = !this.minimized;
      try { localStorage.setItem(FOUNDING_MINIMIZED_KEY, this.minimized ? "1" : "0"); } catch { /* session-only */ }
      this.foundingSignature = "";
      this.renderFounding(true);
      return true;
    }
    if (action === "founding-candidate") {
      const id = button.dataset.id;
      if (id) this.openSystem(id);
      return true;
    }
    if (action !== "founding-action") return false;
    const founding = this.ctx.state.founding;
    if (!founding) return true;
    this.runFoundingAction(button.dataset.action ?? "", founding);
    return true;
  }

  invalidate(): void {
    this.signature = "";
    this.foundingSignature = "";
  }

  teardown(): void {
    this.foundingRoot.hidden = true;
    this.foundingRoot.replaceChildren();
  }

  private renderFounding(force: boolean): void {
    const founding = this.ctx.state.founding;
    const signature = sheetFingerprint([Math.floor(this.ctx.state.simTime), founding, this.minimized, this.ctx.state.systems, this.ctx.state.ghosts]);
    if (!force && signature === this.foundingSignature) return;
    this.foundingSignature = signature;
    if (!founding || (founding.stage === "complete" && !founding.protected)) {
      this.foundingRoot.hidden = true;
      return;
    }
    const content = this.foundingContent(founding);
    const minLeft = Math.max(0, founding.protection_min_until - this.ctx.state.simTime);
    const shield = founding.protected ? minLeft > 0 ? `shield · ${fmtDur(minLeft)}` : "shield active" : "shield ended";
    const prospects = ["survey_candidates", "build_colony", "establish_colony"].includes(founding.stage) ? this.prospectCards(founding) : "";
    const disabled = !this.foundingActionAvailable(content.action, founding);
    const expanded = !this.minimized;
    setHtml(this.foundingRoot,
      `<header><span>Founding ${FOUNDING_STEP[founding.stage]}/12</span><em>${esc(shield)}</em><button type="button" data-deck-act="founding-toggle" aria-expanded="${expanded}" aria-label="${expanded ? "Minimize" : "Expand"} founding guide">${expanded ? "−" : "+"}</button></header>` +
      `<div class="deck-founding__body"><b>${esc(content.title)}</b><p>${esc(content.copy)}</p>${prospects}<button type="button" class="is-primary" data-deck-act="founding-action" data-action="${esc(content.action)}" ${disabled ? "disabled" : ""}>${esc(content.label)}</button></div>`);
    this.foundingRoot.classList.toggle("is-minimized", this.minimized);
    this.foundingRoot.hidden = false;
  }

  private commandHtml(): string {
    const founding = this.ctx.state.founding;
    const step = founding ? FOUNDING_STEP[founding.stage] : 0;
    const foundingCard = founding
      ? `<article class="deck-command-progress"><header><span>Founding programme</span><b>${step}/12</b></header><div><i style="width:${(step / 12 * 100).toFixed(1)}%"></i></div><p>${founding.stage === "complete" ? "Founding complete. Ordinary expansion and trade clocks now apply." : this.foundingContent(founding).title}</p></article>`
      : "";
    const decisions = this.decisions.length
      ? this.decisions.map((decision, index) => `<article class="deck-decision is-${decision.tone}"><span>${icon(decision.icon, "md")}</span><div><b>${esc(decision.title)}</b><p>${esc(decision.copy)}</p></div>${decision.route || decision.systemId || decision.fleetId ? `<button type="button" data-deck-act="command-decision" data-index="${index}">Focus</button>` : ""}</article>`).join("")
      : `<div class="deck-command-clear">${icon("success", "md")}<span><b>Command picture clear</b><small>${esc(nextDecisionLabel())}</small></span></div>`;
    const operations = this.ctx.state.operations.filter((entry) => entry.state === "active" || entry.state === "offered").slice(0, 3);
    const objectives = operations.length
      ? operations.map((entry) => `<button type="button" class="deck-objective" data-deck-act="command-operations"><span><b>${esc(operationTitle(entry))}</b><small>${esc(label(entry.state))} · ${entry.progress}/${entry.goal}</small></span><em>${entry.expires_at > this.ctx.state.simTime ? fmtDur(entry.expires_at - this.ctx.state.simTime) : "closing"}</em></button>`).join("")
      : `<div class="deck-empty-inline">No active or offered operations.</div>`;
    return `<section class="deck-page deck-command-home"><header class="deck-page__lead"><span>Served command picture</span><h2>${esc(this.ctx.state.name || "Corporation")}</h2><p>${esc(label(this.ctx.state.midgameStage))} · ${esc(nextDecisionLabel())}</p></header>${foundingCard}<section class="deck-section"><header><div><h3>Decision digest</h3><p>The four highest-pressure facts already visible in your delayed picture.</p></div><b>${this.decisions.length}</b></header><div class="deck-decision-list">${decisions}</div></section><section class="deck-section"><header><div><h3>Fleet policy</h3><p>Automate physical supply routes and set the corporation's default autonomous behavior.</p></div></header><div class="deck-policy-links"><button type="button" data-deck-act="command-logistics">${icon("freightRoute", "sm")} Standing logistics</button><button type="button" data-deck-act="command-doctrine">${icon("doctrine", "sm")} Fleet doctrine</button></div></section><section class="deck-section"><header><div><h3>Objectives</h3><p>Contracts and strategic work visible to the corporation.</p></div></header>${objectives}<button type="button" class="deck-section-link" data-deck-act="command-operations">Open Operations</button></section></section>`;
  }

  private commandDecisions(): CommandDecision[] {
    const out: CommandDecision[] = [];
    const push = (entry: CommandDecision) => out.push(entry);
    for (const battle of this.ctx.state.battles) {
      if (!battle.own) continue;
      const fleet = this.ctx.state.ghosts.find((entry) => entry.own && battle.participants.includes(entry.id));
      push({ key: `battle:${battle.id}`, weight: 100, tone: "bad", icon: "battle", title: "Your fleet is engaged", copy: "A battle is underway. Open it before issuing unrelated commands.", route: { name: "battle", params: { id: battle.id, label: "Ongoing battle" } }, fleetId: fleet?.id });
    }
    for (const system of this.ctx.state.systems.filter((entry) => entry.owner === this.ctx.state.playerId)) {
      const name = systemName(this.ctx, system.id);
      const route: DeckRoute = { name: "system", params: { id: system.id, systemLabel: name } };
      if (system.blockade) push({ key: `blockade:${system.id}`, weight: 90, tone: "bad", icon: "blockade", title: `${name} is blockaded`, copy: "Shipping is interdicted. Break the blockade before logistics can resume.", route, systemId: system.id });
      if (system.storage_cap > 0 && system.storage_used >= system.storage_cap) push({ key: `storage:${system.id}`, weight: 70, tone: "warn", icon: "storage", title: `${name} storage is full`, copy: "Production idles at capacity. Ship goods or build storage.", route, systemId: system.id });
      if (system.population > 0 && !system.habitat_fed) push({ key: `food:${system.id}`, weight: 65, tone: "warn", icon: "unfed", title: `${name} is ${label(system.food_state)}`, copy: "Supply is constraining workforce and migration.", route, systemId: system.id });
      if (system.slots_total > 0 && system.slots_used === 0 && !system.builds.length) push({ key: `idle:${system.id}`, weight: 42, tone: "info", icon: "build", title: `${name} has no development`, copy: `${system.slots_total} world slots are idle. Give this holding a role.`, route, systemId: system.id });
    }
    const latestWarn = [...this.ctx.state.timeline].reverse().find((entry) => entry.severity === "bad" || entry.severity === "warn");
    if (latestWarn) push({ key: `timeline:${latestWarn.at_time}`, weight: 35, tone: latestWarn.severity === "bad" ? "bad" : "warn", icon: "warning", title: "Recent command report", copy: latestWarn.text, route: { name: "log" } });
    out.sort((a, b) => b.weight - a.weight || a.key.localeCompare(b.key));
    return out;
  }

  private foundingContent(founding: FoundingView): FoundingContent {
    const home = this.homeDynamic();
    const mineBody = home?.bodies.find((body) => (body.structures.mining_complex ?? 0) > 0);
    const mineStaffed = !!mineBody && home?.assignments.some((line) => line.body_id === mineBody.id && line.structure === "mining_complex" && line.workers > 0);
    const academyBody = home?.bodies.find((body) => (body.structures.academy ?? 0) > 0);
    const academyStaffed = !!academyBody && home?.assignments.some((line) => line.body_id === academyBody.id && line.structure === "academy" && (line.workers > 0 || Object.values(line.specialists).some((count) => count > 0)));
    const content: Record<FoundingStage, FoundingContent> = {
      build_shipyard: { title: "Build Shipyard I", copy: "Establish orbital shipbuilding with the local founding kit.", action: "build-shipyard", label: "Build Shipyard I" },
      build_mine: mineBody
        ? { title: mineStaffed ? "Mining Complex staffed" : "Assign workforce to Mining Complex I", copy: mineStaffed ? `${mineBody.name} is producing Metallic Ore.` : "An unstaffed mine produces no ore.", action: "mine", label: `Open ${mineBody.name}` }
        : { title: "Build Mining Complex I", copy: "Establish the ore line for the opening export.", action: "build-mine", label: "Build Mining Complex I" },
      build_convoy: { title: "Build your first Freighter", copy: "Use the remaining local founding kit at the Shipyard.", action: "build-convoy", label: "Build Freighter" },
      export_production: { title: "Dispatch the opening export", copy: "Load Provisions and Metallic Ore, then send the Freighter.", action: "convoy", label: "Select Freighter" },
      defeat_privateer: { title: "Guard the Freighter", copy: "Intercept the Rogue Privateer before it reaches the civilian hull.", action: "privateer", label: "Select Rogue Privateer" },
      complete_export: { title: "Complete the guarded export", copy: "Deliver and sell both opening goods at the Market Hub.", action: "convoy", label: "Select Freighter" },
      build_academy: academyBody
        ? { title: academyStaffed ? "Academy staffed" : "Assign workforce to Academy I", copy: academyStaffed ? "Its first research report is reaching command." : "An unstaffed Academy produces no research.", action: "academy", label: `Open ${academyBody.name}` }
        : { title: "Establish Academy I", copy: "Acquire the required goods, then build and staff an Academy.", action: "market", label: "Open Market" },
      first_research: { title: "Choose your first programme", copy: "Complete any Tier I programme.", action: "research", label: "Open Research" },
      build_scout: { title: "Build a Scout", copy: "Prepare the exploration hull for the next chapter.", action: "market", label: "Open Market" },
      survey_candidates: { title: "Compare two expansion prospects", copy: "Survey both assigned systems and wait for each report.", action: "scout", label: "Select Scout" },
      build_colony: { title: "Build your first Colony Ship", copy: "Choose a prospect and assemble its settlement hull.", action: "market", label: "Open Market" },
      establish_colony: { title: "Establish your second holding", copy: "Send the Colony Ship to the prospect that fits your strategy.", action: "colony", label: "Select Colony Ship" },
      complete: { title: "Founding complete", copy: "Research, trade and expansion now run on their ordinary clocks.", action: "command", label: "Open Command" },
    };
    return content[founding.stage];
  }

  private runFoundingAction(action: string, founding: FoundingView): void {
    const homeId = foundingHomeSystemId();
    const home = this.homeDynamic();
    const system = homeId ? this.ctx.state.galaxy?.systems.find((entry) => entry.id === homeId) : undefined;
    if (action === "build-shipyard" && homeId && home && system) {
      const body = bestShipyardBody(home);
      if (body) this.openBuild(system, body, "structures", "shipyard");
    } else if (action === "build-mine" && homeId && home && system) {
      const body = bestMineBody(home);
      if (body) this.openBuild(system, body, "structures", "mining_complex");
    } else if ((action === "mine" || action === "academy") && homeId && home && system) {
      const structure = action === "mine" ? "mining_complex" : "academy";
      const body = home.bodies.find((entry) => (entry.structures[structure] ?? 0) > 0);
      if (body) this.openWorld(system, body);
    } else if (action === "build-convoy" && homeId && home && system) {
      const body = home.bodies.find((entry) => (entry.structures.shipyard ?? 0) > 0) ?? bestShipyardBody(home);
      if (body) this.openBuild(system, body, "ships", "convoy");
    } else if (action === "market") {
      this.hooks.go({ name: "market" });
    } else if (action === "research") {
      this.hooks.go({ name: "research" });
    } else if (action === "command") {
      this.hooks.go({ name: "command" });
    } else if (action === "privateer" && founding.privateer) {
      this.hooks.focusFleet(founding.privateer);
    } else if (action === "convoy" || action === "scout" || action === "colony") {
      const fleet = this.fleetOfKind(action);
      if (fleet) this.hooks.focusFleet(fleet.id);
    }
  }

  private foundingActionAvailable(action: string, founding: FoundingView): boolean {
    if (["market", "research", "command"].includes(action)) return true;
    if (["build-shipyard", "build-mine", "build-convoy", "mine", "academy"].includes(action)) return !!foundingHomeSystemId();
    if (action === "privateer") return !!founding.privateer && this.ctx.state.ghosts.some((entry) => entry.id === founding.privateer);
    return action === "convoy" || action === "scout" || action === "colony" ? !!this.fleetOfKind(action) : true;
  }

  private prospectCards(founding: FoundingView): string {
    const cards = founding.survey_candidates.slice(0, 2).map((id, index) => {
      const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === id);
      const dynamic = this.ctx.state.systems.find((entry) => entry.id === id);
      if (!system) return "";
      const surveyed = dynamic?.bodies.some((body) => body.geology !== null);
      const top = dynamic?.opportunities?.[0];
      const finding = surveyed ? top ? `${top.title} · ×${top.score.toFixed(2)}` : "Survey received" : `${label(system.band)} spectrum · awaiting survey`;
      return `<button type="button" data-deck-act="founding-candidate" data-id="${esc(id)}"><span><small>${index ? "Industrial prospect" : "Population prospect"}</small><b>${esc(system.name)}</b></span><em>${esc(finding)}</em></button>`;
    }).join("");
    return cards ? `<div class="deck-founding__prospects">${cards}</div>` : "";
  }

  private openBuild(system: SystemInfo, body: BodyView, mode: "structures" | "ships", select: string): void {
    this.focusSystem(system.id);
    this.ctx.renderer.pulseSystemBody(String(body.id));
    this.hooks.go({ name: "build", params: { systemId: system.id, systemLabel: system.name }, query: { body: String(body.id), mode, select } });
  }

  private openWorld(system: SystemInfo, body: BodyView): void {
    this.focusSystem(system.id);
    this.ctx.renderer.pulseSystemBody(String(body.id));
    this.hooks.go({ name: "world", params: { systemId: system.id, systemLabel: system.name, bodyId: String(body.id), worldLabel: body.name } });
  }

  private openSystem(id: string): void {
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === id);
    if (!system) return;
    this.focusSystem(id);
    this.hooks.go({ name: "system", params: { id, systemLabel: system.name } });
  }

  private focusSystem(id: string): void {
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === id);
    if (!system) return;
    this.ctx.state.selectedSystemId = id;
    this.ctx.renderer.centerOnWorld(system.pos);
    this.ctx.renderer.stateVersion++;
  }

  private homeDynamic(): SystemStateView | undefined {
    const id = foundingHomeSystemId();
    return id ? this.ctx.state.systems.find((entry) => entry.id === id) : undefined;
  }

  private fleetOfKind(kind: "convoy" | "scout" | "colony"): GhostView | undefined {
    return this.ctx.state.ghosts.find((entry) => entry.own && entry.composition?.some((stack) => stack.kind === kind));
  }
}

function bestShipyardBody(system: SystemStateView): BodyView | undefined {
  return [...system.bodies].sort((a, b) => {
    const aLow = a.special === "low_gravity" ? 1 : 0;
    const bLow = b.special === "low_gravity" ? 1 : 0;
    return bLow - aLow || (b.industrial_slots ?? 0) - (a.industrial_slots ?? 0) || a.id - b.id;
  })[0];
}

function bestMineBody(system: SystemStateView): BodyView | undefined {
  return [...system.bodies].sort((a, b) => {
    const ar = a.deposits?.filter((entry) => entry.resource === "metallic_ore").reduce((sum, entry) => sum + entry.richness, 0) ?? 0;
    const br = b.deposits?.filter((entry) => entry.resource === "metallic_ore").reduce((sum, entry) => sum + entry.richness, 0) ?? 0;
    return br - ar || (b.resource_slots ?? 0) - (a.resource_slots ?? 0) || a.id - b.id;
  })[0];
}

function operationTitle(operation: { kind: { kind: string } }): string {
  return label(operation.kind.kind);
}

function systemName(ctx: CoreContext, id: string): string {
  return ctx.state.galaxy?.systems.find((entry) => entry.id === id)?.name ?? id;
}

function esc(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!);
}

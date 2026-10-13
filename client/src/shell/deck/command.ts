import { isPlayerFreighter } from "../../protocol";
import { fmtDur, operationTitle } from "../../core/derive/format";
import { foundingHomeSystemId } from "../../core/derive/geo";
import { nextDecisionLabel } from "../../core/derive/orders";
import { postVictoryHandoff } from "../../core/derive/handoff";
import { bestMineBody, FOUNDING_STEP, FOUNDING_TOTAL, foundingBargeGoal, foundingBelt, foundingBusinessGoals, foundingCommands, foundingScoutGoal, foundingLocalGoals, foundingPlan, shieldStatus, type FoundingPlan } from "../../core/derive/founding";
import { coach } from "../coach";
import { charterLedgerHtml } from "./ledger";
import { icon, label } from "../../icons";
import type { BodyView, FoundingStage, FoundingView, GhostView, SystemInfo, SystemStateView } from "../../protocol";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckInboxItem } from "./log";
import type { DeckRoute } from "./router";
import { handoffHtml, handleHandoffAction } from "./handoff";

const FOUNDING_MINIMIZED_KEY = "stellar-syndicates:founding-guide-minimized";

interface CommandHooks {
  go(route: DeckRoute): void;
  openWorld(systemId: string, bodyId: number, structure?: string): void;
  focusFleet(id: string): void;
  /** Open a fleet's details workbench (equipment lives there). */
  openFleetDetails?(id: string): void;
  notice(html: string): void;
  inbox(): DeckInboxItem[];
  runInboxPrimary(key: string): void;
}

interface FoundingContent {
  title: string;
  copy: string;
  action: string;
  label: string;
  alternatives?: FoundingContent[];
}

/** Command is the routed home for the served picture. The short digest is a
 * pure presentation of data already delivered to this client, using the same
 * complete decision-inbox vocabulary as Log. */
export class DeckCommandRoutes {
  private minimized = false;
  private waitingOpen = false;
  private signature = "";
  private foundingSignature = "";
  private decisions: DeckInboxItem[] = [];

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
      this.ctx.state.ghosts, this.ctx.state.research, this.ctx.state.charterLedger, this.ctx.state.syndicateInvites,
      this.ctx.state.syndicate, this.ctx.state.explorationSites,
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.workspaceRoot.id, () => this.render(route, true))) return true;
    this.signature = signature;
    this.decisions = this.hooks.inbox().slice(0, 4);
    setHtml(this.workspaceRoot, this.commandHtml());
    return true;
  }

  handleWorkspaceAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "command") return false;
    if (button.dataset.deckAct === "founding-action") return this.handleFoundingAction(button);
    if (handleHandoffAction(button, this.ctx, { go: this.hooks.go, openWorld: this.hooks.openWorld, selectFleet: this.hooks.focusFleet })) return true;
    if (button.dataset.deckAct === "command-operations") {
      this.hooks.go({ name: "operations" });
      return true;
    }
    if (button.dataset.deckAct === "command-logistics") {
      this.hooks.go({ name: "logistics" });
      return true;
    }
    if (button.dataset.deckAct === "ledger-open") {
      const route = button.dataset.route;
      if (route === "syndicate" || route === "market" || route === "operations" || route === "research") this.hooks.go({ name: route });
      return true;
    }
    if (button.dataset.deckAct === "command-doctrine") {
      this.hooks.go({ name: "doctrine" });
      return true;
    }
    if (button.dataset.deckAct !== "command-decision") return false;
    const decision = this.decisions[Number(button.dataset.index)];
    if (!decision) return true;
    this.hooks.runInboxPrimary(decision.key);
    this.signature = "";
    return true;
  }

  handleFoundingAction(button: HTMLButtonElement): boolean {
    const action = button.dataset.deckAct;
    if (action === "handoff-details") { this.hooks.go({ name: "operations" }); return true; }
    if (action === "founding-toggle") {
      this.minimized = !this.minimized;
      try { localStorage.setItem(FOUNDING_MINIMIZED_KEY, this.minimized ? "1" : "0"); } catch { /* session-only */ }
      this.foundingSignature = "";
      this.renderFounding(true);
      return true;
    }
    if (action === "founding-waiting") {
      this.waitingOpen = !this.waitingOpen;
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

  /** The inbox and floating guide share this exact founding action; neither
   * invents a competing "next step" or a shallower destination. */
  runFoundingPrimary(): void {
    const founding = this.ctx.state.founding;
    if (!founding || founding.stage === "complete") return;
    const plan = foundingPlan(this.ctx.state, this.homeDynamic());
    this.runFoundingAction(plan?.primary?.action ?? this.foundingContent(founding).action, founding);
  }

  private renderFounding(force: boolean): void {
    const founding = this.ctx.state.founding;
    const signature = sheetFingerprint([Math.floor(this.ctx.state.simTime), founding, this.minimized, this.waitingOpen,
      this.ctx.state.systems, this.ctx.state.ghosts, this.ctx.state.research, this.ctx.state.explorationSites]);
    if (!force && signature === this.foundingSignature) return;
    this.foundingSignature = signature;
    if (!founding || (founding.stage === "complete" && !founding.protected && !postVictoryHandoff().some(g => !g.done))) {
      this.foundingRoot.hidden = true;
      return;
    }
    const shield = shieldStatus(founding, this.ctx.state.simTime);
    const expanded = !this.minimized;
    const plan = foundingPlan(this.ctx.state, this.homeDynamic());
    if (plan) {
      const prospects = founding.stage === "survey_candidates" ? this.prospectCards(founding) : "";
      setHtml(this.foundingRoot, this.planHtml(plan, shield, expanded, founding, prospects));
      this.foundingRoot.classList.toggle("is-minimized", this.minimized);
      this.foundingRoot.hidden = false;
      return;
    }
    const content = this.foundingContent(founding);
    const prospects = founding.stage === "survey_candidates" ? this.prospectCards(founding) : "";
    const disabled = !this.foundingActionAvailable(content.action, founding);
    const actions = content.alternatives ? this.businessActions(content.alternatives)
      : `<button type="button" class="is-primary" data-deck-act="founding-action" data-action="${esc(content.action)}" ${disabled ? "disabled" : ""}>${esc(content.label)}</button>`;
    setHtml(this.foundingRoot,
      `<header><span>Founding ${FOUNDING_STEP[founding.stage]}/${FOUNDING_TOTAL}</span><em title="${esc(shield.detail)}">${esc(shield.label)}</em><button type="button" data-deck-act="founding-toggle" aria-expanded="${expanded}" aria-label="${expanded ? "Minimize" : "Expand"} founding guide">${expanded ? "−" : "+"}</button></header>` +
      `<div class="deck-founding__body"><b>${esc(content.title)}</b><p>${esc(content.copy)}</p>${prospects}${actions}${founding.stage === "complete" && postVictoryHandoff().some(g => !g.done) ? `<button type="button" data-deck-act="handoff-details">Next objectives &amp; rewards</button>` : ""}</div>`);
    this.foundingRoot.classList.toggle("is-minimized", this.minimized);
    this.foundingRoot.hidden = false;
  }

  /** §founding R4/R7: one next step with its reason, what is already under
   * way, and optional work behind "While you wait". */
  private planHtml(plan: FoundingPlan, shield: { label: string; detail: string }, expanded: boolean,
    founding: FoundingView, prospects: string): string {
    const p = plan.primary;
    const header = `<header><span>${plan.chapter.name} ${plan.chapter.done}/${plan.chapter.total}</span><em title="${esc(shield.detail)}">${esc(shield.label)}</em><button type="button" data-deck-act="founding-toggle" aria-expanded="${expanded}" aria-label="${expanded ? "Minimize" : "Expand"} founding guide">${expanded ? "−" : "+"}</button></header>`;
    const progress = plan.progress.length
      ? `<ul class="deck-founding__progress" aria-label="Under way">${plan.progress.map((line) => `<li>${esc(line)}</li>`).join("")}</ul>` : "";
    const action = p?.alternatives ? this.businessActions(p.alternatives)
      : p ? `<button type="button" class="is-primary" data-deck-act="founding-action" data-action="${esc(p.action)}" ${this.foundingActionAvailable(p.action, founding) ? "" : "disabled"}>${esc(p.label)}</button>` : "";
    const next = p ? `<div class="deck-founding__next${plan.blocked ? " is-blocked" : ""}"><small>${plan.blocked ? "Needs attention" : "Next"}</small><b>${esc(p.title)}</b>${p.reason ? `<em>${esc(p.reason)}</em>` : ""}<p>${esc(p.copy)}</p>${prospects}${action}</div>` : "";
    const waiting = plan.waiting.length
      ? `<div class="deck-founding__waiting"><button type="button" class="deck-founding__waiting-toggle" data-deck-act="founding-waiting" aria-expanded="${this.waitingOpen}">While you wait · ${plan.waiting.length}</button>${this.waitingOpen ? plan.waiting.map((step) => `<button type="button" data-deck-act="founding-action" data-action="${esc(step.action)}"><span><b>${esc(step.label)}</b><small>${esc(step.copy)}</small></span></button>`).join("") : ""}</div>` : "";
    const rewards = founding.stage === "complete" && postVictoryHandoff().some(g => !g.done)
      ? `<button type="button" data-deck-act="handoff-details">Next objectives &amp; rewards</button>` : "";
    return header + `<div class="deck-founding__body">${progress}${next}${waiting}${rewards}</div>`;
  }

  private commandHtml(): string {
    const founding = this.ctx.state.founding;
    const plan = founding ? foundingPlan(this.ctx.state, this.homeDynamic()) : null;
    const step = plan ? plan.chapter.done : founding ? FOUNDING_STEP[founding.stage] : 0;
    const total = plan ? plan.chapter.total : FOUNDING_TOTAL;
    const title = plan ? `${plan.chapter.name} chapter` : "Founding programme";
    const foundingCard = (founding
      ? `<article class="deck-command-progress"><header><span>${title}</span><b>${step}/${total}</b></header><div><i style="width:${(step / total * 100).toFixed(1)}%"></i></div><p>${founding.stage === "complete" ? "Founding complete. Develop home and grow your trade routes." : esc(plan?.primary?.title ?? this.foundingContent(founding).title)}</p>${plan ? `<ul class="deck-command-progress__steps">${plan.checklist.map((item) => `<li class="${item.done ? "is-done" : ""}"><span aria-label="${item.done ? "Done" : "To do"}">${item.done ? "✓" : "○"}</span>${esc(item.title)}</li>`).join("")}</ul>` : ""}</article>`
      : "") + (founding?.stage === "grow_business" ? this.businessActions(foundingBusinessGoals(this.ctx.state, this.homeDynamic())) : "")
      + charterLedgerHtml(this.ctx.state) + handoffHtml(true);
    const decisions = this.decisions.length
      ? this.decisions.map((decision, index) => `<article class="deck-decision is-${decision.tone === "negative" ? "bad" : decision.tone}"><span>${icon(decision.icon, "md")}</span><div><b>${esc(decision.headline)}</b><p>${esc(decision.stakes ?? "Served information needs your attention.")}</p></div>${decision.actions.length ? `<button type="button" data-deck-act="command-decision" data-index="${index}">${esc((decision.actions.find((action) => action.primary) ?? decision.actions[0]).label)}</button>` : ""}</article>`).join("")
      : `<div class="deck-command-clear">${icon("success", "md")}<span><b>Command picture clear</b><small>${esc(nextDecisionLabel())}</small></span></div>`;
    const operations = this.ctx.state.operations.filter((entry) => entry.state === "active" || entry.state === "offered").slice(0, 3);
    const objectives = operations.length
      ? operations.map((entry) => `<button type="button" class="deck-objective" data-deck-act="command-operations"><span><b>${esc(operationTitle(entry))}</b><small>${esc(label(entry.state))} · ${entry.progress}/${entry.goal}</small></span><em>${entry.expires_at > this.ctx.state.simTime ? fmtDur(entry.expires_at - this.ctx.state.simTime) : "closing"}</em></button>`).join("")
      : `<div class="deck-empty-inline">No active or offered operations.</div>`;
    return `<section class="deck-page deck-command-home"><header class="deck-page__lead"><span>Served command picture</span><h2>${esc(this.ctx.state.name || "Corporation")}</h2><p>${esc(label(this.ctx.state.midgameStage))} · ${esc(nextDecisionLabel())}</p></header>${foundingCard}<section class="deck-section"><header><div><h3>Decision digest</h3><p>The four highest-pressure facts already visible in your delayed picture.</p></div><b>${this.decisions.length}</b></header><div class="deck-decision-list">${decisions}</div></section><section class="deck-section"><header><div><h3>Fleet policy</h3><p>Automate physical supply routes and set the corporation's default autonomous behavior.</p></div></header><div class="deck-policy-links"><button type="button" data-deck-act="command-logistics">${icon("freightRoute", "sm")} Logistics</button><button type="button" data-deck-act="command-doctrine">${icon("doctrine", "sm")} Fleet doctrine</button></div></section><section class="deck-section"><header><div><h3>Objectives</h3><p>Contracts and strategic work visible to the corporation.</p></div></header>${objectives}<button type="button" class="deck-section-link" data-deck-act="command-operations">Open Operations</button></section></section>`;
  }

  private foundingContent(founding: FoundingView): FoundingContent {
    const home = this.homeDynamic();
    const local = foundingLocalGoals(this.ctx.state, home);
    if (local) return { title: founding.stage === "build_mine" ? "Bring the mine online" : "Send your first shipment", copy: founding.stage === "build_mine" ? "Build and staff the Mining Complex, then plan the guarded ore shipment." : "Send the guarded ore shipment when you are ready to leave. Its sale receipt arrives later.", action: local[0].action, label: local[0].label, alternatives: local };
    const mineBody = home?.bodies.find((body) => (body.structures.mining_complex ?? 0) > 0);
    const mineStaffed = !!mineBody && home?.assignments.some((line) => line.body_id === mineBody.id && line.structure === "mining_complex" && line.workers > 0);
    const academyBody = home?.bodies.find((body) => (body.structures.academy ?? 0) > 0);
    const academyStaffed = !!academyBody && home?.assignments.some((line) => line.body_id === academyBody.id && line.structure === "academy" && (line.workers > 0 || Object.values(line.specialists).some((count) => count > 0)));
    const content: Record<FoundingStage, FoundingContent> = {
      local_recovery: { title: "Send your first shipment", copy: "Load Ferrite Ore and send the Freighter to the Market Hub with the Interceptor guarding it.", action: "opening-imports", label: "Plan the shipment" },
      prepare_export: { title: "Send your first shipment", copy: "Load Ferrite Ore and send the Freighter to the Market Hub with the Interceptor guarding it.", action: "opening-imports", label: "Plan the shipment" },
      grow_business: { title: founding.local_opening ? "Shipment underway" : "Grow your business", copy: founding.local_opening ? "Your first session can end here. Your confirmed return route can sell and bring supplies home; check its report next session. A manual one-way order still needs return instructions." : "Expand exports or start refining. Either advances the tutorial; both remain open.", action: "command", label: "Choose next investment", alternatives: foundingBusinessGoals(this.ctx.state, home) },
      build_shipyard: { title: "Build a Shipyard", copy: "Import Alloys, Machinery and Electronics with your export earnings, then staff it.", action: "build-shipyard", label: "Build Shipyard" },
      build_mine: mineBody
        ? { title: mineStaffed ? "Mining Complex staffed" : "Assign workforce to Mining Complex I", copy: mineStaffed ? `${mineBody.name} is producing Ferrite Ore.` : "An unstaffed mine produces no ore.", action: "mine", label: `Open ${mineBody.name}` }
        : { title: "Build Mining Complex I", copy: "Establish the ore line for the opening export.", action: "build-mine", label: "Build Mining Complex I" },
      build_convoy: { title: "Prepare a Freighter", copy: "Assign workforce to the Shipyard, then build a Tiny Freighter.", action: "build-convoy", label: "Build Freighter" },
      build_second_freighter: { title: "Build a second Tiny Freighter", copy: "Import its materials and staff the Shipyard to expand your trade capacity.", action: "build-convoy", label: "Build Tiny Freighter" },
      export_production: { title: "Dispatch the opening export", copy: "Load Ferrite Ore, then send the Freighter.", action: "convoy", label: "Select Freighter" },
      defeat_privateer: { title: "Guard the Freighter", copy: "Intercept the Rogue Privateer before it reaches the civilian hull.", action: "privateer", label: "Select Rogue Privateer" },
      complete_export: { title: "Complete the guarded export", copy: "Sell Ferrite Ore at the Market Hub.", action: "convoy", label: "Select Freighter" },
      build_academy: academyBody
        ? { title: academyStaffed ? "Academy staffed" : "Assign workforce to Academy I", copy: academyStaffed ? "Its first research report is reaching command." : "An unstaffed Academy produces no research.", action: "academy", label: `Open ${academyBody.name}` }
        : { title: "Establish Academy I", copy: "Build and staff an Academy for your first research programme.", action: "build-academy", label: "Build Academy" },
      first_research: { title: "Start research", copy: "Build and staff an Academy, then choose any Tier I programme.", action: "research", label: "Open Research" },
      build_scout: foundingScoutGoal(home),
      survey_candidates: { title: "Chart a nearby system", copy: "Build a Survey Probe and send it to the assigned stop. Its report charts a belt for your first Mining Barge.", action: "build-probe", label: "Build Survey Probe" },
      build_barge: foundingBargeGoal(home, foundingBelt(this.ctx.state)?.name ?? "the charted belt"),
      mine_belt: { title: "Send the barge to the belt", copy: "Mine the belt your survey charted. This step completes when command hears it cutting ore.", action: "mine-belt", label: "Send Mining Barge" },
      haul_ore: { title: "Bring the ore home", copy: "Send the barge home and unload its ore, or sell it at the Market Hub.", action: "barge-home", label: "Send the barge home" },
      build_colony: { title: "Founding complete", copy: "Develop home and grow your trade routes.", action: "command", label: "Open Command" },
      establish_colony: { title: "Founding complete", copy: "Develop home and grow your trade routes.", action: "command", label: "Open Command" },
      complete: { title: "Founding complete", copy: "Develop home, grow your trade routes, or take a contract.", action: "operations", label: "Choose next objective" },
    };
    return content[founding.stage];
  }

  private runFoundingAction(action: string, founding: FoundingView): void {
    const homeId = foundingHomeSystemId();
    const home = this.homeDynamic();
    const system = homeId ? this.ctx.state.galaxy?.systems.find((entry) => entry.id === homeId) : undefined;
    const commands = foundingCommands(action, this.ctx.state, home);
    if (commands) {
      if (action === "guarded-recovery") {
        this.ctx.state.selectedExplorationSiteId = founding.recovery_site ?? null;
        this.hooks.go({ name: "operations" });
      }
      this.ctx.intent.beginFleetCommand(commands);
      coach("confirm-order", "Check the order, then confirm it here.", "above");
      return;
    }
    if (action === "home-development" && home && system) {
      const body = home.bodies.find(b => b.founding_camp);
      if (body) this.openWorld(system, body);
      coach("settlement-projects", "Settlement projects are here.");
    } else if (action === "recovery" || action === "guarded-recovery") {
      this.ctx.state.selectedExplorationSiteId = founding.recovery_site ?? null;
      this.hooks.go({ name: "operations" });
      coach(action === "guarded-recovery" ? "guarded-recovery" : "travel-here",
        action === "guarded-recovery" ? "Send the Freighter with its escort in one order." : "Send your Scout from here.");
    } else if (action === "opening-imports") {
      this.hooks.go({ name: "logistics" });
      coach("send-shipment", "Check the basket and spending cap, then send it. The Interceptor guards the trip.");
    } else if (action === "build-service-berth" && home && system) {
      const body = home.bodies.find(b => (b.structures.warehouse ?? 0) > 0);
      if (body) this.openBuild(system, body, "structures", "service_berth");
    } else
    if (action === "build-shipyard" && homeId && home && system) {
      const body = bestShipyardBody(home);
      if (body) this.openBuild(system, body, "structures", "shipyard");
    } else if (action === "build-mine" && homeId && home && system) {
      const body = bestMineBody(home);
      if (body) this.openBuild(system, body, "structures", "mining_complex");
    } else if (action === "build-academy" && home && system) {
      const body = [...home.bodies].sort((a, b) => (b.infrastructure_slots ?? 0) - (a.infrastructure_slots ?? 0))[0];
      if (body) this.openBuild(system, body, "structures", "academy");
    } else if (action === "build-smelter" && home && system) {
      const body = [...home.bodies].sort((a,b) => Number(b.special === "volcanic_mantle") - Number(a.special === "volcanic_mantle") || (b.industrial_slots ?? 0) - (a.industrial_slots ?? 0))[0];
      if (body) this.openBuild(system, body, "structures", "smelter");
    } else if (["mine", "academy", "shipyard", "smelter"].includes(action) && homeId && home && system) {
      const structure = action === "mine" ? "mining_complex" : action;
      const body = home.bodies.find((entry) => (entry.structures[structure] ?? 0) > 0);
      if (body) this.openWorld(system, body, structure);
    } else if (["build-convoy", "build-scout", "build-colony", "build-barge", "build-probe"].includes(action) && homeId && home && system) {
      const body = home.bodies.find((entry) => (entry.structures.shipyard ?? 0) > 0) ?? bestShipyardBody(home);
      if (body) this.openBuild(system, body, "ships", action === "build-convoy" ? "tiny_freighter"
        : action === "build-barge" ? "mining_barge" : action === "build-probe" ? "survey_probe" : action.slice("build-".length));
    } else if (action === "market") {
      this.hooks.go({ name: "market" });
    } else if (action === "research" || action === "research-enrichment") {
      this.hooks.go({ name: "research", query: action === "research-enrichment" ? { programme: "mat_enrichment" } : undefined });
    } else if (action === "command") {
      this.hooks.go({ name: "command" });
    } else if (action === "operations") {
      this.hooks.go({ name: "operations" });
    } else if (action === "privateer" && founding.privateer) {
      this.hooks.focusFleet(founding.privateer);
    } else if (action === "shipment-trip") {
      const fleet = this.fleetOfKind("convoy");
      this.hooks.go(fleet ? { name: "logistics", query: { fleet: fleet.id } } : { name: "logistics" });
    } else if (action === "convoy" || action === "scout" || action === "colony" || action === "barge" || action === "probe") {
      const fleet = this.fleetOfKind(action);
      if (fleet) this.hooks.focusFleet(fleet.id);
      // The Recon fit is done in the fleet details, where the refits live.
      if (fleet && action === "scout" && founding.recovered) this.hooks.openFleetDetails?.(fleet.id);
    }
    // §founding R5: point at the control the step needs, not just the screen.
    if (action.startsWith("build-")) coach("queue-build", "Queue the build here.");
    else if (["mine", "academy", "shipyard", "smelter"].includes(action)) coach("staff-plus", "Add one workforce here, then confirm it just below.");
    else if (action === "scout" && founding.recovered) coach("refit", "Fit the recovered Recon Suite here.");
  }

  private foundingActionAvailable(action: string, founding: FoundingView): boolean {
    if (["market", "research", "command", "operations"].includes(action)) return true;
    if (["build-shipyard", "build-mine", "build-academy", "build-convoy", "build-scout", "build-colony", "build-barge", "build-probe", "mine", "academy", "shipyard"].includes(action)) return !!foundingHomeSystemId();
    if (action === "privateer") return !!founding.privateer && this.ctx.state.ghosts.some((entry) => entry.id === founding.privateer);
    if (["mine-belt", "survey-stop", "barge-home", "barge-unload"].includes(action)) return !!foundingCommands(action, this.ctx.state, this.homeDynamic());
    return action === "convoy" || action === "scout" || action === "colony" || action === "barge" || action === "probe" ? !!this.fleetOfKind(action) : true;
  }

  private businessActions(goals: FoundingContent[]): string {
    return `<div class="deck-founding__prospects">${goals.map(goal => `<button type="button" data-deck-act="founding-action" data-action="${esc(goal.action)}" title="${esc(goal.copy)}"><span><small>${esc(goal.title)}</small><b>${esc(goal.label)}</b><small>${esc(goal.copy)}</small></span></button>`).join("")}</div>`;
  }

  private prospectCards(founding: FoundingView): string {
    const cards = founding.survey_candidates.slice(0, 2).map((id, index) => {
      const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === id);
      const dynamic = this.ctx.state.systems.find((entry) => entry.id === id);
      if (!system) return "";
      const surveyed = dynamic?.bodies.some((body) => body.geology !== null);
      const top = dynamic?.opportunities?.[0];
      const finding = surveyed ? top ? `${top.title} · ×${top.score.toFixed(2)}` : "Survey received" : `${label(system.band)} spectrum · awaiting survey`;
      return `<button type="button" data-deck-act="founding-candidate" data-id="${esc(id)}"><span><small>${founding.local_opening ? "Nearby survey destination" : index ? "Industrial prospect" : "Population prospect"}</small><b>${esc(system.name)}</b></span><em>${esc(finding)}</em></button>`;
    }).join("");
    return cards ? `<div class="deck-founding__prospects">${cards}</div>` : "";
  }

  private openBuild(system: SystemInfo, body: BodyView, mode: "structures" | "ships", select: string): void {
    this.focusSystem(system.id);
    this.ctx.renderer.pulseSystemBody(String(body.id));
    this.hooks.go({ name: "build", params: { systemId: system.id, systemLabel: system.name }, query: { body: String(body.id), mode, select } });
  }

  private openWorld(system: SystemInfo, body: BodyView, structure?: string): void {
    this.focusSystem(system.id);
    this.hooks.openWorld(system.id, body.id, structure);
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

  private fleetOfKind(kind: "convoy" | "scout" | "colony" | "barge" | "probe"): GhostView | undefined {
    const wanted = kind === "barge" ? "mining_barge" : kind === "probe" ? "survey_probe" : kind;
    return this.ctx.state.ghosts.find((entry) => entry.own && entry.composition?.some((stack) => (kind === "convoy" ? isPlayerFreighter(stack.kind) : stack.kind === wanted) && stack.count > 0));
  }
}

function bestShipyardBody(system: SystemStateView): BodyView | undefined {
  return [...system.bodies].sort((a, b) => {
    const aLow = a.special === "low_gravity" ? 1 : 0;
    const bLow = b.special === "low_gravity" ? 1 : 0;
    return bLow - aLow || (b.industrial_slots ?? 0) - (a.industrial_slots ?? 0) || a.id - b.id;
  })[0];
}

function esc(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!);
}

import { isPlayerFreighter } from "../../protocol";
import { introduceCaptainFor } from "../../captain-intro";
import { foundingHomeSystemId } from "../../core/derive/geo";
import { postVictoryHandoff } from "../../core/derive/handoff";
import { CHARTER_RESEARCH, expansionBody, FOUNDING_STEP, FOUNDING_TOTAL, foundingBargeGoal, foundingBelt, foundingBuildTarget, foundingBusinessGoals, foundingCommands, foundingScoutGoal, foundingLocalGoals, foundingPlan, landingBody, type FoundingPlan } from "../../core/derive/founding";
import { coach, CONTRACT_COACH } from "../coach";
import { repairYardBody } from "../../core/derive/shipyard";
import { advisorOnDuty, advisorTitle, stepAdvice } from "../../core/derive/advisor";
import { label } from "../../icons";
import type { BodyView, FoundingStage, FoundingView, GhostView, SystemInfo, SystemStateView } from "../../protocol";
import { setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

const FOUNDING_MINIMIZED_KEY = "stellar-syndicates:founding-guide-minimized";

interface GuideHooks {
  go(route: DeckRoute): void;
  openWorld(systemId: string, bodyId: number, structure?: string): void;
  /** §planet-build: a world with its builder open over the planet. */
  openWorldBuild?(systemId: string, bodyId: number, query: Record<string, string>, mode?: "structures" | "ships"): void;
  focusFleet(id: string): void;
  /** Open a fleet's details workbench (equipment lives there). */
  openFleetDetails?(id: string): void;
  /** §home-step: fly into a system's System View. */
  flyIntoSystem(id: string): void;
  /** §export-follow: put the camera and tracking window on the first shipment
   * leaving home (again, after the player dragged away). */
  followShipment?(id: string): void;
}

interface FoundingContent {
  title: string;
  copy: string;
  action: string;
  label: string;
  alternatives?: FoundingContent[];
}

/** The floating founding guide: the charter's next step, what is under way,
 * and optional work behind "While you wait". (§command-retired, Peter, Oct 31
 * 2026: the Command page that repeated it is gone; its charter ledger lives on
 * Operations, the chapter's steps on this card's header.) */
export class DeckFoundingGuide {
  private minimized = false;
  private waitingOpen = false;
  private foundingSignature = "";

  constructor(
    private readonly foundingRoot: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: GuideHooks,
  ) {
    try { this.minimized = localStorage.getItem(FOUNDING_MINIMIZED_KEY) === "1"; } catch { /* session-only */ }
  }

  handleFoundingAction(button: HTMLButtonElement): boolean {
    const action = button.dataset.deckAct;
    if (action === "handoff-details") { this.hooks.go({ name: "operations" }); return true; }
    if (action === "founding-toggle") {
      this.minimized = !this.minimized;
      try { localStorage.setItem(FOUNDING_MINIMIZED_KEY, this.minimized ? "1" : "0"); } catch { /* session-only */ }
      this.foundingSignature = "";
      this.render(true);
      return true;
    }
    if (action === "founding-waiting") {
      this.waitingOpen = !this.waitingOpen;
      this.foundingSignature = "";
      this.render(true);
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

  render(force = false): void {
    const founding = this.ctx.state.founding;
    const signature = sheetFingerprint([Math.floor(this.ctx.state.simTime), founding, this.minimized, this.waitingOpen,
      this.ctx.state.systems, this.ctx.state.ghosts, this.ctx.state.research, this.ctx.state.explorationSites]);
    if (!force && signature === this.foundingSignature) return;
    this.foundingSignature = signature;
    if (!founding || (founding.stage === "complete" && !founding.protected && !postVictoryHandoff().some(g => !g.done))) {
      this.foundingRoot.hidden = true;
      return;
    }
    // Peter (Nov 9 2026): no shield countdown on the guide; it moves elsewhere later.
    const expanded = !this.minimized;
    const plan = foundingPlan(this.ctx.state, this.homeDynamic());
    if (plan) {
      const prospects = founding.stage === "survey_candidates" ? this.prospectCards(founding) : "";
      setHtml(this.foundingRoot, this.planHtml(plan, expanded, founding, prospects));
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
      `<header><span>Founding ${FOUNDING_STEP[founding.stage]}/${FOUNDING_TOTAL}</span><b>${esc(content.title)}</b><button type="button" data-deck-act="founding-toggle" aria-expanded="${expanded}" aria-label="${expanded ? "Minimize" : "Expand"} founding guide">${expanded ? "−" : "+"}</button></header>` +
      `<div class="deck-founding__body"><p>${esc(content.copy)}</p>${prospects}${actions}${founding.stage === "complete" && postVictoryHandoff().some(g => !g.done) ? `<button type="button" data-deck-act="handoff-details">Next objectives &amp; rewards</button>` : ""}</div>`);
    this.foundingRoot.classList.toggle("is-minimized", this.minimized);
    this.foundingRoot.hidden = false;
  }

  /** §founding R4/R7: one next step with its reason, what is already under
   * way, and optional work behind "While you wait". */
  private planHtml(plan: FoundingPlan, expanded: boolean, founding: FoundingView, prospects: string): string {
    const p = plan.primary;
    // The chapter's steps on hover, done ones ticked.
    const steps = plan.checklist.map((item) => `${item.done ? "✓" : "○"} ${item.title}`).join("\n");
    // Peter (Nov 9 2026): the step's title beside the chapter (a waiting
    // card's progress line in its place), why on hover; no shield countdown.
    const title = p ? `<b${plan.blocked ? ` class="is-blocked"` : ""}${p.hint ? ` title="${esc(p.hint)}"` : ""}>${esc(p.status ?? p.title)}</b>` : "<b></b>";
    // Minimized, the toggle stands in for the next action (§charter-advisor's stall nudge rings it).
    const header = `<header><span${steps ? ` title="${esc(steps)}"` : ""}>${plan.chapter.name} ${plan.chapter.done}/${plan.chapter.total}</span>${title}<button type="button" data-deck-act="founding-toggle"${expanded ? "" : ` data-coach="founding-next"`} aria-expanded="${expanded}" aria-label="${expanded ? "Minimize" : "Expand"} founding guide">${expanded ? "−" : "+"}</button></header>`;
    const progress = plan.progress.length
      ? `<ul class="deck-founding__progress" aria-label="Under way">${plan.progress.map((line) => `<li>${esc(line)}</li>`).join("")}</ul>` : "";
    // §founding copy (Oct 27 2026): no "Next" eyebrow, the card is always next;
    // choices are buttons that say the whole step, the card's copy above them.
    const button = (action: string, text: string, primary: boolean) =>
      `<button type="button"${primary ? ` class="is-primary" data-coach="founding-next"` : ""} data-deck-act="founding-action" data-action="${esc(action)}" ${this.foundingActionAvailable(action, founding) ? "" : "disabled"}>${esc(text)}</button>`;
    // §charter-advisor: a state-specific reason first, then the advisor's
    // line in the step's own why's place, then that why. Peter (Nov 9 2026):
    // the face right beside the line, no quotes.
    const advisor = advisorOnDuty(founding);
    const advice = advisor && stepAdvice(plan, founding);
    const why = p?.reason ? `<em>${esc(p.reason)}</em>` : advisor && advice
      ? `<div class="deck-founding__advisor"><img src="${esc(advisor.face)}" alt="${esc(advisorTitle(advisor))}" title="${esc(advisorTitle(advisor))}" decoding="async"><p>${esc(advice)}</p></div>`
      : p?.why ? `<em>${esc(p.why)}</em>` : "";
    const action = p?.alternatives?.length ? p.alternatives.map((choice, i) => button(choice.action, choice.label, i === 0)).join("")
      : p ? button(p.action, p.label, true) : "";
    const next = p ? `<div class="deck-founding__next${plan.blocked ? " is-blocked" : ""}">${plan.blocked ? "<small>Needs attention</small>" : ""}`
      + `${why}${p.copy ? `<p>${esc(p.copy)}</p>` : ""}${prospects}${action}</div>` : "";
    const waiting = plan.waiting.length
      ? `<div class="deck-founding__waiting"><button type="button" class="deck-founding__waiting-toggle" data-deck-act="founding-waiting" aria-expanded="${this.waitingOpen}">While you wait · ${plan.waiting.length}</button>${this.waitingOpen ? plan.waiting.map((step) => `<button type="button" data-deck-act="founding-action" data-action="${esc(step.action)}"><span><b>${esc(step.title)}</b>${step.copy ? `<small>${esc(step.copy)}</small>` : ""}</span></button>`).join("") : ""}</div>` : "";
    const rewards = founding.stage === "complete" && postVictoryHandoff().some(g => !g.done)
      ? `<button type="button" data-deck-act="handoff-details">Next objectives &amp; rewards</button>` : "";
    return header + `<div class="deck-founding__body">${progress}${next}${waiting}${rewards}</div>`;
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
      // §habitat-first (Nov 8 2026): the Habitat, the Solar Array, then the food chain on its power.
      build_bioharvester: { title: "Build a Bioharvester", copy: "It harvests Biomass on 1 MW. Staff it when it stands.", action: "build-bioharvester", label: "Build Bioharvester" },
      build_habitat: { title: "Build a Habitat", copy: "Your settlers move out of the colony ship. No power needed.", action: "build-habitat", label: "Build Habitat" },
      build_agroplex: { title: "Build an Agroplex", copy: "It turns Biomass into Provisions on 1 MW. Staff it when it stands.", action: "build-agroplex", label: "Build Agroplex" },
      pirate_raid: { title: "The Smelter is running", copy: "Alloys are coming in.", action: "smelter", label: "Open the Smelter" },
      build_gun_platform: { title: "Build a Laser Battery", copy: "Pirates leave defended worlds alone.", action: "build-gun-platform", label: "Build Laser Battery" },
      sell_ore: { title: "Find semiconductors", copy: "The Shipyard needs them.", action: "market", label: "Open Market" },
      // §charter-v6, §habitat-first: step 2, before the farms and the mine.
      power_shipyard: { title: "Build a Solar Array", copy: "Light the colony: the farms, the mine and the Smelter run on it.", action: "build-power", label: "Build Solar Array" },
      buy_semiconductors: { title: "Build a Shipyard", copy: "40 Alloys, 20 Machinery and 15 Semiconductors, then staff it.", action: "build-shipyard", label: "Build Shipyard" },
      build_freighter: { title: "Build an Interceptor", copy: "It hunts the privateer.", action: "build-interceptor", label: "Build Interceptor" },
      build_interceptor: { title: "Build an Interceptor", copy: "Then assign its captain.", action: "build-interceptor", label: "Build Interceptor" },
      expand_mining: { title: "Expand mining operations", copy: "A Solar Array and a Mining Complex on another world of your system.", action: "build-expansion-power", label: "Build Solar Array" },
      local_recovery: { title: "Export your Alloys", copy: "Load Alloys and send the Freighter to the Market Hub.", action: "opening-imports", label: "Plan the shipment" },
      prepare_export: { title: "Export your Alloys", copy: "Load Alloys and send the Freighter to the Market Hub.", action: "opening-imports", label: "Plan the shipment" },
      grow_business: { title: founding.local_opening ? "Shipment underway" : "Grow your business", copy: founding.local_opening ? "Your first session can end here. Your confirmed return route can sell and bring supplies home; check its report next session. A manual one-way order still needs return instructions." : "Expand exports or start refining. Either advances the tutorial; both remain open.", action: "operations", label: "Choose next investment", alternatives: foundingBusinessGoals(this.ctx.state, home) },
      build_shipyard: { title: "Build a Shipyard", copy: "40 Alloys, 20 Machinery and 15 Semiconductors, then staff it.", action: "build-shipyard", label: "Build Shipyard" },
      build_mine: mineBody
        ? { title: mineStaffed ? "Mining Complex staffed" : "Assign workforce to Mining Complex I", copy: mineStaffed ? `${mineBody.name} is producing Ferrite Ore.` : "An unstaffed mine produces no ore.", action: "mine", label: `Open ${mineBody.name}` }
        : { title: "Build Mining Complex I", copy: "Establish the ore line for the opening export.", action: "build-mine", label: "Build Mining Complex I" },
      build_convoy: { title: "Build a Tiny Freighter", copy: "Assign workforce to the Shipyard, then build a Tiny Freighter.", action: "build-convoy", label: "Build Freighter" },
      build_second_freighter: { title: "Build a second Tiny Freighter", copy: "Import its materials and staff the Shipyard to expand your trade capacity.", action: "build-convoy", label: "Build Tiny Freighter" },
      export_production: { title: "Export your Alloys", copy: "Load Alloys, then send the Freighter.", action: "convoy", label: "Select Freighter" },
      defeat_privateer: { title: "Destroy the privateer", copy: "Send your Interceptor after it.", action: "attack-privateer", label: "Attack" },
      eliminate_den: { title: "Eliminate the privateer den", copy: "Send your Interceptor to the den its log named.", action: "assault-den", label: "Send Interceptor" },
      repair_interceptor: { title: "Repair the Interceptor", copy: "Bring it home; the Shipyard repairs it.", action: "repair-home", label: "Return to Home" },
      build_tiny_freighter: { title: "Build a Tiny Freighter", copy: "For the den's loot.", action: "build-convoy", label: "Build" },
      full_speed: { title: "Switch to Full speed", copy: "Set the Tiny Freighter's Transit to Full speed.", action: "full-speed", label: "Open Freighter" },
      haul_loot: { title: "Haul back the den's loot", copy: "Send the freighter to the den; it loads and comes home.", action: "stash-fetch", label: "Fetch" },
      complete_export: { title: "Complete the guarded export", copy: "Sell Ferrite Ore at the Market Hub.", action: "convoy", label: "Select Freighter" },
      build_academy: academyBody
        ? { title: academyStaffed ? "Academy staffed" : "Assign workforce to Academy I", copy: academyStaffed ? "Its first research report is reaching command." : "An unstaffed Academy produces no research.", action: "academy", label: `Open ${academyBody.name}` }
        : { title: "Establish Academy I", copy: "Build and staff an Academy for your first research programme.", action: "build-academy", label: "Build Academy" },
      first_research: { title: "Research Freight Frames", copy: "Build and staff an Academy, then research Freight Frames.", action: "research-charter", label: "Open Research" },
      answer_signal: { title: "Answer a signal", copy: "Send the Interceptor to a signal near home.", action: "signals", label: "Open Signals" },
      build_small_freighter: { title: "Build a Small Freighter", copy: "150 cargo, three Tiny Freighter holds.", action: "build-small-freighter", label: "Build" },
      build_smelter: { title: "Build a Smelter", copy: "Ferrite Ore → Alloys. Staff it when it stands.", action: "build-smelter", label: "Build Smelter" },
      build_scout: foundingScoutGoal(home),
      survey_candidates: { title: "Chart a nearby system", copy: "Build a Survey Probe and send it to the assigned stop. Its report charts a belt for your first Mining Barge.", action: "build-probe", label: "Build Survey Probe" },
      build_barge: foundingBargeGoal(home, foundingBelt(this.ctx.state)?.name ?? "the charted belt"),
      prospect_belt: { title: "Prospect the belt", copy: "A prospecting pass finds its hidden pockets.", action: "prospect-belt", label: "Prospect" },
      mine_belt: { title: "Send the barge to the belt", copy: "Mine the belt your survey charted. This step completes when command hears it cutting ore.", action: "mine-belt", label: "Send Mining Barge" },
      haul_ore: { title: "Bring the ore home", copy: "Send the barge home and unload its ore, or sell it at the Market Hub.", action: "barge-home", label: "Send the barge home" },
      secure_stores: { title: "Secure your stores", copy: "A Warehouse for room, a Vault raiders can't reach.", action: "build-store-warehouse", label: "Build Warehouse" },
      vendetta: { title: "Defeat the privateer's uncle", copy: "Keep two Interceptors at home. He attacks your strongest warship.", action: "attack-uncle", label: "Attack" },
      build_colony: { title: "Founding complete", copy: "Develop home and grow your trade routes.", action: "operations", label: "Choose next objective" },
      establish_colony: { title: "Founding complete", copy: "Develop home and grow your trade routes.", action: "operations", label: "Choose next objective" },
      complete: { title: "Founding complete", copy: "Develop home, grow your trade routes, or take a contract.", action: "operations", label: "Choose next objective" },
    };
    return content[founding.stage];
  }

  private runFoundingAction(action: string, founding: FoundingView): void {
    if (action === "restart-corporation") {
      // §gun-platform: the sim refuses it unless the colony fell.
      this.ctx.send({ type: "RestartCorporation" });
      return;
    }
    const homeId = foundingHomeSystemId();
    const home = this.homeDynamic();
    const system = homeId ? this.ctx.state.galaxy?.systems.find((entry) => entry.id === homeId) : undefined;
    const commands = foundingCommands(action, this.ctx.state, home);
    // §captain-intro: the captain is introduced first; Assign sends the order.
    if (commands && introduceCaptainFor(this.ctx.state, commands, () => {
      this.ctx.intent.beginFleetCommand(commands);
      this.ctx.intent.confirmPendingIntent();
    })) return;
    if (commands) {
      if (action === "guarded-recovery") {
        this.ctx.state.selectedExplorationSiteId = founding.recovery_site ?? null;
        this.hooks.go({ name: "operations" });
      }
      this.ctx.intent.beginFleetCommand(commands);
      coach("confirm-order", "Check the order, then confirm it here.", "above");
      return;
    }
    if (action === "open-home" && homeId) {
      this.hooks.flyIntoSystem(homeId);
    } else if (action === "home-development" && home && system) {
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
      coach("send-shipment", founding.stage === "defeat_privateer" ? "Check the basket and spending cap, then send it. The Interceptor guards the trip." : "Check the basket and spending cap, then send it.");
    } else if (action === "build-service-berth" && home && system) {
      const body = home.bodies.find(b => (b.structures.warehouse ?? 0) > 0);
      if (body) this.openBuild(system, body, "structures", "service_berth");
    } else if (action.startsWith("build-") && home && system) {
      // §charter-v4: the land grant keeps builds on the landing world until expansion.
      const target = foundingBuildTarget(action, this.ctx.state, home);
      if (target) this.openBuild(system, target.body, target.mode, target.select);
    } else if (action === "home-world" && home && system) {
      const body = landingBody(home);
      if (body) this.openWorld(system, body);
    } else if (action === "expansion-mine" && home && system) {
      const body = expansionBody(home);
      if (body) this.openWorld(system, body, "mining_complex");
    } else if (STAFF_STRUCTURE[action] && home && system) {
      const structure = STAFF_STRUCTURE[action];
      const body = home.bodies.find((entry) => (entry.structures[structure] ?? 0) > 0);
      if (body) this.openWorld(system, body, structure);
    } else if (action === "market") {
      this.hooks.go({ name: "market" });
    } else if (action === "watch-repair" && home && system) {
      // §watch-repair (Peter, Nov 11 2026): the planet view, where the hull
      // stands on the yard's pad and mends (§yard-repair). Its View shipyard
      // button there still opens the close-up.
      const yard = repairYardBody(home);
      const body = home.bodies.find((entry) => entry.id === yard) ?? landingBody(home);
      if (body) this.openWorld(system, body);
    } else if (action.startsWith("merchant-")) {
      // §merchant: open the free trader's stall with this build's shortfall filled in.
      this.hooks.go({ name: "merchant", query: { for: action.slice("merchant-".length) } });
    } else if (action === "research" || action === "research-charter") {
      // §freight-frames: the charter's research step opens on its programme.
      this.hooks.go({ name: "research", query: action === "research-charter" ? { programme: CHARTER_RESEARCH } : undefined });
    } else if (action === "operations" || action === "log" || action === "fleets") {
      this.hooks.go({ name: action });
    } else if (action === "contract") {
      // §privateer-contract: Contracts, the charter's contract on top.
      this.hooks.go({ name: "operations" });
    } else if (action === "privateer" && founding.privateer) {
      this.hooks.focusFleet(founding.privateer);
    } else if (action === "den" && founding.den) {
      this.openSystem(founding.den);
    } else if (action === "follow-shipment") {
      const fleet = this.fleetOfKind("convoy");
      if (fleet && this.hooks.followShipment) this.hooks.followShipment(fleet.id);
      else if (fleet) this.hooks.focusFleet(fleet.id);
    } else if (action === "shipment-trip") {
      const fleet = this.fleetOfKind("convoy");
      this.hooks.go(fleet ? { name: "logistics", query: { fleet: fleet.id } } : { name: "logistics" });
    } else if (action === "full-speed") {
      // §full-speed: the freighter's panel, at its Transit setting.
      const fleet = this.fleetOfKind("convoy");
      if (fleet) this.hooks.focusFleet(fleet.id);
    } else if (action === "signals") {
      this.hooks.go({ name: "signals" });
    } else if (action === "convoy" || action === "scout" || action === "colony" || action === "barge" || action === "probe" || action === "raider") {
      const fleet = this.fleetOfKind(action);
      if (fleet) this.hooks.focusFleet(fleet.id);
      // The Recon fit is done in the fleet details, where the refits live.
      if (fleet && action === "scout" && founding.recovered) this.hooks.openFleetDetails?.(fleet.id);
    }
    // §founding R5: point at the control the step needs, not just the screen.
    if (action.startsWith("build-")) coach("queue-build", "Queue the build here.");
    else if (STAFF_STRUCTURE[action] || action === "expansion-mine") coach("staff-plus", "Add one workforce here, then confirm it just below.");
    else if (action === "scout" && founding.recovered) coach("refit", "Fit the recovered Recon Suite here.");
    else if (action === "full-speed") coach("full-speed", "Full speed: twice as fast, on Deuterium.");
    else if (action === "signals") coach("signal-send", "Send the Interceptor.");
    else if (action === "contract") coach("contract-accept", CONTRACT_COACH);
  }

  private foundingActionAvailable(action: string, founding: FoundingView): boolean {
    if (["market", "research", "operations", "contract", "log", "fleets", "signals"].includes(action)) return true;
    if (action === "full-speed") return !!this.fleetOfKind("convoy");
    if (action.startsWith("build-") || STAFF_STRUCTURE[action] || ["expansion-mine", "home-world", "open-home", "watch-repair"].includes(action)) return !!foundingHomeSystemId();
    if (action === "privateer") return !!founding.privateer && this.ctx.state.ghosts.some((entry) => entry.id === founding.privateer);
    if (action === "den") return !!founding.den;
    if (["mine-belt", "prospect-belt", "survey-stop", "barge-home", "barge-unload", "attack-privateer", "hunt-privateer", "attack-uncle", "attack-strike", "defend-barge", "assign-captain", "interceptor-captain", "assault-den", "stash-fetch", "repair-home"].includes(action)) {
      return !!foundingCommands(action, this.ctx.state, this.homeDynamic());
    }
    return action === "convoy" || action === "scout" || action === "colony" || action === "barge" || action === "probe" || action === "raider"
      ? !!this.fleetOfKind(action) : true;
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
    this.ctx.intent.deselectFleets();
    this.ctx.renderer.pulseSystemBody(String(body.id));
    // Peter (Nov 9 2026): a hull's Build "should not zoom out of the
    // planetary view": the yard's world, its own builder on Ships.
    if (mode === "ships" && this.hooks.openWorldBuild) this.hooks.openWorldBuild(system.id, body.id, { select }, "ships");
    else this.hooks.go({ name: "build", params: { systemId: system.id, systemLabel: system.name }, query: { body: String(body.id), mode, select } });
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

  private fleetOfKind(kind: "convoy" | "scout" | "colony" | "barge" | "probe" | "raider"): GhostView | undefined {
    const wanted = kind === "barge" ? "mining_barge" : kind === "probe" ? "survey_probe" : kind;
    return this.ctx.state.ghosts.find((entry) => entry.own && entry.composition?.some((stack) => (kind === "convoy" ? isPlayerFreighter(stack.kind) : stack.kind === wanted) && stack.count > 0));
  }
}

/** The guide's staffing actions and the structure each opens. */
const STAFF_STRUCTURE: Record<string, string> = {
  mine: "mining_complex", academy: "academy", shipyard: "shipyard", smelter: "smelter", bioharvester: "bioharvester", agroplex: "agroplex",
};

function esc(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!);
}

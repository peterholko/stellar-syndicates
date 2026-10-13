import type { Commodity, FoundingStage, FoundingView, GhostView } from "../../protocol";
import type { BodyView, SystemStateView } from "../../protocol";
import { isPlayerFreighter } from "../../protocol";
import type { ViewState } from "../../state";
import type { FleetCommand } from "../fleetorders";
import { COMMODITY_VALUE, dockedAtSystem, fleetBaseSpeed, guardCapable, mineCapable, surveyCapable, transitSpeedMultiplier, WARP_FACTOR } from "./fleet";
import { fmtDur } from "./format";
import { label } from "../../icons";

/** §charter v3: nine steps in three chapters. Retired stages count as the
 * step that replaced them. */
export const FOUNDING_TOTAL = 9;
export const FOUNDING_STEP: Record<FoundingStage, number> = {
  build_mine: 1, local_recovery: 2, prepare_export: 2, export_production: 2, build_convoy: 2,
  defeat_privateer: 3,
  build_shipyard: 4, complete_export: 4, grow_business: 4, build_second_freighter: 4,
  first_research: 5, build_academy: 5,
  survey_candidates: 6, build_scout: 6,
  build_barge: 7, mine_belt: 8, haul_ore: 9,
  build_colony: 9, establish_colony: 9, complete: 9,
};

/** Next actions, not a class-selection command. Derived solely from served
 * home/research reports; queued builds never pretend to satisfy a prerequisite. */
export function foundingBusinessGoals(st: ViewState, home?: SystemStateView) {
  const built = (kind: string) => home?.bodies.some(b => (b.structures[kind] ?? 0) > 0);
  const staffed = (kind: string) => home?.assignments.some(a => a.structure === kind
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  const enriched = st.research?.programmes.some(p => p.id === "mat_enrichment" && p.state === "completed");
  const freight = !built("shipyard") ? ["build-shipyard", "Build Shipyard"]
    : !staffed("shipyard") ? ["shipyard", "Staff Shipyard"] : ["build-convoy", "Build Tiny Freighter"];
  const refine = !built("academy") && !enriched ? ["build-academy", "Build Academy"]
    : !staffed("academy") && !enriched ? ["academy", "Staff Academy"]
    : !enriched ? ["research-enrichment", "Research Enrichment"]
    : !built("smelter") ? ["build-smelter", "Build Smelter"] : ["smelter", "Staff & supply Smelter"];
  return [
    { title: "Expand exports", copy: "Shipyard → second Freighter", action: freight[0], label: freight[1] },
    { title: "Start refining", copy: "Enrichment → operating Smelter", action: refine[0], label: refine[1] },
  ];
}

/** Landfall's parallel goals from received reports: the fallback card when
 * the plan has no single next step. The guide never sends fleet orders itself.
 * §charter v3: no beacon, cache or Scout refit; null when nothing is left. */
export function foundingLocalGoals(st: ViewState, home?: SystemStateView) {
  const f = st.founding;
  if (!f?.local_opening || !["build_mine", "local_recovery", "prepare_export"].includes(f.stage)) return null;
  const mine = home?.bodies.some(b => (b.structures.mining_complex ?? 0) > 0);
  const staffed = home?.assignments.some(a => a.structure === "mining_complex" && a.workers > 0);
  const goals = [];
  const camp = home?.bodies.find(b => b.founding_camp);
  if (camp && camp.founding_camp!.completed.length < 4) goals.push({ title: "Grow your Founding Camp", copy: "Prospect your world, improve extraction or storage, and prepare housing for arriving workers. Optional projects share your planet's construction queue.", action: "home-development", label: "Develop your settlement" });
  if (!mine || !staffed) goals.push({ title: "Bring the mine online", copy: "Build the Mining Complex and assign its workforce. Ore is your first export.", action: mine ? "mine" : "build-mine", label: mine ? "Staff Mining Complex" : "Build Mining Complex" });
  if (f.privateer && !f.bounty_received) goals.push({ title: "Protect the shipment", copy: "The privateer threatens your Freighter. The Interceptor engages it on its own.", action: "privateer", label: "Inspect Privateer" });
  if (!f.export_dispatched) goals.push({ title: "Send your first shipment", copy: "Choose return supplies and a spending cap. The ship sells, buys what it can afford and returns; you need not wait online.", action: "opening-imports", label: "Plan guarded export & return" });
  return goals.length ? goals : null;
}

/** Refiners may reach exploration without having built the export-route yard. */
export function foundingScoutGoal(home?: SystemStateView) {
  const yard = home?.bodies.find(b => (b.structures.shipyard ?? 0) > 0);
  const staffed = yard && home?.assignments.some(a => a.body_id === yard.id && a.structure === "shipyard"
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  return !yard ? { title: "Prepare for exploration", copy: "Build a Shipyard for your Scout.", action: "build-shipyard", label: "Build Shipyard" }
    : !staffed ? { title: "Prepare for exploration", copy: "Assign workforce to build your Scout.", action: "shipyard", label: "Staff Shipyard" }
    : { title: "Build a Scout", copy: "Discover the resources in nearby systems.", action: "build-scout", label: "Build Scout" };
}

/** §belts: the charted belt the barge lesson points at, with its name, its
 * system's name and position, and the ore worth cutting there. */
export function foundingBelt(st: ViewState): { system: string; body: number; name: string; where: string;
  pos?: { x: number; y: number }; ore?: Commodity } | null {
  const target = st.founding?.belt_target;
  if (!target) return null;
  const system = st.systems.find(s => s.id === target.system);
  const belt = system?.bodies.find(b => b.id === target.body);
  const info = st.galaxy?.systems.find(s => s.id === target.system);
  const ore = [...(belt?.deposits ?? [])]
    .sort((a, b) => b.richness * (COMMODITY_VALUE[b.resource] ?? 0) - a.richness * (COMMODITY_VALUE[a.resource] ?? 0))[0]?.resource;
  return { system: target.system, body: target.body, name: belt?.name ?? "the charted belt",
    where: info?.name ?? "a nearby system", pos: info?.pos, ore };
}

/** §belts: the barge lesson's build step, Shipyard first. */
export function foundingBargeGoal(home: SystemStateView | undefined, belt: string): FoundingStep {
  const yard = home?.bodies.find(b => (b.structures.shipyard ?? 0) > 0);
  const staffed = yard && home?.assignments.some(a => a.body_id === yard.id && a.structure === "shipyard"
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  return !yard ? { title: "Prepare a Shipyard", copy: "Build a Shipyard to make your Mining Barge.", action: "build-shipyard", label: "Build Shipyard" }
    : !staffed ? { title: "Staff the Shipyard", copy: "Assign workforce so the Shipyard can build the barge.", action: "shipyard", label: "Staff Shipyard" }
    : { title: "Build a Mining Barge", copy: `40 Alloys, 20 Machinery, 12 Polymers and 6 Electronics at your Shipyard. It will mine ${belt}, which your survey charted.`,
      reason: "A barge mines a belt in a system you don't own; no colony needed.", action: "build-barge", label: "Build Mining Barge" };
}

// ─── §founding R4/R7: ONE plan for every shell ───────────────────────────────
// The guide shows one next action with its reason, what is already under way
// (with ETAs), and optional work "while you wait". Chapters count real facts
// from received reports, so the numbering never skips a step.

export interface FoundingStep {
  title: string;
  copy: string;
  action: string;
  label: string;
  /** Why this step is next, in a short sentence. */
  reason?: string;
  /** Business choices shown as sibling buttons instead of one action. */
  alternatives?: { title: string; copy: string; action: string; label: string }[];
}

export interface FoundingPlan {
  chapter: { name: "Landfall" | "Industry" | "Reach" | "Founding"; done: number; total: number };
  checklist: { title: string; done: boolean }[];
  /** Things already happening, e.g. "Scout en route to the beacon · ETA 0:41". */
  progress: string[];
  primary: FoundingStep | null;
  waiting: FoundingStep[];
  /** Tone for the primary card: an obstacle the player must clear first. */
  blocked: boolean;
}


function eta(g: GhostView, dest: { x: number; y: number }): string {
  const distance = Math.hypot(dest.x - g.pos.x, dest.y - g.pos.y);
  const moving = Math.hypot(g.vel.x, g.vel.y);
  const cruise = fleetBaseSpeed(g) * WARP_FACTOR * transitSpeedMultiplier(g);
  return fmtDur(distance / Math.max(1, moving > 1 ? Math.max(moving, cruise * 0.5) : cruise));
}

function heading(g: GhostView, dest: { x: number; y: number }, radius = 1_500, st?: ViewState): boolean {
  const last = g.path?.at(-1)?.pos;
  if (last && Math.hypot(last.x - dest.x, last.y - dest.y) <= radius) return true;
  // A confirmed order still in flight counts too, so the guide never offers
  // the same trip twice while its light travels.
  return !!st?.pendingOrders.get(g.id)?.some(o => !o.lost && !!o.dest
    && Math.hypot(o.dest.x - dest.x, o.dest.y - dest.y) <= radius);
}

/** Is this fleet charting `stop` now: dwelling there, or flying there? An
 *  expedition dwell reports survey_progress too, so it never counts. */
function surveyingAt(g: GhostView, stop: { x: number; y: number }, st: ViewState): boolean {
  if (g.expedition) return false;
  if (g.survey_progress != null) return Math.hypot(g.pos.x - stop.x, g.pos.y - stop.y) <= 1_500;
  return heading(g, stop, 1_500, st);
}

const surveyorNoun = (g: GhostView) => g.composition?.some(c => c.kind === "survey_probe" && c.count > 0)
  || g.kind === "survey_probe" ? "Survey Probe" : "Scout";

/** The fleet the guide sends to chart the stop: a free one (a Scout busy on an
 *  expedition stays on it), and the Survey Probe built for it before a Scout. */
function freeSurveyor(own: GhostView[]): GhostView | undefined {
  const free = own.filter(g => surveyCapable(g) && !g.expedition && g.survey_progress == null);
  return free.find(g => surveyorNoun(g) === "Survey Probe") ?? free[0];
}

const aboardUnits = (g: GhostView | undefined) =>
  (g?.cargo_manifest ?? (g?.cargo ? [g.cargo] : [])).reduce((n, c) => n + c.units, 0);

/** Where the guarded export's route run is, in the guide's words. */
function shipmentLine(st: ViewState, freighter: GhostView | undefined, homePos?: { x: number; y: number }): string | null {
  const run = freighter?.industry?.kind === "route" ? freighter.industry.run : null;
  if (!run || !freighter) return null;
  const next = run.route.stops[run.stop];
  const where = next?.port.kind === "hub" ? "the Market Hub" : "home";
  return run.hold ? `Shipment waiting: ${run.hold}` : run.phase === "travelling"
    ? `Shipment heading to ${where}${next?.port.kind === "hub" && st.galaxy ? ` · ETA ${eta(freighter, st.galaxy.hub)}` : homePos ? ` · ETA ${eta(freighter, homePos)}` : ""}`
    : `Shipment ${run.phase} at ${where}`;
}

/** §charter v3: an Industry build step, yard first. */
function foundingIndustryGoal(home: SystemStateView | undefined, structure: "shipyard" | "academy",
  build: FoundingStep): FoundingStep {
  const body = home?.bodies.find(b => (b.structures[structure] ?? 0) > 0);
  const staffed = !!body && !!home?.assignments.some(a => a.body_id === body.id && a.structure === structure
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  if (!body) return build;
  if (!staffed) {
    return { title: structure === "shipyard" ? "Staff the Shipyard" : "Staff the Academy",
      copy: "Assign one workforce. An unstaffed building does nothing.", action: structure, label: "Assign workforce" };
  }
  return build;
}

const LANDFALL: FoundingStage[] = ["build_mine", "local_recovery", "prepare_export", "export_production", "build_convoy", "defeat_privateer"];
const INDUSTRY: FoundingStage[] = ["build_shipyard", "complete_export", "grow_business", "build_second_freighter", "first_research", "build_academy", "survey_candidates", "build_scout"];

/** The founding guide's plan, from served state only. Null when no guide. */
export function foundingPlan(st: ViewState, home?: SystemStateView): FoundingPlan | null {
  const f = st.founding;
  if (!f || !f.local_opening || f.stage === "complete") return null;
  const own = st.ghosts.filter(g => g.own);
  const freighter = own.find(g => g.composition?.some(c => isPlayerFreighter(c.kind) && c.count > 0) ?? isPlayerFreighter(g.kind));
  const built = (kind: string) => home?.bodies.some(b => (b.structures[kind] ?? 0) > 0) ?? false;
  const queued = (kind: string) => home?.builds.some(j => j.key === kind) ?? false;
  const staffed = (kind: string) => home?.assignments.some(a => a.structure === kind
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0))) ?? false;
  const atHome = (g?: GhostView) => !!g && !!home && dockedAtSystem(g, home.id);
  const storageFull = home ? home.storage_used >= home.storage_cap - 1 : false;
  const routeHold = freighter?.industry?.kind === "route" ? freighter.industry.run.hold ?? null : null;
  const homePos = st.galaxy?.systems.find(s => s.id === home?.id)?.pos;
  const step = FOUNDING_STEP[f.stage];
  const progress: string[] = [];
  const waiting: FoundingStep[] = [];
  let primary: FoundingStep | null = null;
  let blocked = false;
  const camp = home?.bodies.find(b => b.founding_camp);
  const settlement: FoundingStep = { title: "Develop your settlement", copy: "Prospect your world (free), then improve extraction, storage or housing.",
    action: "home-development", label: "Settlement projects" };
  const chapter = (name: FoundingPlan["chapter"]["name"], checklist: FoundingPlan["checklist"]): FoundingPlan => {
    if (camp && camp.founding_camp!.completed.length < 4 && primary?.action !== "home-development") waiting.push(settlement);
    return { chapter: { name, done: checklist.filter(s => s.done).length, total: checklist.length }, checklist, progress, primary, waiting, blocked };
  };

  // ─── Landfall: earn your first credits ─────────────────────────────────────
  if (LANDFALL.includes(f.stage)) {
    const mineOnline = built("mining_complex") && staffed("mining_complex");
    const checklist = [
      { title: "Bring the mine online", done: step > 1 || mineOnline },
      { title: "Send your first shipment", done: !!f.export_dispatched },
      { title: "Defeat the Rogue Privateer", done: f.bounty_received },
    ];
    if (queued("mining_complex")) progress.push("Mining Complex under construction");
    const shipment = f.export_dispatched ? shipmentLine(st, freighter, homePos) : null;
    if (shipment) progress.push(shipment);
    const privateerActive = !!f.privateer && !f.bounty_received;
    if (privateerActive) progress.push("Rogue Privateer closing · your escort engages it automatically");
    if (!freighter && !f.export_dispatched) {
      blocked = true;
      primary = { title: "Freighter lost", copy: f.freighter_replaced
        ? "Your replacement Freighter was lost too. Build a Shipyard to make another, or import one."
        : "The Authority is chartering a replacement at home.", action: "home-development", label: "Open home" };
    } else if (!built("mining_complex") && !queued("mining_complex")) {
      primary = { title: "Build the Mining Complex", copy: "25 Alloys and 12 Machinery from your starting kit; it builds in 18 seconds.",
        reason: "Ore is your first export.", action: "build-mine", label: "Build Mining Complex" };
    } else if (built("mining_complex") && !staffed("mining_complex")) {
      primary = { title: "Staff the Mining Complex", copy: "Assign one workforce. Food output dips while the crews share the work.",
        reason: "An unstaffed mine produces nothing.", action: "mine", label: "Assign workforce" };
    } else if (!f.export_dispatched && (routeHold?.startsWith("Storage full") || (storageFull && !atHome(freighter)))) {
      blocked = true;
      primary = { title: "Make room at home", copy: routeHold ?? `Storage is full (${home?.storage_used}/${home?.storage_cap}).`,
        reason: "Expand the storehouse, or ship goods out.", action: "home-development", label: "Open settlement" };
    } else if (!f.export_dispatched) {
      primary = { title: "Send your first shipment", copy: "Load Ferrite Ore and send the Freighter to the Market Hub with the Interceptor guarding it. It sells the ore and brings Shipyard materials home.",
        reason: mineOnline ? "Ore is worth money only at the Market Hub." : "The mine's ore is your cargo.",
        action: "opening-imports", label: "Plan the shipment" };
    } else if (privateerActive) {
      primary = { title: "Protect the shipment", copy: "A Rogue Privateer is closing on your Freighter. The Interceptor engages it on its own.",
        reason: "Its weapons are weak; the bounty is 4,500 credits.", action: "privateer", label: "Watch the fight" };
    } else if (!f.bounty_received) {
      primary = { title: "Shipment under way", copy: "A Rogue Privateer is reported on the Hub lane, and your escort will meet it. You need not wait online.",
        action: "shipment-trip", label: "Follow the trip" };
    }
    return chapter("Landfall", checklist);
  }

  // ─── Industry: build your base ─────────────────────────────────────────────
  if (INDUSTRY.includes(f.stage)) {
    const checklist = [
      { title: "Build a Shipyard", done: step > 4 },
      { title: "Start research", done: step > 5 },
      { title: "Chart a nearby system", done: false },
    ];
    const stop = f.survey_candidates[0];
    const stopName = st.galaxy?.systems.find(s => s.id === stop)?.name ?? "the nearby stop";
    for (const [key, name] of [["shipyard", "Shipyard"], ["academy", "Academy"], ["survey_probe", "Survey Probe"]] as const) {
      if (queued(key)) progress.push(`${name} under construction`);
    }
    if (step <= 4) {
      primary = queued("shipyard") ? { title: "Shipyard under construction", copy: "Staff it when it is finished.", action: "home-development", label: "Open home" }
        : foundingIndustryGoal(home, "shipyard", { title: "Build a Shipyard", copy: "40 Alloys, 20 Machinery and 15 Electronics. Your shipment brings part of the kit; book the rest home on Authority freight from the Market.",
          reason: "It builds every ship that comes next.", action: "build-shipyard", label: "Build Shipyard" });
      if (!built("shipyard")) waiting.push({ title: "Import the rest of the kit", copy: "Buy at the Market and book Authority freight home.", action: "market", label: "Open Market" });
    } else if (step === 5) {
      const academy = foundingIndustryGoal(home, "academy", { title: "Build an Academy", copy: "25 Alloys, 15 Electronics and 20 Provisions. Research unlocks better extraction, hulls and refining.",
        action: "build-academy", label: "Build Academy" });
      primary = queued("academy") ? { title: "Academy under construction", copy: "Staff it when it is finished, then choose a programme.", action: "home-development", label: "Open home" }
        : built("academy") && staffed("academy") ? { title: "Choose your first programme", copy: "Any Tier I programme. The Authority funds most of the first one, and it runs in the background.",
          action: "research", label: "Open Research" }
        : academy;
    } else {
      const stopPos = st.galaxy?.systems.find(s => s.id === stop)?.pos;
      const active = stopPos ? own.find(g => surveyCapable(g) && surveyingAt(g, stopPos, st)) : undefined;
      const surveyor = freeSurveyor(own);
      if (active && stopPos) {
        const noun = surveyorNoun(active);
        progress.push(active.survey_progress != null
          ? `Surveying ${stopName} · ${Math.round(active.survey_progress * 100)}%`
          : `${noun} en route to ${stopName} · ETA ${eta(active, stopPos)}`);
        primary = { title: "The survey is under way", copy: `Its report charts ${stopName}'s worlds and any belts, and comes home at light speed.`,
          action: noun === "Scout" ? "scout" : "probe", label: `Select the ${noun === "Scout" ? "Scout" : "probe"}` };
      } else if (surveyor) {
        primary = { title: `Chart ${stopName}`, copy: `Send your ${surveyorNoun(surveyor)} to survey ${stopName}. A probe is spent when its report is sent.`,
          reason: "Only a survey reveals what a system holds.", action: "survey-stop", label: "Send the survey" };
      } else if (!queued("survey_probe")) {
        primary = { title: "Build a Survey Probe", copy: `6 Alloys and 4 Electronics at your Shipyard. It flies to ${stopName}, charts it and is spent.`,
          reason: "The cheapest way to learn what a system holds.", action: "build-probe", label: "Build Survey Probe" };
      } else {
        primary = { title: "Survey Probe under construction", copy: `When it launches, send it to ${stopName}.`, action: "shipyard", label: "Open Shipyard" };
      }
    }
    return chapter("Industry", checklist);
  }

  // ─── Reach: grow without a colony ──────────────────────────────────────────
  const checklist = [
    { title: "Build a Mining Barge", done: step > 7 },
    { title: "Mine the belt", done: step > 8 },
    { title: "Bring the ore home", done: false },
  ];
  const belt = foundingBelt(st);
  const beltName = belt ? `${belt.name}${belt.system === home?.id ? "" : ` in ${belt.where}`}` : "the charted belt";
  const barge = own.find(g => mineCapable(g));
  const job = barge?.industry?.kind === "mine" ? barge.industry : null;
  const aboard = aboardUnits(barge);
  if (f.stage === "build_barge" || !barge) {
    if (queued("mining_barge")) {
      progress.push("Mining Barge under construction");
      primary = { title: "Mining Barge under construction", copy: `When it launches, send it to ${beltName}.`, action: "shipyard", label: "Open Shipyard" };
    } else {
      primary = foundingBargeGoal(home, beltName);
      if (f.stage !== "build_barge") primary = { ...primary, title: "Replace the Mining Barge", reason: "Your barge was lost." };
      // By now the food reserve and ore fill home storage; the kit needs room to land.
      if (storageFull && primary.action === "build-barge") {
        primary = { ...primary, reason: `Home storage is full (${Math.floor(home!.storage_used)}/${home!.storage_cap}). Ship goods out or add storage so the barge's kit can land.` };
      }
    }
  } else if (f.stage === "mine_belt") {
    const bound = !!belt?.pos && (job?.status === "travelling" || (!job && heading(barge, belt.pos, 1_500, st)));
    if (job && job.status !== "travelling") {
      progress.push(`Mining Barge ${job.status === "mining" ? `cutting ${label(job.ore)}` : job.status} at ${beltName} · its report is on the way to command`);
      primary = { title: "The barge is at work", copy: "This step completes when its report reaches command.", action: "barge", label: "Select Mining Barge" };
    } else if (bound && belt?.pos) {
      progress.push(`Mining Barge en route to ${beltName} · ETA ${eta(barge, belt.pos)}`);
      primary = { title: "The barge is on its way", copy: `It starts cutting ore when it reaches ${beltName}.`, action: "barge", label: "Select Mining Barge" };
    } else {
      primary = { title: "Send the barge to the belt", copy: `Your survey charted ${beltName}. The barge cuts ${belt?.ore ? label(belt.ore) : "ore"} into its own hold.`,
        reason: "Command hears when it starts cutting.", action: "mine-belt", label: "Send Mining Barge" };
    }
  } else {
    // haul_ore: home, unload.
    const homeBound = !!homePos && !atHome(barge) && !job && heading(barge, homePos, 2_500, st);
    if (atHome(barge) && aboard > 0) {
      primary = { title: "Unload the ore", copy: `Store the barge's ${aboard} units in your home stockpile, for the Smelter or the Market.`,
        action: "barge-unload", label: "Unload ore" };
    } else if (homeBound && homePos) {
      progress.push(`Mining Barge bringing ${aboard} units home · ETA ${eta(barge, homePos)}`);
      primary = { title: "The barge is heading home", copy: "Unload it when it docks.", action: "barge", label: "Select Mining Barge" };
    } else if (job) {
      progress.push(`Mining Barge ${job.status === "full" ? "hold full" : `cutting ${label(job.ore)}`} at ${beltName} · ${aboard} units aboard`);
      primary = { title: "Bring the ore home", copy: job.status === "full" ? "The hold is full. Send the barge home and unload it." : "Send the barge home with what it has cut, or let it fill its hold first.",
        reason: "Ore earns nothing until it is unloaded or sold.", action: "barge-home", label: "Send the barge home" };
    } else {
      primary = { title: "Bring the ore home", copy: "Send the barge home and unload its ore, or sell it at the Market Hub.", action: "barge-home", label: "Send the barge home" };
    }
  }
  return chapter("Reach", checklist);
}

/** §founding R3/R4: the guide's one-click orders. Each still goes through the
 * ordinary Confirm/Cancel preview; nothing is sent from here. */
export function foundingCommands(action: string, st: ViewState, home?: SystemStateView): FleetCommand[] | null {
  const f = st.founding;
  if (!f) return null;
  const own = st.ghosts.filter(g => g.own);
  const freighter = own.find(g => g.composition?.some(c => isPlayerFreighter(c.kind) && c.count > 0) ?? isPlayerFreighter(g.kind));
  const scout = own.find(g => g.composition?.some(c => c.kind === "scout" && c.count > 0) ?? g.kind === "scout");
  const escort = own.find(g => guardCapable(g));
  const homePos = st.galaxy?.systems.find(s => s.id === home?.id)?.pos;
  const site = st.explorationSites.find(s => s.id === f.recovery_site);
  switch (action) {
    case "guarded-recovery": {
      if (!freighter || !site?.details) return null;
      const commands: FleetCommand[] = [{ type: "MoveShip", ship_id: freighter.id, dest: site.pos }];
      if (escort && !own.some(g => g.guard_target === freighter.id)) {
        commands.push({ type: "GuardFleet", interceptor_id: escort.id, target_id: freighter.id });
      }
      return commands;
    }
    case "cache-home": return freighter && homePos ? [{ type: "MoveShip", ship_id: freighter.id, dest: homePos }] : null;
    case "cache-unload": return freighter && home ? [{ type: "SystemUnload", fleet_id: freighter.id, system: home.id }] : null;
    case "scout-home": return scout && homePos ? [{ type: "MoveShip", ship_id: scout.id, dest: homePos }] : null;
    case "mine-belt": {
      const barge = own.find(g => mineCapable(g));
      const belt = foundingBelt(st);
      return barge && belt?.ore
        ? [{ type: "MineBelt", fleet_id: barge.id, system_id: belt.system, body_id: belt.body, ore: belt.ore }] : null;
    }
    // §charter v3: the probe survey and the barge's trip home.
    case "survey-stop": {
      const surveyor = freeSurveyor(own);
      const stop = f.survey_candidates[0];
      return surveyor && stop ? [{ type: "SurveySystem", fleet_id: surveyor.id, system_id: stop }] : null;
    }
    case "barge-home": {
      const barge = own.find(g => mineCapable(g));
      return barge && homePos ? [{ type: "MoveShip", ship_id: barge.id, dest: homePos }] : null;
    }
    case "barge-unload": {
      const barge = own.find(g => mineCapable(g));
      return barge && home && dockedAtSystem(barge, home.id) ? [{ type: "SystemUnload", fleet_id: barge.id, system: home.id }] : null;
    }
    default: return null;
  }
}

/** The home body whose reported Ferrite Ore is richest: where the mine goes. */
export function bestMineBody(system: SystemStateView): BodyView | undefined {
  return [...system.bodies].sort((a, b) => {
    const ar = a.deposits?.filter((entry) => entry.resource === "metallic_ore").reduce((sum, entry) => sum + entry.richness, 0) ?? 0;
    const br = b.deposits?.filter((entry) => entry.resource === "metallic_ore").reduce((sum, entry) => sum + entry.richness, 0) ?? 0;
    return br - ar || (b.resource_slots ?? 0) - (a.resource_slots ?? 0) || a.id - b.id;
  })[0];
}

/** Hours-and-minutes for long clocks ("23h 40m"; under an hour, "12m"). */
export function fmtHours(secs: number): string {
  const minutes = Math.max(0, Math.floor(secs / 60));
  const hours = Math.floor(minutes / 60);
  return hours > 0 ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
}

/** §founding R9: the founder shield as players read it — how long it lasts and
 * what it covers. The 30-minute minimum is only when YOU may end it early. */
export function shieldStatus(f: FoundingView, now: number): { label: string; detail: string } {
  if (!f.protected) {
    return { label: "shield ended", detail: "Founder protection has ended. Other corporations can attack you, and you them." };
  }
  const left = fmtHours(f.protection_max_until - now);
  const early = now >= f.protection_min_until && f.bounty_received
    ? "It ends early if you attack another corporation."
    : "Pirates can still attack your ships.";
  return { label: `shield · ${left}`, detail: `Founder shield: no corporation can attack you, and you can't attack them, for ${left}. ${early}` };
}

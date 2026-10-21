import type { CaptainRosterView, Commodity, FoundingStage, FoundingView, GhostView } from "../../protocol";
import type { BodyView, SystemStateView } from "../../protocol";
import { isPlayerFreighter } from "../../protocol";
import type { ViewState } from "../../state";
import type { FleetCommand } from "../fleetorders";
import { COMMODITY_VALUE, dockedAtSystem, fleetBaseSpeed, guardCapable, mineCapable, surveyCapable, transitSpeedMultiplier, WARP_FACTOR } from "./fleet";
import { fmtDur } from "./format";
import { bodyPoolUsage, MINERAL_DEPOSITS } from "./market";
import { dockedMerchant, merchantLeavesIn, merchantShortfall, type MerchantTarget } from "./merchant";
import { label } from "../../icons";

/** §charter-v5: eighteen steps in five chapters. Retired stages count as the
 * step that replaced them; a finished charter counts as the last. */
export const FOUNDING_TOTAL = 18;
export const FOUNDING_STEP: Record<FoundingStage, number> = {
  build_bioharvester: 1, build_habitat: 2, build_agroplex: 3, build_mine: 4,
  pirate_raid: 5, sell_ore: 6,
  power_shipyard: 7, buy_semiconductors: 7,
  build_shipyard: 8, complete_export: 8, grow_business: 8, build_second_freighter: 8,
  build_interceptor: 9, build_freighter: 9, build_convoy: 9,
  defeat_privateer: 10, eliminate_den: 11,
  prepare_export: 12, local_recovery: 12, export_production: 12,
  first_research: 13, build_academy: 13,
  expand_mining: 14,
  survey_candidates: 15, build_scout: 15,
  build_barge: 16, mine_belt: 17, haul_ore: 18,
  build_colony: 18, establish_colony: 18, complete: 18,
};

export type FoundingChapter = "Landfall" | "Raid" | "Pirates" | "Expansion" | "Reach";
/** The guide's checklist in the server's words (tutorial::Step::label). */
export const FOUNDING_CHAPTERS: [FoundingChapter, [FoundingStage, string][]][] = [
  ["Landfall", [["build_bioharvester", "Harvest biomass"], ["build_habitat", "House the settlers"],
    ["build_agroplex", "Make provisions"], ["build_mine", "Bring the mine online"]]],
  ["Raid", [["pirate_raid", "Pirates raid the colony"], ["sell_ore", "Sell ore to the trader"],
    ["power_shipyard", "Power the Shipyard"], ["build_shipyard", "Build a Shipyard"]]],
  ["Pirates", [["build_interceptor", "Build an Interceptor"], ["defeat_privateer", "Destroy the privateer"],
    ["eliminate_den", "Eliminate the privateer den"]]],
  ["Expansion", [["prepare_export", "Export your ore"], ["first_research", "Start research"],
    ["expand_mining", "Expand mining operations"], ["survey_candidates", "Chart a nearby system"]]],
  ["Reach", [["build_barge", "Build a Mining Barge"], ["mine_belt", "Mine the belt"], ["haul_ore", "Bring the ore home"]]],
];

/** §no-spoilers (Oct 15 2026): a twist is news only once it is told — the
 * raid once command sees it, the trader once its pop-up has shown. Until
 * then the guide keeps back its step, even when it is the current one, and
 * calls its chapter by a plain name; a step already passed always shows. */
const TWIST: Partial<Record<FoundingStage, (f: FoundingView, st: ViewState) => boolean>> = {
  pirate_raid: f => !!f.raid_seen,
  sell_ore: (_, st) => traderTold(st),
};
const CHAPTER_TOLD: Partial<Record<FoundingChapter, (f: FoundingView) => boolean>> = { Raid: f => !!f.raid_seen };
const COVER_NAME: Partial<Record<FoundingChapter, string>> = { Raid: "Industry" };

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
  if (!f?.local_opening || !["local_recovery", "prepare_export"].includes(f.stage)) return null;
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

/** §charter-v4: the privateer den its log named, once the kill report is in. */
export function foundingDen(st: ViewState): { system: string; name: string; pos: { x: number; y: number } } | null {
  const id = st.founding?.den;
  const info = id ? st.galaxy?.systems.find(s => s.id === id) : undefined;
  return id && info ? { system: id, name: info.name, pos: info.pos } : null;
}

/** §charter-v4: a captain in reserve where this fleet is docked, free to
 * take command of it. None when the fleet already has one. */
export function reserveCaptain(st: ViewState, fleet: GhostView): CaptainRosterView | undefined {
  if (fleet.captain || st.captains.some(c => c.assigned_fleet === fleet.id)) return undefined;
  return st.captains.find(c => c.assigned_fleet == null && c.loss_fate == null
    && (c.recovering_until == null || c.recovering_until <= st.simTime)
    && !!c.stationed_system && dockedAtSystem(fleet, c.stationed_system));
}

/** §belts: the barge lesson's build step, Shipyard first. */
export function foundingBargeGoal(home: SystemStateView | undefined, belt: string): FoundingStep {
  const yard = home?.bodies.find(b => (b.structures.shipyard ?? 0) > 0);
  const staffed = yard && home?.assignments.some(a => a.body_id === yard.id && a.structure === "shipyard"
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  return !yard ? { title: "Prepare a Shipyard", copy: "Build a Shipyard to make your Mining Barge.", action: "build-shipyard", label: "Build Shipyard" }
    : !staffed ? { title: "Staff the Shipyard", copy: staffCopy(home, "Assign workforce so the Shipyard can build the barge."), action: "shipyard", label: "Staff Shipyard" }
    : { title: "Build a Mining Barge", copy: `40 Alloys, 20 Machinery, 12 Polymers and 6 Semiconductors at your Shipyard. It will mine ${belt}, which your survey charted.`,
      reason: "A barge mines a belt in a system you don't own; no colony needed.", action: "build-barge", label: "Build Mining Barge" };
}

/** §charter-v4: the free trader has been told — its pop-up shown, or its
 * docking news met in an earlier session. Nothing names it before. */
export const traderTold = (st: ViewState): boolean => !!st.storyTold?.has("free_trader");

/** §charter-v5: Deuterium the Interceptor's tank still takes while it sits
 * docked at home, where the trader is: the guide's "Buy Deuterium" prefill. */
export function fuelRoom(st: ViewState): number {
  const home = st.merchant?.system;
  const hunter = home ? st.ghosts.find(g => g.own && guardCapable(g) && dockedAtSystem(g, home)) : undefined;
  return hunter?.fuel_capacity != null && hunter.fuel != null ? Math.max(0, hunter.fuel_capacity - hunter.fuel) : 0;
}

/** §merchant: while the free trader is docked, a build the home is short of
 * offers it as the first choice, the shortfall filled in; the ordinary
 * build step stays beside it. */
export function viaMerchant(st: ViewState, home: SystemStateView | undefined, target: MerchantTarget,
  name: string, step: FoundingStep): FoundingStep {
  const merchant = traderTold(st) ? dockedMerchant(st) : null;
  const short = merchant && home ? merchantShortfall(st, home, target, merchant) : {};
  const goods = Object.entries(short).map(([c, n]) => `${n} ${label(c)}`).join(", ");
  if (!goods) return step;
  const trader = { title: `Buy the ${name} parts`, copy: `Free trader: ${goods}.`,
    action: `merchant-${target}`, label: "Buy from trader" };
  return { ...trader, reason: step.reason, alternatives: [trader, { title: step.title, copy: step.copy, action: step.action, label: step.label }] };
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
  chapter: { name: string; done: number; total: number };
  checklist: { title: string; done: boolean }[];
  /** Things already happening, e.g. "Solar Array under construction · 0:21". */
  progress: string[];
  primary: FoundingStep | null;
  waiting: FoundingStep[];
  /** §charter-v5: a goal beyond the next step: once the den falls, the rich
   * belt it guarded, until the survey step takes it up. */
  goal: FoundingStep | null;
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

/** §charter-v5: the first shipment's trip, while it runs. */
function shipmentLine(st: ViewState, freighter: GhostView | undefined, homePos?: { x: number; y: number }): string | null {
  const run = freighter?.industry?.kind === "route" ? freighter.industry.run : null;
  if (!run || !freighter || run.phase === "complete") return null;
  const next = run.route.stops[run.stop];
  const where = next?.port.kind === "hub" ? "the Market Hub" : "home";
  const dest = next?.port.kind === "hub" ? st.galaxy?.hub : homePos;
  return run.hold ? `Shipment waiting: ${run.hold}` : run.phase === "travelling"
    ? `Shipment heading to ${where}${dest ? ` · ETA ${eta(freighter, dest)}` : ""}`
    : `Shipment ${run.phase} at ${where}`;
}

const aboardUnits = (g: GhostView | undefined) =>
  (g?.cargo_manifest ?? (g?.cargo ? [g.cargo] : [])).reduce((n, c) => n + c.units, 0);

/** An Industry build step, yard first. */
/** §workforce cap: a staffing step's copy — or, when every worker already has
 * a post, where to find one (the + is disabled). */
function staffCopy(home: SystemStateView | undefined, copy: string): string {
  const workforce = home?.workforce;
  return workforce && workforce.units - workforce.posted <= 0 ? "No free workforce: take one off another building." : copy;
}

function foundingIndustryGoal(home: SystemStateView | undefined, structure: "shipyard" | "academy",
  build: FoundingStep): FoundingStep {
  const body = home?.bodies.find(b => (b.structures[structure] ?? 0) > 0);
  const staffed = !!body && !!home?.assignments.some(a => a.body_id === body.id && a.structure === structure
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  if (!body) return build;
  if (!staffed) {
    return { title: structure === "shipyard" ? "Staff the Shipyard" : "Staff the Academy",
      copy: staffCopy(home, "Assign one workforce. An unstaffed building does nothing."), action: structure, label: "Assign workforce" };
  }
  return build;
}

/** A build's recipe from the served catalogue: "25 Alloys · 12 Machinery". */
export function recipeLine(st: ViewState, key: string): string {
  return (st.galaxy?.build_options?.find(o => o.key === key)?.costs ?? [])
    .map(c => `${c.units} ${label(c.commodity)}`).join(" · ");
}

/** §charter-v5 (Peter, Oct 16 2026): once the den falls, the long-term goal
 * is the belt it guarded, rich in minerals by the look of it: a Survey Probe
 * charts it. Done early, the survey step (15) completes as it is reached.
 * None for a home without a den, or once the belt is charted. */
function denBeltGoal(st: ViewState, own: GhostView[], progress: string[], home?: SystemStateView): FoundingStep | null {
  const den = foundingDen(st);
  if (!den || st.founding?.survey_candidates[0] !== den.system) return null;
  if (st.systems.find(s => s.id === den.system)?.bodies.some(b => b.geology !== null)) return null;
  const active = own.find(g => surveyCapable(g) && surveyingAt(g, den.pos, st));
  if (active) {
    progress.push(active.survey_progress != null ? `Surveying ${den.name} · ${Math.round(active.survey_progress * 100)}%`
      : `${surveyorNoun(active)} en route to ${den.name} · ETA ${eta(active, den.pos)}`);
    return null;
  }
  if (home?.builds.some(j => j.key === "survey_probe")) return null;
  const goal = { title: `Survey the belt at ${den.name}`, copy: "The den guarded a belt that looks rich in minerals." };
  return freeSurveyor(own) ? { ...goal, action: "survey-stop", label: "Send the survey" }
    : { ...goal, copy: `${goal.copy} A Survey Probe charts it.`, action: "build-probe", label: "Build Survey Probe" };
}

/** §charter-v4: the landing world, where the Command Center stands. */
export const landingBody = (home?: SystemStateView): BodyView | undefined =>
  home?.bodies.find(b => (b.structures.command_center ?? 0) > 0);

/** §charter-v4: the world "Expand mining operations" builds on: another world
 * of the home system with ore and a free resource site, one already started
 * first, a planet before a belt (the bots' rule too). */
export function expansionBody(home: SystemStateView): BodyView | undefined {
  const ore = (b: BodyView) => b.deposits?.some(d => MINERAL_DEPOSITS.includes(d.resource)) ?? false;
  const free = (b: BodyView) => { const p = bodyPoolUsage(b, home).resource; return p.used < p.total; };
  const level = (b: BodyView, kind: string) => b.structures[kind] ?? 0;
  return home.bodies.filter(b => !level(b, "command_center") && ore(b) && (level(b, "mining_complex") > 0 || free(b)))
    .sort((a, b) => level(b, "mining_complex") - level(a, "mining_complex") || level(b, "power_plant") - level(a, "power_plant")
      || Number(a.kind === "asteroid_belt") - Number(b.kind === "asteroid_belt") || a.id - b.id)[0];
}

/** The founding guide's plan, from served state only. Null when no guide. */
export function foundingPlan(st: ViewState, home?: SystemStateView): FoundingPlan | null {
  const f = st.founding;
  if (!f || !f.local_opening || f.stage === "complete") return null;
  const own = st.ghosts.filter(g => g.own);
  const freighter = own.find(g => g.composition?.some(c => isPlayerFreighter(c.kind) && c.count > 0) ?? isPlayerFreighter(g.kind));
  const routeHold = freighter?.industry?.kind === "route" ? freighter.industry.run.hold ?? null : null;
  const level = (kind: string) => Math.max(0, ...(home?.bodies ?? []).map(b => b.structures[kind] ?? 0));
  const built = (kind: string) => level(kind) > 0;
  const job = (kind: string, body?: number) => home?.builds.find(j => j.key === kind && (body === undefined || j.body_id === body));
  const queued = (kind: string) => !!job(kind);
  const staffed = (kind: string, body?: number) => home?.assignments.some(a => a.structure === kind
    && (body === undefined || a.body_id === body)
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0))) ?? false;
  const atHome = (g?: GhostView) => !!g && !!home && dockedAtSystem(g, home.id);
  const storageFull = home ? home.storage_used >= home.storage_cap - 1 : false;
  const homePos = st.galaxy?.systems.find(s => s.id === home?.id)?.pos;
  const landing = landingBody(home);
  const world = landing?.name ?? "your landing world";
  const kit = (key: string) => recipeLine(st, key);
  const step = FOUNDING_STEP[f.stage];
  const progress: string[] = [];
  const waiting: FoundingStep[] = [];
  let primary: FoundingStep | null = null;
  let blocked = false;
  const camp = home?.bodies.find(b => b.founding_camp);
  const settlement: FoundingStep = { title: "Develop your settlement", copy: "Prospect your world (free), then improve extraction, storage or housing.",
    action: "home-development", label: "Settlement projects" };
  /** A build under way: its progress line, and a card to wait on. */
  const building = (kind: string, name: string, then: string, body?: number): FoundingStep | null => {
    const j = job(kind, body);
    if (!j) return null;
    const left = !j.queued && j.complete_time != null ? Math.max(0, j.complete_time - st.simTime) : null;
    progress.push(`${name} under construction${j.queued ? " · queued" : left != null ? ` · ${fmtDur(left)}` : ""}`);
    return { title: `${name} under construction`, copy: then, action: "home-world", label: `Open ${body !== undefined && body !== landing?.id ? "world" : world}` };
  };
  /** Build a structure, then staff it when it needs a crew. */
  const structure = (kind: string, name: string, build: FoundingStep, crew: boolean): FoundingStep => {
    if (!built(kind)) return building(kind, name, crew ? "Staff it when it is finished." : "It stands when it is finished.") ?? build;
    if (crew && !staffed(kind)) {
      return { title: `Staff the ${name}`, copy: staffCopy(home, "Assign one workforce. An unstaffed building does nothing."),
        reason: `${name} stands on ${world}.`, action: STAFF_ACTION[kind] ?? kind, label: "Assign workforce" };
    }
    return build;
  };
  const chapterOf = FOUNDING_CHAPTERS.find(([, steps]) => steps.some(([s]) => FOUNDING_STEP[s] === step))
    ?? FOUNDING_CHAPTERS[FOUNDING_CHAPTERS.length - 1];
  const told = ([s]: [FoundingStage, string]) => step > FOUNDING_STEP[s] || (TWIST[s]?.(f, st) ?? true);
  const checklist = chapterOf[1].filter(told).map(([s, title]) => ({ title, done: step > FOUNDING_STEP[s] }));
  const chapterName = CHAPTER_TOLD[chapterOf[0]]?.(f) ?? true ? chapterOf[0] : COVER_NAME[chapterOf[0]] ?? chapterOf[0];

  switch (step) {
    // ─── Landfall: the colony ship lands and becomes the Command Center ───────
    case 1:
      primary = structure("bioharvester", "Bioharvester", { title: "Build a Bioharvester", copy: `${kit("bioharvester")}, from the charter kit. It harvests Biomass.`,
        reason: "Your settlers live on the kit's Provisions.", action: "build-bioharvester", label: "Build Bioharvester" }, true);
      break;
    case 2:
      primary = structure("habitat", "Habitat", { title: "Build a Habitat", copy: `${kit("habitat")}. Your settlers move out of the colony ship.`,
        reason: "They live aboard until it stands.", action: "build-habitat", label: "Build Habitat" }, false);
      break;
    case 3:
      primary = structure("agroplex", "Agroplex", { title: "Build an Agroplex", copy: `${kit("agroplex")}. It turns Biomass into Provisions.`,
        reason: "The kit's Provisions won't last.", action: "build-agroplex", label: "Build Agroplex" }, true);
      break;
    case 4:
      primary = structure("mining_complex", "Mining Complex", { title: "Build a Mining Complex", copy: `${kit("mining_complex")}, on ${world}'s Ferrite Ore. The Command Center's 1 MW reactor runs it.`,
        reason: "Ore is your first export.", action: "build-mine", label: "Build Mining Complex" }, true);
      break;

    // ─── Raid: the privateer takes the kit's semiconductors and Armaments (the
    // story is a pop-up, story-events.ts; the guide keeps to what happens) ────
    case 5: {
      const pirate = f.privateer ? st.ghosts.find(g => g.id === f.privateer) : undefined;
      const over = !!pirate && !!homePos && Math.hypot(pirate.pos.x - homePos.x, pirate.pos.y - homePos.y) <= 1_200;
      // Not news until command sees it: the mine's own moment.
      if (!f.raid_seen) {
        primary = { title: "The mine is running", copy: "Ferrite Ore is coming in.", action: "mine", label: "Open the mine" };
      } else if (over) {
        blocked = true;
        progress.push("Raider over the colony");
        primary = { title: "Raider over the colony", copy: "It's stripping the stockpile.", action: "privateer", label: "Watch it" };
      } else {
        blocked = true;
        const homeName = st.galaxy?.systems.find(s => s.id === home?.id)?.name ?? "the colony";
        if (pirate && homePos) progress.push(`Raider closing on ${homeName} · ETA ${eta(pirate, homePos)}`);
        primary = { title: "Raider inbound", copy: "No ships, no defenses.", action: "privateer", label: "Watch it" };
      }
      break;
    }
    case 6: {
      const need = st.galaxy?.build_options?.find(o => o.key === "power_plant")?.costs.find(c => c.commodity === "electronics")?.units ?? 0;
      // The trader is news only once its pop-up has told it (after the
      // server says it docked): until then the guide never names it.
      primary = traderTold(st) && dockedMerchant(st)
        ? { title: "Sell ore to the trader", copy: "We'd better sell ore to the trader to buy semiconductors for our ships.",
          action: "merchant-ore", label: "Sell ore" }
        : { title: "Find semiconductors", copy: `The Solar Array needs ${need}.`, action: "market", label: "Open Market" };
      break;
    }
    case 7:
      primary = building("power_plant", "Solar Array", "The Shipyard comes next.", landing?.id)
        ?? viaMerchant(st, home, "power_plant", "Solar Array", { title: "Build a Solar Array", copy: `${kit("power_plant")}. +8 MW on ${world}.`,
          reason: "A Shipyard needs more than the Command Center's 1 MW.", action: "build-power", label: "Build Solar Array" });
      break;
    case 8:
      primary = structure("shipyard", "Shipyard", viaMerchant(st, home, "shipyard", "Shipyard", { title: "Build a Shipyard", copy: `${kit("shipyard")}.`,
        reason: "It builds every ship that comes next.", action: "build-shipyard", label: "Build Shipyard" }), true);
      break;

    // ─── Pirates: the Interceptor, the privateer and its den ───────────────────
    case 9: {
      const story = "The privateer lurks nearby.";
      if (level("shipyard") < 2) {
        primary = building("shipyard", "Shipyard II", "Then build the Interceptor.")
          ?? viaMerchant(st, home, "shipyard", "Shipyard II", { title: "Upgrade to Shipyard II", copy: `${kit("shipyard")}. Interceptors need a level II yard.`,
            reason: story, action: "build-shipyard-2", label: "Upgrade Shipyard" });
      } else {
        primary = building("raider", "Interceptor", "Then hunt the privateer.")
          ?? foundingIndustryGoal(home, "shipyard", viaMerchant(st, home, "raider", "Interceptor", { title: "Build an Interceptor", copy: `${kit("raider")}.`,
            reason: story, action: "build-interceptor", label: "Build Interceptor" }));
      }
      break;
    }
    case 10: {
      const hunter = own.find(g => guardCapable(g));
      const pirate = f.privateer ? st.ghosts.find(g => g.id === f.privateer) : undefined;
      const fighting = !!hunter && st.battles.some(b => b.own && b.participants.includes(hunter.id));
      const ordered = (kind: string) => !!hunter && !!st.pendingOrders.get(hunter.id)?.some(o => !o.lost && o.kind === kind);
      const captain = hunter && !ordered("assign") ? reserveCaptain(st, hunter) : undefined;
      // An empty tank at home: the trader's Deuterium fills it at the dock.
      const dry = !!hunter && hunter.fuel != null && hunter.fuel < 1 && atHome(hunter);
      const attack: FoundingStep = { title: "Destroy the privateer", copy: "Send your Interceptor after it.",
        reason: "Bounty: 4,500 Cr.", action: "attack-privateer", label: "Attack" };
      if (!hunter) {
        blocked = true;
        primary = building("raider", "Interceptor", "Then hunt the privateer.")
          ?? { title: "Interceptor lost", copy: `${kit("raider")} for another.`, action: "build-interceptor", label: "Build Interceptor" };
      } else if (fighting) {
        progress.push("Interceptor engaging the privateer");
        primary = { title: "The fight is on", copy: "Its report reaches command at light speed.",
          reason: "Bounty: 4,500 Cr.", action: "privateer", label: "Watch the fight" };
      } else if (dry && dockedMerchant(st) && traderTold(st)) {
        blocked = true;
        primary = { title: "Fuel the Interceptor", copy: "Its tank is empty. The trader sells Deuterium.",
          action: "merchant-fuel", label: "Buy Deuterium" };
      } else if (captain) {
        // Guide-only: the kill completes the step, captain or not.
        primary = { title: "Assign a captain", copy: `${captain.name} takes command of the Interceptor.`,
          reason: "The kill promotes its captain.", action: "assign-captain", label: "Assign captain" };
        if (pirate) waiting.push({ ...attack, title: "Attack without a captain", label: "Attack now" });
      } else if (pirate && (ordered("attack") || heading(hunter, pirate.pos, 3_000, st))) {
        progress.push(`Interceptor closing on the privateer · ETA ${eta(hunter, pirate.pos)}`);
        primary = { title: "The hunt is on", copy: "It engages when it closes.", reason: "Bounty: 4,500 Cr.",
          action: "privateer", label: "Watch the hunt" };
      } else if (pirate) {
        primary = attack;
      } else {
        primary = { title: "Destroy the privateer", copy: "It's out of sight.", action: "command", label: "Open Command" };
      }
      break;
    }
    case 11: {
      const den = foundingDen(st);
      const hunter = own.find(g => guardCapable(g));
      const fighting = !!hunter && st.battles.some(b => b.own && b.participants.includes(hunter.id));
      const near = !!den && !!hunter && Math.hypot(hunter.pos.x - den.pos.x, hunter.pos.y - den.pos.y) <= 1_500;
      if (!den) {
        primary = { title: "Eliminate the privateer den", copy: "The privateer's log names its base.", action: "command", label: "Open Command" };
      } else if (!hunter) {
        blocked = true;
        primary = building("raider", "Interceptor", `Then send it to ${den.name}.`)
          ?? { title: "Interceptor lost", copy: `${kit("raider")} for another.`, action: "build-interceptor", label: "Build Interceptor" };
      } else if (fighting || near) {
        progress.push(`Interceptor ${fighting ? "fighting the garrison" : "at the den"} · ${den.name}`);
        primary = { title: "Assault under way", copy: "Its report reaches command at light speed.", action: "den", label: `Open ${den.name}` };
      } else if (heading(hunter, den.pos, 1_500, st)) {
        progress.push(`Interceptor en route to ${den.name} · ETA ${eta(hunter, den.pos)}`);
        primary = { title: "Interceptor on its way", copy: "The garrison engages it on arrival.", action: "den", label: `Open ${den.name}` };
      } else {
        primary = { title: `Destroy the den at ${den.name}`, copy: "Three damaged raiders. No defenses.",
          reason: "It holds what the raid took.", action: "assault-den", label: "Send Interceptor" };
      }
      break;
    }

    // ─── Expansion: an export, research, a second mine, a survey ──────────────
    // §charter-v5 (Peter, Oct 16 2026): the trader left during the den hunt,
    // so the ore goes to the Market Hub.
    case 12: {
      const shipment = shipmentLine(st, freighter, homePos);
      const hubOre = st.market?.prices.find(p => p.commodity === "metallic_ore")?.price;
      const reason = hubOre != null ? `The Market Hub pays ${hubOre.toFixed(1)} Cr a unit.` : "The Market Hub buys ore.";
      if (!freighter) {
        primary = building("tiny_freighter", "Tiny Freighter", "Then load it with ore.")
          ?? foundingIndustryGoal(home, "shipyard", { title: "Build a Tiny Freighter", copy: `${kit("tiny_freighter")}. It fuels with Uranium at the dock.`,
            reason, action: "build-convoy", label: "Build Tiny Freighter" });
      } else if (routeHold?.startsWith("Storage full") || (storageFull && !atHome(freighter))) {
        blocked = true;
        primary = { title: "Make room at home", copy: routeHold ?? `Storage is full (${Math.floor(home!.storage_used)}/${home!.storage_cap}).`,
          reason: "Expand the storehouse, or ship goods out.", action: "home-development", label: "Open settlement" };
      } else if (shipment) {
        primary = { title: "Shipment under way", copy: "It sells your ore at the Market Hub.",
          action: "shipment-trip", label: "Follow the trip" };
      } else {
        primary = { title: "Export your ore", copy: "Load Ferrite Ore and send the Freighter to the Market Hub.",
          reason, action: "opening-imports", label: "Plan the shipment" };
      }
      break;
    }
    case 13: {
      const academy = foundingIndustryGoal(home, "academy", viaMerchant(st, home, "academy", "Academy", { title: "Build an Academy", copy: `${kit("academy")}. Research unlocks better extraction, hulls and refining.`,
        action: "build-academy", label: "Build Academy" }));
      primary = building("academy", "Academy", "Staff it, then choose a programme.")
        ?? (built("academy") && staffed("academy") ? { title: "Choose your first programme", copy: "Any Tier I programme. The Authority funds most of the first one.",
          action: "research", label: "Open Research" } : academy);
      break;
    }
    case 14: {
      const body = home ? expansionBody(home) : undefined;
      if (!body) {
        blocked = true;
        primary = { title: "No free ore world", copy: "Every other world's resource sites are taken.", action: "home-world", label: `Open ${world}` };
        break;
      }
      const ore = body.deposits?.find(d => MINERAL_DEPOSITS.includes(d.resource))?.resource;
      const reason = "The charter's land grant now covers your whole system.";
      if (!(body.structures.power_plant > 0)) {
        primary = building("power_plant", "Solar Array", `Then a Mining Complex on ${body.name}.`, body.id)
          ?? viaMerchant(st, home, "power_plant", "Solar Array", { title: `Power ${body.name}`, copy: `${kit("power_plant")}. A mine there needs its own power.`,
            reason, action: "build-expansion-power", label: "Build Solar Array" });
      } else if (!(body.structures.mining_complex > 0)) {
        primary = building("mining_complex", "Mining Complex", "Staff it when it is finished.", body.id)
          ?? { title: `Mine ${body.name}`, copy: `${kit("mining_complex")}, on its ${ore ? label(ore) : "ore"}.`,
            reason, action: "build-expansion-mine", label: "Build Mining Complex" };
      } else {
        primary = { title: `Staff ${body.name}'s mine`, copy: staffCopy(home, "Assign one workforce."), action: "expansion-mine", label: "Assign workforce" };
      }
      break;
    }
    case 15: {
      const stop = f.survey_candidates[0];
      const stopName = st.galaxy?.systems.find(s => s.id === stop)?.name ?? "the nearby stop";
      const stopPos = st.galaxy?.systems.find(s => s.id === stop)?.pos;
      const active = stopPos ? own.find(g => surveyCapable(g) && surveyingAt(g, stopPos, st)) : undefined;
      const surveyor = freeSurveyor(own);
      // §charter-v4: the den's system, whose belt the den guarded.
      const why = stop && stop === f.den ? "The den guarded a rich belt there." : "Only a survey reveals what a system holds.";
      if (active && stopPos) {
        const noun = surveyorNoun(active);
        progress.push(active.survey_progress != null
          ? `Surveying ${stopName} · ${Math.round(active.survey_progress * 100)}%`
          : `${noun} en route to ${stopName} · ETA ${eta(active, stopPos)}`);
        primary = { title: "The survey is under way", copy: `Its report charts ${stopName}'s worlds and any belts, and comes home at light speed.`,
          action: noun === "Scout" ? "scout" : "probe", label: `Select the ${noun === "Scout" ? "Scout" : "probe"}` };
      } else if (surveyor) {
        primary = { title: `Chart ${stopName}`, copy: `Send your ${surveyorNoun(surveyor)} to survey ${stopName}. A probe is spent when its report is sent.`,
          reason: why, action: "survey-stop", label: "Send the survey" };
      } else {
        primary = building("survey_probe", "Survey Probe", `When it launches, send it to ${stopName}.`)
          ?? viaMerchant(st, home, "survey_probe", "Survey Probe", { title: "Build a Survey Probe", copy: `${kit("survey_probe")} at your Shipyard. It flies to ${stopName}, charts it and is spent.`,
            reason: stop && stop === f.den ? why : "The cheapest way to learn what a system holds.", action: "build-probe", label: "Build Survey Probe" });
      }
      break;
    }

    // ─── Reach: grow without a colony ─────────────────────────────────────────
    default: {
      const belt = foundingBelt(st);
      const beltName = belt ? `${belt.name}${belt.system === home?.id ? "" : ` in ${belt.where}`}` : "the charted belt";
      const barge = own.find(g => mineCapable(g));
      const work = barge?.industry?.kind === "mine" ? barge.industry : null;
      const aboard = aboardUnits(barge);
      if (f.stage === "build_barge" || !barge) {
        primary = building("mining_barge", "Mining Barge", `When it launches, send it to ${beltName}.`);
        if (!primary) {
          primary = foundingBargeGoal(home, beltName);
          if (primary.action === "build-barge") primary = viaMerchant(st, home, "mining_barge", "Mining Barge", primary);
          if (f.stage !== "build_barge") primary = { ...primary, title: "Replace the Mining Barge", reason: "Your barge was lost." };
          // By now the food reserve and ore fill home storage; the kit needs room to land.
          if (storageFull && primary.action === "build-barge") {
            primary = { ...primary, reason: `Home storage is full (${Math.floor(home!.storage_used)}/${home!.storage_cap}). Ship goods out or add storage so the barge's kit can land.` };
          }
        }
      } else if (f.stage === "mine_belt") {
        const bound = !!belt?.pos && (work?.status === "travelling" || (!work && heading(barge, belt.pos, 1_500, st)));
        if (work && work.status !== "travelling") {
          progress.push(`Mining Barge ${work.status === "mining" ? `cutting ${label(work.ore)}` : work.status} at ${beltName} · its report is on the way to command`);
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
        const homeBound = !!homePos && !atHome(barge) && !work && heading(barge, homePos, 2_500, st);
        if (atHome(barge) && aboard > 0) {
          primary = { title: "Unload the ore", copy: `Store the barge's ${aboard} units in your home stockpile, for the Smelter or the Market.`,
            action: "barge-unload", label: "Unload ore" };
        } else if (homeBound && homePos) {
          progress.push(`Mining Barge bringing ${aboard} units home · ETA ${eta(barge, homePos)}`);
          primary = { title: "The barge is heading home", copy: "Unload it when it docks.", action: "barge", label: "Select Mining Barge" };
        } else if (work) {
          progress.push(`Mining Barge ${work.status === "full" ? "hold full" : `cutting ${label(work.ore)}`} at ${beltName} · ${aboard} units aboard`);
          primary = { title: "Bring the ore home", copy: work.status === "full" ? "The hold is full. Send the barge home and unload it." : "Send the barge home with what it has cut, or let it fill its hold first.",
            reason: "Ore earns nothing until it is unloaded or sold.", action: "barge-home", label: "Send the barge home" };
        } else {
          primary = { title: "Bring the ore home", copy: "Send the barge home and unload its ore, or sell it at the Market Hub.", action: "barge-home", label: "Send the barge home" };
        }
      }
    }
  }
  // §charter-v5: the trader's last call. Once the privateer's bounty is paid
  // it leaves within minutes (sooner if the den falls), and after it only the
  // Market Hub sells semiconductors: the bounty buys what the builds ahead need.
  const lastCall = f.bounty_received && traderTold(st) ? dockedMerchant(st) : null;
  if (lastCall && primary) {
    progress.unshift(`Trader leaves in ${fmtDur(merchantLeavesIn(lastCall, st.simTime))}`);
    const short = Object.entries(merchantShortfall(st, home, "ahead", lastCall))
      .sort(([a], [b]) => Number(b === "electronics") - Number(a === "electronics"));
    if (short.length) {
      const stock: FoundingStep = { title: "Stock up before the trader leaves",
        copy: `${short.map(([c, n]) => `${n} ${label(c)}`).join(", ")} for the builds ahead.`,
        action: "merchant-ahead", label: "Buy from trader" };
      if (blocked) waiting.unshift(stock);
      else primary = { ...stock, reason: "After it, only the Market Hub sells them.",
        alternatives: [stock, { title: primary.title, copy: primary.copy, action: primary.action, label: primary.label }] };
    }
  }
  if (queued("mining_complex") && step !== 4 && step !== 14) progress.push("Mining Complex under construction");
  // The first shipment's trip stays in view after its step: it brings parts home.
  const trip = step >= 12 ? shipmentLine(st, freighter, homePos) : null;
  if (trip) progress.unshift(trip);
  if (camp && camp.founding_camp!.completed.length < 4 && primary?.action !== "home-development") waiting.push(settlement);
  const goal = step >= 12 && step <= 14 ? denBeltGoal(st, own, progress, home) : null;
  return { chapter: { name: chapterName, done: checklist.filter(s => s.done).length, total: checklist.length }, checklist, progress, primary, waiting, goal, blocked };
}

/** §charter-v4: the guide's staffing actions by structure. */
const STAFF_ACTION: Record<string, string> = {
  bioharvester: "bioharvester", agroplex: "agroplex", mining_complex: "mine", shipyard: "shipyard", academy: "academy",
};

/** §charter-v4: where a guide build action builds and what it selects. Until
 * "Expand mining operations" opens the land grant, every build stands on the
 * landing world. */
export function foundingBuildTarget(action: string, st: ViewState, home: SystemStateView):
  { body: BodyView; mode: "structures" | "ships"; select: string } | null {
  const target = BUILD_ACTIONS[action];
  if (!target) return null;
  const landing = landingBody(home);
  const grant = st.founding?.land_grant_open !== false || !landing;
  const yard = home.bodies.find(b => (b.structures.shipyard ?? 0) > 0);
  const body = target.at === "landing" ? landing
    : target.at === "expansion" ? expansionBody(home)
    : target.at === "yard" ? yard ?? (grant ? bestShipyardBody(home) : landing)
    : !grant ? landing
    : target.at === "mine" ? bestMineBody(home)
    : target.at === "academy" ? bestAcademyBody(home)
    : bestShipyardBody(home);
  return body ? { body, mode: target.mode, select: target.key } : null;
}

const BUILD_ACTIONS: Record<string, { key: string; mode: "structures" | "ships"; at: "landing" | "mine" | "academy" | "shipyard" | "yard" | "expansion" }> = {
  "build-bioharvester": { key: "bioharvester", mode: "structures", at: "landing" },
  "build-habitat": { key: "habitat", mode: "structures", at: "landing" },
  "build-agroplex": { key: "agroplex", mode: "structures", at: "landing" },
  "build-power": { key: "power_plant", mode: "structures", at: "landing" },
  "build-mine": { key: "mining_complex", mode: "structures", at: "mine" },
  "build-academy": { key: "academy", mode: "structures", at: "academy" },
  "build-shipyard": { key: "shipyard", mode: "structures", at: "shipyard" },
  "build-shipyard-2": { key: "shipyard", mode: "structures", at: "yard" },
  "build-expansion-power": { key: "power_plant", mode: "structures", at: "expansion" },
  "build-expansion-mine": { key: "mining_complex", mode: "structures", at: "expansion" },
  "build-convoy": { key: "tiny_freighter", mode: "ships", at: "yard" },
  "build-interceptor": { key: "raider", mode: "ships", at: "yard" },
  "build-scout": { key: "scout", mode: "ships", at: "yard" },
  "build-colony": { key: "colony", mode: "ships", at: "yard" },
  "build-barge": { key: "mining_barge", mode: "ships", at: "yard" },
  "build-probe": { key: "survey_probe", mode: "ships", at: "yard" },
};

/** Where a Shipyard goes: a low-gravity world first, then the most Industrial sites. */
export function bestShipyardBody(system: SystemStateView): BodyView | undefined {
  return [...system.bodies].sort((a, b) => {
    const aLow = a.special === "low_gravity" ? 1 : 0;
    const bLow = b.special === "low_gravity" ? 1 : 0;
    return bLow - aLow || (b.industrial_slots ?? 0) - (a.industrial_slots ?? 0) || a.id - b.id;
  })[0];
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
    // §charter-v4: the privateer, its captain and its den.
    case "attack-privateer":
      return escort && f.privateer && st.ghosts.some(g => g.id === f.privateer)
        ? [{ type: "AttackFleet", fleet_id: escort.id, target_id: f.privateer }] : null;
    case "assign-captain": {
      const captain = escort ? reserveCaptain(st, escort) : undefined;
      return escort && captain ? [{ type: "AssignCaptain", captain_id: captain.id, fleet_id: escort.id }] : null;
    }
    case "assault-den": {
      const den = foundingDen(st);
      return escort && den ? [{ type: "MoveShip", ship_id: escort.id, dest: den.pos }] : null;
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
  const ore = (b: BodyView) => b.deposits?.filter((entry) => entry.resource === "metallic_ore").reduce((sum, entry) => sum + entry.richness, 0) ?? 0;
  // §home-ore: the command's own planet first while it has ore and a free
  // resource site, so the first mine stands on the habitable world.
  const home = (b: BodyView) => {
    const p = bodyPoolUsage(b, system).resource;
    return Number((b.structures?.command_center ?? 0) > 0 && ore(b) > 0 && p.used < p.total);
  };
  return [...system.bodies].sort((a, b) => home(b) - home(a) || ore(b) - ore(a)
    || (b.resource_slots ?? 0) - (a.resource_slots ?? 0) || a.id - b.id)[0];
}

/** Where the Academy goes: the command's planet while it has a free
 * Infrastructure site, otherwise the planet with the most free sites. */
export function bestAcademyBody(system: SystemStateView): BodyView | undefined {
  const free = (b: BodyView) => { const p = bodyPoolUsage(b, system).infrastructure; return p.total - p.used; };
  const command = (b: BodyView) => Number((b.structures?.command_center ?? 0) > 0);
  return [...system.bodies].sort((a, b) => Number(free(b) > 0) - Number(free(a) > 0)
    || command(b) - command(a) || free(b) - free(a) || a.id - b.id)[0];
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
  // §no-spoilers: pirates are named only once the raid's report is in.
  const early = now >= f.protection_min_until && f.bounty_received
    ? " It ends early if you attack another corporation."
    : f.raided ? " Pirates can still attack you." : "";
  return { label: `shield · ${left}`, detail: `Founder shield: no corporation can attack you, and you can't attack them, for ${left}.${early}` };
}

import type { CaptainRosterView, Commodity, FoundingStage, FoundingView, GhostView } from "../../protocol";
import type { BodyView, SystemStateView } from "../../protocol";
import { isPlayerFreighter } from "../../protocol";
import type { ViewState } from "../../state";
import type { FleetCommand } from "../fleetorders";
import { basketRoom, COMMODITY_VALUE, dockedAtSystem, fleetBaseSpeed, guardCapable, mineCapable, regimeFuel, regimeStore, ROOM_SCALE, surveyCapable, transitSpeedMultiplier, WARP_FACTOR } from "./fleet";
import { fmtDur } from "./format";
import { HYPERLIMIT_SU } from "./geo";
import { bodyPoolUsage, MINERAL_DEPOSITS } from "./market";
import { dockedMerchant, merchantShortfall, type MerchantTarget } from "./merchant";
import { label } from "../../icons";
import { colonyName, pirateNames, PRIVATEER_HUNT_SU, STRIKE_BOUNTY, VENDETTA_BOUNTY, VENDETTA_DEFENDERS } from "./vendetta";

/** §charter-v5: twenty-eight steps in six chapters (§vendetta added the last;
 * §refining and §den-pocket the Smelter and the prospecting pass, and
 * §gun-platform the defense after the raid, Oct 30 2026; §charter-crates the
 * stores before the uncle, Oct 31; §stash the den's aftermath, three steps,
 * Nov 3; §full-speed and §signals one each, Nov 4). Retired stages count as
 * the step that replaced them; a finished charter counts as the last. */
export const FOUNDING_TOTAL = 29;
export const FOUNDING_STEP: Record<FoundingStage, number> = {
  // §charter-v6 (Peter, Nov 7 2026): the Solar Array before the mine (the
  // Command Center's reactor runs the colony, not the mine or the Smelter),
  // the Smelter right after it, then the raid that takes the freighter's
  // engines breaks the plan. §habitat-first (Nov 8 2026): the Habitat first
  // (unpowered), then the Solar Array, then the food chain on its power.
  build_habitat: 1, power_shipyard: 2, build_bioharvester: 3, build_agroplex: 4, build_mine: 5, build_smelter: 6,
  pirate_raid: 7, build_gun_platform: 8, sell_ore: 9,
  build_shipyard: 10, buy_semiconductors: 10, complete_export: 10, grow_business: 10, build_second_freighter: 10,
  build_interceptor: 11, build_freighter: 11, build_convoy: 11,
  defeat_privateer: 12, eliminate_den: 13, repair_interceptor: 14, build_tiny_freighter: 15, full_speed: 16, haul_loot: 17,
  prepare_export: 18, local_recovery: 18, export_production: 18,
  first_research: 19, build_academy: 19, answer_signal: 20,
  // §freight-frames (Nov 6 2026): the hull the research unlocks.
  build_small_freighter: 21, expand_mining: 22,
  survey_candidates: 23, build_scout: 23,
  build_barge: 24, prospect_belt: 25, mine_belt: 26, haul_ore: 27, secure_stores: 28, vendetta: 29,
  build_colony: 29, establish_colony: 29, complete: 29,
};

export type FoundingChapter = "Landfall" | "Raid" | "Pirates" | "Expansion" | "Reach" | "Vendetta";
/** The guide's checklist in the server's words (tutorial::Step::label). */
export const FOUNDING_CHAPTERS: [FoundingChapter, [FoundingStage, string][]][] = [
  ["Landfall", [["build_habitat", "House the settlers"], ["power_shipyard", "Build a Solar Array"],
    ["build_bioharvester", "Harvest biomass"], ["build_agroplex", "Make provisions"], ["build_mine", "Bring the mine online"],
    ["build_smelter", "Smelt your first Alloys"]]],
  ["Raid", [["pirate_raid", "Pirates raid the colony"], ["build_gun_platform", "Build a Laser Battery"],
    ["sell_ore", "Sell Alloys to the trader"], ["build_shipyard", "Build a Shipyard"]]],
  ["Pirates", [["build_interceptor", "Build an Interceptor"], ["defeat_privateer", "Destroy the privateer"],
    ["eliminate_den", "Eliminate the privateer den"]]],
  ["Expansion", [["repair_interceptor", "Repair the Interceptor"], ["build_tiny_freighter", "Build a Tiny Freighter"],
    ["full_speed", "Switch to Full speed"], ["haul_loot", "Haul back the den's loot"], ["prepare_export", "Export your Alloys"],
    ["first_research", "Research Freight Frames"], ["answer_signal", "Answer a signal"], ["build_small_freighter", "Build a Small Freighter"],
    ["expand_mining", "Expand mining operations"],
    ["survey_candidates", "Chart a nearby system"]]],
  ["Reach", [["build_barge", "Build a Mining Barge"], ["prospect_belt", "Prospect the belt"],
    ["mine_belt", "Mine the belt"], ["haul_ore", "Bring the ore home"]]],
  ["Vendetta", [["secure_stores", "Secure your stores"], ["vendetta", "Defeat the privateer's uncle"]]],
];
/** The charter's first step, where the guide's two client steps (open your
 * home system, then a world of it) come first. */
const FIRST_STAGE: FoundingStage = FOUNDING_CHAPTERS[0][1][0][0];

/** §vendetta: the guide's anonymous pirates by their family's names: "the
 * privateer den", "the privateer's uncle" and "the privateer". */
export function withPirateNames(text: string, f?: FoundingView | null): string {
  const names = pirateNames(f);
  if (!names.family) return text;
  const cap = (s: string) => s[0].toUpperCase() + s.slice(1);
  return text
    .replace(/\b([Tt])he privateer den\b/g, (_, t: string) => t === "T" ? cap(names.den) : names.den)
    .replace(/\b[Tt]he privateer's uncle\b/g, names.uncle)
    .replace(/\b[Tt]he (Rogue )?[Pp]rivateer\b/g, names.privateer);
}

/** §freight-frames (Peter, Nov 6 2026): the charter's research step (sim
 * `founding::CHARTER_RESEARCH`), for the Small Freighter it unlocks. It was
 * Enrichment, for the Smelter, until §smelt-from-landing ungated it. */
export const CHARTER_RESEARCH = "prop_freight_frames";
const CHARTER_RESEARCH_NAME = "Freight Frames";
/** §den-stores: a Tiny Freighter's hold (sim `ShipKind::TinyFreighter.cargo_units()`),
 * in room units: §hold-room counts each good's own room. */
const TINY_HOLD = 50;

/** §no-spoilers (Oct 15 2026): a twist is news only once it is told — the
 * raid once command sees it, the trader once its pop-up has shown. Until
 * then the guide keeps back its step, even when it is the current one, and
 * calls its chapter by a plain name; a step already passed always shows. */
const TWIST: Partial<Record<FoundingStage, (f: FoundingView, st: ViewState) => boolean>> = {
  pirate_raid: f => !!f.raid_seen,
  // §gun-platform: a defense step would give the raid away.
  build_gun_platform: f => !!f.raid_seen,
  sell_ore: (_, st) => traderTold(st),
};
const CHAPTER_TOLD: Partial<Record<FoundingChapter, (f: FoundingView) => boolean>> = { Raid: f => !!f.raid_seen };
const COVER_NAME: Partial<Record<FoundingChapter, string>> = { Raid: "Industry" };

/** Next actions, not a class-selection command. Derived solely from served
 * home/research reports; queued builds never pretend to satisfy a prerequisite. */
export function foundingBusinessGoals(_st: ViewState, home?: SystemStateView) {
  const built = (kind: string) => home?.bodies.some(b => (b.structures[kind] ?? 0) > 0);
  const staffed = (kind: string) => home?.assignments.some(a => a.structure === kind
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  const freight = !built("shipyard") ? ["build-shipyard", "Build Shipyard"]
    : !staffed("shipyard") ? ["shipyard", "Staff Shipyard"] : ["build-convoy", "Build Tiny Freighter"];
  // §smelt-from-landing (Nov 6 2026): the Smelter needs no research.
  const refine = !built("smelter") ? ["build-smelter", "Build Smelter"] : ["smelter", "Staff & supply Smelter"];
  return [
    { title: "Expand exports", copy: "Shipyard → second Freighter", action: freight[0], label: freight[1] },
    { title: "Start refining", copy: "Ferrite Ore → Alloys", action: refine[0], label: refine[1] },
  ];
}

/** §camp (Oct 28 2026, Peter): the Founding Camp's settlement projects open
 * once the charter is complete; until then the guide alone leads. */
export const campOpen = (st: ViewState): boolean => !st.founding || st.founding.stage === "complete";

/** Landfall's parallel goals from received reports: the fallback card when
 * the plan has no single next step. The guide never sends fleet orders itself.
 * §charter v3: no beacon, cache or Scout refit; null when nothing is left. */
export function foundingLocalGoals(st: ViewState, home?: SystemStateView) {
  const f = st.founding;
  if (!f?.local_opening || !["local_recovery", "prepare_export"].includes(f.stage)) return null;
  const mine = home?.bodies.some(b => (b.structures.mining_complex ?? 0) > 0);
  const staffed = home?.assignments.some(a => a.structure === "mining_complex" && a.workers > 0);
  const goals = [];
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
 * system's name and position, and the ore worth cutting there (and its
 * richness). */
export function foundingBelt(st: ViewState): { system: string; body: number; name: string; where: string;
  pos?: { x: number; y: number }; ore?: Commodity; richness?: number; pocket?: Commodity } | null {
  const target = st.founding?.belt_target;
  if (!target) return null;
  const system = st.systems.find(s => s.id === target.system);
  const belt = system?.bodies.find(b => b.id === target.body);
  const info = st.galaxy?.systems.find(s => s.id === target.system);
  const best = [...(belt?.deposits ?? [])]
    .sort((a, b) => b.richness * (COMMODITY_VALUE[b.resource] ?? 0) - a.richness * (COMMODITY_VALUE[a.resource] ?? 0))[0];
  // §den-pocket: a pocket this corporation has found, while it lasts.
  const pocket = belt?.pockets?.find(d => d.reserves == null || d.reserves > 0)?.resource;
  return { system: target.system, body: target.body, name: belt?.name ?? "the charted belt",
    where: info?.name ?? "a nearby system", pos: info?.pos, ore: best?.resource, richness: best?.richness, pocket };
}

/** §charter-v4: the privateer den its log named, once the kill report is in. */
export function foundingDen(st: ViewState): { system: string; name: string; pos: { x: number; y: number } } | null {
  const id = st.founding?.den;
  const info = id ? st.galaxy?.systems.find(s => s.id === id) : undefined;
  return id && info ? { system: id, name: info.name, pos: info.pos } : null;
}

/** "an", "a second", "a third": the next Interceptor, by how many fly. */
const interceptorOrdinal = (have: number): string => ["an", "a second", "a third"][have] ?? "another";

/** In reserve and fit for duty, wherever stationed. */
const inReserve = (c: CaptainRosterView, now: number) => c.assigned_fleet == null && c.loss_fate == null
  && (c.recovering_until == null || c.recovering_until <= now);

/** The fleet carries a captain. */
const captained = (st: ViewState, fleet: GhostView) => !!fleet.captain || st.captains.some(c => c.assigned_fleet === fleet.id);

/** §charter-v4: a captain in reserve where this fleet is docked, free to
 * take command of it. None when the fleet already has one. */
export function reserveCaptain(st: ViewState, fleet: GhostView): CaptainRosterView | undefined {
  if (captained(st, fleet)) return undefined;
  return st.captains.find(c => inReserve(c, st.simTime) && !!c.stationed_system && dockedAtSystem(fleet, c.stationed_system));
}

/** §belts: the barge lesson's build step, Shipyard first. */
export function foundingBargeGoal(home: SystemStateView | undefined, belt: string,
  recipe = "3 Fission Reactors · 3 Zephyr Drives · 1 Nav Computer · 5 Hull Sections · 3 Cargo Bays · 20 Alloys · 10 Machinery · 8 Polymers · 6 Semiconductors"): FoundingStep {
  const yard = home?.bodies.find(b => (b.structures.shipyard ?? 0) > 0);
  const staffed = yard && home?.assignments.some(a => a.body_id === yard.id && a.structure === "shipyard"
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  return !yard ? { title: "Prepare a Shipyard", copy: "For the Mining Barge.", action: "build-shipyard", label: "Build Shipyard" }
    : !staffed ? { title: "Staff the Shipyard", copy: staffCopy(home), hint: UNSTAFFED, action: "shipyard", label: "Assign workforce" }
    : { title: "Build a Mining Barge", copy: recipe, hint: `It mines ${belt}. A barge works a belt in a system you don't own; no colony needed.`,
      action: "build-barge", label: "Build" };
}

/** §charter-v4: the free trader has been told — its pop-up shown, or its
 * docking news met in an earlier session. Nothing names it before. */
export const traderTold = (st: ViewState): boolean => !!st.storyTold?.has("free_trader");

/** §charter-v5: the fuel the Interceptor's tank still takes while it sits
 * docked at home, where the trader is: the guide's "Buy Uranium" prefill.
 * §reactors-drives (Nov 8 2026): the tank its reactor burns (a Fission
 * Interceptor's is Uranium), never the Deuterium tank it doesn't have. */
export function fuelNeed(st: ViewState): FuelNeed | null {
  const home = st.merchant?.system;
  const hunter = home ? st.ghosts.find(g => g.own && guardCapable(g) && dockedAtSystem(g, home)) : undefined;
  const tank = hunter ? regimeStore(hunter) : undefined;
  return tank && tank.served != null ? { fuel: tank.fuel, room: Math.max(0, tank.capacity - tank.have) } : null;
}
export interface FuelNeed { fuel: Commodity; room: number }

/** §merchant: while the free trader is docked, a build the home is short of
 * offers it as the first choice, the shortfall filled in; the ordinary
 * build step stays beside it. */
export function viaMerchant(st: ViewState, home: SystemStateView | undefined, target: MerchantTarget,
  name: string, step: FoundingStep): FoundingStep {
  const merchant = traderTold(st) ? dockedMerchant(st) : null;
  const short = merchant && home ? merchantShortfall(st, home, target, merchant) : {};
  const goods = Object.entries(short).map(([c, n]) => `${n} ${label(c)}`).join(" · ");
  if (!goods) return step;
  const trader = { title: `Buy the ${name} parts`, copy: goods, action: `merchant-${target}`, label: "Buy from trader" };
  return { ...trader, reason: step.reason, why: step.why, hint: step.hint, alternatives: [trader, choice(step)] };
}

/** A step offered beside another: its button says the whole step. */
const choice = (step: FoundingStep) => ({ title: step.title, copy: step.copy, action: step.action, label: step.title });
/** Peter (Oct 30 2026): the Interceptor's captain isn't optional. */
const assignCaptain = (captain: CaptainRosterView): FoundingStep => ({ title: "Assign a captain", copy: `${captain.name} → Interceptor`,
  hint: "The kill promotes its captain.", action: "assign-captain", label: "Assign" });

// ─── §founding R4/R7: ONE plan for every shell ───────────────────────────────
// The guide shows one next action with its reason, what is already under way
// (with ETAs), and optional work "while you wait". Chapters count real facts
// from received reports, so the numbering never skips a step.

// Peter (Oct 27 2026): "reduce the repetitive text in the tutorial steps".
// One line per fact: the button never repeats the title, copy is figures, why
// goes on hover, and a card that waits takes its progress line as its title.
export interface FoundingStep {
  title: string;
  /** Figures (a recipe, a count); empty when the title and button say it all. */
  copy: string;
  action: string;
  /** The button: just the verb when the title names the thing ("Build"). */
  label: string;
  /** A reward or an obstacle, in a few words ("Bounty: 8,000 Cr."): the
   * state of play, so it outranks the Charter Advisor's line. */
  reason?: string;
  /** §charter-advisor: the step's ordinary why ("Light the colony."). The
   * advisor's line speaks in its place (core/derive/advisor.ts); without an
   * advisor it shows where a reason would. */
  why?: string;
  /** §charter-advisor: a wait or an interruption with no clock of its own
   * (`status`): the advisor keeps quiet and the card keeps its line. */
  quiet?: boolean;
  /** Why, on hover (desktop only). */
  hint?: string;
  /** A waiting card's own progress line, ETA and all: its title in the guide,
   * and no longer in the under-way list. The Log keeps the plain title. */
  status?: string;
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
  /** Tone for the primary card: an obstacle the player must clear first. */
  blocked: boolean;
}


/** Time to `dest`, or to `within` of it: a raider arrives at the colony's
 * well edge, where the raid (and the charter's mine) begins. */
function eta(g: GhostView, dest: { x: number; y: number }, within = 0): string {
  const distance = Math.max(0, Math.hypot(dest.x - g.pos.x, dest.y - g.pos.y) - within);
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

/** Mirrors sim founding::EXPORT_FOLLOW_MAX_S: the first shipment is followed
 *  out of the well this long at most (Peter, Nov 2 2026: "90 seconds max"). */
export const EXPORT_FOLLOW_MAX_S = 90;

/** §export-follow: the first shipment's run out of the home well, while the
 *  player follows it: seconds until it clears the hyperlimit (where it warps
 *  for the Hub and the step completes on its light home), capped at
 *  EXPORT_FOLLOW_MAX_S from departure. Null before it leaves the dock, once it
 *  has cleared, or when the step is done. */
export function shipmentFollowLeft(st: ViewState, freighter: GhostView | undefined, homePos?: { x: number; y: number }): number | null {
  const run = freighter?.industry?.kind === "route" ? freighter.industry.run : null;
  if (!freighter || !run || !homePos || st.founding?.export_dispatched || run.phase !== "travelling") return null;
  if (run.route.stops[run.stop]?.port.kind !== "hub") return null;
  const px = freighter.pos.x - homePos.x, py = freighter.pos.y - homePos.y;
  const out = Math.hypot(px, py);
  if (out >= HYPERLIMIT_SU) return null;
  // The chord from the ship, along its course to the Hub, to the well's edge.
  const hub = st.galaxy?.hub;
  const dx = hub ? hub.x - freighter.pos.x : px, dy = hub ? hub.y - freighter.pos.y : py;
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len, uy = dy / len;
  const along = px * ux + py * uy;
  const chord = -along + Math.sqrt(Math.max(0, along * along + HYPERLIMIT_SU * HYPERLIMIT_SU - out * out));
  const speed = Math.max(1, Math.hypot(freighter.vel.x, freighter.vel.y), fleetBaseSpeed(freighter));
  const departed = st.founding?.export_departed_at;
  const cap = departed != null ? departed + EXPORT_FOLLOW_MAX_S - st.simTime : Infinity;
  return Math.max(0, Math.min(chord / speed, cap));
}

/** §export-follow: the freighter the camera follows out of the home well
 *  during "Export your ore", or null when there is nothing to follow. */
export function shipmentFollowTarget(st: ViewState, homeId: string | null): GhostView | null {
  if (!homeId || st.founding?.stage !== "prepare_export") return null;
  const freighter = st.ghosts.find(g => g.own && (g.composition?.some(c => isPlayerFreighter(c.kind) && c.count > 0) ?? isPlayerFreighter(g.kind)));
  const homePos = st.galaxy?.systems.find(s => s.id === homeId)?.pos;
  return freighter && shipmentFollowLeft(st, freighter, homePos) != null ? freighter : null;
}

const aboardUnits = (g: GhostView | undefined) =>
  (g?.cargo_manifest ?? (g?.cargo ? [g.cargo] : [])).reduce((n, c) => n + c.units, 0);

/** §workforce cap: a staffing step needs no copy — unless every worker already
 * has a post, when it says where to find one (the + is disabled). */
function staffCopy(home: SystemStateView | undefined): string {
  const workforce = home?.workforce;
  return workforce && workforce.units - workforce.posted <= 0 ? "No free workforce: take one off another building." : "";
}

const UNSTAFFED = "An unstaffed building does nothing.";

/** An Industry build step, yard first. */
function foundingIndustryGoal(home: SystemStateView | undefined, structure: "shipyard" | "academy",
  build: FoundingStep): FoundingStep {
  const body = home?.bodies.find(b => (b.structures[structure] ?? 0) > 0);
  const staffed = !!body && !!home?.assignments.some(a => a.body_id === body.id && a.structure === structure
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  if (!body) return build;
  if (!staffed) {
    return { title: structure === "shipyard" ? "Staff the Shipyard" : "Staff the Academy",
      copy: staffCopy(home), hint: UNSTAFFED, action: structure, label: "Assign workforce" };
  }
  return build;
}

/** A build's recipe from the served catalogue: "25 Alloys · 12 Machinery". */
export function recipeLine(st: ViewState, key: string): string {
  return (st.galaxy?.build_options?.find(o => o.key === key)?.costs ?? [])
    .map(c => `${c.units} ${label(c.commodity)}`).join(" · ");
}

/** §home-step (Peter, Oct 21 2026): the guide's first step is to open your
 * home system; clicking its star, or the button, flies into its System View.
 * A client step before the server's eighteen: done once this session has
 * opened the home System View, or once anything is built or queued at home. */
export const HOME_STEP_TITLE = "Open your home system";

export function homeStepPending(st: ViewState, home?: SystemStateView): boolean {
  const f = st.founding;
  if (!f?.local_opening || f.stage !== FIRST_STAGE || st.homeOpened || !home) return false;
  return untouchedHome(home);
}

/** Nothing built or queued at home beyond the Command Center. */
const untouchedHome = (home: SystemStateView): boolean => !home.builds.length
  && home.bodies.every(b => Object.entries(b.structures).every(([kind, n]) => kind === "command_center" || !n));

/** §home-world-step (Peter, Oct 28 2026): then open one of home's worlds, to
 * see the planet view. A client step like the first, done the same ways. */
export const HOME_WORLD_STEP_TITLE = "Open your home world";

export function homeWorldStepPending(st: ViewState, home?: SystemStateView): boolean {
  const f = st.founding;
  if (!f?.local_opening || f.stage !== FIRST_STAGE || !st.homeOpened || st.homeWorldOpened || !home) return false;
  return untouchedHome(home);
}

/** Latch the second step once a world of home is open. True when it has just
 * latched, so the guide redraws. */
export function noteHomeWorldOpened(st: ViewState, systemId: string | null, homeId: string | null): boolean {
  if (st.homeWorldOpened || !systemId || systemId !== homeId) return false;
  st.homeWorldOpened = true;
  return true;
}

/** The home system while the first step asks for it, else null. */
export function homeStepSystem(st: ViewState, homeId: string | null): string | null {
  const home = homeId ? st.systems.find(s => s.id === homeId) : undefined;
  return home && homeStepPending(st, home) ? home.id : null;
}

/** Latch the first step once the home System View is open, however it was
 * entered. True when it has just latched, so the guide redraws. */
export function noteHomeOpened(st: ViewState, viewedId: string | null, homeId: string | null): boolean {
  if (st.homeOpened || !viewedId || viewedId !== homeId) return false;
  st.homeOpened = true;
  return true;
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
  // §asteroid-quarry: on a belt the mine is its quarry.
  const mine = (b: BodyView) => level(b, "mining_complex") + level(b, "asteroid_quarry");
  return home.bodies.filter(b => !level(b, "command_center") && ore(b) && (mine(b) > 0 || free(b)))
    .sort((a, b) => mine(b) - mine(a) || level(b, "power_plant") - level(a, "power_plant")
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
  // §export-follow: seconds left of the first shipment's run out of the well, or null.
  const followLeft = shipmentFollowLeft(st, freighter, homePos);
  const landing = landingBody(home);
  const world = landing?.name ?? "your landing world";
  const kit = (key: string) => recipeLine(st, key);
  const step = FOUNDING_STEP[f.stage];
  const progress: string[] = [];
  const waiting: FoundingStep[] = [];
  let primary: FoundingStep | null = null;
  let blocked = false;
  /** Something under way: its progress line, and a card that waits on it
   * with that line as its title. */
  const waitOn = (title: string, detail: string | null, card: Omit<FoundingStep, "title" | "status">): FoundingStep => {
    const status = detail ? `${title} · ${detail}` : title;
    progress.push(status);
    return { ...card, title, status };
  };
  /** A build under way: its progress line, and a card to wait on. */
  const building = (kind: string, name: string, then: string, body?: number): FoundingStep | null => {
    const j = job(kind, body);
    if (!j) return null;
    const left = !j.queued && j.complete_time != null ? Math.max(0, j.complete_time - st.simTime) : null;
    return waitOn(`${name} under construction`, j.queued ? "queued" : left != null ? fmtDur(left) : null,
      { copy: then, action: "home-world", label: `Open ${body !== undefined && body !== landing?.id ? "world" : world}` });
  };
  /** Build a structure, then staff it when it needs a crew. */
  const structure = (kind: string, name: string, build: FoundingStep, crew: boolean): FoundingStep => {
    if (!built(kind)) return building(kind, name, crew ? "Then staff it." : "") ?? build;
    if (crew && !staffed(kind)) {
      return { title: `Staff the ${name}`, copy: staffCopy(home), hint: UNSTAFFED, action: STAFF_ACTION[kind] ?? kind, label: "Assign workforce" };
    }
    return build;
  };
  /** §watch-the-pirate (Peter, Nov 9 2026: "a 'Watch the Pirate' step as
   * soon as the Pirate Warning appears and only ends once the Pirate has
   * disappeared"): from the raid's sighting until the colony loses the
   * privateer after it (the server's `raid_lost`), whatever the step. The
   * step's own action comes back once it has gone. §raid-course (Peter, Nov 8 2026: it
   * said "over the colony" before the raider got there): over once the news
   * of its arrival over the colony's world is home (`raid_over`), not on
   * nearing the star. Quiet: no advice or nudge over it. */
  const watchCard = (): FoundingStep | null => {
    if (!f.raid_seen || f.raid_lost || step > FOUNDING_STEP.sell_ore) return null;
    blocked = true;
    const pirate = f.privateer ? st.ghosts.find(g => g.id === f.privateer) : undefined;
    const homeName = st.galaxy?.systems.find(s => s.id === home?.id)?.name ?? "the colony";
    const copy = f.raided ? "Leaving with what it took."
      : f.raid_over ? "Stripping the stockpile."
        : `Closing on ${homeName}${pirate && homePos ? ` · ETA ${eta(pirate, homePos, HYPERLIMIT_SU)}` : ""}`;
    return { title: "Watch the pirate", copy, action: "privateer", label: "Watch", quiet: true };
  };
  const chapterOf = FOUNDING_CHAPTERS.find(([, steps]) => steps.some(([s]) => FOUNDING_STEP[s] === step))
    ?? FOUNDING_CHAPTERS[FOUNDING_CHAPTERS.length - 1];
  const told = ([s]: [FoundingStage, string]) => step > FOUNDING_STEP[s] || (TWIST[s]?.(f, st) ?? true);
  const homeStep = homeStepPending(st, home);
  const worldStep = homeWorldStepPending(st, home);
  const checklist = [...chapterOf[0] === "Landfall"
    ? [{ title: HOME_STEP_TITLE, done: !homeStep }, { title: HOME_WORLD_STEP_TITLE, done: !homeStep && !worldStep }] : [],
    ...chapterOf[1].filter(told).map(([s, title]) => ({ title: withPirateNames(title, f), done: step > FOUNDING_STEP[s] }))];
  const chapterName = CHAPTER_TOLD[chapterOf[0]]?.(f) ?? true ? chapterOf[0] : COVER_NAME[chapterOf[0]] ?? chapterOf[0];

  switch (step) {
    // ─── Landfall: the colony ship lands and becomes the Command Center ───────
    // §habitat-first (Nov 8 2026): a home for the settlers, light for the
    // colony, then the food chain on its power.
    case FOUNDING_STEP.build_habitat: {
      const homeName = st.galaxy?.systems.find(s => s.id === home?.id)?.name;
      const worldName = landingBody(home)?.name;
      primary = homeStep
        ? { title: HOME_STEP_TITLE, copy: `Click ${homeName ?? "your home system"} on the map.`, action: "open-home", label: `Open ${homeName ?? "home"}` }
        : worldStep
          ? { title: HOME_WORLD_STEP_TITLE, copy: `Click ${worldName ?? "your home world"} in the system.`, action: "home-world", label: `Open ${worldName ?? "it"}` }
          : structure("habitat", "Habitat", { title: "Build a Habitat", copy: kit("habitat"),
            hint: "No power needed: the colony ship's reactor keeps your settlers alive.", action: "build-habitat", label: "Build" }, false);
      break;
    }
    case FOUNDING_STEP.power_shipyard:
      // §charter-v6 (Peter, Nov 7 2026): the Command Center's reactor runs the
      // colony, not the mine or the Smelter; §habitat-first, nor the farms.
      primary = building("power_plant", "Solar Array", "Then the farms.", landing?.id)
        ?? viaMerchant(st, home, "power_plant", "Solar Array", { title: "Build a Solar Array", copy: kit("power_plant"),
          why: "Light the colony.",
          hint: `+8 MW on ${world}: enough for the farms, the mine and the Smelter.`, action: "build-power", label: "Build" });
      break;
    case FOUNDING_STEP.build_bioharvester:
      primary = structure("bioharvester", "Bioharvester", { title: "Build a Bioharvester", copy: kit("bioharvester"),
        hint: "Biomass for the Agroplex · 1 MW. Your settlers live on the Landfall crate's Provisions.", action: "build-bioharvester", label: "Build" }, true);
      break;
    case FOUNDING_STEP.build_agroplex:
      primary = structure("agroplex", "Agroplex", { title: "Build an Agroplex", copy: kit("agroplex"),
        hint: "Biomass → Provisions · 1 MW. The crate's Provisions won't last.", action: "build-agroplex", label: "Build" }, true);
      break;
    case FOUNDING_STEP.build_mine: {
      // §charter-v6 spoke the goal here; Peter (Nov 8 2026) removed the line.
      primary = structure("mining_complex", "Mining Complex", { title: "Build a Mining Complex", copy: kit("mining_complex"),
        hint: `On ${world}'s Ferrite Ore, on the Solar Array's power.`, action: "build-mine", label: "Build" }, true);
      break;
    }

    // ─── Raid: the privateer takes the kit's semiconductors and Armaments (the
    // story is a pop-up, story-events.ts; the guide keeps to what happens) ────
    case FOUNDING_STEP.pirate_raid:
      // Not news until command sees it: the Smelter's own moment.
      // §smelter-raid (Peter, Nov 9 2026): the privateer sets out on the
      // Smelter's order, so it is usually in sight by now (§watch-the-pirate).
      primary = { title: "The Smelter is running", copy: "", action: "smelter", label: "Open the Smelter", quiet: true };
      break;
    case FOUNDING_STEP.build_gun_platform:
      // §gun-platform (Peter, Oct 30 2026): straight after the raid, the
      // crudest defense, from kit Alloys and Machinery.
      primary = building("orbital_defense", "Laser Battery", "", landing?.id)
        ?? { title: "Build a Laser Battery", copy: kit("orbital_defense"), why: "Pirates leave defended worlds alone.",
          hint: `One salvaged gun over ${world}. It joins every fight in the system.`, action: "build-gun-platform", label: "Build" };
      break;
    case FOUNDING_STEP.sell_ore: {
      const need = st.galaxy?.build_options?.find(o => o.key === "shipyard")?.costs.find(c => c.commodity === "electronics")?.units ?? 0;
      // The trader is news only once its pop-up has told it (after the
      // server says it docked): until then the guide never names it.
      // §charter-v6: the trader fronts a reactor and a drive — an
      // Interceptor's, not a freighter's.
      primary = traderTold(st) && dockedMerchant(st)
        ? { title: "Sell Alloys to the trader", copy: "It fronts a reactor and a drive: an Interceptor's, not a freighter's.", action: "merchant-sell", label: "Sell" }
        : { title: "Find semiconductors", copy: `The Shipyard needs ${need}.`, action: "market", label: "Open Market" };
      break;
    }
    case FOUNDING_STEP.build_smelter: {
      // §smelt-from-landing (Peter, Nov 6 2026: "should they start with the
      // ability to smelt?"): Ferrite into Alloys, from the colony ship's
      // goods, before the Interceptor its Alloys build. No research. Peter:
      // "maybe we should do it before the shipyard", then "before the solar
      // array too": its 2 MW slow it and the mine until the next step.
      // §charter-v6 (Nov 7 2026): step 6, before the raid: ore to Alloys
      // begins, and the Alloys are the first cargo.
      // §smelter-raid (Peter, Nov 9 2026): the privateer sets out on the
      // Smelter's order, so the raid can come before it runs
      // (§watch-the-pirate takes the card then).
      primary = structure("smelter", "Smelter", { title: "Smelt your first Alloys", copy: kit("smelter"), why: "Your first cargo for the Market Hub.",
        hint: "Ferrite Ore → Alloys. Every build ahead needs Alloys too. It keeps 50 ore unsmelted for the trader.", action: "build-smelter", label: "Build" }, true);
      break;
    }
    case FOUNDING_STEP.build_shipyard:
      primary = structure("shipyard", "Shipyard", viaMerchant(st, home, "shipyard", "Shipyard", { title: "Build a Shipyard", copy: kit("shipyard"),
        why: "For the ship that gets your engines back.", hint: "It builds every ship that comes next.", action: "build-shipyard", label: "Build" }), true);
      break;

    // ─── Pirates: the Interceptor, the privateer and its den ───────────────────
    case FOUNDING_STEP.build_interceptor: {
      // Peter (Oct 25 2026): Shipyard I builds it. Peter (Oct 30 2026): its
      // captain isn't optional; the step completes once one commands it.
      const hunter = own.find(g => guardCapable(g));
      const free = st.captains.find(c => inReserve(c, st.simTime));
      const boarding = !!hunter && (captained(st, hunter)
        || !!st.pendingOrders.get(hunter.id)?.some(o => !o.lost && o.kind === "assign"));
      // A captain boards only a ship at rest: not one on Guard or Defend duty.
      const atRest = !!hunter && !hunter.guard_target && !hunter.defend_system;
      const captain = hunter && !boarding && atRest ? reserveCaptain(st, hunter) : undefined;
      if (!hunter || (!free && !boarding)) {
        primary = building("raider", "Interceptor", "Then assign its captain.")
          ?? foundingIndustryGoal(home, "shipyard", viaMerchant(st, home, "raider", "Interceptor", { title: "Build an Interceptor", copy: kit("raider"),
            hint: "The privateer took your engines to its den.", action: "build-interceptor", label: "Build" }));
      } else if (boarding) {
        primary = waitOn("Captain boarding", null, { copy: "", action: "raider", label: "Select Interceptor" });
      } else if (captain) {
        primary = assignCaptain(captain);
      } else {
        // The free captain waits at its station; the Interceptor is away or on duty.
        const station = st.galaxy?.systems.find(s => s.id === free?.stationed_system);
        primary = { title: station && dockedAtSystem(hunter, station.id) ? "Stand the Interceptor down"
          : station?.id === home?.id ? "Bring the Interceptor home" : `Send the Interceptor to ${station?.name ?? "its captain"}`,
          copy: `${free!.name} boards at the dock.`, action: "interceptor-captain", label: "Send" };
      }
      break;
    }
    case FOUNDING_STEP.defeat_privateer: {
      const hunter = own.find(g => guardCapable(g));
      // §den-retreat (Peter, Nov 12 2026: "let's have the first pirate raid
      // retreat back to their den instead of the hunting area"; "One assault
      // at the den"): home from his raid he joins his den's crew, so the hunt
      // is one assault on the den, known as it begins. Seen away from it, he
      // is still flying home.
      const den = foundingDen(st);
      const atDen = (p: { x: number; y: number }) => !!den && Math.hypot(p.x - den.pos.x, p.y - den.pos.y) <= 3_000;
      const seen = f.privateer ? st.ghosts.find(g => g.id === f.privateer) : undefined;
      const pirate = seen && !atDen(seen.pos) ? seen : undefined;
      const fighting = !!hunter && st.battles.some(b => b.own && b.participants.includes(hunter.id));
      const ordered = (kind: string) => !!hunter && !!st.pendingOrders.get(hunter.id)?.some(o => !o.lost && o.kind === kind);
      const captain = hunter && !ordered("assign") ? reserveCaptain(st, hunter) : undefined;
      // An empty tank at home: the trader's fuel fills it at the dock.
      // §reactors-drives: the tank its reactor burns (Uranium on Fission),
      // and only while the trader stocks that fuel.
      const tank = hunter ? regimeStore(hunter) : undefined;
      const dry = !!tank && tank.served != null && tank.capacity > 0 && tank.have < 1 && atHome(hunter)
        && !!dockedMerchant(st)?.goods.some(g => g.commodity === tank.fuel && g.stock > 0);
      // §privateer-contract (Peter, Nov 9 2026): the Authority's contract
      // on him opens the hunt, the charter's first contract: taken on in
      // Contracts, it pays on his kill's report.
      const contract = st.operations.find(o => o.briefing?.follow_up === "founder_privateer");
      const pays = contract ? `Contract: ${contract.reward.credits.toLocaleString()} Cr.` : "Bounty: 8,000 Cr.";
      const attack: FoundingStep = { title: "Destroy the privateer", copy: "", reason: pays,
        action: "attack-privateer", label: "Attack" };
      // §privateer-hunt (Oct 27 2026): out of sight after its raid, it lies
      // quiet where it went to ground; the Interceptor finds it there. Only
      // a home without a den now: with one, that is the den.
      const lastSeen = !pirate && !den ? f.privateer_last_seen ?? null : null;
      const hunt: FoundingStep = { title: "Find the privateer", copy: "Search its last known position.", reason: pays,
        action: "hunt-privateer", label: "Hunt" };
      if (!hunter) {
        blocked = true;
        primary = building("raider", "Interceptor", "Then hunt the privateer.")
          ?? { title: "Interceptor lost", copy: kit("raider"), action: "build-interceptor", label: "Build Interceptor" };
      } else if (fighting) {
        primary = waitOn(den && atDen(hunter.pos) ? "Interceptor fighting at the den" : "Interceptor engaging the privateer", null,
          { copy: "", action: den && atDen(hunter.pos) ? "den" : "privateer", label: "Watch" });
      } else if (dry && traderTold(st)) {
        blocked = true;
        primary = { title: "Fuel the Interceptor", copy: "Its tank is empty.", action: "merchant-fuel", label: `Buy ${label(tank!.fuel)}` };
      } else if (captain) {
        // Peter (Oct 30 2026): not optional. A replacement Interceptor takes
        // the reserve captain before it hunts.
        primary = assignCaptain(captain);
      } else if (contract?.state === "offered" && !contract.joined) {
        primary = { title: "Take the Authority's contract", copy: contract.briefing?.title ?? "", reason: pays,
          action: "contract", label: "Open Contracts" };
      } else if (pirate && (ordered("attack") || heading(hunter, pirate.pos, 3_000, st))) {
        primary = waitOn("Interceptor closing on the privateer", `ETA ${eta(hunter, pirate.pos)}`, { copy: "", action: "privateer", label: "Watch" });
      } else if (pirate) {
        primary = attack;
      } else if (den && Math.hypot(hunter.pos.x - den.pos.x, hunter.pos.y - den.pos.y) <= 1_500) {
        primary = waitOn("Interceptor at the den", den.name, { copy: "", action: "den", label: `Open ${den.name}` });
      } else if (den && heading(hunter, den.pos, 1_500, st)) {
        primary = waitOn(`Interceptor en route to ${den.name}`, `ETA ${eta(hunter, den.pos)}`, { copy: "", action: "den", label: `Open ${den.name}` });
      } else if (den) {
        primary = { title: `Attack the den at ${den.name}`, copy: "The privateer and his crew: three raiders. No defenses.",
          reason: pays, action: "assault-den", label: "Send Interceptor" };
      } else if (lastSeen && Math.hypot(hunter.pos.x - lastSeen.x, hunter.pos.y - lastSeen.y) <= PRIVATEER_HUNT_SU) {
        primary = waitOn("Interceptor lying in wait", null, { copy: "It attacks on contact.", action: "raider", label: "Select Interceptor" });
      } else if (lastSeen && heading(hunter, lastSeen, PRIVATEER_HUNT_SU, st)) {
        primary = waitOn("Interceptor hunting the privateer", `ETA ${eta(hunter, lastSeen)}`, { copy: "", action: "raider", label: "Watch" });
      } else if (lastSeen) {
        primary = hunt;
      } else {
        primary = { title: "Destroy the privateer", copy: "It's out of sight.", action: "raider", label: "Select Interceptor" };
      }
      break;
    }
    case FOUNDING_STEP.eliminate_den: {
      const den = foundingDen(st);
      const hunter = own.find(g => guardCapable(g));
      const fighting = !!hunter && st.battles.some(b => b.own && b.participants.includes(hunter.id));
      const near = !!den && !!hunter && Math.hypot(hunter.pos.x - den.pos.x, hunter.pos.y - den.pos.y) <= 1_500;
      // §den-contract (Peter, Nov 5 2026): the Authority's contract on it,
      // with the kill's report.
      const contract = st.operations.find(o => o.briefing?.follow_up === "founder_den" && o.state === "active");
      const pays = contract ? `Contract: ${contract.reward.credits.toLocaleString()} Cr.` : "It holds what the raid took.";
      const terms = contract ? "Authority contract. The den holds what the raid took." : undefined;
      if (!den) {
        primary = { title: "Eliminate the privateer den", copy: "Its base is unknown.", action: "log", label: "Open Log" };
      } else if (!hunter) {
        blocked = true;
        primary = building("raider", "Interceptor", `Then send it to ${den.name}.`)
          ?? { title: "Interceptor lost", copy: kit("raider"), action: "build-interceptor", label: "Build Interceptor" };
      } else if (fighting || near) {
        primary = waitOn(`Interceptor ${fighting ? "fighting the garrison" : "at the den"}`, den.name,
          { copy: "", action: "den", label: `Open ${den.name}` });
      } else if (heading(hunter, den.pos, 1_500, st)) {
        primary = waitOn(`Interceptor en route to ${den.name}`, `ETA ${eta(hunter, den.pos)}`, { copy: "", action: "den", label: `Open ${den.name}` });
      } else {
        // §den-retreat: the privateer fell away from it, so its crew alone.
        primary = { title: `Destroy the den at ${den.name}`, copy: "Two damaged raiders. No defenses.",
          reason: pays, hint: terms, action: "assault-den", label: "Send Interceptor" };
      }
      break;
    }

    // ─── Expansion: the den's aftermath, an export, research, a second mine ───
    // §stash (Peter, Nov 3 2026: "split it for this hauling exercise"): the
    // Interceptor the den's blast mauled comes home and is repaired (13), the
    // first Tiny Freighter (14) switches to Full speed (15) and hauls back the
    // den's loot (16). They overlap: step 13 offers the freighter's next order
    // while you wait.
    case FOUNDING_STEP.repair_interceptor:
    case FOUNDING_STEP.build_tiny_freighter:
    case FOUNDING_STEP.full_speed:
    case FOUNDING_STEP.haul_loot: {
      const den = foundingDen(st);
      const stash = f.stash;
      const hull = (g: GhostView) => `${Math.round(100 * (1 - (g.damage ?? 0)))}%`;
      const hurt = own.find(g => f.den_wounded?.includes(g.id));
      const carrier = stash?.fleet ? own.find(g => g.id === stash.fleet) : undefined;
      // Repairs are automatic: a staffed Shipyard mends what is docked at home.
      const mends = "Ships docked at a staffed Shipyard repair on their own.";
      let repair: FoundingStep | null = null;
      // §den-contract (Peter, Nov 5 2026): the yard spends Alloys and
      // Machinery, none of which is home before the den's first load; the
      // trader sells them.
      const trader = traderTold(st) ? dockedMerchant(st) : null;
      const parts = trader && home ? Object.entries(merchantShortfall(st, home, "repair", trader))
        .map(([c, n]) => `${n} ${label(c)}`).join(" · ") : "";
      if (hurt && atHome(hurt)) {
        repair = !staffed("shipyard")
          ? { title: "Staff the Shipyard", copy: staffCopy(home), hint: mends, action: STAFF_ACTION.shipyard, label: "Assign workforce" }
          : parts
            ? { title: "Buy the repair parts", copy: parts, reason: `${hull(hurt)} hull`, hint: `${mends} They spend Alloys and Machinery.`,
              action: "merchant-repair", label: "Buy from trader" }
            // §watch-repair (Peter, Nov 11 2026: "a clear step with a Watch
            // button ... brings the player to the planetary view so they
            // can see the ship being repaired"): it lands on the yard's pad.
            : { title: "Watch the repair", copy: `Interceptor · ${hull(hurt)} hull`, hint: mends, action: "watch-repair", label: "Watch" };
        // The later steps overlap it: there it is a line under way.
        if (repair.action === "watch-repair" && step !== FOUNDING_STEP.repair_interceptor) progress.push(`Repairing the Interceptor · ${hull(hurt)}`);
      } else if (hurt && homePos && heading(hurt, homePos, 2_500, st)) {
        repair = waitOn("Interceptor heading home", `${hull(hurt)} hull · ETA ${eta(hurt, homePos)}`,
          { copy: "", hint: mends, action: "raider", label: "Select Interceptor" });
      } else if (hurt) {
        // §return-home (Peter, Nov 5 2026): it holds at the den until sent.
        repair = { title: "Return to Home", copy: `Interceptor · ${hull(hurt)} hull`, why: "The Shipyard repairs it.", hint: mends,
          action: "repair-home", label: "Return to Home" };
      }
      // The haul's track: the freighter, then the trips. §den-stores (Peter,
      // Nov 5 2026: "if they need to make two trips in their freighter so be
      // it!"): a hold takes 50 room, the rest waits at the den. §hold-room
      // (Nov 10 2026): by room the den's loot is one trip.
      let build: FoundingStep | null = null;
      let haul: FoundingStep | null = null;
      if (stash && !stash.home && den) {
        const units = (goods?: Partial<Record<Commodity, number>>) => Object.values(goods ?? {}).reduce((a, n) => a + (n ?? 0), 0);
        const left = stash.left ?? stash.goods;
        const aboard = Object.fromEntries(Object.entries(stash.goods).map(([c, n]) => [c, (n ?? 0) - (left[c as Commodity] ?? 0)])
          .filter(([, n]) => (n as number) > 0)) as Partial<Record<Commodity, number>>;
        // §hold-room (Nov 10 2026): trips by room, so the den's loot is one.
        const trips = Math.ceil(basketRoom(left) / (TINY_HOLD * ROOM_SCALE));
        const more = trips ? `${units(left)} more at ${den.name}` : undefined;
        if (carrier && homePos) {
          haul = waitOn("Freighter bringing the loot home", `ETA ${eta(carrier, homePos)}`,
            { copy: goodsText(aboard), hint: more, action: "convoy", label: "Select Freighter" });
        } else if (freighter && heading(freighter, den.pos, 1_500, st)) {
          haul = waitOn(`Freighter en route to ${den.name}`, `ETA ${eta(freighter, den.pos)}`,
            { copy: goodsText(left), action: "convoy", label: "Select Freighter" });
        } else if (freighter) {
          haul = { title: "Fetch the loot", copy: goodsText(left), reason: `At ${den.name}.`, hint: trips > 1 ? `${trips} trips: a hold takes ${TINY_HOLD}.` : "It loads there and comes home.",
            action: "stash-fetch", label: "Fetch" };
        } else {
          build = building("tiny_freighter", "Tiny Freighter", "Then fetch the loot.")
            ?? foundingIndustryGoal(home, "shipyard", viaMerchant(st, home, "tiny_freighter", "Tiny Freighter", {
              title: "Build a Tiny Freighter", copy: kit("tiny_freighter"), reason: `Your engines are back. For the den's loot: ${units(stash.goods)} units.`,
              hint: trips > 1 ? `${trips} trips: a hold takes ${TINY_HOLD}.` : "One trip.", action: "build-convoy", label: "Build" }));
        }
      }
      const next = build ?? haul;
      // §full-speed (Peter, Nov 4 2026): a new freighter flies Slow, half
      // speed. §reactors-drives: every speed burns its reactor's fuel
      // (Uranium on Fission); Slow a quarter of it a distance.
      const speed: FoundingStep | null = freighter?.transit === "slow" && !carrier && !stash?.home
        ? { title: "Switch to Full speed", copy: "Tiny Freighter · Transit", why: "Twice as fast.",
          hint: `Slow is half speed on a quarter of the ${label(regimeFuel(freighter))}.`, action: "full-speed", label: "Open Freighter" }
        : null;
      if (step === FOUNDING_STEP.repair_interceptor) {
        primary = repair ?? waitOn("Interceptor repaired", null, { copy: "", action: "raider", label: "Select Interceptor" });
        if (next && !next.status) waiting.push(next);
      } else if (step === FOUNDING_STEP.build_tiny_freighter) {
        primary = build ?? waitOn("Tiny Freighter ready", null, { copy: "", action: "convoy", label: "Select Freighter" });
      } else if (step === FOUNDING_STEP.full_speed) {
        primary = speed ?? waitOn("Full speed set", null, { copy: "", action: "convoy", label: "Select Freighter" });
      } else {
        primary = next ?? waitOn("Loot home", null, { copy: "", action: "home-world", label: `Open ${world}` });
      }
      break;
    }
    // §charter-v5 (Peter, Oct 16 2026): the ore goes to the Market Hub; the
    // trader stays for the charter (§den-contract).
    case FOUNDING_STEP.prepare_export: {
      // §charter-v6 (Nov 7 2026): the Smelter's Alloys, the charter's promise.
      const shipment = shipmentLine(st, freighter, homePos);
      const hubOre = st.market?.prices.find(p => p.commodity === "alloys")?.price;
      const reason = hubOre != null ? `The Market Hub pays ${hubOre.toFixed(1)} Cr an Alloy.` : "The Market Hub buys Alloys.";
      if (!freighter) {
        primary = building("tiny_freighter", "Tiny Freighter", "Then load it with Alloys.")
          ?? foundingIndustryGoal(home, "shipyard", { title: "Build a Tiny Freighter", copy: kit("tiny_freighter"),
            reason, hint: "It fuels with Uranium at the dock.", action: "build-convoy", label: "Build" });
      } else if (routeHold?.startsWith("Storage full") || (storageFull && !atHome(freighter))) {
        blocked = true;
        primary = { title: "Make room at home", copy: routeHold ?? `Storage is full (${Math.floor(home!.storage_used)}/${home!.storage_cap}).`,
          reason: "Build a Warehouse, or ship goods out.", action: "build-warehouse", label: "Build Warehouse" };
      } else if (followLeft != null) {
        // §export-follow (Peter, Nov 2 2026): the player follows the freighter
        // out of the well (boot.ts puts the camera on it); the step completes
        // when it warps at the hyperlimit, EXPORT_FOLLOW_MAX_S after departure
        // at most. Follow re-engages the camera after a drag.
        primary = waitOn("Follow the Freighter", `warp in ${fmtDur(followLeft)}`, { copy: "", action: "follow-shipment", label: "Follow" });
      } else if (routeHold) {
        primary = { title: "Shipment waiting", copy: routeHold, action: "shipment-trip", label: "Open Logistics", quiet: true };
      } else if (shipment) {
        // Its line joins the under-way list below (the trip stays in view after this step).
        primary = { title: shipment.split(" · ")[0], status: shipment, copy: "", action: "shipment-trip", label: "Open Logistics" };
      } else {
        primary = { title: "Export your Alloys", copy: "", reason, action: "opening-imports", label: "Plan the shipment" };
      }
      break;
    }
    case FOUNDING_STEP.first_research: {
      // §freight-frames (Peter, Nov 6 2026): Freight Frames, for the Small
      // Freighter it unlocks (Enrichment and its Smelter before).
      const academy = foundingIndustryGoal(home, "academy", viaMerchant(st, home, "academy", "Academy", { title: "Build an Academy", copy: kit("academy"),
        hint: "Research unlocks better extraction, hulls and refining.", action: "build-academy", label: "Build" }));
      primary = building("academy", "Academy", `Then staff it and research ${CHARTER_RESEARCH_NAME}.`)
        ?? (built("academy") && staffed("academy") ? { title: `Research ${CHARTER_RESEARCH_NAME}`, copy: "Tier I · the Authority funds most of it",
          hint: "It unlocks the Small Freighter: three Tiny Freighter holds.", action: "research-charter", label: "Open Research" } : academy);
      break;
    }
    case FOUNDING_STEP.answer_signal: {
      // §signals (Peter, Nov 4 2026): while the research runs, a captained
      // fleet answers a signal near home.
      const research = st.research?.active;
      if (research?.id === CHARTER_RESEARCH) progress.push(`${CHARTER_RESEARCH_NAME} under way${research.eta_secs != null ? ` · ${fmtDur(research.eta_secs)}` : ""}`);
      primary = { title: "Answer a signal", copy: "Interceptor → a signal near home", why: "It sweeps it and comes home.",
        hint: "Faint contacts near home. A captained fleet sweeps one and brings back what it finds.", action: "signals", label: "Open Signals" };
      break;
    }
    case FOUNDING_STEP.build_small_freighter: {
      // §freight-frames (Peter, Nov 6 2026): the hull the research unlocks,
      // three Tiny Freighter holds for the Hub runs.
      const research = st.research?.active;
      if (!st.research?.programmes.some(p => p.id === CHARTER_RESEARCH && p.state === "completed")) {
        primary = research?.id === CHARTER_RESEARCH
          ? waitOn(`${CHARTER_RESEARCH_NAME} under way`, research.eta_secs != null ? fmtDur(research.eta_secs) : null,
            { copy: "Then a Small Freighter.", action: "research-charter", label: "Open Research" })
          : { title: `Research ${CHARTER_RESEARCH_NAME}`, copy: "Tier I", hint: "It unlocks the Small Freighter.", action: "research-charter", label: "Open Research" };
        break;
      }
      primary = building("small_freighter", "Small Freighter", "")
        ?? viaMerchant(st, home, "small_freighter", "Small Freighter", { title: "Build a Small Freighter", copy: kit("small_freighter"),
          why: "150 cargo.", hint: "Three Tiny Freighter holds a trip to the Market Hub.", action: "build-small-freighter", label: "Build" });
      break;
    }
    case FOUNDING_STEP.expand_mining: {
      const body = home ? expansionBody(home) : undefined;
      if (!body) {
        blocked = true;
        primary = { title: "No free ore world", copy: "Every other world's resource sites are taken.", action: "home-world", label: `Open ${world}` };
        break;
      }
      const ore = body.deposits?.find(d => MINERAL_DEPOSITS.includes(d.resource))?.resource;
      const grant = "The charter's land grant now covers your whole system.";
      // §refining: the obstacle the second mine answers, once it shows.
      const reason = home?.converters?.some(c => c.structure === "smelter" && c.status === "no_inputs")
        ? "The Smelter is short of ore." : undefined;
      if (body.kind === "asteroid_belt") {
        // §asteroid-quarry: a belt's mine crews and powers itself.
        primary = building("asteroid_quarry", "Asteroid Quarry", "", body.id)
          ?? viaMerchant(st, home, "asteroid_quarry", "Asteroid Quarry", { title: `Quarry ${body.name}`, copy: kit("asteroid_quarry"), reason,
            hint: `${grant} It mines every deposit on the belt with its own crew and power, and eats Provisions and Machinery.`,
            action: "build-expansion-quarry", label: "Build Asteroid Quarry" });
      } else if (!(body.structures.power_plant > 0)) {
        primary = building("power_plant", "Solar Array", `Then a Mining Complex on ${body.name}.`, body.id)
          ?? viaMerchant(st, home, "power_plant", "Solar Array", { title: `Power ${body.name}`, copy: kit("power_plant"), reason,
            hint: `${grant} A mine there needs its own power.`, action: "build-expansion-power", label: "Build Solar Array" });
      } else if (!(body.structures.mining_complex > 0)) {
        primary = building("mining_complex", "Mining Complex", "Then staff it.", body.id)
          ?? viaMerchant(st, home, "mining_complex", "Mining Complex", { title: `Mine ${body.name}`, copy: kit("mining_complex"), reason,
            hint: `${grant} It mines ${ore ? label(ore) : "ore"} there.`, action: "build-expansion-mine", label: "Build Mining Complex" });
      } else {
        primary = { title: `Staff ${body.name}'s mine`, copy: staffCopy(home), reason, hint: UNSTAFFED, action: "expansion-mine", label: "Assign workforce" };
      }
      break;
    }
    case FOUNDING_STEP.survey_candidates: {
      const stop = f.survey_candidates[0];
      const stopName = st.galaxy?.systems.find(s => s.id === stop)?.name ?? "the nearby stop";
      const stopPos = st.galaxy?.systems.find(s => s.id === stop)?.pos;
      const active = stopPos ? own.find(g => surveyCapable(g) && surveyingAt(g, stopPos, st)) : undefined;
      const surveyor = freeSurveyor(own);
      // §charter-v4: the den's system, whose belt the den guarded.
      const why = stop && stop === f.den ? "The den guarded a rich belt there." : undefined;
      if (active && stopPos) {
        const noun = surveyorNoun(active);
        const card = { copy: "", action: noun === "Scout" ? "scout" : "probe", label: `Select the ${noun === "Scout" ? "Scout" : "probe"}` };
        primary = active.survey_progress != null
          ? waitOn(`Surveying ${stopName}`, `${Math.round(active.survey_progress * 100)}%`, card)
          : waitOn(`${noun} en route to ${stopName}`, `ETA ${eta(active, stopPos)}`, card);
      } else if (surveyor) {
        primary = { title: `Chart ${stopName}`, copy: "", why,
          hint: "Only a survey reveals what a system holds. A probe is spent when its report is sent.", action: "survey-stop", label: "Send the survey" };
      } else {
        primary = building("survey_probe", "Survey Probe", `Then send it to ${stopName}.`)
          ?? viaMerchant(st, home, "survey_probe", "Survey Probe", { title: "Build a Survey Probe", copy: kit("survey_probe"), why,
            hint: `It flies to ${stopName}, charts it and is spent.`, action: "build-probe", label: "Build" });
      }
      break;
    }

    // ─── Vendetta: the stores secured, then the privateer's uncle ──────────────
    case FOUNDING_STEP.secure_stores:
      // §charter-crates (Peter, Oct 31 2026): a Warehouse for the barge's ore
      // and a Vault the uncle's raid can't reach.
      primary = !built("warehouse")
        ? building("warehouse", "Warehouse", "Then the Vault.")
          ?? { title: "Build a Warehouse", copy: kit("warehouse"), why: "+700 storage.",
            hint: "Room for the barge's ore.", action: "build-store-warehouse", label: "Build" }
        : !built("vault")
          ? building("vault", "Vault", "", landing?.id)
            ?? { title: "Build a Vault", copy: kit("vault"), why: "Raiders can't reach it.",
              hint: "It keeps your 200 most valuable units out of any raid.", action: "build-vault", label: "Build" }
          : waitOn("Stores secured", null, { copy: "", action: "home-world", label: `Open ${world}` });
      break;
    case FOUNDING_STEP.vendetta: {
      const { uncle } = pirateNames(f);
      const v = f.vendetta;
      const pack = v?.fleet ? st.ghosts.find(g => g.id === v.fleet) : undefined;
      const warships = own.filter(g => guardCapable(g));
      const bounty = `Bounty: ${VENDETTA_BOUNTY.toLocaleString()} Cr and his flagship's salvage.`;
      // §vendetta-campaign: three Interceptors for his flagship and four raiders.
      const another: FoundingStep = building("raider", "Interceptor", `Keep it at ${colonyName(st)}.`)
        ?? viaMerchant(st, home, "raider", "Interceptor", { title: `Build ${interceptorOrdinal(warships.length)} Interceptor`, copy: kit("raider"),
          reason: `${uncle} brings his flagship and four raiders.`, action: "build-interceptor", label: "Build" });
      if (v?.fallen) {
        // §gun-platform (Peter, Oct 30 2026): game over, and a restart.
        blocked = true;
        primary = { title: `${colonyName(st)} has fallen`, copy: `${uncle} sacked the colony.`, action: "restart-corporation", label: "Restart" };
      } else if (v?.won != null) {
        primary = { title: v.won ? `${uncle} is dead` : `${uncle} got away`, copy: "", hint: "The charter completes as the report comes in.",
          action: "log", label: "Open Log", quiet: true };
      } else if (v?.arrived || pack) {
        if (pack && homePos) progress.push(v?.arrived ? `${uncle} over ${colonyName(st)}` : `${uncle} inbound · ETA ${eta(pack, homePos)}`);
        primary = foundingCommands("attack-uncle", st, home)
          ? { title: `Attack ${uncle}`, copy: "Every warship → his flagship", reason: bounty, action: "attack-uncle", label: "Attack" }
          : { title: `${uncle} is here`, copy: "Your warships at home fight him.", reason: bounty, action: "open-home", label: "Watch" };
      } else {
        if (v?.depart_at != null) progress.push(`${uncle} sets out in ${fmtDur(Math.max(0, v.depart_at - st.simTime))}`);
        const captain = warships.map(g => reserveCaptain(st, g) ? g : null).find(g => !!g);
        // Peter (Oct 30 2026): online if possible, never avoidable; if the
        // colony's defense can't hold, it falls.
        const when = `His countdown runs while you're online; he comes within a day regardless. If nothing at home still fights when he's done, the colony falls.`;
        primary = warships.length < VENDETTA_DEFENDERS ? { ...another, hint: when }
          : { title: "Hold your warships at home", copy: `Every warship at rest in ${colonyName(st)} and the Laser Battery join the fight.`,
            reason: bounty, hint: `${uncle} attacks your strongest warship. ${when}`, action: "fleets", label: "Open Fleets" };
        if (captain && warships.length >= VENDETTA_DEFENDERS) waiting.push({ title: "Assign a captain", copy: "A captain makes a warship hit harder.", action: "assign-captain", label: "Assign captain" });
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
        primary = building("mining_barge", "Mining Barge", `Then send it to ${beltName}.`);
        if (!primary) {
          primary = foundingBargeGoal(home, beltName, kit("mining_barge") || undefined);
          if (primary.action === "build-barge") primary = viaMerchant(st, home, "mining_barge", "Mining Barge", primary);
          if (f.stage !== "build_barge") primary = { ...primary, title: "Replace the Mining Barge", reason: "Your barge was lost." };
          // By now the food reserve and ore fill home storage; the kit needs room to land.
          if (storageFull && primary.action === "build-barge") {
            primary = { ...primary, reason: `Home storage is full (${Math.floor(home!.storage_used)}/${home!.storage_cap}): make room for the barge's kit.` };
          }
        }
      } else if (f.stage === "prospect_belt") {
        // §den-pocket (Peter, Oct 30 2026): a prospecting pass finds what the
        // den guarded.
        const pass = barge.industry?.kind === "prospect" ? barge.industry : null;
        if (pass && pass.status !== "travelling") {
          primary = waitOn(`Prospecting ${beltName}`, pass.need ? `${Math.round(100 * Math.min(1, pass.dwell / pass.need))}%` : null,
            { copy: "", action: "barge", label: "Select Mining Barge" });
        } else if (pass && belt?.pos) {
          primary = waitOn(`Mining Barge en route to ${beltName}`, `ETA ${eta(barge, belt.pos)}`, { copy: "", action: "barge", label: "Select Mining Barge" });
        } else {
          primary = { title: `Prospect ${beltName}`, copy: "", hint: "A prospecting pass finds a belt's hidden pockets.",
            action: "prospect-belt", label: "Prospect" };
        }
      } else if (f.stage === "mine_belt") {
        const bound = !!belt?.pos && (work?.status === "travelling" || (!work && heading(barge, belt.pos, 1_500, st)));
        const target = belt?.pocket ?? belt?.ore;
        if (work && work.status !== "travelling") {
          primary = waitOn(`Mining Barge ${work.status === "mining" ? `cutting ${label(work.ore)}` : work.status} at ${beltName}`, null,
            { copy: "", hint: "This step completes when its report reaches command.", action: "barge", label: "Select Mining Barge" });
        } else if (bound && belt?.pos) {
          primary = waitOn(`Mining Barge en route to ${beltName}`, `ETA ${eta(barge, belt.pos)}`, { copy: "", action: "barge", label: "Select Mining Barge" });
        } else if (belt?.pos && Math.hypot(barge.pos.x - belt.pos.x, barge.pos.y - belt.pos.y) <= 1_500) {
          // Peter (Nov 4 2026): after the prospecting pass it is already there;
          // the pass only searched, and cutting is its own order.
          primary = { title: `Mine the ${target ? label(target) : "ore"}${belt.pocket ? " pocket" : ""}`, copy: beltName,
            hint: "Prospecting only searches. The barge cuts ore into its own hold. Command hears when it starts.", action: "mine-belt", label: "Mine" };
        } else {
          primary = { title: "Send the barge to the belt", copy: `${beltName} · ${target ? label(target) : "ore"}${belt?.pocket ? " pocket" : ""}`,
            hint: "The barge cuts ore into its own hold. Command hears when it starts.", action: "mine-belt", label: "Send" };
        }
      } else {
        // haul_ore: home, unload.
        const homeBound = !!homePos && !atHome(barge) && !work && heading(barge, homePos, 2_500, st);
        if (atHome(barge) && aboard > 0) {
          primary = { title: "Unload the ore", copy: `${aboard} units → home stockpile`, action: "barge-unload", label: "Unload" };
        } else if (homeBound && homePos) {
          primary = waitOn(`Mining Barge bringing ${aboard} units home`, `ETA ${eta(barge, homePos)}`, { copy: "", action: "barge", label: "Select Mining Barge" });
        } else if (work) {
          progress.push(`Mining Barge ${work.status === "full" ? "hold full" : `cutting ${label(work.ore)}`} at ${beltName} · ${aboard} units aboard`);
          primary = { title: "Bring the ore home", copy: work.status === "full" ? "" : "Or let it fill its hold first.",
            hint: "Ore earns nothing until it is unloaded or sold.", action: "barge-home", label: "Send the barge home" };
        } else {
          primary = { title: "Bring the ore home", copy: "Or sell it at the Market Hub.", action: "barge-home", label: "Send the barge home" };
        }
      }
    }
  }
  // §watch-the-pirate: the raid in view takes the card, alone. Peter (Nov 9
  // 2026): "let's get rid of the While you wait · 1 when the pirate is
  // raiding".
  const watch = watchCard();
  if (watch) {
    primary = watch;
    waiting.length = 0;
  }
  // §crate-panel (Peter, Nov 3 2026): a released charter crate is no step; it
  // pops up to be accepted (story-events `crateEvent`).
  // §vendetta: from step 22 the uncle's oath stands and the guide readies
  // the colony, without saying when he comes (Peter, Oct 28 2026: "why are we
  // giving away to the player what happens here").
  if (step >= FOUNDING_STEP.survey_candidates && step <= FOUNDING_STEP.secure_stores && f.vendetta) {
    const { uncle } = pirateNames(f);
    progress.unshift(`${uncle} swore revenge`);
    // One under way shows as its progress line, not as something to do.
    const warships = own.filter(g => guardCapable(g)).length;
    if (warships < VENDETTA_DEFENDERS && !building("raider", "Interceptor", "")) {
      waiting.push(viaMerchant(st, home, "raider", "Interceptor",
        { title: `Build ${interceptorOrdinal(warships)} Interceptor`, copy: kit("raider"), action: "build-interceptor", label: "Build Interceptor" }));
    }
  }
  // §vendetta-campaign (Peter, Oct 30 2026: "the uncle should harass the
  // player's expanded mining ops"): a strike in sight comes before the step.
  const strike = step >= FOUNDING_STEP.survey_candidates ? f.vendetta?.strikes?.find(s => !s.done && s.fleet && st.ghosts.some(g => g.id === s.fleet)) : undefined;
  if (strike) {
    const { uncle } = pirateNames(f);
    const reason = `Bounty: ${STRIKE_BOUNTY} Cr.`;
    blocked = true;
    if (strike.kind === "mine") {
      // §gun-platform (Peter, Oct 30 2026): it wrecks the world, it steals nothing.
      const world = home?.bodies.find(b => b.id === strike.body)?.name ?? "your second mine";
      primary = { title: strike.over ? `Raider over ${world}` : `Raider closing on ${world}`,
        copy: strike.over ? (strike.wrecked?.length ? `${strike.wrecked.map(k => k === "power_plant" ? "Solar Array" : label(k)).join(", ")} wrecked`
          : "It's wrecking the mine.") : "",
        reason, hint: `${uncle}'s raider wrecks one structure every 30 s, the mine first.`, action: "attack-strike", label: "Attack" };
    } else if (strike.kind === "freighter") {
      primary = { title: "Raider hunting your freighter", copy: "Dock it, or fight.", reason,
        hint: `${uncle}'s raider. A freighter docked or in a gravity well is safe.`, action: "attack-strike", label: "Attack" };
    } else {
      primary = { title: "Raiders on the barge", copy: "", reason, hint: `Two of ${uncle}'s raiders.`, action: "defend-barge", label: "Send Interceptors" };
    }
  } else if (step >= FOUNDING_STEP.survey_candidates && step < FOUNDING_STEP.vendetta) {
    // §gun-platform: a wrecked second mine, until it stands again.
    const hit = f.vendetta?.strikes?.find(s => s.kind === "mine" && s.wrecked?.length);
    const body = hit ? home?.bodies.find(b => b.id === hit.body) : undefined;
    if (body && !(body.structures.mining_complex > 0) && !(body.structures.asteroid_quarry > 0)) {
      const title = `Rebuild ${body.name}'s mine`;
      const reason = "The Smelter needs its ore.";
      const rebuild: FoundingStep = body.kind === "asteroid_belt"
        ? building("asteroid_quarry", "Asteroid Quarry", "", body.id)
          ?? { title, copy: kit("asteroid_quarry"), reason, action: "build-expansion-quarry", label: "Build Asteroid Quarry" }
        : !(body.structures.power_plant > 0)
          ? building("power_plant", "Solar Array", `Then ${body.name}'s mine.`, body.id)
            ?? { title, copy: kit("power_plant"), reason, action: "build-expansion-power", label: "Build Solar Array" }
          : building("mining_complex", "Mining Complex", "Then staff it.", body.id)
            ?? { title, copy: kit("mining_complex"), reason, action: "build-expansion-mine", label: "Build Mining Complex" };
      // One under way shows as its progress line (`building` adds it).
      if (!rebuild.status) waiting.unshift(rebuild);
    }
  }
  if (queued("mining_complex") && step !== FOUNDING_STEP.build_mine && step !== FOUNDING_STEP.expand_mining) progress.push("Mining Complex under construction");
  // The first shipment's trip stays in view after its step: it brings parts home.
  // While the player follows it out of the well, the follow card is its one clock.
  const trip = step >= FOUNDING_STEP.prepare_export && followLeft == null ? shipmentLine(st, freighter, homePos) : null;
  if (trip) progress.unshift(trip);
  // A waiting card's own line is its title, not a second line above it.
  const echo = primary?.status ? progress.indexOf(primary.status) : -1;
  if (echo >= 0) progress.splice(echo, 1);
  // §vendetta: the family's names wherever the guide says "the privateer".
  const name = (text: string) => withPirateNames(text, f);
  const named = (card: FoundingStep | null): FoundingStep | null => card && { ...card, title: name(card.title), copy: name(card.copy),
    reason: card.reason && name(card.reason), why: card.why && name(card.why), hint: card.hint && name(card.hint), status: card.status && name(card.status),
    alternatives: card.alternatives?.map(a => ({ ...a, title: name(a.title), copy: name(a.copy), label: name(a.label) })) };
  return { chapter: { name: chapterName, done: checklist.filter(s => s.done).length, total: checklist.length }, checklist,
    progress: progress.map(line => withPirateNames(line, f)), primary: named(primary), waiting: waiting.map(card => named(card)!),
    blocked };
}

/** §charter-v4: the guide's staffing actions by structure. */
const STAFF_ACTION: Record<string, string> = {
  bioharvester: "bioharvester", agroplex: "agroplex", mining_complex: "mine", shipyard: "shipyard", academy: "academy",
  smelter: "smelter",
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
    : target.at === "store" ? bestStoreBody(home)
    : bestShipyardBody(home);
  return body ? { body, mode: target.mode, select: target.key } : null;
}

const BUILD_ACTIONS: Record<string, { key: string; mode: "structures" | "ships"; at: "landing" | "mine" | "academy" | "shipyard" | "yard" | "expansion" | "store" }> = {
  "build-bioharvester": { key: "bioharvester", mode: "structures", at: "landing" },
  "build-habitat": { key: "habitat", mode: "structures", at: "landing" },
  "build-agroplex": { key: "agroplex", mode: "structures", at: "landing" },
  "build-power": { key: "power_plant", mode: "structures", at: "landing" },
  // §gun-platform: over the landing world.
  "build-gun-platform": { key: "orbital_defense", mode: "structures", at: "landing" },
  "build-warehouse": { key: "warehouse", mode: "structures", at: "landing" },
  // §charter-crates: step 27's Warehouse keeps the landing world's last site
  // free; the Vault is dug under its Command Center.
  "build-store-warehouse": { key: "warehouse", mode: "structures", at: "store" },
  "build-vault": { key: "vault", mode: "structures", at: "landing" },
  "build-mine": { key: "mining_complex", mode: "structures", at: "mine" },
  "build-academy": { key: "academy", mode: "structures", at: "academy" },
  // §refining: inside the land grant still, so the landing world's free
  // industrial site, beside the Shipyard.
  "build-smelter": { key: "smelter", mode: "structures", at: "landing" },
  "build-shipyard": { key: "shipyard", mode: "structures", at: "shipyard" },
  "build-expansion-power": { key: "power_plant", mode: "structures", at: "expansion" },
  "build-expansion-mine": { key: "mining_complex", mode: "structures", at: "expansion" },
  "build-expansion-quarry": { key: "asteroid_quarry", mode: "structures", at: "expansion" },
  "build-convoy": { key: "tiny_freighter", mode: "ships", at: "yard" },
  // §freight-frames: the hull the charter's research unlocks.
  "build-small-freighter": { key: "small_freighter", mode: "ships", at: "yard" },
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
    // §vendetta: every warship near home at the uncle's pack.
    case "attack-uncle": {
      const pack = f.vendetta?.fleet;
      if (!pack || !st.ghosts.some(g => g.id === pack)) return null;
      const fleets = own.filter(g => guardCapable(g) && (!homePos || Math.hypot(g.pos.x - homePos.x, g.pos.y - homePos.y) <= 30_000));
      return fleets.length ? fleets.map(g => ({ type: "AttackFleet", fleet_id: g.id, target_id: pack })) : null;
    }
    // §charter-v4: the privateer, its captain and its den.
    case "attack-privateer":
      return escort && f.privateer && st.ghosts.some(g => g.id === f.privateer)
        ? [{ type: "AttackFleet", fleet_id: escort.id, target_id: f.privateer }] : null;
    // §privateer-hunt: to where it went to ground.
    case "hunt-privateer":
      return escort && f.privateer_last_seen && !st.ghosts.some(g => g.id === f.privateer)
        ? [{ type: "MoveShip", ship_id: escort.id, dest: f.privateer_last_seen }] : null;
    case "assign-captain": {
      const captain = escort ? reserveCaptain(st, escort) : undefined;
      return escort && captain ? [{ type: "AssignCaptain", captain_id: captain.id, fleet_id: escort.id }] : null;
    }
    // The free captain boards at its station: the Interceptor goes there.
    case "interceptor-captain": {
      const station = st.galaxy?.systems.find(s => s.id === st.captains.find(c => inReserve(c, st.simTime))?.stationed_system);
      return escort && station && !captained(st, escort) ? [{ type: "MoveShip", ship_id: escort.id, dest: station.pos }] : null;
    }
    // §vendetta-campaign: the warships near home at the mine's raider, and
    // every warship at the barge's.
    case "attack-strike":
    case "defend-barge": {
      const kind = action === "attack-strike" ? "mine" : "barge";
      const strike = f.vendetta?.strikes?.find(s => !s.done && s.kind === kind && s.fleet);
      const raider = strike ? st.ghosts.find(g => g.id === strike.fleet) : undefined;
      const fleets = own.filter(g => guardCapable(g) && (kind === "barge" || !homePos
        || Math.hypot(g.pos.x - homePos.x, g.pos.y - homePos.y) <= 30_000));
      return raider && fleets.length ? fleets.map(g => ({ type: "AttackFleet", fleet_id: g.id, target_id: raider.id })) : null;
    }
    case "assault-den": {
      const den = foundingDen(st);
      return escort && den ? [{ type: "MoveShip", ship_id: escort.id, dest: den.pos }] : null;
    }
    // §stash: the first freighter to the den, where it loads the loot and comes home.
    case "stash-fetch": {
      const den = foundingDen(st);
      return freighter && den && f.stash && !f.stash.fleet && !f.stash.home
        ? [{ type: "MoveShip", ship_id: freighter.id, dest: den.pos }] : null;
    }
    // §stash: the den's wounded to the Shipyard at home.
    case "repair-home": {
      const hurt = own.filter(g => f.den_wounded?.includes(g.id) && !(home && dockedAtSystem(g, home.id)));
      return hurt.length && homePos ? hurt.map(g => ({ type: "MoveShip", ship_id: g.id, dest: homePos })) : null;
    }
    case "cache-home": return freighter && homePos ? [{ type: "MoveShip", ship_id: freighter.id, dest: homePos }] : null;
    case "cache-unload": return freighter && home ? [{ type: "SystemUnload", fleet_id: freighter.id, system: home.id }] : null;
    case "scout-home": return scout && homePos ? [{ type: "MoveShip", ship_id: scout.id, dest: homePos }] : null;
    case "mine-belt": {
      const barge = own.find(g => mineCapable(g));
      const belt = foundingBelt(st);
      // §den-pocket: a found pocket before the belt's own ore.
      if (barge && belt?.pocket) {
        return [{ type: "MineBelt", fleet_id: barge.id, system_id: belt.system, body_id: belt.body, ore: belt.pocket, pocket: true }];
      }
      return barge && belt?.ore
        ? [{ type: "MineBelt", fleet_id: barge.id, system_id: belt.system, body_id: belt.body, ore: belt.ore }] : null;
    }
    case "prospect-belt": {
      const barge = own.find(g => mineCapable(g));
      const belt = foundingBelt(st);
      return barge && belt ? [{ type: "ProspectBelt", fleet_id: barge.id, system_id: belt.system, body_id: belt.body }] : null;
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

/** §charter-crates: where step 27's Warehouse goes: a home world with a free
 * site, the landing world last, so its last site stays free. */
export function bestStoreBody(system: SystemStateView): BodyView | undefined {
  const free = (b: BodyView) => { const p = bodyPoolUsage(b, system).infrastructure; return p.total - p.used; };
  const command = (b: BodyView) => Number((b.structures?.command_center ?? 0) > 0);
  return [...system.bodies].filter(b => b.kind !== "asteroid_belt" && free(b) > 0)
    .sort((a, b) => command(a) - command(b) || free(b) - free(a) || a.id - b.id)[0] ?? landingBody(system);
}

/** "35 Semiconductors · 15 Armaments". */
export function goodsText(goods: Partial<Record<Commodity, number>>): string {
  return Object.entries(goods).filter(([, n]) => (n ?? 0) > 0).map(([good, n]) => `${n} ${label(good)}`).join(" · ");
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

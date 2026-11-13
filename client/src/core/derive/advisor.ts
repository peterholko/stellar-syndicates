// §charter-advisor (Peter, Nov 8–9 2026; docs/charter-advisor.md): the Terran
// Charter Authority officer assigned to guide a new charter. The server deals
// one of three (`FoundingView.advisor`, game_loop `charter_advisor`); the
// cast, their lines and which line fits the guide's current action live
// here. They leave when the charter is fulfilled.
import type { FoundingStage, FoundingView } from "../../protocol";
import type { StoryEvent } from "../../story-events";
import { FOUNDING_CHAPTERS, FOUNDING_STEP, HOME_STEP_TITLE, HOME_WORLD_STEP_TITLE, withPirateNames,
  type FoundingPlan, type FoundingStep } from "./founding";

export type AdvisorSlug = "veteran" | "lieutenant" | "frontier";
/** The turns each officer speaks in their own words. At the raid (Peter,
 * Nov 9 2026): "something cynical & insulting to the player like 'Typical
 * pirates preying on the weak and disorganized'"; the introductions too:
 * "some snarky, cynical, quasi insulting remark". And as the raider leaves
 * (Peter, Nov 9 2026: "something like 'Better start smelting that ore to
 * make some profits to buy back what you lost and you call yourself an
 * enterpreneur?!'"), in a large panel of their own. */
export type AdvisorMoment = "intro" | "raid" | "left" | "blocked" | "goodbye"
  // §freighter-lost (Peter, Nov 11 2026: "When the player loses their Tiny
  // Freighter, the Authority Advisor should point out something like 'It is
  // odd that a pirate would destroy a freighter and its valuable haul. I
  // suspect you have upset a criminal lowlife...'").
  | "freighter_lost";

/** §advisor-cards: the story cards an officer voices in their own words,
 * each snarky, cynical and quasi insulting, so the Authority is easy to
 * hate. Peter, Nov 9 2026: the Laser Battery's recommendation ("to instill
 * some hate for the Authority in the player early"), the free trader's
 * arrival ("Add some snarky remark about how sleezy free traders but they
 * might save you in this situation") and the Pirates chapter's opening ("Add
 * more snarkiness, cynicism and insulting remarks"). "{trader}" stands for
 * the trader's ship ("Ines Varga's Clean Slate"), "{privateer}" for the
 * pirate by name. */
export type AdvisorCard = "laser_battery" | "free_trader" | "chapter_pirates" | "privateer_contract";

export interface Advisor {
  slug: AdvisorSlug;
  /** Rank and full name: "Commander Imara Okafor". */
  name: string;
  /** The transmission portrait (the trader and pirate crop, 320 px). */
  portrait: string;
  /** The face alone, for the guide card and the coach bubble (128 px). */
  face: string;
  /** Their own lines at the charter's moments: at most 16 words each. */
  moments: Record<AdvisorMoment, string>;
  /** Their own word on these story cards: at most 18 words, names filled in. */
  cards: Record<AdvisorCard, string>;
}

const ART = "/art/derived/advisors/v1";
const officer = (slug: AdvisorSlug, name: string, moments: Record<AdvisorMoment, string>, cards: Record<AdvisorCard, string>): Advisor =>
  ({ slug, name, portrait: `${ART}/${slug}-320.webp`, face: `${ART}/${slug}-face-128.webp`, moments, cards });

/** By `FoundingView.advisor`. */
export const ADVISORS: readonly Advisor[] = [
  // Calm, dry, unhurried: short sentences, understatement.
  officer("veteran", "Commander Imara Okafor", {
    intro: "Another founder. I've buried a dozen colonies; try not to be the thirteenth.",
    raid: "Typical pirates, preying on the weak and disorganized.",
    left: "Gone, with your engines. Smelt that ore and earn them back. Slowly, I expect.",
    blocked: "Read the warning under the task. Fix that first.",
    goodbye: "The charter's fulfilled. The colony stands. What you build next is yours.",
    freighter_lost: "Odd. Pirates rob freighters; they don't destroy the haul. You've upset some criminal lowlife.",
  }, {
    laser_battery: "The Authority protects its investments. You aren't one yet. Shoot back next time.",
    free_trader: "{trader} has docked with semiconductors. Free traders are thieves with permits. Today, you need one.",
    chapter_pirates: "A Shipyard. Finally. {privateer} is still out there, laughing at you. Build something that shoots.",
    privateer_contract: "Authority contract on {privateer}. They waited until you had a warship. Smart. Unlike you.",
  }),
  // Earnest, precise: by the book, says "we".
  officer("lieutenant", "Lieutenant Daniel Seo", {
    intro: "Regulations require an advisor for every charter. Even the hopeless ones.",
    raid: "By the book, raiders hit the least prepared colony in range. Congratulations.",
    left: "Incident logged. Per procedure, smelt ore, sell Alloys and buy it back. Even you can.",
    blocked: "We have a blocker. Check the warning, then the highlighted control.",
    goodbye: "Charter complete. I'm reassigned, but your next objectives are ready. Keep us proud.",
    freighter_lost: "Irregular. Pirates destroying a freighter and its haul? We suspect you've upset a criminal lowlife.",
  }, {
    laser_battery: "Per regulation, colony defense is the founder's expense. We've already drafted your next raid report.",
    free_trader: "{trader} has docked. Unlicensed, overpriced and carrying semiconductors. Regrettably, we need them.",
    chapter_pirates: "Shipyard operational, behind schedule. Regulations permit you to hunt {privateer} yourself. The Authority certainly won't.",
    privateer_contract: "Coincidentally, the Authority has just funded a contract on {privateer}. Sign before they notice you might succeed.",
  }),
  // Blunt, practical, wry: the shortcut the manual leaves out.
  officer("frontier", "Major Rurik Halvorsen", {
    intro: "They give me the founders nobody expects to last. So. Here we are.",
    raid: "Pirates always find the easiest mark in the sector. Can't say I blame them.",
    left: "Better start smelting that ore to buy back what you lost. You call yourself an entrepreneur?!",
    blocked: "Start with the warning. That's what's holding us up.",
    goodbye: "The colony's standing. My work here is done. Keep it that way.",
    freighter_lost: "A pirate destroying a freighter and its valuable haul? I suspect you've upset a criminal lowlife...",
  }, {
    laser_battery: "Authority patrols don't come out this far. Not for you, anyway. Get a gun up.",
    free_trader: "{trader} docked. Free traders would sell their own mothers. Lucky you: they also sell semiconductors.",
    chapter_pirates: "Took you long enough. {privateer} is out there selling your engines. Go take them back.",
    privateer_contract: "What luck. The Authority found money for {privateer}'s head the moment you stopped being useless.",
  }),
];

/** The officer dealt to this charter, once the server has dealt one. */
export function charterAdvisor(f?: FoundingView | null): Advisor | null {
  return f?.local_opening && f.advisor != null ? ADVISORS[f.advisor] ?? null : null;
}

/** The officer while the charter runs: they leave when it is fulfilled. */
export const advisorOnDuty = (f?: FoundingView | null): Advisor | null =>
  f && f.stage !== "complete" ? charterAdvisor(f) : null;

/** An officer's word on a card, its names filled in: one opening a sentence
 * takes a capital ("The privateer is…"). */
export function cardLine(a: Advisor, card: AdvisorCard, names: { trader?: string; privateer?: string } = {}): string {
  const fill = (line: string, slot: string, name: string) =>
    line.replace(new RegExp(`(^|[.!?] )\\{${slot}\\}`, "g"), (_, before: string) => before + name[0].toUpperCase() + name.slice(1))
      .replaceAll(`{${slot}}`, name);
  return fill(fill(a.cards[card], "trader", names.trader ?? "a free trader"), "privateer", names.privateer ?? "the privateer");
}

/** Name and role, for alt text and hover. */
export const advisorTitle = (a: Advisor): string => `${a.name}, Charter Advisor`;

/** The charter's step lines, one voice for all three officers. Peter's copy
 * rule (Nov 8 2026, cutting "Your charter: ore → Alloys → the Market Hub."):
 * at most 12 words, why or how, never the step's title or numbers, never
 * flavour for its own sake. Keyed by the checklist's stages, plus the
 * guide's two opening client steps. */
export const STEP_LINES: Readonly<Record<string, string>> = {
  open_home: "Start at home. Your colony ship just landed there.",
  open_world: "Open the world you landed on to build there.",
  build_habitat: "Housing brings settlers ashore and makes room for the colony.",
  power_shipyard: "Mines and farms need power. Sunlight is free.",
  build_bioharvester: "The crate's Provisions won't last. Food starts with Biomass.",
  build_agroplex: "Biomass feeds no one. The Agroplex turns it into Provisions.",
  build_mine: "Keep ore flowing; the Smelter needs it for every Alloy.",
  build_smelter: "Alloys build everything ahead, and the Hub pays well.",
  pirate_raid: "Let it go. It takes what it came for and leaves.",
  build_gun_platform: "One gun over the colony. Pirates prefer worlds that don't shoot back.",
  sell_ore: "Alloys are money. The trader takes a few.",
  build_shipyard: "No ships, no reach. The Shipyard builds every hull ahead.",
  build_interceptor: "One fast warship is enough for a lone privateer.",
  defeat_privateer: "He went home to his den. His bounty funds the colony.",
  eliminate_den: "Its den holds what it took. Clear it and take it back.",
  repair_interceptor: "Dock it at the Shipyard. Damaged hulls fight badly.",
  build_tiny_freighter: "The den's loot won't fly itself home.",
  full_speed: "Slow saves fuel, but this run is short. Go Full.",
  haul_loot: "Fetch what's left at the den; repeat until it's empty.",
  prepare_export: "The Market Hub buys Alloys. This is the charter's first export.",
  first_research: "Research unlocks hulls. This one gives you the Small Freighter.",
  answer_signal: "Signals are chances near home. Send a captained ship.",
  build_small_freighter: "Three Tiny holds in one hull: fewer trips to the Hub.",
  expand_mining: "The charter opens your other worlds. Mine the richest.",
  survey_candidates: "The survey charts the cleared den's belt for your Mining Barge.",
  build_barge: "Belts offer deposits beyond home. A barge extracts their ore.",
  prospect_belt: "A prospecting pass reveals hidden ore pockets worth mining.",
  mine_belt: "Give the mining order; prospecting alone fills no holds.",
  haul_ore: "Unload at home, or sell the ore at the Market Hub.",
  secure_stores: "Warehouses make room. Vaults keep a reserve beyond raiders' reach.",
  vendetta: "Gather your warships at home; defend the colony together.",
};

/** Lines for the guide's substeps, when the card shows an action other than
 * its step's own: staffing, the trader's parts, a captain. Same rule. */
export const ACTION_LINES: Readonly<Record<string, string>> = {
  staff: "Buildings need workers. An unstaffed one produces nothing.",
  trader_parts: "You're short of parts. The trader sells them at your dock.",
  captain: "A captain makes a warship hit harder. Assign one before it flies.",
  captain_dock: "Captains board at the dock. Bring the ship to them.",
  // §privateer-contract: the charter's first contract.
  contract: "The Authority pays only on contract. Accept this one.",
  // Before the trader is told (§no-spoilers): no word of it.
  find_semiconductors: "Semiconductors are scarce out here. Hold on to your Alloys.",
  // §watch-repair: docked, it mends on the yard's pad.
  watch_repair: "The Shipyard mends it on its own. Watch it on the pad.",
};

/** The checklist stage a served stage counts as: a retired stage, or one of
 * a step's aliases, sits on the step that replaced it. */
export function stepStage(stage: FoundingStage): FoundingStage {
  const n = FOUNDING_STEP[stage];
  for (const [, steps] of FOUNDING_CHAPTERS) for (const [s] of steps) if (FOUNDING_STEP[s] === n) return s;
  return stage;
}

/** Which line fits the action the guide card actually shows. */
export function adviceKey(step: Pick<FoundingStep, "title" | "action" | "label">, stage: FoundingStage): string {
  if (step.title === HOME_STEP_TITLE) return "open_home";
  if (step.title === HOME_WORLD_STEP_TITLE) return "open_world";
  if (step.label === "Assign workforce") return "staff";
  if (step.action === "assign-captain") return "captain";
  if (step.action === "interceptor-captain") return "captain_dock";
  if (step.action === "contract") return "contract";
  const at = stepStage(stage);
  if (at === "sell_ore") return step.action === "market" ? "find_semiconductors" : "sell_ore";
  if (step.action.startsWith("merchant-")) return "trader_parts";
  if (step.action === "watch-repair") return "watch_repair";
  return at;
}

/** The advice for the action shown, whatever else the card says: the stall
 * nudge's caption. Null without an advisor on duty. */
export function actionAdvice(step: FoundingStep | null | undefined, f?: FoundingView | null): string | null {
  if (!step || !f || !advisorOnDuty(f)) return null;
  const key = adviceKey(step, f.stage);
  const line = ACTION_LINES[key] ?? STEP_LINES[key];
  return line ? withPirateNames(line, f) : null;
}

/** The advisor's line under the guide card's title, in the step's `why`
 * place, or null when the card speaks for itself: a blocker, a wait or an
 * interruption keeps its own line, and a state-specific reason (a reward, a
 * count, a place) outranks generic advice. */
export function stepAdvice(plan: FoundingPlan | null, f?: FoundingView | null): string | null {
  const step = plan?.primary;
  if (!plan || !step || plan.blocked || step.status || step.quiet || step.reason) return null;
  return actionAdvice(step, f);
}

/** The advisor as a story event's speaker, under the Authority's colours. */
export function advisorSpeaker(a: Advisor, line: string): NonNullable<StoryEvent["speaker"]> {
  return { speaker: a.name, line, portrait: a.portrait, alt: `${advisorTitle(a)}.`, tone: "authority" };
}

/** One of the officer's own moments, as their transmission. `due` drops it
 * when its turn comes too late; `then` runs once it closes. */
export function advisorEvent(moment: AdvisorMoment, a: Advisor, due?: () => boolean, then?: () => void): StoryEvent {
  const title = { intro: "Assigned to your charter", raid: "Raider sighted", left: "The raider is gone", blocked: "Needs attention", goodbye: "Farewell",
    freighter_lost: "Freighter lost" }[moment];
  return { key: `advisor_${moment}`, title, text: "", speaker: advisorSpeaker(a, a.moments[moment]), due, then };
}

/** Where this browser remembers what the advisor has said and nudged, per
 * account and galaxy (`GalaxyInfo.instance_id`); none without both. */
export const advisorMemoryKey = (instance: string | undefined, playerId: string | null): string | null =>
  instance && playerId !== null ? `ss_advisor_v1:${JSON.stringify([instance, playerId])}` : null;

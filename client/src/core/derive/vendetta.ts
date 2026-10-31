// §vendetta (Oct 25 2026, docs/pirate-vendetta.md): the pirate family a
// founder crosses. The raiding privateer is its nephew; his uncle comes for
// the colony at the charter's end; a brother waits for later. The server
// deals the family and its names; the lines and portraits live here.
import type { FoundingView, PirateFamilySlug, PirateFamilyView } from "../../protocol";
import type { ViewState } from "../../state";

/** The story's turns. `raid` rides the raid's own pop-up; the rest arrive as
 * the server's "Vendetta · …" lines. */
export type VendettaBeat = "raid" | "kill" | "den" | "sworn" | "arrival" | "won" | "lost"
  // §vendetta-campaign: his strikes on the expanded mining.
  | "mine_strike" | "barge_strike"
  // §gun-platform: the first strike's freighter hunt, and the colony's fall.
  | "freighter_strike" | "fallen";
export type PirateRole = "nephew" | "uncle";
export type PirateMood = "smug" | "angry" | "shocked";

/** Who speaks at each turn, and how they look. */
export const SPEAKER: Record<VendettaBeat, [PirateRole, PirateMood]> = {
  raid: ["nephew", "smug"], kill: ["nephew", "shocked"], den: ["uncle", "smug"],
  sworn: ["uncle", "angry"], arrival: ["uncle", "angry"], won: ["uncle", "shocked"], lost: ["uncle", "smug"],
  mine_strike: ["uncle", "smug"], barge_strike: ["uncle", "angry"],
  freighter_strike: ["uncle", "smug"], fallen: ["uncle", "smug"],
};

/** Each family's lines. `{nephew}`, `{uncle}` and `{brother}` are first
 * names; `{surname}` and `{colony}` (the home system) are filled in too. */
export const LINES: Record<PirateFamilySlug, Record<VendettaBeat, string>> = {
  showboat: {
    raid: "Smile for the feeds, {colony}! {nephew} {surname} thanks you for your donation.",
    kill: "This wasn't in the script… Uncle {uncle} will finish the show.",
    den: "{nephew}, my boy. Take what shines, bow to no one, and call home. — {uncle}",
    sworn: "You cut my nephew's final act short. I'm bringing the encore.",
    arrival: "Lights up, {colony}. Final performance.",
    won: "Booed off my own stage… {brother} will want a rematch.",
    lost: "You've been a lovely audience. Until the sequel.",
    mine_strike: "Opening act: your second world comes down, {colony}.",
    barge_strike: "Your barge is our stage now. Places, everyone!",
    freighter_strike: "Your freighter just landed a starring role, {colony}.",
    fallen: "And scene. {colony} closes tonight.",
  },
  hothead: {
    raid: "Nobody's coming for you. Load it all!",
    kill: "No! Uncle {uncle} will break you for this!",
    den: "{nephew}. Stop picking fights you can't finish. — {uncle}",
    sworn: "You killed {nephew}. I'm coming.",
    arrival: "Here.",
    won: "Hm. {brother} won't miss.",
    lost: "Debt's paid. For now.",
    mine_strike: "Your mine. Burning.",
    barge_strike: "Get the barge.",
    freighter_strike: "Your freighter. Dead.",
    fallen: "Told you. {colony}'s mine.",
  },
  collector: {
    raid: "Your semiconductors and armaments, itemised. Your cooperation is noted, {colony}.",
    kill: "An… unbudgeted loss. Uncle {uncle} will audit you.",
    den: "Nephew. Your ledger is in arrears. Collect, then report. — {uncle} {surname}",
    sworn: "You have closed an account of mine. I am coming to settle it.",
    arrival: "Your balance is due, {colony}.",
    won: "A miscalculation… {brother} keeps the books now.",
    lost: "Paid in full. Do not fall behind again.",
    mine_strike: "Your second mine is hereby written off.",
    barge_strike: "Your barge's cargo is hereby repossessed.",
    freighter_strike: "Your freighter is collateral. I am collecting.",
    fallen: "{colony} is foreclosed.",
  },
  scrapper: {
    raid: "Ooh, shiny! Don't mind me, {colony}, just tidying up.",
    kill: "Ha… I'm scrap. Uncle {uncle} will strip you for parts.",
    den: "Bring me the good bits, {nephew}. Leave the bones. — Uncle {uncle}",
    sworn: "You broke my favourite nephew. I'm coming to take you apart.",
    arrival: "Every bolt in {colony} is mine.",
    won: "Heh… good salvage, me. {brother} will want the pieces back.",
    lost: "Good haul. I'll be back for the rest.",
    mine_strike: "Nice little mine. Coming apart lovely.",
    barge_strike: "Ooh, a barge! Strip it, lads.",
    freighter_strike: "A freighter! Plenty of good bits in that.",
    fallen: "All of {colony}, stripped for parts. Lovely.",
  },
  fatalist: {
    raid: "The void gives. The void takes. Today it takes from you.",
    kill: "So this is my hour… Uncle {uncle} will choose yours.",
    den: "{nephew}. Fear nothing, keep nothing. Return when the void allows. — {uncle}",
    sworn: "You hastened {nephew}'s end. The void has named yours.",
    arrival: "It is time, {colony}.",
    won: "Not my hour after all… {brother} will carry it.",
    lost: "Not today. The void is patient. So am I.",
    mine_strike: "Your mine falls silent. The void begins with small things.",
    barge_strike: "Your barge drifts alone. The void notices.",
    freighter_strike: "Your freighter drifts toward the void. I will see it there.",
    fallen: "{colony} returns to the void.",
  },
};

/** §vendetta-campaign: the Interceptors the guide asks for before he comes
 * (`founding::VENDETTA_DEFENDERS`), and a strike's bounty
 * (`founding::STRIKE_BOUNTY`). */
export const VENDETTA_DEFENDERS = 3;
export const STRIKE_BOUNTY = 500;

/** His death's bounty (`founding::VENDETTA_BOUNTY`). */
export const VENDETTA_BOUNTY = 6_000;

const PORTRAITS = "/art/derived/pirates/v1";

/** A face for `role` in `mood` (build-founding-story-art.mjs derives them). */
export function piratePortrait(family: PirateFamilySlug, role: PirateRole, mood: PirateMood): string {
  return `${PORTRAITS}/${family}${role === "uncle" ? "-uncle" : ""}-${mood}-320.webp`;
}

/** One turn of the story as a transmission: who, their face, their line. */
export interface Transmission { beat: VendettaBeat; speaker: string; line: string; portrait: string; alt: string }

const first = (full: string) => full.split(" ")[0];

/** The home system's name: the owned system nearest the command center. */
export function colonyName(st: ViewState): string {
  const cc = st.commandCenter;
  const systems = st.galaxy?.systems ?? [];
  const home = cc ? st.systems.filter(s => s.owner === st.playerId)
    .map(s => systems.find(g => g.id === s.id))
    .filter(s => !!s)
    .sort((a, b) => Math.hypot(a!.pos.x - cc.x, a!.pos.y - cc.y) - Math.hypot(b!.pos.x - cc.x, b!.pos.y - cc.y))[0] : undefined;
  return home?.name ?? "the colony";
}

/** The line at `beat`, or null before the server has dealt a family. */
export function transmission(beat: VendettaBeat, st: ViewState): Transmission | null {
  const cast = st.founding?.pirates;
  if (!cast) return null;
  const [role, mood] = SPEAKER[beat];
  const fill = (text: string) => text.replace(/\{(nephew|uncle|brother|surname|colony)\}/g, (_, key: string) =>
    key === "surname" ? cast.surname : key === "colony" ? colonyName(st) : first(cast[key as "nephew" | "uncle" | "brother"]));
  const speaker = role === "uncle" ? cast.uncle : cast.nephew;
  return { beat, speaker, line: fill(LINES[cast.family][beat]), portrait: piratePortrait(cast.family, role, mood),
    alt: `${speaker}, ${role === "uncle" ? "the privateer's uncle" : "the privateer"}.` };
}

/** Which turn a server "Vendetta · …" line tells (timeline.rs). */
export function vendettaBeatOf(text: string): Exclude<VendettaBeat, "raid"> | null {
  const body = /^Vendetta · (.+)$/.exec(text)?.[1];
  if (!body) return null;
  if (/'s last words named his uncle/.test(body)) return "kill";
  if (/^Among the den's stores/.test(body)) return "den";
  if (/ swore revenge for /.test(body)) return "sworn";
  // §vendetta-campaign: before "is over", which the mine strike also says.
  if (/'s raider is over /.test(body)) return "mine_strike";
  if (/'s raiders are on your Mining Barge/.test(body)) return "barge_strike";
  // §gun-platform: a wreck is a log line, not a transmission.
  if (/'s raider is hunting your /.test(body)) return "freighter_strike";
  if (/'s raider wrecked /.test(body)) return null;
  if (/ is over .+\.$/.test(body)) return "arrival";
  if (/ is dead\.$/.test(body)) return "won";
  if (/ got away( with |\.$)/.test(body)) return "lost";
  if (/ sacked .+\.$/.test(body)) return "fallen";
  return null;
}

/** §privateer-hunt (Peter, Oct 30 2026: "a Galaxy Map marker that indicates
 * the area of last known pirate contact, i.e. the hunting area"): within this
 * of its last known position an Interceptor lies in wait. The map rings it. */
export const PRIVATEER_HUNT_SU = 1_500;

/** The family's names for guide copy, or the anonymous ones without them. */
export function pirateNames(f?: FoundingView | null): { privateer: string; den: string; uncle: string; family: PirateFamilyView | null } {
  const cast = f?.pirates ?? null;
  return cast
    ? { privateer: cast.nephew, den: `the ${cast.surname} den`, uncle: cast.uncle, family: cast }
    : { privateer: "the privateer", den: "the privateer den", uncle: "the privateer's uncle", family: null };
}

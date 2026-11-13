/** §charter-v4: the charter's story turns as illustrated event pop-ups, one
 * native modal shared by both shells. They tell the story; the founding guide
 * keeps to what to do next. */
export interface StoryEvent {
  /** The story turn it tells (`ViewState.storyTold`). */
  key: StoryKey;
  title: string;
  text: string;
  /** A 1672 × 941 illustration's derivatives (build-story-event-art.mjs).
   * Until its master lands, `interim` stands in, letterboxed in the same frame
   * (`icon`: a small icon, shown at 2× on a glow). A turn told by a
   * transmission alone has none. */
  art?: { src: string; srcset?: string; alt: string; interim?: { src: string; alt: string; icon?: boolean } };
  /** §vendetta: a pirate's transmission: face, name and line (§season: or
   * the Authority's). §charter-advisor: `authority` frames the Charter
   * Advisor in the Authority's gold, not a pirate's orange. */
  speaker?: Pick<Transmission, "speaker" | "line" | "portrait" | "alt"> & { tone?: "authority" };
  /** §laser-battery-card: the step it asks for, in the guide's words: its
   * reason, then its recipe, under the title. */
  facts?: string[];
  /** §season: the broadcast telling it, named with its emblem in the header.
   * The crates and the Charter Advisor had the Authority's seal there until
   * Peter (Nov 9 2026): "remove the Terran Charter Authority title at the
   * top of these panels, doesn't make sense". */
  source?: NewsSource;
  /** §crate-panel: markup in place of a painting, made when its turn comes. */
  panel?: () => string;
  /** §raid-losses: goods taken, shown as the crate's grid in reverse under the
   * caption, in place of a sentence listing them. */
  losses?: [Commodity, number][];
  /** §den-loot: goods to be had, in the crate's grid, under the caption. */
  goods?: [Commodity, number][];
  /** §contract-reward: a contract's reward in credits, as a crate tile under
   * the sentence. */
  credits?: number;
  /** §signal-report: what came home, as crate tiles under the sentence (an
   * amount over the name, or the name alone), then short notes. */
  tiles?: StoryTile[];
  notes?: { text: string; tone?: "warn" }[];
  /** §signal-report: a second button beside Continue: it runs, then closes. */
  also?: { label: string; run(): void };
  /** §crate-panel: a pop-up the player must accept: its one button runs
   * this, and Escape does nothing. */
  accept?: { label: string; run(): void };
  /** Asked when its turn comes: one no longer due is skipped. */
  due?: () => boolean;
  /** §charter-advisor: runs once it closes (the first blocker's transmission
   * then points at the guide's action). */
  then?: () => void;
}

/** §charter-advisor: a line as the Charter Advisor's transmission. */
export type Voice = (line: string) => NonNullable<StoryEvent["speaker"]>;

export type StoryKey = "pirates" | "laser_battery" | "free_trader" | "den_contract" | "privateer_contract" | "den_loot" | `vendetta_${Exclude<VendettaBeat, "raid">}` | `decree_${Grip}`
  | `crackdown_${number}` | `exposed_${number}` | `journey_${number}` | "ending" | "relics" | `chapter_${ChapterOpening}`
  | `crate_${CharterCrateId}` | `advisor_${AdvisorMoment}` | `signal_${string}`;

import { traderShipArt, type CharterCrateId, type CharterCrateView, type Commodity, type CrackdownKind, type FoundingView, type Grip, type NewsSource } from "./protocol";
import { commodityIcon, icon, label } from "./icons";
import { CRACKDOWN_TEXT, crackdownTitle, DECREE_TEXT, endingText, gripNumeral, gripTitle, newsEmblem } from "./core/derive/season";
import type { ChapterOpening } from "./core/events";
import type { AdvisorMoment } from "./core/derive/advisor";
import type { Trader } from "./core/derive/trader";
import { pirateNames, type Transmission, type VendettaBeat } from "./core/derive/vendetta";
import { FIRST_TEN, signalAftermathArt, signalDur, signalFrameArt, signalInfo, signalStory } from "./core/derive/signals";
import type { SignalView } from "./protocol";

/** §signal-report: one tile in the crate's grid. */
export interface StoryTile { icon: string; amount?: string; label: string }

const STORY_ART = "/art/derived/story-events/v1";
const storyArt = (slug: string) => ({ src: `${STORY_ART}/${slug}-1280.webp`,
  srcset: [768, 1280, 1672].map(w => `${STORY_ART}/${slug}-${w}.webp ${w}w`).join(", ") });

const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** §charter-v5: the privateer's raid on the colony, on the server's report of
 * what it took. §raid-losses (Peter, Nov 8 2026: "an indicator of resources
 * taken instead of this text … like the opposite of the Crates"): what it
 * took is the crate's grid in reverse; no sentence lists it. */
export function piratesEvent(took: Partial<Record<Commodity, number>>, speaker?: Transmission | null): StoryEvent {
  const losses = (Object.entries(took) as [Commodity, number][]).filter(([, n]) => n > 0);
  return {
    key: "pirates",
    title: "Pirate raid",
    text: losses.length ? "" : "It took nothing.",
    losses,
    speaker: speaker ?? undefined,
    art: { ...storyArt("pirate-raid"), alt: "Armed pirates load stolen supply crates into their landed privateer at dusk while unarmed colonists look on.",
      interim: { src: "/art/ship_sprites/privateer_raider_ship.png", alt: "The privateer.", icon: true } },
  };
}

/** §laser-battery-card (Peter, Nov 5 2026): after the raid, the step it
 * calls for, painted. Nov 6: title and line only, no reason or cost (the
 * guide card carries both). Nov 9: the Charter Advisor's recommendation
 * (`advisor`: their word on it); the plain line without one on duty. */
export function laserBatteryEvent(advisor?: StoryEvent["speaker"] | null): StoryEvent {
  return {
    key: "laser_battery",
    title: "Build a Laser Battery",
    text: advisor ? "" : "It would be wise to build some defenses after that pirate raid…",
    speaker: advisor ?? undefined,
    art: { ...storyArt("laser-battery"), alt: "Colonists finish a laser battery on a rise above their colony at dusk, its barrel raised toward the sky the pirates fled into.",
      interim: { src: "/art/ui_icons/structures/defense_platform-256.png", alt: "The Laser Battery.", icon: true } },
  };
}

/** §den-loot (Peter, Nov 5 2026: "an Event Panel that indicates there is a
 * lot of 'loot' that needs a freighter to haul back home"): the den's fall,
 * in place of the Expansion chapter's "Lanes open" (kept for later). Peter
 * (Nov 9 2026): "list the actual loot from the Pirate Den with the resource
 * icons" in place of its units and trips: what waits there, most first. */
export function denLootEvent(goods: Partial<Record<Commodity, number>>): StoryEvent {
  return {
    key: "den_loot",
    title: "The den's loot",
    goods: (Object.entries(goods) as [Commodity, number][]).filter(([, n]) => n > 0).sort((a, b) => b[1] - a[1]),
    text: "Crates of stolen goods fill the den. It will take a freighter to haul them home.",
    art: { ...storyArt("den-loot"), alt: "The colony's scorched Interceptor floodlights a hollowed asteroid den stacked with stolen crates, pirate wrecks drifting among the rocks.",
      interim: { src: "/art/derived/ships/models-v1/previews/tiny_freighter-768.webp", alt: "A Tiny Freighter." } },
  };
}

/** §vendetta: a turn of the pirate family's story, on the server's line
 * (`text`, without its "Vendetta · " prefix). The uncle's arrival has its
 * painting (the corsair stands in until it lands); the rest are his family's
 * transmissions alone. */
export function vendettaEvent(beat: Exclude<VendettaBeat, "raid">, text: string, speaker: Transmission | null): StoryEvent {
  const who = speaker?.speaker;
  const title = {
    kill: "Last transmission", den: "A message in the den", sworn: "A vow of revenge",
    arrival: who ? `${who} is here` : "The uncle is here", won: who ? `${who} is dead` : "The uncle is dead",
    lost: who ? `${who} got away` : "The uncle got away",
    // §vendetta-campaign: his strikes on the expanded mining. §gun-platform:
    // the second world wrecked, a freighter hunted, the colony's fall.
    mine_strike: "The second mine burns", barge_strike: "An ambush at the belt",
    freighter_strike: "A raider on your freighter", fallen: "The colony has fallen",
  }[beat];
  return {
    key: `vendetta_${beat}`,
    title,
    text,
    speaker: speaker ?? undefined,
    art: beat === "arrival" ? { ...storyArt("pirate-vendetta"), alt: "A pirate warlord's corsair and its raiders drop out of the night sky over the colony as two interceptors climb to meet them.",
      interim: { src: "/art/ship_sprites/npc-contractors/pirate_corsair.png", alt: "The warlord's corsair.", icon: true } } : undefined,
  };
}

/** §season: the GNN stories' paintings (docs/gnn-art-prompts.md), each with
 * what it shows. */
const DECREE_ART: Partial<Record<Grip, readonly [string, string]>> = {
  levies: ["decree-levies", "Authority officers watch from a Hub gallery as a new levy gate scans every crate passing below."],
  inspections: ["decree-inspections", "Authority troopers search a freighter's opened crates in a Hub berth while its crew stands aside."],
  martial: ["decree-martial", "A line of Authority warships garrisons the Hub's approaches, one pinning a freighter in its scanning beam."],
};
const CRACKDOWN_ART: Record<CrackdownKind, readonly [string, string]> = {
  sabotage: ["sabotage-at-the-hub", "A blast tears open a Hub cargo berth as Authority patrol craft sweep it with searchlights."],
  massacre: ["convoy-massacre", "An Authority warship's floodlights sweep the wreckage of a destroyed freight convoy."],
  quarantine: ["quarantine", "Authority troopers hold masked colonists at a quarantine barrier while an Authority freighter hangs overhead."],
  relic_seizure: ["relic-seizure", "Authority troopers clamp a glowing violet relic into a frame as their officer hands the colony's director the order."],
  keystone: ["keystone-crackdown", "Authority warships fire lances on a rising Keystone as the syndicate's warships climb to meet them."],
};
const paint = ([slug, alt]: readonly [string, string]) => ({ ...storyArt(slug), alt });

const AUTHORITY_SEAL = { speaker: "Terran Charter Authority", line: "The Authority will not forget what you owe.",
  portrait: "/art/derived/authority-v2/terran-charter-authority-seal-192.webp", alt: "The Authority's seal." };

/** §den-contract (Peter, Nov 5 2026: "destroying the privateer provides
 * coordinates to the pirate den and it is noticed that there is an Authority
 * contract to eliminate the pirate's den by his name"): the kill's report,
 * under the Authority's seal; its line is the contract. */
export function denContractEvent(title: string, reward: string, privateer: string, den: string | null, fromLog = false): StoryEvent {
  const credits = rewardCredits(reward);
  const who = privateer[0].toUpperCase() + privateer.slice(1);
  // §den-retreat (Nov 12 2026): he went home to his den after his raid, so
  // the hunt is on it; his log names it only if he died first.
  const where = fromLog ? `${who}'s log gives the coordinates of his den${den ? ` at ${den}` : ""}.`
    : `${who} went home to his den${den ? ` at ${den}` : ""}.`;
  return {
    key: "den_contract",
    title: "Authority contract",
    text: `${where} The Authority has a contract out on it${credits === null ? `: ${reward}` : ""}.`,
    credits: credits ?? undefined,
    speaker: { ...AUTHORITY_SEAL, line: `${title}.` },
  };
}

/** §contract-reward (Peter, Nov 11 2026: "put a icon (something like the
 * resource / item icons) for the 8000 credits? and maybe use 8k instead of
 * 8,000"): a reward in short form: 950, 3.5k, 8k, 1.2M. */
export function creditsShort(n: number): string {
  if (n < 1_000) return Math.round(n).toLocaleString();
  if (n < 1_000_000) return `${(n / 1_000).toFixed(1).replace(/\.0$/, "")}k`;
  return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
}

/** The credits in a served reward ("8000 Cr"), or null for any other form. */
function rewardCredits(reward: string): number | null {
  const match = /^(\d+) Cr$/.exec(reward.trim());
  return match ? Number(match[1]) : null;
}

/** §privateer-contract (Peter, Nov 9 2026: "The Authority Advisor gets a
 * transmission from the Authority that there just happens to be a contract
 * out to eliminate the Pirate. Can we introduce the player to the Contracts
 * at this point?"): the Charter Advisor relays the Authority's contract on
 * the privateer, in their own words; its one button opens Contracts on it.
 * Without an advisor on duty, the Authority's seal says it. */
export function privateerContractEvent(title: string, reward: string, advisor: StoryEvent["speaker"] | null,
  open: () => void): StoryEvent {
  const credits = rewardCredits(reward);
  return {
    key: "privateer_contract",
    title: "Authority contract",
    facts: credits === null ? [title, reward] : [title],
    credits: credits ?? undefined,
    text: advisor ? "" : "Accept it in Contracts. The Authority pays when he's gone.",
    speaker: advisor ?? { ...AUTHORITY_SEAL, line: `${title}.` },
    accept: { label: "Open Contracts", run: open },
  };
}

/** §season: an Authority decree tightening its grip, under its seal. */
export function decreeEvent(grip: Grip): StoryEvent {
  const art = DECREE_ART[grip];
  return {
    key: `decree_${grip}`,
    title: `Grip ${gripNumeral(grip)} · ${gripTitle(grip)}`,
    text: DECREE_TEXT[grip],
    speaker: AUTHORITY_SEAL,
    art: art && paint(art),
    source: "gnn",
  };
}

/** §season: a crackdown on you, or the Hub's sabotage. `at` keys it. */
export function crackdownEvent(kind: CrackdownKind, at: number): StoryEvent {
  return { key: `crackdown_${at}`, title: crackdownTitle(kind), text: CRACKDOWN_TEXT[kind], speaker: AUTHORITY_SEAL,
    art: paint(CRACKDOWN_ART[kind]), source: "gnn" };
}

/** §season: a Scout's evidence exposed one of the Authority's false flags. */
export function exposedEvent(line: string, at: number): StoryEvent {
  const [, kind = "", by = ""] = /^False flag exposed · (.+?) · by (.+?) · /.exec(line) ?? [];
  return { key: `exposed_${at}`, title: "False flag exposed",
    text: `${by || "A Scout"} found the evidence: the Authority staged the ${kind.toLowerCase() || "crackdown"}. It lifts at once, and every colony's Resolve rises.`,
    art: paint(["false-flag-exposed", "A scout pilot records a wreck in pirate paint, Authority armour showing where the blast scraped it bare."]),
    source: "free_signal" };
}

/** §season phase 4: the GNN's word that the relics wake. */
export function relicsEvent(): StoryEvent {
  return { key: "relics", title: "The relics stir",
    text: "Exotic nodes across the galaxy wake as relics. Hold one at Resolve 50% and it draws a bearing toward a sealed world.",
    art: paint(["relics-awaken", "A violet crystal relic wakes inside a shattered moonlet as pirate warships close in."]),
    source: "gnn" };
}

/** §season phase 4: the turns of the relic journey. */
export function journeyEvent(beat: "found" | "spy" | "war", line: string, at: number): StoryEvent {
  if (beat === "found") {
    const [, world = "the sealed world"] = /^Sealed world found · (.+?)(?: · by .+?)? · /.exec(line) ?? [];
    return { key: `journey_${at}`, title: "A sealed world",
      text: `${world}: a Precursor world. Its Keystone is the gate network's control machine: whoever completes it can close every wormhole and cut the Authority off. Guardians woke when you came.`,
      art: paint(["sealed-world-found", "A scout's floodlight finds a vast Precursor ruin waking violet in a crater as pirate warships rise from its shadows."]) };
  }
  if (beat === "spy") {
    const [, when = "soon"] = /the Authority will know at (.+?) ·/.exec(line) ?? [];
    return { key: `journey_${at}`, title: "An Authority spy",
      text: `Someone in your syndicate reports to the Authority. It will know at ${when}. Trade at the Hub, bring your freight home and set your defences while you can.`,
      speaker: AUTHORITY_SEAL,
      art: paint(["authority-spy", "A syndicate officer wearing a hidden Authority pin slides a data crystal into a console that beams it to the Hub."]) };
  }
  return { key: `journey_${at}`, title: "War with the Authority",
    text: "The Authority declares permanent war: no Hub market, no Authority freight, your standing frozen, bounties on every hull you fly. Its war fleet sails.",
    speaker: AUTHORITY_SEAL,
    art: paint(["authority-war", "The Authority's war fleet pours out of the Hub's wormhole as a lone corporation freighter turns to flee."]),
    source: "gnn" };
}

/** §season phase 5: the season's end, on the served ending. */
export function endingEvent(ending: Parameters<typeof endingText>[0]): StoryEvent {
  const { title, text } = endingText(ending);
  return ending.kind === "charter_holds"
    ? { key: "ending", title, text, speaker: AUTHORITY_SEAL, source: "gnn",
      art: paint(["charter-holds", "Authority troopers plant the Authority's banner on the unfinished Keystone as an Authority warship holds the sky."]) }
    : { key: "ending", title, text, source: "free_signal",
      art: paint(["seal-fires", "A ring of violet light sweeps across space as the Hub's wormhole collapses and its lights go dark."]) };
}

/** §chapters (Peter, Oct 29 2026: "event art for each chapter of the
 * tutorial"): the Pirates, Expansion and Reach chapters open with a painting
 * (docs/chapter-art-prompts.md); until it lands, the chapter's hull stands
 * in. `privateer` names the nephew; `find` (Peter, Nov 4 2026) the survey's
 * best ore in the den's belt, as the Survey tab shows it. */
export function chapterEvent(chapter: ChapterOpening, privateer = "the privateer",
  find: { ore: Commodity; richness: number } | null = null, voice?: Voice | null, own?: string | null): StoryEvent {
  const hull = (kind: string, alt: string) => ({ src: `/art/derived/ships/models-v1/previews/${kind}-768.webp`, alt });
  const who = privateer[0].toUpperCase() + privateer.slice(1);
  return spoken({
    pirates: { key: "chapter_pirates" as const, title: "Time to Strike Back", text: `The Shipyard stands. ${who} is still out there.`,
      art: { ...storyArt("chapter-pirates"), alt: "The colony's first Interceptor takes shape in its new Shipyard as a pirate's engine trail fades from the dusk sky.",
        interim: hull("raider", "An Interceptor.") } },
    expansion: { key: "chapter_expansion" as const, title: "Lanes open", text: "The den is gone. The lanes to the Market Hub are clear.",
      art: { ...storyArt("chapter-expansion"), alt: "The colony's first freighter climbs from the plain at dawn, loaded with Alloys for the Market Hub.",
        interim: hull("tiny_freighter", "A Tiny Freighter.") } },
    reach: { key: "chapter_reach" as const, title: "Into the belt",
      text: find ? `The survey is in: the den's old belt is rich in ${label(find.ore)}, ×${find.richness.toFixed(2)}.`
        : "The survey is in: the den's old belt is rich in ore.",
      art: { ...storyArt("chapter-reach"), alt: "A Mining Barge cuts ore from a rich asteroid in a quiet belt beneath a distant star.",
        interim: hull("mining_barge", "A Mining Barge.") } },
  }[chapter], voice, own);
}

/** §charter-advisor (docs/charter-advisor.md): the Charter Advisor speaks a
 * chapter's or a crate's existing text as their line, once; it isn't
 * repeated below. */
function spoken(event: StoryEvent, voice?: Voice | null, own?: string | null): StoryEvent {
  const line = own || event.text;
  return voice && line ? { ...event, speaker: voice(line), text: "" } : event;
}

/** The free trader, on the server's word that it docked, with a hint of its
 * cargo. Shown at least FOLLOW_UP_MS after the last pop-up closed (Peter, Oct
 * 27 2026: 15 s left it docked in plain sight too long). §traders: the
 * founder's own trader (core/derive/trader.ts). §advisor-cards (Peter, Nov 9
 * 2026: "let's have the Authority Advisor announce the arrival of the Free
 * Trader"): `advisor`, their announcement in place of the line. */
export const FOLLOW_UP_MS = 3_000;
export function freeTraderEvent(trader: Trader, advisor?: StoryEvent["speaker"] | null): StoryEvent {
  return {
    key: "free_trader",
    title: "A free trader",
    text: advisor ? "" : `${trader.name}'s ${trader.ship} has docked at your colony. Word is its hold carries semiconductors.`,
    speaker: advisor ?? undefined,
    art: { ...storyArt("free-trader-inbound"), alt: "An independent merchant ship descends toward the colony at dusk.",
      interim: { src: `/art/derived/ships/models-v1/previews/${traderShipArt(trader.slug)}-768.webp`, alt: `The ${trader.ship}.` } },
  };
}

/** Home's stores: what is in them and what they hold. */
export interface StorageFill { used: number; cap: number }

type PirateNames = Pick<ReturnType<typeof pirateNames>, "privateer" | "den" | "uncle">;
const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

/** §crate-panel (Peter, Nov 3 2026: "make sure the short text blurb ties it
 * to the chapter or event ... that triggered it"): what released each crate
 * (sim `CharterCrate::released_at`), then what it is for. */
export const CRATE_TEXT: Record<CharterCrateId, (p: PirateNames) => string> = {
  landfall: () => "From the colony ship's holds: your first builds.",
  // Step 23 opens, after the Reach chapter's painting.
  reach: () => "For the charted belt: a Mining Barge to work it.",
  // The belt's ore home, under the uncle's oath: step "Secure your stores".
  vendetta: p => `${cap(p.uncle)} swore revenge. Secure your stores.`,
  // Step 19, "Research Freight Frames": the Expansion chapter's builds.
  expansion: () => "For your Academy and a second world's mine.",
};

/** §crate-panel (Peter, Nov 3 2026: "whenever the player gets a 'Crate of
 * Resources', I don't want it to look like just a step in the tutorial, I
 * want a Panel in the center of the screen to appear and the player has to
 * click accept"): a released charter crate, its goods on show. A crate
 * always opens, past the storage cap if it must
 * (sim `collect_charter_crate`); `storage` is read when its turn comes. */
export function crateEvent(crate: CharterCrateView, names: PirateNames, storage: () => StorageFill | null, accept: () => void,
  due: () => boolean = () => true, voice?: Voice | null): StoryEvent {
  const goods = (Object.entries(crate.goods) as [Commodity, number][]).filter(([, n]) => n > 0);
  const units = goods.reduce((sum, [, n]) => sum + n, 0);
  const num = (n: number) => Math.round(n).toLocaleString();
  return spoken({
    key: `crate_${crate.id}`,
    title: "Charter crate",
    text: CRATE_TEXT[crate.id](names),
    panel: () => {
      const fill = storage();
      const over = !!fill && fill.used + units > fill.cap;
      // Four sit two by two; more, three to a row.
      const cols = goods.length === 4 ? 2 : Math.min(goods.length, 3);
      return `<ul class="crate-goods" style="--cols: ${cols}">${goods.map(([good, n], i) =>
        `<li style="--i: ${i}">${commodityIcon(good)}<b>+${num(n)}</b><small>${esc(label(good))}</small></li>`).join("")}</ul>`
        + (fill ? `<p class="crate-storage${over ? " is-over" : ""}"${over ? ` title="Production waits for room."` : ""}>`
          + `Storage ${num(fill.used)} → ${num(fill.used + units)}/${num(fill.cap)}${over ? " · over capacity" : ""}</p>` : "");
    },
    accept: { label: "Accept", run: accept },
    due,
  }, voice);
}

/** §raid-losses: goods taken, as the crate's grid in reverse: icon, −N, name. */
function lossesHtml(losses: [Commodity, number][]): string {
  const cols = losses.length === 4 ? 2 : Math.min(losses.length, 3);
  return `<ul class="crate-goods is-loss" style="--cols: ${cols}" aria-label="Taken">${losses.map(([good, n], i) =>
    `<li style="--i: ${i}">${commodityIcon(good)}<b>−${Math.round(n).toLocaleString()}</b><small>${esc(label(good))}</small></li>`).join("")}</ul>`;
}

/** §signal-report (Peter, Nov 11 2026: "when the player is visiting/
 * completing a signal operation, an event panel with some art is needed to
 * explain what happen in the operation"): the report as it reaches command.
 * The signal's own painting, what happened in a line or two, what came home
 * as crate tiles, then the captain's experience, an injury and anything left
 * at the site, with Show the cache beside Continue. */
export function signalReportEvent(signal: SignalView, opts: { ship: string | null; trader?: Trader; now: number;
  programmeName: (id: string) => string; showCache?: () => void }): StoryEvent {
  const report = signal.report!;
  const info = signalInfo(signal.kind, opts.trader);
  // The aftermath (Peter, Nov 11 2026: "a second painting per type showing
  // the aftermath"); the card painting stands in until its master lands.
  const art = signalAftermathArt(signal.kind), card = signalFrameArt(signal.kind);
  const tiles: StoryTile[] = [];
  const notes: NonNullable<StoryEvent["notes"]> = [];
  let cache = false;
  for (const find of report.finds) {
    switch (find.kind) {
      case "credits": tiles.push({ icon: icon("credits"), amount: creditsShort(find.credits), label: "Credits" }); break;
      case "standing":
        tiles.push({ icon: icon("credits"), amount: creditsShort(find.credits), label: "Credits" });
        notes.push({ text: `+${find.standing} standing with the Authority` });
        break;
      case "cargo_aboard": for (const [good, n] of find.goods) tiles.push({ icon: commodityIcon(good), amount: Math.round(n).toLocaleString(), label: label(good) }); break;
      case "module_aboard": tiles.push({ icon: commodityIcon(find.module), amount: "1", label: label(find.module) }); break;
      case "blueprint": tiles.push({ icon: commodityIcon(find.module), label: `${label(find.module)} blueprint` }); break;
      case "dossier": tiles.push({ icon: icon("intel"), label: `${opts.programmeName(find.programme)} dossier` }); break;
      case "captain": tiles.push({ icon: icon("population"), label: `Captain ${find.name}` }); break;
      case "specialist": tiles.push({ icon: icon("workforce"), label: label(find.specialist) }); break;
      case "cache":
        cache = true;
        notes.push({ text: `${find.goods.map(([good, n]) => `${Math.round(n).toLocaleString()} ${label(good)}`).join(", ")} wait at the site for a Freighter` });
        break;
    }
  }
  notes.unshift({ text: `+${report.xp} XP · ${report.captain}` });
  // The first ten's tally, which the report's notice used to carry.
  if (signal.number <= FIRST_TEN) notes.unshift({ text: `First find ${signal.number}/${FIRST_TEN}` });
  if (report.hurt_until != null && report.hurt_until > opts.now) {
    notes.push({ text: `${report.captain} is hurt · out ${signalDur(report.hurt_until - opts.now).replace(/ 0m$/, "")}`, tone: "warn" });
  }
  return {
    key: `signal_${signal.id}`,
    title: info.name,
    text: signalStory(signal, opts.ship, opts.trader),
    art: { src: art.src, srcset: art.srcset, alt: `${info.name}, after the sweep.`, interim: { src: card.src, alt: info.blurb } },
    tiles,
    notes,
    also: cache && opts.showCache ? { label: "Show the cache", run: opts.showCache } : undefined,
  };
}

function tilesHtml(tiles: readonly StoryTile[]): string {
  const cols = tiles.length <= 4 ? tiles.length : tiles.length <= 6 ? 3 : 4;
  return `<ul class="crate-goods is-reward" style="--cols: ${cols}" aria-label="Found">${tiles.map((tile, i) =>
    `<li style="--i: ${i}">${tile.icon}${tile.amount ? `<b>${esc(tile.amount)}</b>` : ""}<small>${esc(tile.label)}</small></li>`).join("")}</ul>`;
}

/** §contract-reward: the reward as one crate tile: the credits icon, 8k. */
function rewardHtml(credits: number): string {
  return `<ul class="crate-goods is-reward" style="--cols: 1" aria-label="Reward"><li style="--i: 0">${icon("credits")}`
    + `<b>${creditsShort(credits)}</b><small>Credits</small></li></ul>`;
}

/** §den-loot: goods to be had, the crate's grid: two rows at most up to eight. */
function goodsHtml(goods: [Commodity, number][]): string {
  const cols = goods.length <= 4 ? goods.length : goods.length <= 6 ? 3 : 4;
  return `<ul class="crate-goods is-loot" style="--cols: ${cols}" aria-label="Loot">${goods.map(([good, n], i) =>
    `<li style="--i: ${i}">${commodityIcon(good)}<b>${Math.round(n).toLocaleString()}</b><small>${esc(label(good))}</small></li>`).join("")}</ul>`;
}

/** §crate-panel: an accepted crate waits this long for the server's word,
 * then pops up again, so a lost order or an Escape the browser forced
 * through never loses it. */
export const CRATE_RETRY_MS = 5_000;
/** §crate-panel: a crate pops up this long after its release, so the story
 * its step tells (a chapter's painting) goes first. */
export const CRATE_DELAY_MS = 2_000;

/** §crate-panel: puts up each released crate, in charter order, until the
 * server reports it collected. */
export class CrateOffers {
  private released = new Map<CharterCrateId, number>();
  private accepted = new Map<CharterCrateId, number>();

  constructor(private readonly ui: Pick<StoryEventUI, "has" | "show">, private readonly hooks: {
    founding(): FoundingView | null | undefined;
    storage(): StorageFill | null;
    collect(crate: CharterCrateId): void;
    /** §charter-advisor: the advisor on duty speaks the crate's line. */
    voice?(): Voice | null;
  }) {}

  /** On each server message. */
  offer(now = Date.now()): void {
    const waiting = (id: CharterCrateId) => !!this.hooks.founding()?.crates?.some(c => c.id === id && c.released);
    // Collected (or its charter restarted), a crate's clock starts afresh.
    for (const id of [...this.released.keys()]) if (!waiting(id)) this.released.delete(id);
    for (const crate of this.hooks.founding()?.crates ?? []) {
      if (!crate.released) continue;
      const since = this.released.get(crate.id) ?? now;
      this.released.set(crate.id, since);
      if (now - since < CRATE_DELAY_MS || now - (this.accepted.get(crate.id) ?? -Infinity) < CRATE_RETRY_MS
        || this.ui.has(`crate_${crate.id}`)) continue;
      this.ui.show(crateEvent(crate, pirateNames(this.hooks.founding()), this.hooks.storage, () => {
        this.accepted.set(crate.id, Date.now());
        this.hooks.collect(crate.id);
      }, () => waiting(crate.id), this.hooks.voice?.()));
    }
  }
}

export class StoryEventUI {
  private dialog = document.createElement("dialog");
  private queue: StoryEvent[] = [];
  private current: StoryEvent | null = null;
  private timers: ReturnType<typeof setTimeout>[] = [];
  /** Events waiting for the open pop-up to close, then their gap. */
  private afterClose: { event: StoryEvent; gapMs: number }[] = [];
  private closedAt = -Infinity;

  /** `onShow` hears each event as it is actually put on screen. */
  constructor(private readonly onShow: (event: StoryEvent) => void = () => {}) {
    this.dialog.className = "settlement-dialog story-event";
    this.dialog.setAttribute("aria-labelledby", "story-event-title");
    document.body.append(this.dialog);
    this.dialog.addEventListener("click", event => {
      const target = event.target as HTMLElement;
      if (target.closest("[data-story-act=\"accept\"]")) this.current?.accept?.run();
      // §signal-report: the second button runs, then the panel closes.
      if (target.closest("[data-story-act=\"also\"]")) {
        this.current?.also?.run();
        this.dialog.close();
      }
      if (target.closest("[data-story-act=\"accept\"]") || target.closest("[data-story-act=\"close\"]")) this.dialog.close();
    });
    // §crate-panel: one to accept stays up. Chrome forces a first Escape's
    // cancel through; its keydown can still be held back.
    this.dialog.addEventListener("cancel", event => {
      if (this.current?.accept) event.preventDefault();
    });
    this.dialog.addEventListener("keydown", event => {
      if (event.key === "Escape" && this.current?.accept) event.preventDefault();
    });
    // Escape closes the rest. Waiting follow-ups start their clock; the queue moves on.
    this.dialog.addEventListener("close", () => {
      const closed = this.current;
      this.current = null;
      this.closedAt = Date.now();
      for (const { event, gapMs } of this.afterClose.splice(0)) this.timers.push(setTimeout(() => this.show(event), gapMs));
      this.next();
      closed?.then?.();
    });
  }

  /** A pop-up is on screen. */
  get open(): boolean {
    return this.dialog.open;
  }

  show(event: StoryEvent): void {
    this.queue.push(event);
    if (!this.dialog.open) this.next();
  }

  /** Whether `key` is up or waiting its turn. */
  has(key: StoryKey): boolean {
    return this.current?.key === key || this.queue.some(event => event.key === key)
      || this.afterClose.some(({ event }) => event.key === key);
  }

  /** Show `event` no sooner than `gapMs` after the last pop-up closed, so a
   * follow-up never lands on top of the story before it. */
  showAfterGap(event: StoryEvent, gapMs: number): void {
    if (this.dialog.open) {
      this.afterClose.push({ event, gapMs });
      return;
    }
    const wait = Math.max(0, this.closedAt + gapMs - Date.now());
    if (wait > 0) this.timers.push(setTimeout(() => this.show(event), wait));
    else this.show(event);
  }

  endSession(): void {
    this.timers.forEach(clearTimeout);
    this.timers = [];
    this.queue = [];
    this.afterClose = [];
    if (this.dialog.open) this.dialog.close();
  }

  private next(): void {
    let event = this.queue.shift();
    while (event?.due && !event.due()) event = this.queue.shift();
    if (!event) return;
    this.current = event;
    const art = event.art;
    const speaker = event.speaker;
    const source = event.source
      ? `<div class="story-source"><img src="${newsEmblem(event.source, 128)}" alt="" decoding="async"><small>${event.source === "free_signal" ? "FREE SIGNAL" : "GALACTIC NEWS NETWORK"}</small></div>`
      : `<div><small>STELLAR SYNDICATES</small></div>`;
    const button = event.accept
      ? `<button type="button" class="settlement-primary" data-story-act="accept">${esc(event.accept.label)}</button>`
      : `<button type="button" class="settlement-primary" data-story-act="close">Continue</button>`;
    // §charter-advisor (Peter, Nov 9 2026: "a large Charter Advisor panel"):
    // the advisor alone, no painting or crate, speaks large.
    const advisorAlone = !event.panel && !art && speaker?.tone === "authority";
    this.dialog.className = `settlement-dialog story-event${event.panel ? " is-crate" : ""}${advisorAlone ? " is-advisor" : ""}`;
    this.dialog.innerHTML = `<div class="settlement-story"><header class="settlement-story-header">${source}</header>`
      + (event.panel ? `<div class="settlement-story-frame story-panel">${event.panel()}</div>`
        : art ? `<figure class="settlement-story-frame"><img class="settlement-story-image" src="${esc(art.src)}"${art.srcset ? ` srcset="${esc(art.srcset)}" sizes="(max-width: 892px) calc(100vw - 32px), 860px"` : ""} alt="${esc(art.alt)}" decoding="async"></figure>` : "")
      + `<div class="settlement-content settlement-story-caption"><h1 id="story-event-title" tabindex="-1">${esc(event.title)}</h1>`
      + (speaker ? `<div class="story-transmission${art ? "" : " is-alone"}${speaker.tone === "authority" ? " is-authority" : ""}"><img src="${esc(speaker.portrait)}" alt="${esc(speaker.alt)}" decoding="async">`
        + `<div><strong>${esc(speaker.speaker)}</strong><q>${esc(speaker.line)}</q></div></div>` : "")
      + (event.facts?.length ? `<p class="story-facts">${event.facts.map((fact) => `<span>${esc(fact)}</span>`).join("")}</p>` : "")
      + (event.losses?.length ? lossesHtml(event.losses) : "")
      + (event.goods?.length ? goodsHtml(event.goods) : "")
      + (event.text ? `<p>${esc(event.text)}</p>` : "")
      + (event.credits ? rewardHtml(event.credits) : "")
      + (event.tiles?.length ? tilesHtml(event.tiles) : "")
      + (event.notes?.length ? `<p class="story-notes">${event.notes.map((note) => `<span${note.tone ? ` class="is-${note.tone}"` : ""}>${esc(note.text)}</span>`).join("")}</p>` : "")
      + `<footer>${event.also ? `<button type="button" data-story-act="also">${esc(event.also.label)}</button>` : ""}${button}</footer></div></div>`;
    const image = this.dialog.querySelector<HTMLImageElement>(".settlement-story-image");
    const interim = art?.interim;
    if (image && interim) {
      image.addEventListener("error", () => {
        image.removeAttribute("srcset");
        image.src = interim.src;
        image.alt = interim.alt;
        if (interim.icon) image.parentElement?.classList.add("is-icon");
      }, { once: true });
    }
    this.dialog.showModal();
    this.dialog.querySelector<HTMLButtonElement>("[data-story-act]")?.focus();
    this.onShow(event);
  }
}

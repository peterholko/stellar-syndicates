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
   * the Authority's). */
  speaker?: Pick<Transmission, "speaker" | "line" | "portrait" | "alt">;
  /** §season: the broadcast telling it, named with its emblem in the header. */
  source?: NewsSource;
}

export type StoryKey = "pirates" | "free_trader" | `vendetta_${Exclude<VendettaBeat, "raid">}` | `decree_${Grip}`
  | `crackdown_${number}` | `exposed_${number}` | `journey_${number}` | "ending" | "relics" | `chapter_${ChapterOpening}`;

import { traderShipArt, type CrackdownKind, type Grip, type NewsSource } from "./protocol";
import { CRACKDOWN_TEXT, crackdownTitle, DECREE_TEXT, endingText, gripNumeral, gripTitle, newsEmblem } from "./core/derive/season";
import type { ChapterOpening } from "./core/events";
import type { Trader } from "./core/derive/trader";
import type { Transmission, VendettaBeat } from "./core/derive/vendetta";

const STORY_ART = "/art/derived/story-events/v1";
const storyArt = (slug: string) => ({ src: `${STORY_ART}/${slug}-1280.webp`,
  srcset: [768, 1280, 1672].map(w => `${STORY_ART}/${slug}-${w}.webp ${w}w`).join(", ") });

const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** §charter-v5: the privateer's raid on the colony, on the server's report of
 * what it took. */
export function piratesEvent(took: { semiconductors: number; armaments: number }, speaker?: Transmission | null): StoryEvent {
  const goods = [took.semiconductors ? `all ${took.semiconductors} semiconductors` : "",
    took.armaments ? `${took.armaments} armaments` : ""].filter(Boolean).join(" and ");
  return {
    key: "pirates",
    title: "Pirate raid",
    text: `${speaker ? speaker.speaker : "A privateer"} swept down on the colony and made off with ${goods || "nothing"}.`,
    speaker: speaker ?? undefined,
    art: { ...storyArt("pirate-raid"), alt: "Armed pirates load stolen supply crates into their landed privateer at dusk while unarmed colonists look on.",
      interim: { src: "/art/ship_sprites/privateer_raider_ship.png", alt: "The privateer.", icon: true } },
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
 * in. `privateer` names the nephew. */
export function chapterEvent(chapter: ChapterOpening, privateer = "the privateer"): StoryEvent {
  const hull = (kind: string, alt: string) => ({ src: `/art/derived/ships/models-v1/previews/${kind}-768.webp`, alt });
  const who = privateer[0].toUpperCase() + privateer.slice(1);
  return {
    pirates: { key: "chapter_pirates" as const, title: "Strike back", text: `The Shipyard stands. ${who} is still out there.`,
      art: { ...storyArt("chapter-pirates"), alt: "The colony's first Interceptor takes shape in its new Shipyard as a pirate's engine trail fades from the dusk sky.",
        interim: hull("raider", "An Interceptor.") } },
    expansion: { key: "chapter_expansion" as const, title: "Lanes open", text: "The den is gone. The lanes to the Market Hub are clear.",
      art: { ...storyArt("chapter-expansion"), alt: "The colony's first freighter climbs from the plain at dawn, loaded with ore for the Market Hub.",
        interim: hull("tiny_freighter", "A Tiny Freighter.") } },
    reach: { key: "chapter_reach" as const, title: "Into the belt", text: "The survey is in: the den's old belt is rich in ore.",
      art: { ...storyArt("chapter-reach"), alt: "A Mining Barge cuts ore from a rich asteroid in a quiet belt beneath a distant star.",
        interim: hull("mining_barge", "A Mining Barge.") } },
  }[chapter];
}

/** The free trader, on the server's word that it docked, with a hint of its
 * cargo. Shown at least FOLLOW_UP_MS after the last pop-up closed (Peter, Oct
 * 27 2026: 15 s left it docked in plain sight too long). §traders: the
 * founder's own trader (core/derive/trader.ts). */
export const FOLLOW_UP_MS = 3_000;
export function freeTraderEvent(trader: Trader): StoryEvent {
  return {
    key: "free_trader",
    title: "A free trader",
    text: `${trader.name}'s ${trader.ship} has docked at your colony. Word is its hold carries semiconductors.`,
    art: { ...storyArt("free-trader-inbound"), alt: "An independent merchant ship descends toward the colony at dusk.",
      interim: { src: `/art/derived/ships/models-v1/previews/${traderShipArt(trader.slug)}-768.webp`, alt: `The ${trader.ship}.` } },
  };
}

export class StoryEventUI {
  private dialog = document.createElement("dialog");
  private queue: StoryEvent[] = [];
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
      if ((event.target as HTMLElement).closest("[data-story-act=\"close\"]")) this.dialog.close();
    });
    // Escape closes too. Waiting follow-ups start their clock; the queue moves on.
    this.dialog.addEventListener("close", () => {
      this.closedAt = Date.now();
      for (const { event, gapMs } of this.afterClose.splice(0)) this.timers.push(setTimeout(() => this.show(event), gapMs));
      this.next();
    });
  }

  show(event: StoryEvent): void {
    this.queue.push(event);
    if (!this.dialog.open) this.next();
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
    const event = this.queue.shift();
    if (!event) return;
    const art = event.art;
    const speaker = event.speaker;
    const source = event.source
      ? `<div class="story-source"><img src="${newsEmblem(event.source, 128)}" alt="" decoding="async"><small>${event.source === "free_signal" ? "FREE SIGNAL" : "GALACTIC NEWS NETWORK"}</small></div>`
      : `<div><small>STELLAR SYNDICATES</small></div>`;
    this.dialog.innerHTML = `<div class="settlement-story"><header class="settlement-story-header">${source}</header>`
      + (art ? `<figure class="settlement-story-frame"><img class="settlement-story-image" src="${esc(art.src)}"${art.srcset ? ` srcset="${esc(art.srcset)}" sizes="(max-width: 892px) calc(100vw - 32px), 860px"` : ""} alt="${esc(art.alt)}" decoding="async"></figure>` : "")
      + `<div class="settlement-content settlement-story-caption"><h1 id="story-event-title" tabindex="-1">${esc(event.title)}</h1>`
      + (speaker ? `<div class="story-transmission${art ? "" : " is-alone"}"><img src="${esc(speaker.portrait)}" alt="${esc(speaker.alt)}" decoding="async">`
        + `<div><strong>${esc(speaker.speaker)}</strong><q>${esc(speaker.line)}</q></div></div>` : "")
      + `<p>${esc(event.text)}</p>`
      + `<footer><button type="button" class="settlement-primary" data-story-act="close">Continue</button></footer></div></div>`;
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
    this.dialog.querySelector<HTMLButtonElement>("[data-story-act=\"close\"]")?.focus();
    this.onShow(event);
  }
}

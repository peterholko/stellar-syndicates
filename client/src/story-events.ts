/** §charter-v4: the charter's story turns as illustrated event pop-ups, one
 * native modal shared by both shells. They tell the story; the founding guide
 * keeps to what to do next. */
export interface StoryEvent {
  /** The story turn it tells (`ViewState.storyTold`). */
  key: "pirates" | "free_trader";
  title: string;
  text: string;
  /** A 1672 × 941 illustration's derivatives (build-story-event-art.mjs).
   * Until its master lands, `interim` stands in, letterboxed in the same frame
   * (`icon`: a small icon, shown at 2× on a glow). */
  art: { src: string; srcset?: string; alt: string; interim?: { src: string; alt: string; icon?: boolean } };
}

const STORY_ART = "/art/derived/story-events/v1";
const storyArt = (slug: string) => ({ src: `${STORY_ART}/${slug}-1280.webp`,
  srcset: [768, 1280, 1672].map(w => `${STORY_ART}/${slug}-${w}.webp ${w}w`).join(", ") });

const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

/** §charter-v5: the privateer's raid on the colony, on the server's report of
 * what it took. */
export function piratesEvent(took: { semiconductors: number; armaments: number }): StoryEvent {
  const goods = [took.semiconductors ? `all ${took.semiconductors} semiconductors` : "",
    took.armaments ? `${took.armaments} armaments` : ""].filter(Boolean).join(" and ");
  return {
    key: "pirates",
    title: "Pirate raid",
    text: `A privateer swept down on the colony and made off with ${goods || "nothing"}.`,
    art: { ...storyArt("pirate-raid"), alt: "Armed pirates load stolen supply crates into their landed privateer at dusk while unarmed colonists look on.",
      interim: { src: "/art/ship_sprites/privateer_raider_ship.png", alt: "The privateer.", icon: true } },
  };
}

/** The free trader, on the server's word that it docked, with a hint of its
 * cargo. Shown at least FOLLOW_UP_MS after the last pop-up closed. §persona:
 * Teo Brannick's Long Odds (core/derive/trader.ts). */
export const FOLLOW_UP_MS = 15_000;
export function freeTraderEvent(): StoryEvent {
  return {
    key: "free_trader",
    title: "A free trader",
    text: "Teo Brannick's Long Odds has docked at your colony. Word is its hold carries semiconductors.",
    art: { ...storyArt("free-trader-inbound"), alt: "An independent merchant ship descends toward the colony at dusk.",
      interim: { src: "/art/derived/ships/models-v1/previews/free_trader-768.webp", alt: "The free trader's ship." } },
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
    this.dialog.innerHTML = `<div class="settlement-story"><header class="settlement-story-header"><div><small>STELLAR SYNDICATES</small><span>Charter event</span></div></header>`
      + `<figure class="settlement-story-frame"><img class="settlement-story-image" src="${esc(art.src)}"${art.srcset ? ` srcset="${esc(art.srcset)}" sizes="(max-width: 892px) calc(100vw - 32px), 860px"` : ""} alt="${esc(art.alt)}" decoding="async"></figure>`
      + `<div class="settlement-content settlement-story-caption"><h1 id="story-event-title" tabindex="-1">${esc(event.title)}</h1><p>${esc(event.text)}</p>`
      + `<footer><button type="button" class="settlement-primary" data-story-act="close">Continue</button></footer></div></div>`;
    const image = this.dialog.querySelector<HTMLImageElement>("img");
    const interim = art.interim;
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

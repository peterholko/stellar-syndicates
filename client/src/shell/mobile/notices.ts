import { setHtml } from "../dom";
import type { SheetEntry } from "./sheets";

export type MobileNoticeTone = "info" | "good" | "bad" | "quiet";

export interface MobileNotice {
  html: string;
  tone?: MobileNoticeTone;
  durationMs?: number;
  destination?: SheetEntry;
}

/** A single bounded stream for map readouts and arrived gameplay news.
 * New light is placed nearest the chrome, while old messages fall away rather
 * than covering the remaining map. Destinations are shell navigation only. */
export class MobileNoticeStack {
  private nextId = 1;
  private readonly timers = new Map<number, number>();
  private readonly destinations = new Map<number, SheetEntry>();

  constructor(
    private readonly root: HTMLElement,
    private readonly open: (entry: SheetEntry) => void,
    signal: AbortSignal,
  ) {
    root.addEventListener("click", (event) => {
      const item = (event.target as Element).closest<HTMLElement>("[data-mobile-notice]");
      const id = Number(item?.dataset.mobileNotice);
      if (!item || !Number.isFinite(id)) return;
      const destination = this.destinations.get(id);
      this.dismiss(id);
      if (destination) this.open(destination);
    }, { signal });
  }

  push(notice: MobileNotice | string): void {
    const item = typeof notice === "string" ? { html: notice } : notice;
    const id = this.nextId++;
    const element = document.createElement(item.destination ? "button" : "div");
    if (element instanceof HTMLButtonElement) element.type = "button";
    element.className = `m-map-notice__item is-${item.tone ?? "info"}`;
    element.dataset.mobileNotice = String(id);
    setHtml(element, item.html);
    this.root.prepend(element);
    this.root.hidden = false;
    if (item.destination) this.destinations.set(id, item.destination);
    requestAnimationFrame(() => element.classList.add("is-visible"));

    while (this.root.children.length > 3) {
      const oldest = this.root.lastElementChild as HTMLElement | null;
      if (!oldest) break;
      this.remove(Number(oldest.dataset.mobileNotice));
    }
    this.timers.set(id, window.setTimeout(() => this.dismiss(id), item.durationMs ?? 4200));
  }

  teardown(): void {
    for (const timer of this.timers.values()) window.clearTimeout(timer);
    this.timers.clear();
    this.destinations.clear();
    this.root.replaceChildren();
    this.root.hidden = true;
  }

  private dismiss(id: number): void {
    const element = this.root.querySelector<HTMLElement>(`[data-mobile-notice="${id}"]`);
    if (!element) return;
    element.classList.remove("is-visible");
    window.setTimeout(() => this.remove(id), 160);
  }

  private remove(id: number): void {
    const timer = this.timers.get(id);
    if (timer !== undefined) window.clearTimeout(timer);
    this.timers.delete(id);
    this.destinations.delete(id);
    this.root.querySelector(`[data-mobile-notice="${id}"]`)?.remove();
    if (!this.root.children.length) this.root.hidden = true;
  }
}

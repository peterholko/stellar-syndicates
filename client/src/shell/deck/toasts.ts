import type { DeckRoute } from "./router";

export interface DeckToast {
  title?: string;
  message: string;
  tone?: "quiet" | "good" | "warn" | "bad";
  destination?: DeckRoute;
  durationMs?: number;
}

const MAX_TOASTS = 5;
const DEFAULT_DURATION_MS = 12_000;
const FADE_MS = 240;

/** One transient notice lane for the whole Deck. A notice may also be a route:
 * clicking it uses the same router as nav, breadcrumbs, and the map. */
export class DeckToasts {
  private readonly timers = new Map<HTMLElement, number>();

  constructor(
    private readonly root: HTMLElement,
    private readonly navigate: (route: DeckRoute) => void,
    signal: AbortSignal,
  ) {
    root.addEventListener("click", (event) => {
      const item = (event.target as Element).closest<HTMLElement>(".deck-toast");
      if (!item) return;
      const route = (item as HTMLElement & { deckRoute?: DeckRoute }).deckRoute;
      if (route) this.navigate(route);
      this.dismiss(item);
    }, { signal });
  }

  push(toast: DeckToast): void {
    const item = document.createElement(toast.destination ? "button" : "div");
    item.className = `deck-toast deck-toast--${toast.tone ?? "quiet"}`;
    if (toast.destination) {
      item.setAttribute("type", "button");
      (item as HTMLElement & { deckRoute?: DeckRoute }).deckRoute = toast.destination;
      item.setAttribute("aria-label", `${toast.title ? `${toast.title}: ` : ""}${toast.message}. Open details`);
    }
    if (toast.title) {
      const title = document.createElement("b");
      title.textContent = toast.title;
      item.append(title);
    }
    const message = document.createElement("span");
    message.textContent = toast.message;
    item.append(message);
    this.root.prepend(item);
    while (this.root.children.length > MAX_TOASTS) {
      this.remove(this.root.lastElementChild as HTMLElement);
    }
    const timer = window.setTimeout(() => this.dismiss(item), toast.durationMs ?? DEFAULT_DURATION_MS);
    this.timers.set(item, timer);
  }

  teardown(): void {
    for (const timer of this.timers.values()) window.clearTimeout(timer);
    this.timers.clear();
    this.root.replaceChildren();
  }

  private dismiss(item: HTMLElement): void {
    if (!item.isConnected || item.classList.contains("is-leaving")) return;
    item.classList.add("is-leaving");
    window.setTimeout(() => this.remove(item), FADE_MS);
  }

  private remove(item: HTMLElement): void {
    const timer = this.timers.get(item);
    if (timer !== undefined) window.clearTimeout(timer);
    this.timers.delete(item);
    item.remove();
  }
}

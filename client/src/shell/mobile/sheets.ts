import { renderDeferred, setHtml } from "../dom";
import type { Rect } from "../types";

export type SheetId =
  | "market"
  | "fleets"
  | "research"
  | "officers"
  | "operations"
  | "syndicate"
  | "faction"
  | "rankings"
  | "logistics"
  | "doctrine"
  | "log"
  | "system"
  | "planet"
  | "build"
  | "shipyard"
  | "hub"
  | "ship"
  | "battle"
  | "ground"
  | "intent";

export interface SheetEntry {
  id: SheetId;
  props?: unknown;
}

export type SheetDetent = "half" | "full";

export interface SheetView {
  title: string;
  eyebrow?: string;
  html: string;
  /// A dense workspace may open full-height while ordinary command sheets
  /// retain the half-height default. Refreshes preserve the player's detent.
  detent?: SheetDetent;
}

export type SheetRenderer = (entry: SheetEntry) => SheetView;

const HISTORY_KEY = "stellarSyndicatesMobileSheet";
let nextSession = 1;
let activeStack: SheetStack | null = null;

export function activateSheetStack(stack: SheetStack | null): void {
  activeStack = stack;
}

export function pushSheet(id: SheetId, props?: unknown): void {
  activeStack?.push(id, props);
}

export function popSheet(): void {
  activeStack?.pop();
}

export function replaceSheet(id: SheetId, props?: unknown): void {
  activeStack?.replace(id, props);
}

export class SheetStack {
  private readonly entries: SheetEntry[] = [];
  private readonly session = nextSession++;
  private detent: SheetDetent = "half";
  private dragStartY = 0;
  private dragStartHeight = 0;
  private dragging = false;

  constructor(
    private readonly renderEntry: SheetRenderer,
    private readonly onLayout: () => void,
    private readonly onChange: (entry: SheetEntry | null) => void,
    signal: AbortSignal,
  ) {
    this.sheet.addEventListener("pointerdown", (event) => this.startDrag(event), { signal });
    window.addEventListener("pointermove", (event) => this.moveDrag(event), { signal });
    window.addEventListener("pointerup", (event) => this.endDrag(event), { signal });
    window.addEventListener("pointercancel", () => this.cancelDrag(), { signal });
    window.addEventListener("resize", () => this.layout(), { signal });
    window.addEventListener("popstate", (event) => this.onPopState(event), { signal });
    this.back.addEventListener("click", () => this.pop(), { signal });
    this.close.addEventListener("click", () => this.closeAll(), { signal });
    this.expand.addEventListener("click", () => this.setDetent(this.detent === "half" ? "full" : "half"), { signal });
  }

  get current(): SheetEntry | null {
    return this.entries.at(-1) ?? null;
  }

  push(id: SheetId, props?: unknown): void {
    this.entries.push({ id, props });
    history.pushState({
      ...(typeof history.state === "object" && history.state ? history.state : {}),
      [HISTORY_KEY]: { session: this.session, depth: this.entries.length },
    }, "");
    this.render();
  }

  pop(): void {
    if (!this.entries.length) return;
    const marker = this.historyMarker();
    if (marker?.session === this.session) history.back();
    else {
      this.entries.pop();
      this.render();
    }
  }

  replace(id: SheetId, props?: unknown): void {
    if (!this.entries.length) {
      this.push(id, props);
      return;
    }
    this.entries[this.entries.length - 1] = { id, props };
    this.render();
  }

  closeAll(): void {
    if (!this.entries.length) return;
    const depth = this.entries.length;
    const marker = this.historyMarker();
    if (marker?.session === this.session) history.go(-depth);
    else {
      this.entries.length = 0;
      this.render();
    }
  }

  refresh(): void {
    if (!this.current) return;
    if (renderDeferred("m-sheet", () => this.refresh())) return;
    this.render(false);
  }

  cameraRect(): Rect {
    const top = this.chrome.hidden ? 0 : this.chrome.getBoundingClientRect().bottom;
    const bottom = this.current && !this.sheet.hidden
      ? this.sheet.getBoundingClientRect().top
      : this.tabs.hidden
        ? window.innerHeight
        : this.tabs.getBoundingClientRect().top;
    return {
      x: 0,
      y: Math.max(0, top),
      w: Math.max(1, window.innerWidth),
      h: Math.max(1, bottom - top),
    };
  }

  layout(): void {
    const chromeBottom = this.chrome.hidden ? 0 : this.chrome.getBoundingClientRect().bottom;
    document.documentElement.style.setProperty("--mobile-chrome-bottom", `${Math.ceil(chromeBottom)}px`);
    if (!this.current) {
      this.onLayout();
      return;
    }
    if (this.dragging) return;
    const heights = this.detentHeights();
    const height = heights[this.detent];
    this.sheet.style.height = `${height}px`;
    document.documentElement.style.setProperty("--mobile-sheet-height", `${height}px`);
    this.sheet.dataset.detent = this.detent;
    this.expand.setAttribute("aria-label", this.detent === "half" ? "Expand sheet" : "Collapse sheet");
    this.expand.textContent = this.detent === "half" ? "↑" : "↓";
    this.onLayout();
  }

  private render(resetDetent = true): void {
    const entry = this.current;
    this.sheet.hidden = entry === null;
    if (!entry) {
      this.entries.length = 0;
      document.documentElement.style.setProperty("--mobile-sheet-height", "0px");
      this.onChange(null);
      this.onLayout();
      return;
    }
    const view = this.renderEntry(entry);
    if (resetDetent) this.detent = view.detent ?? "half";
    this.eyebrow.textContent = view.eyebrow ?? "Command workspace";
    this.title.textContent = view.title;
    this.back.hidden = this.entries.length < 2;
    setHtml(this.body, view.html);
    this.onChange(entry);
    this.layout();
  }

  private setDetent(detent: SheetDetent): void {
    this.detent = detent;
    this.layout();
  }

  private startDrag(event: PointerEvent): void {
    if (!(event.target as Element).closest("[data-sheet-drag]")) return;
    this.dragging = true;
    this.dragStartY = event.clientY;
    this.dragStartHeight = this.sheet.getBoundingClientRect().height;
    this.sheet.classList.add("is-dragging");
    try { this.sheet.setPointerCapture(event.pointerId); } catch { /* optional */ }
  }

  private moveDrag(event: PointerEvent): void {
    if (!this.dragging) return;
    const heights = this.detentHeights();
    const height = Math.max(heights.half, Math.min(heights.full, this.dragStartHeight + this.dragStartY - event.clientY));
    this.sheet.style.height = `${height}px`;
    document.documentElement.style.setProperty("--mobile-sheet-height", `${height}px`);
    this.onLayout();
  }

  private endDrag(event: PointerEvent): void {
    if (!this.dragging) return;
    this.dragging = false;
    this.sheet.classList.remove("is-dragging");
    try { this.sheet.releasePointerCapture(event.pointerId); } catch { /* optional */ }
    const moved = Math.abs(event.clientY - this.dragStartY);
    if (moved < 5) this.detent = this.detent === "half" ? "full" : "half";
    else {
      const heights = this.detentHeights();
      this.detent = this.sheet.getBoundingClientRect().height >= (heights.half + heights.full) / 2 ? "full" : "half";
    }
    this.layout();
  }

  private cancelDrag(): void {
    if (!this.dragging) return;
    this.dragging = false;
    this.sheet.classList.remove("is-dragging");
    this.layout();
  }

  private detentHeights(): { half: number; full: number } {
    const chromeBottom = this.chrome.hidden ? 0 : this.chrome.getBoundingClientRect().bottom;
    const tabsHeight = this.tabs.hidden ? 0 : this.tabs.getBoundingClientRect().height;
    const full = Math.max(240, window.innerHeight - chromeBottom - tabsHeight - 8);
    const half = Math.min(full, Math.max(260, window.innerHeight * .48));
    return { half, full };
  }

  private historyMarker(): { session: number; depth: number } | null {
    const marker = history.state?.[HISTORY_KEY];
    return marker && typeof marker.session === "number" && typeof marker.depth === "number" ? marker : null;
  }

  private onPopState(event: PopStateEvent): void {
    const marker = event.state?.[HISTORY_KEY];
    const depth = marker?.session === this.session ? Math.max(0, Number(marker.depth) || 0) : 0;
    if (depth >= this.entries.length) return;
    this.entries.splice(depth);
    this.render();
  }

  private get sheet(): HTMLElement { return document.getElementById("m-sheet")!; }
  private get chrome(): HTMLElement { return document.getElementById("m-chrome")!; }
  private get tabs(): HTMLElement { return document.getElementById("m-tabs")!; }
  private get eyebrow(): HTMLElement { return document.getElementById("m-sheet-eyebrow")!; }
  private get title(): HTMLElement { return document.getElementById("m-sheet-title")!; }
  private get body(): HTMLElement { return document.getElementById("m-sheet-body")!; }
  private get back(): HTMLButtonElement { return document.getElementById("m-sheet-back") as HTMLButtonElement; }
  private get close(): HTMLButtonElement { return document.getElementById("m-sheet-close") as HTMLButtonElement; }
  private get expand(): HTMLButtonElement { return document.getElementById("m-sheet-expand") as HTMLButtonElement; }
}

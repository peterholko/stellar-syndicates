import type { Renderer } from "../../render";
import type { Rect } from "../types";
import { DECK_ROUTES, type DeckCrumb, type DeckRoute, type DeckWidth } from "./router";

const WIDTH_KEY = "stellarSyndicates.deck.width.";

export class DeckWorkspace {
  private route: DeckRoute | null = null;

  constructor(
    private readonly root: HTMLElement,
    private readonly renderer: Renderer,
    signal: AbortSignal,
  ) {
    window.addEventListener("resize", () => this.publishCameraRect(), { signal });
  }

  show(route: DeckRoute, crumbs: readonly DeckCrumb[], hasParent: boolean): void {
    this.route = route;
    this.root.dataset.route = route.name;
    this.root.dataset.width = this.storedWidth(route);
    this.root.setAttribute("aria-hidden", "false");
    this.title.textContent = DECK_ROUTES[route.name].title;
    this.back.disabled = false;
    this.back.title = hasParent ? "Back to previous workspace" : "Back to the galaxy map";
    this.renderBreadcrumbs(crumbs);
    this.syncWidthButton();
    this.publishCameraRect();
  }

  close(): void {
    this.route = null;
    this.root.removeAttribute("data-route");
    this.root.removeAttribute("data-width");
    this.root.setAttribute("aria-hidden", "true");
    this.publishCameraRect();
  }

  toggleWidth(): void {
    if (!this.route) return;
    const next: DeckWidth = this.root.dataset.width === "wide" ? "standard" : "wide";
    this.root.dataset.width = next;
    try { localStorage.setItem(WIDTH_KEY + this.route.name, next); } catch { /* storage is optional */ }
    this.syncWidthButton();
    this.publishCameraRect();
  }

  cameraRect(): Rect {
    if (!this.route || this.root.getAttribute("aria-hidden") === "true") {
      return { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
    }
    const workspaceLeft = this.root.getBoundingClientRect().left;
    return { x: 0, y: 0, w: Math.max(1, workspaceLeft), h: window.innerHeight };
  }

  publishCameraRect(): void {
    const rect = this.cameraRect();
    document.documentElement.style.setProperty("--deck-workspace-inset", `${Math.max(0, window.innerWidth - rect.w)}px`);
    this.renderer.setCameraRect(rect);
  }

  teardown(): void {
    this.route = null;
    this.root.setAttribute("aria-hidden", "true");
    document.documentElement.style.removeProperty("--deck-workspace-inset");
    this.renderer.setCameraRect({ x: 0, y: 0, w: window.innerWidth, h: window.innerHeight });
  }

  private storedWidth(route: DeckRoute): DeckWidth {
    try {
      const stored = localStorage.getItem(WIDTH_KEY + route.name);
      if (stored === "standard" || stored === "wide") return stored;
    } catch { /* storage is optional */ }
    return DECK_ROUTES[route.name].width;
  }

  private renderBreadcrumbs(crumbs: readonly DeckCrumb[]): void {
    this.breadcrumb.replaceChildren();
    crumbs.forEach((crumb, index) => {
      if (index) {
        const separator = document.createElement("span");
        separator.className = "deck-breadcrumb__separator";
        separator.textContent = "›";
        this.breadcrumb.append(separator);
      }
      if (index === crumbs.length - 1) {
        const current = document.createElement("span");
        current.setAttribute("aria-current", "page");
        current.textContent = crumb.label;
        this.breadcrumb.append(current);
      } else {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.deckAct = "breadcrumb";
        button.dataset.crumbIndex = String(index);
        button.textContent = crumb.label;
        this.breadcrumb.append(button);
      }
    });
  }

  private syncWidthButton(): void {
    const wide = this.root.dataset.width === "wide";
    this.width.textContent = wide ? "↤" : "↔";
    this.width.setAttribute("aria-label", wide ? "Use standard workspace width" : "Use wide workspace width");
    this.width.title = wide ? "Standard width" : "Wide workspace";
  }

  private get back(): HTMLButtonElement { return this.root.querySelector("[data-deck-act=back]") as HTMLButtonElement; }
  private get width(): HTMLButtonElement { return this.root.querySelector("[data-deck-act=width]") as HTMLButtonElement; }
  private get breadcrumb(): HTMLElement { return this.root.querySelector("#deck-breadcrumb") as HTMLElement; }
  private get title(): HTMLElement { return this.root.querySelector("#deck-workspace-title") as HTMLElement; }
}

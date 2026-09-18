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
    this.back.setAttribute("aria-label", hasParent ? "Back to previous workspace" : "Back to the galaxy map");
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
    if (!this.route || this.route.name === "market" || this.route.name === "world") return;
    const next: DeckWidth = this.root.dataset.width === "wide" ? "standard" : "wide";
    this.root.dataset.width = next;
    try { localStorage.setItem(WIDTH_KEY + this.widthPreferenceKey(this.route), next); } catch { /* storage is optional */ }
    this.syncWidthButton();
    this.publishCameraRect();
  }

  cameraRect(): Rect {
    const topbar = document.getElementById("deck-topbar");
    const top = topbar && !topbar.hidden ? topbar.getBoundingClientRect().bottom : 0;
    const strip = document.getElementById("deck-command-strip");
    const founding = document.getElementById("deck-founding");
    const zoom = document.getElementById("deck-zoom");
    const bottomChromeVisible = this.visible(strip) || this.visible(founding);
    const bottom = bottomChromeVisible
      ? Math.min(
          window.innerHeight,
          ...[strip, founding, zoom].filter((node) => this.visible(node)).map((node) => node!.getBoundingClientRect().top),
        )
      : window.innerHeight;
    // The Market floats like the construction workbench; it is not a right
    // rail. Reserving its left edge would both pan the galaxy and feed its own
    // centered width back into --deck-workspace-inset on each layout pass.
    const workspaceLeft = !this.route || this.route.name === "market" || this.root.getAttribute("aria-hidden") === "true"
      ? window.innerWidth
      : this.root.getBoundingClientRect().left;
    return { x: 0, y: top, w: Math.max(1, workspaceLeft), h: Math.max(1, bottom - top) };
  }

  publishCameraRect(coalesceVertical = false): void {
    const rect = this.cameraRect();
    document.documentElement.style.setProperty("--deck-workspace-inset", `${Math.max(0, window.innerWidth - rect.w)}px`);
    // Centered workbenches share a lower edge above the actual command/tutorial
    // stack, which may be taller than one row. Keep their actions unobstructed.
    document.documentElement.style.setProperty("--deck-workbench-bottom-inset", `${Math.max(0, window.innerHeight - rect.y - rect.h)}px`);
    if (coalesceVertical) {
      const current = this.renderer.cameraRect;
      const verticalChange = Math.max(
        Math.abs(rect.y - current.y),
        Math.abs(rect.y + rect.h - current.y - current.h),
      );
      const horizontalChange = Math.max(
        Math.abs(rect.x - current.x),
        Math.abs(rect.x + rect.w - current.x - current.w),
      );
      // Selection/status churn may slightly resize the bottom band. Preserve
      // the stable camera until a material 15%-of-viewport inset change; large
      // Founding/strip stacks still publish immediately.
      if (verticalChange < window.innerHeight * .15 && horizontalChange < .5) return;
    }
    this.renderer.setCameraRect(rect);
  }

  teardown(): void {
    this.route = null;
    this.root.setAttribute("aria-hidden", "true");
    document.documentElement.style.removeProperty("--deck-workspace-inset");
    document.documentElement.style.removeProperty("--deck-workbench-bottom-inset");
    this.renderer.setCameraRect({ x: 0, y: 0, w: window.innerWidth, h: window.innerHeight });
  }

  private storedWidth(route: DeckRoute): DeckWidth {
    // Every entry point uses the workbench size, including hub inspection and
    // old saved standard-width preferences. Other routes keep their rail sizes.
    if (route.name === "market") return "wide";
    // A world's scene lives in the map area beside this column; a wide column
    // would crush it, so the planet route always keeps the system rail width.
    if (route.name === "world") return "standard";
    try {
      const stored = localStorage.getItem(WIDTH_KEY + this.widthPreferenceKey(route));
      if (stored === "standard" || stored === "wide") return stored;
    } catch { /* storage is optional */ }
    return DECK_ROUTES[route.name].width;
  }

  /** Build and the corporation-wide fleet roster share the System workspace
   * width, including the user's wide/standard preference. */
  private widthPreferenceKey(route: DeckRoute): string {
    return route.name === "build" || route.name === "fleets" ? "system" : route.name;
  }

  private renderBreadcrumbs(crumbs: readonly DeckCrumb[]): void {
    this.breadcrumb.replaceChildren();
    if (crumbs.length <= 1) return;
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
    this.width.hidden = this.route?.name === "market" || this.route?.name === "world";
    const wide = this.root.dataset.width === "wide";
    this.width.textContent = wide ? "↤" : "↔";
    this.width.setAttribute("aria-label", wide ? "Use standard workspace width" : "Use wide workspace width");
  }

  private visible(node: HTMLElement | null): node is HTMLElement {
    return node !== null && !node.hidden && node.offsetWidth > 0 && node.offsetHeight > 0;
  }

  private get back(): HTMLButtonElement { return this.root.querySelector("[data-deck-act=back]") as HTMLButtonElement; }
  private get width(): HTMLButtonElement { return this.root.querySelector("[data-deck-act=width]") as HTMLButtonElement; }
  private get breadcrumb(): HTMLElement { return this.root.querySelector("#deck-breadcrumb") as HTMLElement; }
  private get title(): HTMLElement { return this.root.querySelector("#deck-workspace-title") as HTMLElement; }
}

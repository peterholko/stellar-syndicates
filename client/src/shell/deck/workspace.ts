import type { Rect } from "../types";
import { DECK_ROUTES, type DeckRoute } from "./router";

export class DeckWorkspace {
  constructor(private readonly root: HTMLElement) {}

  show(route: DeckRoute): void {
    this.root.dataset.route = route.name;
    this.root.dataset.width = DECK_ROUTES[route.name].width;
    this.root.setAttribute("aria-hidden", "false");
  }

  close(): void {
    this.root.removeAttribute("data-route");
    this.root.setAttribute("aria-hidden", "true");
  }

  cameraRect(): Rect {
    return { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }
}

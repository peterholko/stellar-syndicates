import "../../styles/deck.css";

import type { CoreEvent } from "../../core/events";
import { installPressGuard } from "../dom";
import type { CoreContext, Rect, Shell } from "../types";
import { mountDeckMarkup } from "./markup";
import { DeckRouter } from "./router";
import { DeckCommandStrip } from "./strip";
import { DeckToasts } from "./toasts";
import { DeckWorkspace } from "./workspace";

const byId = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;

class DeckShell implements Shell {
  private root: HTMLElement | null = null;
  private abort: AbortController | null = null;
  private router: DeckRouter | null = null;
  private workspace: DeckWorkspace | null = null;
  private toasts: DeckToasts | null = null;
  private strip: DeckCommandStrip | null = null;

  async mount(root: HTMLElement, ctx: CoreContext): Promise<void> {
    this.root = root;
    void ctx;
    this.abort = new AbortController();
    mountDeckMarkup(root);
    installPressGuard();
    this.router = new DeckRouter();
    this.workspace = new DeckWorkspace(byId("deck-workspace"));
    this.toasts = new DeckToasts(byId("deck-toast-lane"));
    this.strip = new DeckCommandStrip(byId("deck-command-strip"));
  }

  onCore(_events: CoreEvent[]): void {}

  onViewTick(): void {}

  framePolicy() {
    return { maxFps: 0, renderGalaxy: true };
  }

  cameraRect(): Rect {
    return this.workspace?.cameraRect() ?? { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }

  teardown(): void {
    this.abort?.abort();
    this.router?.teardown();
    this.toasts?.teardown();
    this.strip?.clear();
    this.router = null;
    this.workspace = null;
    this.toasts = null;
    this.strip = null;
    this.abort = null;
    this.root?.replaceChildren();
    this.root = null;
  }
}

export const createShell = (): Shell => new DeckShell();

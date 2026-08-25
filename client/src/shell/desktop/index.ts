import "../../styles/desktop.css";

import type { Net } from "../../net";
import type { CoreContext, Rect, Shell } from "../types";
import { mountDesktopMarkup } from "./markup";

type DesktopRuntime = typeof import("./runtime");

export let net: Net | null = null;

let runtime: DesktopRuntime | null = null;
let root: HTMLElement | null = null;
let parked: DocumentFragment | null = null;
let active = false;

export const shell: Shell = {
  async mount(nextRoot: HTMLElement, ctx: CoreContext): Promise<void> {
    root = nextRoot;
    net = ctx.net;
    active = true;

    if (parked) {
      root.append(parked);
      parked = null;
    } else if (!runtime) {
      mountDesktopMarkup(root);
      runtime = await import("./runtime");
      runtime.mountDesktop();
    }

    const debug = (window as unknown as { __ss?: { net?: unknown } }).__ss;
    if (debug) debug.net = ctx.net;
    ctx.renderer.setCameraRect(this.cameraRect());
  },

  onCore(events): void {
    if (active) runtime?.handleCoreEvents(events);
  },

  onViewTick(): void {
    if (active) runtime?.onDesktopViewTick();
  },

  cameraRect(): Rect {
    return active && runtime
      ? runtime.desktopCameraRect()
      : { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  },

  teardown(): void {
    if (!active) return;
    active = false;
    runtime?.teardownDesktop();
    if (root) {
      parked = document.createDocumentFragment();
      while (root.firstChild) parked.append(root.firstChild);
    }
    net = null;
  },
};

export const createShell = (): Shell => shell;

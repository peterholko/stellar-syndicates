import "./styles/tokens.css";

import { updateSignals } from "./core/derive/orders";
import * as intent from "./core/intent";
import { applyLinkStatus, applyServerMessage, applySessionReplaced } from "./core/session";
import { Net } from "./net";
import { renderer } from "./render";
import type { CoreContext, Shell } from "./shell/types";
import { state } from "./state";

// Portrait phones enter on width; an already-landscape touch phone enters on
// its short height. Desktop windows do not become mobile merely for being low.
const MOBILE_SHELL_QUERY = "(max-width: 767px), (max-height: 767px) and (pointer: coarse)";
type ShellKind = "deck" | "mobile";

// Read once: the shell is a boot choice, not mutable navigation state. Deck is
// the desktop shell; normal breakpoint crossing swaps it with mobile.
// `?shell=mobile` remains an explicit force switch for shared-core smoke tests.
const requestedShell = new URLSearchParams(window.location.search).get("shell");
const forcedMobile = requestedShell === "mobile";

const appElement = document.getElementById("app");
const shellElement = document.getElementById("shell-root");
if (!appElement || !shellElement) throw new Error("missing application roots");
const appRoot: HTMLElement = appElement;
const shellRoot: HTMLElement = shellElement;

let activeShell: Shell | null = null;
let activeKind: ShellKind | null = null;
let shellGeneration = 0;

function present(events: Parameters<Shell["onCore"]>[0]): void {
  activeShell?.onCore(events);
}

let net!: Net;
net = new Net({
  onOpen: () => {
    present(applyLinkStatus(state.playerId === null ? "connecting" : "reconnecting", state));
    if (state.name) net.join(state.name);
  },
  onMessage: (message) => {
    present(applyServerMessage(message, state));
  },
  onClose: () => {
    present(applyLinkStatus(state.playerId === null ? "offline" : "reconnecting", state));
  },
  onSessionReplaced: () => {
    present(applySessionReplaced(state));
  },
  onError: () => {
    const events = applyLinkStatus(state.playerId === null ? "offline" : "reconnecting", state);
    present([...events, { kind: "TransportError", url: net.url }]);
  },
});

const context: CoreContext = {
  state,
  net,
  renderer,
  intent,
  send: (message) => net.send(message),
};

intent.bindIntentCore(() => net, present);

const shellMedia = matchMedia(MOBILE_SHELL_QUERY);

async function selectShell(): Promise<void> {
  const viewport = window.visualViewport;
  const phoneShortSide = Math.min(viewport?.width ?? window.innerWidth, viewport?.height ?? window.innerHeight);
  // A phone that entered through the portrait mobile breakpoint remains on the
  // same shell while rotated. The mobile shell owns the landscape gate; tearing
  // it down here would also discard sheets, gesture state, and loaded Pixi views.
  const retainMobileAcrossRotation = activeKind === "mobile" && phoneShortSide <= 767;
  const kind: ShellKind = forcedMobile || shellMedia.matches || retainMobileAcrossRotation ? "mobile" : "deck";
  if (kind === activeKind) return;
  net.setViewHz(kind === "mobile" ? 5 : 10);
  const generation = ++shellGeneration;

  try {
    const module = kind === "mobile"
      ? await import("./shell/mobile/index")
      : await import("./shell/deck/index");
    const next = module.createShell();
    if (generation !== shellGeneration) return;

    const previous = activeShell;
    const previousKind = activeKind;
    previous?.teardown();
    try {
      await next.mount(shellRoot, context);
      if (generation !== shellGeneration) {
        next.teardown();
        return;
      }
      activeShell = next;
      activeKind = kind;
      renderer.setCameraRect(next.cameraRect());
    } catch (error) {
      if (previous) {
        await previous.mount(shellRoot, context);
        activeShell = previous;
        activeKind = previousKind;
      }
      throw error;
    }
  } catch (error) {
    console.warn(`could not mount the ${kind} shell:`, error);
  }
}

shellMedia.addEventListener("change", () => { void selectShell(); });

let lastPresentedFrameMs = 0;

function frame(now: number): void {
  const policy = activeShell?.framePolicy() ?? { maxFps: 0, renderGalaxy: true };
  renderer.setRenderPolicy(policy.maxFps, policy.renderGalaxy);
  const interval = policy.maxFps > 0 ? 1000 / policy.maxFps : 0;
  // A small tolerance prevents a 60 Hz display's floating-point cadence from
  // turning the intended every-other-frame mobile schedule into every-third.
  if (interval === 0 || lastPresentedFrameMs === 0 || now - lastPresentedFrameMs >= interval - 1) {
    lastPresentedFrameMs = now;
    updateSignals();
    if (policy.renderGalaxy) renderer.update(state);
    activeShell?.onViewTick();
  }
  requestAnimationFrame(frame);
}

async function boot(): Promise<void> {
  await renderer.init(appRoot);
  await selectShell();
  requestAnimationFrame(frame);
}

void boot();

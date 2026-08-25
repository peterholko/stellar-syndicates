import "./styles/tokens.css";

import { updateSignals } from "./core/derive/orders";
import * as intent from "./core/intent";
import { applyLinkStatus, applyServerMessage } from "./core/session";
import { Net } from "./net";
import { renderer } from "./render";
import type { CoreContext, Shell } from "./shell/types";
import { state } from "./state";

const MOBILE_SHELL_QUERY = "(max-width: 767px)";

const appElement = document.getElementById("app");
const shellElement = document.getElementById("shell-root");
if (!appElement || !shellElement) throw new Error("missing application roots");
const appRoot: HTMLElement = appElement;
const shellRoot: HTMLElement = shellElement;

let activeShell: Shell | null = null;
let activeKind: "desktop" | "mobile" | null = null;
let shellGeneration = 0;

function present(events: Parameters<Shell["onCore"]>[0]): void {
  activeShell?.onCore(events);
}

let net!: Net;
net = new Net({
  onOpen: () => {
    present(applyLinkStatus(state.playerId === null ? "connecting" : "reconnecting", state));
    if (state.name) net.send({ type: "Join", name: state.name });
  },
  onMessage: (message) => {
    present(applyServerMessage(message, state));
  },
  onClose: () => {
    present(applyLinkStatus(state.playerId === null ? "offline" : "reconnecting", state));
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
  const kind = shellMedia.matches ? "mobile" : "desktop";
  if (kind === activeKind) return;
  const generation = ++shellGeneration;

  try {
    const module = kind === "mobile"
      ? await import("./shell/mobile/index")
      : await import("./shell/desktop/index");
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

function frame(): void {
  updateSignals();
  renderer.update(state);
  activeShell?.onViewTick();
  requestAnimationFrame(frame);
}

async function boot(): Promise<void> {
  await renderer.init(appRoot);
  await selectShell();
  requestAnimationFrame(frame);
}

void boot();

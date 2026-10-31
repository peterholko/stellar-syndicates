import "./styles/tokens.css";
import "./styles/settlement.css";
import "./ship-model-preview";
import { SettlementUI } from "./settlement";
import { chapterEvent, crackdownEvent, decreeEvent, endingEvent, exposedEvent, FOLLOW_UP_MS, journeyEvent, freeTraderEvent, piratesEvent, relicsEvent, StoryEventUI, vendettaEvent } from "./story-events";
import { pirateNames, transmission } from "./core/derive/vendetta";
import { foundingHomeSystemId } from "./core/derive/geo";
import { traderOf } from "./core/derive/trader";
import { accounts } from "./auth";

import { updateSignals } from "./core/derive/orders";
import * as intent from "./core/intent";
import { applyLinkStatus, applyServerMessage, applySessionReplaced } from "./core/session";
import { Net } from "./net";
import { renderer } from "./render";
import type { CoreContext, Shell } from "./shell/types";
import { loseSignal } from "./shell/account";
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

// §charter-v4: story turns pop up over either shell.
const storyEvents = new StoryEventUI(event => {
  state.storyTold.add(event.key);
  // §raid-camera: the raid's story takes over from its tracking window.
  if (event.key === "pirates") renderer.trackRaid(null);
  activeShell?.onCore([]);
});

function present(events: Parameters<Shell["onCore"]>[0]): void {
  for (const event of events) {
    // §charter-v5: the raid's report, then the trader, on the server's word
    // that it docked.
    if (event.kind === "StoryEvent" && event.story === "pirates") storyEvents.show(piratesEvent(event, transmission("raid", state)));
    // §vendetta: the pirate family's turns, each its speaker's transmission.
    if (event.kind === "StoryEvent" && event.story === "vendetta") storyEvents.show(vendettaEvent(event.beat, event.text, transmission(event.beat, state)));
    if (event.kind === "StoryEvent" && event.story === "decree" && !state.storyTold.has(`decree_${event.grip}`)) storyEvents.show(decreeEvent(event.grip));
    if (event.kind === "StoryEvent" && event.story === "crackdown") storyEvents.show(crackdownEvent(event.crackdown, event.at));
    if (event.kind === "StoryEvent" && event.story === "exposed") storyEvents.show(exposedEvent(event.text, event.at));
    if (event.kind === "StoryEvent" && event.story === "journey") storyEvents.show(journeyEvent(event.beat, event.text, event.at));
    if (event.kind === "StoryEvent" && event.story === "ending" && state.season?.ending && !state.storyTold.has("ending")) storyEvents.show(endingEvent(state.season.ending));
    if (event.kind === "StoryEvent" && event.story === "relics" && !state.storyTold.has("relics")) storyEvents.show(relicsEvent());
    // §chapters: the Pirates, Expansion and Reach chapters open painted.
    if (event.kind === "StoryEvent" && event.story === "chapter" && !state.storyTold.has(`chapter_${event.chapter}`)) {
      storyEvents.show(chapterEvent(event.chapter, pirateNames(state.founding).privateer));
    }
    if (event.kind === "StoryEvent" && event.story === "free_trader") storyEvents.showAfterGap(freeTraderEvent(traderOf(state.founding)), FOLLOW_UP_MS);
  }
  activeShell?.onCore(events);
}

// §raid-focus (Oct 26 2026, Peter): the raiding privateer's first sighting
// takes the player to it: the galaxy map framing it with the home it closes
// on, it selected, its course drawn, and the tracking window on it until the
// raid's story opens (§raid-camera and §raid-frame, Oct 27). Only a sighting
// seen happening this session: a page loaded after it stays put.
let raidSeen: boolean | null = null;
let raidFocusDue = false;
function noticeRaid(): void {
  const f = state.founding;
  if (!f) return;
  const seen = !!f.raid_seen;
  if (raidSeen === false && seen) raidFocusDue = true;
  raidSeen = seen;
  const id = f.privateer;
  if (raidFocusDue && id && state.ghosts.some((ghost) => ghost.id === id)) {
    raidFocusDue = false;
    const home = foundingHomeSystemId();
    if (home) activeShell?.focusRaid?.(id, home);
    renderer.trackRaid(id, home);
  }
}

let net!: Net;
const settlement = new SettlementUI(state, message => net.send(message));
net = new Net({
  beforeConnect: async () => {
    const account = await accounts.session();
    if (!account) return false;
    state.name = account.corporation_name;
    net.useServer(account.game_url);
    return true;
  },
  onOpen: () => {
    present(applyLinkStatus(state.playerId === null ? "connecting" : "reconnecting", state));
    if (state.name) net.join(state.name);
  },
  onMessage: (message) => {
    present(applyServerMessage(message, state));
    noticeRaid();
    settlement.onMessage(message);
  },
  onClose: () => {
    present(applyLinkStatus(state.playerId === null ? "offline" : "reconnecting", state));
    settlement.connectionChanged();
  },
  onSessionReplaced: () => {
    present(applySessionReplaced(state));
    settlement.endSession();
    storyEvents.endSession();
    renderer.trackRaid(null);
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
    // renderGalaxy is the authoritative opaque-overlay gate: when false the
    // map update is skipped here and Renderer.setRenderPolicy stops Pixi's
    // ticker. canvas[data-render-paused] mirrors that boolean; renderFps is a
    // separate cadence diagnostic.
    if (policy.renderGalaxy) renderer.update(state);
    activeShell?.onViewTick();
  }
  requestAnimationFrame(frame);
}

async function boot(): Promise<void> {
  await renderer.init(appRoot);
  await selectShell();
  requestAnimationFrame(frame);
  try {
    // Resume with the HttpOnly session, never a saved corporation-name login.
    if (await accounts.session()) net.connect();
  } catch {
    // §front-door: the sign-in card waits for Charter Command; a signed-in
    // browser connects once it answers.
    loseSignal(account => { if (account) net.connect(); });
  }
}

void boot();

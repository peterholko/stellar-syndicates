import "./styles/tokens.css";
import "./styles/settlement.css";
import "./ship-model-preview";
import { SettlementUI } from "./settlement";
import { chapterEvent, crackdownEvent, CrateOffers, decreeEvent, denContractEvent, denLootEvent, endingEvent, exposedEvent, FOLLOW_UP_MS, journeyEvent, freeTraderEvent, laserBatteryEvent, piratesEvent, privateerContractEvent, relicsEvent, signalReportEvent, StoryEventUI, vendettaEvent } from "./story-events";
import { shipKindLabel } from "./core/derive/fleet";
import { pirateNames, transmission } from "./core/derive/vendetta";
import { foundingHomeSystemId } from "./core/derive/geo";
import { FOUNDING_STEP, foundingBelt, foundingDen, foundingPlan, shipmentFollowTarget } from "./core/derive/founding";
import { advisorOnDuty, advisorSpeaker, advisorTitle, cardLine } from "./core/derive/advisor";
import { AdvisorDesk, LEFT_GAP_MS } from "./advisor-desk";
import { coach, coachTarget } from "./shell/coach";
import { isPlayerFreighter, type FoundingView } from "./protocol";
import { traderOf } from "./core/derive/trader";
import { accounts } from "./auth";

import { updateSignals } from "./core/derive/orders";
import * as intent from "./core/intent";
import { applyLinkStatus, applyServerMessage, applySessionReplaced } from "./core/session";
import { installClientUpdates } from "./client-update";
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
  advisorDesk.shown(event.key);
  // §raid-camera: the raid's story takes over from its tracking window.
  if (event.key === "pirates") renderer.trackRaid(null);
  activeShell?.onCore([]);
});

// §crate-panel: a released charter crate pops up over either shell, after its
// step's story, until it is accepted.
const crates = new CrateOffers(storyEvents, {
  founding: () => state.founding,
  storage: () => {
    const home = state.systems.find((system) => system.id === foundingHomeSystemId());
    return home ? { used: home.storage_used, cap: home.storage_cap } : null;
  },
  collect: (crate) => net.send({ type: "CollectCharterCrate", charter_crate: crate }),
  voice: () => voiceOf(state.founding),
});

/** §charter-advisor: the officer on duty as a pop-up's speaker, if any. */
function voiceOf(f?: FoundingView | null) {
  const advisor = advisorOnDuty(f);
  return advisor ? (line: string) => advisorSpeaker(advisor, line) : null;
}

// §charter-advisor (docs/charter-advisor.md): the Charter Advisor's moments
// and stall nudge, over either shell.
const advisorDesk = new AdvisorDesk(storyEvents, {
  founding: () => state.founding,
  scope: () => ({ instance: state.galaxy?.instance_id, playerId: state.playerId }),
  storyTold: (key) => state.storyTold.has(key),
  plan: () => foundingPlan(state, state.systems.find((system) => system.id === foundingHomeSystemId())),
  settling: () => settlement.open,
  foreground: () => document.visibilityState === "visible" && state.link === "online",
  nextControl: () => {
    const control = coachTarget("founding-next") as HTMLButtonElement | null;
    return !control ? "none" : control.disabled ? "disabled" : "enabled";
  },
  coachNext: (caption, advisor) => coach("founding-next", caption, "right", { src: advisor.face, alt: advisorTitle(advisor) }),
  orders: () => [...state.pendingOrders.values()].reduce((n, orders) => n + orders.length, 0),
  raidVoiced: () => renderer.raidVoiced(),
  // §freighter-lost: every freighter hull afloat, merged fleets included.
  freighters: () => state.ghosts.filter((ghost) => ghost.own).reduce((n, ghost) => n + (ghost.composition
    ? ghost.composition.filter((entry) => isPlayerFreighter(entry.kind)).reduce((sum, entry) => sum + entry.count, 0)
    : isPlayerFreighter(ghost.kind) ? 1 : 0), 0),
});
setInterval(() => advisorDesk.tick(1_000), 1_000);
// Taking the guide's action, on either shell, restarts the nudge's clock.
document.addEventListener("click", (event) => {
  if ((event.target as Element | null)?.closest?.("[data-deck-act=\"founding-action\"], [data-mobile-act=\"founding-action\"]")) advisorDesk.acted();
}, true);

function present(events: Parameters<Shell["onCore"]>[0]): void {
  for (const event of events) {
    // §charter-v5: the raid's report, then the trader, on the server's word
    // that it docked.
    if (event.kind === "StoryEvent" && event.story === "pirates") {
      storyEvents.show(piratesEvent(event.goods, transmission("raid", state)));
      // §laser-battery-card: then the step it calls for, once it is the step.
      laserBatteryDue = !state.storyTold.has("laser_battery");
      offerLaserBattery();
    }
    // §vendetta: the pirate family's turns, each its speaker's transmission.
    if (event.kind === "StoryEvent" && event.story === "vendetta") storyEvents.show(vendettaEvent(event.beat, event.text, transmission(event.beat, state)));
    // §den-loot: the den's fall, and the loot a freighter has to haul home.
    if (event.kind === "StoryEvent" && event.story === "den_loot" && !state.storyTold.has("den_loot")) {
      const stash = state.founding?.stash;
      const units = Object.values(stash?.goods ?? {}).reduce((sum, n) => sum + (n ?? 0), 0);
      if (units > 0 && !stash?.home) storyEvents.show(denLootEvent(stash!.goods));
    }
    // §privateer-contract: as the hunt begins, the advisor relays the
    // Authority's contract on him.
    if (event.kind === "StoryEvent" && event.story === "privateer_contract") showPrivateerContract(event.title, event.reward);
    // §den-contract: after his last transmission, the Authority's contract on his den.
    if (event.kind === "StoryEvent" && event.story === "den_contract" && !state.storyTold.has("den_contract")) {
      storyEvents.show(denContractEvent(event.title, event.reward, pirateNames(state.founding).privateer, foundingDen(state)?.name ?? null,
        state.founding?.stage !== "defeat_privateer"));
    }
    if (event.kind === "StoryEvent" && event.story === "decree" && !state.storyTold.has(`decree_${event.grip}`)) storyEvents.show(decreeEvent(event.grip));
    if (event.kind === "StoryEvent" && event.story === "crackdown") storyEvents.show(crackdownEvent(event.crackdown, event.at));
    if (event.kind === "StoryEvent" && event.story === "exposed") storyEvents.show(exposedEvent(event.text, event.at));
    if (event.kind === "StoryEvent" && event.story === "journey") storyEvents.show(journeyEvent(event.beat, event.text, event.at));
    if (event.kind === "StoryEvent" && event.story === "ending" && state.season?.ending && !state.storyTold.has("ending")) storyEvents.show(endingEvent(state.season.ending));
    if (event.kind === "StoryEvent" && event.story === "relics" && !state.storyTold.has("relics")) storyEvents.show(relicsEvent());
    // §signal-report: a signal's report reaching command, with its painting.
    if (event.kind === "StoryEvent" && event.story === "signal" && !state.storyTold.has(`signal_${event.signalId}`)) showSignalReport(event.signalId);
    // §chapters: the Pirates, Expansion and Reach chapters open painted.
    if (event.kind === "StoryEvent" && event.story === "chapter" && !state.storyTold.has(`chapter_${event.chapter}`)) {
      const belt = event.chapter === "reach" ? foundingBelt(state) : null;
      const privateer = pirateNames(state.founding).privateer;
      // §advisor-cards: the Pirates chapter opens on the advisor's own scorn.
      const advisor = event.chapter === "pirates" ? advisorOnDuty(state.founding) : null;
      storyEvents.show(chapterEvent(event.chapter, privateer,
        belt?.ore && belt.richness != null ? { ore: belt.ore, richness: belt.richness } : null, voiceOf(state.founding),
        advisor && cardLine(advisor, "chapter_pirates", { privateer })));
    }
    // §advisor-cards: the advisor announces the trader the server dealt.
    if (event.kind === "StoryEvent" && event.story === "free_trader") {
      const trader = traderOf(state.founding);
      const advisor = advisorOnDuty(state.founding);
      storyEvents.showAfterGap(freeTraderEvent(trader,
        advisor && advisorSpeaker(advisor, cardLine(advisor, "free_trader", { trader: `${trader.name}'s ${trader.ship}` }))), FOLLOW_UP_MS);
    }
  }
  activeShell?.onCore(events);
}

/** §laser-battery-card: the raid's report owes the Laser Battery's card.
 * §smelter-raid (Nov 9 2026): a raid can end before the Smelter is staffed,
 * so the card waits for its step (step 8) this session; past it, it lapses.
 * §watch-the-pirate (Nov 9): the step is the battery's once the guide stops
 * watching the raider, and the advisor's scorn at its going comes first;
 * then the advisor recommends the battery in their own words. */
let laserBatteryDue = false;
/** §privateer-contract: the advisor relays the Authority's contract on the
 * privateer; its button opens Contracts on it. Once a session while it waits
 * to be taken on, so a page loaded after its news still hears it. */
function showPrivateerContract(title: string, reward: string): void {
  if (state.storyTold.has("privateer_contract") || storyEvents.has("privateer_contract")) return;
  const advisor = advisorOnDuty(state.founding);
  const privateer = pirateNames(state.founding).privateer;
  // Skipped once taken on or done. The log line can land before the
  // contract's own view (separate lanes), so not yet seen is still due.
  const untaken = () => !state.operations.some((o) => o.briefing?.follow_up === "founder_privateer" && (o.state !== "offered" || o.joined));
  storyEvents.show({ ...privateerContractEvent(title, reward,
    advisor && advisorSpeaker(advisor, cardLine(advisor, "privateer_contract", { privateer })),
    () => activeShell?.openContract?.()), due: untaken });
}

/** §signal-report (docs/signals.md): what a sweep found, as its report
 * reaches command; Show the cache frames what it left for a Freighter. */
function showSignalReport(id: string): void {
  const signal = state.signals?.signals.find((entry) => entry.id === id);
  const report = signal?.report;
  if (!signal || !report) return;
  const fleet = state.ghosts.find((ghost) => ghost.own && ghost.id === report.fleet);
  const cache = report.finds.find((find) => find.kind === "cache");
  storyEvents.show(signalReportEvent(signal, {
    ship: fleet ? shipKindLabel(fleet.kind) : null,
    trader: traderOf(state.founding),
    now: state.simTime,
    programmeName: (programme) => state.research?.programmes.find((entry) => entry.id === programme)?.name ?? programme,
    showCache: cache?.kind === "cache" ? () => {
      const site = state.explorationSites.find((entry) => entry.id === cache.site);
      if (!site) return;
      state.selectedExplorationSiteId = site.id;
      renderer.centerOnWorld(site.pos);
    } : undefined,
  }));
}

/** On every message: an offered contract met on this page's first sight. */
function offerPrivateerContract(): void {
  if (state.founding?.stage !== "defeat_privateer") return;
  const contract = state.operations.find((o) => o.briefing?.follow_up === "founder_privateer" && o.state === "offered" && !o.joined);
  if (contract?.briefing) showPrivateerContract(contract.briefing.title, `${contract.reward.credits} Cr`);
}

function offerLaserBattery(): void {
  const f = state.founding;
  const stage = f?.stage;
  if (!laserBatteryDue || !f || !stage) return;
  if (state.storyTold.has("laser_battery") || FOUNDING_STEP[stage] > FOUNDING_STEP.build_gun_platform) {
    laserBatteryDue = false;
    return;
  }
  if (stage !== "build_gun_platform" || storyEvents.has("laser_battery")) return;
  if ((f.raid_seen && !f.raid_lost) || advisorDesk.leftOwed()) return;
  laserBatteryDue = false;
  const advisor = advisorOnDuty(f);
  storyEvents.showAfterGap({ ...laserBatteryEvent(advisor && advisorSpeaker(advisor, cardLine(advisor, "laser_battery"))),
    due: () => state.founding?.stage === "build_gun_platform" }, LEFT_GAP_MS);
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

// §export-follow (Peter, Nov 2 2026: "the player has to follow the tiny
// freighter for 90 seconds max"): the first shipment's departure takes the
// player to the freighter and follows it out of the well, the way the raid's
// first sighting does, until it warps at the hyperlimit or the step completes
// (founding::EXPORT_FOLLOW_MAX_S after departure at most). Once per ship: a
// player who drags away is not dragged back; the guide's Follow re-engages it.
let shipmentFollowed: string | null = null;
function noticeShipment(): void {
  const home = foundingHomeSystemId();
  const target = shipmentFollowTarget(state, home);
  if (target && home && target.id !== shipmentFollowed) {
    shipmentFollowed = target.id;
    activeShell?.focusShipment?.(target.id, home);
    renderer.trackShipment(target.id, home);
  } else if (!target && shipmentFollowed) {
    shipmentFollowed = null;
    renderer.trackShipment(null);
  }
}

let net!: Net;
const settlement = new SettlementUI(state, message => net.send(message));
// §client-updates: an open tab learns of a newer client and reloads into it.
const checkForUpdate = installClientUpdates();

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
    crates.offer();
    offerLaserBattery();
    offerPrivateerContract();
    noticeRaid();
    noticeShipment();
    settlement.onMessage(message);
    advisorDesk.update();
  },
  onClose: () => {
    present(applyLinkStatus(state.playerId === null ? "offline" : "reconnecting", state));
    settlement.connectionChanged();
  },
  onSessionReplaced: () => {
    present(applySessionReplaced(state));
    settlement.endSession();
    storyEvents.endSession();
    laserBatteryDue = false;
    renderer.trackRaid(null);
    shipmentFollowed = null;
  },
  onError: () => {
    const events = applyLinkStatus(state.playerId === null ? "offline" : "reconnecting", state);
    present([...events, { kind: "TransportError", url: net.url }]);
  },
  onProtocolChanged: () => void checkForUpdate(true),
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

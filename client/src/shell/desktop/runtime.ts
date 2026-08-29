import { bindFleetNet } from "../../core/derive/fleet";
import { bindMarketDerive } from "../../core/derive/market";
import { bindResearchNet } from "../../core/derive/research";
import { type CoreEvent } from "../../core/events";
import { label } from "../../icons";
import { renderer } from "../../render";
import { state } from "../../state";
import { __init_battle_2931, __init_battle_2936, openOngoingBattleId, refreshOpenBattleViewer, refreshOpenGroundViewer, updateOngoingBattlePanel } from "./battle";
import { buildCheckinPanel, computeInbox, updateCheckinPanel } from "./checkin";
import { updateFactionPanel } from "./faction";
import { __init_founding_7165, __init_founding_7356, updateFoundingGuide } from "./founding";
import { __init_mapchrome_142, __init_mapchrome_149, __init_mapchrome_150, __init_mapchrome_314, __init_mapchrome_315, __init_mapchrome_52, __init_mapchrome_53, __init_mapchrome_54, __init_mapchrome_55, __init_mapchrome_56, __init_mapchrome_7576, __init_mapchrome_7592, __init_mapchrome_7598, __init_mapchrome_7599, __init_mapchrome_88, __init_mapchrome_91, __init_mapchrome_92, $, addReport, desktopCameraRect, esc, hud, installInteraction, joinBtn, joinErr, joinScreen, nameInput, onDesktopRenderFrame, readout, setHud, showEngagementEstimate } from "./mapchrome";
import { addTradeNews, buildMarketPanel, marketTab, setMarketTab, updateHubPanel, updateMarket } from "./market";
import { updateOperationsPanel } from "./operations";
import { buildDoctrinePanel, buildRail, buildStandingPanel, buildSystemTab, railTab, setRailTab, updateDoctrinePanel, updateFleetsPanel, updateRankingsPanel, updateStandingPanel, updateSystemTab } from "./rail";
import { updateResearchPanel } from "./research";
import { addTransientReport, buildIntentBar, notifyNewBattles, renderIntentBar, updateOfficersPanel, updateShipPanel } from "./ship";
import { updateSyndicatePanel } from "./syndicate";
import { pendingFit, updateSysviewDynamic } from "./sysview";
import { initDesktopWorkspace, setWorkspaceTitle, type WorkspacePageId } from "./workspace";
import { net } from "./index";


export function __init_index_7136(): void {
bindFleetNet(() => net);
}

export function __init_index_7137(): void {
bindResearchNet(() => net);
}

export function __init_index_7138(): void {
bindMarketDerive(() => net, () => pendingFit);
}


// §perf: coalesce the per-View panel refreshes. Views arrive at ~10 Hz, but if the
// main thread stalls (a GC pause, a background tab that just refocused) the queued
// backlog is delivered in a burst — and re-running the whole DOM-refresh pipeline
// once per queued message turns one hiccup into a multi-frame freeze. Instead we
// stash the work for boot's single rAF: a burst collapses to ONE refresh reading the
// latest state, and background tabs (where rAF doesn't fire) skip panel work
// entirely until refocus. State ingestion + the ongoing-battle high-water tally
// still run inline on every View (see the View handler), so no data is lost.
export let viewRefreshRaf = 0;

export function scheduleViewRefresh(): void {
  viewRefreshRaf = 1; // already queued — the next core-owned frame reads latest state
}


export function applyViewRefresh(): void {
  updateFoundingGuide();
  // Refresh only the currently-visible rail tab — hidden tabs don't churn (they
  // re-render on show via setRailTab). Each updater also guards itself.
  if ($("rail").classList.contains("is-open")) {
    if (railTab === "system") updateSystemTab();
    else if (railTab === "fleets") updateFleetsPanel();
    else if (railTab === "logistics") updateStandingPanel();
    else if (railTab === "doctrine") updateDoctrinePanel();
    else if (railTab === "officers") updateOfficersPanel();
    else if (railTab === "rankings") updateRankingsPanel();
  }
  // The selected-ship panel keeps the information AGE ticking (and handles a
  // contact passing out of view) while it's open.
  if ($("ship-panel").classList.contains("is-open")) updateShipPanel();
  // §syndicates: refresh the alliance roster/invites if the panel is open
  // (guarded by a signature so a half-typed name survives).
  if ($("syndicate-panel").classList.contains("is-open")) updateSyndicatePanel();
  if ($("operations-panel").classList.contains("is-open")) updateOperationsPanel();
  // §TCA: refresh the charter standing if the Faction panel is open (self-guarded).
  updateFactionPanel();
  // §research R6: refresh the Programme Boards if open (coarse signature).
  if ($("research-panel").classList.contains("is-open")) updateResearchPanel();
  // Hub berths are served fleet reports too: keep the open Fleets tab in step
  // with arrivals and departures without revealing the authoritative dock list.
  if ($("hub-panel").classList.contains("is-open")) updateHubPanel();
  // §management-home: inside the System View, refresh the management column +
  // the structure markers (setSystemDynamic is idempotent; the panels self-guard).
  updateSysviewDynamic();
  // §battle-records: keep an open replay viewer live — rounds grow, the light
  // frontier advances, the outcome may arrive (guards itself).
  refreshOpenBattleViewer();
  // The Market is a navbar overlay now — refresh it when open.
  if ($("market").classList.contains("is-open")) updateMarket();
  updateCheckinPanel(); // the check-in modal; guards itself, refreshes ages
  updateNavBadges();
}


function setNavBadge(name: string, count: number): void {
  const el = $(`nav-badge-${name}`);
  el.hidden = count <= 0;
  el.textContent = count > 99 ? "99+" : String(count);
}

export function updateNavBadges(): void {
  if (state.playerId === null) return;
  setNavBadge("market", (state.wallet?.orders.length ?? 0) + (state.freight?.shipments.length ?? 0));
  const ownBattleIds = new Set(state.battles.flatMap((battle) => battle.participants));
  setNavBadge("fleets", state.ghosts.filter((fleet) => fleet.own && (fleet.stalled || fleet.rescue_inbound || ownBattleIds.has(fleet.id))).length);
  setNavBadge("research", state.research?.stalled || (state.research && !state.research.active && state.research.queue.length === 0) ? 1 : 0);
  setNavBadge("officers", state.captains.filter((captain) => !captain.assigned_fleet || (captain.report?.unspent ?? 0) > 0).length);
  setNavBadge("operations", state.operations.filter((operation) => operation.state === "offered" || (operation.state === "active" && !operation.joined)).length);
  setNavBadge("syndicate", state.syndicateInvites.length);
  setNavBadge("faction", (state.charter && state.charter.status !== "good_standing" ? 1 : 0) + (state.diplomacy?.incoming.length ?? 0));
  setNavBadge("log", computeInbox().length);
}


function refreshWorkspacePage(page: WorkspacePageId): void {
  switch (page) {
    case "rail":
      setWorkspaceTitle(railTab === "system" ? "System" : railTab[0].toUpperCase() + railTab.slice(1));
      setRailTab(railTab);
      break;
    case "ship-panel": updateShipPanel(); break;
    case "hub-panel": updateHubPanel(); break;
    case "battle-panel": if (openOngoingBattleId !== null) updateOngoingBattlePanel(); break;
    case "market": setMarketTab(marketTab); break;
    case "research-panel": updateResearchPanel(); break;
    case "operations-panel": updateOperationsPanel(); break;
    case "syndicate-panel": updateSyndicatePanel(); break;
    case "faction-panel": updateFactionPanel(); break;
    case "checkin": updateCheckinPanel(); break;
  }
}


export function handleCoreEvents(events: CoreEvent[]): void {
  for (const event of events) {
    switch (event.kind) {
      case "LinkChanged":
        joinBtn.disabled = event.status !== "offline" || state.playerId !== null;
        break;
      case "GalaxyUpdated":
      case "CommandSignal":
      case "CommandChevron":
      case "BattleConcluded":
        break;
      case "ProtocolMismatch":
        console.warn(`protocol mismatch: server v${event.server}, client expects v${event.client} — a refresh may be needed`);
        break;
      case "Welcomed":
        joinScreen.style.display = "none";
        hud.style.display = "flex";
        $("readout").style.display = "block";
        $("legend").style.display = "block";
        $("zoom-controls").style.display = "flex";
        buildRail();
        buildSystemTab();
        buildMarketPanel();
        buildStandingPanel();
        buildDoctrinePanel();
        updateDoctrinePanel();
        setRailTab("system");
        buildCheckinPanel();
        updateNavBadges();
        if (!interactionInstalled) {
          installInteraction();
          interactionInstalled = true;
        }
        break;
      case "SessionReplaced":
        joinScreen.style.display = "flex";
        joinErr.textContent = "Signed out: this corporation was opened in another browser.";
        joinBtn.disabled = false;
        break;
      case "ViewApplied":
        renderer.stateVersion++;
        notifyNewBattles(state.battles);
        if (openOngoingBattleId !== null && $("battle-panel").classList.contains("is-open")) updateOngoingBattlePanel();
        scheduleViewRefresh();
        break;
      case "BattleRecordsApplied":
        refreshOpenBattleViewer();
        break;
      case "GroundRecordsApplied":
        refreshOpenGroundViewer();
        break;
      case "SectionsApplied":
        scheduleViewRefresh();
        break;
      case "OrderConfirmed":
        if (event.orderKind === "hold") {
          delete state.orders[event.shipId];
          delete state.raids[event.shipId];
        }
        addTransientReport(
          "✓",
          "good",
          `<b>Order confirmed</b> — ${esc(label(event.orderKind))} response light arrived`,
        );
        break;
      case "ReportArrived":
        addReport(event.report);
        break;
      case "EstimateReady":
        showEngagementEstimate(event.estimate);
        break;
      case "TimelineApplied":
        updateCheckinPanel();
        break;
      case "TradeSettled":
        addTradeNews(event.trade);
        break;
      case "IntentChanged":
        if (event.renderIntentBar) {
          buildIntentBar();
          renderIntentBar();
        }
        if (event.refreshShip) updateShipPanel();
        if (event.readout !== undefined) readout().innerHTML = event.readout;
        break;
      case "JoinRejected":
        joinErr.textContent = event.message;
        joinBtn.disabled = false;
        break;
      case "TransportError":
        if (state.playerId === null) joinErr.textContent = `Could not reach server at ${event.url}.`;
        break;
      case "ServerError":
        readout().innerHTML = `<span style="color:var(--warn)">Server refused: ${esc(event.message)}</span>`;
        addTransientReport("!", "bad", `<b>Order refused</b> — ${esc(event.message)}`);
        break;
    }
  }
  setHud();
}


export let interactionInstalled = false;

export function join(): void {
  const name = nameInput.value.trim();
  if (!name) {
    joinErr.textContent = "Enter a corporation name.";
    return;
  }
  joinErr.textContent = "";
  joinBtn.disabled = true;
  state.name = name;
  setHud();
  if (net?.connected) net.join(name);
  else net?.connect();
}


export function __init_index_7571(): void {
joinBtn.addEventListener("click", join);
}

export function __init_index_7572(): void {
nameInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") join();
});
}

export function __init_index_7575(): void {
nameInput.focus();
}


export { desktopCameraRect };

export function onDesktopViewTick(): void {
  onDesktopRenderFrame();
  if (!viewRefreshRaf) return;
  viewRefreshRaf = 0;
  applyViewRefresh();
}

export function teardownDesktop(): void {
  viewRefreshRaf = 0;
}

export let desktopMounted = false;

export function mountDesktop(): void {
  if (desktopMounted) return;
  desktopMounted = true;
  initDesktopWorkspace(refreshWorkspacePage);
  __init_mapchrome_52();
  __init_mapchrome_53();
  __init_mapchrome_54();
  __init_mapchrome_55();
  __init_mapchrome_56();
  __init_mapchrome_88();
  __init_mapchrome_91();
  __init_mapchrome_92();
  __init_mapchrome_142();
  __init_mapchrome_149();
  __init_mapchrome_150();
  __init_mapchrome_314();
  __init_mapchrome_315();
  __init_battle_2931();
  __init_battle_2936();
  __init_index_7136();
  __init_index_7137();
  __init_index_7138();
  __init_founding_7165();
  __init_founding_7356();
  __init_index_7571();
  __init_index_7572();
  __init_index_7575();
  __init_mapchrome_7576();
  __init_mapchrome_7592();
  __init_mapchrome_7598();
  __init_mapchrome_7599();
}

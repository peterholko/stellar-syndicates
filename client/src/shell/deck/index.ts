import "../../styles/deck.css";

import { reservedMarketCredits, spendableMarketCredits } from "../../core/derive/market";
import type { CoreEvent } from "../../core/events";
import { formatId } from "../../protocol";
import { installPressGuard } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext, Rect, Shell } from "../types";
import { installDeckDebug } from "./debug";
import { type SelectTarget } from "../../core/mapclick";
import { DeckMapInteraction } from "./map";
import { mountDeckMarkup } from "./markup";
import { DECK_ROUTES, DeckRouter, type DeckCrumb, type DeckRoute, type DeckRouteName } from "./router";
import { DeckCommandStrip } from "./strip";
import { DeckToasts } from "./toasts";
import { DeckWorkspace } from "./workspace";

const byId = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;

class DeckShell implements Shell {
  private root: HTMLElement | null = null;
  private ctx: CoreContext | null = null;
  private abort: AbortController | null = null;
  private router: DeckRouter | null = null;
  private workspace: DeckWorkspace | null = null;
  private map: DeckMapInteraction | null = null;
  private toasts: DeckToasts | null = null;
  private strip: DeckCommandStrip | null = null;
  private removeDebug: (() => void) | null = null;
  private chromeSignature = "";
  private zoomSignature = "";
  private activeCrumbs: readonly DeckCrumb[] = [];

  async mount(root: HTMLElement, ctx: CoreContext): Promise<void> {
    this.root = root;
    this.ctx = ctx;
    this.abort = new AbortController();
    mountDeckMarkup(root);
    installPressGuard();
    const signal = this.abort.signal;
    this.router = new DeckRouter((route, stack) => this.routeChanged(route, stack), signal);
    this.workspace = new DeckWorkspace(byId("deck-workspace"), ctx.renderer, signal);
    this.map = new DeckMapInteraction(ctx, {
      enteredSystem: (system) => this.router?.go({ name: "system", params: { id: system.id, systemLabel: system.name } }),
      returnedToGalaxy: () => {
        if (this.router?.current?.name === "system" || this.router?.current?.name === "world" || this.router?.current?.name === "build") {
          this.router.back();
        }
      },
      openTarget: (target) => this.openMapTarget(target),
      notice: () => { /* D1.5 promotes resolver readouts into the strip status line. */ },
    }, signal);
    this.toasts = new DeckToasts(byId("deck-toast-lane"), (route) => this.router?.go(route), signal);
    this.strip = new DeckCommandStrip(byId("deck-command-strip"));
    this.removeDebug = installDeckDebug(ctx, (id) => this.router?.go({ name: "battle", params: { id, label: "Theater demo" } }));
    byId<HTMLFormElement>("deck-join-form").addEventListener("submit", (event) => {
      event.preventDefault();
      this.join();
    }, { signal });
    byId("deck-nav").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act=route]");
      const route = button?.dataset.route as DeckRouteName | undefined;
      if (route && route in DECK_ROUTES) this.openRoute(route);
    }, { signal });
    byId("deck-workspace").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (!button) return;
      if (button.dataset.deckAct === "back") this.router?.back();
      else if (button.dataset.deckAct === "close") this.router?.close();
      else if (button.dataset.deckAct === "width") this.workspace?.toggleWidth();
      else if (button.dataset.deckAct === "breadcrumb") {
        const crumb = this.activeCrumbs[Number(button.dataset.crumbIndex)];
        if (crumb) this.router?.go(crumb.route);
      }
    }, { signal });
    byId("deck-zoom").addEventListener("click", (event) => {
      const action = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]")?.dataset.deckAct;
      if (action === "zoom-in") this.map?.zoomIn();
      else if (action === "zoom-out") this.map?.zoomOut();
      else if (action === "zoom-fit") this.map?.fit();
      else if (action === "help") this.setHelpOpen(true);
    }, { signal });
    byId("deck-overlays").addEventListener("click", (event) => {
      const action = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]")?.dataset.deckAct;
      if (action === "close-help") this.setHelpOpen(false);
    }, { signal });
    window.addEventListener("keydown", (event) => {
      if (event.key !== "Escape") return;
      if (!byId("deck-help").hidden) {
        event.preventDefault();
        this.setHelpOpen(false);
      } else if (ctx.renderer.isSystemScrubbing()) {
        event.preventDefault();
        ctx.renderer.cancelSystemScrub();
      } else if (this.router?.current) {
        event.preventDefault();
        this.router.back();
      }
    }, { signal });
    this.syncSessionVisibility();
    this.renderChrome(true);
    if (ctx.state.playerId !== null) this.openRoute("command");
    else requestAnimationFrame(() => byId<HTMLInputElement>("deck-name").focus());
  }

  onCore(events: CoreEvent[]): void {
    for (const event of events) {
      if (event.kind === "Welcomed") {
        this.syncSessionVisibility();
        this.openRoute("command");
      } else if (event.kind === "SessionReplaced") {
        byId("deck-join-error").textContent = "Signed out: this corporation was opened in another browser.";
        byId<HTMLButtonElement>("deck-join-button").disabled = false;
        this.syncSessionVisibility();
      } else if (event.kind === "JoinRejected") {
        byId("deck-join-error").textContent = event.message;
        byId<HTMLButtonElement>("deck-join-button").disabled = false;
      } else if (event.kind === "TransportError" && this.ctx?.state.playerId === null) {
        byId("deck-join-error").textContent = `Could not reach server at ${event.url}.`;
        byId<HTMLButtonElement>("deck-join-button").disabled = false;
      } else if (event.kind === "ProtocolMismatch") {
        console.warn(`protocol mismatch: server v${event.server}, client expects v${event.client}`);
      }
      this.toastFor(event);
    }
    this.syncSessionVisibility();
    this.renderChrome(true);
  }

  onViewTick(): void {
    this.map?.tick();
    this.renderChrome();
    this.renderZoom();
  }

  framePolicy() {
    // Only opaque Deck overlays may rest the galaxy ticker. Workspace and
    // chrome always leave it live; future theaters join this single predicate.
    const join = document.getElementById("deck-join");
    const help = document.getElementById("deck-help");
    const coveringOverlay = (join !== null && !join.hidden) || (help !== null && !help.hidden);
    return { maxFps: 0, renderGalaxy: !coveringOverlay };
  }

  cameraRect(): Rect {
    return this.workspace?.cameraRect() ?? { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }

  teardown(): void {
    this.abort?.abort();
    this.removeDebug?.();
    this.router?.teardown();
    this.workspace?.teardown();
    this.map?.teardown();
    this.toasts?.teardown();
    this.strip?.clear();
    this.router = null;
    this.workspace = null;
    this.map = null;
    this.toasts = null;
    this.strip = null;
    this.removeDebug = null;
    this.abort = null;
    this.root?.replaceChildren();
    this.root = null;
    this.ctx = null;
    this.chromeSignature = "";
    this.zoomSignature = "";
    this.activeCrumbs = [];
  }

  private join(): void {
    if (!this.ctx) return;
    const name = byId<HTMLInputElement>("deck-name").value.trim();
    if (!name) {
      byId("deck-join-error").textContent = "Enter a corporation name.";
      return;
    }
    byId("deck-join-error").textContent = "";
    byId<HTMLButtonElement>("deck-join-button").disabled = true;
    this.ctx.state.name = name;
    if (this.ctx.net.connected) this.ctx.net.join(name);
    else this.ctx.net.connect();
    this.renderChrome(true);
  }

  private openRoute(name: DeckRouteName): void {
    this.router?.go({ name });
  }

  private openMapTarget(target: SelectTarget): void {
    if (!this.ctx || !this.router) return;
    const state = this.ctx.state;
    switch (target.type) {
      case "fleet": {
        const fleet = state.ghosts.find((entry) => entry.id === target.id);
        this.router.go({ name: "fleet", params: { id: target.id, fleetLabel: fleet ? humanize(fleet.kind) : "Fleet" } });
        break;
      }
      case "system": {
        const system = state.galaxy?.systems.find((entry) => entry.id === target.id);
        this.router.go({ name: "system", params: { id: target.id, systemLabel: system?.name ?? "System" } });
        break;
      }
      case "hub":
        this.router.go({ name: "market" });
        break;
      case "ongoingBattle":
        this.router.go({ name: "battle", params: { id: target.id, label: "Ongoing battle" } });
        break;
      case "aftermath":
      case "capture":
        this.router.go({ name: "log", params: { marker: String(target.id), label: "Battle report" } });
        break;
      case "systemBody": {
        const systemId = this.ctx.renderer.viewMode.type === "system" ? this.ctx.renderer.viewMode.systemId : state.selectedSystemId ?? "";
        this.router.go({
          name: "world",
          params: { systemId, bodyId: String(target.detail.id), worldLabel: target.detail.name ?? "World" },
        });
        break;
      }
      case "emplacement":
        this.router.go({ name: "fleet", params: { id: target.id, fleetLabel: "Installation" } });
        break;
      case "jumpDeparture":
        this.ctx.renderer.selectedJumpDepartureKey = target.key;
        this.router.go({ name: "fleet", params: { id: target.key, fleetLabel: "Jump departure" } });
        break;
      case "anchor":
        this.router.go({ name: "command" });
        break;
      case "clearSystemBody":
        if (this.router.current?.name === "world") this.router.back();
        break;
    }
  }

  private routeChanged(route: DeckRoute | null, stack: readonly DeckRoute[]): void {
    const semanticRoute = route?.name === "system" || route?.name === "world" || route?.name === "build";
    if (!semanticRoute && this.ctx?.renderer.viewMode.type === "system" && !this.ctx.renderer.isSystemScrubbing()) {
      this.ctx.renderer.exitSystemView();
      this.ctx.renderer.setSystemDynamic([], [], true);
    }
    if (!route || !this.workspace || !this.router) {
      this.activeCrumbs = [];
      this.workspace?.close();
      this.renderActiveNav(null);
      return;
    }
    this.activeCrumbs = this.router.breadcrumbs(route);
    this.workspace.show(route, this.activeCrumbs, stack.length > 1);
    this.renderPlaceholder(route);
    this.renderActiveNav(route.name);
  }

  private renderPlaceholder(route: DeckRoute): void {
    const body = byId("deck-workspace-body");
    body.replaceChildren();
    const placeholder = document.createElement("div");
    placeholder.className = "deck-placeholder";
    const eyebrow = document.createElement("span");
    eyebrow.textContent = "Route scaffold";
    const title = document.createElement("b");
    title.textContent = DECK_ROUTES[route.name].title;
    const copy = document.createElement("p");
    copy.textContent = "Operational content lands in its scheduled Deck phase.";
    placeholder.append(eyebrow, title, copy);
    body.append(placeholder);
  }

  private renderActiveNav(name: DeckRouteName | null): void {
    for (const button of byId("deck-nav").querySelectorAll<HTMLButtonElement>("[data-route]")) {
      if (button.dataset.route === name) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
  }

  private setHelpOpen(open: boolean): void {
    byId("deck-help").hidden = !open;
  }

  private renderZoom(): void {
    if (!this.ctx) return;
    const mode = this.ctx.renderer.viewMode.type;
    const zoom = this.ctx.renderer.zoomFactor();
    const text = mode === "system" ? "SYSTEM" : mode === "battle" ? "BATTLE" : zoom < 10 ? `${zoom.toFixed(1)}×` : `${Math.round(zoom)}×`;
    if (text === this.zoomSignature) return;
    this.zoomSignature = text;
    const level = byId("deck-zoom-level");
    level.textContent = text;
    level.title = mode === "galaxy" ? `${zoom.toFixed(2)}× galaxy magnification relative to fit` : `${text.toLowerCase()} semantic view`;
  }

  private toastFor(event: CoreEvent): void {
    if (!this.toasts) return;
    if (event.kind === "OrderConfirmed") {
      this.toasts.push({
        title: "Order confirmed",
        message: humanize(event.orderKind),
        tone: "good",
        destination: { name: "fleet", params: { id: event.shipId } },
      });
    } else if (event.kind === "ReportArrived") {
      this.toasts.push({
        title: "Combat report arrived",
        message: `${humanize(event.report.outcome)} · delayed ${Math.round(event.report.age)}s`,
        tone: event.report.outcome === "target_destroyed" && event.report.you === "attacker" ? "good" : "warn",
        destination: { name: "log" },
      });
    } else if (event.kind === "BattleConcluded") {
      this.toasts.push({
        title: "Battle concluded",
        message: humanize(event.outcome),
        tone: event.outcome === "target_destroyed" ? "good" : "warn",
        destination: { name: "battle", params: { id: event.recordId } },
      });
    } else if (event.kind === "EstimateReady") {
      const win = event.estimate.win_pct == null ? "Projection ready" : `${Math.round(event.estimate.win_pct)}% projected win chance`;
      this.toasts.push({
        title: "Engagement estimate",
        message: win,
        destination: { name: "fleet", params: { id: event.estimate.target } },
        durationMs: 15_000,
      });
    } else if (event.kind === "TradeSettled") {
      this.toasts.push({
        title: "Market update",
        message: humanize(event.trade.event),
        tone: event.trade.event === "Rejected" || event.trade.event === "StorageOverflow" ? "warn" : "quiet",
        destination: { name: "market" },
      });
    } else if (event.kind === "ServerError") {
      this.toasts.push({ title: "Command refused", message: event.message, tone: "bad", destination: { name: "log" } });
    }
  }

  private syncSessionVisibility(): void {
    if (!this.ctx) return;
    const ready = this.ctx.state.playerId !== null;
    byId("deck-join").hidden = ready;
    byId("deck-topbar").hidden = !ready;
    if (!ready) {
      this.workspace?.close();
      const connecting = this.ctx.state.link === "connecting" || this.ctx.state.link === "reconnecting";
      byId<HTMLButtonElement>("deck-join-button").disabled = connecting && !!this.ctx.state.name;
    }
  }

  private renderChrome(force = false): void {
    if (!this.ctx) return;
    const state = this.ctx.state;
    const reserved = reservedMarketCredits();
    const counts = this.navBadgeCounts();
    const signature = sheetFingerprint([
      state.name, state.playerId, state.link, state.tick, state.simTime, state.pacingScale,
      state.wallet, reserved, counts,
    ]);
    if (!force && signature === this.chromeSignature) return;
    this.chromeSignature = signature;
    byId("deck-corp").textContent = state.name || "—";
    byId("deck-corp-id").textContent = state.playerId !== null ? formatId(state.playerId) : "—";
    byId("deck-credits").textContent = state.wallet ? `${reserved > 0 ? "~" : ""}${Math.round(spendableMarketCredits()).toLocaleString()}` : "—";
    byId("deck-reserved").textContent = reserved > 0 ? `${Math.round(reserved).toLocaleString()} reserved` : "";
    byId("deck-equity").textContent = state.wallet ? Math.round(state.wallet.valuation).toLocaleString() : "—";
    byId("deck-tick").textContent = state.link === "online" ? state.tick.toLocaleString() : "—";
    byId("deck-pacing").textContent = state.pacingScale !== 1 ? `FAST ${state.pacingScale}×` : "";
    const link = byId("deck-link");
    link.textContent = state.link === "online" ? "● online" : state.link === "reconnecting" ? "reconnecting…" : state.link === "connecting" ? "connecting…" : "offline";
    link.classList.toggle("is-online", state.link === "online");
    for (const [route, count] of Object.entries(counts)) {
      const badge = byId(`deck-badge-${route}`);
      badge.textContent = count > 99 ? "99+" : String(count);
      badge.hidden = count <= 0;
    }
  }

  private navBadgeCounts(): Record<string, number> {
    const state = this.ctx!.state;
    const ownBattleIds = new Set(state.battles.flatMap((battle) => battle.participants));
    const logDecisions = (state.founding && state.founding.stage !== "complete" ? 1 : 0)
      + (state.diplomacy?.incoming.length ?? 0)
      + state.syndicateInvites.length
      + state.operations.filter((operation) => operation.state === "offered" && !operation.joined).slice(0, 3).length
      + state.battles.filter((battle) => battle.own).length;
    return {
      fleets: state.ghosts.filter((fleet) => fleet.own && (fleet.stalled || fleet.rescue_inbound || ownBattleIds.has(fleet.id))).length,
      market: (state.wallet?.orders.length ?? 0) + (state.freight?.shipments.length ?? 0),
      research: state.research?.stalled || (state.research && !state.research.active && state.research.queue.length === 0) ? 1 : 0,
      officers: state.captains.filter((captain) => !captain.assigned_fleet || (captain.report?.unspent ?? 0) > 0).length,
      operations: state.operations.filter((operation) => operation.state === "offered" || (operation.state === "active" && !operation.joined)).length,
      syndicate: state.syndicateInvites.length,
      faction: (state.charter && state.charter.status !== "good_standing" ? 1 : 0) + (state.diplomacy?.incoming.length ?? 0),
      log: logDecisions,
    };
  }
}

export const createShell = (): Shell => new DeckShell();

function humanize(value: string): string {
  return value.replace(/_/g, " ").replace(/([a-z])([A-Z])/g, "$1 $2").replace(/^./, (letter) => letter.toUpperCase());
}

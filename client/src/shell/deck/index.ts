import "../../styles/deck.css";

import { guardCapable as guardCapableForKey, jumpCapable as jumpCapableForKey, shipKindLabel } from "../../core/derive/fleet";
import { bindMarketDerive, reservedMarketCredits, spendableMarketCredits } from "../../core/derive/market";
import type { CoreEvent } from "../../core/events";
import { formatId } from "../../protocol";
import { installPressGuard } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext, Rect, Shell } from "../types";
import { installDeckDebug } from "./debug";
import { DeckEmpireRoutes } from "./empire";
import { type SelectTarget } from "../../core/mapclick";
import { DeckMapInteraction } from "./map";
import { mountDeckMarkup } from "./markup";
import { DECK_ROUTES, DeckRouter, type DeckCrumb, type DeckRoute, type DeckRouteName } from "./router";
import { DeckCommandStrip } from "./strip";
import { DeckCommandRoutes } from "./command";
import { DeckMarketRoutes } from "./market";
import { DeckPolicyRoutes } from "./policy";
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
  private empire: DeckEmpireRoutes | null = null;
  private command: DeckCommandRoutes | null = null;
  private market: DeckMarketRoutes | null = null;
  private policy: DeckPolicyRoutes | null = null;
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
      notice: (html) => this.setStatus(html),
    }, signal);
    this.toasts = new DeckToasts(byId("deck-toast-lane"), (route) => this.router?.go(route), signal);
    this.strip = new DeckCommandStrip(byId("deck-command-strip"), ctx, {
      notice: (html) => this.setStatus(html),
    }, signal);
    this.empire = new DeckEmpireRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      notice: (html) => this.setStatus(html),
      toast: (title, message, tone, destination) => this.toasts?.push({ title, message, tone, destination }),
    });
    this.command = new DeckCommandRoutes(byId("deck-workspace-body"), byId("deck-founding"), ctx, {
      go: (route) => this.router?.go(route),
      focusFleet: (id) => this.map?.focusFleet(id),
      notice: (html) => this.setStatus(html),
    });
    this.market = new DeckMarketRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      notice: (html) => this.setStatus(html),
    });
    this.policy = new DeckPolicyRoutes(byId("deck-workspace-body"), ctx, {
      notice: (html) => this.setStatus(html),
    });
    bindMarketDerive(() => ctx.net, () => this.empire?.composedFit ?? []);
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
      if (this.command?.handleWorkspaceAction(button, this.router?.current ?? null)) return;
      if (this.empire?.handleAction(button, this.router?.current ?? null)) return;
      if (this.market?.handleAction(button, this.router?.current ?? null)) return;
      if (this.policy?.handleAction(button, this.router?.current ?? null)) return;
      if (button.dataset.deckAct === "back") this.router?.back();
      else if (button.dataset.deckAct === "close") this.router?.close();
      else if (button.dataset.deckAct === "width") this.workspace?.toggleWidth();
      else if (button.dataset.deckAct === "breadcrumb") {
        const crumb = this.activeCrumbs[Number(button.dataset.crumbIndex)];
        if (crumb?.route) this.router?.go(crumb.route);
        else if (crumb) this.router?.close();
      }
    }, { signal });
    const workspaceInput = (event: Event) => {
      const target = event.target;
      if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement) {
        // Native selects and checkboxes emit both input and change. Route each
        // control through one event only so a doctrine selection sends once.
        if (event.type === "input" && (target instanceof HTMLSelectElement || target.type === "checkbox")) return;
        if (event.type === "change" && target instanceof HTMLInputElement && target.type !== "checkbox") return;
        if (this.market?.handleInput(target, this.router?.current ?? null)) return;
        this.policy?.handleInput(target, this.router?.current ?? null);
      }
    };
    byId("deck-workspace").addEventListener("input", workspaceInput, { signal });
    byId("deck-workspace").addEventListener("change", workspaceInput, { signal });
    byId("deck-founding").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (button) this.command?.handleFoundingAction(button);
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
    window.addEventListener("keydown", (event) => this.keyDown(event), { signal });
    this.syncSessionVisibility();
    this.renderChrome(true);
    if (ctx.state.playerId !== null) this.openRoute("command");
    else requestAnimationFrame(() => byId<HTMLInputElement>("deck-name").focus());
  }

  onCore(events: CoreEvent[]): void {
    this.empire?.onCore(events, this.router?.current ?? null);
    this.market?.onCore(events, this.router?.current ?? null);
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
      if (event.kind === "IntentChanged" && event.readout) {
        this.setStatus(event.readout);
      } else if (event.kind === "OrderConfirmed") {
        const fleet = this.ctx?.state.ghosts.find((entry) => entry.id === event.shipId);
        this.setStatus(`<b>Order confirmed</b> · ${escapeHtml(humanize(event.orderKind))}${fleet ? ` · ${escapeHtml(shipKindLabel(fleet.kind))}` : ""}`);
      } else if (event.kind === "ServerError") {
        this.setStatus(`<span class="deck-command-status__error"><b>Command refused</b> · ${escapeHtml(event.message)}</span>`);
      }
      this.toastFor(event);
    }
    this.syncSessionVisibility();
    this.renderChrome(true);
    this.strip?.render(true);
  }

  onViewTick(): void {
    this.map?.tick();
    this.strip?.render();
    this.renderChrome();
    this.renderZoom();
    this.empire?.render(this.router?.current ?? null);
    this.command?.render(this.router?.current ?? null);
    this.market?.render(this.router?.current ?? null);
    this.policy?.render(this.router?.current ?? null);
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
    this.empire?.invalidate();
    this.command?.teardown();
    this.market?.invalidate();
    this.policy?.invalidate();
    this.router = null;
    this.workspace = null;
    this.map = null;
    this.toasts = null;
    this.strip = null;
    this.empire = null;
    this.command = null;
    this.market = null;
    this.policy = null;
    bindMarketDerive(() => null, () => []);
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

  private setStatus(html: string): void {
    this.strip?.setStatus(html);
  }

  private keyDown(event: KeyboardEvent): void {
    if (!this.ctx || this.editableTarget(event.target)) return;
    const key = event.key;
    if (key === "Enter" && this.ctx.state.pendingIntent) {
      event.preventDefault();
      this.ctx.intent.confirmPendingIntent();
      this.strip?.render(true);
      return;
    }
    if (key === "Escape") {
      if (this.escapeOneLayer()) event.preventDefault();
      return;
    }
    const routeKeys: Partial<Record<string, DeckRouteName>> = {
      m: "market", v: "fleets", r: "research", p: "officers",
      u: "operations", y: "syndicate", c: "faction", l: "log", s: "system",
    };
    const route = routeKeys[key.toLowerCase()];
    if (route) {
      event.preventDefault();
      this.openRoute(route);
      return;
    }
    const fleet = this.ctx.state.selectedShipId
      ? this.ctx.state.ghosts.find((entry) => entry.id === this.ctx!.state.selectedShipId && entry.own)
      : undefined;
    if (key.toLowerCase() === "j" && fleet && jumpCapableForKey(fleet)) {
      event.preventDefault();
      this.ctx.intent.armJumpAiming(fleet);
      this.strip?.render(true);
    } else if (key.toLowerCase() === "g" && fleet && guardCapableForKey(fleet)) {
      event.preventDefault();
      this.ctx.intent.armGuardAiming(fleet);
      this.strip?.render(true);
    } else if (key === "[") {
      event.preventDefault();
      this.map?.cycleFleet(-1);
      this.strip?.render(true);
    } else if (key === "]") {
      event.preventDefault();
      this.map?.cycleFleet(1);
      this.strip?.render(true);
    } else if (key === "+" || key === "=") {
      event.preventDefault();
      this.map?.zoomIn();
    } else if (key === "-" || key === "_") {
      event.preventDefault();
      this.map?.zoomOut();
    } else if (key === "?") {
      event.preventDefault();
      this.setHelpOpen(true);
    }
  }

  private escapeOneLayer(): boolean {
    if (!this.ctx) return false;
    if (this.ctx.state.pendingIntent) {
      this.ctx.intent.clearPendingIntent();
    } else if (this.ctx.intent.intentAiming.jump) {
      this.ctx.intent.clearJumpAiming();
    } else if (this.ctx.intent.intentAiming.guard) {
      this.ctx.intent.clearGuardAiming();
    } else if (this.ctx.renderer.isSystemScrubbing()) {
      this.ctx.renderer.cancelSystemScrub();
    } else if (!byId("deck-help").hidden) {
      this.setHelpOpen(false);
    } else if (this.ctx.renderer.viewMode.type === "battle") {
      this.ctx.renderer.exitBattleView();
      if (this.router?.current) this.router.back();
    } else if (this.router?.current) {
      this.router.back();
    } else if (this.ctx.state.selectedShipId || this.ctx.state.selectedShipIds.size) {
      this.ctx.intent.clearPendingIntent(true);
      this.ctx.intent.clearJumpAiming(true);
      this.ctx.intent.clearGuardAiming(true);
      this.ctx.state.selectedShipId = null;
      this.ctx.state.selectedShipIds.clear();
      this.ctx.state.selectedOrderId = null;
      this.ctx.renderer.stateVersion++;
    } else {
      return false;
    }
    this.strip?.render(true);
    return true;
  }

  private editableTarget(target: EventTarget | null): boolean {
    return target instanceof HTMLInputElement
      || target instanceof HTMLSelectElement
      || target instanceof HTMLTextAreaElement
      || (target instanceof HTMLElement && target.isContentEditable);
  }

  private openMapTarget(target: SelectTarget): void {
    if (!this.ctx || !this.router) return;
    const state = this.ctx.state;
    switch (target.type) {
      case "fleet": {
        const fleet = state.ghosts.find((entry) => entry.id === target.id);
        this.router.go({ name: "fleet", params: { id: target.id, fleetLabel: fleet ? shipKindLabel(fleet.kind) : "Fleet" } });
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
        this.ctx.renderer.pulseSystemBody(String(target.detail.id));
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
    if (semanticRoute && this.ctx) {
      const systemId = route.params?.systemId ?? route.params?.id;
      const dynamic = this.ctx.state.systems.find((entry) => entry.id === systemId);
      if (systemId) {
        this.ctx.state.selectedSystemId = systemId;
        this.ctx.renderer.setSystemDynamic(
          dynamic?.bodies ?? [],
          (dynamic?.builds ?? []).map((build) => ({ key: build.key, body_id: build.body_id })),
          dynamic?.habitat_fed ?? true,
        );
      }
      if (route.name === "world" && route.params?.bodyId) this.ctx.renderer.pulseSystemBody(route.params.bodyId);
    }
    this.activeCrumbs = this.router.breadcrumbs(route);
    this.workspace.show(route, this.activeCrumbs, stack.length > 1);
    const commandHandled = this.command?.render(route, true) ?? false;
    const empireHandled = this.empire?.render(route, true) ?? false;
    const marketHandled = this.market?.render(route, true) ?? false;
    const policyHandled = this.policy?.render(route, true) ?? false;
    if (!commandHandled && !empireHandled && !marketHandled && !policyHandled) this.renderPlaceholder(route);
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

function escapeHtml(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;",
  })[character]!);
}

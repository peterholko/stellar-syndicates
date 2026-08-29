import "../../styles/deck.css";

import { reservedMarketCredits, spendableMarketCredits } from "../../core/derive/market";
import type { CoreEvent } from "../../core/events";
import { formatId } from "../../protocol";
import { installPressGuard } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext, Rect, Shell } from "../types";
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
  private toasts: DeckToasts | null = null;
  private strip: DeckCommandStrip | null = null;
  private chromeSignature = "";
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
    this.toasts = new DeckToasts(byId("deck-toast-lane"));
    this.strip = new DeckCommandStrip(byId("deck-command-strip"));
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
    }
    this.syncSessionVisibility();
    this.renderChrome(true);
  }

  onViewTick(): void {
    this.renderChrome();
  }

  framePolicy() {
    return { maxFps: 0, renderGalaxy: true };
  }

  cameraRect(): Rect {
    return this.workspace?.cameraRect() ?? { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }

  teardown(): void {
    this.abort?.abort();
    this.router?.teardown();
    this.workspace?.teardown();
    this.toasts?.teardown();
    this.strip?.clear();
    this.router = null;
    this.workspace = null;
    this.toasts = null;
    this.strip = null;
    this.abort = null;
    this.root?.replaceChildren();
    this.root = null;
    this.ctx = null;
    this.chromeSignature = "";
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

  private routeChanged(route: DeckRoute | null, stack: readonly DeckRoute[]): void {
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

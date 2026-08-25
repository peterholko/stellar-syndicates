import "../../styles/mobile.css";

import { reservedMarketCredits, spendableMarketCredits } from "../../core/derive/market";
import { formatId } from "../../protocol";
import type { CoreEvent } from "../../core/events";
import type { CoreContext, Rect, Shell } from "../types";
import { mountMobileMarkup } from "./markup";

const byId = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;

class MobileShell implements Shell {
  private root: HTMLElement | null = null;
  private ctx: CoreContext | null = null;
  private abort: AbortController | null = null;
  private statusSignature = "";

  async mount(root: HTMLElement, ctx: CoreContext): Promise<void> {
    this.root = root;
    this.ctx = ctx;
    this.abort = new AbortController();
    mountMobileMarkup(root);

    const signal = this.abort.signal;
    byId("m-status-toggle").addEventListener("click", () => this.toggleStatus(), { signal });
    byId<HTMLFormElement>("m-join-form").addEventListener("submit", (event) => {
      event.preventDefault();
      this.join();
    }, { signal });
    byId("m-tabs").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("button[data-destination]");
      if (button) this.selectDestination(button.dataset.destination ?? "");
    }, { signal });

    this.syncSessionVisibility();
    this.renderStatus(true);
    if (ctx.state.playerId === null) byId<HTMLInputElement>("m-name").focus();

    const existing = (window as unknown as { __ss?: Record<string, unknown> }).__ss ?? {};
    (window as unknown as { __ss: Record<string, unknown> }).__ss = {
      ...existing,
      state: ctx.state,
      renderer: ctx.renderer,
      net: ctx.net,
    };
  }

  onCore(events: CoreEvent[]): void {
    for (const event of events) {
      if (event.kind === "JoinRejected") {
        byId("m-join-error").textContent = event.message;
        byId<HTMLButtonElement>("m-join-button").disabled = false;
      } else if (event.kind === "TransportError" && this.ctx?.state.playerId === null) {
        byId("m-join-error").textContent = `Could not reach server at ${event.url}.`;
      } else if (event.kind === "ProtocolMismatch") {
        console.warn(`protocol mismatch: server v${event.server}, client expects v${event.client}`);
      }
    }
    this.syncSessionVisibility();
    this.renderStatus(true);
  }

  onViewTick(): void {
    this.renderStatus();
  }

  cameraRect(): Rect {
    return { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }

  teardown(): void {
    this.abort?.abort();
    this.abort = null;
    this.root?.replaceChildren();
    this.root = null;
    this.ctx = null;
  }

  private join(): void {
    if (!this.ctx) return;
    const name = byId<HTMLInputElement>("m-name").value.trim();
    if (!name) {
      byId("m-join-error").textContent = "Enter a corporation name.";
      return;
    }
    byId("m-join-error").textContent = "";
    byId<HTMLButtonElement>("m-join-button").disabled = true;
    this.ctx.state.name = name;
    if (this.ctx.net.connected) this.ctx.send({ type: "Join", name });
    else this.ctx.net.connect();
    this.renderStatus(true);
  }

  private toggleStatus(): void {
    const button = byId<HTMLButtonElement>("m-status-toggle");
    const expanded = button.getAttribute("aria-expanded") !== "true";
    button.setAttribute("aria-expanded", String(expanded));
    byId("m-status-more").hidden = !expanded;
  }

  private selectDestination(destination: string): void {
    for (const button of byId("m-tabs").querySelectorAll<HTMLButtonElement>("button[data-destination]")) {
      if (button.dataset.destination === destination) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
  }

  private syncSessionVisibility(): void {
    if (!this.ctx) return;
    const ready = this.ctx.state.playerId !== null;
    byId("m-join").hidden = ready;
    byId("m-chrome").hidden = !ready;
    byId("m-tabs").hidden = !ready;
    if (!ready) {
      const reconnecting = this.ctx.state.link === "connecting" || this.ctx.state.link === "reconnecting";
      byId<HTMLButtonElement>("m-join-button").disabled = reconnecting && !!this.ctx.state.name;
    }
  }

  private renderStatus(force = false): void {
    const state = this.ctx?.state;
    if (!state) return;
    const reserved = reservedMarketCredits();
    const credits = state.wallet
      ? `${reserved > 0 ? "~" : ""}${Math.round(spendableMarketCredits()).toLocaleString()}`
      : "—";
    const link = state.link === "online"
      ? `● ${state.tick.toLocaleString()}`
      : state.link === "reconnecting"
        ? "reconnecting…"
        : state.link === "connecting"
          ? "connecting…"
          : "offline";
    const values = [credits, link, state.ghosts.length, state.name, state.playerId, state.simTime, state.corpsInView, state.wallet?.valuation];
    const signature = values.join("|");
    if (!force && signature === this.statusSignature) return;
    this.statusSignature = signature;

    byId("m-credits").textContent = credits;
    const linkEl = byId("m-link");
    linkEl.textContent = link;
    linkEl.className = state.link === "online" ? "is-online" : "is-offline";
    byId("m-contacts").textContent = state.link === "online" ? String(state.ghosts.length) : "—";
    byId("m-corp").textContent = state.name || "—";
    byId("m-id").textContent = state.playerId !== null ? formatId(state.playerId) : "—";
    byId("m-time").textContent = state.link === "online" ? `${state.simTime.toFixed(1)}s` : "—";
    byId("m-corps").textContent = state.link === "online" ? String(state.corpsInView) : "—";
    byId("m-equity").textContent = state.wallet ? Math.round(state.wallet.valuation).toLocaleString() : "—";
  }
}

export function createShell(): Shell {
  return new MobileShell();
}

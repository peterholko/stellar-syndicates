import "../../styles/mobile.css";

import { reservedMarketCredits, spendableMarketCredits } from "../../core/derive/market";
import { formatId } from "../../protocol";
import type { CoreEvent } from "../../core/events";
import type { CoreContext, Rect, Shell } from "../types";
import { MobileMapInteraction } from "./map";
import { mountMobileMarkup } from "./markup";
import { activateSheetStack, pushSheet, replaceSheet, SheetStack, type SheetEntry, type SheetView } from "./sheets";

const byId = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;
const escapeHtml = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);

class MobileShell implements Shell {
  private root: HTMLElement | null = null;
  private ctx: CoreContext | null = null;
  private abort: AbortController | null = null;
  private sheets: SheetStack | null = null;
  private map: MobileMapInteraction | null = null;
  private statusSignature = "";

  async mount(root: HTMLElement, ctx: CoreContext): Promise<void> {
    this.root = root;
    this.ctx = ctx;
    this.abort = new AbortController();
    mountMobileMarkup(root);

    const signal = this.abort.signal;
    this.sheets = new SheetStack(
      (entry) => this.renderSheet(entry),
      () => this.syncCameraRect(),
      (entry) => this.syncDestination(entry),
      signal,
    );
    activateSheetStack(this.sheets);
    this.map = new MobileMapInteraction(ctx, {
      openSheet: (entry) => this.openSheet(entry),
      onSemanticChange: (mode) => this.semanticChanged(mode),
    }, signal);
    byId("m-status-toggle").addEventListener("click", () => this.toggleStatus(), { signal });
    byId("m-armed-cancel").addEventListener("click", () => this.map?.cancelArmedMode(), { signal });
    byId<HTMLFormElement>("m-join-form").addEventListener("submit", (event) => {
      event.preventDefault();
      this.join();
    }, { signal });
    byId("m-tabs").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("button[data-destination]");
      if (!button) return;
      const destination = button.dataset.destination as SheetEntry["id"];
      if (this.sheets?.current) replaceSheet(destination);
      else pushSheet(destination);
    }, { signal });
    byId("m-sheet-body").addEventListener("click", (event) => {
      const action = (event.target as Element).closest<HTMLButtonElement>("button[data-mobile-act]")?.dataset.mobileAct;
      if (action === "confirm-intent") ctx.intent.confirmPendingIntent();
      else if (action === "cancel-intent") ctx.intent.clearPendingIntent();
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
    let refreshSheet = false;
    for (const event of events) {
      if (event.kind === "JoinRejected") {
        byId("m-join-error").textContent = event.message;
        byId<HTMLButtonElement>("m-join-button").disabled = false;
      } else if (event.kind === "TransportError" && this.ctx?.state.playerId === null) {
        byId("m-join-error").textContent = `Could not reach server at ${event.url}.`;
      } else if (event.kind === "ProtocolMismatch") {
        console.warn(`protocol mismatch: server v${event.server}, client expects v${event.client}`);
      } else if (event.kind === "IntentChanged") {
        if (event.readout) this.map?.showNotice(event.readout);
        this.syncIntentSheet();
      } else if (event.kind === "ServerError") {
        this.map?.showNotice(`<span class="warn">${escapeHtml(event.message)}</span>`);
      }
      if (event.kind === "ViewApplied" || event.kind === "TimelineApplied" || event.kind === "TradeSettled") {
        refreshSheet = true;
      }
    }
    this.syncSessionVisibility();
    this.renderStatus(true);
    if (refreshSheet) this.sheets?.refresh();
  }

  onViewTick(): void {
    this.renderStatus();
    this.map?.tick();
    this.map?.syncArmedChip();
  }

  cameraRect(): Rect {
    return this.sheets?.cameraRect() ?? { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }

  teardown(): void {
    this.abort?.abort();
    this.map?.teardown();
    this.map = null;
    activateSheetStack(null);
    this.sheets = null;
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
    this.sheets?.layout();
  }

  private syncDestination(entry: SheetEntry | null): void {
    const destination = entry?.id ?? "";
    for (const button of byId("m-tabs").querySelectorAll<HTMLButtonElement>("button[data-destination]")) {
      if (button.dataset.destination === destination) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
  }

  private renderSheet(entry: SheetEntry): SheetView {
    const titles: Record<SheetEntry["id"], [string, string]> = {
      market: ["Market Hub", "Exchange · warehouse"],
      fleets: ["Fleets", "Corporate roster"],
      research: ["Research", "Programme boards"],
      officers: ["Officers", "Captain roster"],
      operations: ["Operations", "Contracts · objectives"],
      syndicate: ["Syndicate", "Alliance network"],
      faction: ["Faction", "Authority charter"],
      log: ["Check-in", "Decision inbox"],
      system: ["System", "System management"],
      ship: ["Fleet", "Fleet command"],
      battle: ["Battle", "Observed theater"],
      intent: ["Confirm order", "Command preview"],
    };
    const [title, eyebrow] = titles[entry.id];
    if (entry.id === "intent") return this.renderIntentSheet(title, eyebrow);
    return {
      title,
      eyebrow,
      html: `<div class="m-sheet-placeholder"><b>${title}</b><span>This mobile workspace is connected to the shared navigation stack. Its live controls land in the core-loop pass.</span></div>`,
    };
  }

  private renderIntentSheet(title: string, eyebrow: string): SheetView {
    const intent = this.ctx?.state.pendingIntent;
    if (!intent) {
      return {
        title,
        eyebrow,
        html: `<div class="m-sheet-placeholder"><span>No order is awaiting confirmation.</span></div>`,
      };
    }
    const destination = intent.dest
      ? `${Math.round(intent.dest.x).toLocaleString()}, ${Math.round(intent.dest.y).toLocaleString()} su`
      : intent.targetId
        ? `Target ${escapeHtml(intent.targetId)}`
        : "Selected target";
    return {
      title,
      eyebrow,
      html: `<div class="m-intent-card">` +
        `<div class="m-intent-card__verb">${escapeHtml(intent.verb)} order</div>` +
        `<div class="m-intent-card__route"><small>Fleet</small><b>${escapeHtml(intent.shipId)}</b>` +
        `<small>Destination</small><b>${destination}</b></div>` +
        `<div class="m-sheet-actions"><button type="button" data-mobile-act="cancel-intent">Cancel</button>` +
        `<button type="button" class="is-primary" data-mobile-act="confirm-intent">Send order</button></div></div>`,
    };
  }

  private syncIntentSheet(): void {
    if (!this.sheets) return;
    if (this.ctx?.state.pendingIntent) {
      if (this.sheets.current?.id === "intent") this.sheets.refresh();
      else this.sheets.push("intent");
    } else if (this.sheets.current?.id === "intent") {
      this.sheets.pop();
    }
    this.map?.syncArmedChip();
  }

  private openSheet(entry: SheetEntry): void {
    if (!this.sheets) return;
    if (this.sheets.current?.id === entry.id) this.sheets.replace(entry.id, entry.props);
    else this.sheets.push(entry.id, entry.props);
  }

  private semanticChanged(mode: "galaxy" | "system" | "battle"): void {
    const current = this.sheets?.current?.id;
    if (mode === "galaxy" && (current === "system" || current === "battle")) this.sheets?.pop();
    else this.sheets?.refresh();
  }

  private syncCameraRect(): void {
    if (!this.ctx) return;
    this.ctx.renderer.setCameraRect(this.cameraRect());
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
    this.sheets?.layout();
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

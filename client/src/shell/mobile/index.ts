import "../../styles/mobile.css";
import { bindAccountForm } from "../account";

import { shipKindLabel } from "../../core/derive/fleet";
import { intentReadinessWarnings } from "../../core/derive/readiness";
import { intentSummary } from "../../core/derive/orders";
import { reservedMarketCredits, spendableMarketCredits } from "../../core/derive/market";
import { label } from "../../icons";
import { formatId, type BattleRecordView, type RaidOutcome, type RaidReport } from "../../protocol";
import type { CoreEvent } from "../../core/events";
import { installPressGuard } from "../dom";
import type { CoreContext, Rect, Shell } from "../types";
import { MobileBattleTheater } from "./battle";
import { MobileGroundTheater } from "./ground";
import { MobileMapInteraction } from "./map";
import { mountMobileMarkup } from "./markup";
import { MobileNoticeStack, type MobileNoticeTone } from "./notices";
import { MobileParitySurfaces } from "./parity";
import { activateSheetStack, pushSheet, replaceSheet, SheetStack, type SheetEntry, type SheetView } from "./sheets";
import { MobileSurfaces } from "./surfaces";

const byId = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;
const escapeHtml = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);

function reportNotice(report: RaidReport, pirateId?: string): { text: string; tone: MobileNoticeTone } {
  const mine = report.you === "attacker" ? report.attacker_kind : report.target_kind;
  const theirs = report.you === "attacker" ? report.target_kind : report.attacker_kind;
  const yourShipDied = report.outcome === "both_destroyed"
    || (report.you === "attacker" && report.outcome === "attacker_destroyed")
    || (report.you === "defender" && report.outcome === "target_destroyed");
  let text: string;
  let tone: MobileNoticeTone = "good";
  switch (report.outcome) {
    case "both_destroyed":
      text = `Your ${shipKindLabel(mine)} and a rival ${shipKindLabel(theirs)} destroyed each other.`;
      tone = "bad";
      break;
    case "both_survive":
      text = report.you === "attacker"
        ? `Your raid on a rival ${shipKindLabel(theirs)} was driven off — both survived.`
        : `A raider attacked your ${shipKindLabel(mine)} but was driven off.`;
      break;
    case "escaped":
      text = report.you === "attacker"
        ? `Your target ${shipKindLabel(theirs)} reached the hub — raid failed.`
        : `Your ${shipKindLabel(mine)} reached the hub safely.`;
      break;
    default:
      if (yourShipDied) {
        text = `Your ${shipKindLabel(mine)} was destroyed by a rival ${shipKindLabel(theirs)}.`;
        tone = "bad";
      } else {
        text = `Your ${shipKindLabel(mine)} destroyed a rival ${shipKindLabel(theirs)}.`;
      }
  }
  if (pirateId && (report.attacker === pirateId || report.defender === pirateId)) text = text.replaceAll("rival", "pirate");
  return { text, tone };
}

function battleNotice(outcome: RaidOutcome, record?: BattleRecordView): { text: string; tone: MobileNoticeTone } {
  const ownSide = record?.own_side;
  switch (outcome) {
    case "both_destroyed": return { text: "Both fleets destroyed each other.", tone: "bad" };
    case "both_survive": return { text: "Both fleets survived.", tone: "quiet" };
    case "escaped": return { text: "The target escaped.", tone: ownSide === 1 ? "good" : "quiet" };
    case "attacker_destroyed":
      return ownSide === 0
        ? { text: "Your attacking fleet was destroyed.", tone: "bad" }
        : ownSide === 1
          ? { text: "Your fleet destroyed the attacker.", tone: "good" }
          : { text: "The attacking fleet was destroyed.", tone: "quiet" };
    case "target_destroyed":
      return ownSide === 1
        ? { text: "Your defending fleet was destroyed.", tone: "bad" }
        : ownSide === 0
          ? { text: "Your fleet destroyed the defender.", tone: "good" }
          : { text: "The defending fleet was destroyed.", tone: "quiet" };
  }
}

class MobileShell implements Shell {
  private root: HTMLElement | null = null;
  private ctx: CoreContext | null = null;
  private abort: AbortController | null = null;
  private sheets: SheetStack | null = null;
  private map: MobileMapInteraction | null = null;
  private notices: MobileNoticeStack | null = null;
  private battle: MobileBattleTheater | null = null;
  private ground: MobileGroundTheater | null = null;
  private surfaces: MobileSurfaces | null = null;
  private parity: MobileParitySurfaces | null = null;
  private statusSignature = "";
  private orientationMedia: MediaQueryList | null = null;
  private foundingResizeObserver: ResizeObserver | null = null;
  private viewportFrame = 0;
  private viewportSettleTimer: number | null = null;
  private cameraRectSynced = false;
  private unreadReports = 0;
  private readonly readDecisionKeys = new Set<string>();

  async mount(root: HTMLElement, ctx: CoreContext): Promise<void> {
    this.root = root;
    this.ctx = ctx;
    this.abort = new AbortController();
    mountMobileMarkup(root);
    installPressGuard();

    const signal = this.abort.signal;
    this.orientationMedia = matchMedia("(orientation: portrait)");
    const viewportChanged = () => {
      this.syncRotateGate();
      this.scheduleViewportLayout();
    };
    this.orientationMedia.addEventListener("change", viewportChanged, { signal });
    window.addEventListener("orientationchange", viewportChanged, { signal });
    window.visualViewport?.addEventListener("resize", viewportChanged, { signal });
    this.sheets = new SheetStack(
      (entry) => this.renderSheet(entry),
      () => this.syncCameraRect(),
      (entry) => this.syncDestination(entry),
      signal,
      (entry) => this.sheetRefreshNeeded(entry),
    );
    activateSheetStack(this.sheets);
    this.notices = new MobileNoticeStack(byId("m-map-notice"), (entry) => this.openSheet(entry), signal);
    this.battle = new MobileBattleTheater(ctx, this.sheets);
    this.ground = new MobileGroundTheater(ctx, this.sheets);
    this.map = new MobileMapInteraction(ctx, {
      openSheet: (entry) => this.openSheet(entry),
      onSemanticChange: (mode) => this.semanticChanged(mode),
      notice: (html) => this.notices?.push(html),
    }, signal);
    this.surfaces = new MobileSurfaces(ctx, this.sheets, {
      openSheet: (entry) => this.openSheet(entry),
      focusFleet: (id) => this.map?.focusFleet(id),
      focusSystem: (id) => this.map?.focusSystem(id),
      armMove: (id) => this.map?.armMove(id),
      enterSystem: (id) => this.map?.enterSystem(id),
      exitSemantic: () => this.map?.exitSemanticView(),
      notice: (html) => this.notices?.push(html),
    });
    this.parity = new MobileParitySurfaces(ctx, this.sheets, {
      openSheet: (entry) => this.openSheet(entry),
      focusFleet: (id) => this.map?.focusFleet(id),
      focusSystem: (id) => this.map?.focusSystem(id),
      enterSystem: (id) => this.map?.enterSystem(id),
      exitSemantic: () => this.map?.exitSemanticView(),
      notice: (html) => this.notices?.push(html),
    });
    this.foundingResizeObserver = new ResizeObserver(() => this.syncFoundingNoticeOffset());
    this.foundingResizeObserver.observe(byId("m-founding"));
    byId("m-status-toggle").addEventListener("click", () => this.toggleStatus(), { signal });
    byId("m-armed-cancel").addEventListener("click", () => this.map?.cancelArmedMode(), { signal });
    bindAccountForm("m", ctx, signal);
    byId("m-tabs").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("button[data-destination]");
      if (!button) return;
      const destination = button.dataset.destination as SheetEntry["id"];
      if (destination === "log") this.markLogRead();
      if (this.sheets?.current?.id === destination) this.sheets.closeAll();
      else if (this.sheets?.current) replaceSheet(destination);
      else pushSheet(destination);
    }, { signal });
    byId("m-sheet-body").addEventListener("click", (event) => {
      if (this.battle?.handleClick(event)) return;
      if (this.ground?.handleClick(event)) return;
      if (this.parity?.handleClick(event)) return;
      if (this.surfaces?.handleClick(event)) return;
      const action = (event.target as Element).closest<HTMLButtonElement>("button[data-mobile-act]")?.dataset.mobileAct;
      if (action === "confirm-intent") ctx.intent.confirmPendingIntent();
      else if (action === "cancel-intent") ctx.intent.clearPendingIntent();
    }, { signal });
    byId("m-founding").addEventListener("click", (event) => { this.surfaces?.handleClick(event); }, { signal });

    this.syncSessionVisibility();
    this.syncRotateGate();
    this.scheduleViewportLayout();
    this.renderStatus(true);
    this.surfaces.refreshFounding();
    this.syncFoundingNoticeOffset();

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
      } else if (event.kind === "SessionReplaced") {
        byId("m-join-error").textContent = "Session ended. Please sign in again.";
        byId<HTMLButtonElement>("m-join-button").disabled = false;
      } else if (event.kind === "IntentChanged") {
        if (event.readout) this.map?.showNotice(event.readout);
        this.syncIntentSheet();
      } else if (event.kind === "ServerError") {
        this.map?.showNotice(`<span class="warn">${escapeHtml(event.message)}</span>`);
      } else if (event.kind === "TradeSettled") {
        this.surfaces?.onTrade(event.trade);
      } else if (event.kind === "ReportArrived") {
        const report = reportNotice(event.report, this.ctx?.state.galaxy?.pirate_id);
        this.unreadReports++;
        this.surfaces?.recordReport(report.text, report.tone === "bad" ? "bad" : "good");
        if (this.sheets?.current?.id === "log") refreshSheet = true;
        this.notices?.push({
          html: `<b>${escapeHtml(report.text)}</b> <span class="m-muted">· delayed ${Math.round(event.report.age)}s</span>`,
          tone: report.tone,
          destination: { id: "log" },
        });
      } else if (event.kind === "BattleConcluded") {
        const record = this.ctx?.state.battleRecords.find((candidate) => candidate.id === event.recordId);
        const outcome = battleNotice(event.outcome, record);
        this.unreadReports++;
        this.surfaces?.recordReport(`Battle concluded — ${outcome.text}`, outcome.tone === "bad" ? "bad" : outcome.tone === "good" ? "good" : "warn");
        if (this.sheets?.current?.id === "log") refreshSheet = true;
        this.notices?.push({
          html: `<b>Battle concluded</b> — ${escapeHtml(outcome.text)}`,
          tone: outcome.tone,
          destination: { id: "battle", props: { id: event.recordId } },
        });
      } else if (event.kind === "OrderConfirmed") {
        this.notices?.push({
          html: `✓ ${escapeHtml(label(event.orderKind))} order confirmed`,
          tone: "quiet",
          durationMs: 2000,
          destination: { id: "ship", props: { kind: "fleet", id: event.shipId } },
        });
      } else if (event.kind === "EstimateReady") {
        this.surfaces?.onEstimate(event.estimate);
        const current = this.sheets?.current;
        const shownId = current?.id === "ship" && current.props && typeof current.props === "object"
          ? (current.props as { id?: string }).id
          : undefined;
        if (shownId === event.estimate.target) this.sheets?.refresh();
        else {
          const pct = event.estimate.win_pct == null
            ? "Projection ready"
            : `${Math.round(event.estimate.win_pct)}% ${event.estimate.win_pct >= 55 ? "favorable" : event.estimate.win_pct >= 45 ? "even" : "unfavorable"}`;
          this.notices?.push({
            html: `<b>${escapeHtml(pct)}</b> · tap for engagement details`,
            destination: { id: "ship", props: { kind: "fleet", id: event.estimate.target } },
          });
        }
      } else if (event.kind === "CommandSignal" || event.kind === "CommandChevron") {
        // The shared session already appended these to renderer-owned state;
        // mobile's ordinary render tick draws the same comet/chevron as desktop.
      }
      if (event.kind === "ViewApplied" || event.kind === "TimelineApplied" || event.kind === "TradeSettled") {
        refreshSheet = true;
      }
    }
    this.syncSessionVisibility();
    this.renderStatus(true);
    if (refreshSheet) this.surfaces?.refreshFounding();
    if (refreshSheet) this.sheets?.refresh();
    this.syncLogBadge();
  }

  onViewTick(): void {
    this.renderStatus();
    this.map?.tick();
    this.map?.syncArmedChip();
    this.battle?.tick();
    this.ground?.tick();
  }

  framePolicy() {
    return { maxFps: 30, renderGalaxy: !this.sheets?.coversMap() };
  }

  cameraRect(): Rect {
    return this.sheets?.cameraRect() ?? { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }

  teardown(): void {
    this.abort?.abort();
    this.map?.teardown();
    this.map = null;
    this.notices?.teardown();
    this.notices = null;
    this.battle?.close();
    this.battle = null;
    this.ground?.close();
    this.ground = null;
    this.parity = null;
    this.surfaces = null;
    activateSheetStack(null);
    this.sheets = null;
    document.documentElement.style.removeProperty("--mobile-sheet-height");
    document.documentElement.style.removeProperty("--mobile-chrome-bottom");
    if (this.viewportFrame) cancelAnimationFrame(this.viewportFrame);
    if (this.viewportSettleTimer !== null) window.clearTimeout(this.viewportSettleTimer);
    this.viewportFrame = 0;
    this.viewportSettleTimer = null;
    this.cameraRectSynced = false;
    this.orientationMedia = null;
    this.foundingResizeObserver?.disconnect();
    this.foundingResizeObserver = null;
    document.documentElement.style.removeProperty("--mobile-founding-stack-height");
    this.abort = null;
    this.root?.replaceChildren();
    this.root = null;
    this.ctx = null;
  }

  private toggleStatus(): void {
    const button = byId<HTMLButtonElement>("m-status-toggle");
    const expanded = button.getAttribute("aria-expanded") !== "true";
    button.setAttribute("aria-expanded", String(expanded));
    byId("m-status-more").hidden = !expanded;
    this.sheets?.layout();
  }

  private syncDestination(entry: SheetEntry | null): void {
    this.battle?.sync(entry);
    this.ground?.sync(entry);
    const destination = entry?.id ?? "";
    if (destination === "log") this.markLogRead();
    for (const button of byId("m-tabs").querySelectorAll<HTMLButtonElement>("button[data-destination]")) {
      if (button.dataset.destination === destination) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
  }

  private markLogRead(): void {
    this.unreadReports = 0;
    for (const key of this.surfaces?.decisionKeys() ?? []) this.readDecisionKeys.add(key);
    this.syncLogBadge();
  }

  private syncLogBadge(): void {
    const badge = document.getElementById("m-log-badge");
    if (!badge) return;
    if (this.sheets?.current?.id === "log") {
      this.unreadReports = 0;
      for (const key of this.surfaces?.decisionKeys() ?? []) this.readDecisionKeys.add(key);
    }
    const unreadDecisions = (this.surfaces?.decisionKeys() ?? []).filter((key) => !this.readDecisionKeys.has(key)).length;
    const count = this.unreadReports + unreadDecisions;
    badge.textContent = count > 99 ? "99+" : String(count);
    badge.hidden = count === 0;
  }

  private syncFoundingNoticeOffset(): void {
    const founding = document.getElementById("m-founding");
    const stackHeight = !founding || founding.hidden ? 0 : Math.ceil(founding.getBoundingClientRect().height) + 6;
    document.documentElement.style.setProperty("--mobile-founding-stack-height", `${stackHeight}px`);
  }

  /** Landscape is a temporary cover over the still-running mobile session.
   * The media query is the law; neither the shell nor either Pixi theater is
   * recreated while this overlay is present. */
  private syncRotateGate(): void {
    const gate = document.getElementById("m-rotate-gate");
    if (!gate || !this.orientationMedia) return;
    gate.hidden = this.orientationMedia.matches;
  }

  /** iOS reports rotation before visualViewport settles. Run once on the next
   * paint and once after the resize burst, preserving the same sheet stack and
   * asking mounted theaters to resize in place. */
  private scheduleViewportLayout(): void {
    if (this.viewportFrame) cancelAnimationFrame(this.viewportFrame);
    this.viewportFrame = requestAnimationFrame(() => {
      this.viewportFrame = 0;
      this.applyViewportLayout();
    });
    if (this.viewportSettleTimer !== null) window.clearTimeout(this.viewportSettleTimer);
    this.viewportSettleTimer = window.setTimeout(() => {
      this.viewportSettleTimer = null;
      this.applyViewportLayout();
    }, 180);
  }

  private applyViewportLayout(): void {
    // SheetStack.layout owns the one camera synchronization for this pass.
    this.sheets?.layout();
    const entry = this.sheets?.current ?? null;
    this.battle?.sync(entry);
    this.ground?.sync(entry);
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
      rankings: ["Rankings", "Published campaign ledger"],
      logistics: ["Auto-supply", "Standing logistics orders"],
      doctrine: ["Fleet Doctrine", "Corporate fleet policy"],
      log: ["Check-in", "Decision inbox"],
      system: ["System", "System management"],
      planet: ["World", "Planet management"],
      build: ["Build", "Structure construction"],
      shipyard: ["Shipyard", "Hull construction"],
      hub: ["Wormhole Hub", "Market infrastructure"],
      ship: ["Fleet", "Fleet command"],
      battle: ["Battle", "Observed theater"],
      ground: ["Ground action", "Observed landing"],
      intent: ["Confirm order", "Command preview"],
    };
    const [title, eyebrow] = titles[entry.id];
    if (entry.id === "intent") return this.renderIntentSheet(title, eyebrow);
    const surface = this.battle?.render(entry) ?? this.ground?.render(entry) ?? this.parity?.render(entry) ?? this.surfaces?.render(entry);
    if (surface) return surface;
    return {
      title,
      eyebrow,
      html: `<div class="m-sheet-placeholder"><b>${title}</b><span>This workspace arrives in the Phase 5 mobile-parity pass.</span></div>`,
    };
  }

  private sheetRefreshNeeded(entry: SheetEntry): boolean {
    for (const renderer of [this.battle, this.ground, this.parity, this.surfaces]) {
      const needed = renderer?.refreshNeeded(entry);
      if (needed !== null && needed !== undefined) return needed;
    }
    return true;
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
        `<div class="m-intent-card__verb">Order preview</div>` +
        (intent.verb === "command" ? `<p>${escapeHtml(intentSummary(intent))}</p>` :
          `<div class="m-intent-card__route"><small>Fleet</small><b>${escapeHtml(intent.shipId)}</b>` +
          `<small>Destination</small><b>${destination}</b></div>`) +
        intentReadinessWarnings(this.ctx!.state, intent).map(w => `<div class="m-warning">${escapeHtml(w)}</div>`).join("") +
        `<div class="m-sheet-actions"><button type="button" data-mobile-act="cancel-intent">Cancel</button>` +
        `<button type="button" class="is-primary" data-mobile-act="confirm-intent">Confirm order</button></div></div>`,
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
    const next = this.cameraRect();
    const current = this.ctx.renderer.cameraRect;
    if (!this.cameraRectSynced) {
      this.ctx.renderer.setCameraRect(next);
      this.cameraRectSynced = true;
      return;
    }
    const same = Math.abs(next.x - current.x) < .5
      && Math.abs(next.y - current.y) < .5
      && Math.abs(next.w - current.w) < .5
      && Math.abs(next.h - current.h) < .5;
    if (same) return;

    const viewportH = window.visualViewport?.height ?? window.innerHeight;
    const verticalChange = Math.max(
      Math.abs(next.y - current.y),
      Math.abs(next.y + next.h - current.y - current.h),
    );
    const horizontalChange = Math.max(
      Math.abs(next.x - current.x),
      Math.abs(next.x + next.w - current.x - current.w),
    );
    // Coalesce small inset changes instead of asking setCameraRect to preserve
    // focus around a slightly different center. The covered strip is not an
    // interactive map surface, and a later material detent change catches up.
    if (verticalChange < viewportH * .15 && horizontalChange < .5) return;
    this.ctx.renderer.setCameraRect(next);
  }

  private syncSessionVisibility(): void {
    if (!this.ctx) return;
    const ready = this.ctx.state.playerId !== null;
    byId("m-join").hidden = ready;
    byId("m-chrome").hidden = !ready;
    byId("m-tabs").hidden = !ready;
    if (!ready) byId("m-founding").hidden = true;
    if (!ready) {
      const reconnecting = this.ctx.state.link === "connecting" || this.ctx.state.link === "reconnecting";
      byId<HTMLButtonElement>("m-join-button").disabled = byId("m-join-form").dataset.busy === "true" || (reconnecting && !!this.ctx.state.name);
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
    const values = [credits, reserved, link, state.ghosts.length, state.name, state.playerId, state.simTime, state.corpsInView, state.wallet?.credits, state.wallet?.valuation];
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
    byId("m-account-credits").textContent = state.wallet ? Math.round(state.wallet.credits).toLocaleString() : "—";
    byId("m-reserved-credits").textContent = state.wallet ? Math.round(reserved).toLocaleString() : "—";
  }
}

export function createShell(): Shell {
  return new MobileShell();
}

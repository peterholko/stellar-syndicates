import { guardCapable, jumpCapable, shipKindLabel } from "../../core/derive/fleet";
import { gravityWellAt, nearestKnownDock } from "../../core/derive/geo";
import { intentSummary } from "../../core/derive/orders";
import { intentReadinessWarnings } from "../../core/derive/readiness";
import { informationDelay } from "../../core/derive/format";
import { jumpRangeAt } from "../../core/derive/nebula";
import type { GhostView, TransitMode } from "../../protocol";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";

interface DeckCommandHooks {
  notice(html: string): void;
}

const esc = (value: string): string => value.replace(
  /[&<>"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!,
);

/** The one surface for fleet selection, armed targeting, and pending intent.
 * Policy controls and map verbs both become PendingIntent previews. The wire
 * messages are unchanged, but no fleet order transmits before confirmation. */
export class DeckCommandStrip {
  private signature = "";
  private statusHtml = "";
  private statusVersion = 0;
  private statusTimer: number | null = null;

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: DeckCommandHooks,
    signal: AbortSignal,
  ) {
    root.addEventListener("click", (event) => this.activate(event), { signal });
  }

  render(force = false): void {
    this.reconcileSelection();
    const state = this.ctx.state;
    const selected = this.selectedFleets();
    const signature = sheetFingerprint([
      state.pendingIntent,
      this.ctx.intent.intentAiming.jump,
      this.ctx.intent.intentAiming.guard,
      state.selectedShipId,
      [...state.selectedShipIds],
      selected.map((fleet) => [fleet.id, fleet.age, fleet.docked, fleet.pos, fleet.vel, fleet.path, fleet.composition, fleet.damage, fleet.fuel, fleet.supplied, fleet.captain]),
      state.orders,
      state.raids,
      selected.map((fleet) => (state.pendingOrders.get(fleet.id) ?? []).map((order) => [order.id, order.kind])),
      this.statusVersion,
    ]);
    if (!force && signature === this.signature) return;
    const intent = state.pendingIntent;
    const armed = this.ctx.intent.intentAiming.jump
      ? { mode: "jump" as const, shipId: this.ctx.intent.intentAiming.jump }
      : this.ctx.intent.intentAiming.guard
        ? { mode: "guard" as const, shipId: this.ctx.intent.intentAiming.guard }
        : null;
    let modeHtml = "";
    if (intent) modeHtml = this.intentHtml(intentSummary(intent));
    else if (armed) modeHtml = this.armedHtml(armed.mode, armed.shipId);
    else if (selected.length) modeHtml = this.selectionHtml(selected);
    if (renderDeferred(this.root.id, () => this.render(true))) return;
    this.signature = signature;
    // Receipts and refusals remain visible even with no fleet selected. A
    // status message is feedback from command, not decoration owned by the
    // selection strip.
    setHtml(this.root, modeHtml + this.statusLineHtml());
    this.root.hidden = !this.root.childElementCount;
  }

  /** Internal command copy is already presentation-safe HTML from core/intent.
   * One generation owns the line, so an older expiry can never erase a newer
   * outcome. CSS supplies the short tail fade before the 15-second removal. */
  setStatus(html: string): void {
    if (!html) return;
    this.statusHtml = html;
    this.statusVersion++;
    if (this.statusTimer !== null) window.clearTimeout(this.statusTimer);
    const generation = this.statusVersion;
    this.statusTimer = window.setTimeout(() => {
      if (generation !== this.statusVersion) return;
      this.statusHtml = "";
      this.statusTimer = null;
      this.statusVersion++;
      this.render(true);
    }, 15_000);
    this.render(true);
  }

  clearStatus(): void {
    if (this.statusTimer !== null) window.clearTimeout(this.statusTimer);
    this.statusTimer = null;
    this.statusHtml = "";
    this.statusVersion++;
    this.render(true);
  }

  clear(): void {
    if (this.statusTimer !== null) window.clearTimeout(this.statusTimer);
    this.statusTimer = null;
    this.statusHtml = "";
    this.statusVersion++;
    this.signature = "";
    this.root.replaceChildren();
    this.root.hidden = true;
  }

  private activate(event: Event): void {
    const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-command]");
    if (!button || button.disabled) return;
    const action = button.dataset.deckCommand;
    const fleetId = button.dataset.fleetId ?? this.ctx.state.selectedShipId ?? undefined;
    const fleet = fleetId ? this.ctx.state.ghosts.find((entry) => entry.id === fleetId && entry.own) : undefined;
    if (action === "confirm") {
      this.ctx.intent.confirmPendingIntent();
    } else if (action === "cancel-intent") {
      this.ctx.intent.clearPendingIntent();
    } else if (action === "cancel-armed") {
      if (this.ctx.intent.intentAiming.jump) this.ctx.intent.clearJumpAiming();
      if (this.ctx.intent.intentAiming.guard) this.ctx.intent.clearGuardAiming();
    } else if (action === "center" && fleetId) {
      const target = this.ctx.state.ghosts.find((entry) => entry.id === fleetId);
      if (target) this.ctx.renderer.centerOnWorld(target.pos);
    } else if (action === "remove" && fleetId) {
      this.removeFleet(fleetId);
    } else if (action === "move" && fleet) {
      this.hooks.notice(`<b>Move ${esc(shipKindLabel(fleet.kind))}</b> · click a star, the Market Hub, or empty map space.`);
    } else if (action === "dock" && fleet) {
      const dock = nearestKnownDock(fleet);
      if (!dock || fleet.docked) return;
      this.ctx.intent.clearPendingIntent(true);
      this.ctx.intent.beginPendingIntent({ verb: "move", shipId: fleet.id, dest: dock.pos });
    } else if (action === "jump" && fleet) {
      this.ctx.intent.armJumpAiming(fleet);
    } else if (action === "guard" && fleet) {
      this.ctx.intent.armGuardAiming(fleet);
    } else if (action === "hold" && fleet) {
      this.ctx.intent.beginFleetCommand({ type: "HoldFleet", ship_id: fleet.id });
    } else if (action === "transit" && fleet) {
      const mode = button.dataset.mode as TransitMode | undefined;
      if (mode !== "full" && mode !== "stealth") return;
      this.ctx.intent.beginFleetCommand({ type: "SetFleetTransit", fleet_id: fleet.id, mode });
    } else if (action === "recall" && fleet) {
      this.ctx.intent.beginFleetCommand({ type: "RecallRaid", raider_id: fleet.id });
    }
    this.ctx.renderer.stateVersion++;
    this.render(true);
  }

  private intentHtml(summary: string): string {
    const warnings = this.ctx.state.pendingIntent ? intentReadinessWarnings(this.ctx.state, this.ctx.state.pendingIntent) : [];
    return `<div class="deck-command-strip__frame deck-command-strip__frame--intent">` +
      `<div class="deck-command-strip__mode">ORDER PREVIEW</div>` +
      `<div class="deck-command-strip__summary">${esc(summary)}</div>` +
      (warnings.length ? `<div class="deck-dispatch-warning" role="status">${warnings.map(esc).join("<br>")}</div>` : "") +
      `<div class="deck-command-strip__actions">` +
        `<button type="button" class="is-primary" data-deck-command="confirm">Confirm <kbd>Enter</kbd></button>` +
        `<button type="button" data-deck-command="cancel-intent">Cancel <kbd>Esc</kbd></button>` +
      `</div></div>`;
  }

  private statusLineHtml(): string {
    return this.statusHtml ? `<div class="deck-command-status">${this.statusHtml}</div>` : "";
  }

  private armedHtml(mode: "jump" | "guard", shipId: string): string {
    const fleet = this.ctx.state.ghosts.find((entry) => entry.id === shipId);
    const detail = mode === "jump"
      ? `click a destination within ${Math.round(fleet ? jumpRangeAt(this.ctx.state.galaxy, fleet.pos) : 0).toLocaleString()} su`
      : "click another one of your fleet markers";
    return `<div class="deck-command-strip__frame deck-command-strip__frame--armed">` +
      `<div class="deck-command-strip__mode">${mode.toUpperCase()} AIMING</div>` +
      `<div class="deck-command-strip__summary">${fleet ? esc(shipKindLabel(fleet.kind)) : "Fleet"} · ${esc(detail)} · Esc cancels</div>` +
      `<div class="deck-command-strip__actions"><button type="button" data-deck-command="cancel-armed">Cancel <kbd>Esc</kbd></button></div>` +
      `</div>`;
  }

  private selectionHtml(fleets: GhostView[]): string {
    const primary = this.ctx.state.selectedShipId
      ? this.ctx.state.ghosts.find((fleet) => fleet.id === this.ctx.state.selectedShipId)
      : fleets[0];
    const chips = fleets.map((fleet) => this.fleetChip(fleet)).join("");
    const verbs = primary?.own ? this.verbHtml(primary) : `<div class="deck-command-strip__foreign">Rival contact · inspection only</div>`;
    const transitPending = primary && (this.ctx.state.pendingOrders.get(primary.id) ?? []).some((order) => !order.lost && order.kind === "configure");
    return `<div class="deck-command-strip__frame deck-command-strip__frame--selection">` +
      `<div class="deck-command-strip__selection"><div class="deck-command-strip__chips">${chips}</div>` +
        (primary?.own ? this.transitVerb(primary, !!transitPending) : "") + `</div>` +
      (fleets.length > 1
        ? `<div class="deck-command-strip__batch"><b>${fleets.length}-fleet command group</b> · map moves apply to every fleet; other controls apply to the primary chip.</div>`
        : "") +
      `<div class="deck-command-strip__verbs">${verbs}</div>` +
      `</div>`;
  }

  private fleetChip(fleet: GhostView): string {
    const composition = fleet.composition?.length
      ? fleet.composition.map((stack) => `${stack.count}× ${shipKindLabel(stack.kind)}`).join(" · ")
      : fleet.own ? "formation report pending" : "composition unresolved";
    return `<div class="deck-fleet-chip${fleet.id === this.ctx.state.selectedShipId ? " is-primary" : ""}">` +
      `<span><b>${esc(shipKindLabel(fleet.kind))}</b><small>${esc(composition)}</small></span>` +
      `<span class="deck-fleet-chip__delay">${esc(informationDelay(fleet.age))}</span>` +
      `<button type="button" data-deck-command="center" data-fleet-id="${esc(fleet.id)}" aria-label="Center fleet" title="Center fleet">⌾</button>` +
      `<button type="button" data-deck-command="remove" data-fleet-id="${esc(fleet.id)}" aria-label="Deselect fleet" title="Deselect fleet">✕</button>` +
      `</div>`;
  }

  private verbHtml(fleet: GhostView): string {
    const dock = nearestKnownDock(fleet);
    const hasCourse = this.hasCourse(fleet);
    const queue = this.ctx.state.pendingOrders.get(fleet.id) ?? [];
    const pending = (kind: (typeof queue)[number]["kind"]) => queue.some((order) => !order.lost && order.kind === kind);
    const jumpReason = !jumpCapable(fleet)
      ? "All hulls must carry compatible jump drives."
      : fleet.docked
        ? "Undock before spooling."
        : gravityWellAt(fleet.pos, this.ctx.state)
          ? "Served sighting is inside a gravity well."
          : pending("jump") ? "Jump order already in flight." : "";
    const guardReason = !guardCapable(fleet)
      ? "Requires an Interceptor fleet."
      : !this.ctx.state.ghosts.some((candidate) => candidate.own && candidate.id !== fleet.id && !candidate.docked)
        ? "No undocked friendly fleet to guard."
        : pending("guard") ? "Guard order already in flight." : "";
    const dockReason = fleet.docked ? "Already docked." : pending("move") ? "Move or docking order already in flight." : dock ? "" : "No known friendly berth.";
    const holdReason = fleet.docked ? "Docked fleets are already stationary." : pending("hold") ? "Hold order already in flight." : hasCourse ? "" : "No active course to cancel.";
    const recallReason = pending("recall") ? "Recall already in flight." : this.ctx.state.raids[fleet.id] ? "" : "Fleet has no active raid/intercept.";
    return [
      this.verb("move", "Move", "Click map", pending("move") ? "Move order already in flight." : ""),
      this.verb("dock", "Dock", dock ? dock.name : "Nearest berth", dockReason),
      this.verb("jump", "Jump", this.ctx.state.galaxy ? `${Math.round(jumpRangeAt(this.ctx.state.galaxy, fleet.pos)).toLocaleString()} su` : "Range unavailable", jumpReason),
      this.verb("guard", "Guard", "Choose fleet", guardReason),
      this.verb("hold", "Hold", "Cancel course", holdReason),
      this.verb("recall", "Recall", "Break off", recallReason),
    ].join("");
  }

  private verb(action: string, label: string, hint: string, reason = "", extra = ""): string {
    const detail = reason || hint;
    // Help must not change a command's dimensions as availability changes.
    // The wrapper retains the hover target for genuinely disabled buttons.
    return `<span class="deck-command-verb${reason ? " is-disabled" : ""}" title="${esc(detail)}">` +
      `<button type="button" data-deck-command="${action}"${extra}${reason ? " disabled" : ""} title="${esc(detail)}" aria-label="${esc(label)}. ${esc(detail)}">${esc(label)}</button></span>`;
  }

  private transitVerb(fleet: GhostView, pending: boolean): string {
    const requested = [...(this.ctx.state.pendingOrders.get(fleet.id) ?? [])]
      .reverse()
      .map((order) => order.configuration)
      .find((configuration) => configuration?.kind === "transit");
    const current = requested?.kind === "transit" ? requested.mode : fleet.transit ?? "full";
    const hint = pending ? "Configuration signal in flight." : "Choose the fleet's standing transit mode.";
    return `<span class="deck-command-transit${pending ? " is-pending" : ""}" role="group" aria-label="Transit mode" title="${esc(hint)}">` +
      `<span>Transit${pending ? " · sent" : ""}</span>` +
      `<button type="button" data-deck-command="transit" data-mode="full" aria-pressed="${current === "full"}" ${pending ? "disabled" : ""} aria-label="Set full-speed transit" title="${pending ? esc(hint) : "Travel at full speed."}">Full</button>` +
      `<button type="button" data-deck-command="transit" data-mode="stealth" aria-pressed="${current === "stealth"}" ${pending ? "disabled" : ""} aria-label="Set stealth transit" title="${pending ? esc(hint) : "Trade speed for a smaller detection signature."}">Stealth</button></span>`;
  }

  private hasCourse(fleet: GhostView): boolean {
    const queue = this.ctx.state.pendingOrders.get(fleet.id) ?? [];
    return !!this.ctx.state.orders[fleet.id]
      || !!fleet.path?.length
      || Math.hypot(fleet.vel.x, fleet.vel.y) >= 0.5
      || queue.some((order) => !order.lost && ["move", "jump", "guard", "raid", "attack", "recall", "withdraw", "haul"].includes(order.kind));
  }

  private selectedFleets(): GhostView[] {
    const state = this.ctx.state;
    const grouped = [...state.selectedShipIds]
      .map((id) => state.ghosts.find((fleet) => fleet.id === id && fleet.own))
      .filter((fleet): fleet is GhostView => !!fleet);
    if (grouped.length) return grouped;
    const primary = state.selectedShipId ? state.ghosts.find((fleet) => fleet.id === state.selectedShipId) : undefined;
    return primary ? [primary] : [];
  }

  private removeFleet(id: string): void {
    const state = this.ctx.state;
    state.selectedShipIds.delete(id);
    if (state.selectedShipId === id) {
      state.selectedShipId = state.selectedShipIds.values().next().value ?? null;
      state.selectedOrderId = null;
    }
    if (!state.selectedShipId) {
      this.ctx.intent.clearPendingIntent(true);
      this.ctx.intent.clearJumpAiming(true);
      this.ctx.intent.clearGuardAiming(true);
    }
  }

  private reconcileSelection(): void {
    const state = this.ctx.state;
    for (const id of [...state.selectedShipIds]) {
      if (!state.ghosts.some((fleet) => fleet.id === id && fleet.own)) state.selectedShipIds.delete(id);
    }
    if (state.selectedShipId && !state.ghosts.some((fleet) => fleet.id === state.selectedShipId)) {
      state.selectedShipId = state.selectedShipIds.values().next().value ?? null;
      state.selectedOrderId = null;
      this.ctx.renderer.stateVersion++;
    }
  }
}

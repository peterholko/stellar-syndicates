import { guardCapable, jumpCapable, shipKindLabel } from "../../core/derive/fleet";
import { gravityWellAt, nearestKnownDock } from "../../core/derive/geo";
import { intentSummary } from "../../core/derive/orders";
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
 * Direct policy verbs retain their existing wire messages; map verbs still
 * become PendingIntent previews and cannot transmit before confirmation. */
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
      selected.map((fleet) => [fleet.id, fleet.age, fleet.docked, fleet.pos, fleet.vel, fleet.path, fleet.composition]),
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
    setHtml(this.root, modeHtml ? modeHtml + this.statusLineHtml() : "");
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
      this.ctx.send({ type: "MoveShip", ship_id: fleet.id, dest: dock.pos });
      this.hooks.notice(`<b>Docking order sent</b> · ${esc(dock.name)} · ${Math.round(dock.distance).toLocaleString()} su from the served sighting.`);
    } else if (action === "jump" && fleet) {
      this.ctx.intent.armJumpAiming(fleet);
    } else if (action === "guard" && fleet) {
      this.ctx.intent.armGuardAiming(fleet);
    } else if (action === "hold" && fleet) {
      this.ctx.intent.clearPendingIntent(true);
      this.ctx.send({ type: "HoldFleet", ship_id: fleet.id });
      this.hooks.notice(`<b>Hold order sent</b> · the fleet stops at its true position when the signal reaches it.`);
    } else if (action === "transit" && fleet) {
      const mode = button.dataset.mode as TransitMode | undefined;
      if (!mode) return;
      this.ctx.send({ type: "SetFleetTransit", fleet_id: fleet.id, mode });
      this.hooks.notice(`<b>${mode === "full" ? "Full-speed" : "Stealth"} transit requested</b> · visible when the next report reaches command.`);
    } else if (action === "recall" && fleet) {
      this.ctx.send({ type: "RecallRaid", raider_id: fleet.id });
      delete this.ctx.state.raids[fleet.id];
      this.hooks.notice(`<b>Recall sent</b> · it may arrive after contact has already begun.`);
    }
    this.ctx.renderer.stateVersion++;
    this.render(true);
  }

  private intentHtml(summary: string): string {
    return `<div class="deck-command-strip__frame deck-command-strip__frame--intent">` +
      `<div class="deck-command-strip__mode">ORDER PREVIEW</div>` +
      `<div class="deck-command-strip__summary">${esc(summary)}</div>` +
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
      ? `click a destination within ${Math.round(this.ctx.state.galaxy?.jump_range ?? 0).toLocaleString()} su`
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
    return `<div class="deck-command-strip__frame deck-command-strip__frame--selection">` +
      `<div class="deck-command-strip__chips">${chips}</div>` +
      (fleets.length > 1
        ? `<div class="deck-command-strip__batch"><b>${fleets.length}-fleet command group</b> · map moves apply to every fleet; other controls apply to the primary chip.</div>`
        : "") +
      `<div class="deck-command-strip__verbs">${verbs}</div>` +
      `</div>`;
  }

  private fleetChip(fleet: GhostView): string {
    const composition = fleet.composition?.length
      ? fleet.composition.map((stack) => `◆ ${stack.count} ${shipKindLabel(stack.kind)}`).join(" · ")
      : "composition delayed";
    return `<div class="deck-fleet-chip${fleet.id === this.ctx.state.selectedShipId ? " is-primary" : ""}">` +
      `<span><b>${esc(shipKindLabel(fleet.kind))}</b><small>${esc(composition)}</small></span>` +
      `<span class="deck-fleet-chip__delay">Δt ${Math.round(fleet.age)}s</span>` +
      `<button type="button" data-deck-command="center" data-fleet-id="${esc(fleet.id)}" aria-label="Center fleet">⌾</button>` +
      `<button type="button" data-deck-command="remove" data-fleet-id="${esc(fleet.id)}" aria-label="Deselect fleet">✕</button>` +
      `</div>`;
  }

  private verbHtml(fleet: GhostView): string {
    const dock = nearestKnownDock(fleet);
    const hasCourse = this.hasCourse(fleet);
    const jumpReason = !jumpCapable(fleet)
      ? "All hulls must carry compatible jump drives."
      : fleet.docked
        ? "Undock before spooling."
        : gravityWellAt(fleet.pos, this.ctx.state)
          ? "Served sighting is inside a gravity well."
          : "";
    const guardReason = !guardCapable(fleet)
      ? "Requires an Interceptor fleet."
      : !this.ctx.state.ghosts.some((candidate) => candidate.own && candidate.id !== fleet.id && !candidate.docked)
        ? "No undocked friendly fleet to guard."
        : "";
    const dockReason = fleet.docked ? "Already docked." : dock ? "" : "No known friendly berth.";
    const holdReason = fleet.docked ? "Docked fleets are already stationary." : hasCourse ? "" : "No active course to cancel.";
    const recallReason = this.ctx.state.raids[fleet.id] ? "" : "Fleet has no active raid/intercept.";
    return [
      this.verb("move", "Move", "Click map"),
      this.verb("dock", "Dock", dock ? dock.name : "Nearest berth", dockReason),
      this.verb("jump", "Jump", this.ctx.state.galaxy ? `${Math.round(this.ctx.state.galaxy.jump_range).toLocaleString()} su` : "Range unavailable", jumpReason),
      this.verb("guard", "Guard", "Choose fleet", guardReason),
      this.verb("hold", "Hold", "Cancel course", holdReason),
      this.transitVerb(),
      this.verb("recall", "Recall", "Break off", recallReason),
    ].join("");
  }

  private verb(action: string, label: string, hint: string, reason = "", extra = ""): string {
    const detail = reason || hint;
    return `<span class="deck-command-verb${reason ? " is-disabled" : ""}">` +
      `<button type="button" data-deck-command="${action}"${extra}${reason ? " disabled" : ""} aria-label="${esc(label)}. ${esc(detail)}">${esc(label)}</button>` +
      `<em>${esc(detail)}</em>` +
      `</span>`;
  }

  private transitVerb(): string {
    return `<span class="deck-command-verb deck-command-verb--transit">` +
      `<span class="deck-command-transit" role="group" aria-label="Transit mode">` +
        `<span>Transit</span>` +
        `<button type="button" data-deck-command="transit" data-mode="full" aria-label="Set full-speed transit">Full</button>` +
        `<button type="button" data-deck-command="transit" data-mode="stealth" aria-label="Set stealth transit">Stealth</button>` +
      `</span><em>Choose the fleet's standing transit mode.</em></span>`;
  }

  private hasCourse(fleet: GhostView): boolean {
    const queue = this.ctx.state.pendingOrders.get(fleet.id) ?? [];
    return !!this.ctx.state.orders[fleet.id]
      || !!fleet.path?.length
      || Math.hypot(fleet.vel.x, fleet.vel.y) >= 0.5
      || queue.some((order) => order.kind !== "hold");
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

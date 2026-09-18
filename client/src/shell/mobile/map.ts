import { resolveMapClick, resolveSystemClick, type MapClickResult } from "../../core/mapclick";
import type { SystemInfo } from "../../protocol";
import type { CoreContext } from "../types";
import type { SheetEntry } from "./sheets";

const DRAG_THRESHOLD_PX = 7;
const LONG_PRESS_MS = 520;
const SCRUB_PER_LOG_SCALE = 1.6;

interface PointerPoint {
  x: number;
  y: number;
  startX: number;
  startY: number;
}

interface MobileMapHooks {
  openSheet(entry: SheetEntry): void;
  onSemanticChange(mode: "galaxy" | "system" | "battle", id?: string): void;
  notice(html: string): void;
}

// Mobile owns gesture interpretation, while core/mapclick owns every gameplay
// decision. A tap and long-press therefore cannot drift from desktop legality:
// they differ only in the `long` modifier passed to the shared resolver.
export class MobileMapInteraction {
  private readonly points = new Map<number, PointerPoint>();
  private longTimer: number | null = null;
  private primaryId: number | null = null;
  private lastPinchDistance = 0;
  private lastPinchMidpoint: { x: number; y: number } | null = null;
  private panning = false;
  private pinching = false;
  private longFired = false;
  private suppressTap = false;
  private moveAimingShipId: string | null = null;
  private readonly previousTouchAction: string;

  constructor(
    private readonly ctx: CoreContext,
    private readonly hooks: MobileMapHooks,
    signal: AbortSignal,
  ) {
    const canvas = ctx.renderer.canvas;
    this.previousTouchAction = canvas.style.touchAction;
    canvas.style.touchAction = "none";
    canvas.addEventListener("pointerdown", (event) => this.pointerDown(event), { signal });
    canvas.addEventListener("pointermove", (event) => this.pointerMove(event), { signal });
    canvas.addEventListener("pointerup", (event) => this.pointerUp(event), { signal });
    canvas.addEventListener("pointercancel", (event) => this.pointerCancel(event), { signal });
    canvas.addEventListener("pointerleave", (event) => {
      if (event.pointerType === "mouse") ctx.renderer.cursorWorld = null;
    }, { signal });
  }

  teardown(): void {
    this.cancelLongPress();
    this.ctx.renderer.canvas.style.touchAction = this.previousTouchAction;
    this.points.clear();
  }

  tick(): void {
    const endpoint = this.ctx.renderer.consumeSystemScrubEndpoint();
    if (endpoint?.type === "system") {
      this.enteredSystem(endpoint.systemId);
    } else if (endpoint?.type === "galaxy") {
      this.ctx.renderer.setSystemDynamic([], [], true);
      this.hooks.onSemanticChange("galaxy");
    }
  }

  syncArmedChip(): void {
    const intent = this.ctx.state.pendingIntent;
    const jump = this.ctx.intent.intentAiming.jump;
    const guard = this.ctx.intent.intentAiming.guard;
    const chip = document.getElementById("m-armed");
    const label = document.getElementById("m-armed-label");
    if (!chip || !label) return;
    const text = intent
      ? `${intent.verb.toUpperCase()} preview`
      : jump
        ? "JUMP · choose destination"
        : guard
          ? "GUARD · choose fleet"
          : this.moveAimingShipId
            ? "MOVE · choose destination"
          : "";
    chip.hidden = text === "";
    label.textContent = text;
  }

  cancelArmedMode(): void {
    if (this.ctx.state.pendingIntent) this.ctx.intent.clearPendingIntent();
    if (this.ctx.intent.intentAiming.jump) this.ctx.intent.clearJumpAiming();
    if (this.ctx.intent.intentAiming.guard) this.ctx.intent.clearGuardAiming();
    this.moveAimingShipId = null;
    this.syncArmedChip();
  }

  focusFleet(id: string): void {
    this.selectFleet(id);
    this.ctx.renderer.stateVersion++;
    this.hooks.openSheet({ id: "ship", props: { kind: "fleet", id } });
  }

  armMove(id: string): void {
    const fleet = this.ctx.state.ghosts.find((ghost) => ghost.id === id && ghost.own);
    if (!fleet) return;
    this.selectFleet(id);
    this.ctx.intent.clearJumpAiming(true);
    this.ctx.intent.clearGuardAiming(true);
    this.moveAimingShipId = id;
    this.showNotice(`<b>Move armed.</b> Tap empty map space for the destination.`);
    this.syncArmedChip();
  }

  focusSystem(id: string): void {
    this.clearSelection();
    this.ctx.state.selectedSystemId = id;
    this.ctx.renderer.stateVersion++;
    this.hooks.openSheet({ id: "system", props: { id } });
  }

  enterSystem(id: string): void {
    const system = this.systemById(id);
    if (!system) return;
    const dynamic = this.ctx.state.systems.find((entry) => entry.id === id);
    this.ctx.renderer.enterSystemView(system, dynamic?.bodies ?? []);
    this.ctx.state.selectedSystemId = id;
    this.ctx.renderer.setSystemDynamic(
      dynamic?.bodies ?? [],
      (dynamic?.builds ?? []).map((build) => ({ key: build.key, body_id: build.body_id })),
      dynamic?.habitat_fed ?? true,
    );
    this.hooks.openSheet({ id: "system", props: { id, semantic: true } });
    this.hooks.onSemanticChange("system", id);
  }

  exitSemanticView(): void {
    if (this.ctx.renderer.isSystemScrubbing()) this.ctx.renderer.cancelSystemScrub();
    else if (this.ctx.renderer.viewMode.type === "system") this.ctx.renderer.exitSystemView();
    else if (this.ctx.renderer.viewMode.type === "battle") this.ctx.renderer.exitBattleView();
    this.ctx.renderer.setSystemDynamic([], [], true);
    this.hooks.onSemanticChange("galaxy");
  }

  showNotice(html: string): void {
    this.hooks.notice(html);
  }

  private pointerDown(event: PointerEvent): void {
    if (event.button !== 0 && event.pointerType === "mouse") return;
    const point = { x: event.clientX, y: event.clientY, startX: event.clientX, startY: event.clientY };
    this.points.set(event.pointerId, point);
    try { this.ctx.renderer.canvas.setPointerCapture(event.pointerId); } catch { /* capture is optional */ }

    if (this.points.size === 1) {
      this.primaryId = event.pointerId;
      this.panning = false;
      this.pinching = false;
      this.longFired = false;
      this.suppressTap = false;
      this.startLongPress(event.pointerId);
    } else if (this.points.size === 2) {
      this.cancelLongPress();
      this.pinching = true;
      this.suppressTap = true;
      this.lastPinchDistance = this.pinchDistance();
      this.lastPinchMidpoint = this.pinchMidpoint();
    }
  }

  private pointerMove(event: PointerEvent): void {
    const point = this.points.get(event.pointerId);
    if (!point) return;
    const previousX = point.x;
    const previousY = point.y;
    point.x = event.clientX;
    point.y = event.clientY;

    if (event.pointerType === "mouse") {
      this.ctx.renderer.cursorWorld = this.ctx.renderer.screenToWorld(event.clientX, event.clientY);
    }

    if (this.points.size >= 2) {
      this.cancelLongPress();
      this.pinching = true;
      this.suppressTap = true;
      const distance = this.pinchDistance();
      const midpoint = this.pinchMidpoint();
      const previousMidpoint = this.lastPinchMidpoint;
      if (previousMidpoint && this.ctx.renderer.viewMode.type === "galaxy" && !this.ctx.renderer.isSystemScrubbing()) {
        this.ctx.renderer.panBy(midpoint.x - previousMidpoint.x, midpoint.y - previousMidpoint.y);
      }
      if (distance > 0 && this.lastPinchDistance > 0) {
        const factor = distance / this.lastPinchDistance;
        if (Number.isFinite(factor) && factor > 0) this.applyPinch(factor, midpoint);
      }
      this.lastPinchDistance = distance;
      this.lastPinchMidpoint = midpoint;
      return;
    }

    if (event.pointerId !== this.primaryId || this.longFired) return;
    const moved = Math.hypot(point.x - point.startX, point.y - point.startY);
    if (!this.panning && moved > DRAG_THRESHOLD_PX) {
      this.panning = true;
      this.suppressTap = true;
      this.cancelLongPress();
    }
    if (this.panning && this.ctx.renderer.viewMode.type === "galaxy" && !this.ctx.renderer.isSystemScrubbing()) {
      this.ctx.renderer.panBy(point.x - previousX, point.y - previousY);
    }
  }

  private pointerUp(event: PointerEvent): void {
    const point = this.points.get(event.pointerId);
    if (!point) return;
    const wasPrimary = event.pointerId === this.primaryId;
    const wasPinching = this.pinching;
    this.points.delete(event.pointerId);
    try { this.ctx.renderer.canvas.releasePointerCapture(event.pointerId); } catch { /* not captured */ }

    if (wasPinching) {
      this.cancelLongPress();
      if (this.points.size >= 2) {
        this.lastPinchDistance = this.pinchDistance();
        this.lastPinchMidpoint = this.pinchMidpoint();
      } else {
        this.pinching = false;
        this.lastPinchDistance = 0;
        this.lastPinchMidpoint = null;
        const remaining = this.points.entries().next().value as [number, PointerPoint] | undefined;
        if (remaining) {
          this.primaryId = remaining[0];
          remaining[1].startX = remaining[1].x;
          remaining[1].startY = remaining[1].y;
        } else {
          this.primaryId = null;
        }
      }
      return;
    }

    this.cancelLongPress();
    if (wasPrimary && !this.panning && !this.longFired && !this.suppressTap && !this.ctx.renderer.isSystemScrubbing()) {
      this.handleTap(point.x, point.y, false);
    }
    if (wasPrimary) this.primaryId = null;
    this.panning = false;
    this.longFired = false;
    this.suppressTap = false;
  }

  private pointerCancel(event: PointerEvent): void {
    this.points.delete(event.pointerId);
    this.cancelLongPress();
    if (this.pinching && this.points.size < 2) {
      this.pinching = false;
      this.lastPinchDistance = 0;
      this.lastPinchMidpoint = null;
    }
    if (!this.points.size) {
      this.primaryId = null;
      this.panning = false;
      this.pinching = false;
      this.longFired = false;
      this.suppressTap = false;
      this.lastPinchMidpoint = null;
    }
  }

  private startLongPress(pointerId: number): void {
    this.cancelLongPress();
    this.longTimer = window.setTimeout(() => {
      const point = this.points.get(pointerId);
      if (!point || this.points.size !== 1 || this.panning || this.pinching) return;
      this.longFired = true;
      this.suppressTap = true;
      navigator.vibrate?.(18);
      this.handleTap(point.x, point.y, true);
    }, LONG_PRESS_MS);
  }

  private cancelLongPress(): void {
    if (this.longTimer === null) return;
    window.clearTimeout(this.longTimer);
    this.longTimer = null;
  }

  private pinchDistance(): number {
    const [a, b] = [...this.points.values()];
    return a && b ? Math.hypot(b.x - a.x, b.y - a.y) : 0;
  }

  private pinchMidpoint(): { x: number; y: number } {
    const [a, b] = [...this.points.values()];
    return a && b ? { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 } : { x: 0, y: 0 };
  }

  private applyPinch(factor: number, midpoint: { x: number; y: number }): void {
    const renderer = this.ctx.renderer;
    const scrubDelta = Math.log(factor) * SCRUB_PER_LOG_SCALE;
    // Translation was already applied from the midpoint delta. A negligible
    // scale delta skips only zoom; two-finger drag remains live.
    if (Math.abs(scrubDelta) < 0.0001) return;

    if (renderer.isSystemScrubbing()) {
      renderer.adjustSystemScrub(scrubDelta);
      return;
    }
    if (renderer.viewMode.type === "battle") {
      if (factor < 1) {
        renderer.exitBattleView();
        this.hooks.onSemanticChange("galaxy");
      }
      return;
    }
    if (renderer.viewMode.type === "system") {
      if (factor < 1) {
        const system = this.systemById(renderer.viewMode.systemId);
        if (system && renderer.beginSystemScrubOut(system)) renderer.adjustSystemScrub(scrubDelta);
      }
      return;
    }

    if (factor > 1 && renderer.atBattleZoomThreshold()) {
      const battleId = renderer.battlePick(midpoint.x, midpoint.y);
      const battle = battleId === null ? undefined : this.ctx.state.battles.find((entry) => entry.id === battleId);
      if (battle) {
        renderer.enterBattleView(battle.id, battle.pos);
        this.hooks.openSheet({ id: "battle", props: { id: battle.id } });
        this.hooks.onSemanticChange("battle", battle.id);
        return;
      }
    }

    if (factor > 1 && renderer.atMaxZoom()) {
      const system = this.systemUnderPoint(midpoint.x, midpoint.y);
      const bodies = system ? this.ctx.state.systems.find((entry) => entry.id === system.id)?.bodies ?? [] : [];
      if (system && renderer.beginSystemScrubIn(system, bodies)) {
        renderer.adjustSystemScrub(scrubDelta);
        return;
      }
    }
    renderer.zoomAt(midpoint.x, midpoint.y, factor);
  }

  private handleTap(x: number, y: number, long: boolean): void {
    const renderer = this.ctx.renderer;
    renderer.selectedBattleMarkerId = null;
    const clickCtx = {
      state: this.ctx.state,
      renderer,
      jumpAiming: this.ctx.intent.intentAiming.jump,
      guardAiming: this.ctx.intent.intentAiming.guard,
      emplaceArmed: null,
    };
    const result = renderer.viewMode.type === "system"
      ? resolveSystemClick(x, y, clickCtx)
      : renderer.viewMode.type === "galaxy"
        ? resolveMapClick(x, y, { shift: false, long, inspect: false }, clickCtx)
        : { kind: "none" } as const;
    this.applyResult(result);
  }

  private applyResult(result: MapClickResult): void {
    if (result.kind === "reject") {
      if (result.clearAiming === "jump") this.ctx.intent.clearJumpAiming(true);
      else if (result.clearAiming === "guard") this.ctx.intent.clearGuardAiming(true);
      this.showNotice(result.reason);
      this.syncArmedChip();
      return;
    }
    if (result.kind === "intent") {
      if (result.clearAiming === "jump") this.ctx.intent.clearJumpAiming(true);
      else if (result.clearAiming === "guard") this.ctx.intent.clearGuardAiming(true);
      this.ctx.intent.beginPendingIntent(result.intent);
      this.moveAimingShipId = null;
      if (result.readout) this.showNotice(result.readout);
      this.syncArmedChip();
      return;
    }
    if (result.kind !== "select") return;

    const target = result.target;
    switch (target.type) {
      case "fleet":
        this.focusFleet(target.id);
        break;
      case "jumpDeparture":
        this.clearSelection();
        this.ctx.renderer.selectedJumpDepartureKey = target.key;
        this.hooks.openSheet({ id: "ship", props: { kind: "jumpDeparture", key: target.key } });
        break;
      case "emplacement":
        this.clearSelection();
        this.ctx.state.selectedEmplacementId = target.id;
        this.hooks.openSheet({ id: "ship", props: { kind: "emplacement", id: target.id } });
        break;
      case "system":
        this.clearSelection();
        this.ctx.state.selectedSystemId = target.id;
        this.hooks.openSheet({ id: "system", props: { id: target.id } });
        break;
      case "hub":
        this.clearSelection();
        this.hooks.openSheet({ id: "hub" });
        break;
      case "exploration":
        this.clearSelection();
        this.ctx.state.selectedExplorationSiteId = target.id;
        this.hooks.openSheet({ id: "operations" });
        break;
      case "ongoingBattle":
        this.hooks.openSheet({ id: "battle", props: { id: target.id } });
        break;
      case "aftermath":
        this.ctx.renderer.selectedBattleMarkerId = target.id;
        this.hooks.openSheet({ id: "log", props: { aftermathId: target.id } });
        break;
      case "capture":
        this.ctx.renderer.selectedBattleMarkerId = target.id;
        this.hooks.openSheet({ id: "log", props: { captureId: target.id } });
        break;
      case "systemBody":
        this.hooks.openSheet({
          id: "planet",
          props: {
            systemId: this.ctx.renderer.viewMode.type === "system" ? this.ctx.renderer.viewMode.systemId : null,
            detail: target.detail,
          },
        });
        break;
      case "clearSystemBody":
      case "anchor":
        break;
    }
    this.ctx.renderer.stateVersion++;
    if (target.readout) this.showNotice(target.readout);
  }

  private selectFleet(id: string): void {
    if (this.ctx.state.selectedShipId !== id) {
      this.ctx.intent.clearPendingIntent(true);
      this.ctx.intent.clearJumpAiming(true);
      this.ctx.state.selectedOrderId = null;
    }
    this.ctx.intent.clearGuardAiming(true);
    this.ctx.renderer.selectedJumpDepartureKey = null;
    this.moveAimingShipId = null;
    this.ctx.state.selectedShipId = id;
    this.ctx.state.selectedSystemId = null;
    this.ctx.state.selectedEmplacementId = null;
  }

  private clearSelection(): void {
    this.ctx.state.selectedExplorationSiteId = null;
    this.ctx.intent.clearPendingIntent(true);
    this.ctx.intent.clearJumpAiming(true);
    this.ctx.intent.clearGuardAiming(true);
    this.ctx.state.selectedOrderId = null;
    this.ctx.state.selectedShipId = null;
    this.ctx.state.selectedSystemId = null;
    this.ctx.state.selectedEmplacementId = null;
    this.ctx.renderer.selectedJumpDepartureKey = null;
  }

  private systemUnderPoint(x: number, y: number): SystemInfo | null {
    let best: SystemInfo | null = null;
    let bestDistance = Infinity;
    for (const system of this.ctx.state.galaxy?.systems ?? []) {
      const screen = this.ctx.renderer.worldToScreen(system.pos);
      const distance = Math.hypot(screen.x - x, screen.y - y);
      if (distance < Math.max(22, this.ctx.renderer.systemHitRadius(system)) && distance < bestDistance) {
        best = system;
        bestDistance = distance;
      }
    }
    return best;
  }

  private systemById(id: string): SystemInfo | undefined {
    return this.ctx.state.galaxy?.systems.find((system) => system.id === id);
  }

  private enteredSystem(id: string): void {
    const dynamic = this.ctx.state.systems.find((entry) => entry.id === id);
    this.ctx.state.selectedSystemId = id;
    this.ctx.renderer.setSystemDynamic(
      dynamic?.bodies ?? [],
      (dynamic?.builds ?? []).map((build) => ({ key: build.key, body_id: build.body_id })),
      dynamic?.habitat_fed ?? true,
    );
    this.hooks.openSheet({ id: "system", props: { id, semantic: true } });
    this.hooks.onSemanticChange("system", id);
  }
}

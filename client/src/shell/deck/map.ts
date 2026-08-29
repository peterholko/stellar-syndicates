import { shipKindLabel } from "../../core/derive/fleet";
import { type MapClickResult, type SelectTarget, resolveMapClick, resolveSystemClick } from "../../core/mapclick";
import type { GhostView, SystemInfo } from "../../protocol";
import type { CoreContext } from "../types";

const DRAG_THRESHOLD_PX = 5;
const SYSTEM_SCRUB_STEP = 0.18;

interface DeckMapHooks {
  enteredSystem(system: SystemInfo): void;
  returnedToGalaxy(): void;
  openTarget(target: SelectTarget): void;
  notice(html: string): void;
}

/** The Deck's map grammar lives on the shared resolver: a short left press may
 * select/command, a drag only pans, and right-click is inspect-only. */
export class DeckMapInteraction {
  private down = false;
  private panning = false;
  private startX = 0;
  private startY = 0;
  private lastX = 0;
  private lastY = 0;
  private readonly previousTouchAction: string;
  private dynamicTick = -1;
  private dynamicSystemId = "";

  constructor(
    private readonly ctx: CoreContext,
    private readonly hooks: DeckMapHooks,
    signal: AbortSignal,
  ) {
    const canvas = ctx.renderer.canvas;
    this.previousTouchAction = canvas.style.touchAction;
    canvas.style.touchAction = "none";
    canvas.addEventListener("pointerdown", (event) => this.pointerDown(event), { signal });
    canvas.addEventListener("pointermove", (event) => this.pointerMove(event), { signal });
    canvas.addEventListener("pointerup", (event) => this.pointerUp(event), { signal });
    canvas.addEventListener("pointercancel", () => this.pointerCancel(), { signal });
    canvas.addEventListener("pointerleave", (event) => {
      if (event.pointerType === "mouse") {
        ctx.renderer.cursorWorld = null;
        this.hideHover();
      }
    }, { signal });
    canvas.addEventListener("contextmenu", (event) => this.inspect(event), { signal });
    canvas.addEventListener("dblclick", (event) => this.doubleClick(event), { signal });
    canvas.addEventListener("wheel", (event) => this.wheel(event), { passive: false, signal });
  }

  tick(): void {
    if (this.ctx.renderer.viewMode.type === "system") {
      const systemId = this.ctx.renderer.viewMode.systemId;
      if (this.ctx.state.tick !== this.dynamicTick || systemId !== this.dynamicSystemId) {
        this.dynamicTick = this.ctx.state.tick;
        this.dynamicSystemId = systemId;
        this.pushSystemDynamic(systemId);
      }
    }
    const endpoint = this.ctx.renderer.consumeSystemScrubEndpoint();
    if (endpoint?.type === "system") {
      const system = this.systemById(endpoint.systemId);
      if (!system) return;
      this.pushSystemDynamic(system.id);
      this.hooks.enteredSystem(system);
    } else if (endpoint?.type === "galaxy") {
      this.ctx.renderer.setSystemDynamic([], [], true);
      this.hooks.returnedToGalaxy();
    }
  }

  zoomIn(): void {
    if (this.ctx.renderer.viewMode.type === "galaxy") this.ctx.renderer.zoomByFactor(1.3);
  }

  zoomOut(): void {
    const renderer = this.ctx.renderer;
    if (renderer.isSystemScrubbing()) {
      renderer.adjustSystemScrub(-SYSTEM_SCRUB_STEP);
      return;
    }
    if (renderer.viewMode.type === "system") {
      const system = this.systemById(renderer.viewMode.systemId);
      if (system && renderer.beginSystemScrubOut(system)) renderer.adjustSystemScrub(-SYSTEM_SCRUB_STEP);
      return;
    }
    if (renderer.viewMode.type === "galaxy") renderer.zoomByFactor(1 / 1.3);
  }

  fit(): void {
    const renderer = this.ctx.renderer;
    if (renderer.isSystemScrubbing()) renderer.cancelSystemScrub();
    else if (renderer.viewMode.type === "system") {
      const system = this.systemById(renderer.viewMode.systemId);
      if (system && renderer.beginSystemScrubOut(system)) renderer.adjustSystemScrub(-1);
    }
    else if (renderer.viewMode.type === "galaxy") renderer.resetView();
  }

  teardown(): void {
    this.ctx.renderer.canvas.style.touchAction = this.previousTouchAction;
    this.ctx.renderer.canvas.style.cursor = "";
    this.ctx.renderer.cursorWorld = null;
    this.hideHover();
  }

  private pointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    this.down = true;
    this.panning = false;
    this.startX = this.lastX = event.clientX;
    this.startY = this.lastY = event.clientY;
    try { this.ctx.renderer.canvas.setPointerCapture(event.pointerId); } catch { /* optional */ }
  }

  private pointerMove(event: PointerEvent): void {
    this.ctx.renderer.cursorWorld = this.ctx.renderer.screenToWorld(event.clientX, event.clientY);
    this.updateHover(event.clientX, event.clientY);
    if (!this.down) return;
    if (!this.panning && Math.hypot(event.clientX - this.startX, event.clientY - this.startY) > DRAG_THRESHOLD_PX) {
      this.panning = true;
    }
    if (this.panning && this.ctx.renderer.viewMode.type === "galaxy" && !this.ctx.renderer.isSystemScrubbing()) {
      this.ctx.renderer.panBy(event.clientX - this.lastX, event.clientY - this.lastY);
    }
    this.lastX = event.clientX;
    this.lastY = event.clientY;
  }

  private pointerUp(event: PointerEvent): void {
    if (!this.down) return;
    const wasPanning = this.panning;
    this.down = false;
    this.panning = false;
    try { this.ctx.renderer.canvas.releasePointerCapture(event.pointerId); } catch { /* optional */ }
    if (!wasPanning && !this.ctx.renderer.isSystemScrubbing()) {
      this.activate(event.clientX, event.clientY, event.shiftKey, false, event.ctrlKey || event.metaKey);
    }
  }

  private pointerCancel(): void {
    this.down = false;
    this.panning = false;
  }

  private wheel(event: WheelEvent): void {
    event.preventDefault();
    const renderer = this.ctx.renderer;
    if (renderer.viewMode.type === "battle") return;
    if (renderer.isSystemScrubbing()) {
      renderer.adjustSystemScrub(event.deltaY < 0 ? SYSTEM_SCRUB_STEP : -SYSTEM_SCRUB_STEP);
      return;
    }
    if (renderer.viewMode.type === "system") {
      if (event.deltaY > 0) {
        const system = this.systemById(renderer.viewMode.systemId);
        if (system && renderer.beginSystemScrubOut(system)) renderer.adjustSystemScrub(-SYSTEM_SCRUB_STEP);
      }
      return;
    }
    const battleId = renderer.battlePick(event.clientX, event.clientY);
    if (event.deltaY < 0 && battleId !== null && renderer.atBattleZoomThreshold()) {
      const battle = this.ctx.state.battles.find((entry) => entry.id === battleId);
      if (battle) {
        renderer.enterBattleView(battle.id, battle.pos);
        this.hooks.openTarget({ type: "ongoingBattle", id: battle.id });
        return;
      }
    }
    if (event.deltaY < 0 && renderer.atMaxZoom()) {
      const system = this.systemUnderPoint(event.clientX, event.clientY);
      const bodies = system ? this.ctx.state.systems.find((entry) => entry.id === system.id)?.bodies ?? [] : [];
      if (system && renderer.beginSystemScrubIn(system, bodies)) {
        renderer.adjustSystemScrub(SYSTEM_SCRUB_STEP);
        return;
      }
    }
    renderer.zoomAt(event.clientX, event.clientY, Math.exp(-event.deltaY * 0.0016));
  }

  private inspect(event: MouseEvent): void {
    event.preventDefault();
    if (this.ctx.renderer.viewMode.type !== "galaxy" || this.ctx.renderer.isSystemScrubbing()) return;
    this.activate(event.clientX, event.clientY, event.shiftKey, true, false);
  }

  private doubleClick(event: MouseEvent): void {
    const renderer = this.ctx.renderer;
    if (renderer.viewMode.type !== "galaxy" || renderer.isSystemScrubbing()) return;
    const battleId = renderer.battlePick(event.clientX, event.clientY);
    if (battleId !== null) {
      const battle = this.ctx.state.battles.find((entry) => entry.id === battleId);
      if (battle) {
        renderer.enterBattleView(battle.id, battle.pos);
        this.hooks.openTarget({ type: "ongoingBattle", id: battle.id });
      }
      return;
    }
    const system = this.systemUnderPoint(event.clientX, event.clientY);
    if (!system) return;
    // The browser delivers the constituent click before dblclick. Remove the
    // star command that first click may have previewed: double-click means
    // semantic entry, never a hidden move/blockade/survey on the way in.
    const pending = this.ctx.state.pendingIntent;
    if (pending?.targetId === system.id
      && (pending.verb === "move" || pending.verb === "blockade" || pending.verb === "survey")) {
      this.ctx.intent.clearPendingIntent(true);
    }
    this.pushSystemDynamic(system.id);
    const dynamic = this.ctx.state.systems.find((entry) => entry.id === system.id);
    renderer.enterSystemView(system, dynamic?.bodies ?? []);
    this.hooks.enteredSystem(system);
  }

  cycleFleet(direction: -1 | 1): void {
    const fleets = this.ctx.state.ghosts
      .filter((fleet) => fleet.own)
      .sort((a, b) => a.id.localeCompare(b.id));
    if (!fleets.length) return;
    const current = fleets.findIndex((fleet) => fleet.id === this.ctx.state.selectedShipId);
    const index = current < 0 ? 0 : (current + direction + fleets.length) % fleets.length;
    const fleet = fleets[index];
    this.selectFleet(fleet.id);
    this.ctx.renderer.centerOnWorld(fleet.pos);
  }

  private activate(x: number, y: number, shift: boolean, inspect: boolean, multi: boolean): void {
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
      ? inspect ? { kind: "none" } as const : resolveSystemClick(x, y, clickCtx)
      : renderer.viewMode.type === "galaxy"
        ? resolveMapClick(x, y, { shift, long: false, inspect }, clickCtx)
        : { kind: "none" } as const;
    this.applyResult(result, inspect, multi);
  }

  private applyResult(result: MapClickResult, inspect: boolean, multi: boolean): void {
    if (result.kind === "reject") {
      if (result.clearAiming === "jump") this.ctx.intent.clearJumpAiming(true);
      else if (result.clearAiming === "guard") this.ctx.intent.clearGuardAiming(true);
      this.hooks.notice(result.reason);
      return;
    }
    if (result.kind === "intent") {
      if (result.clearAiming === "jump") this.ctx.intent.clearJumpAiming(true);
      else if (result.clearAiming === "guard") this.ctx.intent.clearGuardAiming(true);
      if (result.intent.verb === "move" && this.ctx.state.selectedShipIds.size > 1) {
        result.intent.shipIds = [...this.ctx.state.selectedShipIds]
          .filter((id) => this.ctx.state.ghosts.some((fleet) => fleet.id === id && fleet.own));
      }
      this.ctx.intent.beginPendingIntent(result.intent);
      if (result.readout) this.hooks.notice(result.readout);
      return;
    }
    if (result.kind !== "select") return;
    if (!inspect && multi && result.target.type === "fleet") {
      const fleetId = result.target.id;
      const fleet = this.ctx.state.ghosts.find((entry) => entry.id === fleetId && entry.own);
      if (fleet) {
        this.toggleFleet(fleet.id);
        const count = this.ctx.state.selectedShipIds.size;
        this.hooks.notice(count > 1
          ? `<b>${count} fleets grouped</b> · map moves apply to every fleet; other verbs apply to the primary chip.`
          : count === 1
            ? `<b>Fleet selected</b> · Ctrl/⌘-click another owned marker to add it.`
            : `<span class="dim">Fleet group cleared.</span>`);
        return;
      }
    }
    if (!inspect && result.target.type === "fleet") this.selectFleet(result.target.id);
    else if (!inspect && result.target.type === "system") {
      this.ctx.state.selectedSystemId = result.target.id;
      this.ctx.renderer.stateVersion++;
    } else if (!inspect && result.target.type === "emplacement") {
      this.clearFleetSelection();
      this.ctx.state.selectedEmplacementId = result.target.id;
      this.ctx.renderer.stateVersion++;
    } else if (!inspect && (result.target.type === "aftermath" || result.target.type === "capture")) {
      this.ctx.renderer.selectedBattleMarkerId = result.target.id;
    }
    this.hooks.openTarget(result.target);
    if (result.target.readout) this.hooks.notice(result.target.readout);
  }

  private selectFleet(id: string): void {
    const state = this.ctx.state;
    const fleet = state.ghosts.find((ghost) => ghost.id === id);
    if (!fleet) return;
    if (state.selectedShipId !== id) {
      this.ctx.intent.clearPendingIntent(true);
      this.ctx.intent.clearJumpAiming(true);
      this.ctx.intent.clearGuardAiming(true);
      state.selectedOrderId = null;
    }
    state.selectedShipId = id;
    state.selectedShipIds.clear();
    if (fleet.own) state.selectedShipIds.add(id);
    state.selectedSystemId = null;
    state.selectedEmplacementId = null;
    this.ctx.renderer.selectedJumpDepartureKey = null;
    this.ctx.renderer.stateVersion++;
  }

  private toggleFleet(id: string): void {
    const state = this.ctx.state;
    if (state.selectedShipIds.has(id)) {
      state.selectedShipIds.delete(id);
      if (state.selectedShipId === id) {
        state.selectedShipId = state.selectedShipIds.values().next().value ?? null;
        state.selectedOrderId = null;
      }
    } else {
      state.selectedShipIds.add(id);
      if (!state.selectedShipId || !state.ghosts.some((fleet) => fleet.id === state.selectedShipId && fleet.own)) {
        state.selectedShipId = id;
      }
    }
    this.ctx.intent.clearPendingIntent(true);
    this.ctx.renderer.stateVersion++;
  }

  private clearFleetSelection(): void {
    this.ctx.intent.clearPendingIntent(true);
    this.ctx.intent.clearJumpAiming(true);
    this.ctx.intent.clearGuardAiming(true);
    this.ctx.state.selectedOrderId = null;
    this.ctx.state.selectedShipId = null;
    this.ctx.state.selectedShipIds.clear();
  }

  private updateHover(clientX: number, clientY: number): void {
    const renderer = this.ctx.renderer;
    const state = this.ctx.state;
    if (renderer.viewMode.type !== "galaxy") {
      this.hideHover();
      renderer.canvas.style.cursor = "default";
      return;
    }

    let copy = "";
    let bestDistance = Infinity;
    let bestPriority = Infinity;
    const consider = (priority: number, distance: number, text: string): void => {
      if (priority > bestPriority || (priority === bestPriority && distance >= bestDistance)) return;
      bestPriority = priority;
      bestDistance = distance;
      copy = text;
    };
    const engaged = new Map<string, { x: number; y: number }>();
    for (const battle of state.battles) for (const id of battle.participants) engaged.set(id, battle.pos);
    for (const ghost of state.ghosts) {
      const battlePos = engaged.get(ghost.id);
      const point = battlePos ? renderer.worldToScreen(battlePos) : renderer.fleetScreenPosition(ghost);
      const distance = Math.hypot(point.x - clientX, point.y - clientY);
      const radius = ghost.docked ? 11 : Math.max(18, renderer.fleetHitRadius(ghost));
      if (distance >= radius) continue;
      consider(ghost.own ? 0 : 1, distance, this.fleetHover(ghost, !!battlePos));
    }
    const selected = state.selectedShipId
      ? state.ghosts.find((ghost) => ghost.id === state.selectedShipId && ghost.own)
      : undefined;
    if (state.galaxy) {
      for (const system of state.galaxy.systems) {
        const point = renderer.worldToScreen(system.pos);
        const distance = Math.hypot(point.x - clientX, point.y - clientY);
        if (distance >= Math.max(15, renderer.systemHitRadius(system))) continue;
        consider(1, distance, selected
          ? `Move ${shipKindLabel(selected.kind)} to ${system.name}`
          : `Open ${system.name}`);
      }
      const hub = renderer.worldToScreen(state.galaxy.hub);
      const distance = Math.hypot(hub.x - clientX, hub.y - clientY);
      if (distance < Math.max(24, renderer.hubHitRadius())) {
        consider(1, distance, selected ? `Move ${shipKindLabel(selected.kind)} to Market Hub` : "Open Market Hub");
      }
    }
    const battleId = renderer.battlePick(clientX, clientY);
    if (battleId !== null) consider(1, 0, "Open ongoing battle");
    if (!copy) {
      this.hideHover();
      renderer.canvas.style.cursor = selected ? "crosshair" : "grab";
      return;
    }
    const tip = document.getElementById("deck-hover")!;
    tip.textContent = `${copy} · RMB inspect`;
    tip.hidden = false;
    const left = Math.min(window.innerWidth - Math.max(180, tip.offsetWidth) - 8, clientX + 14);
    const top = Math.min(window.innerHeight - tip.offsetHeight - 8, clientY + 16);
    tip.style.left = `${Math.max(8, left)}px`;
    tip.style.top = `${Math.max(8, top)}px`;
    renderer.canvas.style.cursor = "pointer";
  }

  private fleetHover(ghost: GhostView, engaged: boolean): string {
    if (ghost.own) {
      return `${shipKindLabel(ghost.kind)} · ${ghost.docked ? "select berth" : engaged ? "select engaged fleet" : "select fleet"}`;
    }
    const selected = this.ctx.state.selectedShipId
      ? this.ctx.state.ghosts.find((fleet) => fleet.id === this.ctx.state.selectedShipId && fleet.own)
      : undefined;
    if (selected?.kind === "raider") return `Raid ${shipKindLabel(ghost.kind)} contact · Shift attacks`;
    return `${shipKindLabel(ghost.kind)} contact · delayed intelligence`;
  }

  private hideHover(): void {
    const tip = document.getElementById("deck-hover");
    if (tip) tip.hidden = true;
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

  private pushSystemDynamic(id: string): void {
    const dynamic = this.ctx.state.systems.find((entry) => entry.id === id);
    this.ctx.state.selectedSystemId = id;
    this.ctx.renderer.setSystemDynamic(
      dynamic?.bodies ?? [],
      (dynamic?.builds ?? []).map((build) => ({ key: build.key, body_id: build.body_id })),
      dynamic?.habitat_fed ?? true,
    );
  }
}

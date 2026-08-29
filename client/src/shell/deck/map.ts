import type { SystemInfo } from "../../protocol";
import type { CoreContext } from "../types";

const DRAG_THRESHOLD_PX = 5;
const SYSTEM_SCRUB_STEP = 0.18;

interface DeckMapHooks {
  enteredSystem(system: SystemInfo): void;
  returnedToGalaxy(): void;
}

/** D0 owns only the continuous map camera: pan, wheel zoom, and the existing
 * galaxy/system semantic scrub. Selection and command clicks arrive in D1,
 * keeping this scaffold incapable of accidentally issuing gameplay orders. */
export class DeckMapNavigation {
  private down = false;
  private panning = false;
  private startX = 0;
  private startY = 0;
  private lastX = 0;
  private lastY = 0;
  private readonly previousTouchAction: string;

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
      if (event.pointerType === "mouse") ctx.renderer.cursorWorld = null;
    }, { signal });
    canvas.addEventListener("wheel", (event) => this.wheel(event), { passive: false, signal });
  }

  tick(): void {
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
    this.ctx.renderer.cursorWorld = null;
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
    this.down = false;
    this.panning = false;
    try { this.ctx.renderer.canvas.releasePointerCapture(event.pointerId); } catch { /* optional */ }
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

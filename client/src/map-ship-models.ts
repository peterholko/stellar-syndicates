import { CanvasSource, Rectangle, Sprite, Texture } from "pixi.js";
import { MAP_MODEL_CLOSE_SLOTS, MAP_MODEL_FADE_MS, MAP_MODEL_FRAME_PADDING, MAP_MODEL_IDLE_MS, MAP_MODEL_TILT,
  mapModelBlend, mapModelCloseUp, pickMapModels, type MapModelCandidate } from "./map-ship-model-policy";
import type { MapModelDraw, MapShipModelRuntime } from "./map-ship-model-runtime";

interface Candidate extends MapModelCandidate { sprite: Sprite; model: Sprite }
interface Slot { index: number; mix: number }

/** The small, eager bridge. Three.js and GLBs are absent at normal map zoom. */
export class MapShipModels {
  private candidates: Candidate[] = [];
  private applied: Candidate[] = [];
  private slots = new Map<string, Slot>();
  private runtime: MapShipModelRuntime | null = null;
  private source: CanvasSource | null = null;
  private textures: Texture[] = [];
  private importing = false;
  private wanted = false;
  private retryAt = 0;
  private idleTimer: number | null = null;
  private lastFrame = 0;
  private active: string[] = [];

  beginFrame(): void {
    for (const item of this.applied) {
      if (!item.sprite.destroyed) item.sprite.alpha = item.alpha;
      if (!item.model.destroyed) item.model.visible = false;
    }
    this.applied = [];
    this.candidates = [];
    this.active = [];
  }

  add(candidate: Candidate): void { this.candidates.push(candidate); }

  private load(narrow: boolean, closeUp: boolean): void {
    if (this.runtime || this.importing || performance.now() < this.retryAt) return;
    this.importing = true;
    void import("./map-ship-model-runtime").then(({ MapShipModelRuntime }) => {
      if (!this.wanted) return;
      const runtime = new MapShipModelRuntime(narrow, closeUp);
      this.runtime = runtime;
      // Three supplies premultiplied canvas pixels. Let Pixi upload the canvas
      // once per changed atlas, never once per hull or ordinary movement frame.
      this.source = new CanvasSource({ resource: runtime.canvas, transparent: true,
        resolution: 1, autoDensity: false, autoGenerateMipmaps: false });
      this.textures = Array.from({ length: runtime.slotCount }, (_, slot) => new Texture({ source: this.source!,
        frame: new Rectangle(slot % runtime.columns * runtime.tileSize,
          Math.floor(slot / runtime.columns) * runtime.tileSize, runtime.tileSize, runtime.tileSize) }));
    }).catch(error => {
      this.release();
      this.retryAt = performance.now() + 30_000;
      console.warn("Map 3D is unavailable; retaining ship artwork.", error);
    }).finally(() => { this.importing = false; });
  }

  endFrame(zoom: number, width: number, height: number, now: number): void {
    const desired = mapModelBlend(zoom);
    const narrow = width < 700;
    let picked = desired > 0 ? pickMapModels(this.candidates, width, height) : [];
    const closeUp = mapModelCloseUp(picked, narrow, this.runtime?.closeUp ?? false);
    if (closeUp) picked = picked.slice(0, MAP_MODEL_CLOSE_SLOTS);
    this.wanted = picked.length > 0;
    const dt = this.lastFrame ? Math.min(now - this.lastFrame, 50) : 16;
    this.lastFrame = now;
    if (!this.wanted) {
      this.slots.clear();
      this.runtime?.cancelPending();
      if (this.runtime && this.idleTimer === null) this.idleTimer = window.setTimeout(() => this.release(), MAP_MODEL_IDLE_MS);
      return;
    }
    if (this.idleTimer !== null) { window.clearTimeout(this.idleTimer); this.idleTimer = null; }
    if (this.runtime && !this.runtime.healthy) {
      this.release(); this.retryAt = now + 30_000;
    }
    if (this.runtime && (this.runtime.closeUp !== closeUp || this.runtime.narrow !== narrow)) this.release();
    this.load(narrow, closeUp);
    if (!this.runtime || !this.source) return;
    const ids = new Set(picked.map(item => item.id));
    for (const id of this.slots.keys()) if (!ids.has(id)) this.slots.delete(id);
    const taken = new Set([...this.slots.values()].map(slot => slot.index));
    for (const item of picked) if (!this.slots.has(item.id)) {
      const index = Array.from({ length: this.runtime!.slotCount }, (_, i) => i).find(i => !taken.has(i))!;
      this.slots.set(item.id, { index, mix: 0 }); taken.add(index);
    }
    const draws: MapModelDraw[] = picked.map(item => ({ ...item, slot: this.slots.get(item.id)!.index,
      tilt: desired * MAP_MODEL_TILT }));
    try {
      if (this.runtime.render(draws, now)) this.source.update();
      for (let i = 0; i < picked.length; i++) {
        const item = picked[i], draw = draws[i], slot = this.slots.get(item.id)!;
        const sample = this.runtime.sample(draw);
        if (!sample) { slot.mix = 0; continue; }
        slot.mix += Math.sign(desired - slot.mix) * Math.min(Math.abs(desired - slot.mix), dt / MAP_MODEL_FADE_MS);
        item.model.texture = this.textures[slot.index];
        item.model.anchor.set(.5);
        item.model.scale.set(item.canvasPx * MAP_MODEL_FRAME_PADDING / this.runtime.tileSize);
        item.model.rotation = item.angle - sample.angle;
        item.model.alpha = item.alpha * slot.mix;
        item.model.visible = true;
        item.sprite.alpha = item.alpha * (1 - slot.mix);
        this.applied.push(item);
        this.active.push(item.id);
      }
    } catch (error) {
      this.beginFrame(); this.release(); this.retryAt = now + 30_000;
      console.warn("Map model render failed; retaining ship artwork.", error);
    }
  }

  debug() { return { active: [...this.active], slots: this.slots.size, importing: this.importing,
    runtime: this.runtime?.debug() ?? null }; }

  /** Also used when the mobile shell stops rendering the covered map. */
  suspend(): void { this.beginFrame(); this.wanted = false; this.release(); }

  private release(): void {
    if (this.idleTimer !== null) window.clearTimeout(this.idleTimer);
    this.idleTimer = null;
    // Detach sprite references before destroying the shared atlas.
    for (const item of this.applied) if (!item.model.destroyed) {
      item.model.visible = false; item.model.texture = Texture.EMPTY;
      if (!item.sprite.destroyed) item.sprite.alpha = item.alpha;
    }
    this.applied = []; this.active = []; this.slots.clear();
    for (const texture of this.textures) texture.destroy(false);
    this.textures = [];
    this.source?.destroy(); this.source = null;
    this.runtime?.dispose(); this.runtime = null;
  }
}

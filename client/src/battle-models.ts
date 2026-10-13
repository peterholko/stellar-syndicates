import { CanvasSource, Sprite, Texture } from "pixi.js";
import type { BattleModelFrame } from "./battle-model-policy";
import type { BattleModelRuntime, BattleModelSample } from "./battle-model-runtime";

const EMPTY: BattleModelSample = { ships: new Set(), bursts: new Set(), wrecks: new Set(), changed: false };

/** Eager Pixi bridge; Three and the GLBs load only after an actual keyframe. */
export class BattleModels {
  readonly sprite = new Sprite(Texture.EMPTY);
  private runtime: BattleModelRuntime | null = null;
  private source: CanvasSource | null = null;
  private texture: Texture | null = null;
  private importing = false;
  private generation = 0;
  private retryAt = 0;
  private narrow = false;

  constructor() { this.sprite.eventMode = "none"; this.sprite.visible = false; }

  render(frame: BattleModelFrame, now: number): BattleModelSample {
    const wanted = frame.ships.length > 0 || frame.bursts.length > 0 || (frame.wrecks?.length ?? 0) > 0;
    if (this.runtime && !this.runtime.healthy) { this.close(); this.retryAt = now + 30_000; }
    if ((this.runtime || this.importing) && this.narrow !== (frame.width < 700)) this.close();
    if (!this.runtime && wanted && !this.importing && now >= this.retryAt) {
      const generation = this.generation;
      this.narrow = frame.width < 700;
      this.importing = true;
      void import("./battle-model-runtime").then(({ BattleModelRuntime }) => {
        if (generation !== this.generation) return;
        this.runtime = new BattleModelRuntime(this.narrow);
        this.source = new CanvasSource({ resource: this.runtime.canvas, resolution: 1,
          autoDensity: false, transparent: true, autoGenerateMipmaps: false });
        this.texture = new Texture({ source: this.source, dynamic: true }); this.sprite.texture = this.texture;
      }).catch(error => {
        if (generation !== this.generation) return;
        this.close(); this.retryAt = performance.now() + 30_000;
        console.warn("Battle 3D is unavailable; retaining ship artwork and effects.", error);
      }).finally(() => { if (generation === this.generation) this.importing = false; });
    }
    if (!this.runtime || !this.source || !this.texture) return EMPTY;
    try {
      const sample = this.runtime.render(frame, now);
      if (sample.changed) {
        this.source.resize(this.runtime.canvas.width, this.runtime.canvas.height);
        this.source.update(); this.texture.update();
      }
      this.sprite.width = frame.width; this.sprite.height = frame.height;
      this.sprite.visible = wanted && (sample.ships.size > 0 || sample.bursts.size > 0 || sample.wrecks.size > 0);
      return sample;
    } catch (error) {
      this.close(); this.retryAt = now + 30_000;
      console.warn("Battle model render failed; retaining ship artwork and effects.", error);
      return EMPTY;
    }
  }

  debug() { return { importing: this.importing, runtime: this.runtime?.debug() ?? null }; }

  close(): void {
    this.generation++; this.importing = false;
    this.sprite.visible = false; this.sprite.texture = Texture.EMPTY;
    this.texture?.destroy(false); this.texture = null;
    this.source?.destroy(); this.source = null;
    this.runtime?.dispose(); this.runtime = null;
  }
}

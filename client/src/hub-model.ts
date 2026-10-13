import { CanvasSource, Sprite, Texture } from "pixi.js";
import { HUB_MODEL } from "./hub-model.generated";
import { HUB_MODEL_FADE_MS, HUB_MODEL_FIELD, HUB_MODEL_IDLE_MS, HUB_RIM, HUB_SPRITE_FIELD, hubLeanBottom,
  hubModelBlend, hubModelOnScreen, hubModelTilePx, hubModelTileSize, hubModelTilt, hubTrafficWanted } from "./hub-model-policy";
import type { HubModelRuntime } from "./hub-model-runtime";

interface Placement { flat: Sprite; x: number; y: number; tilePx: number; zoom: number }

/** The small, eager bridge for the live Market Hub. Three.js and the station
 * GLB are absent until the hub is close and on screen; the flat sprite stays
 * underneath and takes over whenever the live station is unavailable. */
export class HubModel {
  /** The live station. The renderer layers it directly above the flat sprite. */
  readonly sprite = new Sprite(Texture.EMPTY);
  private placement: Placement | null = null;
  private flat: Sprite | null = null;
  private runtime: HubModelRuntime | null = null;
  private source: CanvasSource | null = null;
  private texture: Texture | null = null;
  private importing = false;
  private wanted = false;
  private retryAt = 0;
  private idleTimer: number | null = null;
  private mix = 0;
  private desired = 0;
  private tilt = 0;
  private traffic = false;
  private trafficRuntime: HubModelRuntime | null = null;
  private lastFrame = 0;

  constructor() {
    this.sprite.anchor.set(.5);
    this.sprite.visible = false;
  }

  beginFrame(): void { this.placement = null; }

  /** The flat hub sprite (the baked station art) is drawn, centred at (x, y). */
  place(flat: Sprite, x: number, y: number, zoom: number): void {
    this.placement = { flat, x, y, tilePx: hubModelTilePx(flat.scale.x * flat.texture.width), zoom };
    this.flat = flat;
  }

  endFrame(width: number, height: number, now: number): void {
    const placed = this.placement;
    const desired = this.desired = placed ? hubModelBlend(placed.zoom) : 0;
    this.tilt = hubModelTilt(desired);
    this.wanted = !!placed && desired > 0 && hubModelOnScreen(placed.x, placed.y, placed.tilePx, width, height);
    const dt = this.lastFrame ? Math.min(now - this.lastFrame, 50) : 16;
    this.lastFrame = now;
    if (!this.wanted || !placed) {
      this.showFlat();
      if (this.runtime && this.idleTimer === null) this.idleTimer = window.setTimeout(() => this.release(), HUB_MODEL_IDLE_MS);
      return;
    }
    if (this.idleTimer !== null) { window.clearTimeout(this.idleTimer); this.idleTimer = null; }
    if (this.runtime && !this.runtime.healthy) { this.release(); this.retryAt = now + 30_000; }
    const tile = hubModelTileSize(placed.tilePx, window.devicePixelRatio, this.runtime?.tileSize);
    this.load(tile);
    if (!this.runtime?.ready || !this.source || !this.texture) { this.showFlat(); return; }
    if (tile !== this.runtime.tileSize) {
      // A closer look needs more pixels: resize, and let Pixi take the canvas afresh.
      this.runtime.resize(tile);
      this.attach(this.runtime);
    }
    // Up close, the asset's small traffic flies into the vortex.
    const traffic = hubTrafficWanted(placed.tilePx / HUB_MODEL_FIELD, this.traffic);
    if (traffic !== this.traffic || this.trafficRuntime !== this.runtime) {
      this.runtime.setTraffic(traffic);
      this.traffic = traffic;
      this.trafficRuntime = this.runtime;
    }
    try {
      if (this.runtime.render(this.tilt, now)) this.source!.update();
    } catch (error) {
      this.release(); this.retryAt = now + 30_000;
      console.warn("Hub model render failed; retaining the hub artwork.", error);
      return;
    }
    this.mix += Math.sign(desired - this.mix) * Math.min(Math.abs(desired - this.mix), dt / HUB_MODEL_FADE_MS);
    this.sprite.texture = this.texture!;
    this.sprite.position.set(placed.x, placed.y);
    this.sprite.scale.set(placed.tilePx / this.runtime.tileSize);
    this.sprite.alpha = this.mix;
    this.sprite.visible = true;
    placed.flat.alpha = 1 - this.mix;
  }

  /** How far below the hub's centre its label belongs, given the flat
   * sprite's half height: under the sprite, or under the leaning station's
   * nearer rim as the live model takes over (one frame behind is fine). */
  labelDrop(flatHalf: number): number {
    return flatHalf * (1 + this.mix * (hubLeanBottom(this.tilt) - HUB_RIM) / HUB_SPRITE_FIELD);
  }

  private showFlat(): void {
    this.mix = 0;
    this.sprite.visible = false;
    if (this.flat && !this.flat.destroyed) this.flat.alpha = 1;
  }

  private load(tileSize: number): void {
    if (this.runtime || this.importing || performance.now() < this.retryAt) return;
    this.importing = true;
    void import("./hub-model-runtime").then(({ HubModelRuntime }) => {
      if (!this.wanted) return;
      const runtime = new HubModelRuntime(HUB_MODEL.url, tileSize);
      this.runtime = runtime;
      this.attach(runtime);
    }).catch(error => {
      this.release();
      this.retryAt = performance.now() + 30_000;
      console.warn("Hub model is unavailable; retaining the hub artwork.", error);
    }).finally(() => { this.importing = false; });
  }

  /** A power-of-two tile with mipmaps: the station is often drawn well below
   * its tile size and must not shimmer. Rebuilt whenever the tile resizes. */
  private attach(runtime: HubModelRuntime): void {
    this.texture?.destroy(false);
    this.source?.destroy();
    this.source = new CanvasSource({ resource: runtime.canvas, transparent: true,
      resolution: 1, autoDensity: false, autoGenerateMipmaps: true });
    this.texture = new Texture({ source: this.source });
    if (this.sprite.visible) this.sprite.texture = this.texture;
  }

  debug() {
    return { wanted: this.wanted, desired: this.desired, mix: this.mix, traffic: this.traffic, visible: this.sprite.visible, importing: this.importing,
      runtime: this.runtime?.debug() ?? null };
  }

  /** Also used when the shell stops rendering the covered map. */
  suspend(): void { this.beginFrame(); this.wanted = false; this.release(); }

  private release(): void {
    if (this.idleTimer !== null) window.clearTimeout(this.idleTimer);
    this.idleTimer = null;
    this.showFlat();
    this.sprite.texture = Texture.EMPTY;
    this.texture?.destroy(false);
    this.texture = null;
    this.source?.destroy();
    this.source = null;
    this.runtime?.dispose();
    this.runtime = null;
    this.trafficRuntime = null;
    this.traffic = false;
  }
}

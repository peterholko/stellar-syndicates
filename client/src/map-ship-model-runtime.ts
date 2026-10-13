// Dynamically imported only for nearby visible hulls. One shared WebGL context
// renders an atlas, which Pixi composites inside the existing fleet layers.
// No DOM overlay can cover labels, intercept gestures, or escape galaxy fades.
import * as THREE from "three";
import type { ShipKind } from "./protocol";
import { disposeModel, loadShipModel, modelLighting } from "./ship-model-stage";
import { MAP_MODEL_CLOSE_SLOTS, MAP_MODEL_FRAME_PADDING, MAP_MODEL_SLOTS } from "./map-ship-model-policy";

export interface MapModelDraw {
  id: string;
  kind: ShipKind;
  slot: number;
  escorts: number;
  angle: number;
  tilt: number;
}
interface CachedHull { model: THREE.Group; used: number }
interface Instance { group: THREE.Group; yaw: THREE.Group; kind: ShipKind; escorts: number }
interface Sample { id: string; kind: ShipKind; escorts: number; angle: number }

export class MapShipModelRuntime {
  readonly canvas = document.createElement("canvas");
  readonly columns: number;
  readonly slotCount: number;
  readonly tileSize: number;
  private renderer: THREE.WebGLRenderer;
  private lighting: ReturnType<typeof modelLighting>;
  private camera = new THREE.OrthographicCamera(-1, 1, 1, -1, .01, 100);
  private hulls = new Map<ShipKind, CachedHull>();
  private loading = new Map<ShipKind, AbortController>();
  private failures = new Map<ShipKind, number>();
  private instances = new Map<string, Instance>();
  private wanted = new Set<ShipKind>();
  private samples = new Map<number, Sample>();
  private signature = "";
  private lastRender = -Infinity;
  private disposed = false;
  private lost = false;
  private frames = 0;
  private triangles = 0;
  private calls = 0;
  private textureSize: number;
  private contextLost = (event: Event) => {
    event.preventDefault();
    this.lost = true;
    this.samples.clear();
    this.cancelPending();
  };

  constructor(readonly narrow: boolean, readonly closeUp = false) {
    this.columns = closeUp ? 2 : 4;
    this.slotCount = closeUp ? MAP_MODEL_CLOSE_SLOTS : MAP_MODEL_SLOTS;
    this.tileSize = (narrow ? 384 : 768) * (closeUp ? 2 : 1);
    this.textureSize = (narrow ? 512 : 1024) * (closeUp ? 2 : 1);
    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, alpha: true,
      antialias: true, preserveDrawingBuffer: true, powerPreference: "low-power" });
    this.renderer.setPixelRatio(1);
    this.renderer.setSize(this.columns * this.tileSize, Math.ceil(this.slotCount / this.columns) * this.tileSize, false);
    this.renderer.setClearColor(0, 0);
    this.renderer.autoClear = false;
    this.renderer.info.autoReset = false;
    this.lighting = modelLighting(this.renderer);
    const field = 2.24 * MAP_MODEL_FRAME_PADDING;
    this.camera.left = this.camera.bottom = -field / 2;
    this.camera.right = this.camera.top = field / 2;
    this.camera.position.set(0, 12, 0);
    this.camera.up.set(0, 0, -1);
    this.camera.lookAt(0, 0, 0);
    this.camera.updateProjectionMatrix();
    this.canvas.addEventListener("webglcontextlost", this.contextLost);
  }

  get healthy(): boolean { return !this.lost && !this.disposed; }

  private request(kind: ShipKind, now: number): void {
    if (this.hulls.has(kind) || this.loading.has(kind) || this.loading.size >= 2
      || (this.failures.get(kind) ?? 0) > now) return;
    const abort = new AbortController();
    this.loading.set(kind, abort);
    const timeout = window.setTimeout(() => abort.abort(), 30_000);
    void loadShipModel(kind, abort.signal, this.textureSize).then(model => {
      if (!this.healthy || abort.signal.aborted || !this.wanted.has(kind)) { disposeModel(model); return; }
      this.hulls.set(kind, { model, used: performance.now() });
      this.signature = "";
    }).catch(error => {
      if (!abort.signal.aborted) {
        this.failures.set(kind, performance.now() + 30_000);
        console.warn(`Map model unavailable: ${kind}; retaining ship artwork.`, error);
      }
    }).finally(() => {
      window.clearTimeout(timeout);
      if (this.loading.get(kind) === abort) this.loading.delete(kind);
    });
  }

  private instance(item: MapModelDraw, hull: THREE.Group): Instance {
    let instance = this.instances.get(item.id);
    if (instance && (instance.kind !== item.kind || instance.escorts !== item.escorts)) {
      instance.group.removeFromParent();
      this.instances.delete(item.id);
      instance = undefined;
    }
    if (instance) return instance;
    const group = new THREE.Group(), yaw = new THREE.Group();
    group.add(yaw);
    // Match the baked formation offsets and 154/512 escort scale. Clones share
    // the cached hull's geometry/materials/textures; only the source owns them.
    for (let index = 0; index < item.escorts; index++) {
      const escort = new THREE.Group();
      escort.add(hull.clone(true));
      escort.scale.setScalar(154 / 512);
      escort.position.set(((index % 2 ? 427 : 85) / 512 - .5) * 2.24,
        0, ((105 + Math.floor(index / 2) * 105) / 512 - .5) * 2.24);
      yaw.add(escort);
    }
    yaw.add(hull.clone(true));
    this.lighting.scene.add(group);
    instance = { group, yaw, kind: item.kind, escorts: item.escorts };
    this.instances.set(item.id, instance);
    return instance;
  }

  /** Returns true only when the canvas changed and needs a Pixi texture upload.
   * Translation stays in Pixi: steady flight/panning needs no 3D rerender. */
  render(items: readonly MapModelDraw[], now: number): boolean {
    if (!this.healthy) return false;
    this.wanted = new Set(items.map(item => item.kind));
    for (const [kind, abort] of this.loading) if (!this.wanted.has(kind)) abort.abort();
    const ids = new Set(items.map(item => item.id));
    for (const [id, instance] of this.instances) if (!ids.has(id)) {
      instance.group.removeFromParent(); this.instances.delete(id);
    }
    for (const item of items) this.request(item.kind, now);
    for (const [kind, cached] of this.hulls) {
      if (this.wanted.has(kind)) cached.used = now;
      else if (now - cached.used > 15_000 || this.hulls.size >= this.slotCount) {
        disposeModel(cached.model); this.hulls.delete(kind);
      }
    }
    const ready = items.filter(item => this.hulls.has(item.kind));
    const signature = ready.map(item => `${item.slot}:${item.id}:${item.kind}:${item.escorts}:${item.angle.toFixed(3)}:${item.tilt.toFixed(3)}`).join("|");
    if (signature === this.signature || now - this.lastRender < 1000 / 30) return false;
    this.renderer.setScissorTest(false);
    this.renderer.clear();
    this.renderer.info.reset();
    for (const instance of this.instances.values()) instance.group.visible = false;
    this.samples.clear();
    for (const item of ready) {
      const instance = this.instance(item, this.hulls.get(item.kind)!.model);
      instance.group.visible = true;
      instance.group.rotation.x = item.tilt;
      // Correct the yaw for the viewing tilt, keeping the projected nose
      // exactly on the existing served heading, including diagonal flight.
      instance.yaw.rotation.y = Math.atan2(-Math.cos(item.angle), -Math.sin(item.angle) / Math.cos(item.tilt));
      const x = item.slot % this.columns * this.tileSize;
      const y = this.canvas.height - (Math.floor(item.slot / this.columns) + 1) * this.tileSize;
      this.renderer.setViewport(x, y, this.tileSize, this.tileSize);
      this.renderer.setScissor(x, y, this.tileSize, this.tileSize);
      this.renderer.setScissorTest(true);
      this.renderer.render(this.lighting.scene, this.camera);
      instance.group.visible = false;
      this.samples.set(item.slot, { id: item.id, kind: item.kind, escorts: item.escorts, angle: item.angle });
    }
    this.renderer.setScissorTest(false);
    this.signature = signature; this.lastRender = now; this.frames++;
    this.triangles = this.renderer.info.render.triangles;
    this.calls = this.renderer.info.render.calls;
    return true;
  }

  sample(item: MapModelDraw): Sample | undefined {
    const sample = this.samples.get(item.slot);
    return this.healthy && sample?.id === item.id && sample.kind === item.kind && sample.escorts === item.escorts ? sample : undefined;
  }

  cancelPending(): void {
    this.wanted.clear();
    for (const abort of this.loading.values()) abort.abort();
  }

  debug() { return { frames: this.frames, triangles: this.triangles, drawCalls: this.calls,
    closeUp: this.closeUp, slots: this.slotCount, tileSize: this.tileSize, textureSize: this.textureSize,
    cached: [...this.hulls.keys()], loading: [...this.loading.keys()], healthy: this.healthy }; }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.cancelPending();
    for (const instance of this.instances.values()) instance.group.removeFromParent();
    this.instances.clear();
    for (const cached of this.hulls.values()) disposeModel(cached.model);
    this.hulls.clear(); this.samples.clear();
    this.canvas.removeEventListener("webglcontextlost", this.contextLost);
    this.lighting.dispose();
    this.renderer.dispose(); this.renderer.forceContextLoss();
    this.canvas.width = this.canvas.height = 1;
  }
}

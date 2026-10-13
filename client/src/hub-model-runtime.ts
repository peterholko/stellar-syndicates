// Dynamically imported only while the Market Hub is close and on screen. One
// small WebGL context renders the Authority Nexus into a tile, which Pixi
// composites in place of the flat hub sprite. No DOM overlay.
import * as THREE from "three";
import type { ShipKind } from "./protocol";
import { disposeModel, loadShipModel, modelLighting } from "./ship-model-stage";
import { hubCamera, loadHubStation, type HubStation } from "./hub-model-stage";
import { HUB_MODEL_FIELD, HUB_MODEL_TEXTURE, HUB_TRAFFIC_SCALE, HUB_TRAFFIC_TEXTURE, HUB_VORTEX_STILL_TIME,
  hubModelFps } from "./hub-model-policy";
import { createWormholeTraffic, type TrafficShip, type WormholeTraffic } from "./wormhole-traffic";

/** The traffic's hulls are the game's own LOD2 models (convoy is the Medium Freighter). */
const TRAFFIC_HULLS: Record<TrafficShip, ShipKind> = {
  interceptor: "raider", "tiny-freighter": "tiny_freighter", "small-freighter": "small_freighter", "medium-freighter": "convoy",
};

export class HubModelRuntime {
  readonly canvas = document.createElement("canvas");
  private renderer: THREE.WebGLRenderer;
  private lighting: ReturnType<typeof modelLighting>;
  private camera = hubCamera(HUB_MODEL_FIELD);
  private station: HubStation | null = null;
  private loading = new AbortController();
  private signature = "";
  private lastRender = -Infinity;
  private disposed = false;
  private lost = false;
  private failed = false;
  private frames = 0;
  private triangles = 0;
  private traffic: WormholeTraffic | null = null;
  private trafficHulls: THREE.Object3D[] = [];
  private trafficWanted = false;
  private trafficLoading = false;
  private trafficFailed = false;
  private contextLost = (event: Event) => { event.preventDefault(); this.lost = true; this.loading.abort(); };

  /** The vortex starts at the baked sprite's phase and flows from there. */
  constructor(url: string, public tileSize: number) {
    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, alpha: true,
      antialias: true, preserveDrawingBuffer: true, powerPreference: "low-power" });
    this.renderer.setPixelRatio(1);
    this.renderer.setSize(tileSize, tileSize, false);
    this.renderer.setClearColor(0, 0);
    this.lighting = modelLighting(this.renderer);
    this.canvas.addEventListener("webglcontextlost", this.contextLost);
    const timeout = window.setTimeout(() => this.loading.abort(), 30_000);
    void loadHubStation(url, { signal: this.loading.signal, maxTextureSize: HUB_MODEL_TEXTURE,
      time: HUB_VORTEX_STILL_TIME }).then(station => {
      if (!this.healthy) { station.dispose(); return; }
      this.station = station;
      this.lighting.scene.add(station.root);
    }).catch(error => {
      if (this.disposed) return;
      this.failed = true;
      console.warn("Hub model unavailable; retaining the hub artwork.", error);
    }).finally(() => window.clearTimeout(timeout));
  }

  get healthy(): boolean { return !this.lost && !this.disposed && !this.failed; }
  get ready(): boolean { return this.healthy && !!this.station; }

  /** Redraw at a new tile size on the next render, which is not rate-limited:
   * the resized canvas is blank until then. */
  resize(tileSize: number): void {
    if (tileSize === this.tileSize || this.disposed) return;
    this.tileSize = tileSize;
    this.renderer.setSize(tileSize, tileSize, false);
    this.signature = "";
    this.lastRender = -Infinity;
  }

  /** Show or hide the decorative traffic. Its four hulls load the first time
   * it is wanted (often from the browser cache: the map's live hulls share them). */
  setTraffic(on: boolean): void {
    this.trafficWanted = on;
    if (this.traffic) { this.traffic.setEnabled(on); this.signature = ""; return; }
    if (!on || this.trafficLoading || this.trafficFailed || !this.ready) return;
    this.trafficLoading = true;
    const types = Object.keys(TRAFFIC_HULLS) as TrafficShip[];
    void Promise.allSettled(types.map(type => loadShipModel(TRAFFIC_HULLS[type], this.loading.signal, HUB_TRAFFIC_TEXTURE)))
      .then(results => {
        const hulls = results.flatMap(result => result.status === "fulfilled" ? [result.value] : []);
        const failure = results.find(result => result.status === "rejected");
        if (failure || !this.station || this.disposed) {
          hulls.forEach(disposeModel);
          if (failure && !this.disposed) {
            this.trafficFailed = true;
            console.warn("Hub traffic unavailable.", failure.reason);
          }
          return;
        }
        this.trafficHulls = hulls;
        const templates = {} as Record<TrafficShip, THREE.Object3D>;
        types.forEach((type, i) => { templates[type] = hulls[i]; });
        this.traffic = createWormholeTraffic(templates, { enabled: this.trafficWanted, scale: HUB_TRAFFIC_SCALE });
        this.station.local.add(this.traffic.group);
        this.signature = "";
      }).finally(() => { this.trafficLoading = false; });
  }

  /** Returns true only when the canvas changed and needs a Pixi texture upload.
   * Panning and zoom stay in Pixi; only the lean and the flowing vortex redraw. */
  render(tilt: number, now: number): boolean {
    const station = this.station;
    if (!this.ready || !station) return false;
    if (now - this.lastRender < 1000 / hubModelFps(this.tileSize) - 1) return false;
    if (this.lastRender > -Infinity) {
      const dt = (now - this.lastRender) / 1000;
      if (station.flow.active) station.flow.update(dt);
      if (this.traffic?.active) this.traffic.update(dt);
    }
    const signature = `${tilt.toFixed(4)}:${station.flow.elapsed.toFixed(4)}:${this.traffic?.active ? now : ""}`;
    if (signature === this.signature) return false;
    station.pose.rotation.x = tilt;
    this.renderer.render(this.lighting.scene, this.camera);
    this.signature = signature;
    this.lastRender = now;
    this.frames++;
    this.triangles = this.renderer.info.render.triangles;
    return true;
  }

  debug() {
    return { ready: this.ready, healthy: this.healthy, frames: this.frames, triangles: this.triangles,
      tileSize: this.tileSize, flowing: this.station?.flow.active ?? false,
      elapsed: this.station?.flow.elapsed ?? 0,
      traffic: { wanted: this.trafficWanted, loaded: !!this.traffic, failed: this.trafficFailed,
        ships: this.traffic?.snapshot().filter(ship => ship.visible).length ?? 0 } };
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.loading.abort();
    // Traffic first: its cloned materials share the hulls' textures, and its
    // group leaves the station before the station frees its own assets.
    this.traffic?.dispose();
    this.traffic = null;
    this.trafficHulls.forEach(disposeModel);
    this.trafficHulls = [];
    this.station?.dispose();
    this.station = null;
    this.canvas.removeEventListener("webglcontextlost", this.contextLost);
    this.lighting.dispose();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
    this.canvas.width = this.canvas.height = 1;
  }
}

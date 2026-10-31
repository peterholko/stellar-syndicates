import * as THREE from "three";
import type { ShipArtKind } from "./shipart";
import { disposeModel, loadShipModel, modelLighting } from "./ship-model-stage";
import type { YardScene } from "./shipyard-scene";

/** The weld line's width, as a share of the hull's normalized length (2). */
const WELD_WIDTH = 0.035;
const FOV = 30;
const UP = new THREE.Vector3(0, 1, 0);

/** The hull under construction in its yard, rendered on demand over the
 * plate (§shipyard-view): built from the stern forward, the built part solid,
 * the rest a blueprint, a weld line at the cut. The camera never moves. */
export class ConstructionScene {
  private readonly renderer: THREE.WebGLRenderer;
  private readonly lighting: ReturnType<typeof modelLighting>;
  private readonly camera = new THREE.PerspectiveCamera(FOV, 16 / 9, 0.01, 200);
  private readonly resize: ResizeObserver;
  private readonly events = new AbortController();
  /** The hull's pose: its heading, and its keel on the deck. In rig space the
   * hull runs nose (−z) to tail (+z) over z ∈ [−1, 1]. */
  private readonly rig = new THREE.Group();
  private readonly work = new THREE.Group();
  private readonly ghostFill = new THREE.MeshBasicMaterial({ color: 0x4fb8e8, transparent: true, opacity: 0.07, depthWrite: false });
  private readonly ghostWire = new THREE.MeshBasicMaterial({ color: 0x7fd6ff, wireframe: true, transparent: true, opacity: 0.2, depthWrite: false });
  private readonly weldMaterial = new THREE.MeshBasicMaterial({ color: 0xffb347, side: THREE.DoubleSide });
  private readonly built = new THREE.Plane();
  private readonly ghost = new THREE.Plane();
  private readonly weldLow = new THREE.Plane();
  private readonly weldHigh = new THREE.Plane();
  private shadow: THREE.Mesh | null = null;
  private warmLight: THREE.DirectionalLight | null = null;
  private request: AbortController | null = null;
  private model: THREE.Group | null = null;
  private copies: THREE.Object3D[] = [];
  private vertices: THREE.Vector3[] = [];
  private kind: ShipArtKind | null = null;
  private scene: YardScene | null = null;
  private fill = 0.5;
  /** 0–1 built; null while the hull waits for a slip (all blueprint). */
  private progress: number | null = 0;
  private frame = 0;
  private visible = true;
  private disposed = false;
  private lost = false;

  constructor(private readonly canvas: HTMLCanvasElement,
    private readonly status: (state: "loading" | "ready" | "error") => void) {
    this.renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "low-power" });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
    this.renderer.setClearColor(0, 0);
    this.renderer.localClippingEnabled = true;
    this.lighting = modelLighting(this.renderer);
    this.lighting.scene.add(this.rig);
    this.ghostFill.clippingPlanes = this.ghostWire.clippingPlanes = [this.ghost];
    this.weldMaterial.clippingPlanes = [this.weldLow, this.weldHigh];
    this.resize = new ResizeObserver(this.render);
    this.resize.observe(canvas);
    canvas.addEventListener("webglcontextlost", event => {
      event.preventDefault();
      this.lost = true;
      this.request?.abort();
      this.status("error");
    }, { signal: this.events.signal });
    document.addEventListener("visibilitychange", this.render, { signal: this.events.signal });
  }

  /** Stand `kind` in `scene`, spanning `fill` of the plate's width. */
  async show(kind: ShipArtKind, scene: YardScene, fill: number): Promise<void> {
    if (this.disposed) return;
    const sameKind = kind === this.kind && (this.model || this.request);
    this.scene = scene;
    this.fill = fill;
    this.dressScene();
    if (sameKind) { this.frameHull(); this.render(); return; }
    this.kind = kind;
    this.request?.abort();
    const request = new AbortController();
    this.request = request;
    const timeout = window.setTimeout(() => request.abort(new Error("Model load timed out")), 30_000);
    this.clearHull();
    this.status("loading");
    try {
      const model = await loadShipModel(kind, request.signal);
      if (this.disposed || this.request !== request) { disposeModel(model); return; }
      this.addHull(model);
      this.status("ready");
      this.render();
    } catch (error) {
      if (!this.disposed && this.request === request) {
        this.status("error");
        console.warn(`Could not load ${kind} model`, error);
      }
    } finally {
      window.clearTimeout(timeout);
      if (this.request === request) this.request = null;
    }
  }

  /** 0–1 of the hull built, or null while it waits for a slip. */
  setProgress(progress: number | null): void {
    const next = progress === null ? null : Math.max(0, Math.min(1, progress));
    if (next === this.progress) return;
    this.progress = next;
    this.cut();
    this.render();
  }

  setVisible(visible: boolean): void {
    this.visible = visible;
    if (visible) this.render();
  }

  private addHull(model: THREE.Group): void {
    this.model = model;
    // Built: the hull's own materials, clipped at the cut.
    model.traverse(node => {
      if (!(node as THREE.Mesh).isMesh) return;
      const mesh = node as THREE.Mesh;
      for (const material of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) material.clippingPlanes = [this.built];
    });
    // The blueprint and the weld share its geometry, with their own materials.
    const copy = (material: THREE.Material): THREE.Object3D => {
      const clone = model.clone(true);
      clone.traverse(node => { if ((node as THREE.Mesh).isMesh) (node as THREE.Mesh).material = material; });
      return clone;
    };
    this.copies = [copy(this.ghostFill), copy(this.ghostWire), copy(this.weldMaterial)];
    this.work.add(model, ...this.copies);
    if (!this.work.parent) this.rig.add(this.work);
    // The hull's vertices in rig space (the work group is the rig's identity),
    // for framing and the deck under its keel.
    this.vertices = [];
    this.work.updateMatrixWorld(true);
    const toRig = this.work.matrixWorld.clone().invert();
    model.traverse(node => {
      const position = (node as THREE.Mesh).isMesh ? (node as THREE.Mesh).geometry.getAttribute("position") : null;
      if (!position) return;
      for (let i = 0; i < position.count; i++) {
        this.vertices.push(new THREE.Vector3().fromBufferAttribute(position, i).applyMatrix4(node.matrixWorld).applyMatrix4(toRig));
      }
    });
    this.frameHull();
  }

  private clearHull(): void {
    for (const copy of this.copies) copy.removeFromParent();
    this.copies = [];
    if (this.model) disposeModel(this.model);
    this.model = null;
    this.vertices = [];
  }

  /** The yard's light and deck. */
  private dressScene(): void {
    const scene = this.scene!;
    if (!this.warmLight) {
      this.warmLight = new THREE.DirectionalLight("#ffb265", 0);
      this.warmLight.position.set(-6, 10, 6);
      this.lighting.scene.add(this.warmLight);
    }
    this.warmLight.intensity = 2.2 * scene.warm;
    this.renderer.toneMappingExposure = 1.12 - 0.25 * scene.warm;
    if (scene.floor && !this.shadow) {
      // A soft contact shadow on the deck: dark under the keel, gone by the rim.
      this.shadow = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), new THREE.ShaderMaterial({
        transparent: true, depthWrite: false,
        vertexShader: "varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }",
        fragmentShader: "varying vec2 vUv; void main() { float d = length(vUv - 0.5) * 2.0; gl_FragColor = vec4(0.0, 0.0, 0.0, 0.85 * (1.0 - smoothstep(0.15, 1.0, d))); }",
      }));
      this.shadow.rotation.x = -Math.PI / 2;
      this.rig.add(this.shadow);
    }
    if (this.shadow) this.shadow.visible = scene.floor;
  }

  /** Pose the hull and set the camera so it spans its fill of the plate. The
   * canvas is centred on the plate's (cx, cy), so the hull's centre is the
   * view's centre. */
  private frameHull(): void {
    const scene = this.scene;
    if (!scene || !this.model) return;
    this.rig.rotation.set(0, scene.heading, 0);
    // The deck under the keel, as wide as the hull and a little longer.
    if (this.shadow) {
      let minX = Infinity, maxX = -Infinity, minY = Infinity;
      for (const v of this.vertices) { minX = Math.min(minX, v.x); maxX = Math.max(maxX, v.x); minY = Math.min(minY, v.y); }
      this.shadow.scale.set(Math.max(0.4, maxX - minX) * 1.3, 2.35, 1);
      this.shadow.position.set(0, minY - 0.002, 0);
    }
    const direction = new THREE.Vector3(0, Math.sin(scene.elevation), Math.cos(scene.elevation));
    this.camera.aspect = 16 / 9;
    let distance = 8;
    for (let pass = 0; pass < 3; pass++) {
      this.place(direction, distance);
      const { length, width, height } = this.measure();
      distance *= Math.max(length / this.fill, width / scene.maxW, height / scene.maxH);
    }
    this.place(direction, distance);
    this.cut();
  }

  private place(direction: THREE.Vector3, distance: number): void {
    this.camera.position.copy(direction).multiplyScalar(distance);
    this.camera.up.copy(UP);
    this.camera.lookAt(0, 0, 0);
    this.camera.updateProjectionMatrix();
    this.camera.updateMatrixWorld(true);
    this.rig.updateMatrixWorld(true);
  }

  /** The hull's nose-to-tail span and its extent on the plate, as shares of
   * the plate's width and height. */
  private measure(): { length: number; width: number; height: number } {
    const project = (p: THREE.Vector3) => p.clone().applyMatrix4(this.rig.matrixWorld).project(this.camera);
    const nose = project(new THREE.Vector3(0, 0, -1));
    const tail = project(new THREE.Vector3(0, 0, 1));
    const length = Math.hypot((nose.x - tail.x) / 2, (nose.y - tail.y) / 2 / this.camera.aspect);
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    for (const vertex of this.vertices) {
      const p = project(vertex);
      minX = Math.min(minX, p.x); maxX = Math.max(maxX, p.x);
      minY = Math.min(minY, p.y); maxY = Math.max(maxY, p.y);
    }
    return { length, width: (maxX - minX) / 2, height: (maxY - minY) / 2 };
  }

  /** Move the cut to the progress: built behind it, blueprint ahead, the weld
   * just ahead of it. The planes live in rig space, where the hull spans z ∈ [−1, 1]. */
  private cut(): void {
    const progress = this.progress ?? 0;
    const z = 1 - 2 * progress;
    const matrix = this.rig.matrixWorld;
    this.built.set(new THREE.Vector3(0, 0, 1), -z).applyMatrix4(matrix);
    this.ghost.set(new THREE.Vector3(0, 0, -1), z).applyMatrix4(matrix);
    this.weldLow.set(new THREE.Vector3(0, 0, 1), -(z - WELD_WIDTH)).applyMatrix4(matrix);
    this.weldHigh.set(new THREE.Vector3(0, 0, -1), z).applyMatrix4(matrix);
    const [fill, wire, weld] = this.copies;
    if (fill) fill.visible = wire.visible = progress < 1;
    if (weld) weld.visible = this.progress !== null && progress > 0 && progress < 1;
  }

  private render = (): void => {
    if (this.frame || this.disposed || this.lost || !this.visible || document.hidden) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      if (this.disposed || this.lost || !this.visible || document.hidden) return;
      const { width, height } = this.canvas.getBoundingClientRect();
      if (!width || !height) return;
      const size = this.renderer.getSize(new THREE.Vector2());
      if (size.x !== Math.floor(width) || size.y !== Math.floor(height)) this.renderer.setSize(Math.floor(width), Math.floor(height), false);
      this.renderer.render(this.lighting.scene, this.camera);
      this.canvas.dataset.frames = String(Number(this.canvas.dataset.frames ?? 0) + 1);
    });
  };

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.request?.abort();
    cancelAnimationFrame(this.frame);
    this.events.abort();
    this.resize.disconnect();
    this.clearHull();
    if (this.shadow) {
      (this.shadow.material as THREE.Material).dispose();
      this.shadow.geometry.dispose();
    }
    this.ghostFill.dispose();
    this.ghostWire.dispose();
    this.weldMaterial.dispose();
    this.lighting.dispose();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
  }
}

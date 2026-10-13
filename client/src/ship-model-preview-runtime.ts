import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { ShipKind } from "./protocol";
import { disposeModel, loadShipModel, modelLighting } from "./ship-model-stage";

/** How far toward the frame's edges the framed hull may reach. */
const FRAME_FILL = .84;
/** The showcase sway: either side of the framed angle, and one full cycle. It
 * never swings nose-on, so the hull can be framed large. */
const SWAY_RADIANS = .5;
const SWAY_PERIOD_S = 24;
const Y_AXIS = new THREE.Vector3(0, 1, 0);

/** The model's vertices in its world frame, for framing. */
function modelVertices(model: THREE.Object3D): THREE.Vector3[] {
  const vertices: THREE.Vector3[] = [];
  model.updateMatrixWorld(true);
  model.traverse(node => {
    const position = (node as THREE.Mesh).isMesh ? (node as THREE.Mesh).geometry.getAttribute("position") : null;
    if (!position) return;
    for (let i = 0; i < position.count; i++) vertices.push(new THREE.Vector3().fromBufferAttribute(position, i).applyMatrix4(node.matrixWorld));
  });
  return vertices;
}

/** One on-demand renderer for the selected hull. No animation loop, global
 * observers or GPU model cache survive a closed construction panel. */
export class ModelPreview {
  private readonly renderer: THREE.WebGLRenderer;
  private readonly lighting: ReturnType<typeof modelLighting>;
  private readonly camera = new THREE.PerspectiveCamera(34, 1, .01, 100);
  private readonly controls: OrbitControls;
  private readonly resize: ResizeObserver;
  private readonly events = new AbortController();
  private request: AbortController | null = null;
  private model: THREE.Group | null = null;
  private vertices: THREE.Vector3[] = [];
  private kind: ShipKind | null = null;
  private radius = 1.5;
  private frame = 0;
  private visible = true;
  private disposed = false;
  private lost = false;
  /** Slow showcase sway; a drag, a wheel zoom, an arrow key or a view button ends it. */
  private turntable = false;
  private spinFrame = 0;
  private spinLast = 0;
  private swayPhase = 0;
  private swayAngle = 0;

  constructor(private readonly canvas: HTMLCanvasElement,
    private readonly status: (state: "loading" | "ready" | "error") => void) {
    this.renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, powerPreference: "low-power" });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
    this.renderer.setClearColor(0, 0);
    this.lighting = modelLighting(this.renderer);
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enablePan = false;
    this.controls.enableDamping = false;
    this.controls.addEventListener("change", this.render);
    this.controls.addEventListener("start", () => this.setTurntable(false));
    this.resize = new ResizeObserver(this.render);
    this.resize.observe(canvas.parentElement!);
    canvas.addEventListener("keydown", event => this.keydown(event), { signal: this.events.signal });
    canvas.addEventListener("webglcontextlost", event => {
      event.preventDefault();
      this.lost = true;
      this.request?.abort();
      this.status("error");
    }, { signal: this.events.signal });
    document.addEventListener("visibilitychange", this.render, { signal: this.events.signal });
  }

  async setKind(kind: ShipKind): Promise<void> {
    if (this.disposed || (kind === this.kind && (this.model || this.request))) return;
    this.kind = kind;
    this.request?.abort();
    const request = new AbortController();
    this.request = request;
    const timeout = window.setTimeout(() => request.abort(new Error("Model load timed out")), 30_000);
    if (this.model) { disposeModel(this.model); this.model = null; this.vertices = []; }
    this.status("loading");
    try {
      const model = await loadShipModel(kind, request.signal);
      if (this.disposed || this.request !== request) { disposeModel(model); return; }
      this.model = model;
      this.lighting.scene.add(model);
      this.vertices = modelVertices(model);
      this.radius = new THREE.Box3().setFromObject(model).getBoundingSphere(new THREE.Sphere()).radius;
      this.controls.minDistance = this.radius * 1.25;
      this.controls.maxDistance = this.radius * 10;
      this.canvas.dataset.kind = kind;
      this.setView("orbit");
      this.status("ready");
      this.spin();
    } catch (error) {
      // A superseded request is silent. A current timeout/network failure keeps
      // its poster and build controls usable, and offers an explicit retry.
      if (!this.disposed && this.request === request) {
        this.status("error");
        console.warn(`Could not load ${kind} model`, error);
      }
    } finally {
      window.clearTimeout(timeout);
      if (this.request === request) this.request = null;
    }
  }

  setVisible(visible: boolean): void {
    this.visible = visible;
    if (visible) this.render();
    this.spin();
  }

  /** Sway the camera slowly either side of the framed angle. */
  setTurntable(on: boolean): void {
    if (on && !this.turntable) this.swayPhase = this.swayAngle = 0;
    this.turntable = on;
    if (!on) {
      cancelAnimationFrame(this.spinFrame);
      this.spinFrame = 0;
    }
    this.spin();
  }

  private spin(): void {
    if (this.spinFrame || !this.turntable || this.disposed || this.lost || !this.visible || !this.model) return;
    this.spinLast = performance.now();
    const step = (now: number): void => {
      this.spinFrame = 0;
      if (!this.turntable || this.disposed || this.lost || !this.visible) return;
      const dt = Math.min(0.1, Math.max(0, (now - this.spinLast) / 1000));
      this.spinLast = now;
      if (!document.hidden && this.model) {
        // Step by the change in angle, so a zoom during the sway holds.
        this.swayPhase += dt * Math.PI * 2 / SWAY_PERIOD_S;
        const angle = SWAY_RADIANS * Math.sin(this.swayPhase);
        this.camera.position.applyAxisAngle(Y_AXIS, angle - this.swayAngle);
        this.swayAngle = angle;
        this.camera.lookAt(this.controls.target);
        this.render();
      }
      this.spinFrame = requestAnimationFrame(step);
    };
    this.spinFrame = requestAnimationFrame(step);
  }

  setView(view: string): void {
    const directions: Record<string, [number, number, number]> = {
      orbit: [8, 6, -10], top: [0, 1, -.0001], bottom: [0, -1, .0001],
      side: [1, 0, 0], front: [0, 0, -1], back: [0, 0, 1],
    };
    const rect = this.canvas.parentElement!.getBoundingClientRect();
    const aspect = Math.max(.3, rect.width / Math.max(1, rect.height));
    const fov = THREE.MathUtils.degToRad(this.camera.fov) / 2;
    const distance = this.radius / Math.sin(Math.atan(Math.tan(fov) * Math.min(1, aspect))) * 1.08;
    this.camera.position.set(...(directions[view] ?? directions.orbit)).normalize().multiplyScalar(distance);
    this.camera.up.set(0, 1, 0);
    this.controls.target.set(0, 0, 0);
    this.camera.aspect = aspect;
    this.camera.updateProjectionMatrix();
    this.camera.lookAt(0, 0, 0);
    // Fit the hull's own vertices, not its sphere or box: a long, flat hull
    // otherwise looks tiny inside a wide panel.
    if (this.model) {
      const direction = this.camera.position.clone().normalize();
      const fit = THREE.MathUtils.clamp(this.fitDistance(direction, aspect), this.controls.minDistance, this.controls.maxDistance);
      this.camera.position.copy(direction).multiplyScalar(fit);
      this.camera.lookAt(0, 0, 0);
    }
    this.swayPhase = this.swayAngle = 0;
    this.controls.update();
    this.render();
  }

  /** The nearest distance along `direction` (target → camera) at which every
   * vertex stays within FRAME_FILL of the frame's edges, at every bearing the
   * sway passes through. Exact per vertex: for a vertex p, the depth
   * d − p·back must cover |p·right| and |p·up| at the frame's slope. */
  private fitDistance(direction: THREE.Vector3, aspect: number): number {
    const slope = Math.tan(THREE.MathUtils.degToRad(this.camera.fov) / 2) * FRAME_FILL;
    const back = new THREE.Vector3(), right = new THREE.Vector3(), up = new THREE.Vector3();
    const bearings = this.turntable ? 9 : 1;
    let fit = 0;
    for (let bearing = 0; bearing < bearings; bearing++) {
      const swing = bearings === 1 ? 0 : SWAY_RADIANS * (2 * bearing / (bearings - 1) - 1);
      back.copy(direction).applyAxisAngle(Y_AXIS, swing);
      right.crossVectors(Y_AXIS, back).normalize();
      up.crossVectors(back, right);
      for (const p of this.vertices) {
        fit = Math.max(fit, p.dot(back) + Math.max(Math.abs(p.dot(right)) / (slope * aspect), Math.abs(p.dot(up)) / slope));
      }
    }
    return fit;
  }

  zoom(factor: number): void {
    const distance = THREE.MathUtils.clamp(this.camera.position.length() * factor, this.controls.minDistance, this.controls.maxDistance);
    this.camera.position.setLength(distance);
    this.controls.update();
    this.render();
  }

  private keydown(event: KeyboardEvent): void {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "+", "=", "-", "Home"].includes(event.key)) return;
    event.preventDefault(); event.stopPropagation();
    if (["+", "=", "-"].includes(event.key)) { this.zoom(event.key === "-" ? 1.2 : .8); return; }
    this.setTurntable(false);
    if (event.key === "Home") { this.setView("orbit"); return; }
    const position = new THREE.Spherical().setFromVector3(this.camera.position);
    if (event.key === "ArrowLeft") position.theta -= .16;
    if (event.key === "ArrowRight") position.theta += .16;
    if (event.key === "ArrowUp") position.phi -= .16;
    if (event.key === "ArrowDown") position.phi += .16;
    position.makeSafe();
    this.camera.position.setFromSpherical(position);
    this.controls.update();
    this.render();
  }

  private render = (): void => {
    if (this.frame || this.disposed || this.lost || !this.visible || document.hidden) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      if (this.disposed || this.lost || !this.visible || document.hidden) return;
      const { width, height } = this.canvas.parentElement!.getBoundingClientRect();
      if (!width || !height) return;
      const size = this.renderer.getSize(new THREE.Vector2());
      if (size.x !== Math.floor(width) || size.y !== Math.floor(height)) this.renderer.setSize(Math.floor(width), Math.floor(height), false);
      this.camera.aspect = width / height;
      this.camera.updateProjectionMatrix();
      this.renderer.render(this.lighting.scene, this.camera);
      this.canvas.dataset.triangles = String(this.renderer.info.render.triangles);
      this.canvas.dataset.drawCalls = String(this.renderer.info.render.calls);
      this.canvas.dataset.frames = String(Number(this.canvas.dataset.frames ?? 0) + 1);
      this.canvas.dataset.camera = this.camera.position.toArray().map(v => v.toFixed(3)).join(",");
    });
  };

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    cancelAnimationFrame(this.spinFrame);
    this.request?.abort();
    cancelAnimationFrame(this.frame);
    this.events.abort();
    this.resize.disconnect();
    this.controls.dispose();
    if (this.model) disposeModel(this.model);
    this.lighting.dispose();
    this.renderer.dispose();
    this.renderer.forceContextLoss();
  }
}

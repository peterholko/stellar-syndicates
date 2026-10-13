// Loaded only while a participant battle is open. A single transparent Three
// surface joins the theater's Pixi layers; no extra input surface or simulation.
import * as THREE from "three";
import type { ShipKind } from "./protocol";
import { disposeModel, loadShipModel, modelLighting } from "./ship-model-stage";
import { BattleHullChunks, BattleInstancedHull, instanceOpacity } from "./battle-model-geometry";
import { battleModelBudget, pickBattleBursts, pickBattleModels, pickBattleWrecks, type BattleModelFrame } from "./battle-model-policy";
import { WRECK_PIECES, wreckCuts } from "./battlewrecks";
import { visitBattleBurst, type BattleParticle, type BattleParticleKind } from "./battle-model-effects";

class ParticlePool {
  readonly mesh: THREE.InstancedMesh;
  private opacity: THREE.InstancedBufferAttribute;
  private pose = new THREE.Object3D();
  private color = new THREE.Color();
  constructor(geometry: THREE.BufferGeometry, material: THREE.Material, readonly capacity: number) {
    this.opacity = new THREE.InstancedBufferAttribute(new Float32Array(capacity), 1).setUsage(THREE.DynamicDrawUsage);
    geometry.setAttribute("battleOpacity", this.opacity);
    if (!(material instanceof THREE.ShaderMaterial)) instanceOpacity(material);
    material.depthWrite = false;
    this.mesh = new THREE.InstancedMesh(geometry, material, capacity);
    this.mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    this.mesh.setColorAt(0, this.color); // allocate before the first shader compilation
    this.mesh.instanceColor!.setUsage(THREE.DynamicDrawUsage);
    this.mesh.frustumCulled = false;
    this.mesh.count = 0;
  }
  add(p: BattleParticle, width: number, height: number, depthSlope: number): void {
    const index = this.mesh.count;
    if (index >= this.capacity) return;
    this.pose.position.set(p.x - width / 2, p.height + (p.y - height / 2) * depthSlope, p.y - height / 2);
    this.pose.rotation.set(p.kind === "glow" ? -Math.PI / 2 : p.rx, p.ry, p.rz);
    this.pose.scale.set(p.sx, p.sy, p.sz); this.pose.updateMatrix();
    this.mesh.setMatrixAt(index, this.pose.matrix);
    this.mesh.setColorAt(index, this.color.setHex(p.color));
    this.opacity.setX(index, p.alpha);
    this.mesh.count++;
  }
  finish(): void {
    this.mesh.visible = this.mesh.count > 0;
    this.mesh.instanceMatrix.needsUpdate = true;
    this.mesh.instanceColor!.needsUpdate = true;
    this.opacity.needsUpdate = true;
  }
  dispose(): void {
    this.mesh.removeFromParent(); this.mesh.dispose();
    this.mesh.geometry.dispose(); (this.mesh.material as THREE.Material).dispose();
  }
}

function glowMaterial(): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({ transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    toneMapped: false,
    vertexShader: `attribute float battleOpacity;
      varying vec2 vUv; varying vec3 vTint; varying float vAlpha;
      void main() { vUv = uv; vTint = instanceColor; vAlpha = battleOpacity;
        gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0); }`,
    fragmentShader: `varying vec2 vUv; varying vec3 vTint; varying float vAlpha;
      void main() { float radius = length(vUv * 2.0 - 1.0);
        float alpha = pow(max(0.0, 1.0 - radius), 2.6) * vAlpha;
        gl_FragColor = vec4(vTint, alpha);
        #include <colorspace_fragment>
      }`,
  });
}

interface CachedHull { source: THREE.Group; instances: BattleInstancedHull; used: number; chunks?: BattleHullChunks }
export interface BattleModelSample { ships: ReadonlySet<number>; bursts: ReadonlySet<string>; wrecks: ReadonlySet<string>; changed: boolean }

export class BattleModelRuntime {
  readonly canvas = document.createElement("canvas");
  private renderer: THREE.WebGLRenderer;
  private lighting: ReturnType<typeof modelLighting>;
  private camera = new THREE.OrthographicCamera(-1, 1, 1, -1, .1, 4000);
  private cache = new Map<ShipKind, CachedHull>();
  private loading = new Map<ShipKind, AbortController>();
  private failures = new Map<ShipKind, number>();
  private wanted = new Set<ShipKind>();
  private pools: Record<BattleParticleKind, ParticlePool>;
  private flashes = Array.from({ length: 4 }, () => new THREE.PointLight(0xffaa66, 0, 500, 2));
  private disposed = false;
  private lost = false;
  private width = 0;
  private height = 0;
  private signature = "";
  private shown: BattleModelSample = { ships: new Set(), bursts: new Set(), wrecks: new Set(), changed: false };
  private frames = 0;
  private contextLost = (event: Event) => { event.preventDefault(); this.lost = true; this.cancelPending(); };

  constructor(private narrow: boolean) {
    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, alpha: true, antialias: true,
      preserveDrawingBuffer: true, powerPreference: "low-power" });
    this.renderer.debug.onShaderError = () => { throw new Error("Battle 3D shader compilation failed"); };
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, narrow ? 1 : 1.5));
    this.renderer.setClearColor(0, 0);
    try { this.lighting = modelLighting(this.renderer); }
    catch (error) { this.renderer.dispose(); this.renderer.forceContextLoss(); throw error; }
    this.camera.position.set(0, 2000, 0); this.camera.up.set(0, 0, -1); this.camera.lookAt(0, 0, 0);
    this.pools = {
      // Two subdivisions keep a big capital fireball round when zoomed in.
      fire: new ParticlePool(new THREE.IcosahedronGeometry(1, narrow ? 1 : 2), new THREE.MeshStandardMaterial({
        color: 0xffffff, emissive: 0xff4a0f, emissiveIntensity: 1.1, roughness: .8 }), 192),
      smoke: new ParticlePool(new THREE.IcosahedronGeometry(1, narrow ? 1 : 2), new THREE.MeshStandardMaterial({ roughness: 1 }), 112),
      shard: new ParticlePool(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial({
        color: 0xffffff, metalness: .65, roughness: .6 }), 320),
      ring: new ParticlePool(new THREE.TorusGeometry(1, .012, 4, 64), new THREE.MeshBasicMaterial({
        color: 0xffffff, blending: THREE.AdditiveBlending }), 16),
      glow: new ParticlePool(new THREE.PlaneGeometry(2, 2), glowMaterial(), 16),
    };
    // Smoke is behind solid hulls; additive bloom follows all other surfaces.
    this.pools.smoke.mesh.renderOrder = -1;
    this.pools.fire.mesh.renderOrder = 1; this.pools.shard.mesh.renderOrder = 2;
    this.pools.ring.mesh.renderOrder = 3; this.pools.glow.mesh.renderOrder = 4;
    for (const pool of Object.values(this.pools)) this.lighting.scene.add(pool.mesh);
    for (const flash of this.flashes) this.lighting.scene.add(flash);
    this.canvas.addEventListener("webglcontextlost", this.contextLost);
  }

  get healthy(): boolean { return !this.disposed && !this.lost; }

  private request(kind: ShipKind, now: number): void {
    if (this.cache.has(kind) || this.loading.has(kind) || this.loading.size >= 2 || (this.failures.get(kind) ?? 0) > now) return;
    const abort = new AbortController(); this.loading.set(kind, abort);
    const timer = window.setTimeout(() => abort.abort(), 30_000);
    void loadShipModel(kind, abort.signal, battleModelBudget(this.narrow, 0).textureSize).then(source => {
      if (!this.healthy || abort.signal.aborted || !this.wanted.has(kind)) { disposeModel(source); return; }
      try {
        const instances = new BattleInstancedHull(source);
        this.lighting.scene.add(instances.group);
        this.cache.set(kind, { source, instances, used: performance.now() });
        this.signature = "";
      } catch (error) { disposeModel(source); throw error; }
    }).catch(error => {
      if (!abort.signal.aborted) {
        this.failures.set(kind, performance.now() + 30_000);
        console.warn(`Battle model unavailable: ${kind}; retaining ship artwork.`, error);
      }
    }).finally(() => {
      window.clearTimeout(timer);
      if (this.loading.get(kind) === abort) this.loading.delete(kind);
    });
  }

  render(frame: BattleModelFrame, now: number): BattleModelSample {
    if (!this.healthy) return { ships: new Set(), bursts: new Set(), wrecks: new Set(), changed: false };
    const picked = pickBattleModels(frame, this.narrow), bursts = pickBattleBursts(frame, this.narrow);
    const wrecks = pickBattleWrecks(frame, this.narrow).filter(w => (WRECK_PIECES[w.kind] ?? 0) >= 2);
    // A wreck keeps its hull type loaded (the living ship loaded it already).
    this.wanted = new Set([...picked.map(s => s.kind), ...wrecks.map(w => w.kind)]);
    for (const [kind, abort] of this.loading) if (!this.wanted.has(kind)) abort.abort();
    // Dispose unused kinds before requesting replacements: texture memory is
    // bounded even while panning between completely different fleets.
    for (const [kind, hull] of this.cache) {
      if (this.wanted.has(kind)) hull.used = now;
      else if (now - hull.used > 10_000 || this.cache.size + [...this.wanted].filter(k => !this.cache.has(k)).length
        > battleModelBudget(this.narrow, frame.tier).kinds) {
        hull.instances.dispose(); hull.chunks?.dispose(); disposeModel(hull.source); this.cache.delete(kind);
      }
    }
    for (const kind of this.wanted) this.request(kind, now);
    const ships = picked.filter(ship => this.cache.has(ship.kind));
    const sections = wrecks.filter(w => this.cache.has(w.kind));
    const depthSlope = frame.depthSlope ?? 0;
    const signature = JSON.stringify([frame.width, frame.height, frame.reducedMotion, frame.pitch, frame.ringTilt,
      depthSlope, ships, bursts, sections]);
    if (signature === this.signature) return { ...this.shown, changed: false };
    if (frame.width !== this.width || frame.height !== this.height) {
      this.width = frame.width; this.height = frame.height;
      this.renderer.setSize(this.width, this.height, false);
      this.camera.left = -this.width / 2; this.camera.right = this.width / 2;
      this.camera.top = this.height / 2; this.camera.bottom = -this.height / 2;
      this.camera.updateProjectionMatrix();
    }
    for (const [kind, hull] of this.cache) hull.instances.update(ships.filter(s => s.kind === kind), this.width, this.height, frame.pitch,
      depthSlope);
    // Wreck sections: each hull type is cut into its sections on first need.
    const wreckShown = new Set<string>();
    for (const [kind, hull] of this.cache) {
      const own = sections.filter(w => w.kind === kind);
      if (own.length && !hull.chunks) {
        hull.chunks = new BattleHullChunks(hull.instances, wreckCuts(kind));
        this.lighting.scene.add(hull.chunks.group);
      }
      if (hull.chunks) for (const id of hull.chunks.update(own, this.width, this.height, frame.pitch, depthSlope)) wreckShown.add(id);
    }
    for (const pool of Object.values(this.pools)) pool.mesh.count = 0;
    for (const flash of this.flashes) flash.intensity = 0;
    let lightIndex = 0;
    for (const burst of bursts) {
      visitBattleBurst(burst, frame.reducedMotion,
        particle => this.pools[particle.kind].add(particle, this.width, this.height, depthSlope), frame.ringTilt);
      const heat = Math.pow(1 - burst.age / burst.life, 3);
      if (lightIndex < this.flashes.length && heat > .08) {
        const light = this.flashes[lightIndex++], r = burst.radius;
        light.position.set(burst.x - this.width / 2, Math.max(12, r * 1.5) + (burst.y - this.height / 2) * depthSlope,
          burst.y - this.height / 2);
        light.color.setHex(burst.color); light.distance = Math.max(80, r * 9);
        light.intensity = r * r * heat * (frame.reducedMotion ? 2 : 5);
      }
    }
    for (const pool of Object.values(this.pools)) pool.finish();
    this.renderer.render(this.lighting.scene, this.camera);
    this.signature = signature; this.frames++;
    this.shown = { ships: new Set(ships.map(s => s.id)), bursts: new Set(bursts.map(b => b.id)), wrecks: wreckShown, changed: true };
    return this.shown;
  }

  private cancelPending(): void { this.wanted.clear(); for (const abort of this.loading.values()) abort.abort(); }

  debug() { return { frames: this.frames, ships: this.shown.ships.size, bursts: this.shown.bursts.size,
    triangles: this.renderer.info.render.triangles, drawCalls: this.renderer.info.render.calls,
    cached: [...this.cache.keys()], loading: [...this.loading.keys()], healthy: this.healthy }; }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true; this.cancelPending();
    for (const hull of this.cache.values()) { hull.instances.dispose(); hull.chunks?.dispose(); disposeModel(hull.source); }
    this.cache.clear();
    for (const pool of Object.values(this.pools)) pool.dispose();
    this.canvas.removeEventListener("webglcontextlost", this.contextLost);
    this.lighting.dispose(); this.renderer.dispose(); this.renderer.forceContextLoss();
    this.canvas.width = this.canvas.height = 1;
  }
}

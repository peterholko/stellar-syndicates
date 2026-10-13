// Loaded only for a visible shipyard preview, close-up map hull or battle.
// All three use the same approved LOD2 geometry and PBR lighting.
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";
import { SHIP_MODELS } from "./ship-models.generated";
import type { ShipKind } from "./protocol";

export function modelLighting(renderer: THREE.WebGLRenderer): { scene: THREE.Scene; dispose(): void } {
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.12;
  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  const room = new RoomEnvironment();
  const environment = pmrem.fromScene(room, .04);
  room.dispose();
  pmrem.dispose();
  scene.environment = environment.texture;
  scene.environmentIntensity = .55;
  scene.add(new THREE.HemisphereLight("#deeaff", "#444754", .60));
  const lights: [string, number, [number, number, number]][] = [
    ["#fff8ed", 1.8, [-10, 16, -8]], ["#b1d7ff", 1, [10, 10, 2]],
    ["#8dccff", 1.4, [-4, 9, 14]], ["#dce7f4", 1.6, [0, -14, 3]],
  ];
  for (const [color, intensity, position] of lights) {
    const light = new THREE.DirectionalLight(color, intensity);
    light.position.set(...position);
    scene.add(light);
  }
  return { scene, dispose: () => environment.dispose() };
}

/** Dispose an owning source, after removing any clones that share its assets.
 * Frees geometry, textures and decoded image bitmaps together. */
export function disposeModel(model: THREE.Object3D): void {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();
  model.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return;
    geometries.add(object.geometry);
    for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
      materials.add(material);
      for (const value of Object.values(material)) if (value instanceof THREE.Texture) textures.add(value);
    }
  });
  for (const texture of textures) {
    texture.dispose();
    if (typeof ImageBitmap !== "undefined" && texture.image instanceof ImageBitmap) texture.image.close();
  }
  for (const material of materials) material.dispose();
  for (const geometry of geometries) geometry.dispose();
  model.removeFromParent();
}

/** Display-sized texture copies keep several live hulls affordable. Source GLBs and
 * shipyard previews keep their original textures; geometry is never modified. */
export async function resizeModelTextures(model: THREE.Object3D, maxSize: number, signal?: AbortSignal): Promise<void> {
  const images = new Map<CanvasImageSource, Set<THREE.Texture>>();
  model.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return;
    for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
      for (const value of Object.values(material)) if (value instanceof THREE.Texture && value.image) {
        const image = value.image as CanvasImageSource;
        let textures = images.get(image);
        if (!textures) { textures = new Set(); images.set(image, textures); }
        textures.add(value);
      }
    }
  });
  for (const [image, textures] of images) {
    signal?.throwIfAborted();
    const dimensions = image as { width: number; height: number };
    const ratio = Math.min(1, maxSize / Math.max(dimensions.width, dimensions.height));
    if (ratio === 1) continue;
    const width = Math.max(1, Math.round(dimensions.width * ratio));
    const height = Math.max(1, Math.round(dimensions.height * ratio));
    let resized: ImageBitmap | HTMLCanvasElement;
    if (typeof createImageBitmap === "function") {
      resized = await createImageBitmap(image, { resizeWidth: width, resizeHeight: height,
        resizeQuality: "high", premultiplyAlpha: "none", colorSpaceConversion: "none" });
      if (signal?.aborted) { resized.close(); signal.throwIfAborted(); }
    } else {
      resized = document.createElement("canvas"); resized.width = width; resized.height = height;
      const context = resized.getContext("2d");
      if (!context) throw new Error("Map texture resize unavailable");
      context.drawImage(image, 0, 0, width, height);
    }
    for (const texture of textures) { texture.image = resized; texture.needsUpdate = true; }
    if (typeof ImageBitmap !== "undefined" && image instanceof ImageBitmap) image.close();
  }
}

export async function loadShipModel(kind: ShipKind, signal?: AbortSignal, maxTextureSize?: number): Promise<THREE.Group> {
  const response = await fetch(SHIP_MODELS[kind].url, { signal });
  if (!response.ok) throw new Error(`Ship model HTTP ${response.status}`);
  const bytes = await response.arrayBuffer();
  signal?.throwIfAborted();
  const { scene } = await new GLTFLoader().parseAsync(bytes, "");
  if (signal?.aborted) { disposeModel(scene); signal.throwIfAborted(); }
  if (maxTextureSize) {
    try { await resizeModelTextures(scene, maxTextureSize, signal); }
    catch (error) { disposeModel(scene); throw error; }
  }
  // Center the bounding box and normalize nose-to-tail length. This is preview
  // framing only; map display hierarchy is still set by shipart.ts/render.ts.
  const bounds = new THREE.Box3().setFromObject(scene);
  const center = bounds.getCenter(new THREE.Vector3());
  const length = bounds.max.z - bounds.min.z;
  const wrapper = new THREE.Group();
  scene.position.sub(center);
  wrapper.add(scene);
  wrapper.scale.setScalar(2 / length);
  wrapper.updateMatrixWorld(true);
  return wrapper;
}

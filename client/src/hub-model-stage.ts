// The Authority Nexus for the live map hub and the art baker: one loader and
// one camera, so the baked sprite and the live render frame the station alike.
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { disposeModel, resizeModelTextures } from "./ship-model-stage";
import { createWormholeEffect, type WormholeEffect } from "./wormhole-effect";
import { HUB_YAW } from "./hub-model-policy";

export interface HubStation {
  /** Add this to the scene. */
  readonly root: THREE.Group;
  /** Lean the station by setting `pose.rotation.x` (negative: seen from the bottom edge). */
  readonly pose: THREE.Group;
  /** The station's own frame (its authored units, diameter 19.8): decorative
   * traffic belongs here so it leans and turns with the station. */
  readonly local: THREE.Object3D;
  readonly flow: WormholeEffect;
  dispose(): void;
}

/** Load the station centred on its aperture, its diameter normalized to 2 units,
 * turned by HUB_YAW, with the living vortex attached. The disc lies in the XZ
 * plane, towers up. */
export async function loadHubStation(url: string, options: {
  signal?: AbortSignal; maxTextureSize?: number; playing?: boolean; time?: number;
} = {}): Promise<HubStation> {
  const { signal, maxTextureSize, playing = true, time = 0 } = options;
  const response = await fetch(url, { signal });
  if (!response.ok) throw new Error(`Hub model HTTP ${response.status}`);
  const bytes = await response.arrayBuffer();
  signal?.throwIfAborted();
  const { scene } = await new GLTFLoader().parseAsync(bytes, "");
  if (signal?.aborted) { disposeModel(scene); signal.throwIfAborted(); }
  if (maxTextureSize) {
    try { await resizeModelTextures(scene, maxTextureSize, signal); }
    catch (error) { disposeModel(scene); throw error; }
  }
  const wormhole = scene.getObjectByName("wormhole");
  if (!wormhole) { disposeModel(scene); throw new Error("Hub model has no wormhole assembly"); }
  // Centre the disc on its aperture; keep the disc plane where it was authored.
  const bounds = new THREE.Box3().setFromObject(scene);
  const center = bounds.getCenter(new THREE.Vector3());
  const diameter = Math.max(bounds.max.x - bounds.min.x, bounds.max.z - bounds.min.z);
  scene.position.set(-center.x, 0, -center.z);
  const scale = new THREE.Group(), pose = new THREE.Group(), root = new THREE.Group();
  scale.add(scene);
  scale.scale.setScalar(2 / diameter);
  scale.rotation.y = HUB_YAW;
  pose.add(scale);
  root.add(pose);
  root.updateMatrixWorld(true);
  const flow = createWormholeEffect(wormhole, { playing, time });
  return {
    root, pose, flow, local: scene,
    dispose() { flow.dispose(); disposeModel(root); },
  };
}

/** Straight down from +Y with screen-up −Z, like the map's hull atlas: a
 * negative pose pitch shows the station from the screen's bottom edge. `field`
 * is the half-width of the square view in station radii. */
export function hubCamera(field: number): THREE.OrthographicCamera {
  const camera = new THREE.OrthographicCamera(-field, field, field, -field, .01, 100);
  camera.position.set(0, 12, 0);
  camera.up.set(0, 0, -1);
  camera.lookAt(0, 0, 0);
  camera.updateProjectionMatrix();
  return camera;
}

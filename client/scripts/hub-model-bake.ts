// Renders the Authority Nexus from the approved, full-resolution GLB with the
// live map's lighting, camera and still vortex phase, and posts the PNGs to the
// loopback baker (scripts/bake-hub-model-art.mjs).
import * as THREE from "three";
import { modelLighting } from "../src/ship-model-stage";
import { hubCamera, loadHubStation } from "../src/hub-model-stage";
import { HUB_HERO_ELEVATION, HUB_SPRITE_FIELD, HUB_VORTEX_STILL_TIME } from "../src/hub-model-policy";

const SOURCE = "/art-src/ship-models/authority-nexus-textured-v1/authority-nexus.glb";
const HERO_FILL = .9;

const canvas = document.querySelector("canvas")!;
const status = document.querySelector("#status")!;
const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(1);
renderer.setClearColor(0, 0);
const lighting = modelLighting(renderer);

async function post(view: "top" | "hero"): Promise<Blob> {
  const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob(b => b ? resolve(b) : reject(new Error("PNG export failed"))));
  const result = await fetch(`/__hub-bake/${view}`, { method: "POST", body: blob });
  if (!result.ok) throw new Error(await result.text());
  return blob;
}

/** A perspective camera that fits every vertex inside HERO_FILL of the frame. */
function heroCamera(station: THREE.Object3D, width: number, height: number): THREE.PerspectiveCamera {
  const camera = new THREE.PerspectiveCamera(28, width / height, .01, 100);
  const direction = new THREE.Vector3(0, Math.sin(HUB_HERO_ELEVATION), Math.cos(HUB_HERO_ELEVATION));
  const points: THREE.Vector3[] = [];
  station.updateMatrixWorld(true);
  station.traverse(node => {
    const position = (node as THREE.Mesh).isMesh ? (node as THREE.Mesh).geometry.getAttribute("position") : null;
    if (position) for (let i = 0; i < position.count; i++) points.push(new THREE.Vector3().fromBufferAttribute(position, i).applyMatrix4(node.matrixWorld));
  });
  let low = .5, high = 40;
  for (let i = 0; i < 40; i++) {
    const distance = (low + high) / 2;
    camera.position.copy(direction).multiplyScalar(distance);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld();
    camera.updateProjectionMatrix();
    const fits = points.every(point => {
      const p = point.clone().project(camera);
      return Math.abs(p.x) <= HERO_FILL && Math.abs(p.y) <= HERO_FILL && p.z < 1;
    });
    if (fits) high = distance; else low = distance;
  }
  camera.position.copy(direction).multiplyScalar(high);
  camera.lookAt(0, 0, 0);
  camera.updateProjectionMatrix();
  return camera;
}

try {
  status.textContent = "Rendering the Authority Nexus…";
  const station = await loadHubStation(SOURCE, { playing: false, time: HUB_VORTEX_STILL_TIME });
  lighting.scene.add(station.root);
  renderer.setSize(1254, 1254, false);
  renderer.render(lighting.scene, hubCamera(HUB_SPRITE_FIELD));
  const top = await post("top");
  renderer.setSize(1024, 768, false);
  renderer.render(lighting.scene, heroCamera(station.root, 1024, 768));
  const hero = await post("hero");
  for (const [blob, label] of [[top, "Map sprite (top-down)"], [hero, "Panel hero"]] as const) {
    const figure = document.createElement("figure"), image = new Image(), caption = document.createElement("figcaption");
    image.src = URL.createObjectURL(blob); image.alt = label; caption.textContent = label;
    figure.append(image, caption); document.querySelector("#renders")!.append(figure);
  }
  station.dispose();
  const response = await fetch("/__hub-bake/done", { method: "POST" });
  if (!response.ok) throw new Error(await response.text());
  status.textContent = `Authority Nexus baked and web assets built. ${JSON.stringify(await response.json())}`;
  document.documentElement.dataset.ready = "true";
} catch (error) {
  status.textContent = `Bake failed: ${error}`;
  document.documentElement.dataset.error = String(error);
  console.error(error);
} finally { lighting.dispose(); renderer.dispose(); }

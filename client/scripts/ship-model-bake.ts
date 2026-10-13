import * as THREE from "three";
import { SHIP_MODELS } from "../src/ship-models.generated";
import { disposeModel, loadShipModel, modelLighting } from "../src/ship-model-stage";
import type { ShipKind } from "../src/protocol";

const canvas = document.querySelector("canvas")!;
const status = document.querySelector("#status")!;
const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, preserveDrawingBuffer: true });
renderer.setPixelRatio(1);
renderer.setClearColor(0, 0);
const lighting = modelLighting(renderer);
const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, .01, 100);
try {
  const planResponse = await fetch("/__ship-bake/plan");
  if (!planResponse.ok) throw new Error(await planResponse.text());
  const plan: { kind: ShipKind; views: ("top" | "perspective" | "bottom")[] }[] = await planResponse.json();
  for (const { kind, views } of plan) {
    if (!views.length) continue;
    status.textContent = `Rendering ${SHIP_MODELS[kind].title}…`;
    const model = await loadShipModel(kind);
    lighting.scene.add(model);
    const box = new THREE.Box3().setFromObject(model);
    const size = box.getSize(new THREE.Vector3());
    const radius = box.getBoundingSphere(new THREE.Sphere()).radius;
    for (const view of views) {
      const px = view === "top" ? 1254 : 768;
      renderer.setSize(px, px, false);
      const field = view === "top" ? Math.max(size.x, size.z) * 1.12 : radius * 2.22;
      camera.left = -field / 2; camera.right = field / 2;
      camera.top = field / 2; camera.bottom = -field / 2;
      camera.up.set(0, 0, -1);
      if (view === "top") camera.position.set(0, 12, 0);
      else if (view === "bottom") { camera.position.set(0, -12, 0); camera.up.set(0, 0, 1); }
      else { camera.position.set(8, 6, -10); camera.up.set(0, 1, 0); }
      camera.lookAt(0, 0, 0); camera.updateProjectionMatrix();
      renderer.render(lighting.scene, camera);
      if (renderer.info.render.triangles !== SHIP_MODELS[kind].triangles) throw new Error(`${kind}: unexpected triangle count`);
      const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob(b => b ? resolve(b) : reject(new Error("PNG export failed"))));
      const result = await fetch(`/__ship-bake/${kind}/${view}`, { method: "POST", body: blob });
      if (!result.ok) throw new Error(await result.text());
      if (view === "top") {
        const figure = document.createElement("figure"), image = new Image(), caption = document.createElement("figcaption");
        image.src = URL.createObjectURL(blob); image.alt = SHIP_MODELS[kind].title;
        caption.textContent = `${SHIP_MODELS[kind].title} · ${renderer.info.render.triangles.toLocaleString()} tris`;
        figure.append(image, caption); document.querySelector("#renders")!.append(figure);
      }
    }
    disposeModel(model);
  }
  const response = await fetch("/__ship-bake/done", { method: "POST" });
  if (!response.ok) throw new Error(await response.text());
  const rendered = plan.reduce((n, ship) => n + ship.views.length, 0);
  status.textContent = `${plan.length} lowest-detail models ready: ${rendered} views baked, ${plan.length * 3 - rendered} verified views retained. Production derivatives built. ${JSON.stringify(await response.json())}`;
  document.documentElement.dataset.ready = "true";
} catch (error) {
  status.textContent = `Bake failed: ${error}`;
  document.documentElement.dataset.error = String(error);
  console.error(error);
} finally { lighting.dispose(); renderer.dispose(); }

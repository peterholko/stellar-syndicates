import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';

const fleet = window.PIRATE_FLEET;
const stage = document.getElementById('stage');
const status = document.getElementById('status');
const reference = document.getElementById('reference');
const referenceImage = document.getElementById('reference-image');
const contextError = document.getElementById('context-error');
const modelCache = new Map();
let renderer;
try {
  renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
} catch (error) {
  contextError.hidden = false;
  contextError.textContent = 'This device could not start WebGL. The reference sheets, rendered previews and GLB downloads remain available.';
  console.error(error);
}

if (renderer) {
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.25;
  renderer.domElement.setAttribute('aria-label', 'Interactive pirate ship. Drag to rotate, scroll to zoom, right-drag to pan.');
  renderer.domElement.tabIndex = 0;
  stage.append(renderer.domElement);
}

const scene = new THREE.Scene();
const camera = new THREE.OrthographicCamera(-14, 14, 14, -14, .1, 300);
const controls = renderer ? new OrbitControls(camera, renderer.domElement) : null;
if (controls) {
  controls.enableDamping = false;
  controls.minZoom = .15;
  controls.maxZoom = 12;
  controls.addEventListener('change', draw);
}
if (renderer) {
  const pmrem = new THREE.PMREMGenerator(renderer);
  const room = new RoomEnvironment();
  const environment = pmrem.fromScene(room, .04);
  scene.environment = environment.texture;
  scene.environmentIntensity = .65;
  room.dispose();
  pmrem.dispose();
}
scene.add(new THREE.HemisphereLight(0xe3edff, 0x746856, 2.0));
const key = new THREE.DirectionalLight(0xffe9ce, 3.1);
key.position.set(-12, 22, -14);
scene.add(key);
const lowerFill = new THREE.DirectionalLight(0xd1deff, 1.8);
lowerFill.position.set(10, -12, 8);
scene.add(lowerFill);
const rim = new THREE.DirectionalLight(0xf7ce89, 1.1);
rim.position.set(0, 4, 20);
scene.add(rim);

let selected = 0;
let current = null;
let loadToken = 0;
let frameSpan = 24;
let wireframe = false;
let spin = false;
let spinFrame = null;
let lastTime = 0;
const views = {
  hero: [[-1.1, 1.55, -1.45], [0, 1, 0]],
  top: [[0, 1, 0], [0, 0, -1]],
  bottom: [[0, -1, 0], [0, 0, -1]],
  front: [[0, 0, -1], [0, 1, 0]],
  back: [[0, 0, 1], [0, 1, 0]],
  side: [[-1, 0, 0], [0, 1, 0]],
};

function decode(base64) {
  return Uint8Array.from(atob(base64), char => char.charCodeAt(0));
}

function draw() {
  if (renderer && !reference.classList.contains('open')) renderer.render(scene, camera);
}

function resize() {
  const width = stage.clientWidth;
  const height = stage.clientHeight;
  const aspect = width/Math.max(1, height);
  camera.left = -frameSpan*aspect/2;
  camera.right = frameSpan*aspect/2;
  camera.top = frameSpan/2;
  camera.bottom = -frameSpan/2;
  camera.updateProjectionMatrix();
  renderer?.setSize(width, height);
  draw();
}

function fitView(name = 'hero') {
  if (!current) return;
  stopSpin();
  current.rotation.set(0, 0, 0);
  const [direction, up] = views[name];
  const target = new THREE.Vector3();
  const box = new THREE.Box3().setFromObject(current);
  box.getCenter(target);
  camera.up.set(...up);
  camera.position.copy(target).add(new THREE.Vector3(...direction).normalize().multiplyScalar(70));
  camera.zoom = 1;
  camera.lookAt(target);
  camera.updateMatrixWorld();
  const corners = [];
  for (const x of [box.min.x, box.max.x]) for (const y of [box.min.y, box.max.y]) for (const z of [box.min.z, box.max.z]) {
    corners.push(new THREE.Vector3(x, y, z).applyMatrix4(camera.matrixWorldInverse));
  }
  const projected = new THREE.Box3().setFromPoints(corners).getSize(new THREE.Vector3());
  const aspect = stage.clientWidth/Math.max(1, stage.clientHeight);
  frameSpan = Math.max(projected.y, projected.x/aspect)*1.25;
  if (controls) {
    controls.target.copy(target);
    controls.update();
  }
  document.querySelectorAll('[data-view]').forEach(button => button.setAttribute('aria-pressed', String(button.dataset.view === name)));
  resize();
}

function applyWireframe(object) {
  object.traverse(node => {
    if (node.isMesh) {
      const materials = Array.isArray(node.material) ? node.material : [node.material];
      materials.forEach(material => { material.wireframe = wireframe; });
    }
  });
}

async function selectModel(index) {
  selected = index;
  const token = ++loadToken;
  const item = fleet[index];
  document.querySelectorAll('[data-ship]').forEach(button => button.setAttribute('aria-pressed', String(Number(button.dataset.ship) === index)));
  document.getElementById('ship-name').textContent = item.title;
  document.getElementById('ship-description').textContent = item.description;
  document.getElementById('triangles').textContent = item.info.triangles.toLocaleString();
  document.getElementById('sections').textContent = item.info.renderSections;
  document.getElementById('reference-image').src = item.reference;
  referenceImage.alt = `${item.title}: front, back, top, bottom and side reference sheet`;
  document.getElementById('fallback-preview').src = item.preview;
  document.getElementById('fallback-preview').hidden = Boolean(renderer);
  status.textContent = renderer ? 'Loading textured model…' : 'Rendered preview';
  if (!renderer) return;
  try {
    if (!modelCache.has(item.id)) {
      modelCache.set(item.id, new Promise((resolve, reject) => {
        new GLTFLoader().parse(decode(item.glb).buffer, '', resolve, reject);
      }));
    }
    const gltf = await modelCache.get(item.id);
    if (token !== loadToken) return;
    if (current) scene.remove(current);
    current = gltf.scene;
    applyWireframe(current);
    scene.add(current);
    fitView();
    status.textContent = 'Drag to rotate · Scroll to zoom · Right-drag to pan';
  } catch (error) {
    status.textContent = 'Model could not be displayed. Use the GLB download or rendered preview.';
    document.getElementById('fallback-preview').hidden = false;
    console.error(error);
  }
}

function stopSpin() {
  spin = false;
  if (spinFrame !== null) cancelAnimationFrame(spinFrame);
  spinFrame = null;
  document.getElementById('spin').setAttribute('aria-pressed', 'false');
}

function spinTick(time) {
  if (!spin) return;
  if (current && !document.hidden && !reference.classList.contains('open')) {
    current.rotation.y += Math.min((time-lastTime)/1000, .04)*.28;
    draw();
  }
  lastTime = time;
  spinFrame = requestAnimationFrame(spinTick);
}

function saveBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 30_000);
}

document.querySelectorAll('[data-ship]').forEach(button => button.addEventListener('click', () => selectModel(Number(button.dataset.ship))));
document.querySelectorAll('[data-view]').forEach(button => button.addEventListener('click', () => fitView(button.dataset.view)));
document.getElementById('wireframe').addEventListener('click', event => {
  wireframe = !wireframe;
  event.currentTarget.setAttribute('aria-pressed', String(wireframe));
  if (current) applyWireframe(current);
  draw();
});
document.getElementById('spin').addEventListener('click', event => {
  if (spin) return stopSpin();
  if (!current) return;
  spin = true;
  event.currentTarget.setAttribute('aria-pressed', 'true');
  lastTime = performance.now();
  spinFrame = requestAnimationFrame(spinTick);
});
document.getElementById('download').addEventListener('click', () => {
  const item = fleet[selected];
  saveBlob(new Blob([decode(item.glb)], { type: 'model/gltf-binary' }), `${item.id}.glb`);
});
document.getElementById('download-reference').addEventListener('click', () => {
  const item = fleet[selected];
  saveBlob(new Blob([decode(item.reference.split(',')[1])], { type: 'image/png' }), `${item.id}-reference.png`);
});
function toggleReference(show) {
  reference.classList.toggle('open', show);
  reference.hidden = !show;
  document.getElementById('reference-toggle').setAttribute('aria-expanded', String(show));
  if (show) document.getElementById('reference-close').focus();
  else { document.getElementById('reference-toggle').focus(); draw(); }
}
document.getElementById('reference-toggle').addEventListener('click', () => toggleReference(true));
document.getElementById('reference-close').addEventListener('click', () => toggleReference(false));
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && !reference.hidden) toggleReference(false);
});
new ResizeObserver(resize).observe(stage);
renderer?.domElement.addEventListener('webglcontextlost', event => {
  event.preventDefault();
  stopSpin();
  status.textContent = 'Graphics context paused. Reload to restore the interactive model.';
  document.getElementById('fallback-preview').hidden = false;
});
selectModel(0);

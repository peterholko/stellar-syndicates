import * as THREE from 'three';
import {GLTFLoader} from 'three/addons/loaders/GLTFLoader.js';
import {OrbitControls} from 'three/addons/controls/OrbitControls.js';
import {RoomEnvironment} from 'three/addons/environments/RoomEnvironment.js';

const canvas = document.querySelector('canvas');
const scene = new THREE.Scene();
scene.background = new THREE.Color('#10131c');
const renderer = new THREE.WebGLRenderer({canvas, antialias: true, preserveDrawingBuffer: true});
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = .96;
const pmrem = new THREE.PMREMGenerator(renderer);
const room = new RoomEnvironment();
scene.environment = pmrem.fromScene(room, .04).texture;
scene.environmentIntensity = .42;
room.dispose();
pmrem.dispose();
scene.add(new THREE.HemisphereLight('#deeaff', '#444754', .60));
for (const [color, intensity, position] of [
  ['#fff8ed', 1.8, [-10, 16, -8]], ['#b1d7ff', 1, [10, 10, 2]],
  ['#8dccff', 1.4, [-4, 9, 14]], ['#dce7f4', 1.6, [0, -14, 3]],
]) {
  const light = new THREE.DirectionalLight(color, intensity);
  light.position.set(...position);
  scene.add(light);
}
const camera = new THREE.PerspectiveCamera(34, 1, .1, 300);
const ortho = new THREE.OrthographicCamera(-12, 12, 12, -12, .1, 300);
let activeCamera = camera, model, currentView = 'perspective', wire = false, unlit = false;
const center = new THREE.Vector3(), size = new THREE.Vector3(12, 4, 20);
const controls = new OrbitControls(camera, canvas);
controls.enableDamping = true;
controls.minDistance = 12;
controls.maxDistance = 90;
function resize() {
  renderer.setSize(innerWidth, innerHeight);
  camera.aspect = innerWidth / innerHeight;
  camera.updateProjectionMatrix();
  const overhead = ['top', 'bottom'].includes(currentView);
  const side = currentView === 'side';
  const width = side ? size.z : size.x;
  const height = overhead ? size.z : size.y;
  const halfHeight = Math.max(height / 2, width / camera.aspect / 2) * 1.38;
  ortho.left = -halfHeight * camera.aspect;
  ortho.right = halfHeight * camera.aspect;
  ortho.top = halfHeight;
  ortho.bottom = -halfHeight;
  ortho.updateProjectionMatrix();
}
function view(name) {
  if (!model) return;
  currentView = name;
  controls.autoRotate = false;
  document.querySelector('#spin').classList.remove('active');
  document.querySelectorAll('[data-view]').forEach(el => el.classList.toggle('active', el.dataset.view === name));
  controls.enabled = name === 'perspective';
  if (controls.enabled) {
    activeCamera = camera;
    const fit = 1.35 * Math.max(1, .8 / (innerWidth / innerHeight));
    camera.position.copy(new THREE.Vector3(1.1, .85, -1.3).normalize().multiplyScalar(size.length() * fit).add(center));
    controls.target.copy(center);
    camera.lookAt(center);
  } else {
    activeCamera = ortho;
    const directions = {top: [0, 1, 0], bottom: [0, -1, 0], front: [0, 0, -1], back: [0, 0, 1], side: [-1, 0, 0]};
    ortho.up.set(...(['top', 'bottom'].includes(name) ? [0, 0, -1] : [0, 1, 0]));
    ortho.position.copy(new THREE.Vector3(...directions[name]).multiplyScalar(45).add(center));
    ortho.lookAt(center);
  }
  resize();
  controls.update();
  renderer.render(scene, activeCamera);
}
window.addEventListener('resize', resize);
for (const button of document.querySelectorAll('[data-view]')) button.onclick = () => view(button.dataset.view);
document.querySelector('#wire').onclick = () => {
  if (!model) return;
  wire = !wire;
  document.querySelector('#wire').classList.toggle('active', wire);
  model.traverse(obj => {if (obj.isMesh) obj.material.wireframe = wire;});
};
document.querySelector('#unlit').onclick = () => {
  if (!model) return;
  unlit = !unlit;
  document.querySelector('#unlit').classList.toggle('active', unlit);
  model.traverse(obj => {
    if (!obj.isMesh) return;
    obj.userData.original ??= obj.material;
    obj.userData.unlit ??= new THREE.MeshBasicMaterial({map: obj.userData.original.map, color: obj.userData.original.color});
    obj.material = unlit ? obj.userData.unlit : obj.userData.original;
    obj.material.wireframe = wire;
  });
};
document.querySelector('#spin').onclick = () => {
  if (!model) return;
  if (currentView !== 'perspective') view('perspective');
  controls.autoRotate = !controls.autoRotate;
  controls.autoRotateSpeed = .65;
  document.querySelector('#spin').classList.toggle('active', controls.autoRotate);
};
const bytes = Uint8Array.from(atob(document.querySelector('#model-data').textContent.trim()), c => c.charCodeAt(0));
function download(data, name, type) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([data], {type}));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}
document.querySelector('#download').onclick = () => download(bytes, 'free-trader.glb', 'model/gltf-binary');
document.querySelector('#snapshot').onclick = () => {
  renderer.render(scene, activeCamera);
  canvas.toBlob(blob => download(blob, 'free-trader-' + currentView + '.png', 'image/png'));
};
const query = new URLSearchParams(location.search);
if (query.has('clean')) document.body.classList.add('clean');
new GLTFLoader().parse(bytes.buffer, '', gltf => {
  model = gltf.scene;
  scene.add(model);
  const box = new THREE.Box3().setFromObject(model);
  box.getCenter(center);
  box.getSize(size);
  const requested = query.get('view');
  view(['top', 'bottom', 'front', 'back', 'side'].includes(requested) ? requested : 'perspective');
  document.querySelector('#loading').hidden = true;
  document.documentElement.dataset.modelLoaded = 'true';
  document.querySelector('#render-count').textContent = renderer.info.render.calls + ' draw calls';
}, error => {
  document.querySelector('#loading').textContent = 'Unable to load model: ' + error.message;
  console.error(error);
});
resize();
renderer.setAnimationLoop(() => {
  if (controls.enabled) controls.update();
  renderer.render(scene, activeCamera);
});

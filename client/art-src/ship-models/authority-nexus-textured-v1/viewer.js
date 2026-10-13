import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';
import { createNexusState } from './nexus-state.js';
import { createWormholeTraffic } from './wormhole-traffic.js';

const asset = window.NEXUS_ASSET;
const stage = document.querySelector('#stage');
const status = document.querySelector('#status');
const fallback = document.querySelector('#preview');
const reference = document.querySelector('#reference');
const power = document.querySelector('#power');
const collapse = document.querySelector('#collapse');
const flow = document.querySelector('#flow');
const trafficButton = document.querySelector('#traffic');
const trafficStatus = document.querySelector('#traffic-status');
const damage = document.querySelector('#damage');
let damageMode = new URLSearchParams(location.search).get('state') === 'disabled';
const bytes = Uint8Array.from(atob(asset.glb), c => c.charCodeAt(0));
const disabledBytes = Uint8Array.from(atob(asset.disabled.glb), c => c.charCodeAt(0));
function updateVariantInfo() {
  const selected = damageMode ? asset.disabled : asset;
  fallback.src = selected.preview;
  document.querySelector('#triangle-count').textContent = selected.info.triangles.toLocaleString();
  document.querySelector('#section-count').textContent = selected.info.renderSections;
  damage.setAttribute('aria-pressed', String(damageMode));
}
updateVariantInfo();
document.querySelector('#reference-image').src = asset.reference;
document.querySelector('#download').addEventListener('click', () => {
  const url = URL.createObjectURL(new Blob([damageMode ? disabledBytes : bytes], { type: 'model/gltf-binary' }));
  const a = document.createElement('a');
  a.href = url; a.download = damageMode ? 'authority-nexus-disabled.glb' : 'authority-nexus.glb'; a.click();
  setTimeout(() => URL.revokeObjectURL(url), 30000);
});
document.querySelector('#show-reference').addEventListener('click', () => { reference.hidden = false; });
document.querySelector('#hide-reference').addEventListener('click', () => { reference.hidden = true; });

let renderer;
try {
  renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
} catch (e) {
  status.textContent = 'WebGL unavailable. The rendered preview, reference sheet and GLB download remain available.';
  document.querySelectorAll('[data-view], #wireframe, #spin, #flow, #traffic, #collapse, #power, #damage').forEach(el => { el.disabled = true; });
  trafficStatus.textContent = 'Ship traffic requires the interactive viewer.';
}

if (renderer) start();

function start() {
  renderer.setPixelRatio(Math.min(devicePixelRatio || 1, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.10;
  renderer.domElement.tabIndex = 0;
  renderer.domElement.setAttribute('aria-label', 'Authority Nexus 3D model. Drag to rotate, scroll to zoom, right-drag to pan.');
  stage.append(renderer.domElement);
  const scene = new THREE.Scene();
  const camera = new THREE.OrthographicCamera(-15, 15, 15, -15, .1, 200);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.minZoom = .2; controls.maxZoom = 12;
  controls.addEventListener('change', draw);
  const generator = new THREE.PMREMGenerator(renderer);
  const room = new RoomEnvironment();
  scene.environment = generator.fromScene(room, .04).texture;
  scene.environmentIntensity = .52;
  room.dispose(); generator.dispose();
  scene.add(new THREE.HemisphereLight(0xe7efff, 0x817462, 1.6));
  for (const [color, strength, position] of [
    [0xffeedb, 2.4, [-12, 22, -14]], [0xc5ddff, 1.5, [10, -14, 9]], [0xffdfae, .7, [5, 5, 18]],
  ]) {
    const light = new THREE.DirectionalLight(color, strength);
    light.position.set(...position); scene.add(light);
  }
  const views = {
    hero: [[-.8, 1.42, -1.34], [0, 1, 0]],
    top: [[0, 1, 0], [0, 0, -1]], bottom: [[0, -1, 0], [0, 0, -1]],
    front: [[0, 0, -1], [0, 1, 0]], back: [[0, 0, 1], [0, 1, 0]],
    side: [[-1, 0, 0], [0, 1, 0]], underside: [[1.1, -1.65, 1.3], [0, -1, 0]],
  };
  let model = null, nexus = null;
  let span = 27, powerValue = 1;
  let spin = false, frame = null, lastTime = 0, transition = null;
  let contextLost = false, motionOverride = false;
  let traffic = null, trafficOverride = false;
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  let motion = !reducedMotion.matches;
  let trafficEnabled = !reducedMotion.matches;
  flow.setAttribute('aria-pressed', String(motion));
  trafficButton.setAttribute('aria-pressed', String(trafficEnabled));

  function draw() { renderer.render(scene, camera); }
  function resize() {
    const w = stage.clientWidth, h = Math.max(stage.clientHeight, 1), aspect = w/h;
    camera.left = -span*aspect/2; camera.right = span*aspect/2;
    camera.top = span/2; camera.bottom = -span/2;
    camera.updateProjectionMatrix(); renderer.setSize(w, h); draw();
  }
  function view(name) {
    if (!model) return;
    model.rotation.set(0, 0, 0);
    const target = new THREE.Vector3(0, 1.55, 0);
    const [direction, up] = views[name];
    camera.up.set(...up);
    camera.position.copy(target).add(new THREE.Vector3(...direction).normalize().multiplyScalar(65));
    camera.zoom = 1; camera.lookAt(target); camera.updateMatrixWorld();
    controls.target.copy(target); controls.update();
    const corners = [];
    for (const x of [-10, 10]) for (const y of [-1.5, 5]) for (const z of [-10, 10])
      corners.push(new THREE.Vector3(x, y, z).applyMatrix4(camera.matrixWorldInverse));
    const projected = new THREE.Box3().setFromPoints(corners).getSize(new THREE.Vector3());
    span = Math.max(projected.y, projected.x/(stage.clientWidth/Math.max(1, stage.clientHeight)))*1.12;
    document.querySelectorAll('[data-view]').forEach(button => button.setAttribute('aria-pressed', String(button.dataset.view === name)));
    resize();
  }
  function setPower(value, repaint = true) {
    nexus?.setPower(value);
    powerValue = nexus?.power ?? Math.min(1, Math.max(0, value));
    power.value = String(Math.round(powerValue*100));
    power.disabled = damageMode; collapse.disabled = damageMode; flow.disabled = damageMode;
    flow.setAttribute('aria-pressed', String(!damageMode && motion));
    if (traffic) trafficStatus.textContent = !trafficEnabled ? 'Ship traffic off' :
      damageMode ? 'Attack disabled — cargo holding outside the ring' :
      powerValue > .94 ? 'Cargo freighters and escorts entering the passage' : 'Traffic holding for stable containment';
    document.querySelector('#power-label').textContent = damageMode ? 'Disabled' : powerValue < .001 ? 'Closed' : powerValue > .999 ? 'Open' : 'Losing containment';
    collapse.textContent = damageMode ? 'Repair needed' : powerValue < .5 ? 'Restore wormhole' : 'Close wormhole';
    if (repaint) draw();
  }
  function needsAnimation() {
    return !document.hidden && !contextLost && model && (spin || transition || nexus?.active);
  }
  function tick(time) {
    frame = null;
    if (document.hidden || contextLost) return;
    const dt = Math.min((time-lastTime)/1000, .05); lastTime = time;
    if (spin && model && !document.hidden) model.rotation.y += dt*.20;
    if (transition) {
      transition.elapsed += dt;
      const t = Math.min(1, transition.elapsed/2.7);
      setPower(THREE.MathUtils.lerp(transition.from, transition.to, t), false);
      if (t === 1) transition = null;
    }
    nexus?.update(dt);
    draw();
    if (needsAnimation()) frame = requestAnimationFrame(tick);
  }
  function run() {
    if (frame === null && needsAnimation()) { lastTime = performance.now(); frame = requestAnimationFrame(tick); }
  }

  new ResizeObserver(resize).observe(stage);
  document.querySelectorAll('[data-view]').forEach(button => button.addEventListener('click', () => view(button.dataset.view)));
  document.querySelector('#wireframe').addEventListener('click', event => {
    const enabled = event.currentTarget.getAttribute('aria-pressed') !== 'true';
    event.currentTarget.setAttribute('aria-pressed', String(enabled));
    model?.traverse(node => { if (node.isMesh) node.material.wireframe = enabled; }); draw();
  });
  document.querySelector('#spin').addEventListener('click', event => {
    spin = !spin; event.currentTarget.setAttribute('aria-pressed', String(spin)); run();
  });
  flow.addEventListener('click', () => {
    motionOverride = true; motion = !motion;
    flow.setAttribute('aria-pressed', String(motion));
    nexus?.setMotion(motion); run();
  });
  damage.addEventListener('click', () => {
    damageMode = !damageMode; transition = null;
    nexus.setDisabled(damageMode); updateVariantInfo();
    setPower(nexus.power); run();
    status.textContent = damageMode ? 'Attack-disabled variant · Toggle Attack disabled to compare with the operational station' :
      'Drag to rotate · Scroll to zoom · Right-drag to pan';
  });
  trafficButton.addEventListener('click', () => {
    trafficOverride = true; trafficEnabled = !trafficEnabled;
    trafficButton.setAttribute('aria-pressed', String(trafficEnabled));
    traffic?.setEnabled(trafficEnabled); setPower(powerValue); run();
  });
  reducedMotion.addEventListener('change', event => {
    if (!motionOverride) {
      motion = !event.matches;
      flow.setAttribute('aria-pressed', String(motion));
      nexus?.setMotion(motion);
    }
    if (!trafficOverride) {
      trafficEnabled = !event.matches;
      trafficButton.setAttribute('aria-pressed', String(trafficEnabled));
      traffic?.setEnabled(trafficEnabled);
    }
    setPower(powerValue); run();
  });
  document.addEventListener('visibilitychange', () => {
    if (document.hidden && frame !== null) { cancelAnimationFrame(frame); frame = null; }
    else run();
  });
  power.addEventListener('input', () => { transition = null; setPower(Number(power.value)/100); run(); });
  collapse.addEventListener('click', () => {
    transition = {elapsed: 0, from: powerValue, to: powerValue < .5 ? 1 : 0}; run();
  });
  renderer.domElement.addEventListener('webglcontextlost', event => {
    event.preventDefault(); fallback.hidden = false;
    contextLost = true;
    if (frame !== null) { cancelAnimationFrame(frame); frame = null; }
    status.textContent = 'Graphics context lost. Reload this viewer to restore interaction.';
  });

  Promise.all([bytes, disabledBytes].map(data => new GLTFLoader().parseAsync(data.buffer, ''))).then(([gltf, disabled]) => {
    nexus = createNexusState(gltf, disabled, { motion });
    nexus.setDisabled(damageMode);
    model = nexus.group; scene.add(model);
    model.traverse(node => {
      if (node.isMesh) {
        for (const texture of [node.material.map, node.material.metalnessMap, node.material.roughnessMap, node.material.emissiveMap]) {
          if (texture) texture.anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
        }
      }
    });
    damage.disabled = false;
    fallback.hidden = true;
    status.textContent = damageMode ? 'Attack-disabled variant · Toggle Attack disabled to compare with the operational station' :
      'Drag to rotate · Scroll to zoom · Right-drag to pan';
    setPower(nexus.power); view('hero'); run();
    // Ship assets are embedded too, so traffic works when viewer.html is opened
    // directly from disk. Failure here leaves the station and vortex usable.
    Promise.all(Object.entries(asset.traffic).map(async ([id, encoded]) => {
      const buffer = Uint8Array.from(atob(encoded), c => c.charCodeAt(0)).buffer;
      return [id, (await new GLTFLoader().parseAsync(buffer, '')).scene];
    })).then(entries => {
      traffic = createWormholeTraffic(Object.fromEntries(entries), { enabled: trafficEnabled });
      nexus.attachTraffic(traffic);
      const wireframe = document.querySelector('#wireframe').getAttribute('aria-pressed') === 'true';
      traffic.group.traverse(node => { if (node.isMesh) node.material.wireframe = wireframe; });
      setPower(powerValue); run();
    }).catch(error => {
      trafficButton.disabled = true;
      trafficStatus.textContent = 'Ship traffic could not load. The station remains available.';
      console.error(error);
    });
  }).catch(error => {
    fallback.hidden = false;
    status.textContent = 'The model could not be displayed. The GLB download and rendered preview are available.';
    console.error(error);
  });
}

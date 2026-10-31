import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/addons/postprocessing/OutputPass.js';
import { createWormholeTraffic } from './wormhole-traffic.js';
import { HUB_FLOW_PERIOD, hubFlowMapFragment } from './wormhole-flow.js';

const asset = window.HUB_ASSET;
const $ = id => document.getElementById(id);
const fromBase64 = encoded => Uint8Array.from(atob(encoded), char => char.charCodeAt(0));
const bytes = fromBase64(asset.glb);
const status = $('status');
$('reference-image').src = asset.reference;
$('triangles').textContent = asset.info.triangles.toLocaleString();
$('download').addEventListener('click', () => {
  const url = URL.createObjectURL(new Blob([bytes], {type: 'model/gltf-binary'}));
  const link = document.createElement('a');
  link.href = url; link.download = 'wormhole-hub.glb'; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});
$('reference').addEventListener('click', () => $('reference-dialog').showModal());
$('close-reference').addEventListener('click', () => $('reference-dialog').close());

async function startViewer() {
try {
  const renderer = new THREE.WebGLRenderer({antialias: true, alpha: false, preserveDrawingBuffer: true});
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.0;
  $('stage').append(renderer.domElement);
  renderer.domElement.setAttribute('aria-label', 'Interactive wormhole hub model. Drag to orbit; scroll to zoom.');
  const scene = new THREE.Scene();
  scene.background = new THREE.Color('#080e18');
  const camera = new THREE.OrthographicCamera(-12, 12, 12, -12, .1, 200);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.enableDamping = true;
  controls.dampingFactor = .085;
  controls.minZoom = .4; controls.maxZoom = 6;
  controls.autoRotateSpeed = .65;
  const environment = new RoomEnvironment();
  const pmrem = new THREE.PMREMGenerator(renderer);
  const environmentTarget = pmrem.fromScene(environment, .04);
  scene.environment = environmentTarget.texture;
  scene.environmentIntensity = .65;
  environment.dispose(); pmrem.dispose();
  scene.add(new THREE.HemisphereLight(0xdcefff, 0x463c2f, .9));
  const key = new THREE.DirectionalLight(0xf4f4ff, 2.6);
  key.position.set(-8, 12, 14); scene.add(key);
  const fill = new THREE.DirectionalLight(0x7cb9ff, 1.3);
  fill.position.set(10, -3, 5); scene.add(fill);
  const rim = new THREE.DirectionalLight(0xe9b56d, 2.0);
  rim.position.set(-5, 8, -10); scene.add(rim);
  const composer = new EffectComposer(renderer);
  composer.addPass(new RenderPass(scene, camera));
  const bloom = new UnrealBloomPass(new THREE.Vector2(1, 1), .28, .45, .92);
  composer.addPass(bloom); composer.addPass(new OutputPass());
  const gltf = await new GLTFLoader().parseAsync(bytes.buffer, '');
  const model = gltf.scene; scene.add(model);
  const portal = model.getObjectByName('wormhole');
  const field = model.getObjectByName('containment-field');
  const materials = new Set();
  const hardware = [];
  const maxAnisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
  model.traverse(object => {
    if (!object.isMesh) return;
    materials.add(object.material);
    for (const name of ['map', 'metalnessMap', 'roughnessMap', 'emissiveMap']) {
      if (object.material[name]) object.material[name].anisotropy = maxAnisotropy;
    }
    if (!object.name.startsWith('wormhole')) hardware.push(object);
  });
  const originalMaterials = new Map(hardware.map(object => [object, object.material]));
  const clay = new THREE.MeshStandardMaterial({color: 0x889bad, metalness: .12, roughness: .65});
  const flow = {time: {value: 0}};
  portal.traverse(object => {
    if (!object.isMesh) return;
    object.material.onBeforeCompile = shader => {
      shader.uniforms.uHubTime = flow.time;
      shader.fragmentShader = 'uniform float uHubTime;\n' + shader.fragmentShader;
      shader.fragmentShader = shader.fragmentShader.replace('#include <map_fragment>', hubFlowMapFragment);
    };
    object.material.customProgramCacheKey = () => 'wormhole-hub-bounded-flow-v2';
  });
  const bounds = new THREE.Box3().setFromObject(model);
  const target = bounds.getCenter(new THREE.Vector3());
  let viewName = 'orbit';
  const views = {
    orbit: [[.65, .36, 1], [0, 1, 0]], front: [[0, 0, 1], [0, 1, 0]],
    back: [[0, 0, -1], [0, 1, 0]], left: [[-1, 0, 0], [0, 1, 0]],
    right: [[1, 0, 0], [0, 1, 0]], top: [[0, 1, 0], [0, 0, -1]],
    bottom: [[0, -1, 0], [0, 0, 1]],
  };
  let frame = 0, lastTime = performance.now();
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let motion = !reduced;
  let traffic = null;
  let trafficEnabled = !reduced;
  let rotating = false;
  let wired = false;
  let clayMode = false;
  function requestFrame() { if (!frame && !document.hidden) frame = requestAnimationFrame(draw); }
  function draw(now) {
    frame = 0;
    const delta = Math.min((now-lastTime)/1000, .05); lastTime = now;
    if (motion && portal.visible) flow.time.value = (flow.time.value + delta) % HUB_FLOW_PERIOD;
    traffic?.update(delta);
    controls.update(delta); composer.render();
    if ((motion && portal.visible) || rotating || traffic?.active) requestFrame();
  }
  controls.addEventListener('change', requestFrame);
  function resize() {
    const width = $('stage').clientWidth, height = $('stage').clientHeight;
    const aspect = width/height;
    const span = 21.5;
    camera.left = -span*aspect/2; camera.right = span*aspect/2;
    camera.top = span/2; camera.bottom = -span/2;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height); composer.setSize(width, height);
    requestFrame();
  }
  function setView(name) {
    viewName = name;
    const [direction, up] = views[name];
    camera.position.copy(target).add(new THREE.Vector3(...direction).normalize().multiplyScalar(55));
    camera.up.set(...up); camera.zoom = 1;
    controls.target.copy(target); camera.lookAt(target); camera.updateMatrixWorld();
    const size = $('stage').clientWidth / $('stage').clientHeight;
    const corners = [];
    for (const x of [bounds.min.x, bounds.max.x]) for (const y of [bounds.min.y, bounds.max.y])
      for (const z of [bounds.min.z, bounds.max.z]) corners.push(new THREE.Vector3(x,y,z).applyMatrix4(camera.matrixWorldInverse));
    const projected = new THREE.Box3().setFromPoints(corners).getSize(new THREE.Vector3());
    camera.zoom = 21.5 / (Math.max(projected.y, projected.x/size)*1.17);
    camera.updateProjectionMatrix(); controls.update();
    document.querySelectorAll('[data-view]').forEach(button => button.setAttribute('aria-pressed', String(button.dataset.view===name)));
    requestFrame();
  }
  function toggle(id, state) { $(id).setAttribute('aria-pressed', String(state)); }
  document.querySelectorAll('[data-view]').forEach(button => button.addEventListener('click', () => setView(button.dataset.view)));
  $('portal').addEventListener('click', () => {
    portal.visible = !portal.visible; field.visible = portal.visible;
    traffic?.setPower(portal.visible ? 1 : 0);
    toggle('portal', portal.visible); requestFrame();
  });
  $('traffic').addEventListener('click', () => {
    trafficEnabled=!trafficEnabled;
    traffic?.setEnabled(trafficEnabled);
    toggle('traffic',trafficEnabled); requestFrame();
  });
  $('motion').addEventListener('click', () => { motion=!motion; toggle('motion',motion); requestFrame(); });
  $('rotate').addEventListener('click', () => { rotating=!rotating; controls.autoRotate=rotating; toggle('rotate',rotating); requestFrame(); });
  $('wireframe').addEventListener('click', () => {
    wired=!wired; for (const material of [...materials,clay]) material.wireframe=wired;
    traffic?.group.traverse(object => { if (object.isMesh) object.material.wireframe=wired; });
    toggle('wireframe',wired); requestFrame();
  });
  $('clay').addEventListener('click', () => {
    clayMode=!clayMode;
    hardware.forEach(object => { object.material=clayMode?clay:originalMaterials.get(object); });
    toggle('clay',clayMode); requestFrame();
  });
  $('reset').addEventListener('click', () => { rotating=false; controls.autoRotate=false; toggle('rotate',false); setView('orbit'); });
  toggle('motion',motion);
  toggle('traffic',trafficEnabled);
  new ResizeObserver(() => { resize(); setView(viewName); }).observe($('stage'));
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) { cancelAnimationFrame(frame); frame=0; }
    else { lastTime=performance.now(); requestFrame(); }
  });
  resize(); setView('orbit');
  status.textContent = 'Drag to orbit · Scroll to zoom · Right-drag to pan';
  $('loading').hidden = true;
  window.hubViewer = {scene, model, camera, renderer, composer, controls, setView, portal,
    info: asset.info, render: () => composer.render(), getPortalTime: () => flow.time.value,
    get traffic() { return traffic; }};
  // Embedded hulls preserve the same offline behavior as the station viewer.
  Promise.all(Object.entries(asset.traffic).map(async ([type,encoded]) => {
    const shipBytes=fromBase64(encoded);
    return [type,(await new GLTFLoader().parseAsync(shipBytes.buffer,'')).scene];
  })).then(entries => {
    traffic=createWormholeTraffic(Object.fromEntries(entries),{enabled:trafficEnabled});
    traffic.setPower(portal.visible ? 1 : 0);
    traffic.group.traverse(object => {
      if (!object.isMesh) return;
      object.material.wireframe=wired;
      for (const name of ['map','metalnessMap','roughnessMap','emissiveMap']) {
        if (object.material[name]) object.material[name].anisotropy=maxAnisotropy;
      }
    });
    scene.add(traffic.group);
    $('traffic').disabled=false;
    requestFrame();
  }).catch(error => {
    console.error(error);
    $('traffic').disabled=true;
    toggle('traffic',false);
    status.textContent='Freighters could not load. The hub is still available.';
  });
} catch (error) {
  console.error(error);
  $('loading').textContent = 'The 3D view could not start. Open this file in a WebGL-capable browser or download the GLB.';
  status.textContent = error.message;
}
}
startViewer();

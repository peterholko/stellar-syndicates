import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';

const canvas=document.querySelector('canvas');
const status=document.querySelector('#loading');
const scene=new THREE.Scene();
scene.background=new THREE.Color('#0d121a');
const skyCanvas=document.createElement('canvas');
skyCanvas.width=1600;skyCanvas.height=1000;
const sky=skyCanvas.getContext('2d');
sky.fillStyle='#05070d';sky.fillRect(0,0,1600,1000);
let seed=6173;
const random=()=>{seed=(1664525*seed+1013904223)>>>0;return seed/4294967296;};
const stars=Array.from({length:150},()=>({x:random()*1600,y:random()*1000,size:random()>.9?1.7:.8,alpha:.10+random()*.38}));
sky.strokeStyle='#111c2d';sky.lineWidth=.7;
for(let i=0;i<stars.length;i+=7) {
  const a=stars[i],b=stars[(i+3)%stars.length];
  if(Math.hypot(a.x-b.x,a.y-b.y)<340){sky.beginPath();sky.moveTo(a.x,a.y);sky.lineTo(b.x,b.y);sky.stroke();}
}
for(const star of stars){sky.fillStyle=`rgba(169,190,214,${star.alpha})`;sky.fillRect(star.x,star.y,star.size,star.size);}
const galaxyTexture=new THREE.CanvasTexture(skyCanvas);
galaxyTexture.colorSpace=THREE.SRGBColorSpace;
galaxyTexture.minFilter=THREE.LinearFilter;
scene.background=galaxyTexture;
let renderer;
try { renderer=new THREE.WebGLRenderer({canvas,antialias:true,preserveDrawingBuffer:true}); }
catch(error) { status.textContent='A WebGL-enabled browser is needed for the interactive 3D preview.'; throw error; }
renderer.outputColorSpace=THREE.SRGBColorSpace;
renderer.toneMapping=THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure=1.12;
const pmrem=new THREE.PMREMGenerator(renderer);
const environment=new RoomEnvironment();
const envTarget=pmrem.fromScene(environment,.04);
scene.environment=envTarget.texture;
scene.environmentIntensity=.55;
environment.dispose();pmrem.dispose();
// Match the game's map/shipyard lighting in client/src/ship-model-stage.ts.
const lights=[];
const ambient=new THREE.HemisphereLight('#deeaff','#444754',.60);
scene.add(ambient);lights.push({light:ambient,intensity:.60});
for(const [color,intensity,position] of [
  ['#fff8ed',1.8,[-10,16,-8]],['#b1d7ff',1.0,[10,10,2]],
  ['#8dccff',1.4,[-4,9,14]],['#dce7f4',1.6,[0,-14,3]]
]) {
  const light=new THREE.DirectionalLight(color,intensity);
  light.position.set(...position);scene.add(light);lights.push({light,intensity});
}
const camera=new THREE.PerspectiveCamera(34,1,.04,1000);
const ortho=new THREE.OrthographicCamera(-16,16,16,-16,.04,1000);
let activeCamera=camera;
const controls=new OrbitControls(camera,canvas);
controls.enableDamping=true;
controls.autoRotateSpeed=.6;
const center=new THREE.Vector3();
const bounds=new THREE.Box3();
const detailViews={
  driver:{prefix:'mass-driver.',bounds:new THREE.Box3(),center:new THREE.Vector3(),corners:[]},
  spines:{prefix:'spine.',bounds:new THREE.Box3(),center:new THREE.Vector3(),corners:[]}
};
let model, radius=15, currentView='perspective', wire=false, unlit=false, gunMotion=false;
let galaxy=true,lowLight=false;
let currentVariant=new URLSearchParams(location.search).get('variant')==='ivory'?'ivory':'dark';
let corners=[];
const mounts=[];
const models={};
const sources=Object.fromEntries([['ivory','model-data'],['dark','dark-model-data']].map(([name,id])=>
  [name,Uint8Array.from(atob(document.getElementById(id).textContent.trim()),c=>c.charCodeAt(0))]));

function applyLook() {
  model.traverse(obj=>{
    if(!obj.isMesh)return;
    obj.userData.original??=obj.material;
    if(unlit) {
      obj.userData.unlit??=new THREE.MeshBasicMaterial({map:obj.userData.original.map,
        color:obj.userData.original.color,vertexColors:obj.userData.original.vertexColors});
      obj.material=obj.userData.unlit;
    } else obj.material=obj.userData.original;
    obj.material.wireframe=wire;
  });
}
function setVariant(name) {
  if(!models[name])return;
  if(model)scene.remove(model);
  currentVariant=name;model=models[name];scene.add(model);mounts.length=0;
  model.traverse(node=>{
    if(['turret-yaw','secondary-turret-yaw','point-defense-yaw'].includes(node.userData.role)) {
      if(!gunMotion)node.rotation.y=0;
      mounts.push({node,phase:mounts.length*.65});
    }
  });
  applyLook();
  document.querySelectorAll('[data-variant]').forEach(button=>{
    const active=button.dataset.variant===name;
    button.disabled=false;button.classList.toggle('active',active);button.setAttribute('aria-pressed',String(active));
  });
  document.querySelector('#palette-note').textContent=name==='dark'?
    'Blue-charcoal armor · nickel edges · antique gold':'Ivory ceramic armor · antique gold';
}
for(const button of document.querySelectorAll('[data-variant]'))button.onclick=()=>setVariant(button.dataset.variant);

function fitOrthographic(aspect) {
  if(!model)return;
  ortho.updateMatrixWorld();
  const inverse=ortho.matrixWorldInverse;
  const projected=corners.map(p=>p.clone().applyMatrix4(inverse));
  const xs=projected.map(p=>p.x),ys=projected.map(p=>p.y);
  const width=Math.max(...xs)-Math.min(...xs),height=Math.max(...ys)-Math.min(...ys);
  const extent=Math.max(height,width/aspect)*1.18;
  ortho.left=-extent*aspect/2;ortho.right=extent*aspect/2;
  ortho.top=extent/2;ortho.bottom=-extent/2;
  ortho.updateProjectionMatrix();
}
function resize() {
  const area=canvas.parentElement.getBoundingClientRect();
  const width=Math.max(1,area.width),height=Math.max(1,area.height);
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.setSize(width,height,false);
  camera.aspect=width/height;camera.updateProjectionMatrix();
  fitOrthographic(width/height);
}
function setButton(id,active) {
  const el=document.querySelector('#'+id);
  el.classList.toggle('active',active);el.setAttribute('aria-pressed',String(active));
}
function view(name) {
  if(!model)return;
  currentView=name;controls.autoRotate=false;setButton('spin',false);
  document.querySelectorAll('[data-view]').forEach(el=>{
    const selected=el.dataset.view===name;
    el.classList.toggle('active',selected);el.setAttribute('aria-pressed',String(selected));
  });
  const direction={perspective:[1,.85,-1.4],driver:[.8,.65,-1.3],spines:[1,1.1,-1.25],top:[0,1,0],bottom:[0,-1,0],
    front:[0,0,-1],back:[0,0,1],side:[-1,0,0],underside:[1,-.9,-1.3]}[name];
  const detail=detailViews[name];
  const orbit=name==='perspective'||Boolean(detail);
  const target=detail?.center||center;
  const framing=detail?.corners||corners;
  activeCamera=orbit?camera:ortho;
  controls.object=activeCamera;controls.enableRotate=orbit;
  activeCamera.up.set(0,1,0);
  if(name==='top'||name==='bottom')activeCamera.up.set(0,0,-1);
  const axis=new THREE.Vector3(...direction).normalize();
  const right=new THREE.Vector3().crossVectors(activeCamera.up,axis).normalize();
  const up=new THREE.Vector3().crossVectors(axis,right).normalize();
  const tangent=Math.tan(THREE.MathUtils.degToRad(camera.fov/2));
  const fit=Math.max(...framing.map(point=>{
    const p=point.clone().sub(target);
    return p.dot(axis)+Math.max(Math.abs(p.dot(right))/(tangent*camera.aspect),Math.abs(p.dot(up))/tangent);
  }));
  const distance=orbit?fit*1.12:radius*4;
  activeCamera.position.copy(target).add(axis.multiplyScalar(distance));
  activeCamera.zoom=1;activeCamera.lookAt(target);
  controls.target.copy(target);controls.update();resize();
  document.querySelector('#view-hint').textContent=orbit?
    'Drag to orbit · scroll to zoom · right-drag to pan':'Scroll to zoom · right-drag to pan · Orbit returns to free rotation';
  renderer.render(scene,activeCamera);
}
window.addEventListener('resize',resize);
for(const button of document.querySelectorAll('[data-view]'))button.onclick=()=>view(button.dataset.view);
document.querySelector('#wire').onclick=()=>{
  if(!model)return;wire=!wire;setButton('wire',wire);
  model.traverse(obj=>{if(obj.isMesh)obj.material.wireframe=wire;});
};
document.querySelector('#unlit').onclick=()=>{
  if(!model)return;unlit=!unlit;setButton('unlit',unlit);
  applyLook();
};
document.querySelector('#galaxy').onclick=()=>{
  galaxy=!galaxy;setButton('galaxy',galaxy);
  scene.background=galaxy?galaxyTexture:new THREE.Color('#0d121a');
};
document.querySelector('#low-light').onclick=()=>{
  lowLight=!lowLight;setButton('low-light',lowLight);
  const scale=lowLight ? .40 : 1;
  for(const {light,intensity} of lights)light.intensity=intensity*scale;
  scene.environmentIntensity=.55*scale;
};
document.querySelector('#spin').onclick=()=>{
  if(!model)return;if(currentView!=='perspective')view('perspective');
  controls.autoRotate=!controls.autoRotate;setButton('spin',controls.autoRotate);
};
document.querySelector('#guns').onclick=()=>{
  if(!model)return;gunMotion=!gunMotion;setButton('guns',gunMotion);
  if(!gunMotion)for(const mount of mounts)mount.node.rotation.y=0;
};
function download(data,name,type) {
  const url=URL.createObjectURL(new Blob([data],{type}));
  const a=document.createElement('a');a.href=url;a.download=name;a.click();
  setTimeout(()=>URL.revokeObjectURL(url),1500);
}
document.querySelector('#download').onclick=()=>download(sources[currentVariant],
  currentVariant==='dark'?'authority-arbiter-dark.glb':'authority-arbiter.glb','model/gltf-binary');
document.querySelector('#snapshot').onclick=()=>{
  if(!model)return;renderer.render(scene,activeCamera);
  canvas.toBlob(blob=>{if(blob)download(blob,'authority-arbiter-'+currentVariant+'-'+currentView+'.png','image/png');});
};
resize();
const loader=new GLTFLoader();
Promise.all(Object.entries(sources).map(async([name,bytes])=>{
  const gltf=await loader.parseAsync(bytes.buffer,'');models[name]=gltf.scene;
})).then(()=>{
  setVariant(currentVariant);model.updateMatrixWorld(true);
  bounds.setFromObject(model);bounds.getCenter(center);radius=bounds.getSize(new THREE.Vector3()).length()/2;
  for(const x of [bounds.min.x,bounds.max.x])for(const y of [bounds.min.y,bounds.max.y])for(const z of [bounds.min.z,bounds.max.z])corners.push(new THREE.Vector3(x,y,z));
  model.traverse(node=>{
    if(node.isMesh)for(const component of node.userData.sourceComponents||[]) {
      const detail=Object.values(detailViews).find(d=>component.name.startsWith(d.prefix));
      if(!detail)continue;
      const positions=node.geometry.attributes.position,index=node.geometry.index;
      const point=new THREE.Vector3();
      for(let i=component.indexStart;i<component.indexStart+component.indexCount;i++) {
        point.fromBufferAttribute(positions,index.getX(i)).applyMatrix4(node.matrixWorld);
        detail.bounds.expandByPoint(point);
      }
    }
  });
  for(const detail of Object.values(detailViews)) {
    const box=detail.bounds;if(box.isEmpty())box.copy(bounds);
    box.getCenter(detail.center);
    for(const x of [box.min.x,box.max.x])for(const y of [box.min.y,box.max.y])for(const z of [box.min.z,box.max.z])detail.corners.push(new THREE.Vector3(x,y,z));
  }
  controls.minDistance=radius*.16;controls.maxDistance=radius*12;
  controls.minZoom=.25;controls.maxZoom=15;
  const requestedView=new URLSearchParams(location.search).get('view');
  view(Object.hasOwn(detailViews,requestedView)||['top','bottom','front','back','side','underside'].includes(requestedView)?requestedView:'perspective');status.hidden=true;
  document.querySelector('#render-count').textContent=renderer.info.render.calls+' render sections';
  document.querySelector('#controls').inert=false;
  document.documentElement.dataset.modelLoaded='true';
}).catch(error=>{status.textContent='Unable to load the model: '+error.message;console.error(error);});
renderer.setAnimationLoop(time=>{
  if(gunMotion)for(const mount of mounts)mount.node.rotation.y=Math.sin(time*.0004+mount.phase)*.18;
  controls.update();renderer.render(scene,activeCamera);
});

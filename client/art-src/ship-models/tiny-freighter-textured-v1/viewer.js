import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';

const canvas=document.querySelector('canvas');
const scene=new THREE.Scene();
scene.background=new THREE.Color('#0d141e');
const renderer=new THREE.WebGLRenderer({canvas,antialias:true,preserveDrawingBuffer:true});
renderer.setPixelRatio(Math.min(devicePixelRatio,2));
renderer.outputColorSpace=THREE.SRGBColorSpace;
renderer.toneMapping=THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure=.96;
const pmrem=new THREE.PMREMGenerator(renderer);
scene.environment=pmrem.fromScene(new RoomEnvironment(),.04).texture;
scene.environmentIntensity=.42;
const camera=new THREE.PerspectiveCamera(34,1,.1,200);
const ortho=new THREE.OrthographicCamera(-9,9,7,-7,.1,200);
let activeCamera=camera;
const controls=new OrbitControls(camera,canvas);
controls.target.set(0,1.5,0);
controls.enableDamping=true;
controls.minDistance=10;
controls.maxDistance=65;
const ambient=new THREE.HemisphereLight('#deeaff','#444754',.60);
scene.add(ambient);
for(const [color,intensity,position] of [['#fff8ed',1.8,[-10,16,-8]],['#b1d7ff',1.0,[10,10,2]],['#8dccff',1.4,[-4,9,14]],['#dce7f4',1.6,[0,-14,3]]]){
  const light=new THREE.DirectionalLight(color,intensity);
  light.position.set(...position);
  scene.add(light);
}
let model;
let wire=false;
let unlit=false;
let currentView='perspective';
let captureSize=null;
const bytes=Uint8Array.from(atob(document.querySelector('#model-data').textContent.trim()),c=>c.charCodeAt(0));
function resize(){
  const [w,h]=captureSize||[innerWidth,innerHeight];
  renderer.setPixelRatio(captureSize?1:Math.min(devicePixelRatio,2));
  renderer.setSize(w,h,!captureSize);
  camera.aspect=w/h;
  camera.updateProjectionMatrix();
  const aspect=w/h;
  const height=['top','bottom'].includes(currentView)?18.5:['front','back'].includes(currentView)?8.0:currentView==='side'?13.0:17.5;
  ortho.left=-height*aspect/2;
  ortho.right=height*aspect/2;
  ortho.top=height/2;
  ortho.bottom=-height/2;
  ortho.updateProjectionMatrix();
}
function view(name){
  currentView=name;
  controls.autoRotate=false;
  document.querySelector('#spin').classList.remove('active');
  document.querySelectorAll('[data-view]').forEach(el=>el.classList.toggle('active',el.dataset.view===name));
  controls.enabled=name==='perspective';
  if(name==='perspective'){
    activeCamera=camera;
    camera.position.set(11,16,23);
    controls.target.set(0,1.5,0);
    camera.lookAt(controls.target);
  }else{
    activeCamera=ortho;
    ortho.up.set(0,1,0);
    if(name==='top'){
      ortho.position.set(0,35,0);
      ortho.up.set(0,0,-1);
    }else if(name==='bottom'){
      ortho.position.set(0,-35,0);
      ortho.up.set(0,0,-1);
    }else if(name==='bottom-quarter'){
      ortho.position.set(16,-21,26);
    }else if(name==='front')ortho.position.set(0,1.5,-35);
    else if(name==='back')ortho.position.set(0,1.5,35);
    else if(name==='side')ortho.position.set(-35,1.5,0);
    else ortho.position.set(16,15,-27);
    ortho.lookAt(0,1.5,0);
  }
  resize();
  controls.update();
  renderer.render(scene,activeCamera);
}
window.addEventListener('resize',resize);
for(const button of document.querySelectorAll('[data-view]'))button.onclick=()=>view(button.dataset.view);
document.querySelector('#wire').onclick=()=>{
  wire=!wire;
  document.querySelector('#wire').classList.toggle('active',wire);
  model.traverse(obj=>{if(obj.isMesh)obj.material.wireframe=wire;});
};
document.querySelector('#unlit').onclick=()=>{
  unlit=!unlit;
  document.querySelector('#unlit').classList.toggle('active',unlit);
  model.traverse(obj=>{
    if(!obj.isMesh)return;
    if(!obj.userData.original)obj.userData.original=obj.material;
    if(unlit){
      const original=obj.userData.original;
      if(!obj.userData.unlit)obj.userData.unlit=new THREE.MeshBasicMaterial({map:original.map,color:original.color});
      obj.material=obj.userData.unlit;
      obj.material.wireframe=wire;
    }else{
      obj.material=obj.userData.original;
      obj.material.wireframe=wire;
    }
  });
};
document.querySelector('#spin').onclick=()=>{
  if(currentView!=='perspective')view('perspective');
  controls.autoRotate=!controls.autoRotate;
  controls.autoRotateSpeed=.65;
  document.querySelector('#spin').classList.toggle('active',controls.autoRotate);
};
function download(data,name,type){
  const a=document.createElement('a');
  a.href=URL.createObjectURL(new Blob([data],{type}));
  a.download=name;
  a.click();
  setTimeout(()=>URL.revokeObjectURL(a.href),2000);
}
document.querySelector('#download').onclick=()=>download(bytes,'tiny-freighter.glb','model/gltf-binary');
document.querySelector('#snapshot').onclick=()=>{
  renderer.render(scene,activeCamera);
  canvas.toBlob(blob=>download(blob,'tiny-freighter-'+currentView+'.png','image/png'));
};

const frames=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
new GLTFLoader().parse(bytes.buffer,'',async gltf=>{
  model=gltf.scene;
  scene.add(model);
  view('perspective');
  document.querySelector('#loading').hidden=true;
  document.documentElement.dataset.modelLoaded='true';
  document.querySelector('#render-count').textContent=renderer.info.render.calls+' draw calls';
  if(new URLSearchParams(location.search).has('capture')){
    document.body.classList.add('capture');
    captureSize=[1600,1200];
    for(const name of ['perspective','top','bottom','front','back','side','front-quarter','bottom-quarter']){
      view(name);
      await frames();
      renderer.render(scene,activeCamera);
      const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));
      await fetch('/capture/'+name+'.png',{method:'POST',body:blob});
    }
    captureSize=null;
    view('perspective');
    document.body.classList.remove('capture');
    document.documentElement.dataset.captureComplete='true';
  }
},error=>{
  document.querySelector('#loading').textContent='Unable to load model: '+error.message;
  console.error(error);
});
renderer.setAnimationLoop(()=>{
  if(controls.enabled)controls.update();
  renderer.render(scene,activeCamera);
});

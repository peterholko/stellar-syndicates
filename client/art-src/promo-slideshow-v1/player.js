import * as THREE from 'three';
import {GLTFLoader} from 'three/addons/loaders/GLTFLoader.js';
import {RoomEnvironment} from 'three/addons/environments/RoomEnvironment.js';
import {createWormholeTraffic} from '../ship-models/wormhole-hub-textured-v1/wormhole-traffic.js';
import {HUB_FLOW_PERIOD,hubFlowMapFragment} from '../ship-models/wormhole-hub-textured-v1/wormhole-flow.js';
import {W,H,slides,duration} from './storyboard.mjs';

const $=id=>document.getElementById(id);
const screen=$('screen'),ctx=screen.getContext('2d',{alpha:false});
const buffer=document.createElement('canvas'); buffer.width=W; buffer.height=H;
const bctx=buffer.getContext('2d',{alpha:false});
const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;
const ease=x=>{x=THREE.MathUtils.clamp(x,0,1);return x*x*(3-2*x);};
const times=[];let sum=0;for(const slide of slides){times.push(sum);sum+=slide.duration;}
let images={},models={},renderer,scene,camera,traffic,keyLight,fillLight,rig;
const videos={};
let elapsed=0,playing=false,ready=false,frame=0,last=0,active=-1,recorder=null,recordChunks=[],recordMime='';
let playBase=0,playStart=0;
let hiddenPause=false,recordAborted=false,recordTimer=0,recordedUrl=null;
const flow={value:0};
const toBytes=s=>Uint8Array.from(atob(s),c=>c.charCodeAt(0));
function timeLabel(s){return `${Math.floor(s/60)}:${String(Math.floor(s%60)).padStart(2,'0')}`;}
function locate(t){for(let i=slides.length-1;i>=0;i--)if(t>=times[i])return {i,p:Math.min(1,(t-times[i])/slides[i].duration)};return {i:0,p:0};}
function download(blob,name){const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),10000);}
function fail(error){console.error(error);$('loading').hidden=false;$('loading').textContent='The promo could not load. Reload to try again.';$('ready').className='status-error';$('ready').textContent=String(error.message||error);}

function normalize(source){
  const bounds=new THREE.Box3().setFromObject(source);const size=bounds.getSize(new THREE.Vector3());
  const center=bounds.getCenter(new THREE.Vector3());source.position.sub(center);
  const outer=new THREE.Group();outer.add(source);outer.scale.setScalar(10/Math.max(size.x,size.y,size.z));
  const holder=new THREE.Group();holder.add(outer);return holder;
}
function setupRenderer(){
  renderer=new THREE.WebGLRenderer({alpha:true,antialias:true,preserveDrawingBuffer:true,powerPreference:'high-performance'});
  renderer.setSize(W,H);renderer.setPixelRatio(1);renderer.setClearColor(0,0);
  renderer.outputColorSpace=THREE.SRGBColorSpace;renderer.toneMapping=THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure=.92;renderer.localClippingEnabled=true;
  scene=new THREE.Scene();camera=new THREE.OrthographicCamera(-20,20,10,-10,.1,200);
  const room=new RoomEnvironment();const pmrem=new THREE.PMREMGenerator(renderer);
  const env=pmrem.fromScene(room,.04);scene.environment=env.texture;scene.environmentIntensity=.6;room.dispose();pmrem.dispose();
  scene.add(new THREE.HemisphereLight(0xd2e8ff,0x263348,.9));
  keyLight=new THREE.DirectionalLight(0xffefd7,2.4);keyLight.position.set(-10,16,14);scene.add(keyLight);
  fillLight=new THREE.DirectionalLight(0x7bbaff,1.7);fillLight.position.set(12,6,-9);scene.add(fillLight);
  const bottom=new THREE.DirectionalLight(0x9ec8ec,.9);bottom.position.set(0,-12,6);scene.add(bottom);
  renderer.domElement.addEventListener('webglcontextlost',e=>{e.preventDefault();setPlaying(false);fail(new Error('3D rendering was interrupted. Reload to restore the models.'));});
}

async function load(){
  try{
    const data=JSON.parse($('assets').textContent);$('assets').remove();
    let done=0;const total=Object.keys(data.images).length+Object.keys(data.models).length+Object.keys(data.video).length;
    const progress=()=>{$('loading').textContent=`Preparing the frontier… ${++done} / ${total}`;};
    await Promise.all(Object.entries(data.images).map(async([id,url])=>{const img=new Image();img.src=url;await img.decode();images[id]=img;progress();}));
    await Promise.all(Object.entries(data.video).map(async([id,b64])=>{
      const video=document.createElement('video');video.muted=true;video.playsInline=true;video.preload='auto';
      await new Promise((resolve,reject)=>{
        video.onloadeddata=resolve;video.onerror=()=>reject(new Error(`${id} video could not load.`));
        video.src=URL.createObjectURL(new Blob([toBytes(b64)],{type:'video/mp4'}));
      });videos[id]=video;progress();
      video.addEventListener('seeked',()=>{if(ready&&!playing)draw();});
    }));
    setupRenderer();
    const loader=new GLTFLoader();
    await Promise.all(Object.entries(data.models).map(async([id,b64])=>{
      const gltf=await loader.parseAsync(toBytes(b64).buffer,'');
      gltf.scene.traverse(o=>{if(o.isMesh){const mats=Array.isArray(o.material)?o.material:[o.material];for(const m of mats){for(const k of ['map','emissiveMap','roughnessMap','metalnessMap'])if(m[k])m[k].anisotropy=4;}}});
      models[id]=gltf.scene;progress();
    }));
    const portal=models.hub.getObjectByName('wormhole');
    portal?.traverse(o=>{if(!o.isMesh)return;o.material.onBeforeCompile=shader=>{shader.uniforms.uHubTime=flow;shader.fragmentShader='uniform float uHubTime;\n'+shader.fragmentShader;shader.fragmentShader=shader.fragmentShader.replace('#include <map_fragment>',hubFlowMapFragment);};o.material.customProgramCacheKey=()=> 'promo-bounded-hub-flow';});
    traffic=createWormholeTraffic({'tiny-freighter':models['tiny-freighter'],'small-freighter':models['small-freighter'],'medium-freighter':models['medium-freighter']});
    models.hub.add(traffic.group);
    for(const id of ['hub','interceptor','cruiser','titan','medium-freighter','small-freighter'])models[id]=normalize(models[id]);
    // This is a fresh display instance. The shared game models and materials remain untouched.
    const build=models.interceptor.clone(true);const built=new THREE.Plane(new THREE.Vector3(0,0,1),20);
    build.traverse(o=>{if(!o.isMesh)return;o.material=o.material.clone();o.material.clippingPlanes=[built];});
    const ghost=models.interceptor.clone(true);
    ghost.traverse(o=>{if(o.isMesh)o.material=new THREE.MeshBasicMaterial({color:0x8cdbff,wireframe:true,transparent:true,opacity:.15,depthWrite:false});});
    rig=new THREE.Group();rig.add(build,ghost);rig.userData={built,ghost};
    const shadow=new THREE.Mesh(new THREE.PlaneGeometry(8,11),new THREE.ShaderMaterial({
      transparent:true,depthWrite:false,
      vertexShader:'varying vec2 vUv; void main(){vUv=uv;gl_Position=projectionMatrix*modelViewMatrix*vec4(position,1.0);}',
      fragmentShader:'varying vec2 vUv; void main(){float d=length(vUv-.5)*2.0;gl_FragColor=vec4(0.0,0.0,0.0,.65*(1.0-smoothstep(.1,1.0,d)));}',
    }));
    shadow.rotation.x=-Math.PI/2;shadow.position.y=-.95;rig.add(shadow);rig.userData.frameObject=build;
    $('loading').textContent='Preparing cinematic views…';
    for(let i=0;i<slides.length;i++){compose(bctx,i,.5);await new Promise(requestAnimationFrame);}
    ready=true;$('loading').hidden=true;
    document.querySelectorAll('button,input').forEach(el=>el.disabled=false);
    $('ready').textContent=`${slides.length} scenes / ${timeLabel(duration)} / Game art + 3D + orbital motion + fleet combat`;
    if(typeof MediaRecorder==='undefined'||!screen.captureStream){$('record').disabled=true;$('record').textContent='Video recording unavailable';}
    updateUI(true);draw();
  }catch(error){fail(error);}
}

function cover(c,img,p=0,scale=1){
  const factor=Math.max(W/img.width,H/img.height)*scale;
  const width=img.width*factor,height=img.height*factor;
  c.drawImage(img,(W-width)/2+(p-.5)*14,(H-height)/2+(p-.5)*8,width,height);
}
function background(c,slide,p){
  c.fillStyle='#030911';c.fillRect(0,0,W,H);
  if(videos[slide.scene]){
    const video=videos[slide.scene];c.drawImage(video.readyState>=2?video:images[slide.art],0,0,W,H);
  }else if(slide.scene==='screenshot'){
    c.drawImage(images[slide.art],0,0,W,H);
  }else if(slide.art==='nebula'){
    c.save();c.globalAlpha=slide.scene==='hub'?.32:.43;
    const width=2020+40*p;
    c.drawImage(images.nebula,350-40*p,-480,width,width);c.restore();
  }else cover(c,images[slide.art],p,1.02+(reduced?0:p*.035));
}
function shade(c,slide){
  if(['combat','capture'].includes(slide.layout)){
    const top=c.createLinearGradient(0,0,0,300);top.addColorStop(0,'rgba(2,7,14,.5)');top.addColorStop(1,'rgba(2,7,14,0)');c.fillStyle=top;c.fillRect(0,0,W,300);return;
  }
  const left=c.createLinearGradient(0,0,W,0);
  left.addColorStop(0,'rgba(2,7,14,.88)');left.addColorStop(.30,'rgba(2,7,14,.58)');left.addColorStop(.58,'rgba(2,7,14,.05)');left.addColorStop(1,'rgba(2,7,14,0)');
  const bottom=c.createLinearGradient(0,0,0,H);
  bottom.addColorStop(0,'rgba(2,7,14,0)');bottom.addColorStop(.53,'rgba(2,7,14,0)');bottom.addColorStop(1,'rgba(2,7,14,.92)');
  if(['bottom'].includes(slide.layout)){c.fillStyle=bottom;c.fillRect(0,0,W,H);}
  else{c.fillStyle=left;c.fillRect(0,0,W,H);if(slide.layout==='closing'){c.fillStyle=bottom;c.fillRect(0,0,W,H);}}
  if(slide.layout==='top'){
    const top=c.createLinearGradient(0,0,0,600);top.addColorStop(0,'rgba(2,7,14,.35)');top.addColorStop(1,'rgba(2,7,14,0)');c.fillStyle=top;c.fillRect(0,0,W,600);
  }
}

function renderModel(c,object,{cx,cy,maxW,maxH,view=[9,8,17],heading=0,warm=false,tilt=0}){
  scene.add(object);object.rotation.set(0,heading,tilt);object.updateMatrixWorld(true);
  keyLight.color.set(warm?0xffc27c:0xffefd7);keyLight.intensity=warm?2.6:2.4;
  camera.position.set(...view).normalize().multiplyScalar(45);camera.up.set(0,1,0);camera.lookAt(0,0,0);camera.updateMatrixWorld(true);
  const bounds=new THREE.Box3().setFromObject(object.userData.frameObject||object),points=[];
  for(const x of [bounds.min.x,bounds.max.x])for(const y of [bounds.min.y,bounds.max.y])for(const z of [bounds.min.z,bounds.max.z])points.push(new THREE.Vector3(x,y,z).applyMatrix4(camera.matrixWorldInverse));
  const projected=new THREE.Box3().setFromPoints(points);const size=projected.getSize(new THREE.Vector3());
  const span=Math.max(size.y/(maxH/H),size.x/(maxW/W)/(W/H));const spanX=span*W/H;
  const center=projected.getCenter(new THREE.Vector3());
  const ox=-(cx/W-.5)*spanX+center.x,oy=(cy/H-.5)*span+center.y;
  camera.left=-spanX/2+ox;camera.right=spanX/2+ox;camera.top=span/2+oy;camera.bottom=-span/2+oy;camera.updateProjectionMatrix();
  renderer.render(scene,camera);c.drawImage(renderer.domElement,0,0);object.removeFromParent();
}
function modelsFor(c,slide,p){
  const drift=reduced?0:p-.5;
  flow.value=((times[slides.indexOf(slide)]+p*slide.duration)*.9)%HUB_FLOW_PERIOD;
  switch(slide.scene){
    case 'hub':renderModel(c,models.hub,{cx:1375,cy:540,maxW:930,maxH:920,view:[5+drift,2.2,18],heading:-.08+drift*.11});break;
    case 'shipyard':{
      const fraction=.28+.72*ease((p-.03)/.77);
      const cut=5-10*fraction;
      // The clip plane tracks the model's heading rather than accumulating transforms.
      rig.rotation.y=-2.35;rig.updateMatrixWorld(true);
      rig.userData.built.set(new THREE.Vector3(0,0,1),-cut).applyMatrix4(rig.matrixWorld);
      rig.userData.ghost.visible=fraction<.999;
      renderModel(c,rig,{cx:1160,cy:715,maxW:1080,maxH:470,view:[0,12,18],heading:-2.35,warm:true});break;
    }
    case 'freight':
      renderModel(c,models['small-freighter'],{cx:1600+drift*70,cy:330,maxW:350,maxH:230,view:[10,9,17],heading:-.8});
      renderModel(c,models['medium-freighter'],{cx:1235+drift*55,cy:575-drift*20,maxW:1170,maxH:630,view:[11,8,17],heading:-.75+drift*.15});break;
    case 'fleet':
      renderModel(c,models.interceptor,{cx:1040+drift*30,cy:280,maxW:350,maxH:200,view:[10,14,16],heading:-.5});
      renderModel(c,models.cruiser,{cx:1320+drift*35,cy:548,maxW:1070,maxH:610,view:[12,12,17],heading:-.55+drift*.16});
      renderModel(c,models.interceptor,{cx:1720+drift*25,cy:825,maxW:280,maxH:180,view:[10,14,16],heading:-.5});break;
    case 'capital':renderModel(c,models.titan,{cx:1220,cy:655,maxW:1110,maxH:580,view:[0,6,18],heading:2.25+drift*.07,warm:true});break;
  }
}
function textRuns(slide){
  const runs=[];
  if(slide.layout==='combat')runs.push({text:'IN-GAME BATTLE REPLAY',x:100,y:48,size:22,color:slide.accent,weight:400,spacing:3});
  if(slide.kicker)runs.push({text:slide.kicker,x:100,y:48,size:22,color:slide.accent,weight:400,spacing:3});
  if(slide.layout==='hero')runs.push({text:'A SPACE STRATEGY GAME',x:120,y:320,size:24,color:slide.accent,weight:400,spacing:4});
  if(!slide.brand)slide.title.forEach((text,i)=>runs.push({text,x:slide.x,y:slide.y+i*slide.leading,size:slide.size,color:i===slide.title.length-1&&slide.title.length>1?slide.accent:'#f3f5f5',weight:700,spacing:-2}));
  slide.subtitle.forEach((text,i)=>runs.push({text,x:slide.x,y:slide.bodyY+i*43,size:31,color:'#d4e1e8',weight:400,spacing:0}));
  return runs;
}
function type(c,slide,p,animated){
  const amount=animated?ease(Math.min(p*slide.duration/.8,1)):1;
  c.save();c.globalAlpha=amount;c.translate(0,animated?(1-amount)*20:0);c.textBaseline='top';
  if(slide.brand){const b=slide.brand,img=images[b.art];c.drawImage(img,b.x,b.y,b.width,b.width*img.height/img.width);}
  for(const run of textRuns(slide)){c.font=`${run.weight} ${run.size}px Arial`;c.letterSpacing=`${run.spacing}px`;c.fillStyle=run.color;c.fillText(run.text,run.x,run.y);}
  c.restore();
}
function compose(c,i,p,{text=true,animated=false}={}){
  const slide=slides[i];background(c,slide,p);modelsFor(c,slide,p);shade(c,slide);
  if(text)type(c,slide,p,animated);
}
function syncVideos(i,p,on){
  for(const [id,video] of Object.entries(videos)){
    const current=slides[i].scene===id;
    const upcoming=i<slides.length-1&&slides[i+1].scene===id&&slides[i].duration*(1-p)<.8;
    if(!current&&!upcoming){video.pause();continue;}
    const t=current?Math.min(p*slides[i].duration,Math.max(0,video.duration-.04)):0;
    if(!video.seeking&&Math.abs(video.currentTime-t)>(on?.25:.025))video.currentTime=t;
    if(on&&current){if(video.paused)video.play().catch(fail);}else video.pause();
  }
}
function draw(){
  if(!ready)return;
  const {i,p}=locate(elapsed);syncVideos(i,p,playing);compose(ctx,i,p,{animated:playing});
  const remaining=slides[i].duration*(1-p);
  if(playing&&remaining<.7&&i<slides.length-1){compose(bctx,i+1,0,{text:false});ctx.save();ctx.globalAlpha=ease(1-remaining/.7);ctx.drawImage(buffer,0,0);ctx.restore();}
  if(playing&&i===slides.length-1&&elapsed>duration-.7){ctx.fillStyle=`rgba(2,6,11,${.65*ease((elapsed-duration+.7)/.7)})`;ctx.fillRect(0,0,W,H);}
  updateUI();
}
function updateUI(force=false){
  const {i}=locate(elapsed);
  if(force||active!==i){
    active=i;const slide=slides[i];const text=`${slide.title.join(' ')}. ${slide.subtitle.join(' ')}`;
    screen.setAttribute('aria-label',`Scene ${i+1} of ${slides.length}: ${text}`);$('description').textContent=text;
    document.querySelectorAll('.chapter').forEach((b,n)=>b.setAttribute('aria-current',String(n===i)));
  }
  $('clock').textContent=`${timeLabel(elapsed)} / ${timeLabel(duration)}`;$('seek').value=String(elapsed);
  document.querySelectorAll('.chapter').forEach((b,n)=>b.style.setProperty('--fill',`${THREE.MathUtils.clamp((elapsed-times[n])/slides[n].duration,0,1)*100}%`));
}
function tick(now){
  frame=0;if(!playing||document.hidden)return;
  if(now-last<1000/30){frame=requestAnimationFrame(tick);return;}
  const dt=Math.min((now-last)/1000,.12);last=now;elapsed=Math.min(duration,playBase+(now-playStart)/1000);traffic?.update(dt);
  draw();
  if(elapsed>=duration){setPlaying(false);if(recorder?.state==='recording')stopRecording();else{elapsed=duration;draw();$('play').textContent='Replay promo';}return;}
  frame=requestAnimationFrame(tick);
}
function setPlaying(on){
  playing=on;$('play').textContent=on?'Pause':'Play promo';
  if(on){last=performance.now();playStart=last;playBase=elapsed;if(!frame)frame=requestAnimationFrame(tick);}else{cancelAnimationFrame(frame);frame=0;Object.values(videos).forEach(video=>video.pause());}
}
function seek(t){if(recorder?.state==='recording')return;setPlaying(false);elapsed=THREE.MathUtils.clamp(t,0,duration-.01);draw();}
function jump(i){i=THREE.MathUtils.clamp(i,0,slides.length-1);seek(times[i]+slides[i].duration*.5);}
function lockControls(lock){for(const id of ['play','prev','next','seek','still'])$(id).disabled=lock;document.querySelectorAll('.chapter').forEach(b=>b.disabled=lock);}

function stopRecording(){
  clearInterval(recordTimer);if(recorder?.state==='recording')recorder.stop();setPlaying(false);
}
function record(){
  if(recorder?.state==='recording'){recordAborted=true;stopRecording();return;}
  const types=['video/mp4;codecs=avc1.42E01E','video/webm;codecs=vp9','video/webm;codecs=vp8','video/webm'];
  recordMime=types.find(type=>MediaRecorder.isTypeSupported(type));
  if(!recordMime){$('record-status').textContent='Video recording is unavailable in this browser.';return;}
  setPlaying(false);elapsed=0;draw();recordChunks=[];recordAborted=false;
  if(recordedUrl){URL.revokeObjectURL(recordedUrl);recordedUrl=null;}
  const stream=screen.captureStream(30);
  recorder=new MediaRecorder(stream,{mimeType:recordMime,videoBitsPerSecond:10000000});
  recorder.ondataavailable=e=>{if(e.data.size)recordChunks.push(e.data);};
  recorder.onstop=()=>{
    stream.getTracks().forEach(t=>t.stop());lockControls(false);$('record').textContent='Record 1080p video';
    if(recordAborted){$('record-status').textContent='Recording cancelled. Keep the tab visible to record.';return;}
    const ext=recordMime.startsWith('video/mp4')?'mp4':'webm';
    recordedUrl=URL.createObjectURL(new Blob(recordChunks,{type:recordMime}));
    const save=document.createElement('a');save.href=recordedUrl;save.download=`stellar-syndicates-promo-v09-1080p.${ext}`;save.textContent='Save video';
    $('record-status').textContent='Video ready. ';$('record-status').append(save);
    elapsed=duration;draw();$('play').textContent='Replay promo';
  };
  recorder.onerror=event=>{recordAborted=true;stopRecording();$('record-status').textContent='Recording stopped. Try again in a Chromium browser.';console.error(event.error);};
  recorder.start(1000);lockControls(true);$('record').textContent='Cancel recording';$('exports').open=false;
  recordTimer=setInterval(()=>{$('record-status').textContent=`Recording ${timeLabel(elapsed)} / ${timeLabel(duration)}`;},500);
  setPlaying(true);
}

slides.forEach((slide,i)=>{const b=document.createElement('button');b.className='chapter';b.disabled=true;b.setAttribute('aria-label',`Scene ${i+1}: ${slide.chapter}`);b.innerHTML=`<small>${String(i+1).padStart(2,'0')}</small>${slide.chapter}`;b.addEventListener('click',()=>jump(i));$('chapters').append(b);});
$('play').addEventListener('click',()=>{if(elapsed>=duration-.1)elapsed=0;setPlaying(!playing);});
$('next').addEventListener('click',()=>jump(locate(elapsed).i+1));$('prev').addEventListener('click',()=>jump(locate(elapsed).i-1));
$('seek').addEventListener('input',e=>seek(Number(e.target.value)));
function closeCinema(){document.body.classList.remove('cinema');if(document.fullscreenElement)document.exitFullscreen().catch(()=>{});}
$('full').addEventListener('click',async()=>{document.body.classList.add('cinema');try{if(document.fullscreenEnabled)await $('stage').requestFullscreen();}catch{/* The cinema layout also works in embedded browsers. */}});
$('exit-cinema').addEventListener('click',closeCinema);
$('record').addEventListener('click',record);
$('still').addEventListener('click',()=>{setPlaying(false);const {i,p}=locate(elapsed);compose(bctx,i,p,{text:$('include-text').checked});buffer.toBlob(blob=>download(blob,`${String(i+1).padStart(2,'0')}-${slides[i].id}${$('include-text').checked?'':'-artwork'}.png`),'image/png');});
document.addEventListener('keydown',e=>{if(e.code==='Escape'){closeCinema();return;}if(!ready||recorder?.state==='recording'||['INPUT','SUMMARY'].includes(e.target.tagName))return;if(e.code==='Space'){e.preventDefault();$('play').click();}if(e.code==='ArrowRight'){e.preventDefault();$('next').click();}if(e.code==='ArrowLeft'){e.preventDefault();$('prev').click();}if(e.code==='Home'){e.preventDefault();seek(0);}});
document.addEventListener('visibilitychange',()=>{
  if(document.hidden){hiddenPause=playing;if(recorder?.state==='recording'){recordAborted=true;stopRecording();hiddenPause=false;}setPlaying(false);}
  else if(hiddenPause){hiddenPause=false;setPlaying(true);}
});
load();

// Capture the production renderer at explicit simulation times. Absolute time
// keeps replay, seeking, and repeated exports deterministic without game commands.
import {Application,Assets} from 'pixi.js';
import {SystemViewScene,buildVisualSystem} from '../../src/systemview';
import {starTypeFor} from '../../src/stars';
import {starArtwork,starArtUrl} from '../../src/starart';
import {bodies,system} from './world-demo.js';
const $=id=>document.getElementById(id);
const W=1920,H=1080,SECONDS=12,SPEED=40;
const app=new Application();
await app.init({width:W,height:H,resolution:1,antialias:true,background:'#050a12',preserveDrawingBuffer:true,autoStart:false});
$('scene').append(app.canvas);
const view=new SystemViewScene();app.stage.addChild(view.root);
const star=starTypeFor(system.id),art=starArtwork('system',star.slug);
const texture=await Assets.load(starArtUrl('system',star.slug,1254));
view.setSystem(buildVisualSystem(system,bodies),texture,art);
view.root.visible=true;view.layout(W,H);view.zoomAt(W/2,H/2,1.65);
let seconds=0,playing=false,start=0,last=0,recorder=null,chunks=[],aborted=false,downloadUrl=null;
function draw(now){view.update('promo','promo',now,seconds*SPEED);app.render();}
// Allow the production camera's easing to settle before recording any frames.
for(let n=0;n<90;n++)draw(n*1000/30);
function controls(lock){for(const id of ['play','still','seek'])$(id).disabled=lock;}
function tick(now){
  requestAnimationFrame(tick);
  if(document.hidden||now-last<1000/30)return;last=now;
  if(playing)seconds=Math.min(SECONDS,(now-start)/1000);
  draw(now);$('seek').value=String(seconds);
  $('status').textContent=`${playing?'Playing':'Ready'} · ${seconds.toFixed(1)} / ${SECONDS} seconds · Orbital motion 40×${recorder?.state==='recording'?' · Recording':''}`;
  if(playing&&seconds>=SECONDS){playing=false;$('play').textContent='Play 12-second clip';if(recorder?.state==='recording')recorder.stop();}
}
$('play').onclick=()=>{playing=!playing;if(playing){if(seconds>=SECONDS)seconds=0;start=performance.now()-seconds*1000;}$('play').textContent=playing?'Pause clip':'Play 12-second clip';};
$('seek').oninput=()=>{seconds=Number($('seek').value);playing=false;$('play').textContent='Play 12-second clip';draw(performance.now());};
$('record').onclick=()=>{
  if(recorder?.state==='recording'){aborted=true;recorder.stop();playing=false;return;}
  const mime=['video/mp4;codecs=avc1.42E01E','video/webm;codecs=vp9','video/webm'].find(t=>MediaRecorder.isTypeSupported(t));
  if(!mime){$('status').textContent='Video recording is unavailable in this browser.';return;}
  seconds=0;playing=false;draw(performance.now());chunks=[];aborted=false;
  if(downloadUrl)URL.revokeObjectURL(downloadUrl);$('download').textContent='';
  const stream=app.canvas.captureStream(30);recorder=new MediaRecorder(stream,{mimeType:mime,videoBitsPerSecond:12000000});
  recorder.ondataavailable=e=>{if(e.data.size)chunks.push(e.data);};
  recorder.onstop=()=>{
    stream.getTracks().forEach(t=>t.stop());controls(false);$('record').textContent='Record orbital clip';
    if(aborted){$('download').textContent='Recording cancelled. Keep this tab active to record.';return;}
    downloadUrl=URL.createObjectURL(new Blob(chunks,{type:mime}));
    const a=document.createElement('a');a.href=downloadUrl;a.download=`stellar-syndicates-star-system-orbits.${mime.startsWith('video/mp4')?'mp4':'webm'}`;a.textContent='Save orbital video';$('download').append(a);
  };
  recorder.onerror=()=>{aborted=true;playing=false;if(recorder.state==='recording')recorder.stop();$('download').textContent='Recording failed. Try again.';};
  recorder.start(1000);controls(true);$('record').textContent='Cancel recording';start=performance.now();playing=true;
};
$('still').onclick=()=>{playing=false;$('play').textContent='Play 12-second clip';app.canvas.toBlob(blob=>{const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download='star-system-orbits-poster.png';a.click();setTimeout(()=>URL.revokeObjectURL(url),60000);});};
document.addEventListener('visibilitychange',()=>{if(document.hidden){playing=false;if(recorder?.state==='recording'){aborted=true;recorder.stop();}}});
for(const element of document.querySelectorAll('button,input'))element.disabled=false;
document.documentElement.dataset.ready='true';requestAnimationFrame(tick);

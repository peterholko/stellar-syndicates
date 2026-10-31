// This capture harness replays a checked-in engine battle through the unchanged
// production theater. No socket, game session, or synthetic combat animation.
import {theaterAttach,theaterSetTime,theaterStep,theaterSetView,theaterDebug,theaterClose} from '../../src/battletheater';
const $=id=>document.getElementById(id);
const output=$('output'),context=output.getContext('2d',{alpha:false});
const CLIP_SECONDS=16,START_ROUND=45,ROUNDS_PER_SECOND=1.5;
const record=await fetch('/dev/battles/mixed.json').then(r=>r.json());
let seconds=6,playing=false,start=0,last=0,ready=false,recorder=null,chunks=[],aborted=false;
theaterAttach($('source'),record,null,{width:1600,height:900,maxFps:30});
theaterSetView(true);
function save(blob,name){const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),60000);}
function draw(){
  const round=START_ROUND+seconds*ROUNDS_PER_SECOND;
  theaterSetTime(Math.floor(round),round%1,false);theaterStep(1/30);
  const canvas=$('source').querySelector('canvas');
  if(canvas){context.fillStyle='#05090f';context.fillRect(0,0,1920,1080);context.drawImage(canvas,0,0,1920,1080);}
}
function tick(now){
  requestAnimationFrame(tick);
  if(document.hidden||now-last<1000/30)return;last=now;
  if(playing)seconds=Math.min(CLIP_SECONDS,(now-start)/1000);
  draw();
  const debug=theaterDebug(),models=debug?.models?.runtime;
  if(!ready&&models?.healthy&&models.cached.length>=7&&!models.loading.length){ready=true;for(const b of document.querySelectorAll('button'))b.disabled=false;}
  $('status').textContent=ready?`${playing?'Playing':'Ready'} · ${seconds.toFixed(1)} / ${CLIP_SECONDS} seconds · ${models?.ships??0} 3D ships · ${models?.cached.length??0} hull types${recorder?.state==='recording'?' · Recording':''}`:'Loading the game’s combat models…';
  if(playing&&seconds>=CLIP_SECONDS){playing=false;if(recorder?.state==='recording')recorder.stop();$('play').textContent='Play 16-second clip';}
}
$('play').onclick=()=>{if(playing){playing=false;$('play').textContent='Play 16-second clip';return;}seconds=0;start=performance.now();playing=true;$('play').textContent='Pause clip';};
$('record').onclick=()=>{
  if(recorder?.state==='recording'){aborted=true;recorder.stop();playing=false;return;}
  const mime=['video/mp4;codecs=avc1.42E01E','video/webm;codecs=vp9','video/webm'].find(t=>MediaRecorder.isTypeSupported(t));
  seconds=0;playing=false;draw();chunks=[];aborted=false;
  const stream=output.captureStream(30);recorder=new MediaRecorder(stream,{mimeType:mime,videoBitsPerSecond:16000000});
  recorder.ondataavailable=e=>{if(e.data.size)chunks.push(e.data);};
  recorder.onstop=()=>{stream.getTracks().forEach(t=>t.stop());$('record').textContent='Record combat clip';$('play').disabled=false;$('still').disabled=false;if(!aborted)save(new Blob(chunks,{type:mime}),`stellar-syndicates-fleet-combat.${mime.startsWith('video/mp4')?'mp4':'webm'}`);};
  recorder.start(1000);$('record').textContent='Cancel recording';$('play').disabled=true;$('still').disabled=true;start=performance.now();playing=true;
};
$('still').onclick=()=>{playing=false;output.toBlob(b=>save(b,'fleet-combat-poster.png'));};
$('zoom').onclick=()=>{const canvas=$('source').querySelector('canvas'),r=canvas.getBoundingClientRect();canvas.dispatchEvent(new WheelEvent('wheel',{deltaY:-300,clientX:r.x+r.width/2,clientY:r.y+r.height/2,cancelable:true}));draw();};
document.addEventListener('visibilitychange',()=>{if(document.hidden){playing=false;if(recorder?.state==='recording'){aborted=true;recorder.stop();}}});
window.addEventListener('pagehide',theaterClose);
requestAnimationFrame(tick);

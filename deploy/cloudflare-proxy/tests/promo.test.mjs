import assert from 'node:assert/strict';
import worker from '../src/promo.ts';

const data=Uint8Array.from({length:256},(_,i)=>i);
const metadata={size:data.length,httpEtag:'"promo-v09"',uploaded:new Date('2026-10-31T08:00:00Z'),writeHttpMetadata(){}};
let reads=0;
const env={
  PROMO_VIDEO_KEY:'stellar-syndicates-promo-v09-web.mp4',
  ASSETS:{fetch:async()=>new Response('Not found',{status:404})},
  MEDIA:{head:async()=>metadata,get:async(_key,{range}={})=>{
    reads++;
    const bytes=range?data.slice(range.offset,range.offset+range.length):data;
    return {...metadata,body:new Response(bytes).body};
  }},
};
const request=(headers={},method='GET',path=`/${env.PROMO_VIDEO_KEY}`)=>worker.fetch(new Request(`https://promo.stellarsyndicates.com${path}`,{method,headers}),env);
for(const [range,start,end] of [['bytes=0-31',0,31],['bytes=200-',200,255],['bytes=-16',240,255],['bytes=250-999',250,255],['bytes=-999',0,255]]){
  const response=await request({Range:range});
  assert.equal(response.status,206,range);
  assert.equal(response.headers.get('Content-Range'),`bytes ${start}-${end}/256`);
  assert.equal(Number(response.headers.get('Content-Length')),end-start+1);
  assert.deepEqual(new Uint8Array(await response.arrayBuffer()),data.slice(start,end+1));
}
for(const range of ['bytes=256-','bytes=80-40','bytes=-0']){
  const response=await request({Range:range});
  assert.equal(response.status,416);
  assert.equal(response.headers.get('Content-Range'),'bytes */256');
}
let response=await request();
assert.equal(response.status,200);
assert.deepEqual(new Uint8Array(await response.arrayBuffer()),data);
response=await request({Range:'bytes=0-3','If-Range':'"old-version"'});
assert.equal(response.status,200,'stale If-Range requests receive the complete representation');
response=await request({Range:'bytes=0-3','If-Range':metadata.httpEtag});
assert.equal(response.status,206);
const before=reads;
response=await request({'If-None-Match':'W/"promo-v09"'});
assert.equal(response.status,304);
response=await request({Range:'bytes=0-3'},'HEAD');
assert.equal(response.status,200);
assert.equal(response.headers.get('Content-Length'),'256');
assert.equal((await response.arrayBuffer()).byteLength,0);
assert.equal(reads,before,'HEAD and cache validation never read the video body');
response=await request({},'GET','/video.mp4');
assert.equal(response.status,302);
assert.equal(response.headers.get('Location'),`/${env.PROMO_VIDEO_KEY}`);
assert.equal((await request({},'GET','/private-file.mp4')).status,404);
assert.equal((await request({},'PUT')).status,405);
console.log('Promo streaming: byte ranges, seeks, full playback, HEAD, cache validation, fixed public key and read-only methods passed.');

import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import crypto from 'node:crypto';
import * as THREE from 'three';
import { createWormholeTraffic, trafficRoute } from './wormhole-traffic.js';
import { readInspectionModel } from './traffic-inspection.mjs';

const root=new URL('./',import.meta.url);
const manifest=JSON.parse(await fs.readFile(new URL('traffic-models/manifest.json',root),'utf8'));
const templates=Object.fromEntries(await Promise.all(manifest.ships.map(async ship => {
  const file=new URL(ship.file,root);
  const bytes=await fs.readFile(file);
  assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),ship.sha256);
  return [ship.id,await readInspectionModel(file)];
})));
const hullRadii=Object.fromEntries(Object.entries(templates).map(([id,model]) => {
  const size=new THREE.Box3().setFromObject(model).getSize(new THREE.Vector3());
  return [id,size.length()/size.z/2];
}));
const traffic=createWormholeTraffic(templates);
assert.equal(traffic.stats.ships,9);
assert.deepEqual(traffic.stats.shipCounts,{'tiny-freighter':3,'small-freighter':3,'medium-freighter':3});
assert.ok(Math.max(...traffic.stats.shipLengths)/17.1<.025,'Freighters remain small beside the hub');
assert.ok(traffic.stats.maximumTriangles<60000,'Keep the nine low-detail freighters within the traffic budget');
const initial=traffic.snapshot();
assert.ok(initial.some(s=>s.visible));
traffic.setEnabled(false); traffic.update(.05);
assert.equal(traffic.active,false);
assert.deepEqual(traffic.snapshot().map(s=>s.clock),initial.map(s=>s.clock));
traffic.setEnabled(true); traffic.update(.05);
assert.ok(traffic.snapshot()[0].clock>initial[0].clock);

// Check envelopes against the actual exported hub, excluding energy surfaces.
const station=await readInspectionModel(new URL('wormhole-hub.glb',root));
station.updateMatrixWorld(true);
const solid=[];
const membrane=[];
station.traverse(node=>{
  if (!node.isMesh) return;
  for (let p=node;p;p=p.parent) {
    if (p.name==='wormhole') { membrane.push(node); return; }
    if (p.name==='containment-field') return;
  }
  solid.push(node);
});
const ray=new THREE.Raycaster(), direction=new THREE.Vector3(0,0,-1);
let clearanceSamples=0, minimumClearance=Infinity;
function checkClearance(position,radius) {
  for (const [dx,dy] of [[0,0],[radius,0],[-radius,0],[0,radius],[0,-radius]]) {
    ray.set(new THREE.Vector3(position.x+dx,position.y+dy,20),direction);
    const hit=ray.intersectObjects(solid,false)[0];
    if (hit) {
      const gap=position.z-radius-hit.point.z;
      minimumClearance=Math.min(minimumClearance,gap);
      assert.ok(gap>.1,`Freighter intersects hardware: ${position.toArray()}, clearance ${gap}`);
    }
    clearanceSamples++;
  }
}
for (const ship of initial) for (let i=0;i<=60;i++) {
  checkClearance(trafficRoute(i/60,ship.approachAngle),hullRadii[ship.type]*ship.length);
}
let separationSamples=0, minimumHullGap=Infinity;
function checkSeparation(ships) {
  for (let i=0;i<ships.length;i++) for (let j=i+1;j<ships.length;j++) {
    const a=ships[i],b=ships[j];
    if (!a.visible || !b.visible || !a.renderedLength || !b.renderedLength) continue;
    const gap=new THREE.Vector3(...a.position).distanceTo(new THREE.Vector3(...b.position))
      -hullRadii[a.type]*a.renderedLength-hullRadii[b.type]*b.renderedLength;
    minimumHullGap=Math.min(minimumHullGap,gap);
    assert.ok(gap>.02,`Freighters overlap: ${a.id}/${b.id}, gap ${gap}`);
    separationSamples++;
  }
}
let arrivals=0,maxVisible=0,previous=traffic.snapshot(),portalClearanceSamples=0,minimumPortalGap=Infinity;
for (let frame=0;frame<3600;frame++) {
  traffic.update(.05);
  const ships=traffic.snapshot();
  maxVisible=Math.max(maxVisible,ships.filter(s=>s.visible).length);
  checkSeparation(ships);
  for (const ship of ships) {
    assert.ok(ship.position.every(Number.isFinite));
    assert.ok(Math.abs(new THREE.Quaternion(...ship.quaternion).length()-1)<1e-6);
    if (ship.clock/26>=.969) assert.equal(ship.visible,false,'Freighters are consumed at entry');
    if (previous[ship.id].visible && !ship.visible) arrivals++;
    if (ship.visible && Math.hypot(...ship.position.slice(0,2))<3.4) {
      const hullRadius=ship.renderedLength*hullRadii[ship.type];
      for (const [dx,dy] of [[0,0],[hullRadius,0],[-hullRadius,0],[0,hullRadius],[0,-hullRadius]]) {
        ray.set(new THREE.Vector3(ship.position[0]+dx,ship.position[1]+dy,20),direction);
        const hit=ray.intersectObjects(membrane,false)[0];
        assert.ok(hit,'Arrival must remain inside the real curved membrane');
        const gap=ship.position[2]-hullRadius-hit.point.z;
        minimumPortalGap=Math.min(minimumPortalGap,gap);
        assert.ok(gap>0,'Ships must be consumed before crossing the curved membrane');
        portalClearanceSamples++;
      }
    }
  }
  previous=ships;
}
assert.ok(arrivals>=43,'Freighters must repeatedly enter and respawn');
assert.equal(maxVisible,7);
traffic.setPower(0);
assert.ok(traffic.snapshot().some(s=>s.state==='diverting'));
for (let frame=0;frame<700;frame++) {
  traffic.update(.05);
  checkSeparation(traffic.snapshot());
  if (frame%15===0) for (const ship of traffic.snapshot()) {
    if (ship.visible) checkClearance(new THREE.Vector3(...ship.position),hullRadii[ship.type]*ship.renderedLength);
  }
}
assert.equal(traffic.active,false,'Closed-portal traffic settles without a perpetual render loop');
const holding=traffic.snapshot();
assert.ok(holding.every(s=>!s.visible || s.state==='holding'));
for (const ship of holding.filter(s=>s.visible)) {
  assert.ok(Math.hypot(ship.position[0],ship.position[1])>3.8);
  assert.ok(ship.position[2]>1.5);
}
traffic.setPower(1);
assert.equal(traffic.active,true);
for (let frame=0;frame<1440;frame++) { traffic.update(.05); checkSeparation(traffic.snapshot()); }
const beforeInvalid=traffic.snapshot();
for (const delta of [0,-1,NaN,Infinity]) traffic.update(delta);
assert.deepEqual(traffic.snapshot(),beforeInvalid);

// Check the offline deliverable carries the current hub and every ship.
const html=await fs.readFile(new URL('viewer.html',root),'utf8');
const embedded=JSON.parse(html.match(/window\.HUB_ASSET=(\{.*?\});<\/script>/s)[1]);
assert.deepEqual(Buffer.from(embedded.glb,'base64'),await fs.readFile(new URL('wormhole-hub.glb',root)));
assert.deepEqual(Object.keys(embedded.traffic).sort(),manifest.ships.map(s=>s.id).sort());
for (const ship of manifest.ships) {
  assert.deepEqual(Buffer.from(embedded.traffic[ship.id],'base64'),await fs.readFile(new URL(ship.file,root)));
}
assert.ok(html.includes('id="traffic"'));
assert.ok(!/<script[^>]+src=/i.test(html));
const report={...traffic.stats,maximumVisible:maxVisible,recurringArrivals:arrivals,
  clearanceSamples,minimumPhysicalClearance:Number(minimumClearance.toFixed(4)),
  separationSamples,minimumHullGap:Number(minimumHullGap.toFixed(4)),
  portalClearanceSamples,minimumPortalGap:Number(minimumPortalGap.toFixed(4)),
  portalOffDiversionAndHolding:true,portalRestoreAndTrafficPause:true,
  noShipsEmergeBehindPortal:true,offlinePayloadVerified:true,
  method:'Real GLTFLoader mesh/material parsing with image-decoding placeholders; Three.js instancing, flight simulation, bounding hull separation and ray intersections against the exported hub'};
await fs.writeFile(new URL('traffic-validation.json',root),JSON.stringify(report,null,2)+'\n');
traffic.dispose();
assert.equal(traffic.active,false); assert.equal(traffic.group.children.length,0);
console.log('PASS:',JSON.stringify(report,null,2));

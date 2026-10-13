import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import * as THREE from 'three';
import { createWormholeTraffic, trafficRoute } from './wormhole-traffic.js';
import { readInspectionModel, saveTrafficSnapshot } from './traffic-inspection.mjs';

const root = new URL('./',import.meta.url);
const manifest = JSON.parse(await fs.readFile(new URL('traffic-models/manifest.json',root),'utf8'));
const templates = Object.fromEntries(await Promise.all(manifest.ships.map(async ship =>
  [ship.id,await readInspectionModel(new URL(ship.file,root))])));
const hullRadii = Object.fromEntries(Object.entries(templates).map(([id,template]) => {
  const size = new THREE.Box3().setFromObject(template).getSize(new THREE.Vector3());
  return [id,size.length()/size.z/2];
}));
const traffic = createWormholeTraffic(templates);
assert.equal(traffic.stats.ships,10);
assert.deepEqual(traffic.stats.shipCounts,{'tiny-freighter':3,interceptor:3,'small-freighter':2,'medium-freighter':2});
assert.equal(traffic.stats.renderSections,29,'Ship material batches plus shared wakes/ripples');
assert.ok(traffic.stats.maximumTriangles < 45000,'Use the lowest-detail approved hulls');
assert.ok(Math.max(...traffic.stats.shipLengths)/19.8 < .019,'Ships must be tiny beside the station');
const initial = traffic.snapshot();
assert.ok(initial.some(s => s.visible));
traffic.setEnabled(false);
traffic.update(.05);
assert.equal(traffic.active,false);
assert.deepEqual(traffic.snapshot().map(s=>s.clock),initial.map(s=>s.clock),'Disabled traffic freezes');
traffic.setEnabled(true);
traffic.update(.05);
assert.ok(traffic.snapshot()[0].clock > initial[0].clock);

// Inspect the real exported station, keeping field/membrane out of the physical
// obstacle set. Rays sample a clearance envelope around every approach route.
const station = await readInspectionModel(new URL('authority-nexus.glb',root));
station.updateMatrixWorld(true);
const solid = [];
station.traverse(node => {
  if (!node.isMesh) return;
  for (let p=node;p;p=p.parent) if (['wormhole','containment-field'].includes(p.userData.assembly)) return;
  solid.push(node);
});
const ray = new THREE.Raycaster(), direction = new THREE.Vector3(0,-1,0);
let clearanceSamples = 0, minimumClearance = Infinity;
function checkClearance(position,radius) {
  for (const [dx,dz] of [[0,0],[radius,0],[-radius,0],[0,radius],[0,-radius]]) {
    ray.set(new THREE.Vector3(position.x+dx,20,position.z+dz),direction);
    const hit = ray.intersectObjects(solid,false)[0];
    if (hit) {
      const clearance = position.y-radius-hit.point.y;
      minimumClearance = Math.min(minimumClearance,clearance);
      assert.ok(clearance > .1,`Traffic intersects physical station: ${position.toArray()} (${clearance})`);
    }
    clearanceSamples++;
  }
}
for (const ship of initial) for (let j=0;j<=40;j++) {
  checkClearance(trafficRoute(j/40,ship.approachAngle),hullRadii[ship.type]*ship.length);
}

let separationSamples = 0, minimumHullGap = Infinity;
function checkSeparation(snapshot) {
  for (let i=0;i<snapshot.length;i++) for (let j=i+1;j<snapshot.length;j++) {
    const a=snapshot[i], b=snapshot[j];
    if (!a.visible || !b.visible || !a.renderedLength || !b.renderedLength) continue;
    const distance = new THREE.Vector3(...a.position).distanceTo(new THREE.Vector3(...b.position));
    const gap = distance-hullRadii[a.type]*a.renderedLength-hullRadii[b.type]*b.renderedLength;
    minimumHullGap = Math.min(minimumHullGap,gap);
    assert.ok(gap > .02,`Traffic hulls overlap: ${a.id}/${b.id}, gap ${gap}`);
    separationSamples++;
  }
}
let maximumVisible = 0;
let arrivals = 0, previous = traffic.snapshot();
for (let frame=0;frame<3600;frame++) {
  traffic.update(.05);
  const snapshot = traffic.snapshot();
  maximumVisible = Math.max(maximumVisible,snapshot.filter(s=>s.visible).length);
  checkSeparation(snapshot);
  for (const ship of snapshot) {
    assert.ok(ship.position.every(Number.isFinite));
    if (ship.clock/26 >= .969) assert.equal(ship.visible,false,'Ships disappear at the horizon, including from underneath');
    if (previous[ship.id].visible && !ship.visible) arrivals++;
  }
  previous = snapshot;
}
assert.ok(arrivals >= 48,'The expanded traffic must repeatedly enter and respawn');
assert.equal(maximumVisible,7,'Busier cargo traffic stays small and uncrowded');
traffic.setPower(.8);
assert.ok(traffic.snapshot().some(s=>s.state==='diverting'),'Ships near the aperture turn back when containment weakens');
for (let frame=0;frame<700;frame++) {
  traffic.update(.05);
  checkSeparation(traffic.snapshot());
  if (frame%15===0) for (const ship of traffic.snapshot()) {
    if (ship.visible) checkClearance(new THREE.Vector3(...ship.position),hullRadii[ship.type]*ship.length);
  }
}
assert.equal(traffic.active,false,'Settled holding traffic does not keep rendering');
const holding = traffic.snapshot();
assert.ok(holding.every(s=>!s.visible || s.state==='holding'));
for (const ship of holding.filter(s=>s.visible)) {
  assert.ok(Math.hypot(ship.position[0],ship.position[2]) > 4.8,'Holding ships stay outside the closing aperture');
}
for (let a=0;a<holding.length;a++) for (let b=a+1;b<holding.length;b++) {
  if (!holding[a].visible || !holding[b].visible) continue;
  assert.ok(new THREE.Vector3(...holding[a].position).distanceTo(new THREE.Vector3(...holding[b].position)) > .4,'Separate holding positions');
}
traffic.setPower(1);
assert.equal(traffic.active,true);
traffic.update(.05);
assert.ok(traffic.snapshot().some((s,i)=>s.clock>holding[i].clock),'Restoring containment resumes traffic');
// Holding lanes must remain separated while the larger freighters resume.
let maximumVisibleAfterRestore = 0;
for (let frame=0;frame<1440;frame++) {
  traffic.update(.05);
  const snapshot = traffic.snapshot();
  maximumVisibleAfterRestore = Math.max(maximumVisibleAfterRestore,snapshot.filter(s=>s.visible).length);
  checkSeparation(snapshot);
}
const beforeInvalid = traffic.snapshot();
for (const dt of [0,-1,NaN,Infinity]) traffic.update(dt);
assert.deepEqual(traffic.snapshot(),beforeInvalid);

const report = {steadyVisibleAtOnce:maximumVisible,visibleAfterRestore:maximumVisibleAfterRestore,...traffic.stats,
  recurringArrivalsVerified:arrivals,clearanceSamples,minimumPhysicalClearance:Number(minimumClearance.toFixed(4)),
  separationSamples,minimumHullGap:Number(minimumHullGap.toFixed(4)),
  closingDiversionAndHold:true,restoreAndPause:true,
  validation:'Real GLTFLoader geometry/material parsing with image-decoding placeholders; real Three.js instancing, route simulation and ray intersections against exported station solids. Browser interaction not exercised.'};
await fs.writeFile(new URL('traffic-validation.json',root),JSON.stringify(report,null,2)+'\n');
traffic.dispose(); assert.equal(traffic.active,false); assert.equal(traffic.group.children.length,0);

// Render inspection uses actual merged geometry, UVs and instance transforms.
const preview = createWormholeTraffic(templates);
await saveTrafficSnapshot(preview,new URL('previews/traffic-snapshot.json',root));
preview.dispose();
console.log('PASS:',JSON.stringify(report));

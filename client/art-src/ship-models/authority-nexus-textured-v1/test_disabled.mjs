import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import * as THREE from 'three';
import { readInspectionGltf, readInspectionModel } from './traffic-inspection.mjs';
import { createWormholeTraffic } from './wormhole-traffic.js';
import { createNexusState } from './nexus-state.js';

const root = new URL('./',import.meta.url);
const [operational, disabled] = await Promise.all([
  readInspectionGltf(new URL('authority-nexus.glb',root)),
  readInspectionGltf(new URL('disabled/authority-nexus-disabled.glb',root)),
]);
assert.equal(disabled.scene.children[0].userData.state,'attack-disabled');
assert.equal(disabled.animations.length,0);
assert.equal(disabled.scene.getObjectByName('wormhole'),undefined);
assert.equal(disabled.scene.getObjectByName('containment-field'),undefined);
const emergency = new Map();
disabled.scene.traverse(node => {
  if (!node.isMesh) return;
  if (node.material.emissive?.getHex()) {
    assert.equal(node.material.name,'Emergency amber markers / reserve power');
    emergency.set(node.material,node.material.emissive.clone());
  }
});
assert.equal(emergency.size,1);
const originalEmission = new Map();
operational.scene.traverse(node => {
  if (node.isMesh && node.material.name.startsWith('Powered cyan ')) {
    originalEmission.set(node.material,node.material.emissive.clone());
  }
});
const manifest = JSON.parse(await fs.readFile(new URL('traffic-models/manifest.json',root),'utf8'));
const templates = Object.fromEntries(await Promise.all(manifest.ships.map(async s =>
  [s.id,await readInspectionModel(new URL(s.file,root))])));
const radii = Object.fromEntries(Object.entries(templates).map(([id,scene]) => {
  const size = new THREE.Box3().setFromObject(scene).getSize(new THREE.Vector3());
  return [id,size.length()/size.z/2];
}));
const traffic = createWormholeTraffic(templates);
const nexus = createNexusState(operational,disabled);
nexus.attachTraffic(traffic);
assert.equal(nexus.power,1);
assert.equal(disabled.scene.visible,false);
assert.equal(nexus.active,true);
const wormhole = operational.scene.getObjectByName('wormhole');
const solids=[];
disabled.scene.traverse(node=>{if(node.isMesh)solids.push(node);});
disabled.scene.updateMatrixWorld(true);
const ray = new THREE.Raycaster(), down = new THREE.Vector3(0,-1,0);
let clearanceSamples=0, minimumClearance=Infinity;
function checkShips() {
  for (const ship of traffic.snapshot()) {
    if (!ship.visible) continue;
    const radius=radii[ship.type]*ship.renderedLength;
    for (const [dx,dz] of [[0,0],[radius,0],[-radius,0],[0,radius],[0,-radius]]) {
      ray.set(new THREE.Vector3(ship.position[0]+dx,20,ship.position[2]+dz),down);
      const hit=ray.intersectObjects(solids,false)[0];
      if (hit) {
        const gap=ship.position[1]-radius-hit.point.y;
        minimumClearance=Math.min(minimumClearance,gap);
        assert.ok(gap>.1,'Diverting cargo clears the damaged hull');
      }
      clearanceSamples++;
    }
  }
}
for (let cycle=0;cycle<3;cycle++) {
  nexus.setDisabled(true);
  for (const attemptedPower of [1,.5,0,1]) nexus.setPower(attemptedPower);
  assert.equal(nexus.power,0,'Damage locks out attempts to reopen the passage');
  assert.equal(operational.scene.visible,false);
  assert.equal(disabled.scene.visible,true);
  assert.equal(wormhole.visible,false);
  assert.ok(wormhole.scale.x<1e-6);
  originalEmission.forEach((original,material)=>assert.equal(material.emissive.getHex(),0));
  for (let frame=0;frame<400;frame++) {
    nexus.update(.05);
    if(frame%10===0)checkShips();
  }
  assert.equal(nexus.active,false,'Settled disabled scene stops its animation loop');
  for (const ship of traffic.snapshot().filter(s=>s.visible)) {
    assert.equal(ship.state,'holding');
    assert.ok(Math.hypot(ship.position[0],ship.position[2])>4.8);
  }
  emergency.forEach((original,material)=>assert.deepEqual(material.emissive.toArray(),original.toArray()));
  nexus.setDisabled(false);
  assert.equal(nexus.power,1);
  assert.equal(operational.scene.visible,true);
  assert.equal(disabled.scene.visible,false);
  assert.equal(wormhole.visible,true);
  assert.ok(wormhole.scale.x>.999);
  originalEmission.forEach((original,material)=>assert.deepEqual(material.emissive.toArray(),original.toArray()));
  for(let frame=0;frame<120;frame++)nexus.update(.05);
}
nexus.setPower(.4);
nexus.setDisabled(true); nexus.setPower(1); nexus.setDisabled(false);
assert.equal(nexus.power,.4,'Comparison restores the previous operational power setting');
nexus.dispose();
assert.equal(nexus.active,false);

// A disabled URL loads directly into a still holding pattern, including when
// reduced-motion preferences initially hide the traffic.
const direct=createNexusState(operational,disabled,{motion:false});
direct.setDisabled(true);
const paused=createWormholeTraffic(templates,{enabled:false});
direct.attachTraffic(paused);
assert.equal(paused.group.visible,false);
paused.setEnabled(true);
assert.equal(direct.active,false);
assert.ok(paused.snapshot().filter(s=>s.visible).every(s=>s.state==='holding'));
direct.setDisabled(false);
assert.equal(direct.power,1);
assert.equal(direct.active,true);
direct.dispose();
const opBounds=new THREE.Box3().setFromObject(operational.scene);
const disabledBounds=new THREE.Box3().setFromObject(disabled.scene);
assert.ok(opBounds.min.distanceTo(disabledBounds.min)<1e-6 &&
  opBounds.max.distanceTo(disabledBounds.max)<1e-6,'Station variants retain matching bounds');
const report={variantSwitchCycles:3,damagePowerLock:true,portalAbsentFromDisabledExport:true,
  emergencyLightsPreserved:true,restoredOperationalPower:true,initialDisabledHolding:true,
  reducedMotionInitialization:true,stationClearanceSamples:clearanceSamples,
  minimumPhysicalClearance:Number(minimumClearance.toFixed(4)),matchingStationBounds:true,
  method:'Real Three.js models, animation mixer, runtime state controller and traffic geometry. Browser interaction not exercised.'};
await fs.writeFile(new URL('disabled/runtime-validation.json',root),JSON.stringify(report,null,2)+'\n');
console.log('PASS:',JSON.stringify(report));

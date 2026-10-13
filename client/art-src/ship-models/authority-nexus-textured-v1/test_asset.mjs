import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as THREE from 'three';

const root = path.dirname(fileURLToPath(import.meta.url));
const bytes = await fs.readFile(path.join(root, 'authority-nexus.glb'));
const jsonLength = bytes.readUInt32LE(12);
const doc = JSON.parse(bytes.subarray(20, 20+jsonLength).toString());
const binary = bytes.subarray(28+jsonLength);
function accessor(index) {
  const a = doc.accessors[index], view = doc.bufferViews[a.bufferView];
  const size = {SCALAR:1,VEC2:2,VEC3:3}[a.type];
  assert.equal(a.componentType, 5126);
  const array = new Float32Array(a.count*size);
  for(let i=0;i<array.length;i++) array[i]=binary.readFloatLE((view.byteOffset||0)+(a.byteOffset||0)+i*4);
  return array;
}

// Exercise the exported animation with Three's real mixer without a browser/GPU.
const nodes = doc.nodes.map(n => {const object=new THREE.Object3D();object.name=n.name;return object;});
for(let i=0;i<doc.nodes.length;i++) for(const child of doc.nodes[i].children||[]) nodes[i].add(nodes[child]);
const animation = doc.animations.find(a => a.name==='WormholeCollapse');
const tracks = animation.channels.map(channel => {
  const sampler=animation.samplers[channel.sampler];
  return new THREE.VectorKeyframeTrack(nodes[channel.target.node].uuid+'.scale', accessor(sampler.input), accessor(sampler.output));
});
const clip=new THREE.AnimationClip(animation.name,-1,tracks);
const mixer=new THREE.AnimationMixer(nodes[0]);
const action=mixer.clipAction(clip);
action.setLoop(THREE.LoopOnce,1);action.clampWhenFinished=true;
const wormhole=nodes[doc.nodes.findIndex(n=>n.name==='wormhole')];
const field=nodes[doc.nodes.findIndex(n=>n.name==='containment-field')];
for (const value of [1, .5, 0, 1, .3, 0, 1]) {
  action.reset().play();mixer.setTime((1-value)*clip.duration);
  if(value===0){assert.ok(wormhole.scale.x<1e-6);assert.ok(wormhole.scale.z<1e-6);assert.ok(field.scale.length()<1e-6);}
  if(value===1){assert.ok(wormhole.scale.distanceTo(new THREE.Vector3(1,1,1))<1e-6);assert.ok(field.scale.distanceTo(new THREE.Vector3(1,1,1))<1e-6);}
  if(value>0 && value<1){assert.ok(wormhole.scale.x>0 && wormhole.scale.x<1);assert.equal(wormhole.scale.y,1);}
}
const html=await fs.readFile(path.join(root,'viewer.html'),'utf8');
assert.ok(!html.includes('__ASSET_DATA__')&&!html.includes('__VIEWER_BUNDLE__'));
assert.ok(!/<script[^>]+src=/i.test(html),'Offline viewer must embed its scripts');
const match=html.match(/window\.NEXUS_ASSET=(\{.*?\});<\/script>/s);
assert.ok(match,'Embedded asset payload');
const asset=JSON.parse(match[1]);
assert.deepEqual(Buffer.from(asset.glb,'base64'),bytes,'Viewer contains the current GLB');
const info=JSON.parse(await fs.readFile(path.join(root,'model-info.json'),'utf8'));
assert.deepEqual(asset.info,info);
assert.deepEqual(Buffer.from(asset.disabled.glb,'base64'),await fs.readFile(path.join(root,'disabled/authority-nexus-disabled.glb')),
  'Offline viewer embeds the current disabled export');
assert.deepEqual(asset.disabled.info,JSON.parse(await fs.readFile(path.join(root,'disabled/model-info.json'),'utf8')));
assert.deepEqual(Buffer.from(asset.disabled.preview.split(',')[1],'base64'),await fs.readFile(path.join(root,'disabled/previews/hero.png')));
assert.ok(html.includes('id="damage"'),'Disabled-state control is present');
assert.ok(asset.preview.startsWith('data:image/png;base64,'));
assert.ok(asset.reference.startsWith('data:image/png;base64,'));
const trafficManifest = JSON.parse(await fs.readFile(path.join(root,'traffic-models/manifest.json'),'utf8'));
assert.deepEqual(Object.keys(asset.traffic).sort(),trafficManifest.ships.map(s=>s.id).sort());
for (const ship of trafficManifest.ships) {
  assert.deepEqual(Buffer.from(asset.traffic[ship.id],'base64'),await fs.readFile(path.join(root,ship.file)),
    'Offline traffic uses the packaged LOD2 models');
}
console.log('PASS: exported animation closes and reopens repeatedly; offline viewer embeds the current GLB, metadata, reference and preview.');

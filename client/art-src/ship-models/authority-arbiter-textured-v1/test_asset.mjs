// Three.js scene-graph/GLTFLoader checks in Node. No browser or WebGL is used.
// PNG headers stand in for image bitmaps; pixel decoding and visual inspection
// are handled separately by inspect_model.py and its actual-texture renders.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import {createRequire} from 'node:module';
import path from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
let require=createRequire(import.meta.url);
try {require.resolve('three');}
catch {require=createRequire(new URL('../pirate-fleet-v1/package.json',import.meta.url));}
globalThis.self=globalThis;
globalThis.createImageBitmap=async blob=>{
  const data=Buffer.from(await blob.arrayBuffer());
  if(blob.type==='image/jpeg') {
    assert.equal(data.readUInt16BE(0),0xffd8);
    return {width:info.textureResolution[0],height:info.textureResolution[1],close(){}};
  }
  assert.equal(data.subarray(1,4).toString(),'PNG');
  return {width:data.readUInt32BE(16),height:data.readUInt32BE(20),close(){}};
};
const THREE=await import(pathToFileURL(require.resolve('three')).href);
const {GLTFLoader}=await import(pathToFileURL(require.resolve('three/addons/loaders/GLTFLoader.js')).href);
const dark=process.argv.includes('--dark');
const bytes=await fs.readFile(path.join(root,dark?'authority-arbiter-dark.glb':'authority-arbiter.glb'));
const info=JSON.parse(await fs.readFile(path.join(root,dark?'dark-model-info.json':'model-info.json'),'utf8'));
const gltf=await new GLTFLoader().parseAsync(bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength),'');
const scene=gltf.scene;scene.updateMatrixWorld(true);
let triangles=0,meshes=0;
const byName=new Map();
scene.traverse(node=>{
  const index=gltf.parser.associations.get(node)?.nodes;
  if(index!==undefined)byName.set(gltf.parser.json.nodes[index].name,node);
  if(node.isMesh) {
    meshes++;triangles+=node.geometry.index.count/3;
    assert(node.material.isMeshStandardMaterial);
    if(dark) {
      assert.equal(node.material.vertexColors,true);
      assert.equal(node.geometry.attributes.color.count,node.geometry.attributes.position.count);
      assert([...node.geometry.attributes.color.array].every(x=>Number.isFinite(x)&&x>=0&&x<=1));
    }
    for(const tex of [node.material.map,node.material.metalnessMap,node.material.roughnessMap,node.material.emissiveMap]) {
      assert(tex?.isTexture);assert.equal(tex.image.width,info.textureResolution[0]);assert.equal(tex.image.height,info.textureResolution[1]);
    }
  }
});
assert.equal(triangles,info.triangles);assert.equal(meshes,info.renderSections);
const size=new THREE.Box3().setFromObject(scene).getSize(new THREE.Vector3()).toArray();
size.forEach((v,i)=>assert(Math.abs(v-info.dimensionsXYZ[i])<.0002));
const detailBounds={driver:new THREE.Box3(),spines:new THREE.Box3()};
scene.traverse(node=>{
  if(!node.isMesh)return;
  for(const component of node.userData.sourceComponents||[]) {
    const box=component.name.startsWith('mass-driver.')?detailBounds.driver:component.name.startsWith('spine.')?detailBounds.spines:null;
    if(!box)continue;
    const p=node.geometry.attributes.position,index=node.geometry.index;
    for(let i=component.indexStart;i<component.indexStart+component.indexCount;i++) {
      box.expandByPoint(new THREE.Vector3().fromBufferAttribute(p,index.getX(i)).applyMatrix4(node.matrixWorld));
    }
  }
});
for(const [name,box] of Object.entries(detailBounds))assert(!box.isEmpty(),'Viewer cannot find '+name);
assert(detailBounds.driver.getSize(new THREE.Vector3()).z>4,'Mass-driver framing omits the breech or muzzle');
assert(detailBounds.driver.getSize(new THREE.Vector3()).z<6,'Mass driver still has the long projecting barrel');
assert(detailBounds.driver.getSize(new THREE.Vector3()).x>2,'Mass-driver framing is undersized');
assert(detailBounds.driver.max.z<0,'Mass-driver framing includes the aft hull');
assert(detailBounds.spines.getSize(new THREE.Vector3()).x>2,'Spine framing omits an outer spine');
assert(!byName.has('socket.lance.tip'));
assert(byName.has('socket.weapon.mass-driver.muzzle'));
let movingMounts=0,movingSockets=0;
for(const [name,settings] of Object.entries(info.assemblies)) {
  if(!settings.role.endsWith('-yaw'))continue;
  const mount=byName.get(name);assert(mount,name);movingMounts++;
  const muzzleSpecs=info.sockets.filter(s=>s.extras.assembly===name&&s.extras.role.endsWith('muzzle'));
  assert.equal(muzzleSpecs.length,2);
  for(const socket of muzzleSpecs)assert.equal(byName.get(socket.name).parent,mount);
  const socket=byName.get(muzzleSpecs[0].name);
  const before=socket.getWorldPosition(new THREE.Vector3());
  mount.rotation.y=.25;scene.updateMatrixWorld(true);
  const after=socket.getWorldPosition(new THREE.Vector3());
  assert(before.distanceTo(after)>.03,name+' does not move its muzzle');
  mount.rotation.y=0;scene.updateMatrixWorld(true);
  for(const specification of muzzleSpecs) {
    const actual=byName.get(specification.name).getWorldPosition(new THREE.Vector3());
    assert(actual.distanceTo(new THREE.Vector3(...specification.translation))<1e-6);
    movingSockets++;
  }
}
assert.equal(movingMounts,22);assert.equal(movingSockets,44);
// Run the same exported extractor used by the game, without changing its catalog.
const {hardpointsOf}=await import(new URL('../../../scripts/build-ship-hardpoints.mjs',import.meta.url));
const hardpoints=hardpointsOf(scene);
assert.equal(hardpoints.guns.length,45,'Game effects must discover every physical muzzle');
assert.equal(hardpoints.engines.length,4);
assert.equal(new Set(hardpoints.guns.map(p=>p.join(','))).size,45);
const viewer=await fs.readFile(path.join(root,'viewer.html'),'utf8');
const embedded=viewer.match(dark?/<script id="dark-model-data"[^>]*>([^<]+)<\/script>/:/<script id="model-data"[^>]*>([^<]+)<\/script>/)?.[1];
assert(embedded);assert(Buffer.from(embedded.trim(),'base64').equals(bytes));
assert(!viewer.includes('__VIEWER_BUNDLE__')&&!viewer.includes('__TRIANGLES__'));
assert(viewer.includes(info.triangles.toLocaleString('en-US')+' triangles'));
assert(viewer.includes('data-view="driver"')&&viewer.includes('data-view="spines"'));
assert(!viewer.includes('data-view="lance"'));
assert(viewer.includes('data-variant="dark"')&&viewer.includes('data-variant="ivory"'));
assert(viewer.includes('id="galaxy"')&&viewer.includes('id="low-light"'));
const result={ok:true,threeRevision:THREE.REVISION,triangles,renderSections:meshes,
  variant:dark?'dark-imperial':'ivory',standardVertexColors:dark,
  movingGunMounts:movingMounts,muzzleSocketsFollowingMounts:movingSockets,
  gameExtractorWeaponMuzzles:hardpoints.guns.length,gameExtractorEngines:hardpoints.engines.length,
  viewerEmbedsExactModel:true,detailViewBounds:Object.fromEntries(Object.entries(detailBounds).map(([name,box])=>[name,{min:box.min.toArray(),max:box.max.toArray()}])),embeddedTextures:3,textureResolution:info.textureResolution,
  method:'Node GLTFLoader with decoded-dimension bitmap stand-ins; real Three.js scene graph, bounds and gun/socket transforms. No browser/WebGL rendering.'};
await fs.writeFile(path.join(root,dark?'dark-runtime-load-validation.json':'runtime-load-validation.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));

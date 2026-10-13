// Exercise the game's installed Three.js GLTFLoader in Node, without a browser.
// Bitmap dimensions stand in for browser ImageBitmap objects. Actual embedded
// image pixels are separately decoded and rendered by inspect_model.py.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';
const root=path.dirname(fileURLToPath(import.meta.url));
const require=createRequire(new URL('../../../package.json',import.meta.url));
globalThis.self=globalThis;
globalThis.createImageBitmap=async blob=>{
  const b=Buffer.from(await blob.arrayBuffer());
  assert.equal(b.subarray(1,4).toString(),'PNG');
  return {width:b.readUInt32BE(16),height:b.readUInt32BE(20),close(){}};
};
const THREE=await import(pathToFileURL(require.resolve('three')).href);
const {GLTFLoader}=await import(pathToFileURL(require.resolve('three/addons/loaders/GLTFLoader.js')).href);
const bytes=await fs.readFile(path.join(root,'survey-probe.glb'));
const info=JSON.parse(await fs.readFile(path.join(root,'model-info.json'),'utf8'));
const gltf=await new GLTFLoader().parseAsync(bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength),'');
const scene=gltf.scene;scene.updateMatrixWorld(true);
let triangles=0,meshes=0,socket;
scene.traverse(node=>{
  const index=gltf.parser.associations.get(node)?.nodes;
  if(index!==undefined&&gltf.parser.json.nodes[index].name==='socket.engine.main')socket=node;
  if(!node.isMesh)return;
  meshes++;triangles+=node.geometry.index.count/3;
  assert(node.material.isMeshStandardMaterial);
  for(const tex of [node.material.map,node.material.metalnessMap,node.material.roughnessMap,node.material.emissiveMap]){
    assert(tex?.isTexture);assert.equal(tex.image.width,2048);assert.equal(tex.image.height,2048);
  }
});
assert.equal(meshes,2);assert.equal(triangles,info.triangles);assert(socket);
const dims=new THREE.Box3().setFromObject(scene).getSize(new THREE.Vector3()).toArray();
dims.forEach((v,i)=>assert(Math.abs(v-info.dimensionsXYZ[i])<.0002));
const local=new THREE.Vector3(...info.sockets[0].translation);
assert(socket.getWorldPosition(new THREE.Vector3()).distanceTo(local)<1e-6);
scene.position.set(2,3,4);scene.rotation.y=.73;scene.scale.setScalar(2);scene.updateMatrixWorld(true);
assert(socket.getWorldPosition(new THREE.Vector3()).distanceTo(scene.localToWorld(local.clone()))<1e-6);
const report={threeRevision:THREE.REVISION,triangles,meshes,embeddedTexturesLoaded:3,
  engineSocketTransformsWithModel:true,dimensionsMatch:true,
  method:'Node GLTFLoader scene graph and texture-header loading; software previews inspect real texture pixels. No browser/WebGL performance test.'};
await fs.writeFile(path.join(root,'three-validation.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report,null,2));

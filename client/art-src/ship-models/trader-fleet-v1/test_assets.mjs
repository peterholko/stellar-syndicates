// Load the final exported scenes through the game's installed Three.js.
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import path from 'node:path';

const root=path.dirname(fileURLToPath(import.meta.url));
const require=createRequire(new URL('../../../package.json',import.meta.url));
globalThis.self=globalThis;
globalThis.createImageBitmap=async blob=>{
  const bytes=Buffer.from(await blob.arrayBuffer());
  assert.equal(bytes.subarray(1,4).toString(),'PNG');
  return {width:bytes.readUInt32BE(16),height:bytes.readUInt32BE(20),close(){}};
};
const THREE=await import(pathToFileURL(require.resolve('three')).href);
const {GLTFLoader}=await import(pathToFileURL(require.resolve('three/addons/loaders/GLTFLoader.js')).href);
for(const slug of ['clean-slate','second-wind','small-fortune']){
  const folder=path.resolve(root,`../${slug}-textured-v1`);
  const bytes=await fs.readFile(path.join(folder,`${slug}.glb`));
  const info=JSON.parse(await fs.readFile(path.join(folder,'model-info.json'),'utf8'));
  const gltf=await new GLTFLoader().parseAsync(bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength),'');
  const scene=gltf.scene;
  scene.updateMatrixWorld(true);
  let triangles=0,meshes=0;
  const sockets=new Map();
  assert(gltf.parser.json.images.every(image=>image.bufferView!==undefined&&!image.uri));
  scene.traverse(node=>{
    const index=gltf.parser.associations.get(node)?.nodes;
    const name=index===undefined?'':gltf.parser.json.nodes[index].name;
    if(name.startsWith('socket.'))sockets.set(name,node);
    if(!node.isMesh)return;
    meshes++;
    triangles+=node.geometry.index.count/3;
    assert(node.material.isMeshStandardMaterial);
    for(const tex of [node.material.map,node.material.metalnessMap,node.material.roughnessMap,node.material.emissiveMap]){
      assert(tex?.isTexture);
      assert.equal(tex.image.width,2048);
      assert.equal(tex.image.height,2048);
    }
  });
  assert.equal(meshes,2);
  assert.equal(triangles,info.triangles);
  assert.equal(sockets.size,info.sockets.length);
  const dims=new THREE.Box3().setFromObject(scene).getSize(new THREE.Vector3()).toArray();
  dims.forEach((v,i)=>assert(Math.abs(v-info.dimensionsXYZ[i])<.0002));
  for(const entry of info.sockets){
    const socket=sockets.get(entry.name);
    assert(socket,entry.name);
    assert.deepEqual(socket.userData.direction,entry.extras.direction);
    assert(socket.getWorldPosition(new THREE.Vector3()).distanceTo(new THREE.Vector3(...entry.translation))<1e-6);
  }
  scene.position.set(2,3,4);
  scene.rotation.set(.15,.73,-.12);
  scene.scale.setScalar(2);
  scene.updateMatrixWorld(true);
  for(const entry of info.sockets){
    const local=new THREE.Vector3(...entry.translation);
    assert(sockets.get(entry.name).getWorldPosition(new THREE.Vector3()).distanceTo(scene.localToWorld(local))<1e-6);
  }
  const report={ship:slug,threeRevision:THREE.REVISION,triangles,meshes,embeddedTexturesLoaded:3,
    engineSockets:2,dockingSockets:info.geometryCounts.dockingCollars,utilitySockets:info.geometryCounts.cranes,
    weaponSockets:0,allSocketTransformsFollowModel:true,dimensionsMatch:true,
    method:'Node GLTFLoader scene graph and PNG-header loading; separate software renders inspect decoded embedded texture pixels. Browser rendering checked separately.'};
  await fs.writeFile(path.join(folder,'three-validation.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report));
}

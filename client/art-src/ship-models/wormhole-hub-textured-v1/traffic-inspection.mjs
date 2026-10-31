import fs from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';

// Real GLTFLoader geometry/material parsing in Node; only DOM image decoding is
// replaced. Texture indices are retained for the independent software renderer.
export async function readInspectionGltf(file) {
  const loader = new GLTFLoader();
  loader.register(parser => ({
    name:'NEXUS_INSPECTION_TEXTURES',
    loadTexture(index) {
      const texture = new THREE.Texture();
      texture.flipY = false;
      texture.userData = {sourceFile:file instanceof URL ? fileURLToPath(file) : String(file),textureIndex:index};
      parser.associations.set(texture,{textures:index});
      return Promise.resolve(texture);
    },
  }));
  const bytes = await fs.readFile(file);
  return loader.parseAsync(bytes.buffer.slice(bytes.byteOffset,bytes.byteOffset+bytes.byteLength),'');
}

export async function readInspectionModel(file) { return (await readInspectionGltf(file)).scene; }

export async function saveTrafficSnapshot(traffic,filename) {
  const primitives = [];
  traffic.group.updateMatrixWorld(true);
  const matrix = new THREE.Matrix4();
  traffic.group.traverse(mesh => {
    if (!mesh.isInstancedMesh) return;
    for (let instance=0;instance<mesh.count;instance++) {
      mesh.getMatrixAt(instance,matrix);
      matrix.premultiply(mesh.matrixWorld);
      if (Math.abs(matrix.determinant()) < 1e-15) continue;
      const geometry = mesh.geometry.clone().applyMatrix4(matrix);
      const m = mesh.material, color = m.color.clone();
      if (mesh.instanceColor) {
        color.multiply(new THREE.Color().fromArray(mesh.instanceColor.array,instance*3));
      }
      primitives.push({name:mesh.name,position:Array.from(geometry.attributes.position.array),
        normal:Array.from(geometry.attributes.normal.array),uv:Array.from(geometry.attributes.uv.array),
        indices:Array.from(geometry.index.array),
        material:{color:color.toArray(),emissive:m.emissive?.toArray() ?? [0,0,0],
          map:m.map?.userData,emissiveMap:m.emissiveMap?.userData,unlit:!!m.isMeshBasicMaterial,
          doubleSided:m.side===THREE.DoubleSide}});
      geometry.dispose();
    }
  });
  await fs.writeFile(filename,JSON.stringify({stats:traffic.stats,ships:traffic.snapshot(),primitives}));
}

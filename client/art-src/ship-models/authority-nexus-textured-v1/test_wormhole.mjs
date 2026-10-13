import assert from 'node:assert/strict';
import * as THREE from 'three';
import { createWormholeEffect, TIME_PERIOD } from './wormhole-effect.js';

// Test the runtime with real Three objects, sharing the same texture with the
// hull as GLTFLoader does. No browser or mocked WebGL renderer is involved.
const texture = new THREE.Texture();
const original = new THREE.MeshBasicMaterial({ map: texture, side: THREE.DoubleSide });
const geometry = new THREE.CircleGeometry(4, 64);
const hull = new THREE.Mesh(geometry, original);
const group = new THREE.Group();
group.userData.assembly = 'wormhole';
const membrane = new THREE.Mesh(geometry, original);
group.add(membrane);
const positions = new Float32Array(geometry.attributes.position.array);
const effect = createWormholeEffect(group);
assert.equal(hull.material, original, 'Station material must stay unchanged');
assert.equal(membrane.geometry, geometry, 'Do not add or replace geometry');
assert.deepEqual(geometry.attributes.position.array, positions);
assert.ok(membrane.material.isShaderMaterial);
assert.equal(membrane.material.uniforms.uAtlas.value, texture, 'Reuse the embedded atlas');
assert.equal(membrane.material.side, THREE.DoubleSide);

effect.update(1/60);
const first = effect.elapsed;
assert.ok(first > 0);
effect.setPlaying(false);
effect.update(.02);
assert.equal(effect.elapsed, first, 'Pause must hold the current frame');
effect.setPlaying(true);
effect.setPower(0);
assert.equal(effect.active, false, 'A closed wormhole must stop the continuous render loop');
effect.update(.02);
assert.equal(effect.elapsed, first);
for (const p of [.2, 1, 0, 1]) {
  effect.setPower(p);
  assert.equal(membrane.material.uniforms.uPower.value, p);
  const before = effect.elapsed;
  effect.update(.02);
  assert.equal(effect.elapsed > before, p > 0, 'Flow resumes after restoring containment');
}
effect.setPower(-1); assert.equal(effect.active, false);
effect.setPower(2); assert.equal(membrane.material.uniforms.uPower.value, 1);
effect.setPower(NaN); assert.equal(membrane.material.uniforms.uPower.value, 1);
const beforeInvalid = effect.elapsed;
for (const dt of [NaN, Infinity, -1, 0]) effect.update(dt);
assert.equal(effect.elapsed, beforeInvalid);
effect.update(10);
assert.ok(effect.elapsed-beforeInvalid < .051, 'Clamp time jumps after suspension');
for (let i=0;i<Math.ceil(TIME_PERIOD/.05);i++) effect.update(.05);
assert.ok(effect.elapsed < 1 && effect.elapsed >= 0, 'Time wraps without growing indefinitely');

let disposedTexture = false, disposedShader = 0;
texture.addEventListener('dispose', () => { disposedTexture = true; });
membrane.material.addEventListener('dispose', () => { disposedShader++; });
effect.dispose(); effect.dispose();
assert.equal(membrane.material, original, 'Disposal restores the export material');
assert.equal(disposedTexture, false, 'The shared atlas must remain alive');
assert.equal(disposedShader, 1);
assert.equal(effect.active, false);
assert.throws(() => createWormholeEffect(new THREE.Group()), /wormhole assembly/);

const reduced = createWormholeEffect(group, { playing: false });
reduced.update(.05);
assert.equal(reduced.elapsed, 0, 'Reduced-motion initialization is still');
reduced.setPlaying(true); reduced.update(.05);
assert.ok(reduced.elapsed > 0, 'The user can explicitly enable motion');
reduced.dispose();
console.log('PASS: wormhole flow pause/resume, containment power, time wrap, geometry/material isolation and resource cleanup.');

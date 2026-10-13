import * as THREE from 'three';

// Registration of the aperture in the approved reference atlas. Coordinates
// use the existing glTF convention (the texture has flipY=false).
export const WORMHOLE_ATLAS = Object.freeze({
  center: [395 / 1536, 320 / 1024],
  radius: [122 / 1536, 122 / 1024],
  apertureRadius: 4,
});
// Every temporal frequency below is a multiple of .01. Wrapping at this common
// period avoids precision drift without a visible jump in long-running viewers.
export const TIME_PERIOD = 200 * Math.PI;

export const wormholeVertexShader = /* glsl */`
uniform float uApertureRadius;
varying vec2 vAperture;
void main() {
  vAperture = position.xz / uApertureRadius;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const wormholeFragmentShader = /* glsl */`
uniform sampler2D uAtlas;
uniform vec2 uAtlasCenter;
uniform vec2 uAtlasRadius;
uniform float uTime;
uniform float uPower;
varying vec2 vAperture;

void main() {
  float radius = clamp(length(vAperture), 0.0, 1.0);
  float angle = radius > 0.0001 ? atan(vAperture.y, vAperture.x) : 0.0;
  float interior = 1.0 - smoothstep(0.84, 0.995, radius);

  // A continuous rotation plus bounded radial shear makes the vortex flow
  // without accumulating ever-tighter UV stretching as time passes.
  float turn = -uTime * 0.22 + 0.42 * sin(radius * 9.0 + uTime * 0.55);
  float flowingRadius = clamp(radius + 0.016 * interior *
    sin(radius * 23.0 + uTime * 1.40), 0.0, 0.998);
  vec2 flowing = vec2(cos(angle + turn), sin(angle + turn)) * flowingRadius;
  vec2 stationary = vAperture * min(1.0, 0.998 / max(radius, 0.0001));
  vec3 fixedColor = texture2D(uAtlas, uAtlasCenter + stationary * uAtlasRadius).rgb;
  vec3 movingColor = texture2D(uAtlas, uAtlasCenter + flowing * uAtlasRadius).rgb;
  vec3 color = mix(fixedColor, movingColor, interior);

  // Thin inward-travelling spiral crests and restrained cyan accents. All
  // angular frequencies are integers, so there is no seam at +/- pi.
  float crest = sin(radius * 31.0 + angle * 3.0 + uTime * 1.40 +
                    0.6 * sin(angle * 5.0 - uTime * 0.45));
  float filament = pow(max(crest, 0.0), 16.0);
  float arc = pow(0.5 + 0.5 * sin(angle * 2.0 + radius * 11.0 - uTime * 0.65), 6.0);
  float envelope = smoothstep(0.12, 0.30, radius) * interior;
  color += (vec3(0.12, 0.025, 0.40) + vec3(0.025, 0.20, 0.30) * arc) *
           filament * envelope * 0.32;
  color *= 0.96 + 0.04 * sin(uTime * 1.10 + radius * 4.0) * interior;
  color *= mix(0.42, 1.0, smoothstep(0.04, 0.22, radius));
  color *= uPower;

  gl_FragColor = vec4(color, 1.0);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

/** Attach after GLTFLoader loads the `wormhole` assembly. No geometry is added.
 * Call update(deltaSeconds) from the engine's render loop and setPower(0..1)
 * alongside the glTF WormholeCollapse animation. The shader is a runtime effect;
 * a generic GLB viewer still has the original reference-textured material.
 */
export function createWormholeEffect(assembly, { playing = true } = {}) {
  if (!assembly || assembly.userData.assembly !== 'wormhole') {
    throw new Error('The wormhole effect must attach to the wormhole assembly.');
  }
  const meshes = [];
  assembly.traverse(node => { if (node.isMesh) meshes.push(node); });
  if (!meshes.length || meshes.some(node => !node.material?.map)) {
    throw new Error('The wormhole surface needs its reference texture.');
  }
  const time = { value: 0 }, power = { value: 1 };
  let enabled = Boolean(playing), disposed = false;
  const originals = meshes.map(node => ({ node, material: node.material }));
  const materials = new Map();
  for (const { node, material } of originals) {
    if (!materials.has(material)) {
      materials.set(material, new THREE.ShaderMaterial({
        name: 'Living wormhole / violet energy flow',
        uniforms: {
          uAtlas: { value: material.map },
          uAtlasCenter: { value: new THREE.Vector2(...WORMHOLE_ATLAS.center) },
          uAtlasRadius: { value: new THREE.Vector2(...WORMHOLE_ATLAS.radius) },
          uApertureRadius: { value: WORMHOLE_ATLAS.apertureRadius },
          uTime: time, uPower: power,
        },
        vertexShader: wormholeVertexShader,
        fragmentShader: wormholeFragmentShader,
        side: THREE.DoubleSide,
        wireframe: material.wireframe,
        depthWrite: true,
        toneMapped: material.toneMapped,
      }));
    }
    node.material = materials.get(material);
  }
  return {
    get active() { return !disposed && enabled && power.value > 0.001; },
    get elapsed() { return time.value; },
    setPlaying(value) { enabled = Boolean(value); },
    setPower(value) {
      if (Number.isFinite(value)) power.value = THREE.MathUtils.clamp(value, 0, 1);
    },
    update(deltaSeconds) {
      if (this.active && Number.isFinite(deltaSeconds) && deltaSeconds > 0) {
        time.value = (time.value + Math.min(deltaSeconds, 0.05)) % TIME_PERIOD;
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const { node, material } of originals) node.material = material;
      for (const shader of materials.values()) shader.dispose();
      // The atlas is shared with the station; this effect does not own it.
    },
  };
}

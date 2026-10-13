// The Authority Nexus's living vortex: a runtime shader on the GLB's own
// `wormhole` surface. Ported from art-src/ship-models/authority-nexus-textured-v1
// (wormhole-effect.js); the shaders are unchanged. It adds no geometry: UV
// warping and procedural filaments run in the fragment shader, reusing the
// station's reference atlas.
import * as THREE from "three";

/** Registration of the aperture in the approved reference atlas, in normalized
 * UVs (the glTF texture has flipY=false), so a resized atlas still registers. */
export const WORMHOLE_ATLAS = Object.freeze({
  center: [395 / 1536, 320 / 1024] as const,
  radius: [122 / 1536, 122 / 1024] as const,
  apertureRadius: 4,
});
/** Every temporal frequency is a multiple of .01; wrapping at this common period
 * avoids precision drift without a visible jump. */
export const TIME_PERIOD = 200 * Math.PI;

const vertexShader = /* glsl */`
uniform float uApertureRadius;
varying vec2 vAperture;
void main() {
  vAperture = position.xz / uApertureRadius;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const fragmentShader = /* glsl */`
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

export interface WormholeEffect {
  /** False while paused, closed (power 0) or disposed: no frames are needed. */
  readonly active: boolean;
  readonly elapsed: number;
  setPlaying(playing: boolean): void;
  /** Containment power, 0..1; 0 renders the aperture black. */
  setPower(power: number): void;
  /** Advance the flow; a long gap (a hidden tab) counts as one short step. */
  update(deltaSeconds: number): void;
  /** Restores the export material; the shared atlas stays alive. */
  dispose(): void;
}

/** Attach after GLTFLoader loads the `wormhole` assembly (its node extras carry
 * `assembly: "wormhole"`). Pass `time` to pose a still frame. */
export function createWormholeEffect(assembly: THREE.Object3D, { playing = true, time: start = 0 } = {}): WormholeEffect {
  if (assembly.userData.assembly !== "wormhole") throw new Error("The wormhole effect must attach to the wormhole assembly.");
  const meshes: THREE.Mesh[] = [];
  assembly.traverse(node => { if ((node as THREE.Mesh).isMesh) meshes.push(node as THREE.Mesh); });
  const mapOf = (mesh: THREE.Mesh) => (mesh.material as THREE.MeshBasicMaterial).map;
  if (!meshes.length || meshes.some(mesh => !mapOf(mesh))) throw new Error("The wormhole surface needs its reference texture.");
  const time = { value: ((start % TIME_PERIOD) + TIME_PERIOD) % TIME_PERIOD }, power = { value: 1 };
  let enabled = playing, disposed = false;
  const originals = meshes.map(mesh => ({ mesh, material: mesh.material }));
  const shaders = new Map<THREE.Material | THREE.Material[], THREE.ShaderMaterial>();
  for (const { mesh, material } of originals) {
    if (!shaders.has(material)) {
      const source = material as THREE.MeshBasicMaterial;
      shaders.set(material, new THREE.ShaderMaterial({
        name: "Living wormhole / violet energy flow",
        uniforms: {
          uAtlas: { value: source.map },
          uAtlasCenter: { value: new THREE.Vector2(...WORMHOLE_ATLAS.center) },
          uAtlasRadius: { value: new THREE.Vector2(...WORMHOLE_ATLAS.radius) },
          uApertureRadius: { value: WORMHOLE_ATLAS.apertureRadius },
          uTime: time, uPower: power,
        },
        vertexShader, fragmentShader,
        side: THREE.DoubleSide,
        wireframe: source.wireframe,
        depthWrite: true,
        toneMapped: source.toneMapped,
      }));
    }
    mesh.material = shaders.get(material)!;
  }
  return {
    get active() { return !disposed && enabled && power.value > .001; },
    get elapsed() { return time.value; },
    setPlaying(value) { enabled = value; },
    setPower(value) { if (Number.isFinite(value)) power.value = THREE.MathUtils.clamp(value, 0, 1); },
    update(deltaSeconds) {
      if (this.active && Number.isFinite(deltaSeconds) && deltaSeconds > 0) {
        time.value = (time.value + Math.min(deltaSeconds, .05)) % TIME_PERIOD;
      }
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const { mesh, material } of originals) mesh.material = material;
      for (const shader of shaders.values()) shader.dispose();
    },
  };
}

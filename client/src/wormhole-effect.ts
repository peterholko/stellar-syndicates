// The Wormhole Hub's living portal: the asset's runtime flow on the GLB's own
// `wormhole` membrane. Ported from art-src/ship-models/wormhole-hub-textured-v1
// (wormhole-flow.js); its GLSL is unchanged. It patches the membrane's own
// unlit material: the galaxy turns at one rate inside a fixed rim with a
// bounded ripple, so it never winds tighter however long it runs.
import * as THREE from "three";

/** Every temporal frequency is a multiple of .01; wrapping at this common period
 * avoids precision drift without a visible jump. */
export const TIME_PERIOD = 200 * Math.PI;

/** The asset's map fragment, plus the containment power (0 renders it black).
 * The centre and radius register the aperture in the front artwork's UVs. */
const mapFragment = /* glsl */`
  #ifdef USE_MAP
    vec2 center = vec2(626.0 / 1254.0, 603.0 / 1254.0);
    vec2 p = vMapUv - center;
    float radius = length(p) / (3.685 * 70.5 / 1254.0);
    float interior = 1.0 - smoothstep(0.82, 0.98, radius);
    float angle = uHubTime * 0.06 + 0.10 * sin(radius * 8.0 - uHubTime * 0.30);
    float c = cos(angle), s = sin(angle);
    vec2 uv = center + mat2(c, -s, s, c) * p;
    vec4 texel = mix(texture2D(map, vMapUv), texture2D(map, uv), interior);
    float wave = sin(radius * 51.0 - uHubTime * 1.80) * 0.018 * interior;
    texel.rgb += vec3(0.05, 0.3, 0.65) * wave;
    diffuseColor *= texel;
    diffuseColor.rgb *= uHubPower;
  #endif
`;

export interface WormholeEffect {
  /** False while paused, closed (power 0) or disposed: no frames are needed. */
  readonly active: boolean;
  readonly elapsed: number;
  setPlaying(playing: boolean): void;
  /** Containment power, 0..1; 0 renders the portal black. */
  setPower(power: number): void;
  /** Advance the flow; a long gap (a hidden tab) counts as one short step. */
  update(deltaSeconds: number): void;
  /** Restores the export material's own shader; the galaxy texture stays alive. */
  dispose(): void;
}

/** Attach after GLTFLoader loads the `wormhole` assembly. Pass `time` to pose a
 * still frame. */
export function createWormholeEffect(assembly: THREE.Object3D, { playing = true, time: start = 0 } = {}): WormholeEffect {
  if (assembly.name !== "wormhole") throw new Error("The portal flow must attach to the wormhole assembly.");
  const materials = new Set<THREE.MeshBasicMaterial>();
  assembly.traverse(node => {
    const mesh = node as THREE.Mesh;
    if (mesh.isMesh) materials.add(mesh.material as THREE.MeshBasicMaterial);
  });
  if (!materials.size || [...materials].some(material => Array.isArray(material) || !material.map)) {
    throw new Error("The wormhole membrane needs its galaxy texture.");
  }
  const time = { value: ((start % TIME_PERIOD) + TIME_PERIOD) % TIME_PERIOD }, power = { value: 1 };
  let enabled = playing, disposed = false;
  const originals = [...materials].map(material => ({ material, compile: material.onBeforeCompile, key: material.customProgramCacheKey }));
  for (const { material } of originals) {
    material.onBeforeCompile = shader => {
      shader.uniforms.uHubTime = time;
      shader.uniforms.uHubPower = power;
      shader.fragmentShader = "uniform float uHubTime;\nuniform float uHubPower;\n"
        + shader.fragmentShader.replace("#include <map_fragment>", mapFragment);
    };
    material.customProgramCacheKey = () => "wormhole-hub-flow-v1";
    material.needsUpdate = true;
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
      for (const { material, compile, key } of originals) {
        material.onBeforeCompile = compile;
        material.customProgramCacheKey = key;
        material.needsUpdate = true;
      }
    },
  };
}

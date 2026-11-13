// ============================================================================
// ROLLING STAR — a star's painting turning about its north–south axis.
// ============================================================================
//
// Peter (Nov 9 2026): the System View's stars spun "as a clock", the painting
// turned in the plane of the screen, "not as if it is pivoting over the north
// - south axis of the star". Here the surface rolls instead. The approved
// paintings show one face, so the sphere's surface is drawn from the
// painting's interior: four 90° tiles round the star, the painting and its
// north–south mirror in turn, cross-faded at their edges (no east–west
// mirror: two neighbours mirrored that way would meet as a butterfly). Features
// cross the disc left to right, foreshortening at the limbs. The painted limb,
// prominences, corona and field loops hold still. A binary's stars each turn
// on their own axis; a pulsar's beams swing round the axis like a lighthouse
// seen from the side; a black hole has no surface to turn.
//
// One display object for the System View's star, the raid camera's, and the
// galaxy map's disc on the final dive, so the doorway still crossfades two
// identical pictures. Everything is a pure function of wall time.

import { Container, GlProgram, Mesh, MeshGeometry, Shader, UniformGroup, type Texture } from "pixi.js";

export interface StarDisc {
  /** Centre and radius as fractions of the texture's width (it is square). */
  readonly cx: number;
  readonly cy: number;
  readonly r: number;
  /** Turns per one of the star type's periods (a binary's smaller star is quicker). */
  readonly rate: number;
}

/** §star-flares (Peter, Nov 9 2026: "animate a bit of the solar flares from
 * the larger stars ... just to make them a little more alive"): the painted
 * flares beyond each disc's limb lift, sway and pulse on slow waves round the
 * limb, and now and then one stretch of limb erupts. */
export interface StarFlare {
  /** 0–1: how far the flares move and pulse. */
  readonly strength: number;
  /** Where the painted flares end, in disc radii; nothing moves past it. */
  readonly reach: number;
  /** Seconds between eruptions. */
  readonly every: number;
}

export interface StarSurface {
  /** Seconds per turn about the north–south axis (0 = never turns). */
  readonly period: number;
  readonly discs: readonly StarDisc[];
  /** A pulsar's painted beam axis, degrees above the +x axis. */
  readonly beam?: number;
  readonly flare?: StarFlare;
}

/** Each system painting's photosphere, fitted as a circle to the opaque disc
 * of the approved masters (art-src/stars-2026-09-08). Re-measure if the art
 * changes. The periods are the old clock spin's, so the pace is unchanged.
 * A flare's reach is where that painting's flares end (alpha > 0.25 on most
 * of the limb, measured the same way); the glowing white stars barely move,
 * red dwarfs and magnetars erupt most often. */
export const STAR_SURFACES: Record<string, StarSurface> = {
  red_dwarf: { period: 260, discs: [{ cx: 0.4993, cy: 0.4953, r: 0.349, rate: 1 }],
    flare: { strength: 0.9, reach: 1.25, every: 13 } },
  yellow_star: { period: 300, discs: [{ cx: 0.4971, cy: 0.4967, r: 0.373, rate: 1 }],
    flare: { strength: 1, reach: 1.25, every: 18 } },
  white_star: { period: 300, discs: [{ cx: 0.4986, cy: 0.4973, r: 0.3495, rate: 1 }],
    flare: { strength: 0.5, reach: 1.18, every: 22 } },
  blue_giant: { period: 340, discs: [{ cx: 0.4989, cy: 0.5034, r: 0.3565, rate: 1 }],
    flare: { strength: 1, reach: 1.3, every: 16 } },
  red_giant: { period: 520, discs: [{ cx: 0.4978, cy: 0.5013, r: 0.4007, rate: 1 }],
    flare: { strength: 1.1, reach: 1.32, every: 20 } },
  white_dwarf: { period: 200, discs: [{ cx: 0.4987, cy: 0.4991, r: 0.256, rate: 1 }],
    flare: { strength: 0.35, reach: 1.18, every: 25 } },
  neutron_star: { period: 7, discs: [{ cx: 0.5092, cy: 0.506, r: 0.0941, rate: 1 }], beam: 47.1 },
  binary_star: { period: 140, discs: [
    { cx: 0.3712, cy: 0.5779, r: 0.2552, rate: 1 },
    { cx: 0.7466, cy: 0.3469, r: 0.165, rate: 1.6 },
  ], flare: { strength: 1, reach: 1.3, every: 15 } },
  black_hole: { period: 0, discs: [] },
  magnetar: { period: 46, discs: [{ cx: 0.5227, cy: 0.5202, r: 0.2426, rate: 1 }],
    flare: { strength: 0.7, reach: 1.9, every: 9 } },
};

const STILL: StarSurface = { period: 0, discs: [] };

export function starSurface(slug: string | null | undefined): StarSurface {
  return (slug && STAR_SURFACES[slug]) || STILL;
}

/** Seconds per turn of a star type (0 = still). */
export function starPeriod(slug: string): number {
  return starSurface(slug).period;
}

/** The surface's turn about the north–south axis at wall time `t` (s), in
 * [0, 2π): features drift left to right as it grows. */
export function starRoll(slug: string, t: number, rate = 1): number {
  const period = starPeriod(slug);
  if (!(period > 0) || !Number.isFinite(t)) return 0;
  const turns = (t * rate) / period;
  return (turns - Math.floor(turns)) * Math.PI * 2;
}

/** §star-flares: the limb's waves, four round the limb (3, 5, 8 and 13 to a
 * turn), each drifting at its own pace (radians of phase a second): one set
 * lifts the flares, the other sways their tips and pulses their light. */
export const FLARE_LIFT_SPEEDS = [0.21, -0.33, 0.47, -0.61] as const;
export const FLARE_SWAY_SPEEDS = [-0.17, 0.29, -0.43, 0.55] as const;
/** An eruption: seconds rising, held and falling back, and how far it lifts
 * its stretch of limb's flares (a fraction of their height). */
export const ERUPTION = { rise: 2.5, hold: 1.2, fall: 4.5, lift: 0.7 } as const;

const wrapAngle = (a: number) => a - Math.floor(a / (Math.PI * 2)) * Math.PI * 2;
const ease = (x: number) => x * x * (3 - 2 * x);
const hash01 = (n: number) => {
  const h = Math.sin(n * 12.9898 + 78.233) * 43758.5453;
  return h - Math.floor(h);
};

/** The waves' phases at wall time `t` (s), each in [0, 2π). */
export function flareWaves(t: number): { lift: number[]; sway: number[] } {
  const at = (speeds: readonly number[]) => speeds.map(v => Number.isFinite(t) ? wrapAngle(v * t) : 0);
  return { lift: at(FLARE_LIFT_SPEEDS), sway: at(FLARE_SWAY_SPEEDS) };
}

/** A disc's eruption at wall time `t` (s): where on its limb (radians,
 * texture space: 0 is +x, y runs down) and how high it lifts now. One per
 * `every` seconds at a place drawn from the period's number; a binary's
 * second star keeps its own clock. */
export function starEruption(slug: string, t: number, disc = 0): { angle: number; lift: number } {
  const flare = starSurface(slug).flare;
  if (!flare || !Number.isFinite(t)) return { angle: 0, lift: 0 };
  const shifted = t + disc * 0.37 * flare.every;
  const n = Math.floor(shifted / flare.every);
  const local = shifted - n * flare.every;
  const { rise, hold, fall, lift } = ERUPTION;
  const env = local < rise ? ease(local / rise)
    : local < rise + hold ? 1
    : local < rise + hold + fall ? 1 - ease((local - rise - hold) / fall)
    : 0;
  return { angle: hash01(n * 2 + disc) * Math.PI * 2, lift: lift * env };
}

/** A pulsar's beams in the picture at turn `angle`: the painted axis
 * (`painted`), the projected one (`axis`) and its length relative to the
 * painting's. The spin axis is the screen's vertical; the beams lean off it
 * by the painted angle, so at a quarter turn they point at the viewer:
 * straight up and down, shortest, and the flash is brightest. */
export function pulsarBeam(beamDeg: number, angle: number): { painted: [number, number]; axis: [number, number]; length: number } {
  const beta = (beamDeg * Math.PI) / 180;
  const lean = Math.PI / 2 - beta;
  const bx = Math.sin(lean) * Math.cos(angle);
  const by = -Math.cos(lean); // texture y runs down
  const length = Math.hypot(bx, by);
  return { painted: [Math.cos(beta), -Math.sin(beta)], axis: [bx / length, by / length], length };
}

const VERTEX = `
in vec2 aPosition;
in vec2 aUV;
out vec2 vUV;
out vec4 vColor;
uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform vec4 uWorldColorAlpha;
uniform mat3 uTransformMatrix;
uniform vec4 uColor;
void main(void) {
  mat3 mvp = uProjectionMatrix * uWorldTransformMatrix * uTransformMatrix;
  gl_Position = vec4((mvp * vec3(aPosition, 1.0)).xy, 0.0, 1.0);
  vUV = aUV;
  vColor = uColor * uWorldColorAlpha;
}
`;

// Texture samples stay outside branches that differ from pixel to pixel, so
// mip selection never sees a broken derivative. Each tile is sampled only
// where its weight is above zero, and its coordinates are continuous there.
const FRAGMENT = `precision highp float;
in vec2 vUV;
in vec4 vColor;
out vec4 finalColor;
uniform sampler2D uTexture;
uniform vec4 uDiscA;
uniform vec4 uDiscB;
uniform vec2 uRoll;
uniform vec4 uBeam;
uniform vec2 uBeamLen;
uniform float uMeanBias;
uniform vec4 uFlareA;
uniform vec4 uFlareB;
uniform vec4 uWaves;
uniform vec4 uWaves2;

const float TILE = 1.5707963;
const float FADE = 0.2094395;
const float SOURCE = 0.84;
const float ROLL_IN = 0.82;
const float ROLL_OUT = 0.96;

vec4 tileSample(vec2 centre, float radius, float tile, float local, float cosLat, float lat, out vec4 mean) {
  float flip = mod(tile, 2.0) < 0.5 ? 1.0 : -1.0;
  vec2 src = centre + radius * SOURCE * vec2(sin(local) * cosLat, flip * lat);
  mean = texture(uTexture, src, uMeanBias);
  return texture(uTexture, src);
}

vec4 rolled(vec4 painted, vec4 disc, float turn) {
  vec2 d = (vUV - disc.xy) / max(disc.z, 1e-5);
  float r = length(d);
  float z = sqrt(max(0.0, 1.0 - r * r));
  float lat = clamp(d.y, -1.0, 1.0);
  float cosLat = sqrt(max(0.0, 1.0 - lat * lat));
  float u = (atan(d.x, z) - turn) / TILE;
  float even = 2.0 * floor(u * 0.5 + 0.5);
  float odd = 2.0 * floor((u - 1.0) * 0.5 + 0.5) + 1.0;
  float le = (u - even) * TILE;
  float lo = (u - odd) * TILE;
  float we = 1.0 - smoothstep(TILE * 0.5 - FADE, TILE * 0.5 + FADE, abs(le));
  float wo = 1.0 - we;
  vec4 me;
  vec4 mo;
  vec4 se = tileSample(disc.xy, disc.z, even, le, cosLat, lat, me);
  vec4 so = tileSample(disc.xy, disc.z, odd, lo, cosLat, lat, mo);
  vec4 mean = we * me + wo * mo;
  vec4 surface = clamp(mean + (we * (se - me) + wo * (so - mo)) / sqrt(we * we + wo * wo), 0.0, 1.0);
  float keep = max(smoothstep(ROLL_IN, ROLL_OUT, r), 1.0 - disc.w);
  return mix(surface, painted, keep);
}

float waves(float a, vec4 p) {
  return 0.40 * sin(3.0 * a + p.x) + 0.30 * sin(5.0 * a + p.y) + 0.20 * sin(8.0 * a + p.z) + 0.10 * sin(13.0 * a + p.w);
}

// The painted flares beyond a disc's limb (flare: strength, reach in disc
// radii, eruption angle, eruption lift): lifted and swayed by slow waves round
// the limb, brighter where they rise, and nothing moves inside the limb or
// over the other disc. Adds to the light; returns where to sample instead.
vec2 flareShift(vec4 disc, vec4 flare, vec4 other, float salt, inout float glow) {
  vec2 d = (vUV - disc.xy) / max(disc.z, 1e-5);
  float r = length(d);
  float a = atan(d.y, d.x);
  float beside = length((vUV - other.xy) / max(other.z, 1e-5));
  float zone = flare.x * smoothstep(0.98, 1.06, r) * (1.0 - smoothstep(flare.y * 0.85, flare.y * 1.05, r))
    * mix(1.0, smoothstep(0.98, 1.06, beside), other.w);
  float da = abs(mod(a - flare.z + 3.14159265, 6.28318531) - 3.14159265);
  float erupt = flare.w * exp(-da * da / 0.12);
  float lift = zone * (0.22 * waves(a + salt, uWaves) + erupt);
  float tip = clamp((r - 1.0) / max(flare.y - 1.0, 0.05), 0.0, 1.0);
  float sway = zone * 0.09 * tip * waves(a + salt + 1.3, uWaves2);
  glow += zone * (0.28 * waves(a + salt + 2.1, uWaves2.yzwx) + 0.55 * erupt);
  float rs = 1.0 + (r - 1.0) / (1.0 + lift);
  vec2 src = disc.xy + disc.z * rs * vec2(cos(a - sway), sin(a - sway));
  return (src - vUV) * step(1e-4, zone);
}

void main(void) {
  float glow = 1.0;
  vec2 base = vUV + flareShift(uDiscA, uFlareA, uDiscB, 0.0, glow) + flareShift(uDiscB, uFlareB, uDiscA, 2.4, glow);
  vec2 uv = base;
  // A pulsar: outside its core, the painting turned with the beams.
  vec2 d = base - uDiscA.xy;
  vec2 n0 = vec2(-uBeam.y, uBeam.x);
  vec2 n = vec2(-uBeam.w, uBeam.z);
  vec2 swung = dot(d, uBeam.zw) / max(uBeamLen.x, 1e-3) * uBeam.xy + dot(d, n) * n0;
  float w = uBeamLen.y * smoothstep(uDiscA.z * 0.9, uDiscA.z * 1.25, length(d));
  uv = uDiscA.xy + mix(d, swung, w);
  vec2 inside = step(vec2(0.0), uv) * step(uv, vec2(1.0));
  vec4 painted = texture(uTexture, uv) * inside.x * inside.y;
  painted = vec4(painted.rgb * glow, painted.a * min(glow, 1.0));
  vec4 color = rolled(painted, uDiscA, uRoll.x);
  color = rolled(color, uDiscB, uRoll.y);
  finalColor = color * vColor;
}
`;

const QUAD_UVS = [0, 0, 1, 0, 1, 1, 0, 1];
const QUAD_INDICES = [0, 1, 2, 0, 2, 3];

/** A star's painting drawn as a turning sphere. Placed like a Sprite: its
 * `anchor` is the texture point at its position; its scale maps texture
 * pixels as a Sprite's does. */
export class RollingStar extends Container {
  readonly anchor: { x: number; y: number; set(x: number, y?: number): void };
  /** The surface's turn as last drawn (radians), for the first disc. */
  turn = 0;
  private time = 0;
  private art: Texture;
  private slug: string | null = null;
  private readonly quad: MeshGeometry;
  private readonly shader: Shader;
  private readonly params: UniformGroup;
  private readonly mesh: Mesh<MeshGeometry, Shader>;

  constructor(texture: Texture, slug: string | null) {
    super();
    this.art = texture;
    const anchor = { x: 0.5, y: 0.5, set: (x: number, y = x) => {
      anchor.x = x;
      anchor.y = y;
      this.fitQuad();
    } };
    this.anchor = anchor;
    this.quad = new MeshGeometry({
      positions: new Float32Array(8),
      uvs: new Float32Array(QUAD_UVS),
      indices: new Uint32Array(QUAD_INDICES),
    });
    this.params = new UniformGroup({
      uDiscA: { value: new Float32Array(4), type: "vec4<f32>" },
      uDiscB: { value: new Float32Array(4), type: "vec4<f32>" },
      uRoll: { value: new Float32Array(2), type: "vec2<f32>" },
      uBeam: { value: new Float32Array([1, 0, 1, 0]), type: "vec4<f32>" },
      uBeamLen: { value: new Float32Array([1, 0]), type: "vec2<f32>" },
      uMeanBias: { value: 0, type: "f32" },
      uFlareA: { value: new Float32Array(4), type: "vec4<f32>" },
      uFlareB: { value: new Float32Array(4), type: "vec4<f32>" },
      uWaves: { value: new Float32Array(4), type: "vec4<f32>" },
      uWaves2: { value: new Float32Array(4), type: "vec4<f32>" },
    });
    this.shader = new Shader({
      glProgram: GlProgram.from({ vertex: VERTEX, fragment: FRAGMENT, name: "rolling-star" }),
      resources: { uTexture: texture.source, starRoll: this.params },
    });
    this.mesh = new Mesh({ geometry: this.quad, shader: this.shader, texture });
    this.addChild(this.mesh);
    this.eventMode = "none";
    this.fitQuad();
    this.setStar(slug);
  }

  get texture(): Texture {
    return this.art;
  }

  set texture(texture: Texture) {
    if (texture === this.art) return;
    this.art = texture;
    this.mesh.texture = texture;
    this.shader.resources.uTexture = texture.source;
    this.fitQuad();
  }

  /** The star type whose surface turns; null keeps the painting still
   * (art this model was not measured on, e.g. the battle backdrop's icons). */
  setStar(slug: string | null): void {
    this.slug = slug;
    const surface = starSurface(slug);
    const u = this.params.uniforms;
    const disc = (out: Float32Array, d: StarDisc | undefined) => {
      out[0] = d?.cx ?? 0.5;
      out[1] = d?.cy ?? 0.5;
      out[2] = d?.r ?? 0;
      out[3] = d && surface.period > 0 ? 1 : 0;
    };
    disc(u.uDiscA as Float32Array, surface.discs[0]);
    disc(u.uDiscB as Float32Array, surface.discs[1]);
    (u.uBeamLen as Float32Array)[1] = surface.beam !== undefined && surface.period > 0 ? 1 : 0;
    this.roll(this.time);
  }

  /** Turn to wall time `t` (s). */
  roll(t: number): void {
    this.time = t;
    const surface = starSurface(this.slug);
    const u = this.params.uniforms;
    const turns = u.uRoll as Float32Array;
    this.turn = this.slug ? starRoll(this.slug, t, surface.discs[0]?.rate ?? 1) : 0;
    turns[0] = this.turn;
    turns[1] = this.slug ? starRoll(this.slug, t, surface.discs[1]?.rate ?? 1) : 0;
    if (surface.beam !== undefined) {
      const beam = pulsarBeam(surface.beam, this.turn);
      const b = u.uBeam as Float32Array;
      b[0] = beam.painted[0];
      b[1] = beam.painted[1];
      b[2] = beam.axis[0];
      b[3] = beam.axis[1];
      (u.uBeamLen as Float32Array)[0] = beam.length;
    }
    // §star-flares: the limb's waves and each disc's eruption.
    const waves = flareWaves(t);
    (u.uWaves as Float32Array).set(waves.lift);
    (u.uWaves2 as Float32Array).set(waves.sway);
    [u.uFlareA as Float32Array, u.uFlareB as Float32Array].forEach((out, i) => {
      const flare = surface.flare;
      if (!this.slug || !flare || !surface.discs[i]) {
        out.fill(0);
        return;
      }
      const eruption = starEruption(this.slug, t, i);
      out.set([flare.strength, flare.reach, eruption.angle, eruption.lift]);
    });
  }

  override destroy(): void {
    this.quad.destroy();
    this.shader.destroy();
    super.destroy({ children: true });
  }

  /** The quad covers the texture, offset by the anchor, in texture pixels. */
  private fitQuad(): void {
    const w = this.art.width;
    const h = this.art.height;
    const x0 = -this.anchor.x * w;
    const y0 = -this.anchor.y * h;
    this.quad.positions = new Float32Array([x0, y0, x0 + w, y0, x0 + w, y0 + h, x0, y0 + h]);
    // A local mean about 1/48 of the painting across, whatever its level.
    this.params.uniforms.uMeanBias = Math.max(0, Math.log2(Math.max(1, w) / 48));
  }
}

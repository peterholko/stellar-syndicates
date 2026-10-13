import { mulberry32 } from "./prng";
import type { BattleBurst } from "./battle-model-policy";

export type BattleParticleKind = "fire" | "smoke" | "shard" | "ring" | "glow";
export interface BattleParticle {
  kind: BattleParticleKind;
  x: number; y: number; height: number;
  sx: number; sy: number; sz: number;
  rx: number; ry: number; rz: number;
  color: number; alpha: number;
}
const clamp = (x: number) => Math.max(0, Math.min(1, x));

/** Stateless geometry samples: seeking directly to a time and playing to it
 * yield exactly the same fragments. A miss never reaches this function. */
export function visitBattleBurst(b: BattleBurst, reducedMotion: boolean, emit: (p: BattleParticle) => void,
  ringTilt = -.18): void {
  if (b.age < 0 || b.age >= b.life || b.life <= 0) return;
  const k = clamp(b.age / b.life), fade = 1 - k, r = b.radius;
  const burst = 1 - Math.pow(fade, 3), death = b.type === "death";
  const rng = mulberry32(b.seed);
  const particle = (kind: BattleParticleKind, x: number, y: number, height: number,
    sx: number, sy: number, sz: number, color: number, alpha: number, rx = 0, ry = 0, rz = 0) => {
    if (alpha > .002) emit({ kind, x: b.x + x, y: b.y + y, height, sx, sy, sz, color, alpha, rx, ry, rz });
  };

  // A soft additive halo gives the mesh a hot edge without fullscreen bloom.
  const halo = r * (death ? 2.8 : 2) * (.35 + burst);
  particle("glow", 0, 0, r * .25, halo, halo, 1, b.color, (reducedMotion ? .24 : .5) * fade * fade);
  if (b.type === "muzzle") return;

  const fireCount = death ? 6 + b.cls * 2 : 3;
  for (let i = 0; i < fireCount; i++) {
    const angle = rng() * Math.PI * 2, phase = i === 0 ? 0 : rng() * .24;
    const reach = .15 + rng() * .8, size = .22 + rng() * .34;
    const elevation = (rng() - .3) * r * .6;
    const local = clamp((k - phase) / (1 - phase));
    if (k < phase) continue;
    const rr = r * size * (.45 + local), alpha = Math.pow(1 - local, 1.5);
    // Saturated fire: yellow-orange cores cooling to deep red (a white core
    // reads as pale bubbles once tone-mapped).
    const color = local < .15 ? 0xffc35a : local < .5 ? (death ? 0xff8a24 : b.color) : 0xb5300e;
    particle("fire", Math.cos(angle) * r * reach * burst, Math.sin(angle) * r * reach * burst,
      r * .35 + elevation * burst, rr, rr * (.8 + size), rr, color, .8 * alpha,
      angle + local, local * 2, angle);
  }

  if (death) {
    const smokeT = clamp((k - .16) / .84);
    for (let i = 0; i < 4 + b.cls; i++) {
      const angle = rng() * Math.PI * 2, reach = .2 + rng() * .9, size = .3 + rng() * .35;
      const rr = r * size * (.6 + smokeT);
      particle("smoke", Math.cos(angle) * r * reach * burst, Math.sin(angle) * r * reach * burst,
        -r * .3, rr, rr * .8, rr, i & 1 ? 0x40363b : 0x252b35, .28 * smokeT * fade, angle, k, 0);
    }
  }

  const ring = r * (.35 + 2.7 * burst);
  if (death || b.type === "flak") particle("ring", 0, 0, r * .2,
    ring, ring, ring, death ? 0xffc374 : b.color, .48 * fade * fade,
    Math.PI / 2 + ringTilt, .12, 0); // the shock front lies in the battlefield plane

  // Hot shrapnel glows and goes out as it cools. The cold metal a blast
  // throws is the hull's own debris, cut from its art by the theater.
  const shards = reducedMotion ? 3 : death ? 8 + b.cls * 4 : 5;
  const glowing = 1 - clamp((k - .18) / .2);
  for (let i = 0; i < shards; i++) {
    const angle = rng() * Math.PI * 2, speed = .7 + rng() * 2.3;
    const elevation = (rng() - .45) * r, size = Math.max(.6, r * (.018 + rng() * .035));
    const spin = (rng() - .5) * 18 * (reducedMotion ? .15 : 1);
    const distance = r * speed * (reducedMotion ? burst * .45 : burst);
    particle("shard", Math.cos(angle) * distance, Math.sin(angle) * distance,
      r * .3 + elevation * burst, size, size * .55, size * (2 + rng() * 2),
      k < .12 ? 0xfff0c0 : 0xffb050, fade * .95 * glowing,
      angle + spin * k, spin * k, angle * 2 + spin * k);
  }
}

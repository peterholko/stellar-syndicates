// Deterministic client-side PRNG utilities — ONE home for the hash + stream
// pair that was previously copy-pasted privately into systemview.ts and
// stars.ts (and is now also the battle theater's cosmetic dice).
//
// DETERMINISM CONTRACTS — do not "improve" these:
//   * `hashId` (FNV-1a over the id string) is replicated BIT-FOR-BIT by the
//     sim (`crates/sim/src/node.rs::node_bonus_for` re-derives the client's
//     exotic-star assignment from it — a parity test asserts agreement).
//   * `mulberry32` streams drive public geography (system layouts) and the
//     battle theater's cosmetic FX; both must render identically for every
//     viewer, so seeds always come from stable ids, never wall-clock.

/// Deterministic 32-bit hash of an id string (FNV-1a).
export function hashId(id: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h >>> 0;
}

/// A tiny fast seeded stream in [0,1) — cosmetics only, never gameplay.
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

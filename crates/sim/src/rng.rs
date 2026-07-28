//! Deterministic, dependency-free pseudo-random number generator.
//!
//! The pure simulation core must be reproducible from `seed + command
//! sequence` (GAME_DESIGN §14). We use SplitMix64 — a tiny, well-distributed
//! generator with no platform-dependent behaviour — so a given seed produces
//! the same galaxy and the same outcomes on every machine.

use serde::{Deserialize, Serialize};

/// A reproducible SplitMix64 generator. `Copy` so callers can snapshot/clone
/// the stream position trivially.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// §keyed-streams: A STREAM PER PURPOSE, derived from the seed and a tag.
    ///
    /// The determinism guarantee only needs "same seed → same outcomes"; it
    /// does NOT need every subsystem drawing from one shared sequence — and a
    /// shared sequence is a trap: adding or removing a single draw anywhere
    /// re-rolls everything downstream of it, so unrelated features perturb
    /// each other and tests fail for reasons that have nothing to do with the
    /// code under test. Keying isolates the streams: `keyed(seed, "pirates")`
    /// is unaffected by how much the market drifted or how many players joined.
    ///
    /// This formalizes what the codebase already did by hand with xor'd magic
    /// constants ("TRAITS_S", "PIRATE_S", "LANE_GEN") — new streams should use
    /// this instead of inventing another constant. FNV-1a over the tag bytes:
    /// tiny, stable across platforms and Rust versions, and never the std
    /// hasher, whose output is not a cross-version promise.
    pub fn keyed(seed: u64, tag: &str) -> Self {
        let mut h: u64 = 0xCBF2_9CE4_8422_2325; // FNV offset basis
        for b in tag.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3); // FNV prime
        }
        Rng::new(seed ^ h)
    }

    /// A keyed stream further split by an id — one independent stream per
    /// entity (`keyed_id(seed, "join", player.0)`), so per-entity outcomes
    /// depend on the entity alone, never on arrival order.
    pub fn keyed_id(seed: u64, tag: &str, id: u64) -> Self {
        let mut r = Rng::keyed(seed, tag);
        // Fold the id through one SplitMix64 step so adjacent ids diverge fully.
        Rng::new(r.next_u64() ^ id.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }

    pub fn new(seed: u64) -> Self {
        // Avoid the trivial all-zero state.
        Rng {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// Next raw 64-bit value (SplitMix64).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform `f64` in `[0, 1)` (53 bits of mantissa precision).
    pub fn next_f64(&mut self) -> f64 {
        // Take the high 53 bits for a uniform double.
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform `f64` in `[lo, hi)`.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Derive an independent child stream from this one (advances `self`).
    /// Useful for giving each subsystem/entity its own reproducible stream.
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_from_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn range_within_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.range(-3.0, 9.0);
            assert!((-3.0..9.0).contains(&v));
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }
}

#[cfg(test)]
mod keyed_tests {
    use super::*;

    /// The properties the isolation rests on: keyed streams are deterministic,
    /// tag-distinct, and id-distinct — and never collide with the bare stream.
    #[test]
    fn keyed_streams_are_deterministic_and_distinct() {
        assert_eq!(Rng::keyed(7, "pirates").next_u64(), Rng::keyed(7, "pirates").next_u64());
        assert_ne!(Rng::keyed(7, "pirates").next_u64(), Rng::keyed(7, "traits").next_u64());
        assert_ne!(Rng::keyed(7, "a").next_u64(), Rng::new(7).next_u64());
        assert_ne!(
            Rng::keyed_id(7, "join", 1).next_u64(),
            Rng::keyed_id(7, "join", 2).next_u64()
        );
        assert_eq!(
            Rng::keyed_id(7, "join", 1).next_u64(),
            Rng::keyed_id(7, "join", 1).next_u64()
        );
    }
}

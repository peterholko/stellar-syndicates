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

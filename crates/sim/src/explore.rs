//! EXPLORATION FOG (§explore) — the deposit knowledge ladder.
//!
//! Deposits used to be public knowledge (a full-galaxy geology dump at join).
//! This module fogs them behind a LADDER — the composition-ladder idiom (§13.1)
//! applied to geology:
//!
//! * **R0/R1 — public, free**: position, name, star type, and a richness BAND
//!   (Poor / Fair / Rich) — the spectral read anyone can take from home. The
//!   band is a pure function of a system's STATIC deposits, so it is the same
//!   for every corp and never changes: safe to ship in the once-at-join galaxy.
//! * **R2 — Surveyed (corp knowledge, permanent)**: the exact deposit table.
//!   Deposits are static, so survey data never stales. Lives in
//!   [`crate::world::Corporation::surveyed`], gated in the server View.
//! * **R3 — Trait (ownership knowledge)**: the system's hidden trait, revealed
//!   to whoever holds it (Part 3).
//!
//! The band VALUE weights are the market's fixed BOOTSTRAP PRICE ANCHORS
//! ([`crate::market::base_price`] — Provisions 6 · Ore 8 · Fuel 10 · Volatiles 18
//! · Alloys 26), NOT live prices: the band must be static and public, and those
//! anchors are already the one fixed per-commodity value table (the client
//! mirrors them as `COMMODITY_VALUE`; `claim_cost_for` uses them too). One
//! source of truth — a second weight table would only drift.
//!
//! Band THRESHOLDS are the terciles of `band_value` across all systems, computed
//! once at galaxy generation and stored on the `World` (deposits never change, so
//! neither do the terciles). Pre-feature snapshots load with the serde default
//! (0,0) and are healed by `World::fixup_after_load` — a pure recompute.

use serde::{Deserialize, Serialize};

use crate::galaxy::Deposit;

// ── Tunables (playtest values; every one a named constant) ──────────────────────

/// Systems within this radius of a corp's HOME are pre-surveyed at join — your
/// starting valley is known; the frontier isn't. Tunable.
pub const SURVEY_INITIAL_RADIUS: f64 = 1200.0;

/// §explore Part 2 — the SURVEY order (the scout's second job). The fleet must
/// close to within this range of the star to survey (hulls on-site only — no
/// remote surveying). Tunable.
pub const SURVEY_RANGE: f64 = 120.0;

/// The DWELL: how long the fleet must hold on-site, uninterrupted, for the
/// survey to complete. All-or-nothing — leaving range or entering an engagement
/// aborts with no partial credit. Tunable.
pub const SURVEY_SECS: f64 = 20.0;

/// ACTIVE SENSING IS LOUD: while dwelling, the fleet's detection signature is
/// multiplied by this (> 1 — seen from farther). Applies during the dwell window
/// ONLY, through the one shared `detection::signature` path — the risk price of
/// knowledge. Tunable.
pub const SURVEY_SIGNATURE_FACTOR: f64 = 1.5;

/// The public richness BAND — the spectral read (R1). Poor / Fair / Rich by the
/// terciles of the galaxy's system values. Should predict ~70% of a system's
/// worth; the survey buys the rest (the exact composition + the trait gamble).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RichnessBand {
    Poor,
    Fair,
    Rich,
}

impl RichnessBand {
    /// Stable machine slug (the wire form the client keys sizing/labels off).
    pub fn slug(self) -> &'static str {
        match self {
            RichnessBand::Poor => "poor",
            RichnessBand::Fair => "fair",
            RichnessBand::Rich => "rich",
        }
    }
}

/// A system's STATIC value scalar: `Σ dep.richness × base_price(commodity)` —
/// richness only (reserves deplete but richness doesn't), weighted by the fixed
/// bootstrap anchors. Pure + deterministic; the single banding input.
pub fn band_value(deposits: &[Deposit]) -> f64 {
    deposits.iter().map(|d| d.richness * crate::market::base_price(d.resource)).sum()
}

/// Bucket a system value against the stored tercile thresholds `(lo, hi)`.
pub fn band_for(value: f64, lo: f64, hi: f64) -> RichnessBand {
    if value <= lo {
        RichnessBand::Poor
    } else if value <= hi {
        RichnessBand::Fair
    } else {
        RichnessBand::Rich
    }
}

/// The tercile thresholds `(lo, hi)` of `band_value` across `systems` — the
/// bottom third are Poor, the middle third Fair, the top third Rich. Sorted by
/// total order (values are finite sums of finite products); deterministic for a
/// given galaxy. Empty/small galaxies degrade gracefully (everything Rich).
pub fn band_thresholds(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let mut v: Vec<f64> = values.collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if v.is_empty() {
        return (0.0, 0.0);
    }
    let n = v.len();
    let lo = v[(n - 1) / 3];
    let hi = v[(2 * (n - 1)) / 3];
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Commodity;

    fn dep(resource: Commodity, richness: f64) -> Deposit {
        Deposit { resource, richness, reserves: None, accessibility: 0.5 }
    }

    /// band_value mirrors the bootstrap anchors (Σ richness × base_price).
    #[test]
    fn band_value_uses_the_bootstrap_anchors() {
        let deps = vec![dep(Commodity::Ore, 2.0), dep(Commodity::Alloys, 1.0)];
        assert_eq!(band_value(&deps), 2.0 * 8.0 + 26.0);
        assert_eq!(band_value(&[]), 0.0);
    }

    /// Terciles split a spread of values into thirds; banding follows them.
    #[test]
    fn terciles_split_into_thirds() {
        // Nine values 1..=9 → lo = v[2] = 3, hi = v[5] = 6.
        let (lo, hi) = band_thresholds((1..=9).map(|x| x as f64));
        assert_eq!((lo, hi), (3.0, 6.0));
        assert_eq!(band_for(2.0, lo, hi), RichnessBand::Poor);
        assert_eq!(band_for(3.0, lo, hi), RichnessBand::Poor); // inclusive lo
        assert_eq!(band_for(5.0, lo, hi), RichnessBand::Fair);
        assert_eq!(band_for(9.0, lo, hi), RichnessBand::Rich);
    }
}

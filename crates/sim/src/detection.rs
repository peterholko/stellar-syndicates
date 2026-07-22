//! Speed-signature detection (§Part 4).
//!
//! Replaces binary dark-ship visibility with a unified, four-factor model shared
//! by BOTH the server's View gating and the sim's picket/platform sensing (one
//! function, so they can never disagree — there's a parity test):
//!
//! ```text
//! detected  ⇔  distance ≤ sensor_capability(observer) × signature(target)
//! ```
//!
//! * `signature(fleet) = size_mult × speed_mult × cloak_mult`
//!   - **size** — per-kind `SIG_SIZE` summed over the composition, with detection
//!     range scaling as √(signal): a big dark pack is seen farther, but
//!     SUB-linearly (flux × inverse-square).
//!   - **speed** — a fleet creeping at the stealth fraction is quietest; ramping
//!     to `SPEED_SIG_MAX×` louder at full formation speed ("flank speed lights
//!     you up").
//!   - **cloak** — a research STUB returning 1.0 (the future cloak-tech hook).
//! * `sensor_capability(range) = bubble_range × SENSOR_TECH_MULT` — the second
//!   research STUB at 1.0.
//!
//! NORMALIZATION ANCHOR (migration-gentle): a SINGLE RAIDER AT FULL SPEED has a
//! total multiplier of exactly **1.0**, so its detection radius is the plain
//! bubble range — today's behavior, byte-for-byte. Scouts (smaller) run quieter,
//! multi-ship dark packs louder, stealth transit quieter.
//!
//! Applies to DARK fleets only (raiders/scouts); broadcasters stay galaxy-visible
//! through the bucket ladder, own fleets exact.

use std::collections::BTreeMap;

use crate::math::Vec2;
use crate::ship::ShipKind;

// --- TUNABLE DETECTION BLOCK ---------------------------------------------
/// Fraction of full formation speed a fleet moves at under STEALTH transit — and
/// the speed at/below which its signature is quietest. Suggest 0.5 (→ 2× trip).
pub const STEALTH_FRACTION: f64 = 0.5;
/// How much LOUDER a fleet at full speed is than one at/below the stealth
/// fraction — the full:stealth signature ratio. "Flank speed lights you up."
pub const SPEED_SIG_MAX: f64 = 2.5;
/// Research STUB (§Part 4): the observer's sensor-tech multiplier on capability.
/// A real no-op at 1.0 today; the future sensor-upgrade hook.
pub const SENSOR_TECH_MULT: f64 = 1.0;

/// Per-kind SIZE SIGNAL (hull-size/mass proxy): scout smallest … colony largest.
/// The RAIDER is the reference (1.0), so a single raider's `size_mult` is exactly
/// 1.0 — the normalization anchor. Tunable.
pub fn sig_size(kind: ShipKind) -> f64 {
    match kind {
        ShipKind::Scout => 0.5,
        ShipKind::Raider => 1.0, // the reference
        ShipKind::Corvette => 2.0,
        ShipKind::Convoy => 4.0,
        ShipKind::Colony => 5.0,
        // §ladder: capitals light sensors from far off — the size proxy tracks
        // the mass ladder (a Titan is unmistakable long before it arrives).
        // Mostly moot in practice: every capital BROADCASTS anyway.
        ShipKind::Destroyer => 3.0,
        ShipKind::Cruiser => 4.5,
        ShipKind::Battleship => 6.5,
        ShipKind::Dreadnought => 9.0,
        ShipKind::Titan => 13.0,
    }
}

/// The reference signal (a single raider) — the `size_mult` normaliser.
const SIG_REF: f64 = 1.0;

/// Research STUB (§Part 4): the target's cloak multiplier on its signature. A
/// real no-op at 1.0 today; the future cloak-tech hook. Kept as a function (not
/// a const) so the hook is a drop-in.
pub fn cloak_mult(_composition: &BTreeMap<ShipKind, u32>) -> f64 {
    1.0
}

/// Total SIZE SIGNAL of a fleet = Σ `sig_size(kind) × count`.
pub fn size_signal(composition: &BTreeMap<ShipKind, u32>) -> f64 {
    composition.iter().map(|(k, n)| sig_size(*k) * *n as f64).sum()
}

/// The SIZE multiplier: √(signal / reference), so range scales as the root of the
/// signal — a single raider = 1.0, six raiders = √6 ≈ 2.45 (farther, but not 6×).
pub fn size_mult(composition: &BTreeMap<ShipKind, u32>) -> f64 {
    (size_signal(composition) / SIG_REF).max(0.0).sqrt()
}

/// The SPEED multiplier from the speed FRACTION `f = speed / max_formation_speed`
/// (evaluated from the retarded SAMPLE's velocity — a fleet that sprinted then
/// coasted is caught by its old flare). Quietest (= `1/SPEED_SIG_MAX`) at/below
/// the stealth fraction, ramping continuously to 1.0 at full speed. Normalised so
/// FULL speed is the 1.0 anchor and the full:stealth ratio is `SPEED_SIG_MAX`.
pub fn speed_mult(speed: f64, max_speed: f64) -> f64 {
    let f = if max_speed > 1e-9 { (speed / max_speed).clamp(0.0, 1.0) } else { 0.0 };
    let quiet = 1.0 / SPEED_SIG_MAX;
    if f <= STEALTH_FRACTION {
        quiet
    } else {
        quiet + (1.0 - quiet) * (f - STEALTH_FRACTION) / (1.0 - STEALTH_FRACTION)
    }
}

/// The full four-factor SIGNATURE of a dark fleet, from its composition (size +
/// cloak) and its retarded speed fraction. A single raider at full speed = 1.0.
pub fn signature(composition: &BTreeMap<ShipKind, u32>, speed: f64, max_speed: f64) -> f64 {
    size_mult(composition) * speed_mult(speed, max_speed) * cloak_mult(composition)
}

/// The observer's SENSOR CAPABILITY for a bubble of `range` — `range ×
/// SENSOR_TECH_MULT` (the stub). Kept separate so the detection rule reads
/// `distance ≤ capability × signature`.
pub fn sensor_capability(range: f64) -> f64 {
    range * SENSOR_TECH_MULT
}

/// THE detection rule (shared by the View and the sim): a target of `signature`
/// is detected if any coverage source `(center, range)` reaches it —
/// `distance ≤ sensor_capability(range) × signature`.
pub fn detected(signature: f64, sources: &[(Vec2, f64)], pos: Vec2) -> bool {
    sources.iter().any(|(center, range)| pos.distance(*center) <= sensor_capability(*range) * signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(items: &[(ShipKind, u32)]) -> BTreeMap<ShipKind, u32> {
        items.iter().copied().collect()
    }

    #[test]
    fn single_raider_at_full_speed_is_the_anchor_exactly_one() {
        let c = comp(&[(ShipKind::Raider, 1)]);
        let max = ShipKind::Raider.max_speed();
        let sig = signature(&c, max, max); // moving at full formation speed
        assert!((sig - 1.0).abs() < 1e-12, "the anchor case must be exactly 1.0 (got {sig})");
    }

    #[test]
    fn stubs_are_provable_no_ops() {
        assert_eq!(SENSOR_TECH_MULT, 1.0);
        assert_eq!(cloak_mult(&comp(&[(ShipKind::Raider, 3)])), 1.0);
        assert_eq!(sensor_capability(2200.0), 2200.0);
    }

    #[test]
    fn stealth_is_quieter_than_full_by_the_speed_ratio() {
        let c = comp(&[(ShipKind::Raider, 1)]);
        let max = ShipKind::Raider.max_speed();
        let full = signature(&c, max, max);
        let stealth = signature(&c, max * STEALTH_FRACTION, max);
        assert!((full / stealth - SPEED_SIG_MAX).abs() < 1e-9, "full is SPEED_SIG_MAX× the stealth signature");
    }

    #[test]
    fn scout_runs_quieter_than_a_raider() {
        let max = ShipKind::Raider.max_speed();
        let raider = signature(&comp(&[(ShipKind::Raider, 1)]), max, max);
        let scout = signature(&comp(&[(ShipKind::Scout, 1)]), max, max);
        assert!(scout < raider, "a scout is smaller → quieter ({scout} < {raider})");
    }

    #[test]
    fn size_aggregates_as_root_signal() {
        // 6 raiders seen farther than 1, but LESS than 6× (√6 ≈ 2.45).
        let six = size_mult(&comp(&[(ShipKind::Raider, 6)]));
        let one = size_mult(&comp(&[(ShipKind::Raider, 1)]));
        assert!(six > one, "a bigger pack is louder");
        assert!((six - 6.0_f64.sqrt()).abs() < 1e-9, "√-aggregation");
        assert!(six < 6.0 * one, "but sub-linearly (not 6×)");
    }

    #[test]
    fn detection_range_scales_with_signature() {
        // A loud fleet is detected from farther than the plain bubble; a quiet one
        // must be closer.
        let sources = [(Vec2::ZERO, 1000.0)];
        let max = ShipKind::Raider.max_speed();
        let loud = comp(&[(ShipKind::Raider, 6)]);
        let sig_loud = signature(&loud, max, max); // ≈ 2.45
        assert!(detected(sig_loud, &sources, Vec2::new(2000.0, 0.0)), "a loud pack is seen well past the bubble");
        let one = comp(&[(ShipKind::Raider, 1)]);
        let sig_stealth = signature(&one, max * STEALTH_FRACTION, max); // 0.4
        assert!(!detected(sig_stealth, &sources, Vec2::new(500.0, 0.0)), "a creeping raider stays hidden inside 500 su (0.4×1000=400)");
        assert!(detected(sig_stealth, &sources, Vec2::new(399.0, 0.0)), "…but is caught once inside 0.4× the bubble");
    }
}

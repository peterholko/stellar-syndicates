//! Core-output detection (§sensors, Oct 16 2026; replaces §Part 4's hull-size ×
//! speed curve).
//!
//! A ship is found by what its CORE puts out — the drive work its reactor is
//! doing — and that light falls off with the square of distance. One function
//! serves BOTH the server's View gating and the sim's own sensing (pickets,
//! defenders, hunters, research observation), so they can never disagree:
//!
//! ```text
//! detected    ⇔  distance ≤ min(sensor_capability(observer) × signature, NOISE_HORIZON_SU)
//! signature   =  √(core_output) × cloak_mult
//! core_output =  mass × rated speed / (Raider mass × Raider speed) × state
//! ```
//!
//! * **mass × rated speed** — the fuel model's own per-second burn
//!   (`fuel::fuel_tick`): hull plus cargo, at the formation's rated speed. It is
//!   the same on thrusters as in warp (warp only covers more ground per
//!   engine-second). It is read off the DRIVE, never the tank: pirates and the
//!   Authority burn nothing in the sim, and fuel research or a captain's thrift
//!   must not double as stealth.
//! * **state** — what the core is doing ([`CoreState`]): full throttle 1.0,
//!   work in place 0.5, Slow warp 0.125, Stealth 0.075, holding on standby 0.05.
//! * **√** — inverse square: four times the output is seen at twice the range.
//!
//! NORMALIZATION ANCHOR: one Raider at full throttle has a signature of exactly
//! **1.0**, so a bubble's quoted range is the distance at which it sees that
//! Raider. A Titan under way is seen about six times as far; anything holding
//! still, at under a quarter of the range.
//!
//! The NOISE HORIZON caps everything: past 200,000 su even a declared
//! transponder drowns in the galaxy's noise, and so does any drive.

use crate::math::Vec2;
use crate::ship::{ShipKind, TransitMode};

// --- TUNABLE DETECTION BLOCK ---------------------------------------------
/// Nothing is detected farther than this from its observer, however loud: the
/// galaxy's background noise. Declared transponders are heard out to exactly
/// this range from a corporation's receivers (Command Center, Sensor Arrays,
/// deep-space sensors).
pub const NOISE_HORIZON_SU: f64 = 200_000.0;
/// Fraction of full formation speed a fleet moves at under STEALTH transit.
pub const STEALTH_FRACTION: f64 = 0.5;
/// §fuel-overhaul: STEALTH runs dark on top of its slow speed — a dampened
/// drive. Its core output is Slow warp's, times this. Tunable.
pub const STEALTH_QUIET: f64 = 0.6;
/// Core on standby: life support and station-keeping, no drive.
pub const HOLD_OUTPUT: f64 = 0.05;
/// Machinery running in place: mining, prospecting, building, tearing down,
/// an active survey sweep, an anchored belt depot.
pub const WORK_OUTPUT: f64 = 0.5;
/// Slow warp on the fission store: half speed on a quarter of the energy per
/// distance — an eighth of the full burn per second.
pub const SLOW_OUTPUT: f64 = crate::fuel::SLOW_SPEED_FRACTION * crate::fuel::SLOW_FUEL_MULT;
/// Stealth: Slow warp's output, dampened.
pub const STEALTH_OUTPUT: f64 = SLOW_OUTPUT * STEALTH_QUIET;
/// A fleet moving faster than this is under way; slower, its core holds.
pub const UNDER_WAY_SU_S: f64 = 1.0;
/// Research STUB (§Part 4): the observer's sensor-tech multiplier on capability.
/// A real no-op at 1.0 today; the future sensor-upgrade hook.
pub const SENSOR_TECH_MULT: f64 = 1.0;

/// What a fleet's core is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    /// Station-keeping on standby.
    Holding,
    /// Holding position with machinery running.
    Working,
    /// Moving at its transit mode's throttle.
    UnderWay(TransitMode),
}

impl CoreState {
    /// This state's share of the hull's full-throttle output.
    pub fn factor(self) -> f64 {
        match self {
            CoreState::Holding => HOLD_OUTPUT,
            CoreState::Working => WORK_OUTPUT,
            CoreState::UnderWay(TransitMode::Full) => 1.0,
            CoreState::UnderWay(TransitMode::Slow) => SLOW_OUTPUT,
            CoreState::UnderWay(TransitMode::Stealth) => STEALTH_OUTPUT,
        }
    }
}

/// The reference output: one Raider at full throttle.
fn output_ref() -> f64 {
    ShipKind::Raider.hull_mass() * ShipKind::Raider.max_speed()
}

/// A formation's full-throttle output (mass × rated speed), in Raiders.
pub fn full_output(mass: f64, rated_speed: f64) -> f64 {
    mass.max(0.0) * rated_speed.max(0.0) / output_ref()
}

/// What a formation's core puts out in `state`, in Raiders at full throttle.
pub fn core_output(mass: f64, rated_speed: f64, state: CoreState) -> f64 {
    full_output(mass, rated_speed) * state.factor()
}

/// Research STUB (§Part 4): the target's cloak multiplier on its signature. A
/// real no-op at 1.0 today; the future cloak-tech hook.
pub fn cloak_mult() -> f64 {
    1.0
}

/// The SIGNATURE of a core putting out `output`: inverse square, so detection
/// reaches `range × √output`. One Raider at full throttle = 1.0.
pub fn signature(output: f64) -> f64 {
    output.max(0.0).sqrt() * cloak_mult()
}

/// The observer's SENSOR CAPABILITY for a bubble of `range` — `range ×
/// SENSOR_TECH_MULT` (the stub). Kept separate so the detection rule reads
/// `distance ≤ capability × signature`.
pub fn sensor_capability(range: f64) -> f64 {
    range * SENSOR_TECH_MULT
}

/// THE detection rule (shared by the View and the sim): a target of `signature`
/// is detected if any coverage source `(center, range)` reaches it, never past
/// the noise horizon.
pub fn detected(signature: f64, sources: &[(Vec2, f64)], pos: Vec2) -> bool {
    sources.iter().any(|(center, range)| {
        pos.distance(*center) <= (sensor_capability(*range) * signature).min(NOISE_HORIZON_SU)
    })
}

/// Can a declared transponder at `pos` be heard by any of these receivers?
pub fn heard(receivers: impl IntoIterator<Item = Vec2>, pos: Vec2) -> bool {
    receivers.into_iter().any(|receiver| receiver.distance(pos) <= NOISE_HORIZON_SU)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raider_full() -> f64 {
        core_output(ShipKind::Raider.hull_mass(), ShipKind::Raider.max_speed(), CoreState::UnderWay(TransitMode::Full))
    }

    #[test]
    fn single_raider_at_full_throttle_is_the_anchor_exactly_one() {
        let sig = signature(raider_full());
        assert!((sig - 1.0).abs() < 1e-12, "the anchor case must be exactly 1.0 (got {sig})");
    }

    #[test]
    fn stubs_are_provable_no_ops() {
        assert_eq!(SENSOR_TECH_MULT, 1.0);
        assert_eq!(cloak_mult(), 1.0);
        assert_eq!(sensor_capability(2200.0), 2200.0);
    }

    #[test]
    fn the_core_states_order_full_work_slow_stealth_holding() {
        let raider = |state| core_output(ShipKind::Raider.hull_mass(), ShipKind::Raider.max_speed(), state);
        let states = [
            CoreState::UnderWay(TransitMode::Full),
            CoreState::Working,
            CoreState::UnderWay(TransitMode::Slow),
            CoreState::UnderWay(TransitMode::Stealth),
            CoreState::Holding,
        ];
        let outputs: Vec<f64> = states.iter().map(|s| raider(*s)).collect();
        assert!(outputs.windows(2).all(|w| w[0] > w[1]), "{outputs:?}");
        assert!((outputs[2] - 0.125).abs() < 1e-12, "Slow: half speed on a quarter of the energy");
        assert!((outputs[3] - 0.125 * STEALTH_QUIET).abs() < 1e-12, "Stealth: Slow's output, dampened");
        assert!((signature(outputs[4]) - 0.05_f64.sqrt()).abs() < 1e-12, "a holding Raider is seen at √0.05");
    }

    #[test]
    fn output_follows_mass_and_rated_speed() {
        let full = CoreState::UnderWay(TransitMode::Full);
        let scout = core_output(ShipKind::Scout.hull_mass(), ShipKind::Scout.max_speed(), full);
        let titan = core_output(ShipKind::Titan.hull_mass(), ShipKind::Titan.max_speed(), full);
        assert!(scout < 1.0, "a scout is lighter → quieter ({scout})");
        assert!(signature(titan) > 6.0, "a Titan under way is seen six times as far");
        let two = core_output(2.0 * ShipKind::Raider.hull_mass(), ShipKind::Raider.max_speed(), full);
        assert!((signature(two) - 2.0_f64.sqrt()).abs() < 1e-12, "two Raiders: √2, not 2×");
        let laden = core_output(ShipKind::Raider.hull_mass() + 200.0, ShipKind::Raider.max_speed(), full);
        assert!(laden > raider_full(), "cargo is mass: a laden hull runs louder");
    }

    #[test]
    fn detection_range_scales_with_signature() {
        let sources = [(Vec2::ZERO, 1000.0)];
        let loud = signature(6.0); // ≈ 2.45
        assert!(detected(loud, &sources, Vec2::new(2000.0, 0.0)), "a loud pack is seen well past the bubble");
        let holding = signature(HOLD_OUTPUT * raider_full()); // ≈ 0.224
        assert!(!detected(holding, &sources, Vec2::new(300.0, 0.0)), "a holding Raider hides at 300 su");
        assert!(detected(holding, &sources, Vec2::new(220.0, 0.0)), "…but is caught inside 0.224× the bubble");
    }

    #[test]
    fn nothing_is_detected_past_the_noise_horizon() {
        let sources = [(Vec2::ZERO, 70_000.0)];
        let titan = signature(40.0);
        assert!(detected(titan, &sources, Vec2::new(NOISE_HORIZON_SU - 1.0, 0.0)));
        assert!(!detected(titan, &sources, Vec2::new(NOISE_HORIZON_SU + 1.0, 0.0)), "lost in the noise");
        assert!(heard([Vec2::ZERO], Vec2::new(NOISE_HORIZON_SU, 0.0)));
        assert!(!heard([Vec2::ZERO, Vec2::new(-1.0, 0.0)], Vec2::new(NOISE_HORIZON_SU + 1.0, 0.0)));
    }
}

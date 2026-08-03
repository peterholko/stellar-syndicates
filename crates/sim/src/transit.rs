//! Straight-line transit and information propagation.
//!
//! The retired hyperspace layer used geography and player-built relays to
//! modify both hull and signal speed. The live model has only two cruise
//! regimes: thrusters inside gravity wells (or with warp disabled), and warp
//! everywhere else. Information always travels directly at warp light.

use serde::{Deserialize, Serialize};

use crate::math::Vec2;

/// Warp speed as a multiple of a hull's thruster speed. Information uses the
/// same factor over normal-space `c`.
pub const WARP_FACTOR: f64 = 5.0;

/// Warp drive spin-up and shut-down times, in seconds.
pub const WARP_SPOOL_S: f64 = 1.5;
pub const WARP_DROP_S: f64 = 1.0;

/// A warp fleet cannot steer. A meaningful heading change drops the drive so
/// the hull can turn on thrusters before spooling again.
pub const COURSE_LOCK_RAD: f64 = 0.09;

/// Jump-drive playtest tunables.
pub const JUMP_SPOOL_S: f64 = 10.0;
pub const JUMP_RANGE: f64 = 50_000.0;
pub const JUMP_FUEL_FACTOR: f64 = WARP_FACTOR;

/// Radius of a gravity well. Warp and jump drives cannot operate inside it.
pub const HYPERLIMIT: f64 = 900.0;

/// Which drive is carrying a fleet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Regime {
    #[default]
    Thrusters,
    Warp,
}

impl Regime {
    pub fn warp() -> Self {
        Self::Warp
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Thrusters => "impulse",
            Self::Warp => "warp",
        }
    }
}

pub fn spool_seconds(to: Regime) -> f64 {
    match to {
        Regime::Thrusters => 0.0,
        Regime::Warp => WARP_SPOOL_S,
    }
}

pub fn drop_seconds(from: Regime) -> f64 {
    match from {
        Regime::Thrusters => 0.0,
        Regime::Warp => WARP_DROP_S,
    }
}

/// Static gravity-well environment used by fleet movement.
pub struct TransitEnv<'a> {
    pub wells: &'a [(Vec2, f64)],
}

impl TransitEnv<'_> {
    pub fn in_well(&self, p: Vec2) -> bool {
        self.wells
            .iter()
            .any(|(centre, radius)| p.distance(*centre) <= *radius)
    }

    pub fn factor(&self, p: Vec2, _dir: Vec2, drive_on: bool) -> f64 {
        if drive_on && !self.in_well(p) {
            WARP_FACTOR
        } else {
            1.0
        }
    }

    pub fn regime(&self, p: Vec2, dir: Vec2, drive_on: bool) -> Regime {
        if self.factor(p, dir, drive_on) >= WARP_FACTOR - 1e-9 {
            Regime::Warp
        } else {
            Regime::Thrusters
        }
    }
}

/// Speed of all information in the plain model: straight warp light.
pub fn signal_speed(c: f64) -> f64 {
    c * WARP_FACTOR
}

/// Straight-line information delay between two points.
pub fn delay(a: Vec2, b: Vec2, c: f64) -> f64 {
    a.distance(b) / signal_speed(c)
}

/// Solve when a straight warp-light command meets a linearly moving receiver.
///
/// The receiver's velocity is frozen from the picture at issue time. Eight
/// refinements are sufficient at the enforced light/hull speed ratio; stopping
/// within one sim tick avoids pretending to know sub-tick physics.
pub fn command_meeting_delay(
    source: Vec2,
    receiver_pos: Vec2,
    receiver_vel: Vec2,
    c: f64,
    delay_factor: f64,
) -> (f64, Vec2) {
    let travel_time = |target| delay(source, target, c) * delay_factor;
    let mut seconds = travel_time(receiver_pos);
    for _ in 0..8 {
        let meeting = receiver_pos + receiver_vel * seconds;
        let next = travel_time(meeting);
        let converged = (next - seconds).abs() < crate::config::DT;
        seconds = next;
        if converged {
            break;
        }
    }
    let meeting = receiver_pos + receiver_vel * seconds;
    (seconds, meeting)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_light_uses_warp_speed() {
        assert_eq!(delay(Vec2::ZERO, Vec2::new(10_000.0, 0.0), 400.0), 5.0);
    }

    #[test]
    fn meeting_solve_converges_for_inbound_and_outbound_receivers() {
        let source = Vec2::ZERO;
        for velocity in [Vec2::new(-100.0, 0.0), Vec2::new(100.0, 0.0)] {
            let (seconds, point) =
                command_meeting_delay(source, Vec2::new(10_000.0, 0.0), velocity, 400.0, 1.0);
            assert!((delay(source, point, 400.0) - seconds).abs() < crate::config::DT);
        }
    }
}

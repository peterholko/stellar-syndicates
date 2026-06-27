//! Flip-and-burn movement (§7).
//!
//! Ships accelerate toward a destination, flip at the midpoint, and decelerate
//! to arrive **at rest** — the engine *always* plans the arrival burn, so the
//! player never manages momentum (no overshoot, no Newtonian misery). Travel
//! time is non-linear: `t ≈ 2·√(distance / acceleration)`.
//!
//! Implemented as an acceleration-limited velocity-matching controller: each
//! tick it picks the fastest speed from which the ship can still brake to rest
//! within the remaining distance (`√(2·a·d)`, capped at `max_speed`) and steers
//! toward that velocity within the per-tick acceleration budget. This is stable
//! under a discrete timestep, reduces to the clean trapezoidal/triangular
//! profile of §7 for a from-rest straight run, and generalises to a moving
//! destination (the M4 intercept) without change.

use serde::{Deserialize, Serialize};

use crate::math::Vec2;

/// Distance (sim units) within which a ship is considered to have arrived.
const ARRIVE_DIST: f64 = 2.0;

/// The result of advancing a ship one tick toward a destination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MoveStep {
    pub pos: Vec2,
    pub vel: Vec2,
    /// True once the ship has reached the destination at rest.
    pub arrived: bool,
}

/// Advance a body one timestep toward `dest`, planning the arrival burn.
///
/// * `accel`     — acceleration magnitude (sim units / s²)
/// * `max_speed` — cruise speed cap (sim units / s); kept below `c`
pub fn flip_and_burn(
    pos: Vec2,
    vel: Vec2,
    dest: Vec2,
    accel: f64,
    max_speed: f64,
    dt: f64,
) -> MoveStep {
    let to_dest = dest - pos;
    let dist = to_dest.length();

    // Already there (and essentially stopped): snap to rest.
    if dist <= ARRIVE_DIST && vel.length() <= accel * dt {
        return MoveStep { pos: dest, vel: Vec2::ZERO, arrived: true };
    }

    let dir = to_dest / dist; // unit vector toward the destination
    // Fastest speed from which we can still brake to rest within `dist`.
    let v_brake = (2.0 * accel * dist).sqrt();
    let target_speed = max_speed.min(v_brake);
    let desired_vel = dir * target_speed;

    // Steer toward the desired velocity, limited by this tick's accel budget.
    let dv = desired_vel - vel;
    let dv_len = dv.length();
    let max_dv = accel * dt;
    let applied = if dv_len > max_dv && dv_len > 1e-12 {
        dv * (max_dv / dv_len)
    } else {
        dv
    };

    let new_vel = vel + applied;
    let new_pos = pos + new_vel * dt;

    // Did the step land us at the destination?
    if (dest - new_pos).length() <= ARRIVE_DIST && new_vel.length() <= accel * dt {
        MoveStep { pos: dest, vel: Vec2::ZERO, arrived: true }
    } else {
        MoveStep { pos: new_pos, vel: new_vel, arrived: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DT;

    /// Simulate a from-rest run and return (arrival_time, max_speed_seen, final_pos).
    fn run(dist: f64, accel: f64, max_speed: f64) -> (f64, f64, Vec2) {
        let dest = Vec2::new(dist, 0.0);
        let mut pos = Vec2::ZERO;
        let mut vel = Vec2::ZERO;
        let mut t = 0.0;
        let mut vmax = 0.0_f64;
        for _ in 0..1_000_000 {
            let step = flip_and_burn(pos, vel, dest, accel, max_speed, DT);
            pos = step.pos;
            vel = step.vel;
            t += DT;
            vmax = vmax.max(vel.length());
            if step.arrived {
                return (t, vmax, pos);
            }
        }
        panic!("did not arrive");
    }

    #[test]
    fn arrives_at_rest_at_destination() {
        let (_t, _v, pos) = run(5000.0, 30.0, 1e9);
        assert!((pos - Vec2::new(5000.0, 0.0)).length() <= ARRIVE_DIST);
    }

    #[test]
    fn travel_time_matches_two_root_d_over_a() {
        // With no speed cap, a triangular profile gives t ≈ 2√(d/a).
        let dist = 5000.0;
        let accel = 20.0;
        let (t, _v, _p) = run(dist, accel, 1e9);
        let expected = 2.0 * (dist / accel).sqrt();
        let rel_err = (t - expected).abs() / expected;
        assert!(rel_err < 0.05, "t={t} expected≈{expected} rel_err={rel_err}");
    }

    #[test]
    fn respects_max_speed_cap() {
        let cap = 40.0;
        let (_t, vmax, _p) = run(20000.0, 30.0, cap);
        // Never exceeds the cap by more than one tick's acceleration.
        assert!(vmax <= cap + 30.0 * DT + 1e-6, "vmax={vmax} cap={cap}");
    }

    #[test]
    fn faster_ship_arrives_sooner() {
        let (t_slow, _, _) = run(4000.0, 9.0, 36.0); // convoy-like
        let (t_fast, _, _) = run(4000.0, 30.0, 90.0); // raider-like
        assert!(t_fast < t_slow, "raider {t_fast} should beat convoy {t_slow}");
    }
}

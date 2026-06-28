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

/// One tick of **proportional pursuit** (§7, §8) — steer-and-correct, NOT a
/// closed-form intercept solver. Each tick the pursuer simply:
///   1. forms a crude, light-delayed read of where the target IS — the position
///      its arriving light shows, `target_pos − target_vel·(range/c)` (a
///      constant-velocity retardation; it sharpens to the truth as `range→0`,
///      exactly like the fog model: act on a stale observation, correct as
///      fresher light arrives);
///   2. steers toward that observed point, accelerating within its budget while
///      easing toward the target's velocity as range closes (a brake term), so
///      it slides into contact instead of blowing past into an orbit (no donut).
///
/// Convergence is EMERGENT from this feedback loop, like a guided missile — there
/// is no boundary-value solver. Cheap and robust: it doesn't depend on any
/// prediction being right, only on the error shrinking as range closes. Contact
/// itself is decided by the world (within `CONTACT_RADIUS`). Pass `c = INFINITY`
/// to pursue the true present position (used by tests).
#[allow(clippy::too_many_arguments)] // a focused kinematics step, not a config
pub fn pursue_step(
    pos: Vec2,
    vel: Vec2,
    target_pos: Vec2,
    target_vel: Vec2,
    accel: f64,
    max_speed: f64,
    c: f64,
    dt: f64,
) -> MoveStep {
    // (1) Where the pursuer SEES the target: light-delayed by the current range.
    let range = (target_pos - pos).length();
    let obs_delay = if c.is_finite() && c > 1e-9 { range / c } else { 0.0 };
    let observed = target_pos - target_vel * obs_delay;

    let to_obs = observed - pos;
    let obs_range = to_obs.length();
    let dir = if obs_range > 1e-9 { to_obs / obs_range } else { Vec2::ZERO };

    // (2) Brake term: never close faster than we can bleed off to MATCH the
    //     target's velocity within the remaining range (√(2·a·d)). So far out we
    //     run flat-out toward it, and as range shrinks we ease alongside for a
    //     clean contact rather than slamming past into a loop.
    let closing = (2.0 * accel * obs_range).sqrt();
    let mut desired = target_vel + dir * closing;
    let ds = desired.length();
    if ds > max_speed && ds > 1e-12 {
        desired = desired * (max_speed / ds);
    }

    // Steer toward the desired velocity within this tick's acceleration budget.
    let dv = desired - vel;
    let dv_len = dv.length();
    let max_dv = accel * dt;
    let applied = if dv_len > max_dv && dv_len > 1e-12 {
        dv * (max_dv / dv_len)
    } else {
        dv
    };
    let new_vel = vel + applied;
    let new_pos = pos + new_vel * dt;
    MoveStep {
        pos: new_pos,
        vel: new_vel,
        arrived: false, // interception ends on contact, decided by the world
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

    /// Proportional pursuit (steer-and-correct, no closed-form solver) runs a
    /// fleeing target down, converging cleanly to contact — and over a watchable
    /// span of TENS OF SECONDS at the tuned acceleration, not an instant.
    #[test]
    fn pursuit_runs_down_a_fleeing_target_and_makes_contact() {
        let (accel, max_speed, c) = (11.0, 120.0, 300.0); // raider-like
        const CONTACT: f64 = 80.0;
        let mut pos = Vec2::ZERO;
        let mut vel = Vec2::ZERO;
        let mut tpos = Vec2::new(3000.0, 0.0);
        let tvel = Vec2::new(40.0, 0.0); // convoy fleeing along +x at cruise

        let mut t = 0.0;
        let mut contact_t: Option<f64> = None;
        let mut min_dist = f64::INFINITY;
        for _ in 0..(150.0 / DT) as usize {
            let step = pursue_step(pos, vel, tpos, tvel, accel, max_speed, c, DT);
            pos = step.pos;
            vel = step.vel;
            tpos = tpos + tvel * DT;
            t += DT;
            let d = (tpos - pos).length();
            min_dist = min_dist.min(d);
            if d <= CONTACT && contact_t.is_none() {
                contact_t = Some(t);
            }
        }
        let ct = contact_t.unwrap_or_else(|| panic!("pursuer never made contact; closest it got was {min_dist:.0} su"));
        // Watchable, not instant — and not absurdly long.
        assert!((5.0..90.0).contains(&ct), "chase should resolve in tens of seconds, took {ct:.1}s");
        // Never blows wildly past: cruise cap means closing speed is bounded, and
        // the brake eases it in — so the first contact really is a contact.
        assert!(vel.length() <= max_speed + accel * DT + 1e-6, "must respect the speed cap");
    }
}

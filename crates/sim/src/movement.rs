//! Constant-velocity movement (§14.1).
//!
//! Playtesting retired the flip-and-burn acceleration model (§7): at the async
//! check-in cadence the burn was invisible, and its `t = 2√(d/a)` travel law
//! defeated the mental arithmetic a lightspeed-prediction game needs. We restore
//! the GDD §14.1 original: **constant-velocity, piecewise-linear movement** — a
//! fleet departs at its (per-kind, formation-capped) speed straight toward the
//! destination and stops on arrival. Travel time is simply `t = d / v`.
//!
//! The payoff the design doc always intended: piecewise-LINEAR trajectories make
//! retarded-position observation and command/intercept interception **analytic**
//! (a closed-form lead, not a feedback controller). Pursuit is lead-pursuit
//! against a constant-velocity target.

use serde::{Deserialize, Serialize};

use crate::math::Vec2;

/// Distance (sim units) within which a body is considered to have arrived.
const ARRIVE_DIST: f64 = 2.0;

/// The result of advancing a body one tick toward a destination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MoveStep {
    pub pos: Vec2,
    pub vel: Vec2,
    /// True once the body has reached the destination.
    pub arrived: bool,
}

/// Advance a body one timestep toward `dest` at CONSTANT `speed` (§14.1). It
/// travels in a straight line at `speed` and stops exactly on arrival — no
/// acceleration, no overshoot. `vel` is reported as `speed × direction` (for
/// heading / dead-reckoning), zero once arrived.
pub fn advance_toward(pos: Vec2, dest: Vec2, speed: f64, dt: f64) -> MoveStep {
    let to_dest = dest - pos;
    let dist = to_dest.length();
    let step = speed * dt;

    // Arrived (or the final partial step lands us there): snap to the dest.
    if dist <= ARRIVE_DIST || dist <= step {
        return MoveStep { pos: dest, vel: Vec2::ZERO, arrived: true };
    }
    let dir = to_dest / dist;
    MoveStep { pos: pos + dir * step, vel: dir * speed, arrived: false }
}

/// The ANALYTIC interception point (§14.1): where a pursuer at `p` moving at
/// constant `speed` should aim to catch a target at `t` moving at constant
/// velocity `vt`. Solves `|t + vt·τ − p| = speed·τ` for the smallest `τ ≥ 0` and
/// returns `t + vt·τ`. `None` if no interception exists (target faster and
/// opening) — the caller falls back to pure pursuit (aim at the target now).
pub fn intercept_point(p: Vec2, speed: f64, t: Vec2, vt: Vec2) -> Option<Vec2> {
    let d = t - p;
    // (vt·vt − s²)τ² + 2(d·vt)τ + (d·d) = 0
    let a = vt.dot(vt) - speed * speed;
    let b = 2.0 * d.dot(vt);
    let c = d.dot(d);
    let tau = if a.abs() < 1e-9 {
        // Degenerate to linear: bτ + c = 0.
        if b.abs() < 1e-12 {
            return None;
        }
        -c / b
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return None;
        }
        let sq = disc.sqrt();
        let (t1, t2) = ((-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a));
        // Smallest non-negative root.
        [t1, t2].into_iter().filter(|x| *x >= -1e-9).fold(f64::INFINITY, f64::min)
    };
    if tau.is_finite() && tau >= -1e-9 {
        Some(t + vt * (tau.max(0.0)))
    } else {
        None
    }
}

/// One tick of **lead pursuit** (§8, §14.1) at constant speed. The pursuer:
///   1. forms a light-delayed read of where the target IS — the position its
///      arriving light shows, `target_pos − target_vel·(range/c)` (a
///      constant-velocity retardation; act on the stale sighting, sharpen as
///      fresher light arrives, exactly like the fog model);
///   2. solves the ANALYTIC intercept against that observed constant-velocity
///      target and steers straight at the lead point at `speed`.
///
/// Contact itself is decided by the world (within `CONTACT_RADIUS`). Pass
/// `c = INFINITY` to pursue the true present position (used by tests).
pub fn pursue_step(pos: Vec2, target_pos: Vec2, target_vel: Vec2, speed: f64, c: f64, dt: f64) -> MoveStep {
    // (1) Light-delayed observation of the target.
    let range = (target_pos - pos).length();
    let obs_delay = if c.is_finite() && c > 1e-9 { range / c } else { 0.0 };
    let observed = target_pos - target_vel * obs_delay;

    // (2) Analytic lead against the observed constant-velocity target; fall back
    //     to pure pursuit (aim at it now) when no interception exists.
    let aim = intercept_point(pos, speed, observed, target_vel).unwrap_or(observed);
    let to_aim = aim - pos;
    let d = to_aim.length();
    let dir = if d > 1e-9 { to_aim / d } else { Vec2::ZERO };
    let new_vel = dir * speed;
    MoveStep {
        pos: pos + new_vel * dt,
        vel: new_vel,
        arrived: false, // interception ends on contact, decided by the world
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DT;

    /// Simulate a from-rest run at constant speed; return (arrival_time, final_pos).
    fn run(dist: f64, speed: f64) -> (f64, Vec2) {
        let dest = Vec2::new(dist, 0.0);
        let mut pos = Vec2::ZERO;
        let mut t = 0.0;
        for _ in 0..10_000_000 {
            let step = advance_toward(pos, dest, speed, DT);
            pos = step.pos;
            t += DT;
            if step.arrived {
                return (t, pos);
            }
        }
        panic!("did not arrive");
    }

    #[test]
    fn arrives_exactly_at_destination() {
        let (_t, pos) = run(5000.0, 30.0);
        assert!((pos - Vec2::new(5000.0, 0.0)).length() <= ARRIVE_DIST);
    }

    #[test]
    fn travel_time_is_distance_over_speed() {
        let (dist, speed) = (5000.0, 25.0);
        let (t, _p) = run(dist, speed);
        let expected = dist / speed;
        let rel_err = (t - expected).abs() / expected;
        assert!(rel_err < 0.01, "t={t} expected={expected} (t = d/v)");
    }

    #[test]
    fn never_exceeds_the_constant_speed() {
        let speed = 40.0;
        let dest = Vec2::new(20000.0, 0.0);
        let mut pos = Vec2::ZERO;
        for _ in 0..100 {
            let step = advance_toward(pos, dest, speed, DT);
            assert!(step.vel.length() <= speed + 1e-9, "speed is constant, never exceeded");
            pos = step.pos;
        }
    }

    #[test]
    fn faster_body_arrives_sooner() {
        let (t_slow, _) = run(4000.0, 30.0); // convoy-like
        let (t_fast, _) = run(4000.0, 75.0); // raider-like
        assert!(t_fast < t_slow, "faster {t_fast} beats slower {t_slow}");
    }

    #[test]
    fn intercept_point_is_analytic_and_correct() {
        // Pursuer at origin, speed 75; target at (1000,0) moving +y at 30.
        let p = Vec2::ZERO;
        let (t, vt) = (Vec2::new(1000.0, 0.0), Vec2::new(0.0, 30.0));
        let aim = intercept_point(p, 75.0, t, vt).expect("interception exists");
        // The lead point must be reachable in equal time by both.
        let tau_target = (aim.y - t.y) / vt.y;
        let tau_pursuer = (aim - p).length() / 75.0;
        assert!((tau_target - tau_pursuer).abs() < 1e-6, "both reach the aim point at the same time");
    }

    #[test]
    fn no_interception_when_target_outruns_the_pursuer() {
        // Target directly ahead, moving away faster than the pursuer can chase.
        let aim = intercept_point(Vec2::ZERO, 30.0, Vec2::new(100.0, 0.0), Vec2::new(60.0, 0.0));
        assert!(aim.is_none(), "a faster, opening target cannot be intercepted");
    }

    /// Lead pursuit runs a fleeing target down and makes contact, over a
    /// watchable span (tens of seconds), never exceeding the constant speed.
    #[test]
    fn pursuit_runs_down_a_fleeing_target_and_makes_contact() {
        let (speed, c) = (75.0, 300.0); // raider-like
        const CONTACT: f64 = 80.0;
        let mut pos = Vec2::ZERO;
        let mut tpos = Vec2::new(3000.0, 0.0);
        let tvel = Vec2::new(30.0, 0.0); // convoy fleeing at cruise
        let mut t = 0.0;
        let mut contact_t: Option<f64> = None;
        let mut min_dist = f64::INFINITY;
        for _ in 0..(200.0 / DT) as usize {
            let step = pursue_step(pos, tpos, tvel, speed, c, DT);
            pos = step.pos;
            tpos = tpos + tvel * DT;
            t += DT;
            let d = (tpos - pos).length();
            min_dist = min_dist.min(d);
            if d <= CONTACT && contact_t.is_none() {
                contact_t = Some(t);
            }
        }
        let ct = contact_t.unwrap_or_else(|| panic!("never made contact; closest {min_dist:.0} su"));
        assert!((5.0..120.0).contains(&ct), "chase resolves in tens of seconds, took {ct:.1}s");
    }
}

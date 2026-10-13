//! Hulls are bodies. Every hull keeps clear of the others (smaller hulls give
//! way to bigger ones), and a large hull (8000+ mass) that dies leaves its
//! sections as wreckage: obstacles that drift with the dead hull's momentum,
//! that hulls steer around, that stop torpedoes, and that damage a hull ramming
//! them. Beams and mass drivers still fire through wreckage. Everything else,
//! including v8's weaving, is unchanged. Frozen: new battles open on v10
//! (`open_current`), which keeps all of this.

use super::rules_v1::{hull_mass, Rng};
use super::v1::{
    Combatant, Distribution, ProjSetup, SimOutcome, StepOutcome, TacticalState, ACCEL_CAL, PROJECTION_BATTLE_ID,
};
use super::{v3, Math, Rules};
use crate::{doctrine::MissionProfile, ship::Ship, EntityId, ShipKind, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Hulls this massive leave physical wreckage.
pub const LARGE_HULL_MASS: f64 = 8000.0;
/// Hulls start steering clear at this multiple of their contact distance.
const AVOID_REACH: f64 = 1.6;
/// Wreckage keeps this share of its speed each step.
const WRECK_DRAG: f64 = 0.8;
/// Past this many wrecked hulls, the oldest wreck breaks up for good.
pub const MAX_WRECK_HULLS: usize = 8;
/// A hull ramming wreckage at this closing speed (arena units per step) takes
/// the full `RAM_MAX_SHARE` of its hull; slower impacts scale with the square.
const RAM_SPEED: f64 = 100.0;
const RAM_MAX_SHARE: f64 = 0.5;
/// Gentler contact than this is a scrape, not an impact.
const RAM_MIN_SPEED: f64 = 4.0;
/// An independent stream: wreckage never spends a targeting or damage roll.
const WRECK_SALT: u64 = 0x5752_4543_4b41_4745;

/// A section of a dead large hull: a physical obstacle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wreck {
    /// Battle-local id of the hull it came from.
    pub hull: u32,
    /// Section index from the bow.
    pub piece: u8,
    pub side: u8,
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f64,
    pub mass: f64,
    /// The step the sections fly apart (`sep` is added to the velocity then).
    pub breach: u64,
    pub sep: Vec2,
}

impl TacticalState {
    /// Frozen v9 constructor (new battles open on the current rules: `open_current`).
    pub(crate) fn open_v9(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v8(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V8 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V9 { maneuver_seed };
        state
    }
}

/// A hull's body radius, arena units: about 30% of its drawn length in the
/// battle viewer, so hulls may brush at the tips but never stack.
pub fn body_radius(kind: ShipKind, platform: bool, math: Math) -> f64 {
    if platform {
        return 33.0;
    }
    let length = match kind {
        ShipKind::Raider => 1.0,
        ShipKind::Corvette => 1.5,
        ShipKind::Destroyer => 2.5,
        ShipKind::Cruiser => 4.0,
        ShipKind::Battleship => 5.0,
        ShipKind::Dreadnought => 6.0,
        ShipKind::Titan => 8.0,
        // The viewer's size ladder for every other hull.
        other => (0.7 * math.powf(hull_mass(other) / 100.0, 0.4)).clamp(1.0, 6.4),
    };
    12.6 * length
}

/// How fast a hull heavier than a destroyer can change course, radians per
/// step (the battle viewer turns its nose at the same rate). Destroyers and
/// lighter hulls turn as freely as their acceleration allows.
pub fn turn_rate(kind: ShipKind) -> Option<f64> {
    Some(match kind {
        ShipKind::Cruiser => 0.30,
        ShipKind::Convoy => 0.28,
        ShipKind::Colony | ShipKind::Freighter => 0.24,
        ShipKind::Transport => 0.21,
        ShipKind::Battleship | ShipKind::LargeFreighter => 0.20,
        ShipKind::HeavyFreighter => 0.14,
        ShipKind::Dreadnought => 0.13,
        ShipKind::BulkFreighter => 0.10,
        ShipKind::Titan => 0.085,
        _ => return None,
    })
}

/// A slow-turning hull's new velocity: its course swings toward the one it
/// asked for by at most its turn rate a step, keeping the asked-for speed (so
/// asked to reverse, it brakes as it comes about rather than flipping round).
pub(super) fn turn_capped(kind: ShipKind, before: Vec2, after: Vec2, math: Math) -> Vec2 {
    let Some(rate) = turn_rate(kind) else { return after };
    let (s0, s1) = (before.length(), after.length());
    if s0 < 1e-6 || s1 < 1e-6 { return after; }
    let from = math.atan2(before.y, before.x);
    let mut d = math.atan2(after.y, after.x) - from;
    while d > std::f64::consts::PI { d -= std::f64::consts::TAU; }
    while d <= -std::f64::consts::PI { d += std::f64::consts::TAU; }
    if d.abs() <= rate { return after; }
    math.polar(from + rate * d.signum(), s1)
}

/// Sections a large hull's wreck breaks into (0: not large enough to leave
/// physical wreckage). Matches the battle viewer's break-up table.
pub fn wreck_pieces(kind: ShipKind) -> u8 {
    if hull_mass(kind) < LARGE_HULL_MASS {
        return 0;
    }
    match kind {
        ShipKind::Titan => 5,
        ShipKind::Dreadnought | ShipKind::HeavyFreighter | ShipKind::BulkFreighter => 4,
        _ => 3,
    }
}

fn body(c: &Combatant, math: Math) -> (f64, f64) {
    // Platforms are emplacements: they never give way.
    (body_radius(c.kind, c.platform, math), if c.platform { f64::INFINITY } else { c.max_hp.max(1.0) })
}

/// Every close pair (i < j) of `points` within `reach` of each other, found by
/// a sweep along x (deterministic, near-linear for spread-out battles).
fn close_pairs(points: &[Vec2], reach: f64, mut visit: impl FnMut(usize, usize)) {
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by(|&a, &b| points[a].x.total_cmp(&points[b].x).then(a.cmp(&b)));
    for (k, &i) in order.iter().enumerate() {
        for &j in &order[k + 1..] {
            if points[j].x - points[i].x > reach { break; }
            if (points[j].y - points[i].y).abs() <= reach {
                visit(i.min(j), i.max(j));
            }
        }
    }
}

/// Shares of a separation each body takes: the lighter gives way more; an
/// immovable body (a platform) gives none.
fn shares(ma: f64, mb: f64) -> (f64, f64) {
    match (ma.is_finite(), mb.is_finite()) {
        (false, false) => (0.0, 0.0),
        (false, true) => (0.0, 1.0),
        (true, false) => (1.0, 0.0),
        (true, true) => (mb / (ma + mb), ma / (ma + mb)),
    }
}

/// A fixed direction for two bodies at exactly the same point.
fn tie_break(i: usize, math: Math) -> Vec2 {
    math.polar(i as f64 * 2.399_963, 1.0)
}

/// Steering for each hull, added to its desired point: away from nearby hulls
/// (the lighter giving way more) and from wreckage.
pub(super) fn avoidance(state: &TacticalState) -> Vec<Vec2> {
    let math = state.math();
    let n = state.combatants.len();
    let mut push = vec![Vec2::ZERO; n];
    let points: Vec<Vec2> = state.combatants.iter().map(|c| c.pos).collect();
    let bodies: Vec<(f64, f64)> = state.combatants.iter().map(|c| body(c, math)).collect();
    let widest = bodies.iter().map(|b| b.0).fold(0.0, f64::max);
    close_pairs(&points, 2.0 * widest * AVOID_REACH, |i, j| {
        let reach = (bodies[i].0 + bodies[j].0) * AVOID_REACH;
        let gap = points[i] - points[j];
        let dist = gap.length();
        if dist >= reach { return; }
        let dir = if dist > 1e-9 { gap * (1.0 / dist) } else { tie_break(i, math) };
        let w = (reach - dist) / reach * reach;
        let (si, sj) = shares(bodies[i].1, bodies[j].1);
        push[i] = push[i] + dir * (w * si);
        push[j] = push[j] + dir * (-w * sj);
    });
    for (i, c) in state.combatants.iter().enumerate() {
        if c.platform { continue; }
        for wreck in &state.wrecks {
            let reach = (bodies[i].0 + wreck.radius) * AVOID_REACH;
            let gap = c.pos - wreck.pos;
            let dist = gap.length();
            if dist >= reach { continue; }
            let dir = if dist > 1e-9 { gap * (1.0 / dist) } else { tie_break(i, math) };
            push[i] = push[i] + dir * (reach - dist);
        }
    }
    push
}

/// A hull never aims into wreckage. Its desired point is moved out of every
/// section's keep-out zone, and when the straight run there would cross a
/// zone it aims for the zone's edge instead, so it goes around. The zone
/// grows with the hull's stopping distance: a ponderous capital at speed
/// starts its turn early. (Without this, a weave around an anchor a wreck now
/// sits on drives the hull through the wreck.)
pub(super) fn clear_of_wreckage(state: &TacticalState, i: usize, want: Vec2) -> Vec2 {
    let math = state.math();
    let c = &state.combatants[i];
    if c.platform || state.wrecks.is_empty() { return want; }
    let r = body_radius(c.kind, false, math);
    let speed = c.vel.length();
    let stopping = speed * speed / (2.0 * ACCEL_CAL / c.max_hp.max(1.0).sqrt());
    let keep = |w: &Wreck| ((r + w.radius) * AVOID_REACH).max(r + w.radius + stopping);
    let mut p = want;
    for _ in 0..3 {
        for w in &state.wrecks {
            let k = keep(w);
            // Aiming inside the zone: aim at its edge, on the near side.
            let gap = p - w.pos;
            let d = gap.length();
            if d < k {
                let away = if d > 1e-9 { gap * (1.0 / d) }
                    else if c.pos.distance(w.pos) > 1e-9 { (c.pos - w.pos).normalized() }
                    else { tie_break(i, math) };
                p = w.pos + away * k;
                continue;
            }
            // A straight run through the zone: aim past its edge instead.
            let run = p - c.pos;
            let len2 = run.length_sq();
            if len2 < 1e-9 || c.pos.distance(w.pos) < k { continue; }
            let t = ((w.pos - c.pos).dot(run) / len2).clamp(0.0, 1.0);
            let closest = c.pos + run * t;
            let miss = closest.distance(w.pos);
            if miss < k {
                let side = if miss > 1e-9 { (closest - w.pos) * (1.0 / miss) } else { Vec2::new(-run.y, run.x).normalized() };
                p = w.pos + side * (k * 1.05);
            }
        }
    }
    p
}

/// After the hulls move: wreckage drifts (and flies apart at its breach),
/// overlapping hulls are pushed apart, and a hull that runs into wreckage
/// takes damage by its closing speed and rebounds.
pub(super) fn after_move(state: &mut TacticalState, out: &mut StepOutcome) {
    let math = state.math();
    let step = state.step;
    for w in &mut state.wrecks {
        if step == w.breach { w.vel = w.vel + w.sep; }
        w.pos = w.pos + w.vel;
        w.vel = w.vel * WRECK_DRAG;
    }
    // Hull against hull: separate overlapping bodies (no damage). A few
    // passes settle a crowd, where pushing one pair apart squeezes another.
    let bodies: Vec<(f64, f64)> = state.combatants.iter().map(|c| body(c, math)).collect();
    let widest = bodies.iter().map(|b| b.0).fold(0.0, f64::max);
    for _ in 0..4 {
        let points: Vec<Vec2> = state.combatants.iter().map(|c| c.pos).collect();
        let mut moves = vec![Vec2::ZERO; points.len()];
        close_pairs(&points, 2.0 * widest, |i, j| {
            let contact = bodies[i].0 + bodies[j].0;
            let gap = points[i] - points[j];
            let dist = gap.length();
            if dist >= contact { return; }
            let dir = if dist > 1e-9 { gap * (1.0 / dist) } else { tie_break(i, math) };
            let (si, sj) = shares(bodies[i].1, bodies[j].1);
            moves[i] = moves[i] + dir * ((contact - dist) * si);
            moves[j] = moves[j] + dir * (-(contact - dist) * sj);
        });
        for (c, m) in state.combatants.iter_mut().zip(moves) {
            if !c.platform { c.pos = c.pos + m; }
        }
    }
    // Hull against wreckage: rams.
    for (i, c) in state.combatants.iter_mut().enumerate() {
        if c.platform || c.hp <= 0.0 { continue; }
        let (r, m) = bodies[i];
        for w in &mut state.wrecks {
            let contact = r + w.radius;
            let gap = c.pos - w.pos;
            let dist = gap.length();
            if dist >= contact { continue; }
            let n = if dist > 1e-9 { gap * (1.0 / dist) } else { tie_break(i, math) };
            let closing = -(c.vel - w.vel).dot(n);
            // A wreck still breaking up shoulders neighbours aside unharmed;
            // after that, running into it is a ram.
            if closing > RAM_MIN_SPEED && step > w.breach {
                let damage = c.max_hp * RAM_MAX_SHARE * (closing / RAM_SPEED).min(1.0).powi(2);
                c.hp -= damage;
                let at = w.pos + n * w.radius;
                out.rams.push(crate::combat::KfRam {
                    cid: c.cid, hull: w.hull, piece: w.piece, x: at.x as f32, y: at.y as f32, damage: damage as f32,
                });
                // An inelastic knock: both lose the closing speed between them.
                let impulse = closing * m * w.mass / (m + w.mass);
                c.vel = c.vel + n * (impulse / m);
                w.vel = w.vel + n * (-impulse / w.mass);
            } else if closing > 0.0 {
                let impulse = closing * m * w.mass / (m + w.mass);
                c.vel = c.vel + n * (impulse / m);
                w.vel = w.vel + n * (-impulse / w.mass);
            }
            let (sc, sw) = shares(m, w.mass);
            c.pos = c.pos + n * ((contact - dist) * sc);
            w.pos = w.pos + n * (-(contact - dist) * sw);
        }
    }
}

/// Where a torpedo flying `from` → `to` this step first runs into wreckage.
pub(super) fn torpedo_blocked(wrecks: &[Wreck], from: Vec2, to: Vec2) -> Option<Vec2> {
    let d = to - from;
    let a = d.length_sq();
    if a < 1e-12 { return None; }
    let mut best: Option<f64> = None;
    for w in wrecks {
        let f = from - w.pos;
        let c = f.length_sq() - w.radius * w.radius;
        if c <= 0.0 { continue; } // already inside: it is flying out
        let b = 2.0 * f.dot(d);
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 { continue; }
        let t = (-b - disc.sqrt()) / (2.0 * a);
        if (0.0..=1.0).contains(&t) && best.is_none_or(|bt| t < bt) { best = Some(t); }
    }
    best.map(|t| from + d * t)
}

/// A large hull that died this step leaves its sections where it was, joined
/// and moving with it; they fly apart a step later (a titan two). Past
/// [`MAX_WRECK_HULLS`] wrecked hulls, the oldest wreck breaks up for good.
pub(super) fn spawn_wreck(state: &mut TacticalState, dead: &Combatant, seed: u64) {
    let pieces = wreck_pieces(dead.kind);
    if dead.platform || pieces == 0 { return; }
    // The hull lay along its course, else toward the enemy, else the bearing.
    let axis = if dead.vel.length() > 1.0 {
        dead.vel.normalized()
    } else {
        state.centroid(1 - dead.side).map(|c| c - dead.pos).filter(|v| v.length() > 1e-6)
            .map(|v| v.normalized()).unwrap_or(state.bearing)
    };
    let side = Vec2::new(-axis.y, axis.x);
    let length = body_radius(dead.kind, false, state.math()) / 0.3;
    let seg = length / f64::from(pieces);
    let mut rng = Rng::new(seed ^ WRECK_SALT ^ u64::from(dead.cid).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let breach = state.step + if dead.kind == ShipKind::Titan { 2 } else { 1 };
    for piece in 0..pieces {
        let along = length / 2.0 - (f64::from(piece) + 0.5) * seg; // bow (+) … stern (−)
        let away = if along.abs() < 1e-9 { rng.next_f64() - 0.5 } else { along.signum() };
        let sep = axis * (away * 0.03 * length * (0.6 + rng.next_f64() * 0.8))
            + side * ((rng.next_f64() - 0.5) * 0.02 * length);
        state.wrecks.push(Wreck {
            hull: dead.cid, piece, side: dead.side, pos: dead.pos + axis * along, vel: dead.vel,
            radius: seg * 0.575, mass: dead.max_hp / f64::from(pieces), breach, sep,
        });
    }
    let mut hulls: Vec<u32> = Vec::new();
    for w in &state.wrecks {
        if !hulls.contains(&w.hull) { hulls.push(w.hull); }
    }
    for gone in hulls.iter().take(hulls.len().saturating_sub(MAX_WRECK_HULLS)) {
        state.wrecks.retain(|w| w.hull != *gone);
    }
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V9 { maneuver_seed }
}

#[allow(dead_code)] // archived rules remain executable for regression comparisons
pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    super::v1::simulate_engagement_with_rules(setup, seed, rules(seed))
}

#[allow(dead_code)]
pub fn project_distribution(setup: &ProjSetup, seed: u64, k: u32) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, simulate_engagement)
}

/// Estimation receives only profiles in the observer's arrived picture (v6).
#[allow(dead_code)]
pub fn project_distribution_with_missions(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        super::v1::simulate_engagement_with_missions(s, seed, rules(seed), missions)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::v1::{SideMods, Torpedo};
    use crate::combat::TorpedoEnd;
    use crate::module::Loadout;

    fn hull(id: u64, kind: ShipKind, fit: &str) -> (EntityId, Ship) {
        (EntityId(id), Ship::new(0, kind, Loadout::from_key(fit)))
    }

    /// Attackers `a` against one stationary defense platform at the origin.
    fn versus_platform(a: &[(EntityId, Ship)], current: bool) -> TacticalState {
        if current {
            TacticalState::open_v9(5, 40, a, &[], 1, 0.0, Vec2::new(1.0, 0.0))
        } else {
            TacticalState::open_v8(5, 40, a, &[], 1, 0.0, Vec2::new(1.0, 0.0))
        }
    }

    fn calm(state: &mut TacticalState) -> StepOutcome {
        for c in &mut state.combatants { c.hp = c.max_hp; }
        state.step(false, [SideMods::default(); 2])
    }

    /// The deepest overlap between any two hulls, as a share of their contact distance.
    fn worst_overlap(state: &TacticalState) -> f64 {
        let mut worst: f64 = 0.0;
        for (i, a) in state.combatants.iter().enumerate() {
            for b in &state.combatants[i + 1..] {
                let contact = body_radius(a.kind, a.platform, Math::Soft) + body_radius(b.kind, b.platform, Math::Soft);
                worst = worst.max(1.0 - a.pos.distance(b.pos) / contact);
            }
        }
        worst
    }

    #[test]
    fn large_hulls_break_like_the_viewer_draws_them() {
        // The battle viewer's WRECK_PIECES (client/src/battlewrecks.ts) for
        // every hull of 8000+ mass; the viewer maps recorded sections onto them.
        let viewer = [(ShipKind::LargeFreighter, 3), (ShipKind::Battleship, 3), (ShipKind::HeavyFreighter, 4),
            (ShipKind::Dreadnought, 4), (ShipKind::BulkFreighter, 4), (ShipKind::Titan, 5)];
        for (kind, n) in viewer { assert_eq!(wreck_pieces(kind), n, "{kind:?}"); }
        for kind in crate::ship::ALL_SHIP_KINDS {
            assert_eq!(wreck_pieces(kind) > 0, hull_mass(kind) >= LARGE_HULL_MASS, "{kind:?}");
        }
    }

    #[test]
    fn hulls_keep_clear_of_each_other() {
        let fleet = [hull(1, ShipKind::Cruiser, ""), hull(2, ShipKind::Cruiser, ""), hull(3, ShipKind::Cruiser, ""),
            hull(4, ShipKind::Battleship, ""), hull(5, ShipKind::Titan, ""), hull(6, ShipKind::Convoy, ""), hull(7, ShipKind::Convoy, "")];
        let (mut v9, mut v8) = (versus_platform(&fleet, true), versus_platform(&fleet, false));
        let (mut worst9, mut worst8) = (0.0f64, 0.0f64);
        for step in 0..160 {
            calm(&mut v9);
            calm(&mut v8);
            if step > 20 {
                worst9 = worst9.max(worst_overlap(&v9));
                worst8 = worst8.max(worst_overlap(&v8));
            }
        }
        assert!(worst8 > 0.5, "v8 hulls stacked ({worst8})");
        assert!(worst9 < 0.1, "v9 hulls never overlap past a brush ({worst9})");
    }

    #[test]
    fn a_large_hull_leaves_wreckage_that_flies_apart_and_is_recorded() {
        let mut state = versus_platform(&[hull(1, ShipKind::Titan, ""), hull(2, ShipKind::Cruiser, "")], true);
        for _ in 0..30 { calm(&mut state); }
        let titan = state.combatants.iter().position(|c| c.kind == ShipKind::Titan).unwrap();
        let cid = state.combatants[titan].cid;
        state.combatants[titan].hp = -1.0;
        let out = state.step(false, [SideMods::default(); 2]);
        let death = out.deaths.iter().find(|d| d.kind == ShipKind::Titan).unwrap();
        assert_eq!(death.cid, Some(cid));
        let joined: Vec<Vec2> = state.wrecks.iter().map(|w| w.pos).collect();
        assert_eq!(joined.len(), 5, "a titan breaks into five sections");
        let seg = body_radius(ShipKind::Titan, false, Math::Soft) / 0.3 / 5.0;
        for pair in joined.windows(2) {
            assert!((pair[0].distance(pair[1]) - seg).abs() < 1e-6, "at first the sections lie end to end");
        }
        let frame = state.step_keyframe(out);
        assert_eq!(frame.wrecks.as_ref().unwrap().len(), 5, "the wreck is in the record");
        assert!(frame.wrecks.as_ref().unwrap().iter().all(|w| w.hull == cid));
        let gap = |s: &TacticalState| s.wrecks[0].pos.distance(s.wrecks[4].pos);
        calm(&mut state);
        assert!((gap(&state) - 4.0 * seg).abs() < 1e-6, "a titan burns a step before it breaks");
        let start = gap(&state);
        for _ in 0..12 { calm(&mut state); }
        assert!(gap(&state) > start + 40.0, "then its sections fly apart ({} → {})", start, gap(&state));
        let before = state.wrecks[0].pos;
        for _ in 0..30 { calm(&mut state); }
        assert!(state.wrecks[0].pos.distance(before) < 25.0, "and drift to a stop");
    }

    #[test]
    fn torpedoes_detonate_on_wreckage_in_their_path() {
        let mut state = versus_platform(&[hull(1, ShipKind::Cruiser, "")], true);
        let platform = state.combatants.iter().find(|c| c.platform).unwrap().cid;
        state.wrecks.push(Wreck { hull: 99, piece: 0, side: 1, pos: Vec2::new(120.0, 0.0), vel: Vec2::ZERO,
            radius: 40.0, mass: 5000.0, breach: 0, sep: Vec2::ZERO });
        state.torpedoes.push(Torpedo { side: 0, pos: Vec2::new(250.0, 0.0), target: platform, dmg: 500.0 });
        let before = state.combatants.iter().find(|c| c.platform).unwrap().hp;
        let out = state.step(false, [SideMods::default(); 2]);
        let end = out.torp_ends.iter().find(|e| e.end == TorpedoEnd::Wreck).expect("it struck the wreck");
        assert!((f64::from(end.x) - 160.0).abs() < 1e-3 && f64::from(end.y).abs() < 1e-3, "on the wreck's near side");
        assert!(state.torpedoes.is_empty());
        assert_eq!(state.combatants.iter().find(|c| c.platform).unwrap().hp, before, "its target is untouched");
    }

    #[test]
    fn ramming_wreckage_hurts_by_closing_speed() {
        // The collision step alone (steering would normally turn a hull away).
        let ram = |speed: f64| {
            let mut state = versus_platform(&[hull(1, ShipKind::Raider, "")], true);
            let r = state.combatants.iter().position(|c| !c.platform).unwrap();
            let contact = body_radius(ShipKind::Raider, false, Math::Soft) + 40.0;
            state.wrecks.push(Wreck { hull: 99, piece: 0, side: 1, pos: Vec2::new(-300.0, 500.0),
                vel: Vec2::ZERO, radius: 40.0, mass: 5000.0, breach: 0, sep: Vec2::ZERO });
            state.combatants[r].pos = Vec2::new(-300.0 - contact + 10.0, 500.0);
            state.combatants[r].vel = Vec2::new(speed, 0.0);
            state.step = 10; // long after the wreck broke up
            let mut out = StepOutcome::default();
            after_move(&mut state, &mut out);
            let c = &state.combatants[r];
            let clear = c.pos.distance(state.wrecks[0].pos) >= contact - 1e-6;
            (out.rams.first().map(|ram| f64::from(ram.damage)), c.max_hp - c.hp, clear, c.vel.x)
        };
        let (hard, lost, clear, rebound) = ram(100.0);
        assert!(hard.is_some_and(|d| d > 50.0) && (hard.unwrap() - lost).abs() < 1e-3, "a fast ram hurts: {hard:?}");
        assert!(clear, "the hull is pushed clear of the wreck");
        assert!(rebound.abs() < 5.0, "and loses its closing speed ({rebound})");
        let (soft, ..) = ram(30.0);
        assert!(soft.is_some_and(|d| d < hard.unwrap() * 0.2), "a slower ram hurts far less: {soft:?}");
        let (scrape, ..) = ram(2.0);
        assert_eq!(scrape, None, "a scrape is not an impact");
    }

    #[test]
    fn a_fresh_breakup_shoulders_neighbours_aside_unharmed() {
        let mut state = versus_platform(&[hull(1, ShipKind::Cruiser, "")], true);
        let r = state.combatants.iter().position(|c| !c.platform).unwrap();
        state.step = 10;
        state.wrecks.push(Wreck { hull: 99, piece: 0, side: 1, pos: Vec2::new(-300.0, 500.0),
            vel: Vec2::new(-20.0, 0.0), radius: 40.0, mass: 5000.0, breach: 10, sep: Vec2::ZERO });
        state.combatants[r].pos = Vec2::new(-300.0 - 80.0, 500.0);
        state.combatants[r].vel = Vec2::ZERO;
        let mut out = StepOutcome::default();
        after_move(&mut state, &mut out);
        assert!(out.rams.is_empty(), "no ram while the wreck is still breaking up");
        let c = &state.combatants[r];
        assert_eq!(c.hp, c.max_hp);
        assert!(c.pos.distance(state.wrecks[0].pos) >= body_radius(ShipKind::Cruiser, false, Math::Soft) + 40.0 - 1e-6, "but it is pushed aside");
    }

    #[test]
    fn hulls_route_around_wreckage_instead_of_through_it() {
        let mut state = versus_platform(&[hull(1, ShipKind::Battleship, "")], true);
        let i = state.combatants.iter().position(|c| !c.platform).unwrap();
        state.combatants[i].pos = Vec2::new(-600.0, 0.0);
        state.combatants[i].vel = Vec2::new(30.0, 0.0);
        state.wrecks.push(Wreck { hull: 99, piece: 0, side: 1, pos: Vec2::new(-300.0, 0.0), vel: Vec2::ZERO,
            radius: 40.0, mass: 5000.0, breach: 0, sep: Vec2::ZERO });
        let aim = clear_of_wreckage(&state, i, Vec2::new(0.0, 0.0)); // straight through the wreck
        let run = aim - state.combatants[i].pos;
        let t = ((Vec2::new(-300.0, 0.0) - state.combatants[i].pos).dot(run) / run.length_sq()).clamp(0.0, 1.0);
        let miss = (state.combatants[i].pos + run * t).distance(Vec2::new(-300.0, 0.0));
        assert!(miss >= body_radius(ShipKind::Battleship, false, Math::Soft) + 40.0, "the run passes clear of the wreck ({miss})");
        let inside = clear_of_wreckage(&state, i, Vec2::new(-290.0, 5.0));
        assert!(inside.distance(Vec2::new(-300.0, 0.0)) > body_radius(ShipKind::Battleship, false, Math::Soft) + 40.0, "and never aims into it");
    }

    #[test]
    fn big_hulls_come_about_slowly_and_light_ones_freely() {
        assert_eq!(turn_rate(ShipKind::Raider), None);
        assert_eq!(turn_rate(ShipKind::Corvette), None);
        assert_eq!(turn_rate(ShipKind::Destroyer), None);
        let ladder = [ShipKind::Cruiser, ShipKind::Battleship, ShipKind::Dreadnought, ShipKind::Titan].map(|k| turn_rate(k).unwrap());
        assert!(ladder.windows(2).all(|w| w[0] > w[1]), "the bigger, the slower");
        // Asked to reverse, a titan swings 0.085 rad and keeps its asked speed.
        let v = turn_capped(ShipKind::Titan, Vec2::new(20.0, 0.0), Vec2::new(-5.0, 0.0), Math::Soft);
        assert!((v.y.atan2(v.x).abs() - 0.085).abs() < 1e-9 && (v.length() - 5.0).abs() < 1e-9);
        assert_eq!(turn_capped(ShipKind::Raider, Vec2::new(20.0, 0.0), Vec2::new(-5.0, 0.0), Math::Soft), Vec2::new(-5.0, 0.0));
        // In battle: a titan's course never swings faster than its rate.
        let mut state = versus_platform(&[hull(1, ShipKind::Titan, ""), hull(2, ShipKind::Destroyer, "")], true);
        let mut last: Option<Vec2> = None;
        for _ in 0..160 {
            calm(&mut state);
            let titan = state.combatants.iter().find(|c| c.kind == ShipKind::Titan).unwrap();
            if let Some(prev) = last && prev.length() > 1e-6 && titan.vel.length() > 1e-6 {
                let mut d = titan.vel.y.atan2(titan.vel.x) - prev.y.atan2(prev.x);
                while d > std::f64::consts::PI { d -= std::f64::consts::TAU; }
                while d <= -std::f64::consts::PI { d += std::f64::consts::TAU; }
                assert!(d.abs() <= 0.085 + 1e-9, "a titan swings {d} rad in a step");
            }
            last = Some(titan.vel);
        }
    }

    #[test]
    fn wreckage_is_capped() {
        let mut state = versus_platform(&[hull(1, ShipKind::Cruiser, "")], true);
        for k in 0..10u32 {
            let dead = Combatant { cid: 500 + k, kind: ShipKind::Battleship, max_hp: 8000.0, hp: 0.0,
                pos: Vec2::new(-600.0 + 130.0 * f64::from(k), 300.0), vel: Vec2::new(10.0, 0.0),
                ..state.combatants[0].clone() };
            spawn_wreck(&mut state, &dead, 7);
        }
        let mut hulls: Vec<u32> = state.wrecks.iter().map(|w| w.hull).collect();
        hulls.dedup();
        assert_eq!(hulls, (502..510).collect::<Vec<_>>(), "the eight newest wrecks remain");
        assert_eq!(state.wrecks.len(), 8 * 3);
    }

    #[test]
    fn v9_battles_open_on_v9_and_saved_v8_battles_keep_their_rules() {
        let a = [hull(1, ShipKind::Cruiser, ""), hull(2, ShipKind::Battleship, "")];
        let current = versus_platform(&a, true);
        assert_eq!(current.rules_version(), 9);
        let old = versus_platform(&a, false);
        let mut saved: TacticalState = serde_json::from_slice(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(saved.rules_version(), 8);
        let mut live = old.clone();
        for _ in 0..60 {
            live.step(false, [SideMods::default(); 2]);
            saved.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(saved, live, "a saved v8 fight replays exactly");
        let mut wrecked = current.clone();
        for _ in 0..20 { calm(&mut wrecked); }
        let b = wrecked.combatants.iter().position(|c| c.kind == ShipKind::Battleship).unwrap();
        wrecked.combatants[b].hp = -1.0;
        for _ in 0..3 { wrecked.step(false, [SideMods::default(); 2]); }
        assert_eq!(wrecked.wrecks.len(), 3);
        let restored: TacticalState = serde_json::from_slice(&serde_json::to_vec(&wrecked).unwrap()).unwrap();
        assert_eq!(restored, wrecked, "wreckage is saved with the battle");
        let (mut x, mut y) = (wrecked.clone(), restored);
        for _ in 0..30 {
            x.step(false, [SideMods::default(); 2]);
            y.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(x, y, "and replays exactly");
    }

    #[test]
    fn v9_estimates_run_deterministically() {
        let setup = ProjSetup {
            a: (0..6).map(|i| hull(10 + i, ShipKind::Raider, "")).collect(),
            d: vec![hull(30, ShipKind::Battleship, "")],
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None,
        };
        let first = project_distribution(&setup, 17, 8);
        let again = project_distribution(&setup, 17, 8);
        assert_eq!(first.a_win_pct, again.a_win_pct);
        assert_eq!(first.median.steps, again.median.steps);
        assert_eq!(simulate_engagement(&setup, 3).steps, simulate_engagement(&setup, 3).steps);
    }
}

/// Manual balance comparison against the frozen v8 rules (prints, asserts
/// nothing): `cargo test -p sim --lib balance_probe_v9 -- --ignored --nocapture`.
#[cfg(test)]
mod balance_probe_v9 {
    use super::*;
    use crate::module::Loadout;

    fn side(base: u64, list: &[(ShipKind, &str, u64)]) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        let mut id = base;
        for (kind, fit, n) in list {
            for _ in 0..*n { out.push((EntityId(id), Ship::new(0, *kind, Loadout::from_key(fit)))); id += 1; }
        }
        out
    }

    #[test]
    #[ignore]
    fn print_v8_vs_v9_balance() {
        use ShipKind as K;
        let cases: Vec<(&str, Vec<(K, &str, u64)>, Vec<(K, &str, u64)>, bool)> = vec![
            ("6 raider v 4 corvette", vec![(K::Raider, "", 6)], vec![(K::Corvette, "", 4)], false),
            ("10 raider v 1 battleship", vec![(K::Raider, "", 10)], vec![(K::Battleship, "", 1)], false),
            ("20 raider v 1 titan", vec![(K::Raider, "", 20)], vec![(K::Titan, "", 1)], false),
            ("4 corvette v 2 destroyer", vec![(K::Corvette, "", 4)], vec![(K::Destroyer, "", 2)], false),
            ("2 cruiser v 1 battleship", vec![(K::Cruiser, "", 2)], vec![(K::Battleship, "", 1)], false),
            ("5 cruiser mirror", vec![(K::Cruiser, "", 5)], vec![(K::Cruiser, "", 5)], false),
            ("10 raider mirror", vec![(K::Raider, "", 10)], vec![(K::Raider, "", 10)], false),
            ("3 raider v 5 corvette RAID", vec![(K::Raider, "", 3)], vec![(K::Corvette, "", 5)], true),
            ("1 raider v convoy RAID", vec![(K::Raider, "", 1)], vec![(K::Convoy, "", 1)], true),
            ("6 torp raider v 3 PD corvette+convoy", vec![(K::Raider, "torpedo_rack", 6)], vec![(K::Corvette, "point_defense_screen", 3), (K::Convoy, "", 1)], false),
            ("6 torp raider+4 torp destroyer v 2 cruiser+dread", vec![(K::Raider, "torpedo_rack", 6), (K::Destroyer, "torpedo_rack", 4)], vec![(K::Cruiser, "", 2), (K::Dreadnought, "", 1)], false),
            ("2 dread v 12 raider+6 corvette", vec![(K::Dreadnought, "", 2)], vec![(K::Raider, "", 12), (K::Corvette, "", 6)], false),
            ("titan+4 corv v 3 battleship", vec![(K::Titan, "", 1), (K::Corvette, "", 4)], vec![(K::Battleship, "", 3)], false),
            ("3 battleship v 3 battleship", vec![(K::Battleship, "", 3)], vec![(K::Battleship, "", 3)], false),
            ("8 driver raider v 2 cruiser", vec![(K::Raider, "mass_driver", 8)], vec![(K::Cruiser, "", 2)], false),
            ("4 cruiser v 8 corvette", vec![(K::Cruiser, "", 4)], vec![(K::Corvette, "", 8)], false),
        ];
        for (name, a, d, raid) in cases {
            let setup = ProjSetup { a: side(100, &a), d: side(500, &d), platform_tiers: 0, raid, a_retreat: None, d_retreat: None };
            let old = super::super::v8::project_distribution(&setup, 7, 32);
            let new = project_distribution(&setup, 7, 32);
            let loss = |m: &BTreeMap<ShipKind, u32>| m.values().sum::<u32>();
            eprintln!("{name:48} v8 win {:5.1}% steps {:3} lossA {:2} lossD {:2} | v9 win {:5.1}% steps {:3} lossA {:2} lossD {:2}",
                old.a_win_pct * 100.0, old.median.steps, loss(&old.median.a_losses), loss(&old.median.d_losses),
                new.a_win_pct * 100.0, new.median.steps, loss(&new.median.a_losses), loss(&new.median.d_losses));
        }
    }
}

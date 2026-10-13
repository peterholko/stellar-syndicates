//! Formations. A fleet holds a shape in battle, set in its mission profile: a
//! battle line, an escort ring or a wedge (Automatic picks one from the
//! fleet's makeup when it forms up; Free holds none). The shape faces the
//! enemy and wheels at its slowest hull's pace, closes to a range its ships can
//! fire from, then weaves as one body at half its slowest hull's speed, on a
//! circle that hull can turn. Each hull steers to its slot. Interceptors keep
//! skirmishing (their speed is their armour), and anything withdrawing,
//! datalinked to guard a charge, or screening transports that are present
//! keeps its own station. Everything else (bodies, wreckage, slow turning,
//! weaving for hulls outside a formation) is v9. New battles open on v12
//! (`open_current`), which keeps all of this but holds a battle line no
//! wider than its guns reach.

use super::rules_v1::{attack_weight, hull_mass, max_speed, Rng};
use super::v1::{
    Distribution, ProjSetup, Role, SimOutcome, TacticalState, ACCEL_CAL, ARENA_RADIUS, ARENA_SCALE, BEAM_RANGE,
    PD_RADIUS, PROJECTION_BATTLE_ID,
};
use super::{v3, v4, v9, Rules};
use crate::doctrine::{Formation, MissionProfile, ScreeningRole};
use crate::{ship::Ship, EntityId, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The shapes a fleet can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    /// Side by side across the line of fire: every gun bears.
    Line,
    /// Capitals and civilians in the middle, escorts in a ring at point-defense range.
    Ring,
    /// The heaviest hull at the point, the rest trailing back on both flanks.
    Wedge,
}

/// A fleet's formation in battle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormationFrame {
    /// The shape it holds now (Automatic re-reads the fight every step).
    pub auto: Shape,
    pub centre: Vec2,
    pub vel: Vec2,
    /// Which way it faces (radians; the shape's front points this way).
    pub facing: f64,
    pub phase: f64,
    pub sign: f64,
}

/// Neighbours in a formation keep this multiple of their contact distance
/// apart: clear of v9's steering-away reach, so they hold station calmly.
pub(super) const SLOT_SPACING: f64 = 1.8;
/// A formation wheels at this share of its slowest hull's turn rate.
const WHEEL: f64 = 0.7;
/// How fast a formation of free-turning hulls (destroyers and lighter) wheels.
const FREE_TURN: f64 = 0.6;
/// The formation weaves at this share of its slowest hull's top speed.
const WEAVE_SPEED: f64 = 0.5;
/// An independent stream: formations never spend a targeting or damage roll.
const FORMATION_SALT: u64 = 0x464f_524d_4154_494f;

impl TacticalState {
    /// Frozen v10 constructor (new battles open on the current rules: `open_current`).
    pub(crate) fn open_v10(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v9(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V9 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V10 { maneuver_seed };
        state
    }
}

fn profile(state: &TacticalState, fleet: EntityId) -> MissionProfile {
    state.missions.get(&fleet).copied().unwrap_or_default()
}

/// The shape Automatic picks, from the fleet and the enemy in front of it
/// (re-read every step, so it adapts as the fight changes). Against torpedoes
/// a fleet with point defense forms a crescent screen and one without keeps
/// its own stations (its screens and line hulls do better free); civilians
/// get a ring; otherwise a battle line, so every gun bears. A wedge is a
/// deliberate choice. None: no formation.
pub(super) fn auto_shape(state: &TacticalState, members: &[usize]) -> Option<Shape> {
    let side = state.combatants[members[0]].side;
    let torpedoes = state.combatants.iter().any(|c| c.side != side && c.hp > 0.0 && !c.platform
        && super::v5::offense(&crate::module::Loadout::from_key(&c.stack)).0 == crate::module::DamageType::Torpedo);
    let pd = members.iter().any(|&i| crate::module::Loadout::from_key(&state.combatants[i].stack).has_pd());
    let civilians = members.iter().any(|&i| attack_weight(state.combatants[i].kind) <= 0.0);
    match (torpedoes, pd, civilians) {
        (true, true, _) => Some(Shape::Ring),
        (true, false, _) => None,
        (false, _, true) => Some(Shape::Ring),
        (false, _, false) => Some(Shape::Line),
    }
}

/// Each member's slot (frame coordinates: x toward the enemy, y to the left),
/// for members sorted heaviest first.
pub(super) fn offsets(state: &TacticalState, shape: Shape, members: &[usize]) -> Vec<Vec2> {
    let math = state.math();
    let r = |i: usize| v9::body_radius(state.combatants[i].kind, false, math);
    let line = |list: &[usize]| -> Vec<Vec2> {
        // Heaviest in the middle, then alternating out to the left and right.
        let mut out = vec![Vec2::ZERO; list.len()];
        let (mut left, mut right) = ((0.0, r(list[0])), (0.0, r(list[0])));
        for (k, &i) in list.iter().enumerate().skip(1) {
            let side = if k % 2 == 1 { &mut left } else { &mut right };
            side.0 += (side.1 + r(i)) * SLOT_SPACING;
            side.1 = r(i);
            out[k] = Vec2::new(0.0, if k % 2 == 1 { side.0 } else { -side.0 });
        }
        out
    };
    match shape {
        Shape::Line => line(members),
        Shape::Wedge => {
            let mut out = vec![Vec2::ZERO; members.len()];
            let mut ends = [(Vec2::ZERO, r(members[0])); 2];
            for (k, &i) in members.iter().enumerate().skip(1) {
                let (side, sign) = if k % 2 == 1 { (0, 1.0) } else { (1, -1.0) };
                let (last, lr) = ends[side];
                let step = (lr + r(i)) * SLOT_SPACING / std::f64::consts::SQRT_2;
                let at = last + Vec2::new(-step, sign * step);
                out[k] = at;
                ends[side] = (at, r(i));
            }
            out
        }
        Shape::Ring => {
            let core: Vec<usize> = members.iter().copied()
                .filter(|&i| { let k = state.combatants[i].kind; attack_weight(k) <= 0.0 || hull_mass(k) >= v9::LARGE_HULL_MASS })
                .collect();
            let core = if core.is_empty() { vec![members[0]] } else { core };
            // Point-defense escorts take the middle of the arc facing the
            // enemy; the rest fill out its flanks.
            let pd = |i: &usize| crate::module::Loadout::from_key(&state.combatants[*i].stack).has_pd();
            let mut escorts: Vec<usize> = members.iter().copied().filter(|i| !core.contains(i)).collect();
            escorts.sort_by_key(|i| !pd(i));
            // The core sits side by side, centred in the ring.
            let mut inner = line(&core);
            let mid = inner.iter().map(|o| o.y).sum::<f64>() / inner.len() as f64;
            for o in &mut inner { o.y -= mid; }
            let reach = core.iter().zip(&inner).map(|(&i, o)| o.length() + r(i)).fold(0.0, f64::max);
            let widest = escorts.iter().map(|&i| r(i)).fold(0.0, f64::max);
            let around = escorts.len() as f64 * widest * SLOT_SPACING / std::f64::consts::PI;
            let ring = (reach + widest * SLOT_SPACING).max(PD_RADIUS * 0.75).max(around);
            let mut out = vec![Vec2::ZERO; members.len()];
            // A crescent across the front, widening toward a full ring only
            // when it runs out of room.
            let arc = (escorts.len() as f64 * widest * SLOT_SPACING / ring).clamp(std::f64::consts::FRAC_PI_3, std::f64::consts::TAU * 0.999);
            for (k, &i) in members.iter().enumerate() {
                if let Some(c) = core.iter().position(|&j| j == i) {
                    out[k] = inner[c];
                } else if let Some(e) = escorts.iter().position(|&j| j == i) {
                    // Centre outward: 0, +1, −1, +2, −2 … steps across the arc.
                    let step = if escorts.len() > 1 { arc / (escorts.len() - 1) as f64 } else { 0.0 };
                    let rank = (e as f64 / 2.0).ceil() * if e % 2 == 1 { 1.0 } else { -1.0 };
                    let a = rank * step;
                    out[k] = match math {
                        super::Math::Platform => Vec2::new(a.cos() * ring, a.sin() * ring),
                        super::Math::Soft => crate::dmath::polar(a, ring),
                    };
                }
            }
            out
        }
    }
}

/// How far from its target the formation's centre holds: close enough for
/// its ships to fire. A line holds at its shortest reach, a wedge at its
/// point's, and a ring far enough back that its front escorts are in range
/// while its core stays back. None: nothing armed, so it holds its anchor.
fn hold_distance(state: &TacticalState, shape: Shape, members: &[usize], ring: f64) -> Option<f64> {
    let band = |i: usize| state.preferred_band(&state.combatants[i]);
    let armed = |i: &usize| attack_weight(state.combatants[*i].kind) > 0.0;
    let shortest = members.iter().copied().filter(armed).map(band).reduce(f64::min);
    match shape {
        Shape::Line => shortest.map(|b| b * 0.95),
        Shape::Wedge => members.iter().copied().take(1).filter(armed).map(band).next().or(shortest).map(|b| b * 0.95),
        Shape::Ring => shortest.map(|b| b * 0.95 + ring * 0.6),
    }
}

/// What a formation faces and whether it closes on it: the enemy's main body
/// (its line, capitals, screens and platforms, not its skirmishers). If the
/// enemy has nothing but skirmishers, the formation faces the nearest of them
/// but holds its ground: they come to it, and chasing them only drags its
/// slowest hulls and whatever it guards into the open.
fn target_point(state: &TacticalState, side: u8, centre: Vec2) -> (Vec2, bool) {
    let foes = || state.combatants.iter().filter(move |c| c.side != side && c.hp > 0.0);
    let main: Vec<Vec2> = foes().filter(|c| !matches!(c.role, Role::Skirmish | Role::Withdraw)).map(|c| c.pos).collect();
    if !main.is_empty() {
        return (main.iter().fold(Vec2::ZERO, |a, p| a + *p) * (1.0 / main.len() as f64), true);
    }
    let nearest = foes().map(|c| c.pos).min_by(|a, b| a.distance(centre).total_cmp(&b.distance(centre)).then(a.x.total_cmp(&b.x)))
        .unwrap_or(centre + state.bearing * if side == 0 { -1.0 } else { 1.0 });
    (nearest, false)
}

/// Before the hulls steer: group each fleet's formation hulls, (re)form and
/// move its frame, and return each formation hull's slot point (None for a
/// hull outside any formation).
pub(super) fn update_formations(state: &mut TacticalState, seed: u64) -> Vec<Option<Vec2>> {
    let n = state.combatants.len();
    let mut slots = vec![None; n];
    let civilians = [0u8, 1].map(|side| state.combatants.iter()
        .any(|c| c.side == side && c.hp > 0.0 && !c.platform && attack_weight(c.kind) <= 0.0));
    let mut fleets: BTreeMap<EntityId, Vec<usize>> = BTreeMap::new();
    for (i, c) in state.combatants.iter().enumerate() {
        if c.platform || c.hp <= 0.0 || state.withdrawing[usize::from(c.side.min(1))] { continue; }
        if !matches!(c.role, Role::Anchor | Role::Line | Role::Screen) { continue; }
        let Some((fleet, _)) = c.origin else { continue };
        let p = profile(state, fleet);
        if p.formation == Formation::Free { continue; }
        if p.screening == ScreeningRole::ProtectTransports && attack_weight(c.kind) > 0.0
            && civilians[usize::from(c.side.min(1))] { continue; }
        if v4::guarding(state, c) { continue; }
        fleets.entry(fleet).or_default().push(i);
    }
    fleets.retain(|_, members| members.len() >= 2);
    state.formations.retain(|fleet, _| fleets.contains_key(fleet));
    for (fleet, mut members) in fleets {
        members.sort_by(|&a, &b| {
            let (ca, cb) = (&state.combatants[a], &state.combatants[b]);
            hull_mass(cb.kind).total_cmp(&hull_mass(ca.kind)).then(ca.cid.cmp(&cb.cid))
        });
        let side = state.combatants[members[0]].side;
        let enemy = |state: &TacticalState, from: Vec2| target_point(state, side, from).0;
        let shape = match profile(state, fleet).formation {
            Formation::BattleLine => Shape::Line,
            Formation::EscortRing => Shape::Ring,
            Formation::Wedge => Shape::Wedge,
            Formation::Automatic | Formation::Free => match auto_shape(state, &members) {
                Some(shape) => shape,
                None => { state.formations.remove(&fleet); continue; }
            },
        };
        // v12: a battle line holds no wider than its guns reach; the rest
        // fight on their own.
        if shape == Shape::Line && matches!(state.rules, Rules::V12 { .. } | Rules::V13 { .. } | Rules::V14 { .. } | Rules::V15 { .. } | Rules::V16 { .. }) {
            members = super::v12::line_members(state, &members);
            if members.len() < 2 { state.formations.remove(&fleet); continue; }
        }
        if !state.formations.contains_key(&fleet) {
            let centre = members.iter().fold(Vec2::ZERO, |a, &i| a + state.combatants[i].pos) * (1.0 / members.len() as f64);
            let to = enemy(state, centre) - centre;
            let mut rng = Rng::new(seed ^ FORMATION_SALT ^ fleet.0.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let frame = FormationFrame {
                auto: shape, centre, vel: Vec2::ZERO, facing: state.math().atan2(to.y, to.x),
                phase: rng.next_f64() * std::f64::consts::TAU, sign: if rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 },
            };
            state.formations.insert(fleet, frame);
        }
        let frame = state.formations[&fleet].clone();
        let offs = offsets(state, shape, &members);
        // The slowest hull sets the pace: speed, acceleration, turning.
        let speed = members.iter().map(|&i| max_speed(state.combatants[i].kind) * ARENA_SCALE).fold(f64::INFINITY, f64::min);
        let accel = members.iter().map(|&i| ACCEL_CAL / state.combatants[i].max_hp.max(1.0).sqrt()).fold(f64::INFINITY, f64::min);
        let turn = members.iter().map(|&i| v9::turn_rate(state.combatants[i].kind).unwrap_or(FREE_TURN)).fold(f64::INFINITY, f64::min);
        let extent = offs.iter().map(|o| o.length()).fold(1.0, f64::max);
        // Wheel no faster than the slowest hull turns, nor than the far end
        // of the shape can keep up.
        let wheel = (WHEEL * turn).min(WHEEL * speed / extent);
        let (target_enemy, closes) = target_point(state, side, frame.centre);
        // A ring turns its screen toward the torpedo boats, wherever they are.
        let faces = if shape == Shape::Ring { state.torpedo_threat_centroid(1 - side).unwrap_or(target_enemy) } else { target_enemy };
        let math = state.math();
        let want = math.atan2((faces - frame.centre).y, (faces - frame.centre).x);
        let mut turn_by = want - frame.facing;
        while turn_by > std::f64::consts::PI { turn_by -= std::f64::consts::TAU; }
        while turn_by <= -std::f64::consts::PI { turn_by += std::f64::consts::TAU; }
        let facing = frame.facing + turn_by.clamp(-wheel, wheel);
        let ahead = match math {
            super::Math::Platform => Vec2::new(facing.cos(), facing.sin()),
            super::Math::Soft => crate::dmath::polar(facing, 1.0),
        };
        let ring = if shape == Shape::Ring { extent } else { 0.0 };
        let anchor = if side == 0 { state.bearing * (BEAM_RANGE * 0.85) } else { Vec2::ZERO };
        let hold = match hold_distance(state, shape, &members, ring) {
            Some(d) if closes => target_enemy - ahead * d,
            _ => anchor,
        };
        // The whole shape weaves on a circle its slowest hull can follow.
        let weave = WEAVE_SPEED * speed;
        let rate = (0.6 * accel / weave.max(1e-9)).min(0.8 * turn);
        let theta = frame.phase + frame.sign * rate * state.step as f64;
        let circle = match math {
            super::Math::Platform => Vec2::new(theta.cos(), theta.sin()),
            super::Math::Soft => crate::dmath::polar(theta, 1.0),
        };
        let target = hold + circle * (weave / rate.max(1e-9));
        // Move at the slowest hull's speed, and wait for the straggler: a
        // slow-turning capital chasing a slot that keeps moving at full speed
        // swings too wide to ever take it.
        let (fs, fc) = match math {
            super::Math::Platform => (facing.sin(), facing.cos()),
            super::Math::Soft => crate::dmath::sin_cos(facing),
        };
        let slot = |o: Vec2, centre: Vec2| centre + Vec2::new(o.x * fc - o.y * fs, o.x * fs + o.y * fc);
        let lag = members.iter().zip(&offs).map(|(&i, &o)| state.combatants[i].pos.distance(slot(o, frame.centre)))
            .fold(0.0, f64::max);
        let cap = speed * (1.2 - lag / 120.0).clamp(0.2, 1.0);
        let delta = target - frame.centre;
        let d = delta.length();
        let step = if d > 1e-9 { delta * ((d / 1.5).min(cap) / d) } else { Vec2::ZERO };
        let mut centre = frame.centre + step;
        let cap_r = ARENA_RADIUS * 0.9;
        if centre.length() > cap_r { centre = centre * (cap_r / centre.length()); }
        let vel = centre - frame.centre;
        for (&i, &o) in members.iter().zip(&offs) {
            // Lead the slot by the formation's motion so a hull on station keeps pace.
            slots[i] = Some(slot(o, centre) + vel * 1.5);
        }
        let f = state.formations.get_mut(&fleet).unwrap();
        f.auto = shape;
        f.centre = centre;
        f.vel = vel;
        f.facing = facing;
    }
    slots
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V10 { maneuver_seed }
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
    use super::super::v1::SideMods;
    use crate::doctrine::Formation as F;
    use crate::module::Loadout;
    use crate::ShipKind as K;

    /// One fleet (`id`) of these hulls.
    fn fleet(id: u64, list: &[(K, &str, u32)]) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        for (kind, fit, n) in list {
            for _ in 0..*n { out.push((EntityId(id), Ship::new(out.len() as u32, *kind, Loadout::from_key(fit)))); }
        }
        out
    }

    /// Fleet 1 against one stationary defense platform at the origin, under
    /// `formation`, run for `steps` with every hull kept alive.
    fn formed(list: &[(K, &str, u32)], formation: F, steps: usize) -> TacticalState {
        let mut state = TacticalState::open_v10(5, 40, &fleet(1, list), &[], 1, 0.0, Vec2::new(1.0, 0.0));
        state.set_missions(BTreeMap::from([(EntityId(1), MissionProfile { formation, ..Default::default() })]));
        for _ in 0..steps {
            for c in &mut state.combatants { c.hp = c.max_hp; }
            state.step(false, [SideMods::default(); 2]);
        }
        state
    }

    /// Each fleet-1 hull's (forward, left) position in its formation's frame.
    fn in_frame(state: &TacticalState) -> Vec<(K, f64, f64)> {
        let f = &state.formations[&EntityId(1)];
        let (c, s) = (f.facing.cos(), f.facing.sin());
        state.combatants.iter().filter(|h| !h.platform && h.side == 0).map(|h| {
            let d = h.pos - f.centre;
            (h.kind, d.x * c + d.y * s, -d.x * s + d.y * c)
        }).collect()
    }

    fn spread(v: impl Iterator<Item = f64> + Clone) -> f64 {
        v.clone().fold(f64::NEG_INFINITY, f64::max) - v.fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn a_battle_line_forms_across_the_line_of_fire() {
        let state = formed(&[(K::Battleship, "", 1), (K::Cruiser, "", 4)], F::BattleLine, 160);
        let hulls = in_frame(&state);
        let across = spread(hulls.iter().map(|h| h.2));
        let along = spread(hulls.iter().map(|h| h.1));
        assert!(across > 4.0 * along.max(1.0), "side by side: {across} across, {along} deep");
        let f = &state.formations[&EntityId(1)];
        let to_enemy = Vec2::ZERO - f.centre;
        let off = (to_enemy.y.atan2(to_enemy.x) - f.facing).sin().abs();
        assert!(off < 0.3, "facing the enemy ({off})");
        for (i, a) in state.combatants.iter().enumerate() {
            for b in &state.combatants[i + 1..] {
                assert!(a.pos.distance(b.pos) >= v9::body_radius(a.kind, a.platform, crate::tactical::Math::Soft) + v9::body_radius(b.kind, b.platform, crate::tactical::Math::Soft) - 1e-6);
            }
        }
    }

    #[test]
    fn a_wedge_puts_its_heaviest_hull_at_the_point() {
        let state = formed(&[(K::Battleship, "", 1), (K::Corvette, "", 4)], F::Wedge, 160);
        let hulls = in_frame(&state);
        let point = hulls.iter().find(|h| h.0 == K::Battleship).unwrap();
        assert!(hulls.iter().filter(|h| h.0 != K::Battleship).all(|h| h.1 < point.1 - 20.0), "the escorts trail the battleship");
        assert!(hulls.iter().any(|h| h.2 > 20.0) && hulls.iter().any(|h| h.2 < -20.0), "on both flanks");
    }

    #[test]
    fn an_escort_ring_keeps_its_escorts_around_the_core() {
        // A convoy makes Automatic choose the ring.
        let state = formed(&[(K::Convoy, "", 1), (K::Battleship, "", 1), (K::Corvette, "point_defense_screen", 4)], F::Automatic, 160);
        assert_eq!(state.formations[&EntityId(1)].auto, Shape::Ring);
        let f = &state.formations[&EntityId(1)];
        let ours = || state.combatants.iter().filter(|h| !h.platform);
        let core: Vec<f64> = ours().filter(|h| matches!(h.kind, K::Convoy | K::Battleship)).map(|h| h.pos.distance(f.centre)).collect();
        let ring: Vec<f64> = ours().filter(|h| h.kind == K::Corvette).map(|h| h.pos.distance(f.centre)).collect();
        let mean = ring.iter().sum::<f64>() / ring.len() as f64;
        assert!(ring.iter().all(|d| (d - mean).abs() < mean * 0.3), "escorts evenly around: {ring:?}");
        assert!(core.iter().all(|d| *d < mean * 0.7), "the core inside: {core:?} vs ring {mean}");
        assert!(mean < PD_RADIUS * 1.6, "close enough that their bubbles overlap the core ({mean})");
    }

    #[test]
    fn free_is_v9_exactly() {
        let list = [(K::Battleship, "", 1), (K::Cruiser, "", 3), (K::Raider, "", 4)];
        let missions = BTreeMap::from([(EntityId(1), MissionProfile { formation: F::Free, ..Default::default() })]);
        let mut v10 = TacticalState::open_v10(9, 41, &fleet(1, &list), &fleet(2, &[(K::Cruiser, "", 3)]), 0, 0.0, Vec2::new(1.0, 0.0));
        let mut v9 = TacticalState::open_v9(9, 41, &fleet(1, &list), &fleet(2, &[(K::Cruiser, "", 3)]), 0, 0.0, Vec2::new(1.0, 0.0));
        let both = BTreeMap::from([(EntityId(1), missions[&EntityId(1)]), (EntityId(2), missions[&EntityId(1)])]);
        v10.set_missions(both.clone());
        v9.set_missions(both);
        for _ in 0..90 {
            v10.step(false, [SideMods::default(); 2]);
            v9.step(false, [SideMods::default(); 2]);
            assert_eq!(v10.combatants, v9.combatants, "Free holds no formation");
        }
        assert!(v10.formations.is_empty());
    }

    #[test]
    fn interceptors_keep_skirmishing_and_withdrawing_ships_leave() {
        let mut state = formed(&[(K::Cruiser, "", 3), (K::Raider, "", 4)], F::BattleLine, 10);
        let slots = update_formations(&mut state.clone(), 7);
        for (i, c) in state.combatants.iter().enumerate() {
            if c.platform { continue; }
            assert_eq!(slots[i].is_some(), c.kind == K::Cruiser, "{:?} in the line: {}", c.kind, slots[i].is_some());
        }
        state.order_withdraw(0);
        state.step(false, [SideMods::default(); 2]);
        assert!(state.formations.is_empty(), "a withdrawing side breaks formation");
    }

    #[test]
    fn the_shape_moves_at_its_slowest_hulls_pace() {
        let mut state = TacticalState::open_v10(5, 40, &fleet(1, &[(K::Titan, "", 1), (K::Corvette, "", 4)]), &[], 1, 0.0, Vec2::new(1.0, 0.0));
        state.set_missions(BTreeMap::from([(EntityId(1), MissionProfile { formation: F::BattleLine, ..Default::default() })]));
        let mut last: Option<Vec2> = None;
        for _ in 0..80 {
            for c in &mut state.combatants { c.hp = c.max_hp; }
            state.step(false, [SideMods::default(); 2]);
            let centre = state.formations[&EntityId(1)].centre;
            if let Some(prev) = last { assert!(centre.distance(prev) <= K::Titan.max_speed() + 1e-9, "never outruns its titan"); }
            last = Some(centre);
        }
        let corvettes: Vec<f64> = state.combatants.iter().filter(|c| c.kind == K::Corvette).map(|c| c.vel.length()).collect();
        assert!(corvettes.iter().all(|v| *v <= K::Titan.max_speed() * 1.6), "the corvettes keep pace with it: {corvettes:?}");
    }

    #[test]
    fn automatic_picks_a_shape_from_the_fleet_and_its_enemy() {
        let pick = |list: &[(K, &str, u32)], foes: &[(K, &str, u32)]| {
            let state = TacticalState::open_v10(1, 2, &fleet(1, list), &fleet(2, foes), 0, 0.0, Vec2::new(1.0, 0.0));
            let members: Vec<usize> = (0..state.combatants.len()).filter(|&i| state.combatants[i].side == 0).collect();
            auto_shape(&state, &members)
        };
        let guns = [(K::Cruiser, "", 2)];
        let torpedoes = [(K::Raider, "torpedo_rack", 4)];
        assert_eq!(pick(&[(K::Convoy, "", 1), (K::Cruiser, "", 2)], &guns), Some(Shape::Ring), "civilians get a ring");
        assert_eq!(pick(&[(K::Battleship, "", 1), (K::Corvette, "", 2)], &guns), Some(Shape::Line), "warships form a battle line");
        assert_eq!(pick(&[(K::Cruiser, "", 2), (K::Corvette, "point_defense_screen", 2)], &torpedoes), Some(Shape::Ring),
            "against torpedoes, point defense forms a screen");
        assert_eq!(pick(&[(K::Cruiser, "", 3)], &torpedoes), None, "without it, the fleet keeps its own stations");
    }

    #[test]
    fn new_battles_open_on_v10_and_saved_v9_battles_keep_their_rules() {
        let list = [(K::Battleship, "", 1), (K::Cruiser, "", 3)];
        let current = TacticalState::open_v10(3, 44, &fleet(1, &list), &fleet(2, &[(K::Cruiser, "", 2)]), 0, 0.0, Vec2::new(1.0, 0.0));
        assert_eq!(current.rules_version(), 10);
        let old = TacticalState::open_v9(3, 44, &fleet(1, &list), &fleet(2, &[(K::Cruiser, "", 2)]), 0, 0.0, Vec2::new(1.0, 0.0));
        let mut saved: TacticalState = serde_json::from_slice(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(saved.rules_version(), 9);
        let mut live = old.clone();
        for _ in 0..60 {
            live.step(false, [SideMods::default(); 2]);
            saved.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(saved, live, "a saved v9 fight replays exactly");
        let mut a = current.clone();
        for _ in 0..30 { a.step(false, [SideMods::default(); 2]); }
        assert!(!a.formations.is_empty(), "the fleets formed up");
        let mut b: TacticalState = serde_json::from_slice(&serde_json::to_vec(&a).unwrap()).unwrap();
        assert_eq!(b, a, "formations are saved with the battle");
        for _ in 0..30 {
            a.step(false, [SideMods::default(); 2]);
            b.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(a, b, "and replay exactly");
    }

    #[test]
    fn v10_estimates_run_deterministically() {
        let setup = ProjSetup {
            a: fleet(10, &[(K::Raider, "", 6)]),
            d: fleet(30, &[(K::Battleship, "", 1), (K::Corvette, "", 2)]),
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None,
        };
        let first = project_distribution(&setup, 17, 8);
        let again = project_distribution(&setup, 17, 8);
        assert_eq!(first.a_win_pct, again.a_win_pct);
        assert_eq!(first.median.steps, again.median.steps);
        assert_eq!(simulate_engagement(&setup, 3), simulate_engagement(&setup, 3));
    }
}

/// Manual balance comparisons (prints, asserts nothing):
/// `cargo test -p sim --lib balance_probe_v10 -- --ignored --nocapture`.
#[cfg(test)]
mod balance_probe_v10 {
    use super::*;
    use crate::doctrine::Formation as F;
    use crate::module::Loadout;
    use crate::ShipKind as K;

    /// One fleet per side (formations need fleets, not single hulls).
    fn side(fleet: u64, list: &[(K, &str, u64)]) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        for (kind, fit, n) in list {
            for _ in 0..*n { out.push((EntityId(fleet), Ship::new(out.len() as u32, *kind, Loadout::from_key(fit)))); }
        }
        out
    }

    #[test]
    #[ignore]
    fn print_v9_vs_v10_balance() {
        let cases: Vec<(&str, Vec<(K, &str, u64)>, Vec<(K, &str, u64)>, bool)> = vec![
            ("6 raider v 4 corvette", vec![(K::Raider, "", 6)], vec![(K::Corvette, "", 4)], false),
            ("10 raider v 1 battleship+2 cruiser", vec![(K::Raider, "", 10)], vec![(K::Battleship, "", 1), (K::Cruiser, "", 2)], false),
            ("20 raider v titan+4 corvette", vec![(K::Raider, "", 20)], vec![(K::Titan, "", 1), (K::Corvette, "", 4)], false),
            ("4 corvette v 2 destroyer", vec![(K::Corvette, "", 4)], vec![(K::Destroyer, "", 2)], false),
            ("2 cruiser v 1 battleship", vec![(K::Cruiser, "", 2)], vec![(K::Battleship, "", 1)], false),
            ("5 cruiser mirror", vec![(K::Cruiser, "", 5)], vec![(K::Cruiser, "", 5)], false),
            ("3 raider v 5 corvette RAID", vec![(K::Raider, "", 3)], vec![(K::Corvette, "", 5)], true),
            ("6 torp raider v 3 PD corvette+convoy", vec![(K::Raider, "torpedo_rack", 6)], vec![(K::Corvette, "point_defense_screen", 3), (K::Convoy, "", 1)], false),
            ("torp strike v 2 cruiser+dread", vec![(K::Raider, "torpedo_rack", 6), (K::Destroyer, "torpedo_rack", 4)], vec![(K::Cruiser, "", 2), (K::Dreadnought, "", 1)], false),
            ("titan+4 corv v 3 battleship", vec![(K::Titan, "", 1), (K::Corvette, "", 4)], vec![(K::Battleship, "", 3)], false),
            ("3 battleship mirror", vec![(K::Battleship, "", 3)], vec![(K::Battleship, "", 3)], false),
            ("4 cruiser v 8 corvette", vec![(K::Cruiser, "", 4)], vec![(K::Corvette, "", 8)], false),
        ];
        for (name, a, d, raid) in cases {
            let setup = ProjSetup { a: side(100, &a), d: side(500, &d), platform_tiers: 0, raid, a_retreat: None, d_retreat: None };
            let old = super::super::v9::project_distribution(&setup, 7, 32);
            let new = project_distribution(&setup, 7, 32);
            let loss = |m: &BTreeMap<K, u32>| m.values().sum::<u32>();
            eprintln!("{name:40} v9 win {:5.1}% steps {:3} lossA {:2} lossD {:2} | v10 win {:5.1}% steps {:3} lossA {:2} lossD {:2}",
                old.a_win_pct * 100.0, old.median.steps, loss(&old.median.a_losses), loss(&old.median.d_losses),
                new.a_win_pct * 100.0, new.median.steps, loss(&new.median.a_losses), loss(&new.median.d_losses));
        }
    }

    /// Each formation for a few fleets against a few opponents: no shape
    /// should win everywhere.
    #[test]
    #[ignore]
    fn print_formation_matchups() {
        let fleets: Vec<(&str, Vec<(K, &str, u64)>)> = vec![
            ("battleship+4 cruiser", vec![(K::Battleship, "", 1), (K::Cruiser, "", 4)]),
            ("titan+2 bs+4 PD corv", vec![(K::Titan, "", 1), (K::Battleship, "", 2), (K::Corvette, "point_defense_screen", 4)]),
            ("convoy+2 cruiser+3 PD corv", vec![(K::Convoy, "", 1), (K::Cruiser, "", 2), (K::Corvette, "point_defense_screen", 3)]),
        ];
        let foes: Vec<(&str, Vec<(K, &str, u64)>)> = vec![
            ("14 raider", vec![(K::Raider, "", 14)]),
            ("8 torp raider+4 torp dd", vec![(K::Raider, "torpedo_rack", 8), (K::Destroyer, "torpedo_rack", 4)]),
            ("4 cruiser+4 destroyer", vec![(K::Cruiser, "", 4), (K::Destroyer, "", 4)]),
        ];
        for (fname, fl) in &fleets {
            for (oname, op) in &foes {
                let mut row = format!("{fname:28} v {oname:24}");
                for formation in [F::Free, F::BattleLine, F::EscortRing, F::Wedge] {
                    let setup = ProjSetup { a: side(100, op), d: side(500, fl), platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
                    let missions = BTreeMap::from([(EntityId(500), MissionProfile { formation, ..Default::default() })]);
                    let r = project_distribution_with_missions(&setup, 11, 24, &missions);
                    let lost = r.median.d_losses.values().sum::<u32>();
                    row += &format!(" | {:?} hold {:5.1}% lost {lost}", formation, (1.0 - r.a_win_pct) * 100.0);
                }
                eprintln!("{row}");
            }
        }
    }
}

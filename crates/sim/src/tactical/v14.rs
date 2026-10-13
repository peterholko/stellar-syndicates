//! Home ground. A battle inside a system is fought at its defended world: the
//! world stands in the arena behind the side that holds the system, as
//! terrain (hulls steer around it, torpedoes burst on it), and that side's
//! Defense Platforms hold orbit on its side toward the enemy, grouped where
//! they would stand in open space. Reinforcements enter
//! from the direction their fleet actually arrived, so a relief fleet can hit
//! a besieger from behind. Everything else is v13; a battle in open space is
//! fought exactly as under v13.

use super::v1::{
    Combatant, Distribution, ProjSetup, SimOutcome, TacticalState, PROJECTION_BATTLE_ID, SPAWN_DIST,
    WITHDRAW_EXIT_RADIUS,
};
use super::{v3, v9, Rules};
use crate::doctrine::MissionProfile;
use crate::ship::Ship;
use crate::{EntityId, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The world's radius in the arena...
pub const WORLD_RADIUS: f64 = 260.0;
/// ...and how far its centre sits behind the anchor of the side that holds it
/// (the defenders' origin, or the attackers' deployment point). Far enough
/// back that the holding side keeps the room it has in open space: with the
/// world 520 behind and the platforms spread across its face, attackers won
/// 61% at a defended world against 50% in open space (beam ships backing off
/// from shorter-ranged guns met the world, and spread platforms were picked
/// off one by one).
pub const WORLD_OFFSET: f64 = 900.0;
/// The holding side's platforms hold orbit this far apart, as tightly grouped
/// as they stand in open space.
const PLATFORM_SPACING: f64 = 20.0;
/// How far outside the world a hull keeps its desired point (beyond its own
/// body), so it rounds the world instead of grinding along its surface.
const WORLD_CLEARANCE: f64 = 30.0;

/// The defended world in a battle's arena (v14 battles at a system).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldBody {
    pub pos: Vec2,
    pub radius: f64,
    /// The side whose system it is.
    pub side: u8,
}

impl TacticalState {
    /// Frozen v14 constructor: v15 builds on it, and tests compare against it.
    pub(crate) fn open_v14(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v13(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V13 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V14 { maneuver_seed };
        state
    }

    /// Fight this battle at a world `home` holds (0 attacker, 1 defender):
    /// place the world behind that side, and group its platforms in orbit at
    /// the side's anchor, on the world's side toward the enemy. Call right
    /// after opening, before the first recording
    /// checkpoint (like `add_initial_attacker_platforms`). Only v14 battles
    /// know worlds; a second call changes nothing.
    pub fn place_world(&mut self, home: u8) {
        if !matches!(self.rules, Rules::V14 { .. } | Rules::V15 { .. } | Rules::V16 { .. }) || self.world.is_some() {
            return;
        }
        let home = home.min(1);
        // The holding side's anchor, and the direction toward its enemy.
        let (anchor, toward) = if home == 1 {
            (Vec2::ZERO, self.bearing)
        } else {
            (self.bearing * SPAWN_DIST, self.bearing * -1.0)
        };
        let centre = anchor - toward * WORLD_OFFSET;
        self.world = Some(WorldBody { pos: centre, radius: WORLD_RADIUS, side: home });
        let platforms: Vec<usize> = (0..self.combatants.len())
            .filter(|&i| self.combatants[i].platform && self.combatants[i].side == home)
            .collect();
        let math = self.math();
        let face = math.atan2(toward.y, toward.x);
        let n = platforms.len();
        let arc = PLATFORM_SPACING * n.saturating_sub(1) as f64 / WORLD_OFFSET;
        for (k, &i) in platforms.iter().enumerate() {
            let t = if n > 1 { k as f64 / (n - 1) as f64 - 0.5 } else { 0.0 };
            self.combatants[i].pos = centre + math.polar(face + t * arc, WORLD_OFFSET);
        }
    }

    /// How far out a withdrawing hull must get to leave the battle: the same
    /// at a world as in open space. A farther edge at a world (1.4×) was
    /// tried and measured: fighting hulls keep inside the arena ring, so a
    /// runner tackled at the farther edge sat beyond the pursuers' guns, and
    /// more beaten fleets got away, not fewer (and widening the ring with it
    /// changed the fight itself).
    pub fn exit_radius(&self) -> f64 {
        WITHDRAW_EXIT_RADIUS
    }
}

/// Where a joining hull enters: at the arena's edge on the side its fleet came
/// from (`from`, a unit vector from the battle toward where the fleet was),
/// fanned across that line like an opening deployment.
pub(super) fn arrival_point(from: Vec2, idx: f64) -> Vec2 {
    let lateral = Vec2::new(-from.y, from.x);
    let spread = (idx % 21.0 - 10.0) * 35.0 + (idx / 21.0).floor() * 12.0;
    from * SPAWN_DIST + lateral * spread
}

/// A hull never aims into the world: a desired point inside it, or a straight
/// run that would cross it, is moved to its rim on the near side, so the hull
/// goes around.
pub(super) fn clear_of_world(state: &TacticalState, i: usize, want: Vec2) -> Vec2 {
    let Some(w) = state.world else { return want };
    let c = &state.combatants[i];
    if c.platform {
        return want;
    }
    let keep = w.radius + v9::body_radius(c.kind, false, state.math()) + WORLD_CLEARANCE;
    let gap = want - w.pos;
    let d = gap.length();
    if d < keep {
        let away = if d > 1e-9 { gap * (1.0 / d) } else { rim_side(c, w.pos) };
        return w.pos + away * keep;
    }
    let run = want - c.pos;
    let len2 = run.length_sq();
    if len2 < 1e-9 || c.pos.distance(w.pos) < keep {
        return want;
    }
    let t = ((w.pos - c.pos).dot(run) / len2).clamp(0.0, 1.0);
    let closest = c.pos + run * t;
    let miss = closest.distance(w.pos);
    if miss < keep {
        let side = if miss > 1e-9 { (closest - w.pos) * (1.0 / miss) } else { Vec2::new(-run.y, run.x).normalized() };
        return w.pos + side * (keep * 1.05);
    }
    want
}

fn rim_side(c: &Combatant, centre: Vec2) -> Vec2 {
    if c.pos.distance(centre) > 1e-9 { (c.pos - centre).normalized() } else { Vec2::new(1.0, 0.0) }
}

/// After the hulls move: none may sit inside the world. Each is set back on
/// its surface and loses the speed carrying it inward (no damage: the world
/// is no wreck to ram).
pub(super) fn keep_out_of_world(state: &mut TacticalState) {
    let Some(w) = state.world else { return };
    let math = state.math();
    for c in state.combatants.iter_mut().filter(|c| !c.platform && c.hp > 0.0) {
        let contact = w.radius + v9::body_radius(c.kind, false, math);
        let gap = c.pos - w.pos;
        let d = gap.length();
        if d >= contact {
            continue;
        }
        let n = if d > 1e-9 { gap * (1.0 / d) } else { Vec2::new(1.0, 0.0) };
        c.pos = w.pos + n * contact;
        let inward = c.vel.dot(n);
        if inward < 0.0 {
            c.vel = c.vel - n * inward;
        }
    }
}

/// Where a torpedo flying `from` → `to` this step first strikes the world.
pub(super) fn torpedo_blocked(state: &TacticalState, from: Vec2, to: Vec2) -> Option<Vec2> {
    let w = state.world?;
    let d = to - from;
    let a = d.length_sq();
    if a < 1e-12 {
        return None;
    }
    let f = from - w.pos;
    let c = f.length_sq() - w.radius * w.radius;
    if c <= 0.0 {
        return Some(from); // inside it already: it struck the surface
    }
    let b = 2.0 * f.dot(d);
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / (2.0 * a);
    (0.0..=1.0).contains(&t).then(|| from + d * t)
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V14 { maneuver_seed }
}

#[allow(dead_code)] // archived: new estimates run v15
pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    super::v1::simulate_engagement_with_rules(setup, seed, rules(seed))
}

#[allow(dead_code)] // archived: new estimates run v15
pub fn project_distribution(setup: &ProjSetup, seed: u64, k: u32) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, simulate_engagement)
}

/// Estimation receives only profiles in the observer's arrived picture (v6).
#[allow(dead_code)] // archived: new estimates run v15
pub fn project_distribution_with_missions(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        super::v1::simulate_engagement_full(s, seed, rules(seed), missions, None, &[Vec::new(), Vec::new()])
    })
}

/// The same estimate for a fight at a world (a target within platform cover
/// of a system), held by side `home` (0 attacker, 1 defender): the world
/// stands behind that side as it will in the battle.
#[allow(dead_code)] // archived: new estimates run v15
pub fn project_distribution_at_world(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>, home: u8) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        super::v1::simulate_engagement_full(s, seed, rules(seed), missions, Some(home), &[Vec::new(), Vec::new()])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::v1::SideMods;
    use crate::module::Loadout;
    use crate::ShipKind as K;

    fn fleet(id: u64, list: &[(K, &str, u32)]) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        for (kind, fit, n) in list {
            for _ in 0..*n {
                out.push((EntityId(id), Ship::new(out.len() as u32, *kind, Loadout::from_key(fit))));
            }
        }
        out
    }

    fn open(seed: u64, a: &[(K, &str, u32)], d: &[(K, &str, u32)], tiers: u32) -> TacticalState {
        TacticalState::open_v14(seed, 93, &fleet(1, a), &fleet(2, d), tiers, 0.0, Vec2::new(1.0, 0.0))
    }

    /// Probe: the same fights in open space and at the defender's world, 200
    /// rollouts each. Prints the attacker's win rate and the share of its hulls
    /// that come home. Run with
    /// `cargo test -p sim --release --lib -- probe_home_ground --ignored --nocapture`.
    #[test]
    #[ignore]
    fn probe_home_ground() {
        let missions = BTreeMap::new();
        let bb = "mass_driver+mass_driver+mass_driver+mass_driver";
        let cases: Vec<(&str, Vec<(K, &str, u32)>, Vec<(K, &str, u32)>, u32, Option<f64>)> = vec![
            ("8 torp ints + 3 driver cruisers vs 3 cruisers, 2 PD corvettes, 3 tiers",
                vec![(K::Raider, "torpedo_rack", 8), (K::Cruiser, "mass_driver+mass_driver", 3)],
                vec![(K::Cruiser, "", 3), (K::Corvette, "point_defense_screen", 2)], 3, None),
            ("2 driver BBs vs 4 cruisers, 4 destroyers, 2 tiers",
                vec![(K::Battleship, bb, 2)], vec![(K::Cruiser, "", 4), (K::Destroyer, "", 4)], 2, None),
            ("3 driver BBs vs 4 cruisers, 4 destroyers, 2 tiers",
                vec![(K::Battleship, bb, 3)], vec![(K::Cruiser, "", 4), (K::Destroyer, "", 4)], 2, None),
            ("4 cruisers retreating at 50% vs 6 cruisers + 6 web ints",
                vec![(K::Cruiser, "", 4)], vec![(K::Cruiser, "", 6), (K::Raider, "stasis_web", 6)], 0, Some(0.5)),
            ("3 driver BBs retreating at 75% vs 10 web ints + 4 cruisers",
                vec![(K::Battleship, bb, 3)], vec![(K::Raider, "stasis_web", 10), (K::Cruiser, "", 4)], 0, Some(0.75)),
        ];
        for (name, a, d, tiers, a_retreat) in cases {
            let setup = ProjSetup { a: fleet(1, &a), d: fleet(2, &d), platform_tiers: tiers, raid: false, a_retreat, d_retreat: None };
            let start: u32 = a.iter().map(|(_, _, n)| n).sum();
            // Hulls home counts only fights that finished; a fight still on at
            // the step cap is counted apart.
            let run = |home: Option<u8>| -> (f64, f64, u32) {
                let (mut won, mut home_hulls, mut finished) = (0u32, 0u32, 0u32);
                for i in 0..200u64 {
                    let o = super::super::v1::simulate_engagement_full(&setup, 1000 + i, rules(1000 + i), &missions, home, &[Vec::new(), Vec::new()]);
                    won += o.a_won as u32;
                    if o.steps < super::super::v1::MAX_PROJ_STEPS {
                        finished += 1;
                        home_hulls += o.a_survivors.values().sum::<u32>();
                    }
                }
                (won as f64 / 2.0, home_hulls as f64 * 100.0 / (finished.max(1) as f64 * start as f64), 200 - finished)
            };
            let (open_win, open_home, open_cap) = run(None);
            let (world_win, world_home, world_cap) = run(Some(1));
            eprintln!("{name}: attacker wins {open_win:.0}% open / {world_win:.0}% at the world; \
                hulls home {open_home:.0}% / {world_home:.0}%; at the step cap {open_cap} / {world_cap}");
        }
    }

    #[test]
    fn a_fight_in_open_space_is_exactly_v13() {
        let a = [(K::Cruiser, "mass_driver+mass_driver", 3), (K::Raider, "torpedo_rack", 6)];
        let d = [(K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver", 1), (K::Destroyer, "", 3)];
        let mut new = open(4, &a, &d, 0);
        let mut old = new.clone();
        let Rules::V14 { maneuver_seed } = new.rules else { unreachable!() };
        old.rules = Rules::V13 { maneuver_seed };
        for step in 0..150 {
            new.step(false, [SideMods::default(); 2]);
            old.step(false, [SideMods::default(); 2]);
            let mut same = new.clone();
            same.rules = old.rules;
            assert_eq!(same, old, "step {step}");
        }
    }

    #[test]
    fn at_a_world_platforms_hold_orbit_and_nothing_enters_it() {
        let mut state = open(5, &[(K::Cruiser, "", 4), (K::Raider, "", 10)], &[(K::Cruiser, "", 3)], 3);
        state.place_world(1);
        let w = state.world.expect("placed");
        assert_eq!((w.pos, w.radius, w.side), (Vec2::new(-WORLD_OFFSET, 0.0), WORLD_RADIUS, 1));
        let platforms: Vec<Vec2> = state.combatants.iter().filter(|c| c.platform).map(|c| c.pos).collect();
        assert_eq!(platforms.len(), 3);
        for p in &platforms {
            assert!((p.distance(w.pos) - WORLD_OFFSET).abs() < 1e-9, "in orbit at the anchor: {p:?}");
            assert!(p.length() <= PLATFORM_SPACING + 1e-6, "grouped as in open space: {p:?}");
        }
        assert!((platforms[0].distance(platforms[1]) - PLATFORM_SPACING).abs() < 0.01);
        state.place_world(0);
        assert_eq!(state.world, Some(w), "placed once");
        // The defenders withdraw at once: their way out lies straight through
        // the world, so they go around it, and nothing ever sits inside it.
        state.order_withdraw(1);
        for _ in 0..200 {
            state.step(false, [SideMods::default(); 2]);
            for c in state.combatants.iter().filter(|c| !c.platform) {
                assert!(c.pos.distance(w.pos) >= w.radius + v9::body_radius(c.kind, false, crate::tactical::Math::Soft) - 1e-6,
                    "{:?} at {:?} is inside the world", c.kind, c.pos);
            }
        }
    }

    #[test]
    fn torpedoes_burst_on_the_world() {
        let mut state = open(6, &[(K::Raider, "torpedo_rack", 1)], &[(K::Cruiser, "", 1)], 0);
        state.place_world(1);
        let w = state.world.unwrap();
        let hit = torpedo_blocked(&state, w.pos + Vec2::new(0.0, 600.0), w.pos + Vec2::new(0.0, -600.0)).expect("crosses it");
        assert!((hit.distance(w.pos) - w.radius).abs() < 1e-6 && hit.y > w.pos.y, "{hit:?}");
        assert!(torpedo_blocked(&state, w.pos + Vec2::new(400.0, 600.0), w.pos + Vec2::new(400.0, -600.0)).is_none());
        state.world = None;
        assert!(torpedo_blocked(&state, Vec2::new(-520.0, 600.0), Vec2::new(-520.0, -600.0)).is_none(), "open space");
    }

    #[test]
    fn leaving_a_battle_at_a_world_takes_the_same_run_as_in_open_space() {
        let mut state = open(7, &[(K::Raider, "", 4)], &[(K::Cruiser, "", 2)], 0);
        state.place_world(1);
        assert_eq!(state.exit_radius(), WITHDRAW_EXIT_RADIUS);
        state.order_withdraw(0);
        let out = (0..400).find(|_| {
            state.step(false, [SideMods::default(); 2]);
            state.side_withdrawn(0)
        });
        assert!(out.is_some(), "the interceptors got out");
    }

    #[test]
    fn relief_enters_from_where_it_came_and_replays_the_same() {
        let a = fleet(1, &[(K::Cruiser, "", 3)]);
        let mut d = fleet(2, &[(K::Destroyer, "", 2)]);
        let mut state = TacticalState::open_current(8, 94, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        state.place_world(1);
        for _ in 0..5 {
            state.step(false, [SideMods::default(); 2]);
        }
        let before = state.clone();
        // A relief fleet for the defenders arrives from beyond the attackers.
        let relief: Vec<(EntityId, Ship)> = (0..3).map(|k| (EntityId(3), Ship::new(k, K::Cruiser, Loadout::default()))).collect();
        d.extend(relief);
        let arrivals = BTreeMap::from([(EntityId(3), Vec2::new(1.0, 0.0))]);
        let (_, changes) = state.sync_recorded([&a, &d], &arrivals);
        let entered: Vec<Vec2> = state.combatants.iter().filter(|c| c.origin.is_some_and(|o| o.0 == EntityId(3))).map(|c| c.pos).collect();
        assert_eq!(entered.len(), 3);
        for p in &entered {
            assert!(p.x > SPAWN_DIST - 50.0, "it enters behind the attackers, on the side it came from: {p:?}");
        }
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].from, vec![(EntityId(3), Vec2::new(1.0, 0.0))], "the direction is recorded");
        // A replay applying the recorded change rebuilds the same arena.
        let mut replayed = before;
        replayed.apply_roster_delta(&changes);
        assert_eq!(replayed, state);
        // Rules before v14 bring relief in at the usual deployment.
        let mut old = TacticalState::open_v13(8, 94, &a, &fleet(2, &[(K::Destroyer, "", 2)]), 0, 0.0, Vec2::new(1.0, 0.0));
        let (_, changes) = old.sync_recorded([&a, &d], &arrivals);
        assert!(changes[0].from.is_empty());
        assert!(old.combatants.iter().filter(|c| c.origin.is_some_and(|o| o.0 == EntityId(3))).all(|c| c.pos.x < 100.0));
    }

    #[test]
    fn the_estimate_at_a_world_places_the_world_and_is_deterministic() {
        let setup = ProjSetup { a: fleet(1, &[(K::Raider, "torpedo_rack", 8)]),
            d: fleet(2, &[(K::Cruiser, "", 3)]), platform_tiers: 2, raid: false, a_retreat: None, d_retreat: None };
        let missions = BTreeMap::new();
        let at = project_distribution_at_world(&setup, 17, 8, &missions, 1);
        assert_eq!(at.a_win_pct, project_distribution_at_world(&setup, 17, 8, &missions, 1).a_win_pct);
        let state_at = {
            let mut s = TacticalState::open(17, PROJECTION_BATTLE_ID, &setup.a, &setup.d, 2, 0.0, Vec2::new(1.0, 0.0));
            s.rules = rules(17);
            s.place_world(1);
            s
        };
        assert!(state_at.world.is_some(), "the estimate places the world");
        assert_eq!(crate::tactical::simulate_engagement(&setup, 3), simulate_engagement(&setup, 3), "the estimate runs v14");
    }
}

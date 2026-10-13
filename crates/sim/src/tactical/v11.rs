//! Roles for small ships. Interceptors tackle: a hull trying to leave the
//! battle can't cross the disengage edge while more live enemy interceptors
//! are within tackle range of it than it carries Warp Stabilizers. Kill or
//! outrun the tacklers and it gets away. Interceptors close on whatever is
//! leaving, the heaviest hull first, enough of them to hold each one. A
//! Stasis Web slows the nearest enemy hull in range to half speed, so it is
//! easier to hit and slower to leave. Destroyers fire their stock beams as
//! light batteries: several tracking shots that hit small, fast hulls far
//! more often and barely scratch heavy armour. Small hulls carry light guns:
//! an interceptor's or corvette's beams and drivers hit destroyers and
//! anything bigger for much less (other small hulls, freighters and every
//! torpedo hit in full, so dogfights and raiding are unchanged).
//! Everything else is v10. New battles open on v12 (`open_current`), which
//! keeps all of this.

use super::rules_v1::hull_affinity;
use super::v1::{
    Combatant, Distribution, ProjSetup, Role, SimOutcome, StepOutcome, TacticalState, DMG_VAR, HIT_BASE,
    HIT_CLAMP, PROJECTION_BATTLE_ID,
};
use super::{v3, Rules};
use crate::combat::KfLink;
use crate::doctrine::MissionProfile;
use crate::module::{DamageType, Family, Loadout, ModuleKind};
use crate::ship::{Ship, ShipKind};
use crate::{EntityId, Vec2};
use std::collections::BTreeMap;

/// How close an interceptor must be to hold a hull (arena units, centre to
/// centre). A beam interceptor skirmishes just outside it, so it closes in
/// when something tries to leave.
pub const TACKLE_RANGE: f64 = 280.0;
/// A Stasis Web reaches the nearest enemy hull within this range.
pub const WEB_RANGE: f64 = 240.0;
/// A webbed hull's top speed, as a share of its own. Webs don't stack.
pub const WEB_SPEED: f64 = 0.5;
/// A destroyer's light batteries fire this many shots a round...
pub const BATTERY_SHOTS: usize = 4;
/// ...sharing this much of the stock beam's damage between them (each shot
/// carries 0.3 of a beam)...
pub const BATTERY_DAMAGE: f64 = 1.2;
/// ...with this little speed sensitivity (a stock beam's is 3.6), so they
/// track darting hulls...
pub const BATTERY_SPEED_SENS: f64 = 1.5;
/// ...and hulls heavier than this shrug off part of each hit: the damage
/// falls with (this / mass)^BATTERY_ARMOR_EXP.
pub const BATTERY_ARMOR_MASS: f64 = 2_000.0;
pub const BATTERY_ARMOR_EXP: f64 = 0.5;
/// Small hulls carry light guns: an interceptor's or corvette's beams and
/// drivers hit armed hulls for this share of its attack weight.
pub const INTERCEPTOR_GUNS: f64 = 0.3;
pub const CORVETTE_GUNS: f64 = 0.5;

impl TacticalState {
    /// Frozen v11 constructor (new battles open on the current rules: `open_current`).
    pub(crate) fn open_v11(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v10(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V10 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V11 { maneuver_seed };
        state
    }
}

/// A hull on its way out (its side or its fleet is withdrawing).
fn leaving(c: &Combatant) -> bool {
    c.role == Role::Withdraw && !c.platform && c.hp > 0.0
}

/// A live interceptor that is still fighting can hold a hull.
fn tackler(c: &Combatant) -> bool {
    c.kind == ShipKind::Raider && !c.platform && c.hp > 0.0 && c.role != Role::Withdraw
}

/// How many tacklers a hull shrugs off: one per Warp Stabilizer it carries.
pub fn stabilizers(stack: &str) -> usize {
    Loadout::from_key(stack).modules().iter().filter(|m| **m == ModuleKind::WarpStabilizer).count()
}

/// Every hold this step, as (tackler, held hull) index pairs: a leaving hull
/// that started the step inside the disengage edge is held while more enemy
/// tacklers are within range of it than it carries stabilizers.
pub(super) fn holds(state: &TacticalState, before: &[Vec2]) -> Vec<(usize, usize)> {
    let cs = &state.combatants;
    let mut out = Vec::new();
    if !cs.iter().any(leaving) {
        return out;
    }
    for (i, c) in cs.iter().enumerate() {
        if !leaving(c) || before.get(i).is_none_or(|p| p.length() >= state.exit_radius()) {
            continue;
        }
        let on: Vec<usize> = cs.iter().enumerate()
            .filter(|(_, t)| t.side != c.side && tackler(t) && t.pos.distance(c.pos) <= TACKLE_RANGE)
            .map(|(j, _)| j)
            .collect();
        if on.len() > stabilizers(&c.stack) {
            out.extend(on.into_iter().map(|j| (j, i)));
        }
    }
    out
}

/// A held hull stays inside the disengage edge: it is stopped at the edge
/// and loses its outward speed.
pub(super) fn hold_at_edge(state: &mut TacticalState, holds: &[(usize, usize)]) {
    let edge = state.exit_radius() - 1.0;
    for &(_, i) in holds {
        let c = &mut state.combatants[i];
        let r = c.pos.length();
        if r <= edge {
            continue;
        }
        let out = c.pos * (1.0 / r);
        c.pos = out * edge;
        let radial = c.vel.dot(out);
        if radial > 0.0 {
            c.vel = c.vel - out * radial;
        }
    }
}

/// Where each interceptor goes to hold a leaving enemy: the heaviest leaving
/// hull first, each covered by one more tackler than it carries stabilizers,
/// nearest tacklers first. When the whole enemy side is leaving, every other
/// interceptor chases the nearest runner too; otherwise the rest keep
/// skirmishing (`None`).
pub(super) fn tackle_points(state: &TacticalState) -> Vec<Option<Vec2>> {
    let math = state.math();
    let cs = &state.combatants;
    let mut points = vec![None; cs.len()];
    let mut runners: Vec<usize> = (0..cs.len())
        .filter(|&i| leaving(&cs[i]) && cs[i].pos.length() < state.exit_radius())
        .collect();
    if runners.is_empty() {
        return points;
    }
    runners.sort_by(|&a, &b| cs[b].max_hp.total_cmp(&cs[a].max_hp).then(cs[a].cid.cmp(&cs[b].cid)));
    let mut free: Vec<usize> = (0..cs.len()).filter(|&j| tackler(&cs[j])).collect();
    let mut covering = vec![0usize; cs.len()];
    for &r in &runners {
        let runner = &cs[r];
        let mut near: Vec<usize> = free.iter().copied().filter(|&j| cs[j].side != runner.side).collect();
        near.sort_by(|&a, &b| cs[a].pos.distance(runner.pos).total_cmp(&cs[b].pos.distance(runner.pos))
            .then(cs[a].cid.cmp(&cs[b].cid)));
        near.truncate(stabilizers(&runner.stack) + 1);
        for &j in &near {
            points[j] = Some(hold_point(&cs[j], runner, math));
        }
        covering[r] = near.len();
        free.retain(|j| !near.contains(j));
    }
    // A rout: the rest spread over the runners, thinnest cover first.
    for j in free {
        let t = &cs[j];
        let routed = !cs.iter().any(|e| e.side != t.side && !e.platform && e.hp > 0.0 && !leaving(e));
        let pick = runners.iter().copied().filter(|&r| cs[r].side != t.side)
            .min_by(|&a, &b| covering[a].cmp(&covering[b])
                .then(cs[a].pos.distance(t.pos).total_cmp(&cs[b].pos.distance(t.pos)))
                .then(cs[a].cid.cmp(&cs[b].cid)));
        if let (true, Some(r)) = (routed, pick) {
            points[j] = Some(hold_point(t, &cs[r], math));
            covering[r] += 1;
        }
    }
    points
}

/// A leaving hull heads straight out along its side's line (attackers back the
/// way they came, defenders directly away from them), as v6 fleet
/// withdrawals do. Fleeing "away from the enemy" would spin it in circles
/// once tacklers orbit it.
pub(super) fn exit_point(state: &TacticalState, c: &Combatant) -> Option<Vec2> {
    let exit = state.exit_radius() * 1.5;
    leaving(c).then(|| state.bearing * if c.side == 0 { exit } else { -exit })
}

/// A tackler orbits where the runner is headed, well inside tackle range, at
/// flank speed: the point sits ahead on that circle (cid parity picks the
/// direction), so it keeps its speed, and its speed is its armour.
fn hold_point(t: &Combatant, runner: &Combatant, math: super::Math) -> Vec2 {
    let centre = runner.pos + runner.vel;
    let from = t.pos - centre;
    let dir = if from.length() > 1e-9 {
        from.normalized()
    } else if runner.vel.length() > 1e-9 {
        runner.vel.normalized()
    } else {
        Vec2::new(1.0, 0.0)
    };
    let turn = if t.cid.is_multiple_of(2) { HOLD_LEAD } else { -HOLD_LEAD };
    let (s, c) = math.sin_cos(turn);
    let ahead = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
    centre + ahead * (TACKLE_RANGE * HOLD_ORBIT)
}

/// A tackler's orbit: this share of tackle range, aimed this far ahead (rad).
const HOLD_ORBIT: f64 = 0.6;
const HOLD_LEAD: f64 = 0.9;

/// Every web this step, as (web hull, target) index pairs: each web-fitted
/// hull slows the nearest enemy hull in range (platforms can't be slowed).
pub(super) fn webs(state: &TacticalState) -> Vec<(usize, usize)> {
    let cs = &state.combatants;
    let mut out = Vec::new();
    for (i, c) in cs.iter().enumerate() {
        if c.platform || c.hp <= 0.0 || !Loadout::from_key(&c.stack).modules().contains(&ModuleKind::StasisWeb) {
            continue;
        }
        let target = cs.iter().enumerate()
            .filter(|(_, e)| e.side != c.side && !e.platform && e.hp > 0.0 && e.pos.distance(c.pos) <= WEB_RANGE)
            .min_by(|(_, a), (_, b)| a.pos.distance(c.pos).total_cmp(&b.pos.distance(c.pos)).then(a.cid.cmp(&b.cid)));
        if let Some((j, _)) = target {
            out.push((i, j));
        }
    }
    out
}

/// The links, as battle-local ids for the record.
pub(super) fn links(state: &TacticalState, pairs: &[(usize, usize)]) -> Vec<KfLink> {
    pairs.iter().map(|&(a, b)| KfLink { from: state.combatants[a].cid, to: state.combatants[b].cid }).collect()
}

/// Small hulls' beams and drivers are light guns: against an armed hull of
/// destroyer size or bigger (or a platform) they hit for a share of their
/// weight, and like batteries they barely scratch heavy armour. Other small
/// hulls and freighters take full damage, and torpedoes are never light.
pub(super) fn gun_weight(kind: ShipKind, family: DamageType, target: Option<(f64, bool)>, weight: f64, math: super::Math) -> f64 {
    let light = match kind {
        ShipKind::Raider => INTERCEPTOR_GUNS,
        ShipKind::Corvette => CORVETTE_GUNS,
        _ => return weight,
    };
    match target {
        Some((mass, platform)) if family != DamageType::Torpedo && (platform || mass >= BATTERY_ARMOR_MASS) =>
            weight * light * battery_armor(mass, platform, math),
        _ => weight,
    }
}

/// A destroyer firing its stock beams (or a point-defense beam) fires light
/// batteries.
pub(super) fn fires_batteries(kind: ShipKind, fit: &Loadout, family: DamageType) -> bool {
    kind == ShipKind::Destroyer && family == DamageType::Beam && !fit.modules().contains(&ModuleKind::PrismaticLance)
}

/// A battery shot's chance to hit: the beam's base, no mass term, and little
/// speed sensitivity.
pub fn battery_to_hit(target_speed: f64) -> f64 {
    (HIT_BASE[0] / (1.0 + target_speed / 100.0 * BATTERY_SPEED_SENS)).clamp(HIT_CLAMP.0, HIT_CLAMP.1)
}

/// The share of a battery hit that heavy armour lets through (platforms count
/// as battleship-heavy).
pub fn battery_armor(mass: f64, platform: bool, math: super::Math) -> f64 {
    let mass = if platform { super::v9::LARGE_HULL_MASS } else { mass };
    math.powf(BATTERY_ARMOR_MASS / mass.max(BATTERY_ARMOR_MASS), BATTERY_ARMOR_EXP)
}

/// Each fire candidate's combatant index. The candidates were gathered in
/// combatant order, so one pass finds them all.
pub(super) fn candidate_indices(state: &TacticalState, cands: &[(u32, f64, f64, f64, bool)]) -> Vec<usize> {
    let mut at = Vec::with_capacity(cands.len());
    for (idx, c) in state.combatants.iter().enumerate() {
        if at.len() < cands.len() && c.cid == cands[at.len()].0 {
            at.push(idx);
        }
    }
    if at.len() != cands.len() {
        at = cands.iter().filter_map(|c| state.combatants.iter().position(|x| x.cid == c.0)).collect();
    }
    at
}

/// One battery volley from combatant `i` at the in-range candidates (`cid`,
/// v6 target weight, distance, speed, platform). Each shot picks its own
/// target, favouring small hulls (the v6 weight divided by mass twice), rolls
/// to hit and for damage like a beam (Reflective Plating blunting `reflect`
/// of it), and records its gunfire.
pub(super) fn battery_volley(state: &mut TacticalState, i: usize, cands: &[(u32, f64, f64, f64, bool)],
    base: f64, reflect: f64, out: &mut StepOutcome) {
    let math = state.math();
    let (side, from) = (state.combatants[i].side, state.combatants[i].cid);
    let at = candidate_indices(state, cands);
    for _ in 0..BATTERY_SHOTS {
        let live: Vec<(usize, f64)> = at.iter().zip(cands)
            .filter(|(idx, _)| state.combatants[**idx].hp > 0.0)
            .map(|(&idx, &(_, weight, ..))| {
                let mass = state.combatants[idx].max_hp.max(1.0);
                (idx, weight / (mass * mass))
            })
            .collect();
        let total: f64 = live.iter().map(|c| c.1).sum();
        if live.is_empty() || total <= 0.0 {
            break;
        }
        let mut roll = state.rng.next_f64() * total;
        let mut pick = live.len() - 1;
        for (k, c) in live.iter().enumerate() {
            if roll < c.1 {
                pick = k;
                break;
            }
            roll -= c.1;
        }
        let target = &state.combatants[live[pick].0];
        let (to, mass, speed, platform, kind) = (target.cid, target.max_hp, target.vel.length(), target.platform, target.kind);
        let reflects = !platform && Loadout::from_key(&target.stack).reflects();
        let connects = state.rng.next_f64() < battery_to_hit(speed);
        let mut dmg = base * (BATTERY_DAMAGE / BATTERY_SHOTS as f64) * battery_armor(mass, platform, math)
            * state.rng.range(DMG_VAR.0, DMG_VAR.1);
        if reflects {
            dmg *= 1.0 - (reflect * hull_affinity(kind, Family::Protection)).min(0.95);
        }
        let damage = if connects { state.apply_damage(to, dmg, out, side) } else { 0.0 };
        out.gunfire.push(crate::combat::KfGunfire { side, from, to, weapon: DamageType::Beam, damage: damage as f32 });
    }
}

fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V11 { maneuver_seed }
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
    use super::super::v1::{SideMods, WITHDRAW_EXIT_RADIUS};
    use crate::ShipKind as K;

    /// One fleet (`id`) of these hulls.
    fn fleet(id: u64, list: &[(K, &str, u32)]) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        for (kind, fit, n) in list {
            for _ in 0..*n {
                out.push((EntityId(id), Ship::new(out.len() as u32, *kind, Loadout::from_key(fit))));
            }
        }
        out
    }

    /// Every hull far too tough to die this step, so only tackle decides who
    /// leaves (a battleship one-shots an interceptor on a good roll).
    fn unkillable(state: &mut TacticalState) {
        for c in &mut state.combatants { c.hp = c.max_hp * 100.0; }
    }

    /// Defenders `d` at the origin withdraw at once; attackers `a` close.
    fn runaway(a: &[(K, &str, u32)], d: &[(K, &str, u32)], steps: usize) -> TacticalState {
        let mut state = TacticalState::open_v11(3, 77, &fleet(1, a), &fleet(2, d), 0, 0.0, Vec2::new(1.0, 0.0));
        state.order_withdraw(1);
        for _ in 0..steps {
            unkillable(&mut state);
            state.step(false, [SideMods::default(); 2]);
        }
        state
    }

    fn out_of_the_fight(state: &TacticalState, side: u8) -> usize {
        state.combatants.iter().filter(|c| c.side == side && c.pos.length() >= WITHDRAW_EXIT_RADIUS).count()
    }

    #[test]
    fn interceptors_hold_a_battleship_that_tries_to_leave() {
        let held = runaway(&[(K::Raider, "", 2)], &[(K::Battleship, "", 1)], 120);
        assert_eq!(out_of_the_fight(&held, 1), 0, "two tacklers keep it in");
        let bs = held.combatants.iter().find(|c| c.kind == K::Battleship).unwrap();
        assert!(bs.pos.length() > WITHDRAW_EXIT_RADIUS - 60.0, "it reached the edge and stopped there: {}", bs.pos.length());
        // Without tacklers it gets away.
        let free = runaway(&[(K::Cruiser, "", 2)], &[(K::Battleship, "", 1)], 120);
        assert_eq!(out_of_the_fight(&free, 1), 1);
    }

    #[test]
    fn killing_the_tacklers_frees_the_hull() {
        let mut state = runaway(&[(K::Raider, "", 1)], &[(K::Battleship, "", 1)], 120);
        assert_eq!(out_of_the_fight(&state, 1), 0);
        state.combatants.retain(|c| c.kind != K::Raider);
        for _ in 0..30 {
            unkillable(&mut state);
            state.step(false, [SideMods::default(); 2]);
        }
        assert_eq!(out_of_the_fight(&state, 1), 1);
    }

    #[test]
    fn each_warp_stabilizer_shrugs_off_one_tackler() {
        let one = runaway(&[(K::Raider, "", 1)], &[(K::Battleship, "warp_stabilizer", 1)], 120);
        assert_eq!(out_of_the_fight(&one, 1), 1, "one stabilizer beats one tackler");
        let two = runaway(&[(K::Raider, "", 2)], &[(K::Battleship, "warp_stabilizer", 1)], 120);
        assert_eq!(out_of_the_fight(&two, 1), 0, "but not two");
    }

    #[test]
    fn tacklers_spread_over_everything_leaving() {
        let state = runaway(&[(K::Raider, "", 4)], &[(K::Cruiser, "", 2)], 120);
        assert_eq!(out_of_the_fight(&state, 1), 0, "two each holds both cruisers");
        let thin = runaway(&[(K::Raider, "", 1)], &[(K::Cruiser, "", 2)], 120);
        assert_eq!(out_of_the_fight(&thin, 1), 1, "one tackler holds one");
    }

    #[test]
    fn a_web_halves_its_targets_speed() {
        let mut state = TacticalState::open_v11(5, 78, &fleet(1, &[(K::Corvette, "stasis_web", 1)]),
            &fleet(2, &[(K::Raider, "", 1)]), 0, 0.0, Vec2::new(1.0, 0.0));
        let mut webbed = 0;
        for _ in 0..80 {
            unkillable(&mut state);
            let out = state.step(false, [SideMods::default(); 2]);
            let frame = state.step_keyframe(out);
            let raider = state.combatants.iter().find(|c| c.kind == K::Raider).unwrap();
            if frame.webs.as_ref().is_some_and(|w| w.iter().any(|l| l.to == raider.cid)) {
                webbed += 1;
                assert!(raider.vel.length() <= 100.0 * WEB_SPEED + 45.0, "slowing toward half speed: {}", raider.vel.length());
            }
        }
        assert!(webbed > 10, "the corvette webbed the interceptor ({webbed} rounds)");
    }

    #[test]
    fn destroyer_batteries_fire_four_shots_and_track_small_hulls() {
        assert!(fires_batteries(K::Destroyer, &Loadout::default(), DamageType::Beam));
        assert!(!fires_batteries(K::Destroyer, &Loadout::from_key("mass_driver"), DamageType::Driver));
        assert!(!fires_batteries(K::Cruiser, &Loadout::default(), DamageType::Beam));
        // Against a darting interceptor a battery shot hits twice as often as a beam.
        let beam = super::super::v1::to_hit(DamageType::Beam, 200.0, 100.0, crate::tactical::Math::Soft);
        assert!(battery_to_hit(100.0) > 1.9 * beam, "{} v {beam}", battery_to_hit(100.0));
        assert!(battery_armor(800.0, false, crate::tactical::Math::Soft) == 1.0 && battery_armor(8_000.0, false, crate::tactical::Math::Soft) == 0.5);
        let mut state = TacticalState::open_v11(9, 79, &fleet(1, &[(K::Destroyer, "", 1)]),
            &fleet(2, &[(K::Raider, "", 6)]), 0, 0.0, Vec2::new(1.0, 0.0));
        let mut shots = Vec::new();
        for _ in 0..60 {
            unkillable(&mut state);
            let out = state.step(false, [SideMods::default(); 2]);
            let dd = state.combatants.iter().find(|c| c.kind == K::Destroyer).unwrap().cid;
            let n = out.gunfire.iter().filter(|g| g.from == dd).count();
            if n > 0 { shots.push(n); }
        }
        assert!(!shots.is_empty() && shots.iter().all(|&n| n == BATTERY_SHOTS), "{shots:?}");
        assert_eq!(BATTERY_SHOTS, 4);
    }

    #[test]
    fn small_hull_guns_are_light_and_torpedoes_are_not() {
        let small = Some((800.0, false));
        let line = Some((2_000.0, false));
        assert_eq!(gun_weight(K::Raider, DamageType::Beam, small, 3.0, crate::tactical::Math::Soft), 3.0, "small hulls fight each other in full");
        assert_eq!(gun_weight(K::Raider, DamageType::Beam, line, 3.0, crate::tactical::Math::Soft), 3.0 * INTERCEPTOR_GUNS);
        assert_eq!(gun_weight(K::Raider, DamageType::Driver, line, 3.0, crate::tactical::Math::Soft), 3.0 * INTERCEPTOR_GUNS);
        assert_eq!(gun_weight(K::Corvette, DamageType::Beam, line, 1.0, crate::tactical::Math::Soft), CORVETTE_GUNS);
        assert_eq!(gun_weight(K::Raider, DamageType::Beam, Some((8_000.0, false)), 3.0, crate::tactical::Math::Soft), 3.0 * INTERCEPTOR_GUNS * 0.5,
            "heavy armour shrugs off half of what is left");
        assert_eq!(gun_weight(K::Raider, DamageType::Torpedo, line, 3.0, crate::tactical::Math::Soft), 3.0);
        assert_eq!(gun_weight(K::Raider, DamageType::Beam, None, 3.0, crate::tactical::Math::Soft), 3.0, "raiding a freighter is unchanged");
        assert_eq!(gun_weight(K::Cruiser, DamageType::Beam, line, 4.5, crate::tactical::Math::Soft), 4.5);
    }

    #[test]
    fn v11_estimates_run_deterministically() {
        let setup = ProjSetup { a: fleet(1, &[(K::Raider, "", 6), (K::Destroyer, "", 2)]), d: fleet(2, &[(K::Cruiser, "", 3)]),
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
        let first = project_distribution(&setup, 17, 8);
        assert_eq!(first.a_win_pct, project_distribution(&setup, 17, 8).a_win_pct);
        assert_eq!(simulate_engagement(&setup, 3), simulate_engagement(&setup, 3));
    }

    #[test]
    fn holds_and_webs_are_recorded_for_the_viewer() {
        let mut state = TacticalState::open_v11(3, 80, &fleet(1, &[(K::Raider, "stasis_web", 2)]),
            &fleet(2, &[(K::Battleship, "", 1)]), 0, 0.0, Vec2::new(1.0, 0.0));
        state.order_withdraw(1);
        let (mut tackles, mut webs) = (0, 0);
        for _ in 0..120 {
            unkillable(&mut state);
            let out = state.step(false, [SideMods::default(); 2]);
            let frame = state.step_keyframe(out);
            tackles += frame.tackles.expect("v11 frames carry holds").len();
            webs += frame.webs.expect("and webs").len();
        }
        assert!(tackles > 0 && webs > 0, "{tackles} tackles, {webs} webs");
        // Older rules leave the fields out.
        let mut old = TacticalState::open_v10(3, 80, &fleet(1, &[(K::Raider, "", 2)]),
            &fleet(2, &[(K::Battleship, "", 1)]), 0, 0.0, Vec2::new(1.0, 0.0));
        let frame = { let out = old.step(false, [SideMods::default(); 2]); old.step_keyframe(out) };
        assert!(frame.tackles.is_none() && frame.webs.is_none());
    }
}


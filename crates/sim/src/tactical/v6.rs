//! Standing mission profiles. V1–V5 recordings keep their targeting and motion.
use super::{v1::*, v3, v4, Rules};
use crate::{doctrine::{MissionProfile, TargetPriority, ScreeningRole}, module::{Loadout, ModuleKind}, EntityId, Vec2, ship::Ship};
use std::collections::BTreeMap;

impl TacticalState {
    pub fn open_current(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v5(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V5 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V6 { maneuver_seed };
        state
    }

    pub(crate) fn set_missions(&mut self, missions: BTreeMap<EntityId, MissionProfile>) -> bool {
        if !matches!(self.rules, Rules::V6 { .. }) || self.missions == missions { return false; }
        self.missions = missions;
        true
    }

    pub fn withdrawn_mission_fleets(&self) -> Vec<EntityId> {
        self.mission_retreats.iter().copied().filter(|fid| {
            let mut survivors = self.combatants.iter().filter(|c| c.hp > 0.0 && c.origin.is_some_and(|o| o.0 == *fid));
            survivors.clone().next().is_some() && survivors.all(|c| c.pos.length() >= WITHDRAW_EXIT_RADIUS)
        }).collect()
    }

    pub(super) fn mission_side_withdrawn(&self, side: u8) -> bool {
        !self.mission_retreats.is_empty() && self.combatants.iter().any(|c| c.side == side && !c.platform)
            && self.combatants.iter().filter(|c| c.side == side && c.hp > 0.0).all(|c|
                !c.platform && c.origin.is_some_and(|o| self.mission_retreats.contains(&o.0))
                && c.pos.length() >= WITHDRAW_EXIT_RADIUS)
    }
}

pub(super) fn update_retreats(state: &mut TacticalState) {
    if !matches!(state.rules, Rules::V6 { .. }) { return; }
    for c in &state.combatants {
        if c.hp <= 0.0 || c.platform { continue; }
        if let Some((fleet, _)) = c.origin
            && state.missions.get(&fleet).and_then(|m| m.withdrawal.threshold())
                .is_some_and(|limit| c.hp / c.max_hp.max(1e-9) < limit) {
            state.mission_retreats.insert(fleet);
        }
    }
    for c in &mut state.combatants {
        if c.origin.is_some_and(|o| state.mission_retreats.contains(&o.0)) { c.role = Role::Withdraw; }
    }
    // Reserve waves stay outside this engagement, like a whole-side withdrawal.
    for wave in &mut state.waves { wave.retain(|w| !state.mission_retreats.contains(&w.fleet)); }
}

pub(super) fn target_weight(state: &TacticalState, shooter: &Combatant, target: &Combatant) -> f64 {
    let priority = shooter.origin.and_then(|o| state.missions.get(&o.0)).map(|m| m.priority).unwrap_or_default();
    let preferred = match priority {
        TargetPriority::Balanced => false,
        TargetPriority::MissileShips => !target.platform && Loadout::from_key(&target.stack).modules().contains(&ModuleKind::TorpedoRack),
        TargetPriority::Installations => target.platform,
        TargetPriority::Transports => !target.platform && target.kind.attack_weight() == 0.0,
    };
    // Bias rather than an absolute lock: all in-range enemies remain eligible.
    target.max_hp.max(1.0) * if preferred { 6.0 } else { 1.0 }
}

pub(super) fn desired_point(state: &TacticalState, i: usize, seed: u64) -> Vec2 {
    let c = &state.combatants[i];
    if c.origin.is_some_and(|o| state.mission_retreats.contains(&o.0)) {
        return state.bearing * if c.side == 0 { WITHDRAW_EXIT_RADIUS * 1.5 } else { -WITHDRAW_EXIT_RADIUS * 1.5 };
    }
    if !state.withdrawing[c.side as usize] && c.kind.attack_weight() > 0.0
        && c.origin.and_then(|o| state.missions.get(&o.0)).is_some_and(|m| m.screening == ScreeningRole::ProtectTransports)
        && let Some(charge) = state.combatants.iter().filter(|x| x.side == c.side && x.hp > 0.0 && !x.platform && x.kind.attack_weight() == 0.0)
            .min_by(|a, b| c.pos.distance(a.pos).total_cmp(&c.pos.distance(b.pos)).then(a.cid.cmp(&b.cid))) {
        let threat = state.nearest_enemy(i).unwrap_or(charge.pos + state.bearing);
        return charge.pos + (threat - charge.pos).normalized() * (PD_RADIUS * 0.9);
    }
    v4::desired_point(state, i, seed)
}

pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    super::v1::simulate_engagement_with_rules(setup, seed, Rules::V6 { maneuver_seed })
}
pub fn project_distribution(setup: &ProjSetup, seed: u64, k: u32) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, simulate_engagement)
}

/// Estimation receives only profiles in the observer's arrived picture. Unknown
/// enemy instructions remain the baseline; never consult live fleet truth here.
pub fn project_distribution_with_missions(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
        super::v1::simulate_engagement_with_missions(s, seed, Rules::V6 { maneuver_seed }, missions)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ShipKind as H, doctrine::DamageWithdrawal};

    fn scene() -> TacticalState {
        TacticalState::open_current(99, 21,
            &[(EntityId(1), Ship::new(0, H::Raider, Loadout::default())),
              (EntityId(2), Ship::new(0, H::Corvette, Loadout::default())),
              (EntityId(3), Ship::new(0, H::Convoy, Loadout::default()))],
            &[(EntityId(4), Ship::new(0, H::Destroyer, Loadout::new(vec![ModuleKind::TorpedoRack])))],
            1, 0.0, Vec2::new(1.0, 0.0))
    }

    #[test]
    fn target_preferences_bias_only_matching_visible_candidates() {
        let mut s = scene();
        let shooter = s.combatants.iter().find(|c| c.origin.is_some_and(|o| o.0 == EntityId(1))).unwrap().clone();
        let missile = s.combatants.iter().find(|c| c.origin.is_some_and(|o| o.0 == EntityId(4))).unwrap().clone();
        let platform = s.combatants.iter().find(|c| c.platform).unwrap().clone();
        assert_eq!(target_weight(&s, &shooter, &missile), missile.max_hp);
        s.set_missions(BTreeMap::from([(EntityId(1), MissionProfile { priority: TargetPriority::MissileShips, ..Default::default() })]));
        assert_eq!(target_weight(&s, &shooter, &missile), missile.max_hp * 6.0);
        assert_eq!(target_weight(&s, &shooter, &platform), platform.max_hp);
        s.missions.get_mut(&EntityId(1)).unwrap().priority = TargetPriority::Installations;
        assert_eq!(target_weight(&s, &shooter, &platform), platform.max_hp * 6.0);
        assert_eq!(target_weight(&s, &shooter, &missile), missile.max_hp);
    }

    #[test]
    fn screening_seeks_the_transport_without_granting_a_pd_module() {
        let mut s = scene();
        let i = s.combatants.iter().position(|c| c.origin.is_some_and(|o| o.0 == EntityId(2))).unwrap();
        s.set_missions(BTreeMap::from([(EntityId(2), MissionProfile { screening: ScreeningRole::ProtectTransports, ..Default::default() })]));
        let p = desired_point(&s, i, 100);
        let transport = s.combatants.iter().find(|c| c.origin.is_some_and(|o| o.0 == EntityId(3))).unwrap();
        assert!((p.distance(transport.pos) - PD_RADIUS * 0.9).abs() < 1e-7);
        assert!(!Loadout::from_key(&s.combatants[i].stack).has_pd());
    }

    #[test]
    fn one_damaged_fleet_retreats_physically_not_its_entire_side() {
        let mut s = scene();
        let i = s.combatants.iter().position(|c| c.origin.is_some_and(|o| o.0 == EntityId(1))).unwrap();
        s.combatants[i].hp = s.combatants[i].max_hp * 0.45;
        s.set_missions(BTreeMap::from([(EntityId(1), MissionProfile { withdrawal: DamageWithdrawal::Hull50, ..Default::default() })]));
        let before = s.combatants[i].pos;
        s.step(false, [SideMods { damage_mult: 0.0, ..Default::default() }; 2]);
        assert!(s.combatants[i].pos.distance(before) < 200.0, "no teleport to disengage edge");
        assert!(!s.withdrawing[0]);
        assert_eq!(s.mission_retreats, std::collections::BTreeSet::from([EntityId(1)]));
        for _ in 0..500 { s.step(false, [SideMods { damage_mult: 0.0, ..Default::default() }; 2]); }
        assert_eq!(s.withdrawn_mission_fleets(), vec![EntityId(1)]);
        assert!(s.combatants.iter().filter(|c| c.origin.is_some_and(|o| o.0 == EntityId(2))).all(|c| c.pos.length() < ARENA_RADIUS * 1.05));
        let restored: TacticalState = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(restored, s);
    }
}

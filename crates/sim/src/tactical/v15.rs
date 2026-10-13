//! Orbital stations. A system's station fights through its hardpoints: each
//! is a stationary battery with its own hull, attack weight and reach (set by
//! the station's level), firing whatever it is fitted with, beam by default,
//! and wearing its armour like a ship. Battleships and larger are the siege
//! class: their hits on stations and Defense Platforms land 25% harder. Three
//! researches finally do something: Breaching Ordnance (+25% damage to
//! stations and platforms), Platform Netfire (a side's hardpoints screen
//! torpedoes out to 700 su, up from 400) and Fortress Doctrine (a side's
//! hulls can't be tackled while its own station fights). Everything else is
//! v14; a battle without a station, a platform or a capital is fought
//! exactly as under v14.

use super::v1::{Combatant, Distribution, ProjSetup, Role, SimOutcome, TacticalState, TypedCooldowns, PROJECTION_BATTLE_ID, SPAWN_DIST};
use super::{v3, Rules};
use crate::doctrine::MissionProfile;
use crate::ship::{Ship, ShipKind};
use crate::station::MountSpec;
use crate::{EntityId, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A siege-class hull's hits on a station or platform land this much harder.
pub const SIEGE_BONUS: f64 = 1.25;
/// Platform Netfire: a side's hardpoints screen torpedoes out to this.
pub const NETFIRE_RADIUS: f64 = 700.0;

/// The station hardpoint a combatant is (v15).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mount {
    /// Which of the station's hardpoints: its write-back slot.
    pub index: u8,
    pub aw: f64,
    pub reach: f64,
    /// The station's level (for the theater's picture of it).
    pub level: u8,
}

impl TacticalState {
    /// Rules v15, frozen: battles that opened on it replay on it.
    pub(crate) fn open_v15(seed: u64, battle: u64, a: &[(EntityId, Ship)], d: &[(EntityId, Ship)],
        tiers: u32, pool: f64, bearing: Vec2) -> Self {
        let mut state = Self::open_v14(seed, battle, a, d, tiers, pool, bearing);
        let Rules::V14 { maneuver_seed } = state.rules else { unreachable!() };
        state.rules = Rules::V15 { maneuver_seed };
        state
    }

    /// Stand `side`'s station hardpoints at its anchor before the first
    /// recording checkpoint (and before `place_world`, which groups them with
    /// any platforms). Only v15 battles field stations.
    pub fn add_station(&mut self, side: u8, mounts: &[MountSpec]) {
        if mounts.is_empty() || !matches!(self.rules, Rules::V15 { .. } | Rules::V16 { .. }) {
            return;
        }
        assert_eq!(self.step, 0);
        let side = side.min(1);
        let anchor = if side == 1 { Vec2::ZERO } else { self.bearing * SPAWN_DIST };
        let lateral = Vec2::new(-self.bearing.y, self.bearing.x);
        let n = mounts.len() as f64;
        for (k, m) in mounts.iter().enumerate() {
            let cid = self.next_cid;
            self.next_cid += 1;
            let t = k as f64 - (n - 1.0) / 2.0;
            self.combatants.push(Combatant {
                cid,
                side,
                kind: ShipKind::Corvette, // unused: a hardpoint fights by its mount
                stack: m.stack.clone(),
                hp: m.hp.clamp(1.0, m.max_hp),
                max_hp: m.max_hp,
                pos: anchor + lateral * (t * 24.0),
                vel: Vec2::ZERO,
                cooldowns: TypedCooldowns::default(),
                role: Role::Anchor,
                platform: true,
                origin: None,
                mount: Some(Mount { index: m.index, aw: m.aw, reach: m.reach, level: m.level }),
            });
        }
    }

    /// Each of `side`'s hardpoints still standing, as (index, hull fraction).
    /// One that entered the battle and is missing here was knocked out.
    pub fn station_health(&self, side: u8) -> Vec<(u8, f64)> {
        self.combatants
            .iter()
            .filter(|c| c.side == side && c.hp > 0.0)
            .filter_map(|c| c.mount.map(|m| (m.index, (c.hp / c.max_hp.max(1e-9)).clamp(0.0, 1.0))))
            .collect()
    }

    /// `side`'s legacy Defense Platform tiers still standing (station
    /// hardpoints are counted by `station_health`).
    pub fn legacy_platforms_for(&self, side: u8) -> u32 {
        self.combatants.iter().filter(|c| c.platform && c.mount.is_none() && c.side == side && c.hp > 0.0).count() as u32
    }

    /// The damage carried by `side`'s standing legacy platform tiers.
    pub fn legacy_pool_for(&self, side: u8) -> f64 {
        self.combatants
            .iter()
            .filter(|c| c.platform && c.mount.is_none() && c.side == side && c.hp > 0.0)
            .map(|c| (c.max_hp - c.hp).max(0.0))
            .sum()
    }
}

/// The siege class: battleships and larger.
pub(super) fn siege_class(kind: ShipKind) -> bool {
    matches!(kind, ShipKind::Battleship | ShipKind::Dreadnought | ShipKind::Titan)
}

// The v15 projections, archived: estimates are made on the current rules (v16).
#[allow(dead_code)]
fn rules(seed: u64) -> Rules {
    let Rules::V3 { maneuver_seed } = v3::rules(seed, PROJECTION_BATTLE_ID) else { unreachable!() };
    Rules::V15 { maneuver_seed }
}

#[allow(dead_code)]
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
        super::v1::simulate_engagement_full(s, seed, rules(seed), missions, None, &[Vec::new(), Vec::new()])
    })
}

/// The estimate for a fight at a world held by side `home` (0 attacker, 1
/// defender), with each side's station hardpoints as the viewer knows them.
#[allow(dead_code)]
pub fn project_distribution_at_world(setup: &ProjSetup, seed: u64, k: u32,
    missions: &BTreeMap<EntityId, MissionProfile>, home: u8, stations: &[Vec<MountSpec>; 2]) -> Distribution {
    super::v1::project_distribution_using(setup, seed, k, |s, seed| {
        super::v1::simulate_engagement_full(s, seed, rules(seed), missions, Some(home), stations)
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

    fn mounts(level: u32, fits: &[&str]) -> Vec<MountSpec> {
        let mounts: Vec<crate::station::Mount> = fits.iter()
            .map(|f| crate::station::Mount { fit: (*f).into(), ..Default::default() }).collect();
        crate::station::mount_specs(level, &mounts)
    }

    fn open(seed: u64, a: &[(K, &str, u32)], d: &[(K, &str, u32)]) -> TacticalState {
        TacticalState::open_v15(seed, 95, &fleet(1, a), &fleet(2, d), 0, 0.0, Vec2::new(1.0, 0.0))
    }

    /// Probe: each station level alone against fleets worth its cumulative
    /// cost, and the capstone targets. Prints the attacker's win rate over 200
    /// estimates. Run with
    /// `cargo test -p sim --release --lib -- probe_station_ladder --ignored --nocapture`.
    #[test]
    #[ignore]
    fn probe_station_ladder() {
        let value = |kind: K| -> f64 {
            crate::build::recipe_for(crate::build::BuildKind::Ship { ship: kind })
                .costs.iter().map(|(c, n)| crate::market::base_price(*c) * n).sum()
        };
        let level_cost = |l: u32| -> f64 {
            crate::station::LEVELS.iter().take(l as usize)
                .map(|x| x.recipe.costs.iter().map(|(c, n)| crate::market::base_price(*c) * n).sum::<f64>()).sum()
        };
        const DREAD: &str = "mass_driver+mass_driver+mass_driver+mass_driver+mass_driver";
        const TITAN: &str = "mass_driver+mass_driver+mass_driver+mass_driver+mass_driver+mass_driver";
        let m = BTreeMap::new();
        let run = |a: Vec<(K, &str, u32)>, level: u32| -> f64 {
            let setup = ProjSetup { a: fleet(1, &a), d: Vec::new(), platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
            let fits = vec![""; crate::station::level(level).unwrap().hardpoints as usize];
            let st = [Vec::new(), mounts(level, &fits)];
            project_distribution_at_world(&setup, 31, 200, &m, 1, &st).a_win_pct * 100.0
        };
        for level in 1..=5u32 {
            let cost = level_cost(level);
            let mut line = format!("L{level} {:<16} ({:>6.0} cr):", crate::station::level_name(level), cost);
            for (kind, fit) in [(K::Raider, "torpedo_rack"), (K::Destroyer, ""), (K::Cruiser, "mass_driver+mass_driver"),
                (K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver"), (K::Dreadnought, DREAD)] {
                let n = (cost / value(kind)).round().max(1.0) as u32;
                line += &format!("  {n} {kind:?} {:.0}%", run(vec![(kind, fit, n)], level));
            }
            eprintln!("{line}");
        }
        let fortress = level_cost(5);
        eprintln!("Star Fortress {fortress:.0} cr vs: lone Titan {:.0}%; Titan + 3 dreadnoughts ({:.0} cr) {:.0}%; Titan + 5 dreadnoughts ({:.0} cr) {:.0}%",
            run(vec![(K::Titan, TITAN, 1)], 5),
            value(K::Titan) + 3.0 * value(K::Dreadnought), run(vec![(K::Titan, TITAN, 1), (K::Dreadnought, DREAD, 3)], 5),
            value(K::Titan) + 5.0 * value(K::Dreadnought), run(vec![(K::Titan, TITAN, 1), (K::Dreadnought, DREAD, 5)], 5));
    }

    /// No station, no platform, no capital: v14, step for step.
    #[test]
    fn a_fight_without_stations_or_capitals_is_exactly_v14() {
        let a = [(K::Cruiser, "mass_driver+mass_driver", 3), (K::Raider, "torpedo_rack", 6)];
        let d = [(K::Destroyer, "", 4), (K::Corvette, "point_defense_screen", 3)];
        let mut new = open(4, &a, &d);
        let mut old = new.clone();
        let Rules::V15 { maneuver_seed } = new.rules else { unreachable!() };
        old.rules = Rules::V14 { maneuver_seed };
        for step in 0..150 {
            let a = new.step(false, [SideMods::default(); 2]);
            let b = old.step(false, [SideMods::default(); 2]);
            assert_eq!(a.dealt, b.dealt, "step {step}");
            assert_eq!(new.combatants.len(), old.combatants.len(), "step {step}");
        }
    }

    /// A hardpoint fires what it is fitted with, with its level's weight and
    /// reach, and wears its armour; an unfitted one is the stock beam battery.
    #[test]
    fn hardpoints_fight_as_fitted() {
        let mut s = open(5, &[(K::Cruiser, "", 2)], &[]);
        s.add_station(1, &mounts(4, &["mass_driver+mass_driver+whipple_armor", "", "torpedo_rack+torpedo_rack", ""]));
        let hp: Vec<&Combatant> = s.combatants.iter().filter(|c| c.mount.is_some()).collect();
        assert_eq!(hp.len(), 4);
        assert!(hp.iter().all(|c| c.platform && c.side == 1 && c.max_hp == crate::station::LEVELS[3].hull));
        assert_eq!(hp[0].mount.unwrap().aw, crate::station::LEVELS[3].aw);
        assert_eq!(s.station_health(1).len(), 4);
        let mut fired = std::collections::BTreeSet::new();
        for _ in 0..80 {
            let out = s.step(false, [SideMods::default(); 2]);
            for g in out.gunfire.iter().filter(|g| g.side == 1) {
                fired.insert(format!("{:?}", g.weapon));
            }
            if s.torpedoes.iter().any(|t| t.side == 1) { fired.insert("Torpedo".into()); }
        }
        assert!(fired.contains("Driver") && fired.contains("Beam"), "{fired:?}");
    }

    /// Siege-class hulls hit stations harder; Breaching Ordnance stacks on it.
    #[test]
    fn capitals_besiege_stations() {
        let dealt = |attacker: K, breach: f64| -> f64 {
            let mut s = open(6, &[(attacker, "", 1)], &[]);
            s.add_station(1, &mounts(5, &["", "", "", "", "", ""]));
            let mods = [SideMods { vs_defenses: breach, ..Default::default() }, SideMods::default()];
            (0..60).map(|_| s.step(false, mods).dealt[0]).sum()
        };
        let cruiser = dealt(K::Cruiser, 0.0);
        let bb = dealt(K::Battleship, 0.0);
        assert!(cruiser > 0.0 && bb > 0.0);
        let breached = dealt(K::Battleship, 1.25);
        assert!((breached / bb - 1.25).abs() < 0.05, "Breaching Ordnance ×1.25: {}", breached / bb);
    }

    /// Fortress Doctrine: a side whose own station fights can't be tackled.
    #[test]
    fn fortress_doctrine_slips_tackle() {
        let held = |fortress: bool| -> usize {
            let mut s = open(7, &[(K::Raider, "", 8)], &[(K::Battleship, "", 1)]);
            s.add_station(1, &mounts(1, &[""]));
            s.order_withdraw(1);
            let mods = [SideMods::default(), SideMods { fortress, ..Default::default() }];
            (0..60).map(|_| s.step(false, mods).tackles.len()).sum()
        };
        assert!(held(false) > 0, "interceptors hold the battleship");
        assert_eq!(held(true), 0, "not while its station fights");
    }

    /// Netfire widens every hardpoint's torpedo screen.
    #[test]
    fn netfire_widens_the_screen() {
        let intercepted = |netfire: bool| -> usize {
            let mut s = open(8, &[(K::Raider, "torpedo_rack", 12)], &[]);
            s.add_station(1, &mounts(3, &["", "", ""]));
            let mods = [SideMods::default(), SideMods { netfire, ..Default::default() }];
            (0..80).map(|_| s.step(false, mods).torp_ends.iter()
                .filter(|e| e.end == crate::combat::TorpedoEnd::Intercepted).count()).sum()
        };
        assert!(intercepted(true) > intercepted(false));
    }

    /// The estimate fields the stations it's given, deterministically.
    #[test]
    fn the_estimate_fields_stations() {
        let setup = ProjSetup { a: fleet(1, &[(K::Cruiser, "", 4)]), d: fleet(2, &[(K::Destroyer, "", 1)]),
            platform_tiers: 0, raid: false, a_retreat: None, d_retreat: None };
        let none = [Vec::new(), Vec::new()];
        let fort = [Vec::new(), mounts(4, &["", "", "", ""])];
        let m = BTreeMap::new();
        let open = project_distribution_at_world(&setup, 9, 24, &m, 1, &none);
        let held = project_distribution_at_world(&setup, 9, 24, &m, 1, &fort);
        assert!(held.a_win_pct < open.a_win_pct, "{} vs {}", held.a_win_pct, open.a_win_pct);
        assert_eq!(held.a_win_pct, project_distribution_at_world(&setup, 9, 24, &m, 1, &fort).a_win_pct);
    }
}

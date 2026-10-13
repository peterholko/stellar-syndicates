//! Fingerprints of the current rules' fights, pinned: each scenario's every
//! hull position, velocity and hull, every torpedo, every step. Since rules
//! v16 the battle engine computes the same bits in a debug and a release
//! build and on any platform (`Math::Soft`), so these hold in every build. A
//! failure means a fight changed: a platform-math call crept back into the
//! engine (`cargo test --release` shows it), or the rules changed, which
//! belongs in a new rules version (freeze these under its constructor, then
//! pin the new version's).

use std::collections::BTreeMap;

use super::v1::{SideMods, TacticalState};
use super::Rules;
use crate::module::Loadout;
use crate::ship::{Ship, ShipKind as K};
use crate::station::{mount_specs, Mount};
use crate::{EntityId, Vec2};

type Roster = &'static [(K, &'static str, u32)];

struct Scenario {
    name: &'static str,
    seed: u64,
    a: Roster,
    d: Roster,
    tiers: u32,
    crippled: Option<(u8, K, f64)>,
    withdraw: Option<(u8, u64)>,
    world: bool,
    relief: Option<(u64, Roster)>,
    station: Option<(u32, &'static [&'static str])>,
}

const SCENARIOS: &[Scenario] = &[
    Scenario { name: "mixed", seed: 11,
        a: &[(K::Titan, "", 1), (K::Battleship, "", 1), (K::Cruiser, "", 2), (K::Corvette, "point_defense_screen", 2), (K::Raider, "", 6)],
        d: &[(K::Dreadnought, "", 1), (K::Destroyer, "torpedo_rack", 2), (K::Cruiser, "", 2), (K::Raider, "mass_driver", 6), (K::Corvette, "", 2)],
        tiers: 0, crippled: None, withdraw: None, world: false, relief: None, station: None },
    Scenario { name: "swarm", seed: 23, a: &[(K::Raider, "", 12), (K::Raider, "mass_driver", 6)],
        d: &[(K::Battleship, "", 1), (K::Cruiser, "", 2), (K::Corvette, "point_defense_screen", 2)],
        tiers: 0, crippled: None, withdraw: None, world: false, relief: None, station: None },
    Scenario { name: "torpedo", seed: 37, a: &[(K::Destroyer, "torpedo_rack", 4), (K::Raider, "torpedo_rack", 6), (K::Cruiser, "", 2)],
        d: &[(K::Dreadnought, "", 1), (K::Corvette, "point_defense_screen", 4), (K::Cruiser, "", 2), (K::Convoy, "", 1)],
        tiers: 0, crippled: None, withdraw: None, world: false, relief: None, station: None },
    Scenario { name: "collision", seed: 15, a: &[(K::Raider, "", 10), (K::Raider, "torpedo_rack", 4)],
        d: &[(K::Dreadnought, "", 1), (K::Battleship, "", 2), (K::Cruiser, "", 2)],
        tiers: 0, crippled: Some((1, K::Dreadnought, 0.04)), withdraw: None, world: false, relief: None, station: None },
    Scenario { name: "formations", seed: 8, a: &[(K::Battleship, "", 1), (K::Cruiser, "", 4), (K::Destroyer, "", 2)],
        d: &[(K::Convoy, "", 1), (K::Cruiser, "", 2), (K::Corvette, "point_defense_screen", 3)],
        tiers: 0, crippled: None, withdraw: None, world: false, relief: None, station: None },
    Scenario { name: "lockdown", seed: 4, a: &[(K::Raider, "stasis_web", 8), (K::Raider, "torpedo_rack", 10)],
        d: &[(K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver", 2), (K::Cruiser, "", 2)],
        tiers: 0, crippled: None, withdraw: Some((1, 12)), world: false, relief: None, station: None },
    Scenario { name: "escorts", seed: 6, a: &[(K::Raider, "", 18)],
        d: &[(K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver", 1), (K::Destroyer, "stasis_web", 4)],
        tiers: 0, crippled: None, withdraw: None, world: false, relief: None, station: None },
    Scenario { name: "home", seed: 12, a: &[(K::Cruiser, "mass_driver+mass_driver", 3), (K::Raider, "torpedo_rack", 8)],
        d: &[(K::Destroyer, "", 2), (K::Corvette, "point_defense_screen", 2)],
        tiers: 3, crippled: None, withdraw: None, world: true,
        relief: Some((40, &[(K::Cruiser, "", 3), (K::Destroyer, "", 2)])), station: None },
    Scenario { name: "citadel", seed: 21,
        a: &[(K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver", 2), (K::Cruiser, "mass_driver+mass_driver", 3), (K::Corvette, "point_defense_screen", 2)],
        d: &[(K::Destroyer, "torpedo_rack", 2), (K::Corvette, "", 2)],
        tiers: 0, crippled: None, withdraw: None, world: true, relief: None,
        station: Some((4, &["mass_driver+mass_driver+whipple_armor", "torpedo_rack+mass_driver+reflective_plating", "stasis_web+mass_driver+whipple_armor", ""])) },
    // Non-combat hulls: body radii from the size ladder, slow turns.
    Scenario { name: "convoy", seed: 29, a: &[(K::Raider, "torpedo_rack", 6), (K::Raider, "", 4), (K::Destroyer, "", 1)],
        d: &[(K::LargeFreighter, "", 1), (K::HeavyFreighter, "", 1), (K::Colony, "", 1), (K::Transport, "", 1), (K::Corvette, "point_defense_screen", 2)],
        tiers: 0, crippled: None, withdraw: Some((1, 20)), world: false, relief: None, station: None },
];

fn ships(fleet: u64, list: Roster) -> Vec<(EntityId, Ship)> {
    let mut out = Vec::new();
    for (kind, fit, n) in list {
        for _ in 0..*n {
            out.push((EntityId(fleet), Ship::new(out.len() as u32, *kind, Loadout::from_key(fit))));
        }
    }
    out
}

fn fnv(h: &mut u64, bits: u64) {
    *h = (*h ^ bits).wrapping_mul(0x100000001b3);
}

/// Every hull's position, velocity and hull, every torpedo, every step.
fn run(s: &Scenario, version: u32) -> Option<u64> {
    let mut a = ships(1_000, s.a);
    let mut d = ships(2_000, s.d);
    if let Some((side, kind, share)) = s.crippled {
        let fleet = if side == 0 { &mut a } else { &mut d };
        if let Some((_, ship)) = fleet.iter_mut().find(|(_, ship)| ship.kind == kind) { ship.hp *= share; }
    }
    let bearing = Vec2::new(-1.0, 0.0);
    let (seed, id, t) = (s.seed, 900_001, s.tiers);
    let mut tac = match version {
        1 | 2 => TacticalState::open(seed, id, &a, &d, t, 0.0, bearing),
        3 => TacticalState::open_v3(seed, id, &a, &d, t, 0.0, bearing),
        4 => TacticalState::open_v4(seed, id, &a, &d, t, 0.0, bearing),
        5 => TacticalState::open_v5(seed, id, &a, &d, t, 0.0, bearing),
        6 => TacticalState::open_v6(seed, id, &a, &d, t, 0.0, bearing),
        7 => TacticalState::open_v7(seed, id, &a, &d, t, 0.0, bearing),
        8 => TacticalState::open_v8(seed, id, &a, &d, t, 0.0, bearing),
        9 => TacticalState::open_v9(seed, id, &a, &d, t, 0.0, bearing),
        10 => TacticalState::open_v10(seed, id, &a, &d, t, 0.0, bearing),
        11 => TacticalState::open_v11(seed, id, &a, &d, t, 0.0, bearing),
        12 => TacticalState::open_v12(seed, id, &a, &d, t, 0.0, bearing),
        13 => TacticalState::open_v13(seed, id, &a, &d, t, 0.0, bearing),
        14 => TacticalState::open_v14(seed, id, &a, &d, t, 0.0, bearing),
        15 => TacticalState::open_v15(seed, id, &a, &d, t, 0.0, bearing),
        _ => TacticalState::open_current(seed, id, &a, &d, t, 0.0, bearing),
    };
    if version == 2 {
        tac.rules = Rules::V2;
    }
    if version != 1 && version != 2 && tac.rules_version() != version {
        return None; // past the current version
    }
    if let Some((level, fits)) = s.station {
        let mounts: Vec<Mount> = fits.iter().map(|fit| Mount { fit: fit.to_string(), ..Default::default() }).collect();
        tac.add_station(1, &mount_specs(level, &mounts));
    }
    if s.world {
        tac.place_world(1);
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for step in 0..240u64 {
        if let Some((side, at)) = s.withdraw && step == at {
            tac.order_withdraw(side);
        }
        if let Some((at, relief)) = s.relief && step == at {
            d.extend(ships(2_100, relief));
            let arrivals = BTreeMap::from([(EntityId(2_100), Vec2::new(-1.0, 0.0))]);
            tac.sync_recorded([&a, &d], &arrivals);
        }
        let out = tac.step(false, [SideMods::default(); 2]);
        fnv(&mut h, out.dealt[0].to_bits());
        fnv(&mut h, out.dealt[1].to_bits());
        for c in &tac.combatants {
            for v in [c.pos.x, c.pos.y, c.vel.x, c.vel.y, c.hp] {
                fnv(&mut h, v.to_bits());
            }
        }
        for t in &tac.torpedoes {
            fnv(&mut h, t.pos.x.to_bits());
            fnv(&mut h, t.pos.y.to_bits());
        }
        // Keep the rosters in step with the battle, as the world does.
        let hp: BTreeMap<_, _> = tac.hp_writeback().into_iter().map(|(f, s, hp)| ((f, s), hp)).collect();
        for roster in [&mut a, &mut d] {
            roster.retain_mut(|(f, ship)| hp.get(&(*f, ship.id)).map(|&x| ship.hp = x).is_some());
        }
        if tac.alive(0) == 0 || tac.alive(1) == 0 {
            break;
        }
    }
    Some(h)
}

/// The current rules' fingerprint of each scenario.
const PINNED: [(&str, u64); 10] = [
    ("mixed", 0x4a73_95e4_60a6_8698), ("swarm", 0x0286_c755_4006_51c1), ("torpedo", 0xbaf7_9b41_e87f_7305),
    ("collision", 0x6cc7_5225_d5bb_5fd6), ("formations", 0xa5c8_8450_199c_b850), ("lockdown", 0x0e44_37ba_fe3a_ad1f),
    ("escorts", 0xede8_a136_7fce_7b66), ("home", 0xd8ac_d459_ac48_d31a), ("citadel", 0xcd76_6e5d_f6fd_ca41),
    ("convoy", 0x24c1_c633_0d80_ba07),
];

#[test]
fn current_rules_fight_the_same_in_every_build() {
    let got: Vec<(&str, u64)> = SCENARIOS.iter().map(|s| (s.name, run(s, CURRENT).expect("current rules"))).collect();
    assert_eq!(got, PINNED, "{}", got.iter().map(|(n, h)| format!("(\"{n}\", {h:#018x})")).collect::<Vec<_>>().join(", "));
}

/// The rules version `open_current` opens.
const CURRENT: u32 = 16;

/// Each frozen rules version over the same scenarios (debug and release
/// builds differ for v1–v15; compare like with like).
#[test]
#[ignore]
fn print_rules_fingerprints() {
    for s in SCENARIOS {
        let mut line = format!("{:<10}", s.name);
        for version in 1..=CURRENT {
            if let Some(h) = run(s, version) {
                line += &format!(" v{version}:{:08x}", h >> 32);
            }
        }
        eprintln!("{line}");
    }
}

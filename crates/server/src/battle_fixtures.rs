//! Real engine fights for the battle viewer's test rigs (`__ss.theaterDemo`
//! in the desktop game and client/scripts/battle-model-fixture.html). Each is
//! a seeded fight on the current battle rules, recorded the way live battles
//! are and shaped by the same participant view the client receives, so what
//! you test is what players see. After changing combat, regenerate them:
//!   BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
//! Without the variable, the test fails when the committed files are stale.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use sim::combat::{BattleRecord, LoadoutMap, SideRecord};
use sim::event::RaidOutcome;
use sim::module::Loadout;
use sim::ship::Ship;
use sim::station::{mount_specs, Mount};
use sim::tactical::{SideMods, TacticalState};
use sim::{EntityId, PlayerId, ShipKind, Vec2};

use crate::view::battle_record_views_named;

struct Scenario {
    name: &'static str,
    title: &'static str,
    seed: u64,
    attackers: &'static [(ShipKind, &'static str, u32)],
    defenders: &'static [(ShipKind, &'static str, u32)],
    /// Hulls that start the fight already crippled: (side, kind, hull share),
    /// applied to that side's first hull of that kind.
    crippled: &'static [(u8, ShipKind, f64)],
    /// A side ordered to withdraw from this step on: (side, step).
    withdraw: Option<(u8, u64)>,
    /// Defense Platform tiers on the defending side.
    tiers: u32,
    /// Fought at the defenders' world (rules v14 home ground).
    world: bool,
    /// §orbital-defense: the defenders' station, as (level, each hardpoint's fit).
    station: Option<(u32, &'static [&'static str])>,
    /// Defender relief joining at a step, from a direction (x, y) out of the
    /// battle: it enters the arena on that side.
    relief: Option<(u64, &'static [(ShipKind, &'static str, u32)], (f64, f64))>,
}

use ShipKind as K;
const SCENARIOS: &[Scenario] = &[
    Scenario { name: "mixed", title: "Mixed fleets", seed: 11,
        attackers: &[(K::Titan, "", 1), (K::Battleship, "", 1), (K::Cruiser, "", 2),
            (K::Corvette, "point_defense_screen", 2), (K::Raider, "", 6)],
        defenders: &[(K::Dreadnought, "", 1), (K::Destroyer, "torpedo_rack", 2), (K::Cruiser, "", 2),
            (K::Raider, "mass_driver", 6), (K::Corvette, "", 2)], crippled: &[], withdraw: None, tiers: 0, world: false, station: None, relief: None },
    Scenario { name: "swarm", title: "Interceptor swarm vs capitals", seed: 23,
        attackers: &[(K::Raider, "", 12), (K::Raider, "mass_driver", 6)],
        defenders: &[(K::Battleship, "", 1), (K::Cruiser, "", 2), (K::Corvette, "point_defense_screen", 2)], crippled: &[], withdraw: None, tiers: 0, world: false, station: None, relief: None },
    Scenario { name: "torpedo", title: "Torpedo strike vs a screen", seed: 37,
        attackers: &[(K::Destroyer, "torpedo_rack", 4), (K::Raider, "torpedo_rack", 6), (K::Cruiser, "", 2)],
        defenders: &[(K::Dreadnought, "", 1), (K::Corvette, "point_defense_screen", 4), (K::Cruiser, "", 2),
            (K::Convoy, "", 1)], crippled: &[], withdraw: None, tiers: 0, world: false, station: None, relief: None },
    Scenario { name: "titan", title: "A titan's last stand", seed: 5,
        attackers: &[(K::Titan, "", 1), (K::Corvette, "", 2)],
        defenders: &[(K::Cruiser, "", 4), (K::Destroyer, "", 4), (K::Raider, "", 8)], crippled: &[], withdraw: None, tiers: 0, world: false, station: None, relief: None },
    // A crippled dreadnought dies early and leaves its wreck between the
    // fleets: battleships bump it as it breaks up, interceptors clip it at
    // speed, and torpedoes detonate on it (cover).
    Scenario { name: "collision", title: "Collisions in a wreck field", seed: 15,
        attackers: &[(K::Raider, "", 10), (K::Raider, "torpedo_rack", 4)],
        defenders: &[(K::Dreadnought, "", 1), (K::Battleship, "", 2), (K::Cruiser, "", 2)],
        crippled: &[(1, K::Dreadnought, 0.04)], withdraw: None, tiers: 0, world: false, station: None, relief: None },
    // Automatic formations: the warships form a battle line; the convoy's
    // escorts form a ring with their point defense toward the line.
    Scenario { name: "formations", title: "A battle line against an escort ring", seed: 8,
        attackers: &[(K::Battleship, "", 1), (K::Cruiser, "", 4), (K::Destroyer, "", 2)],
        defenders: &[(K::Convoy, "", 1), (K::Cruiser, "", 2), (K::Corvette, "point_defense_screen", 3)],
        crippled: &[], withdraw: None, tiers: 0, world: false, station: None, relief: None },
    // Rules v11: the battle line tries to leave at round 12; webbing
    // interceptors hold it at the edge of the fight and torpedo interceptors
    // sink it there. (v12's separate driver shots needed sixteen torpedo
    // boats; since v13 the battleships can't track the tacklers.)
    Scenario { name: "lockdown", title: "Interceptors hold a retreating battle line", seed: 4,
        attackers: &[(K::Raider, "stasis_web", 8), (K::Raider, "torpedo_rack", 10)],
        defenders: &[(K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver", 2), (K::Cruiser, "", 2)],
        crippled: &[], withdraw: Some((1, 12)), tiers: 0, world: false, station: None, relief: None },
    // Rules v11: destroyer escorts' light batteries and webs against a swarm.
    Scenario { name: "escorts", title: "Destroyer escorts against an interceptor swarm", seed: 6,
        attackers: &[(K::Raider, "", 18)],
        defenders: &[(K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver", 1), (K::Destroyer, "stasis_web", 4)],
        crippled: &[], withdraw: None, tiers: 0, world: false, station: None, relief: None },
    // Rules v14: a blockade contested at the defenders' world. The world
    // stands behind them with their platforms on its face; at round 40 a
    // relief squadron arrives from beyond the attackers and takes them from
    // behind.
    Scenario { name: "home", title: "Relief strikes a blockade at the world", seed: 12,
        attackers: &[(K::Cruiser, "mass_driver+mass_driver", 3), (K::Raider, "torpedo_rack", 8)],
        defenders: &[(K::Destroyer, "", 2), (K::Corvette, "point_defense_screen", 2)],
        crippled: &[], withdraw: None, tiers: 3, world: true, station: None,
        relief: Some((40, &[(K::Cruiser, "", 3), (K::Destroyer, "", 2)], (-1.0, 0.0))) },
    // Rules v15: a battle line besieges a Citadel at its world. The station's
    // four hardpoints fire as fitted from orbit with a destroyer screen; the
    // battleships hit them ×1.25 (siege class), the cruisers don't.
    Scenario { name: "citadel", title: "A battle line besieges a Citadel", seed: 21,
        attackers: &[(K::Battleship, "mass_driver+mass_driver+mass_driver+mass_driver", 2), (K::Cruiser, "mass_driver+mass_driver", 3),
            (K::Corvette, "point_defense_screen", 2)],
        defenders: &[(K::Destroyer, "torpedo_rack", 2), (K::Corvette, "", 2)],
        crippled: &[], withdraw: None, tiers: 0, world: true,
        station: Some((4, &["mass_driver+mass_driver+whipple_armor", "torpedo_rack+mass_driver+reflective_plating",
            "stasis_web+mass_driver+whipple_armor", ""])),
        relief: None },
];

/// Ticks between engine steps (the playtest cadence) and the longest fight kept.
const STEP_TICKS: u64 = 15;
const MAX_STEPS: u64 = 240;

/// One side's hulls, as one fleet (so it can hold a formation).
fn ships(fleet: u64, list: &[(ShipKind, &str, u32)]) -> Vec<(EntityId, Ship)> {
    let mut out = Vec::new();
    for (kind, fit, n) in list {
        for _ in 0..*n {
            out.push((EntityId(fleet), Ship::new(out.len() as u32, *kind, Loadout::from_key(fit))));
        }
    }
    out
}

fn side_record(corp: PlayerId, list: &[(EntityId, Ship)]) -> SideRecord {
    let mut initial = BTreeMap::new();
    let mut initial_loadouts: LoadoutMap = BTreeMap::new();
    for (_, ship) in list {
        *initial.entry(ship.kind).or_insert(0) += 1;
        let key = ship.loadout.key();
        if !key.is_empty() {
            *initial_loadouts.entry(ship.kind).or_default().entry(key).or_insert(0) += 1;
        }
    }
    SideRecord { corp, initial, initial_loadouts, posture: Default::default(), platform_tiers: 0 }
}

fn survivors(state: &TacticalState) -> [BTreeMap<ShipKind, u32>; 2] {
    let mut counts = [BTreeMap::new(), BTreeMap::new()];
    for c in state.combatants.iter().filter(|c| c.hp > 0.0 && !c.platform) {
        *counts[c.side as usize].entry(c.kind).or_insert(0) += 1;
    }
    counts
}

/// Run one scenario on the current rules, recording every engine step.
fn record(index: usize, s: &Scenario) -> BattleRecord {
    let mut a = ships(1_000, s.attackers);
    let mut d = ships(2_000, s.defenders);
    for &(side, kind, share) in s.crippled {
        let fleet = if side == 0 { &mut a } else { &mut d };
        if let Some((_, ship)) = fleet.iter_mut().find(|(_, ship)| ship.kind == kind) { ship.hp *= share; }
    }
    let id = EntityId(900_001 + index as u64);
    // Attackers close from the viewer's left.
    let mut tac = TacticalState::open_current(s.seed, id.0, &a, &d, s.tiers, 0.0, Vec2::new(-1.0, 0.0));
    if let Some((level, fits)) = s.station {
        let mounts: Vec<Mount> = fits.iter().map(|fit| Mount { fit: fit.to_string(), ..Default::default() }).collect();
        tac.add_station(1, &mount_specs(level, &mounts));
    }
    if s.world {
        tac.place_world(1);
    }
    let mut rec = BattleRecord::open(id, Vec2::ZERO, None, false, 0,
        [side_record(PlayerId(1), &a), side_record(PlayerId(2), &d)]);
    let mut losses = [BTreeMap::new(), BTreeMap::new()];
    let mut tick = 0;
    for step in 0..MAX_STEPS {
        tick += STEP_TICKS;
        if let Some((side, at)) = s.withdraw
            && step == at {
            rec.withdraw_tactical(tick, &mut tac, side);
        }
        if let Some((at, relief, (x, y))) = s.relief
            && step == at {
            d.extend(ships(2_100, relief));
            let arrivals = BTreeMap::from([(EntityId(2_100), Vec2::new(x, y).normalized())]);
            rec.sync_tactical(tick, &mut tac, [&a, &d], &arrivals);
        }
        let out = rec.step_tactical(tick, &mut tac, false, [SideMods::default(); 2]);
        rec.accumulate(out.dealt[0], out.dealt[1], &out.losses[0], &out.losses[1]);
        for (side, lost) in out.losses.iter().enumerate() {
            for (kind, n) in &lost.per_kind { *losses[side].entry(*kind).or_insert(0) += n; }
        }
        rec.flush_step(tick, tac.step_keyframe(out), survivors(&tac));
        // A standing station holds the field for its side.
        if tac.alive(0) == 0 || (tac.alive(1) == 0 && tac.station_health(1).is_empty()) { break; }
    }
    let outcome = match (tac.alive(0) > 0, tac.alive(1) > 0 || !tac.station_health(1).is_empty()) {
        (true, false) => RaidOutcome::TargetDestroyed,
        (false, true) => RaidOutcome::AttackerDestroyed,
        (false, false) => RaidOutcome::BothDestroyed,
        (true, true) => RaidOutcome::BothSurvive,
    };
    rec.finalize(tick, outcome, losses, survivors(&tac));
    rec
}

#[derive(Serialize)]
struct IndexEntry { name: &'static str, title: &'static str, file: String, rounds: usize }

/// Every fixture file, as the attacker (corp 1) sees its own fight.
fn fixtures() -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut index = Vec::new();
    for (i, s) in SCENARIOS.iter().enumerate() {
        let rec = record(i, s);
        let ended = rec.ended_tick.unwrap_or(0) as f64 * sim::config::DT;
        let records = BTreeMap::from([(rec.id, rec)]);
        let names = |corp: PlayerId| Some(if corp == PlayerId(1) { "Aurora" } else { "Vesper" }.to_string());
        // Watched from the battle itself: every round's light has arrived.
        let views = battle_record_views_named(&records, PlayerId(1), Vec2::ZERO, 1.0, ended + 1.0, &[], &names);
        assert_eq!(views.len(), 1, "{}: the participant sees its fight", s.name);
        let view = &views[0];
        assert!(view.rounds.len() > 4 && view.rounds.iter().filter(|r| r.frame.is_some()).count() > 4,
            "{}: a real, framed fight", s.name);
        let file = format!("{}.json", s.name);
        index.push(IndexEntry { name: s.name, title: s.title, file: file.clone(), rounds: view.rounds.len() });
        files.push((file, serde_json::to_string(view).unwrap()));
    }
    files.push(("index.json".to_string(), serde_json::to_string_pretty(&index).unwrap()));
    files
}

#[test]
fn battle_fixtures_are_real_current_rules_fights() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../client/public/dev/battles");
    let files = fixtures();
    if std::env::var_os("BATTLE_FIXTURES_WRITE").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in &files {
            std::fs::write(dir.join(name), text).unwrap();
        }
        return;
    }
    for (name, text) in &files {
        let current = std::fs::read_to_string(dir.join(name)).unwrap_or_default();
        assert!(&current == text,
            "client/public/dev/battles/{name} is stale: BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures");
    }
}

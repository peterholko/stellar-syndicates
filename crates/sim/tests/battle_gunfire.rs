use sim::{EntityId, Loadout, ShipKind, Vec2};
use sim::ship::Ship;
use sim::tactical::{SideMods, StepOutcome, TacticalState};

fn fleet(id: u64) -> Vec<(EntityId, Ship)> {
    vec![(EntityId(id), Ship::new(0, ShipKind::Raider, Loadout::default()))]
}

#[test]
fn gunfire_matches_damage_and_respects_cooldown() {
    let mut saw_miss = false;
    let mut saw_hit = false;
    for seed in 0..32 {
        let mut state = TacticalState::open_current(seed, 99, &fleet(1), &fleet(2), 0, 0.0, Vec2::new(1.0, 0.0));
        state.combatants[0].pos = Vec2::new(-120.0, 0.0);
        state.combatants[1].pos = Vec2::new(120.0, 0.0);
        let out = state.step(false, [SideMods::default(); 2]);
        assert_eq!(out.gunfire.len(), 2, "one actual attempt per ready gun");
        for side in 0..2 {
            let damage: f64 = out.gunfire.iter().filter(|s| s.side as usize == side).map(|s| s.damage as f64).sum();
            assert!((damage - out.dealt[side]).abs() < 1e-4);
        }
        saw_miss |= out.gunfire.iter().any(|s| s.damage == 0.0);
        saw_hit |= out.gunfire.iter().any(|s| s.damage > 0.0);
        let frame = state.step_keyframe(out);
        assert!(frame.ships.iter().all(|s| s.cid.is_some()));
        assert!(frame.gunfire.as_ref().unwrap().iter().all(|s| s.from != s.to));
        let cooldown = state.step(false, [SideMods::default(); 2]);
        assert_eq!(cooldown.dealt, [0.0, 0.0]);
        assert_eq!(state.step_keyframe(cooldown).gunfire, Some(Vec::new()));
    }
    assert!(saw_miss && saw_hit, "both accuracy outcomes must have evidence");
}

#[test]
fn tutorial_duel_shows_resolved_fire_not_one_hit_per_round() {
    let mut pirate = fleet(1);
    // Historical 50%-hull fixture, not today's full-health spawn rule. Keep
    // the reported 43-step battle reproducible when new tutorial tuning changes.
    pirate[0].1.hp *= 0.5;
    // Frozen v1 reproduction of the long-but-low-damage tutorial matchup.
    let mut state = TacticalState::open(47, 99, &pirate, &fleet(2), 0, 0.0, Vec2::new(1.0, 0.0));
    let mods = [SideMods { damage_mult: sim::founding::PRIVATEER_DAMAGE_MULT, ..SideMods::default() }, SideMods::default()];
    let mut hits = [0; 2];
    let mut attempts = 0;
    let mut rounds = 0;
    while state.alive(0) > 0 && state.alive(1) > 0 && rounds < 100 {
        let out = state.step(false, mods);
        for shot in &out.gunfire {
            attempts += 1;
            if shot.damage > 0.0 { hits[shot.side as usize] += 1; }
        }
        rounds += 1;
    }
    assert_eq!(rounds, 43);
    assert_eq!(hits, [2, 2]);
    assert!(attempts > hits.iter().sum::<i32>());
    eprintln!("tutorial duel: {rounds} steps, {attempts} shots, {} damaging hits, {:.2}% Interceptor damage", hits.iter().sum::<i32>(), 100.0 * (1.0 - state.side_hp(1, false) / ShipKind::Raider.hull_mass()));
}

#[test]
fn gunfire_is_bounded_and_legacy_frames_still_load() {
    let state = TacticalState::open_current(0, 99, &fleet(1), &fleet(2), 0, 0.0, Vec2::new(1.0, 0.0));
    let mut out = StepOutcome::default();
    for side in 0..2 {
        for _ in 0..300 {
            out.gunfire.push(sim::combat::KfGunfire { side, from: 1, to: 2, weapon: sim::DamageType::Beam, damage: 1.0 });
        }
    }
    let frame = state.step_keyframe(out);
    let shots = frame.gunfire.unwrap();
    for side in 0..2 { assert_eq!(shots.iter().filter(|s| s.side == side).count(), sim::combat::KEYFRAME_GUNFIRE_CAP_PER_SIDE); }
    let legacy: sim::combat::Keyframe = serde_json::from_str(r#"{"ships":[{"side":0,"kind":"raider","x":0,"y":0,"hp":1}],"torpedoes":[],"deaths":[]}"#).unwrap();
    assert!(legacy.gunfire.is_none());
    assert!(legacy.ships[0].cid.is_none());
}

#[test]
fn no_gunfire_is_invented_out_of_range_or_from_civilians() {
    for kind in [ShipKind::Raider, ShipKind::Convoy] {
        let a = vec![(EntityId(1), Ship::new(0, kind, Loadout::default()))];
        let b = vec![(EntityId(2), Ship::new(0, kind, Loadout::default()))];
        let mut state = TacticalState::open_current(0, 99, &a, &b, 0, 0.0, Vec2::new(1.0, 0.0));
        state.combatants[0].pos = Vec2::ZERO;
        state.combatants[1].pos = Vec2::new(if kind == ShipKind::Raider { 10_000.0 } else { 100.0 }, 0.0);
        let out = state.step(false, [SideMods::default(); 2]);
        assert!(out.gunfire.is_empty());
    }
}

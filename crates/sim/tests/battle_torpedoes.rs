//! Torpedo evidence in battle keyframes: every torpedo in flight with a stable
//! id, and how each one ended. Observed, never re-rolled.
use std::collections::BTreeMap;

use sim::combat::{Keyframe, TorpedoEnd, KEYFRAME_TORPEDO_CAP_PER_SIDE};
use sim::ship::Ship;
use sim::tactical::{SideMods, TacticalState, PD_RADIUS_HEAVY, TORP_HIT_DIST, TORP_SPEED};
use sim::{EntityId, Loadout, ShipKind, Vec2};

fn side(base: u64, list: &[(ShipKind, &str, u64)]) -> Vec<(EntityId, Ship)> {
    let mut out = Vec::new();
    let mut id = base;
    for (kind, fit, n) in list {
        for _ in 0..*n {
            out.push((EntityId(id), Ship::new(0, *kind, Loadout::from_key(fit))));
            id += 1;
        }
    }
    out
}

/// A torpedo strike on a point-defense screen, recorded step by step:
/// (frame, damage dealt per side, gunfire damage per side, live cids).
fn strike(seed: u64) -> Vec<(Keyframe, [f64; 2], [f64; 2], Vec<u32>)> {
    let a = side(1, &[(ShipKind::Raider, "torpedo_rack", 6), (ShipKind::Destroyer, "torpedo_rack", 3)]);
    let d = side(50, &[(ShipKind::Corvette, "point_defense_screen", 3), (ShipKind::Cruiser, "", 2), (ShipKind::Convoy, "", 1)]);
    let mut state = TacticalState::open_current(seed, 77, &a, &d, 0, 0.0, Vec2::new(-1.0, 0.0));
    let mut frames = Vec::new();
    for _ in 0..90 {
        if state.alive(0) == 0 || state.alive(1) == 0 { break; }
        let out = state.step(false, [SideMods::default(); 2]);
        let dealt = out.dealt;
        let mut guns = [0.0; 2];
        for shot in &out.gunfire { guns[shot.side as usize] += f64::from(shot.damage); }
        let live = state.combatants.iter().filter(|c| c.hp > 0.0).map(|c| c.cid).collect();
        frames.push((state.step_keyframe(out), dealt, guns, live));
    }
    frames
}

#[test]
fn every_torpedo_flies_with_a_stable_id_at_torpedo_speed() {
    let mut flights = 0;
    for seed in 0..6 {
        let frames = strike(seed);
        for pair in frames.windows(2) {
            let (before, after) = (&pair[0].0, &pair[1].0);
            let was: BTreeMap<u32, Vec2> = before.torps.as_ref().unwrap().iter()
                .map(|t| (t.id, Vec2::new(f64::from(t.x), f64::from(t.y)))).collect();
            for t in after.torps.as_ref().unwrap() {
                if let Some(p) = was.get(&t.id) {
                    let moved = (Vec2::new(f64::from(t.x), f64::from(t.y)) - *p).length();
                    assert!(moved <= TORP_SPEED + 1e-3, "a torpedo never outruns itself ({moved})");
                    flights += 1;
                }
            }
            let ids: Vec<u32> = after.torps.as_ref().unwrap().iter().map(|t| t.id).collect();
            let mut unique = ids.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), ids.len(), "ids are unique in flight");
        }
        for (frame, ..) in &frames {
            for s in 0..2u8 {
                let shown = frame.torps.as_ref().unwrap().iter().filter(|t| t.side == s).count() as u32;
                let total = frame.torpedoes.iter().find(|salvo| salvo.side == s).map_or(0, |salvo| salvo.n);
                assert_eq!(shown, total.min(KEYFRAME_TORPEDO_CAP_PER_SIDE as u32), "the summary and the list agree");
            }
        }
    }
    assert!(flights > 100, "the strike has torpedoes in flight ({flights})");
}

#[test]
fn each_torpedo_that_vanishes_has_one_recorded_end() {
    let mut seen = BTreeMap::<&str, u32>::new();
    for seed in 0..6 {
        let frames = strike(seed);
        for pair in frames.windows(2) {
            let (before, after) = (&pair[0].0, &pair[1].0);
            let (dealt, guns, live) = (pair[1].1, pair[1].2, &pair[1].3);
            let still: Vec<u32> = after.torps.as_ref().unwrap().iter().map(|t| t.id).collect();
            let ends = after.torp_ends.as_ref().unwrap();
            for t in before.torps.as_ref().unwrap() {
                if still.contains(&t.id) { continue; }
                let matching: Vec<_> = ends.iter().filter(|e| e.id == t.id).collect();
                assert_eq!(matching.len(), 1, "a vanished torpedo ended exactly once");
                let e = matching[0];
                assert_eq!((e.side, e.to), (t.side, t.to));
                let target = after.ships.iter().find(|s| s.cid == Some(e.to));
                match e.end {
                    TorpedoEnd::Hit | TorpedoEnd::Miss => {
                        if let Some(target) = target {
                            let gap = (Vec2::new(f64::from(e.x - target.x), f64::from(e.y - target.y))).length();
                            assert!(gap <= TORP_HIT_DIST + 1e-3, "it reached its target ({gap})");
                        }
                        assert_eq!(e.damage > 0.0, e.end == TorpedoEnd::Hit, "only hits deal damage");
                    }
                    TorpedoEnd::Intercepted => {
                        let by = e.by.expect("an interception names the shooter");
                        let pd = after.ships.iter().find(|s| s.cid == Some(by)).expect("the shooter is on the field");
                        assert_ne!(pd.side, e.side);
                        let gap = (Vec2::new(f64::from(e.x - pd.x), f64::from(e.y - pd.y))).length();
                        assert!(gap <= PD_RADIUS_HEAVY * 2.0 + TORP_SPEED, "shot down near its interceptor ({gap})");
                    }
                    TorpedoEnd::Dry => assert!(!live.contains(&e.to), "only a torpedo whose target is gone runs dry"),
                    TorpedoEnd::Wreck => {
                        let wrecks = after.wrecks.as_ref().expect("a frame with wreckage records it");
                        let edge = wrecks.iter().map(|w| ((w.x - e.x).hypot(w.y - e.y) - w.r).abs()).fold(f32::INFINITY, f32::min);
                        assert!(edge < 0.5, "it struck a wreck's edge ({edge})");
                    }
                }
                *seen.entry(match e.end {
                    TorpedoEnd::Hit => "hit", TorpedoEnd::Miss => "miss",
                    TorpedoEnd::Intercepted => "intercepted", TorpedoEnd::Dry => "dry", TorpedoEnd::Wreck => "wreck",
                }).or_default() += 1;
            }
            // Torpedo hits are the rest of the side's damage after gunfire.
            for s in 0..2 {
                let torp: f64 = ends.iter().filter(|e| usize::from(e.side) == s).map(|e| f64::from(e.damage)).sum();
                assert!((dealt[s] - guns[s] - torp).abs() < 1e-2 * dealt[s].max(1.0), "hits account for the torpedo damage");
            }
        }
    }
    assert!(seen.get("hit").is_some_and(|n| *n > 5) && seen.get("intercepted").is_some_and(|n| *n > 5),
        "a strike on a screen both lands hits and loses torpedoes to point defense: {seen:?}");
}

#[test]
fn legacy_frames_without_torpedo_tracks_still_load() {
    let old = r#"{"ships":[],"torpedoes":[{"side":0,"x":1.0,"y":2.0,"n":3}],"deaths":[]}"#;
    let frame: Keyframe = serde_json::from_str(old).unwrap();
    assert_eq!(frame.torps, None);
    assert_eq!(frame.torp_ends, None);
    let new = serde_json::to_string(&strike(1)[5].0).unwrap();
    assert!(new.contains("\"torps\":") && new.contains("\"torp_ends\":"));
}

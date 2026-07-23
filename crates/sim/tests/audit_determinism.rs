use sim::command::Command;
use sim::config::SimConfig;
use sim::ids::PlayerId;
use sim::world::World;

fn build(seed: u64) -> World {
    let mut cfg = SimConfig::for_players(seed, 4);
    cfg.battle_target_secs = 20.0;
    let mut w = World::new(cfg);
    let a = PlayerId(1);
    let b = PlayerId(2);
    w.step(&[
        Command::AddPlayer { id: a, name: "Acme".into() },
        Command::AddPlayer { id: b, name: "Beta".into() },
    ]);
    w
}

fn drive(w: &mut World, ticks: u64) {
    use sim::cargo::Commodity::*;
    let a = PlayerId(1);
    let b = PlayerId(2);
    for t in 0..ticks {
        let mut cmds = Vec::new();
        if t % 37 == 0 {
            cmds.push(Command::MarketBuy { player_id: a, commodity: Fuel, units: 20, ship_to: None });
            cmds.push(Command::MarketBuy { player_id: b, commodity: Alloys, units: 15, ship_to: None });
        }
        if t % 101 == 5 {
            let sys = w.players[&a].home_system.unwrap();
            cmds.push(Command::BookFreightOut { player_id: a, system: sys, commodity: Fuel, units: 10 });
        }
        if t % 149 == 7 {
            let sys = w.players[&b].home_system.unwrap();
            cmds.push(Command::BookFreightIn { player_id: b, system: sys, commodity: MetallicOre, units: 12, sell_on_arrival: true });
        }
        w.step(&cmds);
    }
}

#[test]
fn two_runs_of_the_same_seed_agree_byte_for_byte() {
    let mut w1 = build(4242);
    let mut w2 = build(4242);
    drive(&mut w1, 4000);
    drive(&mut w2, 4000);
    let j1 = serde_json::to_string(&w1).unwrap();
    let j2 = serde_json::to_string(&w2).unwrap();
    assert_eq!(j1.len(), j2.len(), "world size diverged");
    assert!(j1 == j2, "two runs of the same seed diverged");
    // sanity: the freight machinery actually ran
    eprintln!("runs={} queue={} shipid-present={}", w1.freight_runs.len(), w1.freight_queue.len(), j1.contains("next_shipment_id"));
}

#[test]
fn a_midflight_snapshot_round_trips_and_keeps_stepping_identically() {
    let mut w = build(99);
    drive(&mut w, 2500);
    let json = serde_json::to_string(&w).unwrap();
    let mut restored: World = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&restored).unwrap(), json, "snapshot round-trip is not stable");
    let mut cont = w.clone();
    drive(&mut cont, 1500);
    drive(&mut restored, 1500);
    assert_eq!(
        serde_json::to_string(&cont).unwrap(),
        serde_json::to_string(&restored).unwrap(),
        "a restored snapshot diverged from the live world"
    );
}

/// Strip EVERY field the feature added, as a pre-feature snapshot would lack them,
/// and assert the world still loads.
#[test]
fn a_pre_feature_snapshot_loads() {
    let mut w = build(7);
    drive(&mut w, 1200);
    let mut v: serde_json::Value = serde_json::to_value(&w).unwrap();
    let obj = v.as_object_mut().unwrap();
    for k in ["freight_queue", "freight_runs", "next_shipment_id", "pending_citations", "expeditions", "next_expedition_at"] {
        obj.remove(k);
    }
    for (_p, c) in obj.get_mut("players").unwrap().as_object_mut().unwrap() {
        let c = c.as_object_mut().unwrap();
        c.remove("warehouse");
        c.remove("tca_standing");
        if let Some(so) = c.get_mut("standing_orders").and_then(|x| x.as_array_mut()) {
            for o in so { o.as_object_mut().unwrap().remove("sell_on_arrival"); }
        }
    }
    for (_f, f) in obj.get_mut("fleets").unwrap().as_object_mut().unwrap() {
        let f = f.as_object_mut().unwrap();
        f.remove("engage_freight");
        f.remove("disposable");
    }
    for s in obj.get_mut("systems").unwrap().as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("blockade_prev");
    }
    let s = serde_json::to_string(&v).unwrap();
    let old: World = serde_json::from_str(&s).expect("pre-feature snapshot must load");
    assert_eq!(old.tick, w.tick);
}

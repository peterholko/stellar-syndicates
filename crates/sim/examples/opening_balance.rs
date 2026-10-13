//! Repeatable balance telemetry for the founding charter's Industry and Reach
//! chapters: Shipyard → research → probe survey → barge → belt mining → haul.
//!
//! The runner starts after Landfall (the mine, the first escorted shipment and
//! the privateer), at the Industry chapter. Everything after that uses commands and `World::step`:
//! Authority freight, construction, Academy supply, research, movement, surveys,
//! and report light. It deliberately clears ambient pirate
//! enclaves so the output measures the economic spine rather than combat RNG.
//!
//! Run, for example:
//! `cargo run -p sim --example opening_balance -- --seeds 100`

use std::collections::BTreeMap;

use sim::build::{SlotPool, StructureKind};
use sim::cargo::Commodity;
use sim::command::Command;
use sim::explore::{ColonyRole, colony_opportunities};
use sim::founding::FoundingStage;
use sim::ids::{EntityId, PlayerId};
use sim::module::Loadout;
use sim::ship::ShipKind;
use sim::{SimConfig, World};

const PHASE_TIMEOUT_S: f64 = 8.0 * 60.0 * 60.0;

#[derive(Clone, Copy)]
struct Strategy {
    name: &'static str,
    research: &'static str,
    preferred: &'static [ColonyRole],
}

const STRATEGIES: [Strategy; 3] = [
    Strategy {
        name: "mobility",
        research: "prop_drive_tuning",
        preferred: &[],
    },
    Strategy {
        name: "industry",
        research: "mat_deep_bores",
        preferred: &[ColonyRole::MiningWorld, ColonyRole::ElectronicsCenter],
    },
    Strategy {
        name: "growth",
        research: "life_med_bays",
        preferred: &[
            ColonyRole::PopulationWorld,
            ColonyRole::AgriculturalExporter,
        ],
    },
];

fn step_until(
    world: &mut World,
    label: &str,
    predicate: impl Fn(&World) -> bool,
) -> Result<(), String> {
    let deadline = world.time + PHASE_TIMEOUT_S;
    while !predicate(world) {
        world.step(&[]);
        if world.time > deadline {
            return Err(format!(
                "timed out waiting for {label} at t={:.1}s",
                world.time
            ));
        }
    }
    Ok(())
}

fn stock(world: &World, system: EntityId, commodity: Commodity) -> f64 {
    world
        .systems
        .iter()
        .find(|candidate| candidate.id == system)
        .and_then(|candidate| candidate.stockpile.get(&commodity).copied())
        .unwrap_or(0.0)
}

fn book_out(world: &mut World, owner: PlayerId, home: EntityId, lots: &[(Commodity, u32)]) {
    let commands: Vec<_> = lots
        .iter()
        .map(|(commodity, units)| Command::BookFreightOut {
            player_id: owner,
            system: home,
            commodity: *commodity,
            units: *units,
        })
        .collect();
    world.step(&commands);
}

fn top_opportunity(world: &World, system: EntityId) -> Option<(ColonyRole, f64)> {
    let target = world
        .systems
        .iter()
        .find(|candidate| candidate.id == system)?;
    let neighbors = world
        .systems
        .iter()
        .filter(|candidate| {
            candidate.id != system
                && candidate.pos.distance(target.pos) <= sim::transit::JUMP_RANGE + 1e-9
        })
        .count();
    colony_opportunities(target, neighbors)
        .into_iter()
        .next()
        .map(|opportunity| (opportunity.role, opportunity.score))
}

fn choose_prospect(
    world: &World,
    home: EntityId,
    candidates: &[EntityId],
    strategy: Strategy,
) -> EntityId {
    let home_pos = world
        .systems
        .iter()
        .find(|system| system.id == home)
        .expect("home exists")
        .pos;
    if strategy.preferred.is_empty() {
        return candidates
            .iter()
            .copied()
            .min_by(|a, b| {
                let ap = world
                    .systems
                    .iter()
                    .find(|system| system.id == *a)
                    .unwrap()
                    .pos;
                let bp = world
                    .systems
                    .iter()
                    .find(|system| system.id == *b)
                    .unwrap()
                    .pos;
                ap.distance(home_pos)
                    .total_cmp(&bp.distance(home_pos))
                    .then_with(|| a.cmp(b))
            })
            .expect("a prospect");
    }
    candidates
        .iter()
        .copied()
        .max_by(|a, b| {
            let score = |id| {
                let (role, raw) =
                    top_opportunity(world, id).unwrap_or((ColonyRole::StrategicOutpost, 0.0));
                let preferred = strategy.preferred.contains(&role);
                (preferred, raw)
            };
            let aa = score(*a);
            let bb = score(*b);
            aa.0.cmp(&bb.0)
                .then_with(|| aa.1.total_cmp(&bb.1))
                .then_with(|| b.cmp(a))
        })
        .expect("a prospect")
}

fn milestone(world: &World, owner: PlayerId, stage: FoundingStage, start: f64) -> f64 {
    world.players[&owner]
        .founding
        .milestones
        .get(&stage)
        .copied()
        .unwrap_or(world.time)
        - start
}

fn run(seed: u64, strategy: Strategy) -> Result<String, String> {
    let owner = PlayerId(1);
    let mut world = World::new(SimConfig::for_players(seed, 4));
    world.enclaves.clear();
    world.step(&[Command::AddPlayer {
        id: owner,
        name: format!("Opening {seed}"),
    }]);
    let start = world.time;
    let home = world.players[&owner]
        .home_system
        .expect("new corp has home");
    let stage = |world: &World| world.players[&owner].founding.stage;

    // Deterministic post-Landfall baseline. The first session built and staffed
    // the Mining Complex, sent the first escorted ore shipment and won the
    // privateer fight; its return trip and the Authority freight desk brought
    // Shipyard I home. The run measures the Industry and Reach chapters.
    {
        let system = world
            .systems
            .iter_mut()
            .find(|system| system.id == home)
            .unwrap();
        system.set_tier(StructureKind::Shipyard, 1);
        system.set_tier(StructureKind::MiningComplex, 1);
        system.stockpile.remove(&Commodity::Alloys);
        system.stockpile.remove(&Commodity::Electronics);
        system.stockpile.remove(&Commodity::Machinery);
        system.stockpile.remove(&Commodity::Polymers);
    }
    // The Academy and Survey Probe kits, already bought at the Exchange, with
    // a margin: the first programme's basket draws Electronics from home too.
    let kits = [
        (Commodity::Alloys, 36),
        (Commodity::Electronics, 30),
        (Commodity::Deuterium, 6),
        (Commodity::Provisions, 20),
    ];
    {
        let corp = world.players.get_mut(&owner).unwrap();
        corp.credits = 4_000.0;
        corp.warehouse = kits.into_iter().collect();
        corp.founding.reward_granted = true;
        corp.founding.export_dispatched_at = Some(world.time);
        corp.founding
            .set_stage(FoundingStage::BuildShipyard, world.time);
    }

    // Industry · 1: staff the Shipyard.
    let yard_body = world
        .systems
        .iter()
        .find(|system| system.id == home)
        .and_then(|system| system.bodies.iter().find(|body| body.tier(StructureKind::Shipyard) > 0))
        .map(|body| body.id)
        .ok_or("home has no Shipyard")?;
    world.step(&[Command::SetAssignment {
        refining_ore: None,
        player_id: owner,
        system_id: home,
        structure: StructureKind::Shipyard,
        workers: 1,
        body_id: Some(yard_body),
        specialists: BTreeMap::new(),
    }]);
    step_until(&mut world, "Shipyard staffed", |world| {
        stage(world) == FoundingStage::FirstResearch
    })?;

    // Industry · 2: an Academy and a first programme.
    book_out(&mut world, owner, home, &kits);
    step_until(&mut world, "Academy and probe kit delivery", |world| {
        stock(world, home, Commodity::Alloys) >= 36.0
            && stock(world, home, Commodity::Electronics) >= 30.0
            && stock(world, home, Commodity::Provisions) >= 20.0
    })?;
    let academy_body = world
        .systems
        .iter()
        .find(|system| system.id == home)
        .and_then(|system| {
            system
                .bodies
                .iter()
                .find(|body| {
                    body.pool_slots_built(SlotPool::Infrastructure)
                        < body.pool_slots(SlotPool::Infrastructure)
                })
                .map(|body| body.id)
        })
        .ok_or("home has no free infrastructure slot for its founding Academy")?;
    world.step(&[Command::DevelopSystem {
        player_id: owner,
        system_id: home,
        upgrade: StructureKind::Academy,
        body_id: Some(academy_body),
    }]);
    step_until(&mut world, "Academy construction", |world| {
        world
            .systems
            .iter()
            .find(|system| system.id == home)
            .is_some_and(|system| system.tier(StructureKind::Academy) >= 1)
    })?;
    world.step(&[
        Command::SetAssignment {
            refining_ore: None,
            player_id: owner,
            system_id: home,
            structure: StructureKind::Academy,
            workers: 1,
            body_id: Some(academy_body),
            specialists: BTreeMap::new(),
        },
        Command::SetResearchQueue {
            player_id: owner,
            queue: vec![strategy.research.to_owned()],
        },
    ]);
    step_until(&mut world, "research under way", |world| {
        stage(world) == FoundingStage::SurveyCandidates
    })?;

    // Industry · 3: a Survey Probe charts the assigned local stop.
    world.step(&[Command::BuildShip {
        player_id: owner,
        system_id: home,
        ship_kind: ShipKind::SurveyProbe,
        join: None,
        loadout: Loadout::default(),
    }]);
    step_until(&mut world, "probe construction", |world| {
        world.fleets.values().any(|fleet| fleet.owner == owner && fleet.contains(ShipKind::SurveyProbe))
    })?;
    let probe = world
        .fleets
        .values()
        .find(|fleet| fleet.owner == owner && fleet.contains(ShipKind::SurveyProbe))
        .map(|fleet| fleet.id)
        .unwrap();
    let candidates = world.players[&owner].founding.survey_candidates.clone();
    let candidate = *candidates.first().ok_or_else(|| format!("seed {seed} assigned no survey"))?;
    world.step(&[Command::SurveySystem {
        player_id: owner,
        fleet_id: probe,
        system_id: candidate,
    }]);
    step_until(&mut world, "probe survey report", |world| {
        stage(world) == FoundingStage::BuildBarge
    })?;

    // Reach · 1: buy the barge kit, ship it home and build the barge.
    let kit = [
        (Commodity::Alloys, 40),
        (Commodity::Machinery, 20),
        (Commodity::Polymers, 12),
        (Commodity::Electronics, 6),
    ];
    let buys: Vec<_> = kit
        .iter()
        .map(|(commodity, units)| Command::MarketBuy {
            player_id: owner,
            commodity: *commodity,
            units: *units,
            max_unit_price: None,
        })
        .collect();
    world.step(&buys);
    step_until(&mut world, "barge kit purchase", |world| {
        kit.iter().all(|(commodity, units)| {
            world.players[&owner].warehouse.get(commodity).copied().unwrap_or(0) >= *units
        })
    })
    .map_err(|error| format!("{error}; credits={:.0} warehouse={:?}",
        world.players[&owner].credits, world.players[&owner].warehouse))?;
    // A home's storage is full by now (the food reserve, ore). A founder who
    // keeps importing has added a Warehouse tier; the run models that.
    {
        let system = world.systems.iter_mut().find(|system| system.id == home).unwrap();
        let tier = system.tier(StructureKind::Warehouse);
        system.set_tier(StructureKind::Warehouse, tier + 1);
    }
    book_out(&mut world, owner, home, &kit);
    step_until(&mut world, "barge kit delivery", |world| {
        kit.iter().all(|(commodity, units)| stock(world, home, *commodity) >= *units as f64)
    })?;
    step_until(&mut world, "a free Shipyard slip", |world| {
        !world.build_queue.iter().any(|job| {
            job.system == home && matches!(job.what, sim::build::BuildKind::Ship { .. })
        })
    })?;
    world.step(&[Command::BuildShip {
        player_id: owner,
        system_id: home,
        ship_kind: ShipKind::MiningBarge,
        join: None,
        loadout: Loadout::default(),
    }]);
    step_until(&mut world, "barge construction", |world| {
        stage(world) == FoundingStage::MineBelt
    })?;

    // Reach · 2: mine the charted belt.
    let barge = world
        .fleets
        .values()
        .find(|fleet| fleet.owner == owner && fleet.contains(ShipKind::MiningBarge))
        .map(|fleet| fleet.id)
        .ok_or("barge build did not create a fleet")?;
    let (belt_system, belt) = world.players[&owner]
        .founding
        .belt_target
        .ok_or("the barge lesson has no belt")?;
    let ore = world
        .systems
        .iter()
        .find(|system| system.id == belt_system)
        .and_then(|system| system.bodies.iter().find(|body| body.id == belt))
        .and_then(|body| body.deposits.first())
        .map(|deposit| deposit.resource)
        .ok_or("the lesson's belt has no ore")?;
    world.step(&[Command::MineBelt {
        player_id: owner,
        fleet_id: barge,
        system_id: belt_system,
        body_id: belt,
        ore,
        pocket: false,
    }]);
    step_until(&mut world, "belt mining report", |world| {
        stage(world) == FoundingStage::HaulOre
    })?;

    // Reach · 3: bring the ore home and unload it.
    step_until(&mut world, "a first load", |world| world.fleets[&barge].cargo_units() >= 20)?;
    let home_pos = world.systems.iter().find(|system| system.id == home).unwrap().pos;
    world.step(&[Command::MoveShip {
        player_id: owner,
        ship_id: barge,
        dest: home_pos,
    }]);
    step_until(&mut world, "barge home", |world| {
        let fleet = &world.fleets[&barge];
        fleet.pos.distance(home_pos) <= sim::ship::COLONY_CLAIM_RADIUS
            && matches!(fleet.order, sim::ship::FleetOrder::Idle)
    })?;
    world.step(&[Command::SystemUnload {
        player_id: owner,
        fleet_id: barge,
        system: home,
    }]);
    step_until(&mut world, "ore home report", |world| {
        stage(world) == FoundingStage::Complete
    })?;

    let destination = choose_prospect(&world, home, &candidates, strategy);
    let (role, score) =
        top_opportunity(&world, destination).unwrap_or((ColonyRole::StrategicOutpost, 0.0));
    let m = |stage| milestone(&world, owner, stage, start) / 60.0;
    Ok(format!(
        "{seed},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{},{},{:.2}",
        strategy.name,
        m(FoundingStage::FirstResearch),
        m(FoundingStage::SurveyCandidates),
        m(FoundingStage::BuildBarge),
        m(FoundingStage::MineBelt),
        m(FoundingStage::HaulOre),
        m(FoundingStage::Complete),
        destination.0,
        role.slug(),
        score,
    ))
}

fn main() {
    let mut seeds = 3u64;
    let mut seed_start = 1u64;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--seeds" if index + 1 < args.len() => {
                seeds = args[index + 1].parse().expect("--seeds takes an integer");
                index += 2;
            }
            "--seed-start" if index + 1 < args.len() => {
                seed_start = args[index + 1]
                    .parse()
                    .expect("--seed-start takes an integer");
                index += 2;
            }
            other => panic!("unknown argument {other}; use --seeds N [--seed-start N]"),
        }
    }

    println!(
        "seed,strategy,shipyard_min,research_min,survey_min,barge_min,belt_mining_min,haul_min,surveyed_stop,role,score"
    );
    for seed in seed_start..seed_start + seeds {
        for strategy in STRATEGIES {
            match run(seed, strategy) {
                Ok(row) => println!("{row}"),
                Err(error) => {
                    eprintln!("seed={seed} strategy={} failed: {error}", strategy.name);
                    std::process::exit(1);
                }
            }
        }
    }
}

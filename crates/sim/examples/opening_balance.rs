//! Repeatable balance telemetry for the Academy → first-colony chapter.
//!
//! The runner starts at the moment the founding Convoy's first sale report has
//! reached home. Everything after that uses ordinary commands and `World::step`:
//! Authority freight, construction, Academy supply, research, movement, surveys,
//! report light, and physical settlement. It deliberately clears ambient pirate
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
use sim::math::Vec2;
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

fn unit(from: Vec2, to: Vec2) -> Vec2 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = (dx * dx + dy * dy).sqrt().max(1e-9);
    Vec2::new(dx / length, dy / length)
}

fn outside_wells(world: &World, point: Vec2) -> bool {
    point.distance(world.hub) >= sim::transit::HYPERLIMIT
        && world
            .systems
            .iter()
            .all(|system| point.distance(system.pos) >= sim::transit::HYPERLIMIT)
}

fn safe_jump_waypoint(world: &World, from: Vec2, nominal: Vec2) -> Result<Vec2, String> {
    for radius in [0.0, 1_200.0, 1_800.0, 2_600.0] {
        for spoke in 0..8 {
            let angle = std::f64::consts::TAU * spoke as f64 / 8.0;
            let candidate = nominal + Vec2::new(radius * angle.cos(), radius * angle.sin());
            if outside_wells(world, candidate)
                && from.distance(candidate) <= sim::transit::JUMP_RANGE + 1e-9
            {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "could not place a safe jump waypoint near ({:.0},{:.0})",
        nominal.x, nominal.y
    ))
}

/// Fly clear of the origin well, make as many ordinary ≤50k jumps as needed,
/// and land just outside the destination well. The final short approach remains
/// physical movement; jump never teleports onto a survey/settlement objective.
fn jump_between(
    world: &mut World,
    owner: PlayerId,
    fleet: EntityId,
    origin: Vec2,
    destination: Vec2,
) -> Result<(), String> {
    let direction = unit(origin, destination);
    let clearance = sim::transit::HYPERLIMIT + 250.0;
    let departure = origin + direction * clearance;
    world.step(&[Command::MoveShip {
        player_id: owner,
        ship_id: fleet,
        dest: departure,
    }]);
    step_until(world, "departure-well clearance", |world| {
        world.fleets[&fleet].pos.distance(departure) <= 1.0
    })?;

    let nominal_arrival = destination - direction * clearance;
    let mut arrival = safe_jump_waypoint(world, nominal_arrival, nominal_arrival)?;
    let mut current = world.fleets[&fleet].pos;
    while current.distance(arrival) > sim::transit::JUMP_RANGE {
        let toward = unit(current, arrival);
        let nominal = current + toward * (sim::transit::JUMP_RANGE - 5_000.0);
        let waypoint = safe_jump_waypoint(world, current, nominal)?;
        let before = world.fleets[&fleet].last_jump;
        world.step(&[Command::JumpShip {
            player_id: owner,
            ship_id: fleet,
            dest: waypoint,
        }]);
        step_until(world, "intermediate jump", |world| {
            world.fleets[&fleet].last_jump != before
        })?;
        current = world.fleets[&fleet].pos;
        // A waypoint adjustment can make the original final approach marginally
        // out of range. Re-resolve it from the new physical point.
        arrival = safe_jump_waypoint(world, nominal_arrival, nominal_arrival)?;
    }
    let before = world.fleets[&fleet].last_jump;
    world.step(&[Command::JumpShip {
        player_id: owner,
        ship_id: fleet,
        dest: arrival,
    }]);
    step_until(world, "destination jump", |world| {
        world.fleets[&fleet].last_jump != before
    })?;
    Ok(())
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

fn choose_colony(
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

    // Deterministic post-sale baseline. The earlier chapter already consumed
    // the starter kit on Shipyard I + Mining Complex I and consumed the Convoy
    // portion of the privateer bounty. The remaining lots are exactly Academy,
    // Scout, and the first programme's Electronics.
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
    {
        let corp = world.players.get_mut(&owner).unwrap();
        corp.credits = 2_750.0;
        corp.warehouse = [
            (Commodity::Alloys, 40),
            (Commodity::Electronics, 31),
            (Commodity::Fuel, 8),
            (Commodity::Provisions, 20),
        ]
        .into_iter()
        .collect();
        corp.founding.reward_granted = true;
        corp.founding.market_haul_completed = true;
        corp.founding.sale_report_at = Some(world.time);
        corp.founding
            .set_stage(FoundingStage::BuildAcademy, world.time);
    }

    book_out(
        &mut world,
        owner,
        home,
        &[
            (Commodity::Alloys, 40),
            (Commodity::Electronics, 31),
            (Commodity::Fuel, 8),
            (Commodity::Provisions, 20),
        ],
    );
    step_until(&mut world, "Academy kit delivery", |world| {
        stock(world, home, Commodity::Alloys) >= 25.0
            && stock(world, home, Commodity::Electronics) >= 15.0
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
    world.step(&[Command::SetAssignment {
        player_id: owner,
        system_id: home,
        structure: StructureKind::Academy,
        workers: 1,
        body_id: Some(academy_body),
        specialists: BTreeMap::new(),
    }]);
    step_until(&mut world, "first-research stage", |world| {
        world.players[&owner].founding.stage == FoundingStage::FirstResearch
    })?;
    world.step(&[Command::SetResearchQueue {
        player_id: owner,
        queue: vec![strategy.research.to_owned()],
    }]);
    step_until(&mut world, "first research", |world| {
        world.players[&owner].founding.stage == FoundingStage::BuildScout
    })
    .map_err(|error| {
        let corp = &world.players[&owner];
        format!(
            "{error}; active={:?} progress={:.1} stalled={} home electronics/biomass/provisions={:.2}/{:.2}/{:.2} warehouse={:?}",
            corp.research.active,
            corp.research.progress,
            corp.research.stalled,
            stock(&world, home, Commodity::Electronics),
            stock(&world, home, Commodity::Biomass),
            stock(&world, home, Commodity::Provisions),
            corp.warehouse,
        )
    })?;

    world.step(&[Command::BuildShip {
        player_id: owner,
        system_id: home,
        ship_kind: ShipKind::Scout,
        join: None,
        loadout: Loadout::default(),
    }]);
    step_until(&mut world, "Scout construction", |world| {
        world.players[&owner].founding.stage == FoundingStage::SurveyCandidates
    })?;
    let scout = world
        .fleets
        .values()
        .find(|fleet| fleet.owner == owner && fleet.contains(ShipKind::Scout))
        .map(|fleet| fleet.id)
        .ok_or("Scout build did not create a fleet")?;
    let candidates = world.players[&owner].founding.survey_candidates.clone();
    if candidates.len() != 2 {
        return Err(format!(
            "seed {seed} produced {} founding prospects",
            candidates.len()
        ));
    }
    let home_pos = world
        .systems
        .iter()
        .find(|system| system.id == home)
        .expect("home exists")
        .pos;
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let candidate_pos = world
            .systems
            .iter()
            .find(|system| system.id == *candidate)
            .expect("prospect exists")
            .pos;
        jump_between(&mut world, owner, scout, home_pos, candidate_pos)?;
        world.step(&[Command::SurveySystem {
            player_id: owner,
            fleet_id: scout,
            system_id: *candidate,
        }]);
        step_until(&mut world, "prospect report", |world| {
            world.players[&owner].surveyed.contains(candidate)
        })
        .map_err(|error| {
            let fleet = &world.fleets[&scout];
            format!(
                "{error}; prospect={} id={} scout_pos=({:.0},{:.0}) fuel={:.2} supplied={} order={:?}",
                candidate_index + 1,
                candidate.0,
                fleet.pos.x,
                fleet.pos.y,
                fleet.fuel,
                fleet.supplied,
                fleet.order,
            )
        })?;
        // One Scout tank is a deliberate sortie, not a two-prospect grand tour.
        // Return by the same jump doctrine between reports and let the ordinary
        // home-dock pass refill the short physical approach.
        if candidate_index + 1 < candidates.len() {
            jump_between(&mut world, owner, scout, candidate_pos, home_pos)?;
            world.step(&[Command::MoveShip {
                player_id: owner,
                ship_id: scout,
                dest: home_pos,
            }]);
            step_until(&mut world, "Scout return and refuel", |world| {
                let fleet = &world.fleets[&scout];
                fleet.pos.distance(home_pos) <= sim::ship::COLONY_CLAIM_RADIUS
                    && fleet.fuel + 1e-6 >= fleet.fuel_capacity()
            })
            .map_err(|error| {
                let fleet = &world.fleets[&scout];
                format!(
                    "{error}; distance_home={:.0} fuel={:.2}/{:.2} order={:?}",
                    fleet.pos.distance(home_pos),
                    fleet.fuel,
                    fleet.fuel_capacity(),
                    fleet.order,
                )
            })?;
        }
    }
    step_until(&mut world, "two-report comparison", |world| {
        world.players[&owner].founding.stage == FoundingStage::BuildColony
    })?;

    // The home has been producing throughout both survey sorties. Clear those
    // whole-unit lots through the ordinary inbound Authority channel before the
    // 125-unit Colony kit arrives; this makes storage a measured constraint
    // instead of letting the harness write through the cap.
    world.step(&[Command::ShipProduction {
        player_id: owner,
        system_id: home,
    }]);
    book_out(
        &mut world,
        owner,
        home,
        &[
            (Commodity::Alloys, 45),
            (Commodity::Machinery, 15),
            (Commodity::Polymers, 20),
            (Commodity::Provisions, 30),
            (Commodity::Fuel, 15),
        ],
    );
    step_until(&mut world, "Colony Ship kit delivery", |world| {
        stock(world, home, Commodity::Alloys) >= 45.0
            && stock(world, home, Commodity::Machinery) >= 15.0
            && stock(world, home, Commodity::Polymers) >= 20.0
            && stock(world, home, Commodity::Provisions) >= 30.0
            && stock(world, home, Commodity::Fuel) >= 15.0
    })
    .map_err(|error| {
        format!(
            "{error}; home A/M/P/Pv/F={:.0}/{:.0}/{:.0}/{:.0}/{:.0} warehouse={:?} credits={:.0} queued={} runs={}",
            stock(&world, home, Commodity::Alloys),
            stock(&world, home, Commodity::Machinery),
            stock(&world, home, Commodity::Polymers),
            stock(&world, home, Commodity::Provisions),
            stock(&world, home, Commodity::Fuel),
            world.players[&owner].warehouse,
            world.players[&owner].credits,
            world.freight_queue.len(),
            world.freight_runs.len(),
        )
    })?;
    world.step(&[Command::BuildShip {
        player_id: owner,
        system_id: home,
        ship_kind: ShipKind::Colony,
        join: None,
        loadout: Loadout::default(),
    }]);
    step_until(&mut world, "Colony Ship construction", |world| {
        world.players[&owner].founding.stage == FoundingStage::EstablishColony
    })?;
    let colony = world
        .fleets
        .values()
        .find(|fleet| fleet.owner == owner && fleet.contains(ShipKind::Colony))
        .map(|fleet| fleet.id)
        .ok_or("Colony build did not create a fleet")?;
    let destination = choose_colony(&world, home, &candidates, strategy);
    let destination_pos = world
        .systems
        .iter()
        .find(|system| system.id == destination)
        .expect("prospect exists")
        .pos;
    world.step(&[Command::MoveShip {
        player_id: owner,
        ship_id: colony,
        dest: destination_pos,
    }]);
    step_until(&mut world, "first colony", |world| {
        world.players[&owner].founding.stage == FoundingStage::Complete
    })?;

    let (role, score) =
        top_opportunity(&world, destination).unwrap_or((ColonyRole::StrategicOutpost, 0.0));
    let m = |stage| milestone(&world, owner, stage, start) / 60.0;
    Ok(format!(
        "{seed},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{},{},{:.2}",
        strategy.name,
        m(FoundingStage::FirstResearch),
        m(FoundingStage::BuildScout),
        m(FoundingStage::SurveyCandidates),
        m(FoundingStage::BuildColony),
        m(FoundingStage::EstablishColony),
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
        "seed,strategy,academy_min,research_min,scout_min,two_surveys_min,colony_ship_min,established_min,destination,role,score"
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

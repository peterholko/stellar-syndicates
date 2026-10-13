//! Star-chart density report: how many systems a galaxy has, how many lie near
//! each home, and what share are habitable. Run explicitly:
//! `cargo test -p sim --release --test galaxy_density -- --ignored --nocapture`
use sim::{SimConfig, World};

fn habitable(system: &sim::galaxy::StarSystem) -> bool {
    system.bodies.iter().any(|body| body.habitable)
}

#[test]
#[ignore = "report; prints star-chart density across seeds"]
fn galaxy_density_report() {
    for players in [4u32, 8] {
        let seeds: Vec<u64> = (0..24).map(|i| 0xC0FFEE + i * 7919).collect();
        let (mut total, mut habitable_total, mut sparse) = (0usize, 0usize, 0usize);
        let mut near = [0usize; 4];
        let mut near_hab = [0usize; 4];
        let mut homes = 0usize;
        let mut nearest_hab = Vec::new();
        let mut by_kind = [(0usize, 0usize, 0usize); 4];
        let radii = [40_000.0, 80_000.0, 120_000.0, 160_000.0];
        for &seed in &seeds {
            let world = World::new(SimConfig::for_players(seed, players));
            total += world.systems.len();
            sparse += world.exploration_systems.len();
            habitable_total += world.systems.iter().filter(|s| habitable(s)).count();
            let homes_ids: Vec<_> = world.home_slots.iter().filter_map(|h| h.system).collect();
            for system in &world.systems {
                let kind = if homes_ids.contains(&system.id) { 0 }
                    else if world.home_neighbours.contains(&system.id) { 3 }
                    else if world.exploration_systems.contains(&system.id) { 2 } else { 1 };
                by_kind[kind].0 += 1;
                if habitable(system) { by_kind[kind].1 += 1; }
                by_kind[kind].2 += system.bodies.iter().filter(|b| b.habitable).count();
            }
            for slot in &world.home_slots {
                homes += 1;
                let home = slot.system;
                let mut best = f64::INFINITY;
                for system in world.systems.iter().filter(|s| Some(s.id) != home) {
                    let d = system.pos.distance(slot.pos);
                    for (i, r) in radii.iter().enumerate() {
                        if d <= *r {
                            near[i] += 1;
                            if habitable(system) { near_hab[i] += 1; }
                        }
                    }
                    if habitable(system) { best = best.min(d); }
                }
                nearest_hab.push(best);
            }
        }
        let n = seeds.len() as f64;
        nearest_hab.sort_by(f64::total_cmp);
        println!("players {players}: radius {:.0} su; per galaxy {:.1} systems ({:.1} sparse), {:.1}% habitable",
            SimConfig::for_players(0, players).galaxy_radius, total as f64 / n, sparse as f64 / n,
            100.0 * habitable_total as f64 / total as f64);
        for (name, (count, hab, worlds)) in ["home", "full", "sparse", "neighbour"].iter().zip(by_kind) {
            println!("  {name:>6}: {:.1} per galaxy, {:.0}% habitable, {:.2} habitable worlds each",
                count as f64 / n, 100.0 * hab as f64 / count.max(1) as f64, worlds as f64 / count.max(1) as f64);
        }
        for (i, r) in radii.iter().enumerate() {
            println!("  within {:>6.0} su of a home: {:.2} systems, {:.2} habitable", r,
                near[i] as f64 / homes as f64, near_hab[i] as f64 / homes as f64);
        }
        println!("  nearest habitable system: median {:.0} su, worst {:.0} su",
            nearest_hab[nearest_hab.len() / 2], nearest_hab[nearest_hab.len() - 1]);
    }
}

#[test]
#[ignore = "report; organic survey discovery rate by system type"]
fn discovery_rate_by_kind() {
    let (mut frontier, mut frontier_hit, mut ring, mut ring_hit, mut door, mut door_hit) = (0, 0, 0, 0, 0, 0);
    for seed in 0..80u64 {
        let w = World::new(SimConfig::for_players(seed, 4));
        let homes: Vec<_> = w.home_slots.iter().filter_map(|h| h.system).collect();
        let guarantees: Vec<_> = w.home_slots.iter().flat_map(|h| h.founding_opportunities.iter().copied()).collect();
        for sys in &w.systems {
            if homes.contains(&sys.id) || w.exploration_systems.contains(&sys.id) || guarantees.contains(&sys.id) { continue; }
            let neighbours = w.systems.iter().filter(|o| o.id != sys.id && o.pos.distance(sys.pos) <= sim::transit::JUMP_RANGE + 1e-9).count();
            let hit = sim::explore::colony_opportunities(sys, neighbours).iter().any(|o| o.score >= sim::explore::ROLE_EXCEPTIONAL_MIN);
            let doorstep = w.home_slots.iter().any(|h| h.pos.distance(sys.pos) < w.config.sensor_range);
            if w.home_neighbours.contains(&sys.id) { ring += 1; ring_hit += hit as usize; }
            else if doorstep { door += 1; door_hit += hit as usize; }
            else { frontier += 1; frontier_hit += hit as usize; }
        }
    }
    println!("frontier organic: {frontier_hit}/{frontier} = {:.3}", frontier_hit as f64 / frontier as f64);
    println!("neighbour organic: {ring_hit}/{ring} = {:.3}", ring_hit as f64 / ring as f64);
    println!("other doorstep organic: {door_hit}/{door} = {:.3}", door_hit as f64 / door.max(1) as f64);
}

/// Writes one galaxy's chart as JSON to $CHART_OUT for a quick picture.
#[test]
#[ignore = "writes a chart dump for a picture"]
fn dump_chart() {
    let Ok(out) = std::env::var("CHART_OUT") else { return };
    let seed = std::env::var("CHART_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0xC0FFEE);
    let w = World::new(SimConfig::for_players(seed, 4));
    let homes: Vec<_> = w.home_slots.iter().filter_map(|h| h.system).collect();
    let opportunities: Vec<_> = w.home_slots.iter().flat_map(|h| h.founding_opportunities.iter().copied()).collect();
    let systems: Vec<_> = w.systems.iter().map(|s| serde_json::json!({
        "x": s.pos.x, "y": s.pos.y, "name": s.name,
        "kind": if homes.contains(&s.id) { "home" } else if w.home_neighbours.contains(&s.id) { "neighbour" }
            else if w.exploration_systems.contains(&s.id) { "sparse" } else { "full" },
        "habitable": habitable(s), "starter": opportunities.contains(&s.id),
        "worlds": s.bodies.iter().filter(|b| b.habitable).count(),
        "enclave": w.enclaves.contains_key(&s.id),
    })).collect();
    let chart = serde_json::json!({ "radius": w.config.galaxy_radius, "sensor": w.config.sensor_range, "systems": systems });
    std::fs::write(out, chart.to_string()).unwrap();
}

#[test]
#[ignore = "report; habitable worlds per system"]
fn habitable_worlds_per_system() {
    let mut hist = std::collections::BTreeMap::<&str, [usize; 4]>::new();
    for seed in 0..80u64 {
        let w = World::new(SimConfig::for_players(seed, 4));
        let homes: Vec<_> = w.home_slots.iter().filter_map(|h| h.system).collect();
        let starters: Vec<_> = w.home_slots.iter().flat_map(|h| h.founding_opportunities.iter().copied()).collect();
        for s in &w.systems {
            let kind = if homes.contains(&s.id) { "home" } else if starters.contains(&s.id) { "starter" }
                else if w.home_neighbours.contains(&s.id) { "neighbour" }
                else if w.exploration_systems.contains(&s.id) { "sparse" } else { "full" };
            let n = s.bodies.iter().filter(|b| b.habitable).count().min(3);
            hist.entry(kind).or_default()[n] += 1;
        }
    }
    for (kind, h) in hist {
        let total: usize = h.iter().sum();
        println!("{kind:>9}: 0={:>4} 1={:>4} 2={:>4} 3+={:>4}  (2+ = {:.1}% of all, {:.1}% of habitable)", h[0], h[1], h[2], h[3],
            100.0 * (h[2] + h[3]) as f64 / total as f64, 100.0 * (h[2] + h[3]) as f64 / (h[1] + h[2] + h[3]).max(1) as f64);
    }
}

/// §belts phase 0: how many belts a galaxy rolls, of which types and ores, and
/// how much ore content they add next to the planets. Run explicitly:
/// `cargo test -p sim --release --test galaxy_density belt_report -- --ignored --nocapture`
#[test]
#[ignore = "report; prints asteroid-belt counts, types, ores and added content"]
fn belt_report() {
    use std::collections::BTreeMap;
    let seeds: Vec<u64> = (0..24).map(|i| 0xBE17 + i * 7919).collect();
    let (mut systems, mut sparse, mut belts, mut bare) = (0usize, 0usize, 0usize, 0usize);
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut ores: BTreeMap<String, usize> = BTreeMap::new();
    let (mut planet_content, mut belt_content) = (0.0f64, 0.0f64);
    let (mut planet_value, mut belt_value) = (0.0f64, 0.0f64);
    // §pockets: hidden pockets by ore — how many, and how much ore they hold.
    let mut pockets: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for &seed in &seeds {
        let world = World::new(SimConfig::for_players(seed, 4));
        systems += world.systems.len();
        sparse += world.exploration_systems.len();
        for system in &world.systems {
            for body in &system.bodies {
                let content: f64 = body.deposits.iter().map(|d| d.richness).sum();
                let value: f64 = body.deposits.iter().map(sim::galaxy::deposit_value_rate).sum();
                if let Some(shape) = body.belt {
                    belts += 1;
                    bare += body.deposits.is_empty() as usize;
                    *kinds.entry(shape.kind.slug().to_string()).or_default() += 1;
                    for d in &body.deposits { *ores.entry(d.resource.slug().to_string()).or_default() += 1; }
                    for p in &body.pockets {
                        let row = pockets.entry(p.deposit.resource.slug().to_string()).or_default();
                        row.0 += 1;
                        row.1 += p.deposit.reserves.unwrap_or(0.0);
                    }
                    belt_content += content;
                    belt_value += value;
                } else {
                    planet_content += content;
                    planet_value += value;
                }
            }
        }
    }
    let n = seeds.len() as f64;
    println!("4-player galaxies × {}: {:.1} systems ({:.1} sparse) · {:.1} belts ({:.1} bare)",
        seeds.len(), systems as f64 / n, sparse as f64 / n, belts as f64 / n, bare as f64 / n);
    println!("belt types per galaxy: {}", kinds.iter()
        .map(|(k, c)| format!("{k} {:.1}", *c as f64 / n)).collect::<Vec<_>>().join(" · "));
    println!("belt deposits per galaxy: {}", ores.iter()
        .map(|(k, c)| format!("{k} {:.1}", *c as f64 / n)).collect::<Vec<_>>().join(" · "));
    println!("content (bulk/s): planets {:.1}, belts {:.1} (+{:.0}%)", planet_content / n, belt_content / n,
        100.0 * belt_content / planet_content);
    println!("value rate (cr/s): planets {:.0}, belts {:.0} (+{:.0}%)", planet_value / n, belt_value / n,
        100.0 * belt_value / planet_value);
    // A pocket's reserves in ore units, and what they refine into at the
    // Smelter's ratio (units of the recipe's primary output).
    println!("pockets per galaxy (count · ore units · refined units · base value):");
    for (ore, (count, units)) in &pockets {
        let commodity: sim::Commodity = serde_json::from_value(serde_json::json!(ore)).unwrap();
        let refined = sim::production::ORE_REFINING.iter().find(|r| r.inputs[0].0 == commodity)
            .map(|r| (r.output, units / r.inputs[0].1));
        let value = units * sim::market::base_price(commodity);
        match refined {
            Some((out, n_out)) => println!("  {ore}: {:.1} · {:.0} · {:.0} {} · {:.0} cr", *count as f64 / n, units / n,
                n_out / n, out.slug(), value / n),
            None => println!("  {ore}: {:.1} · {:.0} · {:.0} cr", *count as f64 / n, units / n, value / n),
        }
    }
}

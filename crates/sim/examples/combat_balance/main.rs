//! The equal-cost combat balance matrix: every archetype in
//! `sim::combat_balance` against every other, in both roles, at each fight
//! size, on the current battle rules. Measurement only (docs/combat-balance.md).
//!
//! `cargo run --release -p sim --example combat_balance -- [--seeds 100]
//!     [--tiers skirmish,fleet,major] [--only key,key] [--threads 8]
//!     [--out docs/balance] [--baseline docs/balance/combat-balance-v10.json]`
//!
//! Writes `combat-balance-v<rules>.json` (the numbers) and `.html` (the report,
//! one self-contained page) into `--out`. With `--baseline`, the report also
//! shows how every matchup moved since that earlier run. `--render <json>`
//! rebuilds a report page from saved numbers without fighting again.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sim::combat_balance::{self as cb, Archetype, Cell, EscapeCell, Fleet, Table, Tier};
use sim::module::Family;
use sim::ship::ShipKind;

const TEMPLATE: &str = include_str!("report.html");
const DATA_SLOT: &str = "/*__COMBAT_BALANCE_REPORT__*/null";

struct Args {
    seeds: u32,
    tiers: Vec<Tier>,
    only: Option<Vec<String>>,
    threads: usize,
    out: PathBuf,
    baseline: Option<PathBuf>,
    render: Option<PathBuf>,
}

fn args() -> Args {
    let mut a = Args {
        seeds: 100,
        tiers: cb::TIERS.to_vec(),
        only: None,
        threads: std::thread::available_parallelism().map_or(4, |n| n.get()),
        out: PathBuf::from("docs/balance"),
        baseline: None,
        render: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| panic!("{flag} needs a value"));
        match flag.as_str() {
            "--seeds" => a.seeds = value().parse().expect("--seeds N"),
            "--threads" => a.threads = value().parse().expect("--threads N"),
            "--out" => a.out = PathBuf::from(value()),
            "--baseline" => a.baseline = Some(PathBuf::from(value())),
            "--render" => a.render = Some(PathBuf::from(value())),
            "--only" => a.only = Some(value().split(',').map(str::to_string).collect()),
            "--tiers" => {
                let keys = value();
                a.tiers = keys.split(',').map(|k| *cb::TIERS.iter().find(|t| t.key == k)
                    .unwrap_or_else(|| panic!("unknown tier {k}"))).collect();
            }
            other => panic!("unknown flag {other} (see the file header)"),
        }
    }
    a
}

// --- What the report carries ---------------------------------------------------

#[derive(Serialize)]
struct HullCost {
    kind: ShipKind,
    label: &'static str,
    value: f64,
    build_hours: f64,
    aboard: u32,
    upkeep_day: f64,
    hp: f64,
    attack: f64,
    speed: f64,
    turn: Option<f64>,
    slots: u32,
    points: u32,
    affinity: Vec<String>,
}

#[derive(Serialize)]
struct ModuleCost {
    label: &'static str,
    value: f64,
    points: u32,
    effect: &'static str,
}

#[derive(Serialize)]
struct ArchetypeView {
    key: &'static str,
    name: &'static str,
    hull: &'static str,
    weapon: &'static str,
}

#[derive(Serialize)]
struct TierReport {
    key: &'static str,
    name: &'static str,
    budget: f64,
    /// Indices into `archetypes`: the ones that fit this budget, in order.
    archetypes: Vec<usize>,
    fleets: Vec<Fleet>,
    /// `cells[i][j]`: archetype i attacking archetype j.
    cells: Vec<Vec<Cell>>,
    table: Table,
    /// Catch and hold: each runner (row) withdrawing at once from each hunter
    /// (column), as indices into `archetypes`.
    escapes: Escapes,
}

#[derive(Serialize)]
struct Escapes {
    runners: Vec<usize>,
    hunters: Vec<usize>,
    /// `cells[runner][hunter]`.
    cells: Vec<Vec<EscapeCell>>,
}

#[derive(Serialize)]
struct Baseline {
    rules: u64,
    generated: String,
    seeds: u64,
    /// Tier key → "row|column" archetype keys → score.
    scores: BTreeMap<String, BTreeMap<String, f64>>,
}

#[derive(Serialize)]
struct Report {
    rules: u32,
    generated: String,
    seeds: u32,
    step_cap: u64,
    hulls: Vec<HullCost>,
    modules: Vec<ModuleCost>,
    archetypes: Vec<ArchetypeView>,
    tiers: Vec<TierReport>,
    baseline: Option<Baseline>,
}

fn hull_costs() -> Vec<HullCost> {
    let families = [(Family::Beam, "beams"), (Family::Driver, "drivers"), (Family::Torpedo, "torpedoes"),
        (Family::Interception, "point defense"), (Family::Protection, "armor")];
    cb::WARSHIPS.iter().map(|&kind| HullCost {
        kind,
        label: cb::hull_label(kind),
        value: cb::hull_value(kind).round(),
        build_hours: cb::hull_build_hours(kind),
        aboard: sim::ship::crew_size(kind) + sim::ship::marine_capacity(kind),
        upkeep_day: cb::upkeep_per_day(kind).round(),
        hp: kind.hull_mass(),
        attack: kind.attack_weight(),
        speed: kind.max_speed(),
        turn: sim::tactical::turn_rate(kind),
        slots: kind.module_slots(),
        points: sim::ship::fitting_points(kind),
        affinity: families.iter().filter_map(|&(f, name)| {
            let a = sim::ship::hull_affinity(kind, f);
            (a != 1.0).then(|| format!("{name} ×{a}"))
        }).collect(),
    }).collect()
}

fn module_costs() -> Vec<ModuleCost> {
    use sim::module::ModuleKind as M;
    cb::COMBAT_MODULES.iter().map(|&m| ModuleCost {
        label: m.label(),
        value: cb::module_value(m).round(),
        points: m.fitting_cost(),
        effect: match m {
            M::MassDriver => "fires one ×1.3 driver shot per copy, each at its own target",
            M::TorpedoRack => "launches one ×1.6 torpedo per copy, each at its own target",
            M::PointDefenseScreen => "shoots down torpedoes; its own beam fires at ½",
            M::ReflectivePlating => "−50% beam damage taken",
            M::WhippleArmor => "−60% driver damage taken",
            M::StasisWeb => "slows the nearest enemy hull in range to half speed",
            M::WarpStabilizer => "shrugs off one tackling interceptor when leaving",
            M::InterdictionField => "destroyer only: drops fleets crossing its field out of warp",
            _ => "",
        },
    }).collect()
}

// --- Running it ----------------------------------------------------------------

struct Job {
    tier: usize,
    attacker: usize,
    defender: usize,
    weight: f64,
}

fn main() {
    let args = args();
    if let Some(path) = &args.render {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut value: serde_json::Value = serde_json::from_str(&text).expect("a combat-balance json report");
        derive(&mut value);
        print_targets(&value);
        let json = serde_json::to_string(&value).expect("serialize the report");
        std::fs::write(path, &json).expect("write the json");
        std::fs::write(path.with_extension("html"), page(&json)).expect("write the html");
        eprintln!("rendered {}", path.with_extension("html").display());
        return;
    }
    let archetypes: Vec<&'static Archetype> = cb::ARCHETYPES.iter()
        .filter(|a| args.only.as_ref().is_none_or(|keys| keys.iter().any(|k| k == a.key)))
        .collect();
    assert!(!archetypes.is_empty(), "--only matched no archetype");
    // Each tier's buildable archetypes (indices into `archetypes`) and fleets.
    let built: Vec<Vec<(usize, Fleet)>> = args.tiers.iter().map(|tier| {
        archetypes.iter().enumerate()
            .filter_map(|(i, a)| cb::build_fleet(a, tier.budget).map(|f| (i, f)))
            .collect()
    }).collect();
    let mut jobs = Vec::new();
    for (t, fleets) in built.iter().enumerate() {
        for (i, (_, a)) in fleets.iter().enumerate() {
            for (j, (_, d)) in fleets.iter().enumerate() {
                let weight = f64::from(a.hulls() + d.hulls()).powf(1.5);
                jobs.push(Job { tier: t, attacker: i, defender: j, weight });
            }
        }
    }
    // Biggest fights first, so no thread is left with a long one at the end.
    jobs.sort_by(|x, y| y.weight.total_cmp(&x.weight));
    let fights = jobs.len() as u64 * u64::from(args.seeds);
    eprintln!("{} archetypes, {} tiers, {} cells, {fights} fights on {} threads",
        archetypes.len(), args.tiers.len(), jobs.len(), args.threads);

    let started = Instant::now();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Cell>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|s| {
        for _ in 0..args.threads.max(1) {
            s.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(k) else { break };
                let tier = &args.tiers[job.tier];
                let (ai, a) = &built[job.tier][job.attacker];
                let (di, d) = &built[job.tier][job.defender];
                let cell = cb::run_cell((archetypes[*ai], a), (archetypes[*di], d), tier.budget, args.seeds);
                results.lock().unwrap()[k] = Some(cell);
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n.is_multiple_of((jobs.len() / 20).max(1)) || n == jobs.len() {
                    eprintln!("  {:3.0}%  {:6.1}s", 100.0 * n as f64 / jobs.len() as f64, started.elapsed().as_secs_f64());
                }
            });
        }
    });
    let mut results: Vec<Option<Cell>> = results.into_inner().unwrap();

    // Catch and hold, on the same fleets.
    let index = |t: usize, key: &str| built[t].iter().position(|(i, _)| archetypes[*i].key == key);
    let mut escape_jobs = Vec::new();
    for t in 0..built.len() {
        for runner in cb::RUNNERS {
            for hunter in cb::HUNTERS {
                if let (Some(r), Some(h)) = (index(t, runner), index(t, hunter)) {
                    escape_jobs.push((t, r, h));
                }
            }
        }
    }
    let next = AtomicUsize::new(0);
    let escapes: Mutex<Vec<Option<EscapeCell>>> = Mutex::new(vec![None; escape_jobs.len()]);
    std::thread::scope(|s| {
        for _ in 0..args.threads.max(1) {
            s.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(&(t, r, h)) = escape_jobs.get(k) else { break };
                let (ri, runner) = &built[t][r];
                let (hi, hunter) = &built[t][h];
                let cell = cb::run_escapes((archetypes[*hi], hunter), (archetypes[*ri], runner), args.tiers[t].budget, args.seeds);
                escapes.lock().unwrap()[k] = Some(cell);
            });
        }
    });
    let escapes = escapes.into_inner().unwrap();
    eprintln!("  catch and hold: {} cells in {:.1}s", escape_jobs.len(), started.elapsed().as_secs_f64());

    // Fold each tier's cells into its table.
    let mut grids: Vec<Vec<Vec<Cell>>> = built.iter()
        .map(|f| vec![vec![Cell::default(); f.len()]; f.len()]).collect();
    for (k, job) in jobs.iter().enumerate() {
        grids[job.tier][job.attacker][job.defender] = results[k].take().expect("every job ran");
    }
    let escape_tables: Vec<Escapes> = (0..args.tiers.len()).map(|t| {
        let runners: Vec<usize> = cb::RUNNERS.iter().filter_map(|k| index(t, k)).collect();
        let hunters: Vec<usize> = cb::HUNTERS.iter().filter_map(|k| index(t, k)).collect();
        let cells = runners.iter().map(|&r| hunters.iter().map(|&h| {
            let k = escape_jobs.iter().position(|&j| j == (t, r, h)).expect("every escape ran");
            let mut c = escapes[k].expect("every escape ran");
            for v in [&mut c.escaped, &mut c.held, &mut c.lost] { *v = (*v * 1e4).round() / 1e4; }
            c
        }).collect()).collect();
        Escapes { runners: runners.iter().map(|&r| built[t][r].0).collect(), hunters: hunters.iter().map(|&h| built[t][h].0).collect(), cells }
    }).collect();
    let tiers: Vec<TierReport> = args.tiers.iter().zip(built).zip(grids).zip(escape_tables).map(|(((tier, fleets), cells), escapes)| {
        let mut table = cb::table(&cells);
        round_table(&mut table);
        let cells = cells.into_iter().map(|row| row.into_iter().map(|mut c| {
            c.lost = [c.lost[0].round(), c.lost[1].round()];
            c
        }).collect()).collect();
        let (indices, fleets): (Vec<usize>, Vec<Fleet>) = fleets.into_iter().map(|(i, mut f)| {
            f.cost = f.cost.round();
            for s in &mut f.squadrons { s.unit = s.unit.round(); }
            (i, f)
        }).unzip();
        TierReport { key: tier.key, name: tier.name, budget: tier.budget, archetypes: indices, fleets, cells, table, escapes }
    }).collect();

    let rules = sim::tactical::replay::RULES_VERSION;
    let report = Report {
        rules,
        generated: utc_now(),
        seeds: args.seeds,
        step_cap: sim::tactical::MAX_PROJ_STEPS,
        hulls: hull_costs(),
        modules: module_costs(),
        archetypes: archetypes.iter().map(|a| ArchetypeView { key: a.key, name: a.name, hull: a.hull, weapon: a.weapon }).collect(),
        tiers,
        baseline: args.baseline.as_ref().map(baseline),
    };
    summarize(&report);
    let mut value = serde_json::to_value(&report).expect("serialize the report");
    derive(&mut value);
    print_targets(&value);
    std::fs::create_dir_all(&args.out).expect("create --out");
    let json = serde_json::to_string(&value).expect("serialize the report");
    let stem = args.out.join(format!("combat-balance-v{rules}"));
    std::fs::write(stem.with_extension("json"), &json).expect("write the json");
    std::fs::write(stem.with_extension("html"), page(&json)).expect("write the html");
    eprintln!("wrote {}.json and .html in {:.1}s", stem.display(), started.elapsed().as_secs_f64());
}

/// The report page with the numbers inlined (it needs no other file).
fn page(json: &str) -> String {
    assert!(TEMPLATE.contains(DATA_SLOT), "the template carries the data slot");
    TEMPLATE.replace(DATA_SLOT, &json.replace("</", "<\\/"))
}

/// The summaries computed from saved numbers (so `--render` can add them to
/// an older report without fighting again): the hull classes of the pure
/// archetypes, lightest first, each pair of classes as a game over their
/// weapon fits (`hull_games[row][column]`: what the row class can guarantee
/// when both sides fit their best; null where a class sits the tier out), and
/// each tier's targets (`cb::targets`).
fn derive(report: &mut serde_json::Value) {
    let hulls: Vec<String> = report["archetypes"].as_array().expect("archetypes").iter()
        .map(|a| a["hull"].as_str().unwrap().to_string()).collect();
    let keys: Vec<String> = report["archetypes"].as_array().unwrap().iter()
        .map(|a| a["key"].as_str().unwrap().to_string()).collect();
    let names: Vec<String> = report["archetypes"].as_array().unwrap().iter()
        .map(|a| a["name"].as_str().unwrap().to_string()).collect();
    let mut classes: Vec<String> = Vec::new();
    for h in hulls.iter().filter(|h| *h != "doctrine") {
        if !classes.contains(h) { classes.push(h.clone()); }
    }
    for tier in report["tiers"].as_array_mut().expect("tiers") {
        let local: Vec<&str> = tier["archetypes"].as_array().unwrap().iter()
            .map(|i| hulls[i.as_u64().unwrap() as usize].as_str()).collect();
        let score: Vec<Vec<f64>> = tier["table"]["score"].as_array().unwrap().iter()
            .map(|row| row.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()).collect();
        let members = |class: &str| -> Vec<usize> { (0..local.len()).filter(|&i| local[i] == class).collect() };
        let games: Vec<Vec<Option<f64>>> = classes.iter().map(|row| classes.iter().map(|col| {
            let (rows, cols) = (members(row), members(col));
            if rows.is_empty() || cols.is_empty() { return None; }
            let payoff: Vec<Vec<f64>> = rows.iter().map(|&i| cols.iter().map(|&j| score[i][j]).collect()).collect();
            Some((cb::solve_game(&payoff).0 * 1e4).round() / 1e4)
        }).collect()).collect();
        let order: Vec<usize> = tier["archetypes"].as_array().unwrap().iter().map(|i| i.as_u64().unwrap() as usize).collect();
        let table: Table = serde_json::from_value(tier["table"].clone()).expect("a matchup table");
        let checks = cb::targets(&order.iter().map(|&i| keys[i].as_str()).collect::<Vec<_>>(),
            &order.iter().map(|&i| names[i].as_str()).collect::<Vec<_>>(), &table, &classes, &games);
        tier["hull_games"] = serde_json::json!(games);
        tier["targets"] = serde_json::json!(checks);
    }
    report["hull_classes"] = serde_json::json!(classes);
}

/// Each tier's targets, failures first, to the terminal.
fn print_targets(report: &serde_json::Value) {
    for tier in report["tiers"].as_array().expect("tiers") {
        let checks = tier["targets"].as_array().map(Vec::as_slice).unwrap_or_default();
        let passed = checks.iter().filter(|c| c["pass"].as_bool() == Some(true)).count();
        eprintln!("\n{} targets: {passed} of {} met", tier["name"].as_str().unwrap_or("?"), checks.len());
        let mut sorted: Vec<&serde_json::Value> = checks.iter().collect();
        sorted.sort_by_key(|c| c["pass"].as_bool() == Some(true));
        for c in sorted {
            let value = match c["kind"].as_str() {
                Some("mix") => format!("{}", c["value"].as_f64().unwrap_or(0.0)),
                _ => format!("{:.0}%", c["value"].as_f64().unwrap_or(0.0) * 100.0),
            };
            let trade = c["trade"].as_f64().map(|t| format!(", trade {:.0}%", t * 100.0)).unwrap_or_default();
            eprintln!("  {} {:46} {value}{trade}  (goal {})", if c["pass"].as_bool() == Some(true) { "pass" } else { "MISS" },
                c["label"].as_str().unwrap_or(""), c["goal"].as_str().unwrap_or(""));
        }
    }
}

/// Four decimals is far below the sampling noise and keeps the files small.
fn round_table(t: &mut Table) {
    let r = |x: &mut f64| *x = (*x * 1e4).round() / 1e4;
    for m in [&mut t.score, &mut t.low, &mut t.high, &mut t.attacking, &mut t.steps, &mut t.undecided] {
        m.iter_mut().flatten().for_each(r);
    }
    t.trade.iter_mut().flatten().flatten().for_each(r);
    t.field.iter_mut().for_each(r);
    t.mix.iter_mut().for_each(r);
}

/// Each tier's archetypes in order of their field score, to the terminal.
fn summarize(report: &Report) {
    for tier in &report.tiers {
        eprintln!("\n{} ({} credits): field score, equilibrium share", tier.name, tier.budget);
        let mut order: Vec<usize> = (0..tier.archetypes.len()).collect();
        order.sort_by(|&x, &y| tier.table.field[y].total_cmp(&tier.table.field[x]));
        for i in order {
            let a = &report.archetypes[tier.archetypes[i]];
            eprintln!("  {:28} {:5.1}%  {:5.1}%", a.name, tier.table.field[i] * 100.0, tier.table.mix[i] * 100.0);
        }
    }
}

/// An earlier report's scores, keyed by archetype ids so rows can come and go.
fn baseline(path: &PathBuf) -> Baseline {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let old: serde_json::Value = serde_json::from_str(&text).expect("a combat-balance json report");
    let keys: Vec<&str> = old["archetypes"].as_array().expect("archetypes").iter()
        .map(|a| a["key"].as_str().unwrap()).collect();
    let mut scores = BTreeMap::new();
    for tier in old["tiers"].as_array().expect("tiers") {
        let idx: Vec<&str> = tier["archetypes"].as_array().unwrap().iter()
            .map(|i| keys[i.as_u64().unwrap() as usize]).collect();
        let mut map = BTreeMap::new();
        for (i, row) in tier["table"]["score"].as_array().unwrap().iter().enumerate() {
            for (j, v) in row.as_array().unwrap().iter().enumerate() {
                map.insert(format!("{}|{}", idx[i], idx[j]), v.as_f64().unwrap());
            }
        }
        scores.insert(tier["key"].as_str().unwrap().to_string(), map);
    }
    Baseline {
        rules: old["rules"].as_u64().unwrap_or(0),
        generated: old["generated"].as_str().unwrap_or("").to_string(),
        seeds: old["seeds"].as_u64().unwrap_or(0),
        scores,
    }
}

/// "2026-10-03 14:05 UTC", without a date library.
fn utc_now() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02} UTC", rem / 3_600, rem % 3_600 / 60)
}

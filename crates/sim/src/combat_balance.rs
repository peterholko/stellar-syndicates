//! Equal-cost combat balance, measured on the real battle engine.
//!
//! Prices every warship hull and module in one currency (credits at the
//! reference market prices, [`crate::market::base_price`]), builds archetype
//! fleets to a budget, and fights every pairing in both roles on the kernel
//! the in-game battle estimate uses ([`crate::tactical::simulate_engagement`],
//! current rules, default missions, so Automatic formations). Folded over both
//! roles, the result is a symmetric matchup table: who beats whom at equal
//! cost, how cheaply, how fast, and the equilibrium mix (the archetypes a
//! player who knew the table would still field).
//!
//! Measurement only: nothing here changes a rule and nothing in the game reads
//! it. The runner is `cargo run --release -p sim --example combat_balance`; see
//! `docs/combat-balance.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::build::{BuildKind, Recipe, module_recipe, recipe_for};
use crate::cargo::Commodity;
use crate::config::TICK_HZ;
use crate::ids::EntityId;
use crate::market::base_price;
use crate::module::{Loadout, ModuleKind};
use crate::ship::{Ship, ShipKind};
use crate::math::Vec2;
use crate::tactical::{
    MAX_COMBATANTS_PER_SIDE, MAX_PROJ_STEPS, ProjSetup, SideMods, SimOutcome, TacticalState, WITHDRAW_EXIT_RADIUS,
    simulate_engagement,
};

// --- Cost: one currency ------------------------------------------------------

/// The warship ladder, lightest first.
pub const WARSHIPS: [ShipKind; 7] = [
    ShipKind::Raider,
    ShipKind::Corvette,
    ShipKind::Destroyer,
    ShipKind::Cruiser,
    ShipKind::Battleship,
    ShipKind::Dreadnought,
    ShipKind::Titan,
];

/// The modules that change a fight.
pub const COMBAT_MODULES: [ModuleKind; 8] = [
    ModuleKind::MassDriver,
    ModuleKind::TorpedoRack,
    ModuleKind::PointDefenseScreen,
    ModuleKind::ReflectivePlating,
    ModuleKind::WhippleArmor,
    ModuleKind::StasisWeb,
    ModuleKind::WarpStabilizer,
    ModuleKind::InterdictionField,
];

/// The name players see.
pub fn hull_label(kind: ShipKind) -> &'static str {
    match kind {
        ShipKind::Raider => "Interceptor",
        ShipKind::Corvette => "Corvette",
        ShipKind::Destroyer => "Destroyer",
        ShipKind::Cruiser => "Cruiser",
        ShipKind::Battleship => "Battleship",
        ShipKind::Dreadnought => "Dreadnought",
        ShipKind::Titan => "Titan",
        _ => "Civilian",
    }
}

/// A recipe's worth in credits at the reference prices.
pub fn recipe_value(recipe: &Recipe) -> f64 {
    recipe.costs.iter().map(|&(good, units)| base_price(good) * units).sum()
}

/// What a bare warship hull costs to build.
pub fn hull_value(kind: ShipKind) -> f64 {
    recipe_value(recipe_for(BuildKind::Ship { ship: kind }))
}

pub fn module_value(module: ModuleKind) -> f64 {
    recipe_value(module_recipe(module))
}

/// A fitted ship: its hull plus every module it carries.
pub fn ship_value(kind: ShipKind, fit: &Loadout) -> f64 {
    hull_value(kind) + fit.modules().iter().map(|&m| module_value(m)).sum::<f64>()
}

/// Nominal yard hours for the hull (its modules are made elsewhere, in parallel).
pub fn hull_build_hours(kind: ShipKind) -> f64 {
    recipe_for(BuildKind::Ship { ship: kind }).build_ticks as f64 / f64::from(TICK_HZ) / 3_600.0
}

/// Crew and marine food, in credits a day at the reference Provisions price.
pub fn upkeep_per_day(kind: ShipKind) -> f64 {
    crate::ship::upkeep_per_sec(kind) * 86_400.0 * base_price(Commodity::Provisions)
}

// --- Archetypes ----------------------------------------------------------------

/// How a part of an archetype is fitted.
#[derive(Debug, Clone, Copy)]
pub enum Fit {
    /// No modules: the stock beam.
    Stock,
    /// As many copies of this weapon as the hull's slots and fitting points
    /// take (duplicates stack linearly, so this is the weapon at full weight).
    Stacked(ModuleKind),
    /// Exactly these modules.
    Modules(&'static [ModuleKind]),
}

impl Fit {
    pub fn loadout(self, kind: ShipKind) -> Loadout {
        match self {
            Fit::Stock => Loadout::default(),
            Fit::Stacked(module) => {
                let mut mods = vec![module];
                while Loadout::new(mods.clone()).validate(kind) {
                    mods.push(module);
                }
                mods.pop();
                Loadout::new(mods)
            }
            Fit::Modules(mods) => Loadout::new(mods.to_vec()),
        }
    }
}

/// One hull type in an archetype and its share of the budget.
#[derive(Debug, Clone, Copy)]
pub struct Part {
    pub kind: ShipKind,
    pub fit: Fit,
    pub share: f64,
}

/// A way to spend a budget on a fleet.
#[derive(Debug, Clone, Copy)]
pub struct Archetype {
    /// Stable id: seeds and baselines key on it, so never reuse one.
    pub key: &'static str,
    pub name: &'static str,
    /// Hull class (`interceptor` … `titan`), or `doctrine` for a mixed or
    /// counter-fitted fleet.
    pub hull: &'static str,
    /// `beam`, `driver`, `torpedo` or `mixed`.
    pub weapon: &'static str,
    /// Dearest hull first; the shares sum to 1.
    pub parts: &'static [Part],
}

macro_rules! pure {
    ($key:literal, $name:literal, $hull:literal, $weapon:literal, $kind:ident, $fit:expr) => {
        Archetype {
            key: $key,
            name: $name,
            hull: $hull,
            weapon: $weapon,
            parts: &[Part { kind: ShipKind::$kind, fit: $fit, share: 1.0 }],
        }
    };
}

use ModuleKind::{
    MassDriver as DRIVER, PointDefenseScreen as PD, ReflectivePlating as REFLECTIVE, StasisWeb as WEB,
    TorpedoRack as TORPEDO, WarpStabilizer as STABILIZER, WhippleArmor as WHIPPLE,
};

/// Every hull class with each weapon at full weight (stock beams carry no
/// module), then fleets built around a counter or a mix.
pub const ARCHETYPES: &[Archetype] = &[
    pure!("interceptor_beam", "Interceptors · beams", "interceptor", "beam", Raider, Fit::Stock),
    pure!("interceptor_driver", "Interceptors · drivers", "interceptor", "driver", Raider, Fit::Stacked(DRIVER)),
    pure!("interceptor_torpedo", "Interceptors · torpedoes", "interceptor", "torpedo", Raider, Fit::Stacked(TORPEDO)),
    pure!("corvette_beam", "Corvettes · beams", "corvette", "beam", Corvette, Fit::Stock),
    pure!("corvette_driver", "Corvettes · drivers", "corvette", "driver", Corvette, Fit::Stacked(DRIVER)),
    pure!("corvette_torpedo", "Corvettes · torpedoes", "corvette", "torpedo", Corvette, Fit::Stacked(TORPEDO)),
    pure!("destroyer_beam", "Destroyers · beams", "destroyer", "beam", Destroyer, Fit::Stock),
    pure!("destroyer_driver", "Destroyers · drivers", "destroyer", "driver", Destroyer, Fit::Stacked(DRIVER)),
    pure!("destroyer_torpedo", "Destroyers · torpedoes", "destroyer", "torpedo", Destroyer, Fit::Stacked(TORPEDO)),
    pure!("cruiser_beam", "Cruisers · beams", "cruiser", "beam", Cruiser, Fit::Stock),
    pure!("cruiser_driver", "Cruisers · drivers", "cruiser", "driver", Cruiser, Fit::Stacked(DRIVER)),
    pure!("cruiser_torpedo", "Cruisers · torpedoes", "cruiser", "torpedo", Cruiser, Fit::Stacked(TORPEDO)),
    pure!("battleship_beam", "Battleships · beams", "battleship", "beam", Battleship, Fit::Stock),
    pure!("battleship_driver", "Battleships · drivers", "battleship", "driver", Battleship, Fit::Stacked(DRIVER)),
    pure!("battleship_torpedo", "Battleships · torpedoes", "battleship", "torpedo", Battleship, Fit::Stacked(TORPEDO)),
    pure!("dreadnought_beam", "Dreadnoughts · beams", "dreadnought", "beam", Dreadnought, Fit::Stock),
    pure!("dreadnought_driver", "Dreadnoughts · drivers", "dreadnought", "driver", Dreadnought, Fit::Stacked(DRIVER)),
    pure!("dreadnought_torpedo", "Dreadnoughts · torpedoes", "dreadnought", "torpedo", Dreadnought, Fit::Stacked(TORPEDO)),
    pure!("titan_beam", "Titans · beams", "titan", "beam", Titan, Fit::Stock),
    pure!("titan_driver", "Titans · drivers", "titan", "driver", Titan, Fit::Stacked(DRIVER)),
    pure!("titan_torpedo", "Titans · torpedoes", "titan", "torpedo", Titan, Fit::Stacked(TORPEDO)),
    // Torpedo destroyers and interceptors together: the capital killer.
    Archetype {
        key: "wolfpack",
        name: "Torpedo wolfpack",
        hull: "doctrine",
        weapon: "torpedo",
        parts: &[
            Part { kind: ShipKind::Destroyer, fit: Fit::Stacked(TORPEDO), share: 0.5 },
            Part { kind: ShipKind::Raider, fit: Fit::Stacked(TORPEDO), share: 0.5 },
        ],
    },
    // Driver battleships behind point-defense corvettes: the torpedo answer.
    Archetype {
        key: "screened_battleships",
        name: "Screened battleships",
        hull: "doctrine",
        weapon: "mixed",
        parts: &[
            Part { kind: ShipKind::Battleship, fit: Fit::Stacked(DRIVER), share: 0.65 },
            Part { kind: ShipKind::Corvette, fit: Fit::Modules(&[PD]), share: 0.35 },
        ],
    },
    // Beam cruisers spending their free slots on both armors: the gun answer.
    Archetype {
        key: "armoured_cruisers",
        name: "Armoured cruisers",
        hull: "doctrine",
        weapon: "beam",
        parts: &[Part { kind: ShipKind::Cruiser, fit: Fit::Modules(&[REFLECTIVE, WHIPPLE]), share: 1.0 }],
    },
    // The classic brawler fit: a driver and Whipple armor, exactly 5 points.
    Archetype {
        key: "brawler_corvettes",
        name: "Brawler corvettes",
        hull: "doctrine",
        weapon: "driver",
        parts: &[Part { kind: ShipKind::Corvette, fit: Fit::Modules(&[DRIVER, WHIPPLE]), share: 1.0 }],
    },
    // Rules v11 doctrines. Driver battleships with destroyer escorts whose
    // light batteries and webs answer small ships.
    Archetype {
        key: "escorted_battleships",
        name: "Escorted battleships",
        hull: "doctrine",
        weapon: "mixed",
        parts: &[
            Part { kind: ShipKind::Battleship, fit: Fit::Stacked(DRIVER), share: 0.5 },
            Part { kind: ShipKind::Destroyer, fit: Fit::Modules(&[WEB]), share: 0.5 },
        ],
    },
    // Battleships that give up a driver for a warp stabilizer, so a lone
    // tackler can't hold them.
    Archetype {
        key: "stabilized_battleships",
        name: "Stabilized battleships",
        hull: "doctrine",
        weapon: "driver",
        parts: &[Part { kind: ShipKind::Battleship, fit: Fit::Modules(&[DRIVER, DRIVER, DRIVER, STABILIZER]), share: 1.0 }],
    },
    // Interceptors that slow what they fight: stock beams and a stasis web.
    Archetype {
        key: "web_interceptors",
        name: "Webbing interceptors",
        hull: "doctrine",
        weapon: "beam",
        parts: &[Part { kind: ShipKind::Raider, fit: Fit::Modules(&[WEB]), share: 1.0 }],
    },
    // Rules v12: driver battleships under point-defense dreadnoughts, whose
    // heavy screens cover the whole formation against torpedoes.
    Archetype {
        key: "dreadnought_screen",
        name: "Dreadnought screen",
        hull: "doctrine",
        weapon: "driver",
        parts: &[
            Part { kind: ShipKind::Dreadnought, fit: Fit::Modules(&[PD, DRIVER, DRIVER, DRIVER, DRIVER]), share: 0.5 },
            Part { kind: ShipKind::Battleship, fit: Fit::Stacked(DRIVER), share: 0.5 },
        ],
    },
    // A combined fleet: capitals, line, screens and interceptors.
    Archetype {
        key: "battle_group",
        name: "Battle group",
        hull: "doctrine",
        weapon: "mixed",
        parts: &[
            Part { kind: ShipKind::Battleship, fit: Fit::Stacked(DRIVER), share: 0.4 },
            Part { kind: ShipKind::Cruiser, fit: Fit::Stock, share: 0.25 },
            Part { kind: ShipKind::Corvette, fit: Fit::Modules(&[PD]), share: 0.1 },
            Part { kind: ShipKind::Raider, fit: Fit::Stock, share: 0.25 },
        ],
    },
];

/// A fight size: every archetype is built to this many credits.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Tier {
    pub key: &'static str,
    pub name: &'static str,
    pub budget: f64,
}

/// Picked so whole hulls land close to every budget: a skirmish fields two
/// battleships or one dreadnought (no titan), a fleet action one titan, a
/// major battle three titans or about 180 interceptors.
pub const TIERS: [Tier; 3] = [
    Tier { key: "skirmish", name: "Skirmish", budget: 55_000.0 },
    Tier { key: "fleet", name: "Fleet action", budget: 115_000.0 },
    Tier { key: "major", name: "Major battle", budget: 340_000.0 },
];

/// An archetype that can't come within this share of a budget in whole hulls
/// sits that tier out.
pub const BUDGET_TOLERANCE: f64 = 0.15;

/// One hull type in a built fleet.
#[derive(Debug, Clone, Serialize)]
pub struct Squadron {
    pub kind: ShipKind,
    /// Loadout key, `""` for the stock beam.
    pub fit: String,
    pub count: u32,
    /// Credits per ship.
    pub unit: f64,
}

/// An archetype built to a budget.
#[derive(Debug, Clone, Serialize)]
pub struct Fleet {
    pub squadrons: Vec<Squadron>,
    pub cost: f64,
}

impl Fleet {
    /// These hulls as one fleet, so it can hold a formation.
    pub fn ships(&self, fleet: u64) -> Vec<(EntityId, Ship)> {
        let mut out = Vec::new();
        for s in &self.squadrons {
            for _ in 0..s.count {
                out.push((EntityId(fleet), Ship::new(out.len() as u32, s.kind, Loadout::from_key(&s.fit))));
            }
        }
        out
    }

    /// What these losses cost (each kind flies one fit per archetype).
    pub fn value_of(&self, losses: &BTreeMap<ShipKind, u32>) -> f64 {
        losses.iter().map(|(kind, &n)| {
            let unit = self.squadrons.iter().find(|s| s.kind == *kind).map_or(0.0, |s| s.unit);
            unit * f64::from(n)
        }).sum()
    }

    pub fn hulls(&self) -> u32 {
        self.squadrons.iter().map(|s| s.count).sum()
    }
}

/// Spend `budget` on `arch`: dearest part first, each part rounded to whole
/// hulls with its rounding carried into the next, so the total lands near the
/// budget. `None` when the fleet misses the budget by more than
/// [`BUDGET_TOLERANCE`] (a titan in a skirmish), a part rounds away, or a side
/// would overflow the battle roster.
pub fn build_fleet(arch: &Archetype, budget: f64) -> Option<Fleet> {
    let mut squadrons = Vec::new();
    let mut carry = 0.0;
    for part in arch.parts {
        let fit = part.fit.loadout(part.kind);
        let unit = ship_value(part.kind, &fit);
        let want = budget * part.share + carry;
        let count = (want / unit).round().max(0.0);
        carry = want - count * unit;
        if count < 1.0 {
            return None;
        }
        squadrons.push(Squadron { kind: part.kind, fit: fit.key(), count: count as u32, unit });
    }
    let fleet = Fleet { cost: squadrons.iter().map(|s| s.unit * f64::from(s.count)).sum(), squadrons };
    let off = (fleet.cost / budget - 1.0).abs();
    (off <= BUDGET_TOLERANCE && (fleet.hulls() as usize) < MAX_COMBATANTS_PER_SIDE).then_some(fleet)
}

// --- Fights --------------------------------------------------------------------

/// How one fight ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    AttackerWon,
    DefenderWon,
    BothDestroyed,
    /// Both sides still flying at the engine's step cap.
    Undecided,
}

/// One fight's result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rollout {
    pub ending: Ending,
    pub steps: u64,
    /// Value lost by [attacker, defender].
    pub lost: [f64; 2],
}

const ATTACKER_FLEET: u64 = 100;
const DEFENDER_FLEET: u64 = 500;

/// `attacker` closing on `defender` in open space: no platforms, no raid,
/// fought to the end (or the step cap).
pub fn engagement(attacker: &Fleet, defender: &Fleet) -> ProjSetup {
    ProjSetup {
        a: attacker.ships(ATTACKER_FLEET),
        d: defender.ships(DEFENDER_FLEET),
        platform_tiers: 0,
        raid: false,
        a_retreat: None,
        d_retreat: None,
    }
}

/// One fight on the current rules.
pub fn fight(setup: &ProjSetup, attacker: &Fleet, defender: &Fleet, seed: u64) -> Rollout {
    classify(&simulate_engagement(setup, seed), attacker, defender)
}

fn classify(out: &SimOutcome, attacker: &Fleet, defender: &Fleet) -> Rollout {
    let flying = |m: &BTreeMap<ShipKind, u32>| m.values().sum::<u32>() > 0;
    let (a, d) = (flying(&out.a_survivors), flying(&out.d_survivors));
    let ending = match (out.a_won, a, d) {
        (true, ..) => Ending::AttackerWon,
        (false, false, true) => Ending::DefenderWon,
        (false, false, false) => Ending::BothDestroyed,
        _ => Ending::Undecided,
    };
    Rollout { ending, steps: out.steps, lost: [attacker.value_of(&out.a_losses), defender.value_of(&out.d_losses)] }
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The seed of fight `k` of `attacker` against `defender` at `budget`. Keyed by
/// archetype ids, so adding an archetype never reshuffles another cell.
pub fn fight_seed(budget: f64, attacker: &str, defender: &str, k: u32) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in attacker.bytes().chain([0xFF]).chain(defender.bytes()) {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
    }
    splitmix64(h ^ splitmix64(budget as u64) ^ u64::from(k).wrapping_mul(0xD1B5_4A32_D192_ED03))
}

/// Every fight of one archetype attacking another.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Cell {
    pub runs: u32,
    pub attacker_won: u32,
    pub defender_won: u32,
    pub both_destroyed: u32,
    pub undecided: u32,
    /// Value lost by [attacker, defender], summed over the runs.
    pub lost: [f64; 2],
    #[serde(skip)]
    pub steps: Vec<u64>,
}

impl Cell {
    pub fn add(&mut self, r: &Rollout) {
        self.runs += 1;
        match r.ending {
            Ending::AttackerWon => self.attacker_won += 1,
            Ending::DefenderWon => self.defender_won += 1,
            Ending::BothDestroyed => self.both_destroyed += 1,
            Ending::Undecided => self.undecided += 1,
        }
        self.lost[0] += r.lost[0];
        self.lost[1] += r.lost[1];
        self.steps.push(r.steps);
    }

    /// The attacker's points: a win 1, a mutual kill or an undecided fight ½.
    pub fn attacker_points(&self) -> f64 {
        f64::from(self.attacker_won) + 0.5 * f64::from(self.both_destroyed + self.undecided)
    }

    pub fn median_steps(&self) -> f64 {
        median(&self.steps)
    }
}

/// `runs` fights of `attacker` closing on `defender`.
pub fn run_cell(attacker: (&Archetype, &Fleet), defender: (&Archetype, &Fleet), budget: f64, runs: u32) -> Cell {
    let setup = engagement(attacker.1, defender.1);
    let mut cell = Cell::default();
    for k in 0..runs {
        let seed = fight_seed(budget, attacker.0.key, defender.0.key, k);
        cell.add(&fight(&setup, attacker.1, defender.1, seed));
    }
    cell
}

// --- Catch and hold ----------------------------------------------------------------

/// Fleets that try to leave in the catch-and-hold test...
pub const RUNNERS: [&str; 6] = [
    "cruiser_beam", "battleship_driver", "stabilized_battleships", "escorted_battleships", "titan_driver",
    "interceptor_beam",
];
/// ...and the fleets hunting them.
pub const HUNTERS: [&str; 6] = [
    "interceptor_beam", "web_interceptors", "interceptor_torpedo", "destroyer_beam", "cruiser_beam",
    "battleship_driver",
];

/// One escape attempt, as shares of the runner fleet's value.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Escape {
    /// Alive past the disengage edge.
    pub escaped: f64,
    /// Alive but still inside it at the step cap (held).
    pub held: f64,
    pub lost: f64,
    pub steps: u64,
}

const ESCAPE_BATTLE_ID: u64 = 0x00E5_CA9E;

/// `runner` withdraws at once while `hunter` closes from standoff, on the
/// current rules; ends when the runner is gone, out, or at the step cap.
pub fn escape(hunter: &Fleet, runner: &Fleet, seed: u64) -> Escape {
    let mut state = TacticalState::open_current(seed, ESCAPE_BATTLE_ID, &hunter.ships(ATTACKER_FLEET),
        &runner.ships(DEFENDER_FLEET), 0, 0.0, Vec2::new(1.0, 0.0));
    state.order_withdraw(1);
    let mut steps = 0;
    while steps < MAX_PROJ_STEPS && state.alive(1) > 0 && !state.side_withdrawn(1) {
        state.step(false, [SideMods::default(); 2]);
        steps += 1;
    }
    let unit = |kind: ShipKind| runner.squadrons.iter().find(|s| s.kind == kind).map_or(0.0, |s| s.unit);
    let (mut escaped, mut held) = (0.0, 0.0);
    for c in state.combatants.iter().filter(|c| c.side == 1 && !c.platform) {
        if c.pos.length() >= WITHDRAW_EXIT_RADIUS { escaped += unit(c.kind) } else { held += unit(c.kind) }
    }
    let total = runner.cost.max(1.0);
    Escape { escaped: escaped / total, held: held / total, lost: (1.0 - (escaped + held) / total).max(0.0), steps }
}

/// The mean of `runs` escapes of `runner` from `hunter`.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct EscapeCell {
    pub runs: u32,
    pub escaped: f64,
    pub held: f64,
    pub lost: f64,
    pub median_steps: f64,
}

pub fn run_escapes(hunter: (&Archetype, &Fleet), runner: (&Archetype, &Fleet), budget: f64, runs: u32) -> EscapeCell {
    let mut cell = EscapeCell { runs, ..Default::default() };
    let mut steps = Vec::new();
    for k in 0..runs {
        let seed = fight_seed(budget, hunter.0.key, runner.0.key, k) ^ ESCAPE_BATTLE_ID;
        let e = escape(hunter.1, runner.1, seed);
        cell.escaped += e.escaped;
        cell.held += e.held;
        cell.lost += e.lost;
        steps.push(e.steps);
    }
    let n = f64::from(runs.max(1));
    cell.escaped /= n;
    cell.held /= n;
    cell.lost /= n;
    cell.median_steps = median(&steps);
    cell
}

// --- The table -------------------------------------------------------------------

/// One fight size's matchups, both roles folded together. Every square matrix
/// is indexed `[row][column]` over the tier's archetypes; "row against column".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    /// The row's share of the points over both roles (a win 1, a mutual kill
    /// or an undecided fight ½): `score[i][j] + score[j][i] == 1`.
    pub score: Vec<Vec<f64>>,
    /// The 95% Wilson interval on `score`.
    pub low: Vec<Vec<f64>>,
    pub high: Vec<Vec<f64>>,
    /// The row's share of all the value destroyed between the two (½ is an
    /// even trade); `None` when neither lost a hull.
    pub trade: Vec<Vec<Option<f64>>>,
    /// The row's attacking win rate (the role split behind `score`).
    pub attacking: Vec<Vec<f64>>,
    /// Median fight length in steps, row attacking column.
    pub steps: Vec<Vec<f64>>,
    /// Share of the pair's fights (either role) still undecided at the cap.
    pub undecided: Vec<Vec<f64>>,
    /// Mean score against every other archetype: a uniform field.
    pub field: Vec<f64>,
    /// The equilibrium mix ([`equilibrium`]).
    pub mix: Vec<f64>,
}

/// Fold the cells (`cells[i][j]`: `i` attacking `j`) into the table.
pub fn table(cells: &[Vec<Cell>]) -> Table {
    let n = cells.len();
    let square = |f: &dyn Fn(usize, usize) -> f64| -> Vec<Vec<f64>> {
        (0..n).map(|i| (0..n).map(|j| f(i, j)).collect()).collect()
    };
    let runs = |i: usize, j: usize| f64::from(cells[i][j].runs + cells[j][i].runs);
    let points = |i: usize, j: usize| {
        let defending = &cells[j][i];
        cells[i][j].attacker_points() + f64::from(defending.runs) - defending.attacker_points()
    };
    let score = square(&|i, j| if i == j { 0.5 } else { points(i, j) / runs(i, j) });
    let low = square(&|i, j| wilson(points(i, j), runs(i, j)).0);
    let high = square(&|i, j| wilson(points(i, j), runs(i, j)).1);
    let trade = (0..n).map(|i| (0..n).map(|j| {
        let dealt = cells[i][j].lost[1] + cells[j][i].lost[0];
        let taken = cells[i][j].lost[0] + cells[j][i].lost[1];
        (dealt + taken > 0.0).then(|| dealt / (dealt + taken))
    }).collect()).collect();
    let attacking = square(&|i, j| {
        let c = &cells[i][j];
        if c.runs == 0 { 0.0 } else { f64::from(c.attacker_won) / f64::from(c.runs) }
    });
    let steps = square(&|i, j| cells[i][j].median_steps());
    let undecided = square(&|i, j| f64::from(cells[i][j].undecided + cells[j][i].undecided) / runs(i, j).max(1.0));
    let field = (0..n).map(|i| {
        if n < 2 { return 0.5; }
        (0..n).filter(|&j| j != i).map(|j| score[i][j]).sum::<f64>() / (n - 1) as f64
    }).collect();
    let mix = equilibrium(&score);
    Table { score, low, high, trade, attacking, steps, undecided, field, mix }
}

// --- Targets -----------------------------------------------------------------------

/// Counters the rules should produce at equal cost: the first fleet beats the
/// second, scoring at least [`COUNTER_SCORE`] and destroying at least
/// [`COUNTER_TRADE`] of the value the two lose between them.
pub const COUNTERS: [(&str, &str); 9] = [
    ("screened_battleships", "wolfpack"),
    ("armoured_cruisers", "cruiser_beam"),
    ("armoured_cruisers", "cruiser_driver"),
    ("brawler_corvettes", "corvette_driver"),
    ("destroyer_beam", "interceptor_beam"),
    ("escorted_battleships", "interceptor_beam"),
    ("interceptor_beam", "interceptor_torpedo"),
    ("wolfpack", "battleship_driver"),
    ("interceptor_torpedo", "battleship_driver"),
];
pub const COUNTER_SCORE: f64 = 0.70;
pub const COUNTER_TRADE: f64 = 0.60;
/// No fleet type scores above this against the field...
pub const FIELD_MAX: f64 = 0.70;
/// ...at least this many types hold 1% or more of the equilibrium mix...
pub const MIX_MIN: usize = 5;
/// ...at most this share of pairings is decided 90/10 or worse...
pub const BLOWOUT_MAX: f64 = 0.40;
/// ...at most this share of fights is still going at the step cap...
pub const UNDECIDED_MAX: f64 = 0.10;
/// ...and each capital class beats the one below it at equal cost by this
/// much, both sides fielding their best weapon fits (the hull game).
pub const LADDER: [&str; 4] = ["cruiser", "battleship", "dreadnought", "titan"];
pub const LADDER_SCORE: (f64, f64) = (0.55, 0.70);

/// One target, measured on one fight size.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// `counter`, `field`, `mix`, `blowouts`, `undecided` or `ladder`.
    pub kind: &'static str,
    /// What is measured, in words.
    pub label: String,
    pub value: f64,
    /// A counter's value trade.
    pub trade: Option<f64>,
    pub goal: String,
    pub pass: bool,
}

/// Every target that applies to a fight size: `keys` are its archetypes in
/// table order; `classes` and `hull_games` the hull games over its classes
/// (`None` where a class sits the size out).
pub fn targets(keys: &[&str], names: &[&str], table: &Table, classes: &[String], hull_games: &[Vec<Option<f64>>]) -> Vec<Check> {
    let at = |key: &str| keys.iter().position(|k| *k == key);
    let pct = |x: f64| format!("{:.0}%", x * 100.0);
    let mut out = Vec::new();
    for (a, b) in COUNTERS {
        let (Some(i), Some(j)) = (at(a), at(b)) else { continue };
        let (score, trade) = (table.score[i][j], table.trade[i][j]);
        out.push(Check {
            kind: "counter",
            label: format!("{} beat {}", names[i], names[j]),
            value: score,
            trade,
            goal: format!("≥ {} wins, ≥ {} of the value", pct(COUNTER_SCORE), pct(COUNTER_TRADE)),
            pass: score >= COUNTER_SCORE && trade.is_some_and(|t| t >= COUNTER_TRADE),
        });
    }
    let n = keys.len();
    if let Some(top) = (0..n).max_by(|&x, &y| table.field[x].total_cmp(&table.field[y])) {
        out.push(Check {
            kind: "field",
            label: format!("Strongest type: {}", names[top]),
            value: table.field[top],
            trade: None,
            goal: format!("≤ {} against the field", pct(FIELD_MAX)),
            pass: table.field[top] <= FIELD_MAX,
        });
    }
    let mixed = table.mix.iter().filter(|&&x| x >= 0.01).count();
    out.push(Check {
        kind: "mix",
        label: "Types in the equilibrium mix".into(),
        value: mixed as f64,
        trade: None,
        goal: format!("≥ {MIX_MIN}"),
        pass: mixed >= MIX_MIN,
    });
    let pairs: Vec<(usize, usize)> = (0..n).flat_map(|i| (i + 1..n).map(move |j| (i, j))).collect();
    let share = |f: &dyn Fn(usize, usize) -> f64| pairs.iter().map(|&(i, j)| f(i, j)).sum::<f64>() / pairs.len().max(1) as f64;
    let blowouts = share(&|i, j| f64::from(u8::from(table.score[i][j] >= 0.9 || table.score[i][j] <= 0.1)));
    out.push(Check {
        kind: "blowouts",
        label: "Pairings decided 90/10 or worse".into(),
        value: blowouts,
        trade: None,
        goal: format!("≤ {}", pct(BLOWOUT_MAX)),
        pass: blowouts <= BLOWOUT_MAX,
    });
    let undecided = share(&|i, j| table.undecided[i][j]);
    out.push(Check {
        kind: "undecided",
        label: "Fights still going at the step cap".into(),
        value: undecided,
        trade: None,
        goal: format!("≤ {}", pct(UNDECIDED_MAX)),
        pass: undecided <= UNDECIDED_MAX,
    });
    let class = |c: &str| classes.iter().position(|x| x == c);
    for pair in LADDER.windows(2) {
        let (Some(lo), Some(hi)) = (class(pair[0]), class(pair[1])) else { continue };
        let Some(value) = hull_games[hi][lo] else { continue };
        out.push(Check {
            kind: "ladder",
            label: format!("{} over {}", hull_label_of(pair[1]), hull_label_of(pair[0]).to_lowercase()),
            value,
            trade: None,
            goal: format!("{}–{}", pct(LADDER_SCORE.0), pct(LADDER_SCORE.1)),
            pass: (LADDER_SCORE.0..=LADDER_SCORE.1).contains(&value),
        });
    }
    out
}

/// "battleship" → "Battleships".
fn hull_label_of(class: &str) -> String {
    let mut s = class.to_string();
    if let Some(first) = s.get_mut(0..1) { first.make_ascii_uppercase(); }
    s + "s"
}

fn median(v: &[u64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    let m = s.len() / 2;
    if s.len() % 2 == 1 { s[m] as f64 } else { (s[m - 1] + s[m]) as f64 / 2.0 }
}

/// The 95% Wilson score interval for `points` successes in `n` trials (a
/// mutual kill or undecided fight counts as half a success).
pub fn wilson(points: f64, n: f64) -> (f64, f64) {
    if n <= 0.0 {
        return (0.0, 1.0);
    }
    const Z: f64 = 1.959_963_984_540_054;
    let p = (points / n).clamp(0.0, 1.0);
    let z2 = Z * Z;
    let denom = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denom;
    let half = Z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denom;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// The equilibrium mix of the symmetric constant-sum game "both players pick
/// an archetype, `score[i][j]` is what picking `i` against `j` earns": the
/// mixed choice that no opponent can beat on average. An archetype outside the
/// mix is beaten by some blend of the others whatever the opponent does.
pub fn equilibrium(score: &[Vec<f64>]) -> Vec<f64> {
    // In a symmetric game the column player's optimal mix is also the row
    // player's.
    solve_game(score).1
}

/// The zero-sum game where one player picks a row, the other a column, and
/// the row player earns `payoff[i][j]` (in [0, 1]): its value (what the row
/// player can guarantee) and the column player's optimal mix. Exact (simplex,
/// Bland's rule), so a column outside the mix gets exactly 0.
pub fn solve_game(payoff: &[Vec<f64>]) -> (f64, Vec<f64>) {
    let m = payoff.len();
    let n = payoff.first().map_or(0, Vec::len);
    if m == 0 || n == 0 {
        return (0.5, vec![1.0 / n.max(1) as f64; n]);
    }
    // The column player's program with payoffs shifted positive: maximise Σw
    // subject to Σ_j (payoff[i][j] + 1)·w_j ≤ 1 for every row i, w ≥ 0. Then
    // the value is 1/Σw − 1 and the mix w/Σw. Tableau: m rows of
    // [w (n) | slack (m) | rhs].
    const EPS: f64 = 1e-12;
    let (rhs, width) = (n + m, n + m + 1);
    let mut t: Vec<Vec<f64>> = (0..m).map(|i| {
        let mut row = vec![0.0; width];
        for j in 0..n {
            row[j] = payoff[i][j] + 1.0;
        }
        row[n + i] = 1.0;
        row[rhs] = 1.0;
        row
    }).collect();
    let mut cost = vec![0.0; width];
    for c in cost.iter_mut().take(n) {
        *c = -1.0;
    }
    let mut basis: Vec<usize> = (n..n + m).collect();
    for _ in 0..10_000 {
        let Some(enter) = (0..rhs).find(|&j| cost[j] < -EPS) else { break };
        let mut leave: Option<usize> = None;
        for i in 0..m {
            if t[i][enter] > EPS {
                let ratio = t[i][rhs] / t[i][enter];
                let better = match leave {
                    None => true,
                    Some(l) => {
                        let best = t[l][rhs] / t[l][enter];
                        ratio < best - EPS || (ratio <= best + EPS && basis[i] < basis[l])
                    }
                };
                if better {
                    leave = Some(i);
                }
            }
        }
        let Some(row) = leave else { break };
        let pivot = t[row][enter];
        for v in t[row].iter_mut() {
            *v /= pivot;
        }
        let pivot_row = t[row].clone();
        for (i, r) in t.iter_mut().enumerate() {
            if i != row && r[enter].abs() > EPS {
                let f = r[enter];
                for (v, p) in r.iter_mut().zip(&pivot_row) {
                    *v -= f * p;
                }
            }
        }
        let f = cost[enter];
        for (v, p) in cost.iter_mut().zip(&pivot_row) {
            *v -= f * p;
        }
        basis[row] = enter;
    }
    let mut w = vec![0.0; n];
    for (i, &b) in basis.iter().enumerate() {
        // A degenerate basic variable can sit at rounding noise: that is 0.
        if b < n && t[i][rhs] > 1e-12 {
            w[b] = t[i][rhs];
        }
    }
    let total: f64 = w.iter().sum();
    if total <= EPS {
        return (0.5, vec![1.0 / n as f64; n]);
    }
    (1.0 / total - 1.0, w.iter().map(|x| x / total).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archetype(key: &str) -> &'static Archetype {
        ARCHETYPES.iter().find(|a| a.key == key).unwrap()
    }

    #[test]
    fn hull_values_are_their_recipes_at_reference_prices() {
        // The same reference the capital recipes pin (build.rs).
        for (kind, value) in [(ShipKind::Destroyer, 5_756.0), (ShipKind::Cruiser, 10_923.0),
            (ShipKind::Battleship, 23_910.0), (ShipKind::Dreadnought, 49_265.0), (ShipKind::Titan, 106_970.0)] {
            assert!((hull_value(kind) / value - 1.0).abs() < 0.01, "{kind:?}: {}", hull_value(kind));
        }
        let driver = Loadout::new(vec![DRIVER, DRIVER]);
        assert_eq!(ship_value(ShipKind::Raider, &driver), hull_value(ShipKind::Raider) + 2.0 * module_value(DRIVER));
        assert!(upkeep_per_day(ShipKind::Titan) > 100.0 * upkeep_per_day(ShipKind::Raider));
        assert!(hull_build_hours(ShipKind::Titan) > hull_build_hours(ShipKind::Battleship));
    }

    #[test]
    fn a_stacked_fit_carries_as_many_weapons_as_the_hull_takes() {
        let count = |kind, module| Fit::Stacked(module).loadout(kind).len();
        assert_eq!(count(ShipKind::Raider, DRIVER), 2);
        assert_eq!(count(ShipKind::Raider, TORPEDO), 1);
        assert_eq!(count(ShipKind::Destroyer, DRIVER), 3);
        assert_eq!(count(ShipKind::Destroyer, TORPEDO), 2);
        assert_eq!(count(ShipKind::Cruiser, TORPEDO), 4);
        assert_eq!(count(ShipKind::Titan, DRIVER), 6);
    }

    #[test]
    fn every_archetype_is_a_legal_fleet() {
        let mut keys = std::collections::BTreeSet::new();
        for arch in ARCHETYPES {
            assert!(keys.insert(arch.key), "{} is unique", arch.key);
            let shares: f64 = arch.parts.iter().map(|p| p.share).sum();
            assert!((shares - 1.0).abs() < 1e-9, "{}: shares sum to 1", arch.key);
            let mut kinds = std::collections::BTreeSet::new();
            let mut last = f64::INFINITY;
            for part in arch.parts {
                assert!(kinds.insert(part.kind), "{}: one fit per hull type (losses are by kind)", arch.key);
                let fit = part.fit.loadout(part.kind);
                assert!(fit.validate(part.kind), "{}: {} on a {:?}", arch.key, fit.key(), part.kind);
                let unit = ship_value(part.kind, &fit);
                assert!(unit <= last, "{}: dearest part first", arch.key);
                last = unit;
            }
        }
    }

    #[test]
    fn fleets_land_near_their_budget_or_sit_the_tier_out() {
        for tier in TIERS {
            for arch in ARCHETYPES {
                if let Some(fleet) = build_fleet(arch, tier.budget) {
                    assert!((fleet.cost / tier.budget - 1.0).abs() <= BUDGET_TOLERANCE, "{} at {}", arch.key, tier.key);
                }
            }
        }
        assert!(build_fleet(archetype("titan_beam"), TIERS[0].budget).is_none(), "no titan in a skirmish");
        let titans = build_fleet(archetype("titan_beam"), TIERS[2].budget).unwrap();
        assert_eq!(titans.hulls(), 3);
        // Every archetype fights in the two larger tiers.
        for arch in ARCHETYPES {
            assert!(build_fleet(arch, TIERS[1].budget).is_some(), "{} in a fleet action", arch.key);
            assert!(build_fleet(arch, TIERS[2].budget).is_some(), "{} in a major battle", arch.key);
        }
        // A mixed fleet keeps every part, carrying rounding down the ladder.
        let group = build_fleet(archetype("battle_group"), TIERS[1].budget).unwrap();
        assert_eq!(group.squadrons.len(), 4);
        assert!((group.cost / TIERS[1].budget - 1.0).abs() < 0.02, "{}", group.cost);
    }

    #[test]
    fn a_cell_counts_every_fight_and_replays_exactly() {
        let a = archetype("interceptor_beam");
        let d = archetype("cruiser_beam");
        let budget = 25_000.0;
        let (fa, fd) = (build_fleet(a, budget).unwrap(), build_fleet(d, budget).unwrap());
        let cell = run_cell((a, &fa), (d, &fd), budget, 6);
        assert_eq!(cell.runs, 6);
        assert_eq!(cell.attacker_won + cell.defender_won + cell.both_destroyed + cell.undecided, 6);
        assert_eq!(cell.steps.len(), 6);
        assert!(cell.lost[0] > 0.0 || cell.lost[1] > 0.0, "somebody lost a hull");
        let again = run_cell((a, &fa), (d, &fd), budget, 6);
        assert_eq!((again.attacker_won, again.steps.clone(), again.lost), (cell.attacker_won, cell.steps.clone(), cell.lost));
        assert_ne!(fight_seed(budget, a.key, d.key, 0), fight_seed(budget, d.key, a.key, 0), "roles have their own seeds");
    }

    #[test]
    fn interceptors_hold_what_cruisers_cannot_catch_and_torpedoes_finish_it() {
        let budget = TIERS[0].budget;
        let runner = build_fleet(archetype("battleship_driver"), budget).unwrap();
        let tries = |hunter: &str| -> Vec<Escape> {
            let hunter = build_fleet(archetype(hunter), budget).unwrap();
            (0..4).map(|k| escape(&hunter, &runner, k)).collect()
        };
        let mean = |v: &[Escape], f: fn(&Escape) -> f64| v.iter().map(f).sum::<f64>() / v.len() as f64;
        let free = tries("cruiser_driver");
        let held = tries("interceptor_beam");
        let killed = tries("interceptor_torpedo");
        assert!(mean(&free, |e| e.escaped) > 0.9, "nothing holds it against cruisers: {free:?}");
        // Light guns can't kill a battleship, but they keep it in the fight
        // (since v12 its drivers fire one shot each and clear the tacklers in
        // the end).
        assert!(mean(&held, |e| e.steps as f64) > 2.0 * mean(&free, |e| e.steps as f64), "{held:?} v {free:?}");
        // Torpedo interceptors hold it and sink it.
        assert!(mean(&killed, |e| e.escaped) < 0.3, "{killed:?}");
        for e in free.iter().chain(&held).chain(&killed) {
            assert!((e.escaped + e.held + e.lost - 1.0).abs() < 1e-9);
        }
        for key in RUNNERS.iter().chain(&HUNTERS) {
            assert!(ARCHETYPES.iter().any(|a| a.key == *key), "{key} is an archetype");
        }
    }

    #[test]
    fn the_table_folds_both_roles_into_a_constant_sum_score() {
        // Three archetypes: 0 always wins as attacker; defenders win otherwise.
        let cell = |a_won: u32, d_won: u32, undecided: u32| Cell {
            runs: a_won + d_won + undecided, attacker_won: a_won, defender_won: d_won, undecided,
            lost: [f64::from(d_won), f64::from(a_won)], steps: vec![10; (a_won + d_won + undecided) as usize],
            ..Default::default()
        };
        let cells = vec![
            vec![cell(5, 5, 0), cell(10, 0, 0), cell(10, 0, 0)],
            vec![cell(0, 10, 0), cell(5, 5, 0), cell(4, 4, 2)],
            vec![cell(0, 10, 0), cell(3, 7, 0), cell(5, 5, 0)],
        ];
        let t = table(&cells);
        for i in 0..3 {
            assert_eq!(t.score[i][i], 0.5);
            for j in 0..3 {
                assert!((t.score[i][j] + t.score[j][i] - 1.0).abs() < 1e-12);
                assert!(t.low[i][j] <= t.score[i][j] && t.score[i][j] <= t.high[i][j]);
            }
        }
        assert_eq!(t.score[0][1], 1.0, "won both roles");
        // 1 v 2: attacking 4 + 1 (two undecided) of 10; defending 7 of 10.
        assert!((t.score[1][2] - 12.0 / 20.0).abs() < 1e-12);
        assert!((t.undecided[1][2] - 2.0 / 20.0).abs() < 1e-12);
        assert_eq!(t.trade[0][1], Some(1.0));
        assert_eq!(t.mix[0], 1.0, "a dominant archetype is the whole mix");
        assert!((t.field[0] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn targets_check_counters_spread_and_the_capital_ladder() {
        let n = 3;
        let sq = |v: f64| vec![vec![v; n]; n];
        let mut t = Table { score: sq(0.5), low: sq(0.0), high: sq(1.0), trade: vec![vec![Some(0.5); n]; n],
            attacking: sq(0.5), steps: sq(100.0), undecided: sq(0.0), field: vec![0.875, 0.35, 0.275], mix: vec![0.4, 0.3, 0.3] };
        // Armoured cruisers beat beam cruisers 80% of the time but trade only
        // 55% of the value, and blow out beam battleships.
        (t.score[0][1], t.score[1][0], t.trade[0][1]) = (0.8, 0.2, Some(0.55));
        (t.score[0][2], t.score[2][0]) = (0.95, 0.05);
        let keys = ["armoured_cruisers", "cruiser_beam", "battleship_beam"];
        let classes = vec!["cruiser".to_string(), "battleship".to_string()];
        let games = vec![vec![Some(0.5), Some(0.4)], vec![Some(0.6), Some(0.5)]];
        let checks = targets(&keys, &["A", "B", "C"], &t, &classes, &games);
        let get = |kind: &str| checks.iter().filter(|c| c.kind == kind).collect::<Vec<_>>();
        let counter = get("counter");
        assert_eq!(counter.len(), 1, "only counters whose two fleets both fight at this size");
        assert!(!counter[0].pass, "80% of the wins but a 55% trade misses");
        assert!(!get("field")[0].pass && get("field")[0].label.ends_with('A'));
        assert!(!get("mix")[0].pass, "three types share the mix; five are wanted");
        let blowouts = get("blowouts")[0];
        assert!((blowouts.value - 1.0 / 3.0).abs() < 1e-12 && blowouts.pass);
        let ladder = get("ladder");
        assert_eq!(ladder.len(), 1, "only rungs with both classes present");
        assert!((ladder[0].value - 0.6).abs() < 1e-12 && ladder[0].pass, "what battleships guarantee over cruisers");
    }

    #[test]
    fn the_equilibrium_mix_solves_known_games() {
        let rps = vec![vec![0.5, 0.0, 1.0], vec![1.0, 0.5, 0.0], vec![0.0, 1.0, 0.5]];
        for x in equilibrium(&rps) {
            assert!((x - 1.0 / 3.0).abs() < 1e-9);
        }
        // Lopsided rock-paper-scissors: the mix weights by the other two stakes.
        let lopsided = vec![vec![0.5, 0.25, 1.0], vec![0.75, 0.5, 0.25], vec![0.0, 0.75, 0.5]];
        let mix = equilibrium(&lopsided);
        for (x, want) in mix.iter().zip([0.25, 0.5, 0.25]) {
            assert!((x - want).abs() < 1e-9, "{mix:?}");
        }
        // A fourth choice that loses to everything is never played.
        let mut padded: Vec<Vec<f64>> = rps.iter().map(|r| { let mut r = r.clone(); r.push(0.8); r }).collect();
        padded.push(vec![0.2, 0.2, 0.2, 0.5]);
        let mix = equilibrium(&padded);
        assert_eq!(mix[3], 0.0);
        assert!((mix.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        // Its value is ½: the game is fair.
        assert!((solve_game(&padded).0 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_rectangular_game_has_the_value_the_row_player_can_guarantee() {
        // Two rows that each win one column: mixing them guarantees ½, which
        // beats the safe row's 0.4; the columns must mix evenly to hold it.
        let (value, mix) = solve_game(&[vec![1.0, 0.0], vec![0.0, 1.0], vec![0.4, 0.4]]);
        assert!((value - 0.5).abs() < 1e-9, "{value}");
        assert!((mix[0] - 0.5).abs() < 1e-9 && (mix[1] - 0.5).abs() < 1e-9);
        // One row: the column player simply picks its worst column.
        let (value, mix) = solve_game(&[vec![0.3, 0.6]]);
        assert!((value - 0.3).abs() < 1e-9);
        assert_eq!(mix, vec![1.0, 0.0]);
    }

    #[test]
    fn wilson_intervals_stay_honest_at_the_edges() {
        let (lo, hi) = wilson(200.0, 200.0);
        assert!(hi == 1.0 && lo > 0.97 && lo < 0.99, "{lo}");
        let (lo, hi) = wilson(100.0, 200.0);
        assert!((0.5 - lo - 0.069).abs() < 0.002 && (hi - 0.5 - 0.069).abs() < 0.002, "{lo} {hi}");
    }
}


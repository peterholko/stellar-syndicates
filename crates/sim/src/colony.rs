//! §economy Part 2 — COLONY LIFE: population, workforce, and the food-state
//! ladder. This replaces the old binary Habitat fed/unfed boost with a colony
//! that EATS (Provisions ∝ population), GROWS (toward Habitat capacity, only
//! while well-supplied), and STAFFS industry (workforce units for Part 3's
//! production assignments).
//!
//! The one law above all tunables: POPULATION NEVER DECREASES. Famine walks
//! the food ladder down and freezes growth — it never kills. A returning
//! player finds their colony hungry and idle, exactly as big as they left it
//! (§5.1 async-fair: shortages SUSPEND, they never destroy).

use serde::{Deserialize, Serialize};

/// The colony's FOOD STATE — a 4-rung ladder recomputed every tick from how
/// many seconds of Provisions demand the local stockpile covers. Ordering is
/// worst-first so `Ord` reads as "how well fed" (`NoProvisions < … <
/// WellSupplied`). Workforce efficiency degrades down the ladder; Part 3's
/// assignment engine multiplies it into every output (the `food` factor of
/// the legibility chain richness × tier × staffing × skill × food).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FoodState {
    /// The stockpile is EMPTY. Part 3 suspends everything but raw extraction
    /// and defenses. Growth stops. Nobody dies — ever.
    NoProvisions,
    /// Hand-to-mouth (under `FOOD_RATIONING_S` seconds of stock). Workforce at
    /// half efficiency; Part 3 also suspends ADVANCED-tier assignments.
    Critical,
    /// A thin buffer (under `FOOD_WELL_S` seconds of stock). Mild drag.
    Rationing,
    /// Comfortable stock (≥ `FOOD_WELL_S` seconds of demand). Full efficiency,
    /// and the ONLY state in which population grows. Also the vacuous state of
    /// a colony with no population (no demand = no problem) — the `default`,
    /// so pre-economy snapshots load as untroubled.
    #[default]
    WellSupplied,
}

impl FoodState {
    /// Workforce efficiency multiplier — the `food` factor in every output
    /// chain. `NoProvisions` reads 0.0 here, but Part 3 gives raw EXTRACTION a
    /// floor (miners feed themselves off the land; industry stalls outright).
    pub fn efficiency(self) -> f64 {
        match self {
            FoodState::WellSupplied => 1.0,
            FoodState::Rationing => 0.85,
            FoodState::Critical => 0.5,
            FoodState::NoProvisions => 0.0,
        }
    }

    /// Wire slug (snake_case, matches the serde encoding).
    pub fn slug(self) -> &'static str {
        match self {
            FoodState::WellSupplied => "well_supplied",
            FoodState::Rationing => "rationing",
            FoodState::Critical => "critical",
            FoodState::NoProvisions => "no_provisions",
        }
    }

    /// Human name for timeline prose / panels.
    pub fn title(self) -> &'static str {
        match self {
            FoodState::WellSupplied => "Well Supplied",
            FoodState::Rationing => "Rationing",
            FoodState::Critical => "Critical",
            FoodState::NoProvisions => "No Provisions",
        }
    }
}

// --- CONSUMPTION -----------------------------------------------------------

/// Provisions drawn per second per MILLION population. Sized so the Part-3
/// bootstrap colony (2.0M) eats 0.12/s — comfortably under what its seeded
/// Agroplex turns out, with slack left to grow on. A fresh ship-founded
/// outpost (0.5M) eats 0.03/s: one 40-unit Provisions delivery feeds it for
/// ~22 minutes. Tunable.
pub const PROVISIONS_PER_MILLION_PER_S: f64 = 0.06;

// --- CAPACITY + GROWTH ------------------------------------------------------

/// Population capacity (millions) per Habitat tier. Habitats are the ONLY
/// source of capacity: no Habitat, no growth (a ship-founded outpost holds at
/// its founding size until housing goes up). Two tiers reach `POP_MAJOR`
/// exactly. Tunable.
pub const POP_CAP_PER_HABITAT_TIER: f64 = 4.0;

/// Population growth (millions/second) while Well Supplied and under
/// capacity. Linear and flat — legible ("+1M per ~8min while fed"), no
/// compounding surprises. Growth is the ONLY writer of population besides
/// founding/bootstrap; there is NO negative branch anywhere. Tunable.
pub const POP_GROWTH_PER_S: f64 = 0.002;

/// The founding population (millions) a colony ship plants when it settles a
/// claim — the crew and berths of the ship itself. Small on purpose: a new
/// outpost is a HUNGRY MOUTH first (ship Provisions to grow it), not an
/// instant workforce. Tunable.
pub const COLONY_FOUNDING_POP: f64 = 0.5;

// --- WORKFORCE ---------------------------------------------------------------

/// Millions of population per WORKFORCE UNIT (the staffing currency Part 3's
/// assignments spend). The bootstrap 2.0M colony fields 2 units. Tunable.
pub const POP_PER_WORKFORCE: f64 = 0.8;

/// Workforce units a population fields: `floor(population / POP_PER_WORKFORCE)`.
pub fn workforce_units(population: f64) -> u32 {
    (population / POP_PER_WORKFORCE).floor() as u32
}

// --- THE FOOD LADDER ----------------------------------------------------------

/// Seconds of stocked demand for WELL SUPPLIED. Tunable.
pub const FOOD_WELL_S: f64 = 30.0;
/// Seconds of stocked demand for RATIONING (below it: Critical). Tunable.
pub const FOOD_RATIONING_S: f64 = 8.0;
/// HYSTERESIS margin on the way UP: to IMPROVE a state the stock must clear
/// the higher rung's threshold even when discounted by this factor, so a
/// colony hovering exactly at a boundary (production ≈ consumption) never
/// flickers between rungs / spams transition events. Degradation is always
/// immediate — bad news travels fast. Tunable.
pub const FOOD_IMPROVE_MARGIN: f64 = 1.5;

/// The raw ladder rung for a coverage (seconds of demand stocked), no
/// hysteresis. `NoProvisions` is exactly-empty (the draw floor).
fn raw_food_bucket(coverage_secs: f64) -> FoodState {
    if coverage_secs >= FOOD_WELL_S {
        FoodState::WellSupplied
    } else if coverage_secs >= FOOD_RATIONING_S {
        FoodState::Rationing
    } else if coverage_secs > 0.0 {
        FoodState::Critical
    } else {
        FoodState::NoProvisions
    }
}

/// The next food state given post-draw stock coverage and the current state.
/// DOWN is immediate; UP requires the margin (see `FOOD_IMPROVE_MARGIN`).
/// A zero-demand colony (no population) is vacuously Well Supplied.
pub fn food_state_for(coverage_secs: f64, demand_per_s: f64, current: FoodState) -> FoodState {
    if demand_per_s <= 0.0 {
        return FoodState::WellSupplied;
    }
    let raw = raw_food_bucket(coverage_secs);
    if raw > current {
        // Improving: re-bucket with the stock DISCOUNTED — only a clear
        // improvement climbs (and it may climb several rungs at once when a
        // big shipment lands).
        let margined = raw_food_bucket(coverage_secs / FOOD_IMPROVE_MARGIN);
        if margined > current { margined } else { current }
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_orders_worst_first() {
        assert!(FoodState::NoProvisions < FoodState::Critical);
        assert!(FoodState::Critical < FoodState::Rationing);
        assert!(FoodState::Rationing < FoodState::WellSupplied);
        assert_eq!(FoodState::default(), FoodState::WellSupplied);
    }

    #[test]
    fn degradation_is_immediate_but_improvement_needs_margin() {
        use FoodState::*;
        // Falling below a rung drops instantly.
        assert_eq!(food_state_for(FOOD_WELL_S - 0.1, 1.0, WellSupplied), Rationing);
        assert_eq!(food_state_for(0.0, 1.0, Rationing), NoProvisions);
        // Hovering JUST above a rung does NOT climb back (no flicker)...
        assert_eq!(food_state_for(FOOD_WELL_S + 0.1, 1.0, Rationing), Rationing);
        assert_eq!(food_state_for(FOOD_RATIONING_S + 0.1, 1.0, Critical), Critical);
        // ...but clearing it with the margin does — even multiple rungs at once.
        assert_eq!(food_state_for(FOOD_WELL_S * FOOD_IMPROVE_MARGIN, 1.0, Rationing), WellSupplied);
        assert_eq!(food_state_for(FOOD_WELL_S * FOOD_IMPROVE_MARGIN, 1.0, NoProvisions), WellSupplied);
        // Same-rung coverage is a no-op.
        assert_eq!(food_state_for(FOOD_RATIONING_S + 1.0, 1.0, Rationing), Rationing);
    }

    #[test]
    fn zero_demand_is_vacuously_well_supplied() {
        assert_eq!(food_state_for(0.0, 0.0, FoodState::NoProvisions), FoodState::WellSupplied);
    }

    #[test]
    fn workforce_floors_by_population() {
        assert_eq!(workforce_units(0.0), 0);
        assert_eq!(workforce_units(0.5), 0); // a fresh outpost fields nobody yet
        assert_eq!(workforce_units(2.0), 2); // the bootstrap colony fields 2
        assert_eq!(workforce_units(8.0), 10);
    }

    #[test]
    fn food_state_serde_is_snake_case() {
        let s = serde_json::to_string(&FoodState::NoProvisions).unwrap();
        assert_eq!(s, "\"no_provisions\"");
        let back: FoodState = serde_json::from_str("\"well_supplied\"").unwrap();
        assert_eq!(back, FoodState::WellSupplied);
    }
}

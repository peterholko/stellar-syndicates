//! §economy Part 3 — the PRODUCTION ASSIGNMENT engine: staffed structures turn
//! deposits and stockpiled inputs into goods. Nothing produces by itself any
//! more — a deposit needs its extraction structure STAFFED, a converter needs
//! crews and inputs. Every output is one legible factor chain:
//!
//! `output = base · tier_throughput · staffing · skill · food (· traits)`
//!
//! where `base` is a deposit's richness (extraction) or the converter's rate
//! (units of OUTPUT/s), and every factor is a number a player can read in the
//! colony panel and act on. Shortages SUSPEND — reduced rates, latched
//! suspension notices, instant resumption — and never destroy anything
//! (§5.1 async-fair).

use serde::{Deserialize, Serialize};

use crate::build::StructureKind;
use crate::cargo::Commodity;

// --- THE THROUGHPUT LADDER -----------------------------------------------------

/// Output multiplier by STRUCTURE TIER (index 0 = not built = nothing).
/// Deliberately super-linear: deep tiers out-produce spreading the same slots
/// wide, so a focused colony reads differently from a sprawling one. Tunable.
pub const TIER_THROUGHPUT: [f64; 5] = [0.0, 1.0, 2.2, 3.8, 6.0];

/// The throughput of a built tier (clamped into the table).
pub fn tier_throughput(tier: u32) -> f64 {
    TIER_THROUGHPUT[(tier as usize).min(TIER_THROUGHPUT.len() - 1)]
}

// --- EXTRACTION ------------------------------------------------------------------

/// Which structure works a deposit of this commodity. Extraction only — a
/// processed/advanced good has no deposit to mine.
pub fn extraction_structure(c: Commodity) -> Option<StructureKind> {
    match c {
        Commodity::Biomass => Some(StructureKind::Bioharvester),
        Commodity::Volatiles => Some(StructureKind::VolatileHarvester),
        Commodity::MetallicOre | Commodity::Silicates | Commodity::RareElements => {
            Some(StructureKind::MiningComplex)
        }
        _ => None,
    }
}

/// FOOD FLOOR for the primary sector: extraction structures AND the Agroplex
/// keep working (at half rate) even at NoProvisions — miners and farmers feed
/// themselves off the land, and the Agroplex floor is what makes famine
/// RECOVERABLE (a colony with Biomass can always cook its way back up the
/// ladder; without this, NoProvisions would be a death spiral). Tunable.
pub const EXTRACTION_FOOD_FLOOR: f64 = 0.5;

// --- THE CONVERTER TABLE ----------------------------------------------------------

/// One industrial conversion: `structure` turns `inputs` (per 1.0 unit of
/// OUTPUT) into `output` at `rate` units/s (at tier throughput 1.0, all
/// factors 1.0). THE single source of truth — the market's basket-clearing
/// price invariant reads this same table, so recipes and prices can't drift.
#[derive(Debug, Clone, Copy)]
pub struct Converter {
    pub structure: StructureKind,
    pub output: Commodity,
    /// Units of OUTPUT per second at throughput 1.0 (factors multiply this).
    pub rate: f64,
    /// Inputs drawn from the local stockpile PER UNIT OF OUTPUT. Every basket
    /// sums to ≥ 1.0 units, so conversion never net-adds units — the storage
    /// cap can't be violated by industry (a guard still bounds retunings).
    pub inputs: &'static [(Commodity, f64)],
}

/// The seven conversions of the industrial web (5 processed, 2 advanced),
/// rates straight from the design table. At home-geology extraction rates
/// (~0.4 raw/s) converters run INPUT-BOUND — the rate ceiling starts to matter
/// when bulk raws are IMPORTED, which is exactly what makes a supplied forge
/// world an engine (and its supply line a target). Tunable.
pub const CONVERTERS: [Converter; 7] = [
    Converter { structure: StructureKind::Smelter, output: Commodity::Alloys, rate: 1.0, inputs: &[(Commodity::MetallicOre, 1.5), (Commodity::Fuel, 0.3)] },
    Converter { structure: StructureKind::ElectronicsFabricator, output: Commodity::Electronics, rate: 0.5, inputs: &[(Commodity::RareElements, 0.8), (Commodity::Silicates, 0.8)] },
    Converter { structure: StructureKind::ChemicalWorks, output: Commodity::Polymers, rate: 0.8, inputs: &[(Commodity::Volatiles, 1.0), (Commodity::Biomass, 0.8)] },
    Converter { structure: StructureKind::FuelRefinery, output: Commodity::Fuel, rate: 0.8, inputs: &[(Commodity::Volatiles, 1.0)] },
    Converter { structure: StructureKind::Agroplex, output: Commodity::Provisions, rate: 1.2, inputs: &[(Commodity::Biomass, 1.0)] },
    Converter { structure: StructureKind::MachineWorks, output: Commodity::Machinery, rate: 0.3, inputs: &[(Commodity::Alloys, 1.2), (Commodity::Electronics, 0.6), (Commodity::Fuel, 0.4)] },
    Converter { structure: StructureKind::ArmamentsComplex, output: Commodity::Armaments, rate: 0.35, inputs: &[(Commodity::Alloys, 1.0), (Commodity::Electronics, 0.5), (Commodity::Polymers, 0.5)] },
];

/// The conversion a structure kind runs, if it is a converter.
pub fn converter_for(kind: StructureKind) -> Option<&'static Converter> {
    CONVERTERS.iter().find(|c| c.structure == kind)
}

// --- ASSIGNMENTS -------------------------------------------------------------------

/// A standing PRODUCTION ASSIGNMENT at a system: `workers` workforce crews
/// posted to one structure kind. A tier-N structure wants N crews for full
/// throughput (big plants need big shifts); under-crewing runs it at
/// `workers/tier`. If the colony's total posting exceeds its workforce, every
/// assignment dilutes by the same share (fair, legible, deadlock-free).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Assignment {
    pub workers: u32,
    /// §economy Part 4: SPECIALISTS posted to this line, drawn from the
    /// system's resident pool. Every posted specialist works as crew; the
    /// AFFINE ones additionally drive the `skill` factor. Validated against
    /// the pool at command time AND re-checked (non-destructively) each tick,
    /// so a shrunken pool degrades the bonus instead of panicking.
    #[serde(default)]
    pub specialists: std::collections::BTreeMap<crate::specialist::SpecialistKind, u32>,
    /// LATCHED suspension state — why this line produced nothing last tick
    /// (`None` = running). Transitions emit owner-only notices; the latch is
    /// persisted so a snapshot doesn't re-announce old trouble.
    #[serde(default)]
    pub suspended: Option<SuspendReason>,
}

impl Assignment {
    /// A plain posting of `workers` generic crews (the common test/bootstrap case).
    pub fn crew(workers: u32) -> Assignment {
        Assignment { workers, ..Default::default() }
    }
}

/// Why a production line is suspended. Priority when several bind at once:
/// food > inputs > storage (the cause the player should fix first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuspendReason {
    /// The colony can't feed this line's workers (see `food_factor`).
    NoFood,
    /// The input basket is EMPTY locally — ship raws in or staff extraction.
    NoInputs,
    /// The stockpile is at the storage cap — output idles (nothing destroyed).
    StorageFull,
}

impl SuspendReason {
    pub fn slug(self) -> &'static str {
        match self {
            SuspendReason::NoFood => "no_food",
            SuspendReason::NoInputs => "no_inputs",
            SuspendReason::StorageFull => "storage_full",
        }
    }
}

// --- THE FACTOR CHAIN ---------------------------------------------------------------

/// The SKILL ceiling a fully specialist-staffed line reaches. Tunable.
pub const SPECIALIST_SKILL_MULT: f64 = 1.75;

/// The SKILL factor of a line: `1 + (MULT−1) · matched/tier`, with `matched`
/// (the AFFINE specialists posted) capped at the tier — 1.75× when every crew
/// berth of the plant is an affine specialist, pro-rata below, never < 1.0
/// (off-affinity specialists count as crew, not as a penalty).
pub fn skill_factor(matched: u32, tier: u32) -> f64 {
    if tier == 0 {
        return 1.0;
    }
    1.0 + (SPECIALIST_SKILL_MULT - 1.0) * (matched.min(tier) as f64 / tier as f64)
}

/// The FOOD factor of a structure's output — `FoodState::efficiency()` shaped
/// by sector: the primary sector (extraction + Agroplex) never drops below
/// `EXTRACTION_FOOD_FLOOR`; ADVANCED industry (MachineWorks/ArmamentsComplex)
/// suspends outright at Critical (precision work stops before the mills do).
pub fn food_factor(kind: StructureKind, state: crate::colony::FoodState) -> f64 {
    use crate::colony::FoodState as F;
    let eff = state.efficiency();
    match kind {
        StructureKind::Bioharvester
        | StructureKind::VolatileHarvester
        | StructureKind::MiningComplex
        | StructureKind::Agroplex => eff.max(EXTRACTION_FOOD_FLOOR),
        StructureKind::MachineWorks | StructureKind::ArmamentsComplex => {
            if state <= F::Critical { 0.0 } else { eff }
        }
        _ => eff,
    }
}

// --- SHIPYARD ---------------------------------------------------------------------

/// The Shipyard-boost coefficient: ship jobs enqueue at
/// `build_ticks / (1 + SHIPYARD_BOOST · staffing · skill)` — 25% faster fully
/// crewed. Locked in when the job starts (deterministic — no mid-flight
/// retiming when crews move); structure upgrades are unaffected. Tunable.
pub const SHIPYARD_BOOST: f64 = 0.25;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_raw_has_exactly_one_extraction_structure() {
        for c in Commodity::RAW {
            assert!(extraction_structure(c).is_some(), "{c:?} must be extractable");
        }
        for c in Commodity::ALL {
            if !Commodity::RAW.contains(&c) {
                assert!(extraction_structure(c).is_none(), "{c:?} must NOT be extractable");
            }
        }
    }

    #[test]
    fn converter_baskets_never_net_add_units() {
        for conv in &CONVERTERS {
            let total_in: f64 = conv.inputs.iter().map(|(_, per)| per).sum();
            assert!(
                total_in >= 1.0 - 1e-12,
                "{:?}: basket {total_in} < 1.0 unit per output — industry could overflow the storage cap",
                conv.structure
            );
        }
    }

    #[test]
    fn converters_and_extraction_partition_the_producing_structures() {
        // Each converter's structure appears exactly once, and no extraction
        // structure doubles as a converter.
        for (i, a) in CONVERTERS.iter().enumerate() {
            for b in &CONVERTERS[i + 1..] {
                assert_ne!(a.structure, b.structure);
            }
            assert!(!matches!(a.structure, StructureKind::MiningComplex | StructureKind::VolatileHarvester | StructureKind::Bioharvester));
        }
    }

    #[test]
    fn throughput_ladder_is_superlinear_from_one() {
        assert_eq!(tier_throughput(0), 0.0);
        assert_eq!(tier_throughput(1), 1.0);
        for t in 1..4u32 {
            assert!(
                tier_throughput(t + 1) > 2.0 * tier_throughput(t) - tier_throughput(t.saturating_sub(1)),
                "the ladder accelerates"
            );
        }
        assert_eq!(tier_throughput(99), 6.0, "clamped past the table");
    }

    #[test]
    fn food_factors_by_sector() {
        use crate::colony::FoodState::*;
        // Primary sector floors at 0.5, even bone-dry.
        assert_eq!(food_factor(StructureKind::Bioharvester, NoProvisions), 0.5);
        assert_eq!(food_factor(StructureKind::Agroplex, NoProvisions), 0.5, "the food industry must be able to cook its way back");
        assert_eq!(food_factor(StructureKind::MiningComplex, WellSupplied), 1.0);
        // Ordinary converters track efficiency and die at zero.
        assert_eq!(food_factor(StructureKind::Smelter, Rationing), 0.85);
        assert_eq!(food_factor(StructureKind::Smelter, NoProvisions), 0.0);
        // Advanced industry stops at Critical already.
        assert_eq!(food_factor(StructureKind::MachineWorks, Critical), 0.0);
        assert_eq!(food_factor(StructureKind::ArmamentsComplex, Rationing), 0.85);
    }
}

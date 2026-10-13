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
/// wide, so a focused colony reads differently from a sprawling one. Tiers 5–6
/// are the research-prize ceiling (freely buildable stops at tier 4 — the gate
/// lives in build.rs) and stay on the same accelerating curve. Tunable.
pub const TIER_THROUGHPUT: [f64; 7] = [0.0, 1.0, 2.2, 3.8, 6.0, 8.6, 11.5];

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
        // §pockets: Helium-3 ice is harvested with the Volatiles around it.
        Commodity::Volatiles | Commodity::Helium3Ice => Some(StructureKind::VolatileHarvester),
        c if c.is_mineable_mineral() => {
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

/// Snapshot these with a site's administrative report. Rendering today's
/// research against yesterday's colony report would outrun that site's light.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProductionMods {
    pub extraction: f64,
    /// Strip Rigs: extraction on moons and asteroid belts.
    pub extraction_moons: f64,
    pub processing: f64,
    pub ore_recovery: f64,
    /// Structure-time research is emitted with the same delayed site report.
    pub structure_build_time: f64,
}

impl Default for ProductionMods {
    fn default() -> Self {
        Self { extraction: 1.0, extraction_moons: 1.0, processing: 1.0, ore_recovery: 1.0, structure_build_time: 1.0 }
    }
}

impl ProductionMods {
    pub fn recovery(self, kind: StructureKind) -> f64 {
        if kind == StructureKind::Smelter { self.ore_recovery } else { 1.0 }
    }
}

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
    /// Inputs drawn PER UNIT OF PRIMARY OUTPUT, including its secondary yields.
    /// Base baskets consume at least their combined yield; the storage guard
    /// also handles planetary bonuses that can net-add units.
    pub inputs: &'static [(Commodity, f64)],
}

impl Converter {
    /// Secondary yields share the SAME processing budget/input draw. They are
    /// not extra production lines; storage bounds include every emitted unit.
    pub fn byproducts(&self) -> &'static [(Commodity, f64)] {
        if self.structure != StructureKind::Smelter { return &[]; }
        match self.inputs[0].0 {
            Commodity::CupriteOre | Commodity::TitaniumOre => &[(Commodity::Alloys, 0.15)],
            // A TRACE of rare elements: enough to matter at scale, never enough
            // to turn a silicate smelter into a rare-elements faucet (§ore-ladder).
            Commodity::CrystallineOre => &[(Commodity::RareElements, 0.05)],
            Commodity::RareMetalOre => &[(Commodity::ConductiveMetals, 0.7)],
            // §belt-ores: the mixed ores pay out in a second good.
            Commodity::OlivineOre => &[(Commodity::Alloys, 0.5)],
            Commodity::BorniteOre => &[(Commodity::RareElements, 0.08)],
            Commodity::IlmeniteOre => &[(Commodity::Titanium, 0.25)],
            _ => &[],
        }
    }

    pub fn outputs(&self) -> impl Iterator<Item = (Commodity, f64)> + '_ {
        std::iter::once((self.output, 1.0)).chain(self.byproducts().iter().copied())
    }
}

/// Standing Smelter recipes. Choose one per body, not one per cargo shipment.
/// All use the ordinary workforce, power, research and planetary yield rules.
/// §fuel-overhaul: no recipe burns fuel; the Smelter draws double power instead.
/// Base-price margins are opportunities, not guarantees after market flow and freight.
///
/// §ore-density: the ORE draw per batch is each ore's DENSITY. A bulk ore
/// (Ferrite, Crystalline) spends 1.5 units per refined unit; a unit of Cuprite
/// is a whole batch; a unit of Titanium ore is two and a half; a unit of
/// Rare-metal ore is twenty Rare Elements and fourteen Conductive Metals. Dense ore
/// also comes out of the ground in proportionally FEWER units per second
/// ([`ore_bulk_ratio`]), so a deposit's content rate and a colony's income per
/// hour keep their modest gradient while the PRICE PER UNIT runs two-hundredfold
/// — EVE's shape: Veldspar to Arkonor is hundreds of times per unit and a
/// handful of times per cubic metre. The accrual cap guard bounds the units a
/// dense batch nets into storage.
pub const ORE_REFINING: [Converter; 11] = [
    CONVERTERS[0],
    Converter { structure: StructureKind::Smelter, output: Commodity::ConductiveMetals,
        rate: 0.75, inputs: &[(Commodity::CupriteOre, 1.0)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::Titanium,
        rate: 0.5, inputs: &[(Commodity::TitaniumOre, 0.4)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::Silicates,
        rate: 1.0, inputs: &[(Commodity::CrystallineOre, 1.5)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::RareElements,
        rate: 0.4, inputs: &[(Commodity::RareMetalOre, 0.05)] },
    // §belt-ores. Olivine: Silicates with half an Alloy per batch.
    Converter { structure: StructureKind::Smelter, output: Commodity::Silicates,
        rate: 1.0, inputs: &[(Commodity::OlivineOre, 1.5)] },
    // Bornite: Conductive Metals with a Rare Elements trace — two of the three
    // Electronics inputs from one ore.
    Converter { structure: StructureKind::Smelter, output: Commodity::ConductiveMetals,
        rate: 0.75, inputs: &[(Commodity::BorniteOre, 1.25)] },
    // Ilmenite: iron-titanium ore. Alloys first, with a quarter of Titanium —
    // Titanium nearer the core than Titanium ore, at half the Smelter rate.
    Converter { structure: StructureKind::Smelter, output: Commodity::Alloys,
        rate: 1.0, inputs: &[(Commodity::IlmeniteOre, 1.5)] },
    // §pockets: the rare pocket ores, each into its own top-end material.
    // Rarer is denser: five Helium-3 a unit of ice, ten Iridium a unit of ore,
    // twenty Exotic Matter a Void crystal.
    Converter { structure: StructureKind::Smelter, output: Commodity::Helium3,
        rate: 0.5, inputs: &[(Commodity::Helium3Ice, 0.2)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::Iridium,
        rate: 0.4, inputs: &[(Commodity::IridiumOre, 0.1)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::ExoticMatter,
        rate: 0.3, inputs: &[(Commodity::VoidCrystal, 0.05)] },
];

/// Ore units a BULK refining batch draws — the density every other ore is
/// measured against.
pub const BULK_ORE_PER_BATCH: f64 = 1.5;

/// Ore units one refining batch of `c` draws (bulk density for non-ores).
pub fn ore_units_per_batch(c: Commodity) -> f64 {
    ORE_REFINING
        .iter()
        .find(|r| r.inputs[0].0 == c)
        .map(|r| r.inputs[0].1)
        .unwrap_or(BULK_ORE_PER_BATCH)
}

/// §ore-density: units of `c` that leave the ground per unit of BULK-equivalent
/// extraction. A deposit's `richness` is its content rate in bulk units; the
/// extracted unit count is `richness × this` — 1.0 for the bulk ores, food and
/// gas, two thirds for Cuprite, four fifteenths for Titanium ore, one thirtieth
/// for Rare-metal ore. Reserves, previews and research credit use the same
/// ratio, so recipes, prices and mining can never drift apart.
pub fn ore_bulk_ratio(c: Commodity) -> f64 {
    ore_units_per_batch(c) / BULK_ORE_PER_BATCH
}

pub fn assigned_converter(kind: StructureKind, assignment: Option<&Assignment>) -> Option<&'static Converter> {
    if kind == StructureKind::Smelter {
        let ore = assignment.and_then(|a| a.refining_ore).unwrap_or(Commodity::MetallicOre);
        return ORE_REFINING.iter().find(|r| r.inputs[0].0 == ore);
    }
    // §fuel-overhaul: a Drive Works line makes Warp Cores unless set to Fusion
    // Cores (the same per-line recipe field as the Smelter's ore).
    if kind == StructureKind::DriveWorks
        && assignment.and_then(|a| a.refining_ore) == Some(Commodity::FusionCore)
    {
        return Some(&FUSION_CORE_CONVERTER);
    }
    converter_for(kind)
}

/// §fuel-overhaul: what a Drive Works line may make.
pub fn is_drive_works_product(c: Commodity) -> bool {
    matches!(c, Commodity::WarpCore | Commodity::FusionCore)
}

/// §fuel-overhaul: the Drive Works' second recipe — a Fusion Plant's core,
/// slower than a Warp Core; the basket clears by about a fifth.
pub const FUSION_CORE_CONVERTER: Converter = Converter {
    structure: StructureKind::DriveWorks,
    output: Commodity::FusionCore,
    rate: 0.08,
    inputs: &[(Commodity::PrecisionComponents, 2.0), (Commodity::Composites, 0.6), (Commodity::Iridium, 0.4)],
};

/// One default conversion per industrial structure. Smelters may instead run
/// exactly one of ORE_REFINING's standing recipes, never all five concurrently.
/// Components follow their inputs in this table, so the shared-stockpile tick
/// can consume this tick's upstream output. At home-geology extraction rates
/// (~0.4 raw/s) converters run INPUT-BOUND — the rate ceiling starts to matter
/// when bulk raws are IMPORTED, which is exactly what makes a supplied forge
/// world an engine (and its supply line a target). Tunable.
pub const CONVERTERS: [Converter; 14] = [
    Converter {
        structure: StructureKind::Smelter,
        output: Commodity::Alloys,
        rate: 1.0,
        inputs: &[(Commodity::MetallicOre, 1.5)],
    },
    // §fuel-overhaul: the Uranium Mill sifts fissile grains out of Ferrite Ore —
    // two Uranium per ore, enough for about ten slow Tiny round trips an hour.
    Converter {
        structure: StructureKind::UraniumMill,
        output: Commodity::Uranium,
        rate: 0.05,
        inputs: &[(Commodity::MetallicOre, 0.5)],
    },
    Converter {
        structure: StructureKind::ElectronicsFabricator,
        output: Commodity::Electronics,
        rate: 0.5,
        // §ore-ladder: the dear refined goods are needed in TENTHS of a unit —
        // "a little goes a long way" is what makes a rare-metal haul worth the
        // trip without repricing every end product above it.
        inputs: &[(Commodity::ConductiveMetals, 0.5), (Commodity::RareElements, 0.1), (Commodity::Silicates, 0.6)],
    },
    Converter {
        structure: StructureKind::ChemicalWorks,
        output: Commodity::Polymers,
        rate: 0.8,
        inputs: &[(Commodity::Volatiles, 1.0), (Commodity::Biomass, 0.8)],
    },
    Converter {
        structure: StructureKind::FuelRefinery,
        output: Commodity::Deuterium,
        rate: 0.8,
        inputs: &[(Commodity::Volatiles, 1.0)],
    },
    Converter {
        structure: StructureKind::Agroplex,
        output: Commodity::Provisions,
        rate: 1.2,
        inputs: &[(Commodity::Biomass, 1.0)],
    },
    // §gaia: Living Terraces farm their own gardens — the Agroplex's rate with
    // no Biomass basket. Gaia-only, so the free input is the world's gift.
    Converter {
        structure: StructureKind::LivingTerraces,
        output: Commodity::Provisions,
        rate: 1.2,
        inputs: &[],
    },
    Converter {
        structure: StructureKind::MachineWorks,
        output: Commodity::Machinery,
        rate: 0.3,
        inputs: &[
            (Commodity::Alloys, 1.2),
            (Commodity::Electronics, 0.6),
        ],
    },
    Converter {
        structure: StructureKind::ArmamentsComplex,
        output: Commodity::Armaments,
        rate: 0.35,
        inputs: &[
            (Commodity::Alloys, 1.0),
            (Commodity::Electronics, 0.5),
            (Commodity::Polymers, 0.5),
        ],
    },
    // §industry-chains: modest value added at each stage; specialized colonies
    // supply heavy yards without making these inputs a starter-fleet tax.
    Converter {
        structure: StructureKind::CompositeWorks,
        output: Commodity::Composites,
        rate: 0.30,
        inputs: &[(Commodity::Alloys, 1.0), (Commodity::Polymers, 0.75), (Commodity::Silicates, 0.4)],
    },
    Converter {
        structure: StructureKind::HullFabricator,
        output: Commodity::HullSections,
        rate: 0.18,
        inputs: &[(Commodity::Composites, 1.2), (Commodity::Machinery, 0.5), (Commodity::Titanium, 0.4)],
    },
    Converter {
        structure: StructureKind::PrecisionWorks,
        output: Commodity::PrecisionComponents,
        rate: 0.20,
        inputs: &[(Commodity::Electronics, 1.0), (Commodity::Machinery, 0.5), (Commodity::RareElements, 0.1)],
    },
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::WarpCore,
        rate: 0.12,
        // §rare-materials: Iridium for the hot end, in place of Composites at
        // equal base value. §fuel-overhaul: no furnace fuel (double power).
        inputs: &[(Commodity::PrecisionComponents, 1.25), (Commodity::Composites, 0.82),
            (Commodity::Iridium, 0.1)],
    },
    // §fuel-overhaul: the last tech's factory. Two Deuterium and ten times a
    // factory's power per unit: antimatter is always a net energy loss.
    Converter {
        structure: StructureKind::AntimatterPlant,
        output: Commodity::Antimatter,
        rate: 0.005,
        inputs: &[(Commodity::Deuterium, 2.0)],
    },
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
    /// Absent in older saves: their Smelters continue refining Ferrite Ore.
    #[serde(default)]
    pub refining_ore: Option<Commodity>,
}

impl Assignment {
    /// A plain posting of `workers` generic crews (the common test/bootstrap case).
    pub fn crew(workers: u32) -> Assignment {
        Assignment {
            workers,
            ..Default::default()
        }
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
    /// A founding camp's food line is holding: Provisions + Biomass already
    /// cover the camp's reserve, so the small warehouse keeps room for ore.
    /// A deliberate, quiet pause — never a timeline alarm.
    FoodReserve,
}

impl SuspendReason {
    pub fn slug(self) -> &'static str {
        match self {
            SuspendReason::NoFood => "no_food",
            SuspendReason::NoInputs => "no_inputs",
            SuspendReason::StorageFull => "storage_full",
            SuspendReason::FoodReserve => "food_reserve",
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
/// `EXTRACTION_FOOD_FLOOR`; ADVANCED industry (machinery, armaments and components)
/// suspends outright at Critical (precision work stops before the mills do).
pub fn food_factor(kind: StructureKind, state: crate::colony::FoodState) -> f64 {
    use crate::colony::FoodState as F;
    let eff = state.efficiency();
    match kind {
        StructureKind::Bioharvester
        | StructureKind::VolatileHarvester
        | StructureKind::MiningComplex
        | StructureKind::Agroplex
        | StructureKind::LivingTerraces => eff.max(EXTRACTION_FOOD_FLOOR),
        StructureKind::MachineWorks | StructureKind::ArmamentsComplex
        | StructureKind::CompositeWorks | StructureKind::HullFabricator
        | StructureKind::PrecisionWorks | StructureKind::DriveWorks => {
            if state <= F::Critical {
                0.0
            } else {
                eff
            }
        }
        _ => eff,
    }
}

// --- SHIPYARD ---------------------------------------------------------------------

/// Shipyard speed bonus while staffed. Zero effective workforce means NO work;
/// changing assignments changes the remaining work's rate, not completed work.
pub const SHIPYARD_BOOST: f64 = 0.25;

pub fn shipyard_work_rate(staffing: f64, skill: f64) -> f64 {
    if staffing <= 0.0 { 0.0 } else { 1.0 + SHIPYARD_BOOST * staffing * skill }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_raw_has_exactly_one_extraction_structure() {
        for c in Commodity::RAW {
            assert!(
                extraction_structure(c).is_some(),
                "{c:?} must be extractable"
            );
        }
        for c in Commodity::ALL {
            if !Commodity::RAW.contains(&c) && !c.is_mineable_mineral() {
                assert!(
                    extraction_structure(c).is_none(),
                    "{c:?} must NOT be extractable"
                );
            }
        }
    }

    /// Manufacturing baskets never net-add units. Refining a DENSE ore does by
    /// design (§ore-density) — the accrual cap guard bounds those batches, and
    /// `ore_economy::boosted_secondary_yields_cannot_overfill_storage` proves it
    /// on Rare-metal ore, the densest.
    #[test]
    fn converter_baskets_never_net_add_units() {
        // §gaia: Living Terraces are a farm, not a factory: they grow food from
        // crews alone, so like extraction the accrual cap guard bounds them
        // (`living_terraces::full_storage_idles_living_terraces`).
        // §fuel-overhaul: the Uranium Mill sifts two fissile units from an ore,
        // a dense-ore refinery in all but name; the same guard bounds it.
        for conv in CONVERTERS.iter().filter(|c| !matches!(c.structure,
            StructureKind::LivingTerraces | StructureKind::UraniumMill)) {
            let total_in: f64 = conv.inputs.iter().map(|(_, per)| per).sum();
            assert!(
                total_in >= conv.outputs().map(|(_, units)| units).sum::<f64>() - 1e-12,
                "{:?}: basket {total_in} < 1.0 unit per output — industry could overflow the storage cap",
                conv.structure
            );
        }
        for conv in ORE_REFINING.iter() {
            let ore = conv.inputs[0].0;
            let total_in: f64 = conv.inputs.iter().map(|(_, per)| per).sum();
            let total_out: f64 = conv.outputs().map(|(_, units)| units).sum();
            assert!(
                total_in >= total_out - 1e-12 || ore_bulk_ratio(ore) < 1.0,
                "{ore:?}: only a DENSE ore may net-add units per batch ({total_in} in, {total_out} out)"
            );
        }
        assert_eq!(ore_bulk_ratio(Commodity::MetallicOre), 1.0);
        assert_eq!(ore_bulk_ratio(Commodity::Volatiles), 1.0);
        assert!(ore_bulk_ratio(Commodity::RareMetalOre) < ore_bulk_ratio(Commodity::TitaniumOre));
        assert!(ore_bulk_ratio(Commodity::TitaniumOre) < ore_bulk_ratio(Commodity::CupriteOre));
        assert!(ore_bulk_ratio(Commodity::CupriteOre) < 1.0);
    }

    #[test]
    fn converters_and_extraction_partition_the_producing_structures() {
        // Each converter's structure appears exactly once, and no extraction
        // structure doubles as a converter.
        for (i, a) in CONVERTERS.iter().enumerate() {
            for b in &CONVERTERS[i + 1..] {
                assert_ne!(a.structure, b.structure);
            }
            assert!(!matches!(
                a.structure,
                StructureKind::MiningComplex
                    | StructureKind::VolatileHarvester
                    | StructureKind::Bioharvester
            ));
        }
    }

    #[test]
    fn throughput_ladder_is_superlinear_from_one() {
        assert_eq!(tier_throughput(0), 0.0);
        assert_eq!(tier_throughput(1), 1.0);
        // The two research-prize tiers exist and keep climbing.
        assert!(
            tier_throughput(5) > tier_throughput(4),
            "tier V out-produces IV"
        );
        assert!(
            tier_throughput(6) > tier_throughput(5),
            "tier VI out-produces V"
        );
        // Each step's MARGINAL gain grows — one deep tier (concentration) stays
        // ahead of spreading the same investment across shallow ones
        // (duplication), all the way to the top of the extended ladder.
        for t in 1..6u32 {
            assert!(
                tier_throughput(t + 1)
                    > 2.0 * tier_throughput(t) - tier_throughput(t.saturating_sub(1)),
                "the ladder accelerates at tier {t}→{}",
                t + 1
            );
        }
        assert_eq!(
            tier_throughput(99),
            11.5,
            "clamped past the (now longer) table"
        );
    }

    #[test]
    fn food_factors_by_sector() {
        use crate::colony::FoodState::*;
        // Primary sector floors at 0.5, even bone-dry.
        assert_eq!(food_factor(StructureKind::Bioharvester, NoProvisions), 0.5);
        assert_eq!(
            food_factor(StructureKind::Agroplex, NoProvisions),
            0.5,
            "the food industry must be able to cook its way back"
        );
        assert_eq!(food_factor(StructureKind::MiningComplex, WellSupplied), 1.0);
        // Ordinary converters track efficiency and die at zero.
        assert_eq!(food_factor(StructureKind::Smelter, Rationing), 0.85);
        assert_eq!(food_factor(StructureKind::Smelter, NoProvisions), 0.0);
        // Advanced industry stops at Critical already.
        assert_eq!(food_factor(StructureKind::MachineWorks, Critical), 0.0);
        assert_eq!(
            food_factor(StructureKind::ArmamentsComplex, Rationing),
            0.85
        );
    }
}

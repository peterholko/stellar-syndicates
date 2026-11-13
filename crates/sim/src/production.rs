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
/// §stores-flows (Peter, Nov 5 2026: the Stores panel's in, out and net for
/// each good): what a system's economy makes and uses of a good, per second,
/// smoothed (`StarSystem::fold_flows`). Production lines, food, power, upkeep,
/// repairs and research count; transfers (freight, trade, builds, raids,
/// refuelling) do not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Flow {
    pub made: f64,
    pub used: f64,
}
/// §stores-flows: the smoothing's time constant. A new line shows most of its
/// rate within a minute. Tunable.
pub const FLOW_SMOOTHING_S: f64 = 20.0;
/// §stores-flows: the reported grain, per second (0.1 a minute): a finer
/// change makes no new site report.
pub const FLOW_QUANTUM: f64 = 1.0 / 600.0;
/// A flow rounded to the reported grain.
pub fn quantized_flow(f: Flow) -> Flow {
    let q = |x: f64| (x / FLOW_QUANTUM).round() * FLOW_QUANTUM;
    Flow { made: q(f.made), used: q(f.used) }
}

/// §progression (Peter, Nov 11 2026; docs/balance/progression.md): what
/// gathering and making pay, against everything a corporation buys. Every
/// extraction and value-making converter runs at this share of its tuned rate
/// (the heavy and capital part assemblies, which only repackage light parts,
/// keep theirs), and a person eats this share too
/// (`colony::PROVISIONS_PER_MILLION_PER_S`), so no world starves. Early income
/// stays low, so basic gear stays a decision. Tunable.
pub const OUTPUT_SCALE: f64 = 0.05;

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
    // §research-wiring (R2):
    /// Hydroponics, Gene-Tailored Crops: Agroplex output per Biomass.
    pub agroplex_yield: f64,
    /// Gene-Tailored Crops: Agroplex Biomass per unit.
    pub agroplex_inputs: f64,
    /// Autoforges: Machine Works inputs per unit.
    pub machinery_inputs: f64,
    /// Civic Rations: the Rationing efficiency, as a multiple of its base.
    pub rationing: f64,
    /// Provisions eaten per person.
    pub provisions_use: f64,
    /// Trace Refining: Mining Complexes on metal ores also yield Rare Elements.
    pub trace_refining: bool,
    /// Mantle Taps: Deep Deposits' tier lag no longer applies.
    pub mantle_taps: bool,
    /// Slag Reclamation: Smelters also emit Silicates.
    pub slag_reclamation: bool,
    /// Crown Vein: the designated body (system, body id), one band richer.
    pub crown_vein: Option<(crate::ids::EntityId, u32)>,
    /// Flash Forges: each world builds two structures at once.
    pub flash_forges: bool,
    /// Beneficiation: poor mineral ground mines at 85% of Average at worst.
    pub beneficiation: bool,
}

impl Default for ProductionMods {
    fn default() -> Self {
        Self {
            extraction: 1.0, extraction_moons: 1.0, processing: 1.0, ore_recovery: 1.0, structure_build_time: 1.0,
            agroplex_yield: 1.0, agroplex_inputs: 1.0, machinery_inputs: 1.0, rationing: 1.0, provisions_use: 1.0,
            trace_refining: false, mantle_taps: false, slag_reclamation: false, crown_vein: None, flash_forges: false,
            beneficiation: false,
        }
    }
}

/// §research-wiring: Trace Refining's Rare Elements, per unit of metal ore mined.
pub const TRACE_RARE_PER_ORE: f64 = 0.05;
/// §research-wiring: Slag Reclamation's Silicates, per unit a Smelter emits.
pub const SLAG_SILICATES_PER_OUTPUT: f64 = 0.10;

impl ProductionMods {
    /// Units emitted per unit of work: refining recovery for Smelters, crop
    /// research for Agroplexes. Never changes the input draw or the work rate.
    pub fn recovery(self, kind: StructureKind) -> f64 {
        match kind {
            StructureKind::Smelter => self.ore_recovery,
            StructureKind::Agroplex => self.agroplex_yield,
            _ => 1.0,
        }
    }

    /// The input basket's multiple: Autoforges for Machine Works, Gene-Tailored
    /// Crops for Agroplexes.
    pub fn input_mult(self, kind: StructureKind) -> f64 {
        match kind {
            StructureKind::MachineWorks => self.machinery_inputs,
            StructureKind::Agroplex => self.agroplex_inputs,
            _ => 1.0,
        }
    }

    /// A by-product emitted beside the line's outputs, per unit of output:
    /// Slag Reclamation's Silicates from Smelters.
    pub fn byproduct(self, kind: StructureKind) -> Option<(Commodity, f64)> {
        (self.slag_reclamation && kind == StructureKind::Smelter)
            .then_some((Commodity::Silicates, SLAG_SILICATES_PER_OUTPUT))
    }

    /// The geology research on one body of `system`.
    pub fn geology(self, system: crate::ids::EntityId, body: u32) -> crate::explore::GeologyMods {
        crate::explore::GeologyMods { floor_average: self.mantle_taps, crown: self.crown_vein == Some((system, body)),
            beneficiation: self.beneficiation }
    }

    /// Does a Mining Complex working `ore` also yield Rare Elements?
    pub fn traces(self, ore: Commodity) -> bool {
        self.trace_refining && is_metal_ore(ore)
    }
}

/// §research-wiring: the metal-bearing ores (Trace Refining's "metallic deposits").
pub fn is_metal_ore(c: Commodity) -> bool {
    matches!(c, Commodity::MetallicOre | Commodity::CupriteOre | Commodity::TitaniumOre | Commodity::RareMetalOre
        | Commodity::BorniteOre | Commodity::IlmeniteOre | Commodity::IridiumOre)
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
        rate: 0.75 * OUTPUT_SCALE, inputs: &[(Commodity::CupriteOre, 1.0)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::Titanium,
        rate: 0.5 * OUTPUT_SCALE, inputs: &[(Commodity::TitaniumOre, 0.4)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::Silicates,
        rate: 1.0 * OUTPUT_SCALE, inputs: &[(Commodity::CrystallineOre, 1.5)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::RareElements,
        rate: 0.4 * OUTPUT_SCALE, inputs: &[(Commodity::RareMetalOre, 0.05)] },
    // §belt-ores. Olivine: Silicates with half an Alloy per batch.
    Converter { structure: StructureKind::Smelter, output: Commodity::Silicates,
        rate: 1.0 * OUTPUT_SCALE, inputs: &[(Commodity::OlivineOre, 1.5)] },
    // Bornite: Conductive Metals with a Rare Elements trace — two of the three
    // Electronics inputs from one ore.
    Converter { structure: StructureKind::Smelter, output: Commodity::ConductiveMetals,
        rate: 0.75 * OUTPUT_SCALE, inputs: &[(Commodity::BorniteOre, 1.25)] },
    // Ilmenite: iron-titanium ore. Alloys first, with a quarter of Titanium —
    // Titanium nearer the core than Titanium ore, at half the Smelter rate.
    Converter { structure: StructureKind::Smelter, output: Commodity::Alloys,
        rate: 1.0 * OUTPUT_SCALE, inputs: &[(Commodity::IlmeniteOre, 1.5)] },
    // §pockets: the rare pocket ores, each into its own top-end material.
    // Rarer is denser: five Helium-3 a unit of ice, ten Iridium a unit of ore,
    // twenty Exotic Matter a Void crystal.
    Converter { structure: StructureKind::Smelter, output: Commodity::Helium3,
        rate: 0.5 * OUTPUT_SCALE, inputs: &[(Commodity::Helium3Ice, 0.2)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::Iridium,
        rate: 0.4 * OUTPUT_SCALE, inputs: &[(Commodity::IridiumOre, 0.1)] },
    Converter { structure: StructureKind::Smelter, output: Commodity::ExoticMatter,
        rate: 0.3 * OUTPUT_SCALE, inputs: &[(Commodity::VoidCrystal, 0.05)] },
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
    // §shipbuilding-overhaul: a works with several products runs the one its
    // line is set to (the same per-line recipe field as the Smelter's ore),
    // else its first.
    let products = products_of(kind);
    if let Some(product) = assignment.and_then(|a| a.refining_ore)
        && let Some(conv) = products.iter().find(|c| c.output == product)
    {
        return Some(conv);
    }
    products.first().or_else(|| converter_for(kind))
}

/// §shipbuilding-overhaul: every recipe a works can run, default first. The
/// Smelter's ores live in `ORE_REFINING`; a single-recipe structure lists its
/// one converter.
pub fn products_of(kind: StructureKind) -> &'static [Converter] {
    match kind {
        StructureKind::DriveWorks => &DRIVE_WORKS_PRODUCTS,
        StructureKind::HullFabricator => &HULL_FABRICATOR_PRODUCTS,
        StructureKind::ElectronicsFabricator => &ELECTRONICS_FABRICATOR_PRODUCTS,
        StructureKind::ArmamentsComplex => &ARMAMENTS_COMPLEX_PRODUCTS,
        _ => converter_for(kind).map(std::slice::from_ref).unwrap_or(&[]),
    }
}

/// A works with a recipe picker.
pub fn has_product_picker(kind: StructureKind) -> bool {
    kind == StructureKind::Smelter || products_of(kind).len() > 1
}

/// May a line of `kind` be set to make `c`?
pub fn is_product_of(kind: StructureKind, c: Commodity) -> bool {
    if kind == StructureKind::Smelter {
        return c.is_smeltable();
    }
    products_of(kind).iter().any(|conv| conv.output == c)
}

/// Every recipe in the web: the defaults, the Smelter's ores, the Drive
/// Works' products and the kit parts.
pub fn all_converters() -> impl Iterator<Item = &'static Converter> {
    CONVERTERS.iter().chain(ORE_REFINING.iter()).chain(DRIVE_WORKS_PRODUCTS.iter()).chain(PART_RECIPES.iter())
        .chain(MUNITION_RECIPES.iter()).chain(SIZED_HULL_SECTIONS.iter())
}

/// The converter that makes `c` anywhere in the web (the first found).
pub fn converter_for_output(c: Commodity) -> Option<&'static Converter> {
    all_converters().find(|conv| conv.output == c)
}

/// §shipbuilding-overhaul: the kit parts' second recipes. The Hull Fabricator
/// adds Cargo Bays; the Semiconductor Fab adds Nav Computers (the first part a
/// corporation can make, at Signal Libraries); the Armaments Complex adds the
/// two guns, each carrying its Weapon Mounts. One recipe at a time on each
/// works, like a Smelter. Tunable.
pub const PART_RECIPES: [Converter; 4] = [
    Converter {
        structure: StructureKind::HullFabricator,
        output: Commodity::CargoBay,
        rate: 0.25 * OUTPUT_SCALE,
        inputs: &[(Commodity::Alloys, 3.0), (Commodity::Polymers, 1.0), (Commodity::Machinery, 0.5)],
    },
    Converter {
        structure: StructureKind::ElectronicsFabricator,
        output: Commodity::NavComputer,
        rate: 0.10 * OUTPUT_SCALE,
        inputs: &[(Commodity::Electronics, 2.0), (Commodity::PrecisionComponents, 1.0), (Commodity::RareElements, 0.2)],
    },
    Converter {
        structure: StructureKind::ArmamentsComplex,
        output: Commodity::PulseLaser,
        rate: 0.20 * OUTPUT_SCALE,
        inputs: &[(Commodity::Armaments, 1.0), (Commodity::Electronics, 0.5), (Commodity::PrecisionComponents, 0.25)],
    },
    Converter {
        structure: StructureKind::ArmamentsComplex,
        output: Commodity::BeamBattery,
        rate: 0.05 * OUTPUT_SCALE,
        inputs: &[(Commodity::Armaments, 4.0), (Commodity::Electronics, 2.0), (Commodity::PrecisionComponents, 1.0),
            (Commodity::Machinery, 1.0)],
    },
];

/// §shipbuilding-overhaul Phase 5: the munitions, three more Armaments Complex
/// recipes after Munitions Lines. Torpedoes want Weapon Mounts, Precision
/// Components and Volatiles; Slugs Alloys and Titanium, cheap and heavy; Beam
/// Crystals Crystalline Ore and Rare Elements — Crystalline Ore's first use
/// beyond Silicates. One unit is one shot (one crystal one gun). Tunable.
pub const MUNITION_RECIPES: [Converter; 3] = [
    Converter {
        structure: StructureKind::ArmamentsComplex,
        output: Commodity::Torpedoes,
        rate: 0.30 * OUTPUT_SCALE,
        inputs: &[(Commodity::Armaments, 0.5), (Commodity::PrecisionComponents, 0.3), (Commodity::Volatiles, 0.5)],
    },
    Converter {
        structure: StructureKind::ArmamentsComplex,
        output: Commodity::Slugs,
        rate: 0.60 * OUTPUT_SCALE,
        inputs: &[(Commodity::Alloys, 0.5), (Commodity::Titanium, 0.1)],
    },
    Converter {
        structure: StructureKind::ArmamentsComplex,
        output: Commodity::BeamCrystals,
        rate: 0.20 * OUTPUT_SCALE,
        inputs: &[(Commodity::CrystallineOre, 2.0), (Commodity::RareElements, 0.05)],
    },
];

pub const HULL_FABRICATOR_PRODUCTS: [Converter; 4] = [CONVERTERS[10], PART_RECIPES[0], SIZED_HULL_SECTIONS[0], SIZED_HULL_SECTIONS[1]];
pub const ELECTRONICS_FABRICATOR_PRODUCTS: [Converter; 2] = [CONVERTERS[2], PART_RECIPES[1]];
pub const ARMAMENTS_COMPLEX_PRODUCTS: [Converter; 6] = [CONVERTERS[8], PART_RECIPES[2], PART_RECIPES[3],
    MUNITION_RECIPES[0], MUNITION_RECIPES[1], MUNITION_RECIPES[2]];

/// §shipbuilding-overhaul: what a Drive Works line may make.
pub fn is_drive_works_product(c: Commodity) -> bool {
    is_product_of(StructureKind::DriveWorks, c)
}

/// §reactors-drives: the research a Drive Works line needs to make a part,
/// and the Drive Works tier. Rung 1 (Fission Reactor, Zephyr Drive) at tier I
/// (the structure's own gate, Autoforges); rung 2 (Fusion Reactor, Gale Drive)
/// at tier II; rung 3 (Advanced Fusion Reactor, Tempest Drive) after Helium-3
/// Cycle; rung 4 (Antimatter Reactor, Maelstrom Drive) after Antimatter
/// Synthesis. Buying waits for the same research (`crate::fuel::known_after`).
pub fn drive_works_gate(c: Commodity) -> (u32, Option<&'static str>) {
    // §hull-ladder: a sized part takes its light base's gate; its size's
    // programme is `size_gate`.
    let c = c.light_base();
    match c {
        Commodity::FissionReactor | Commodity::ZephyrDrive => (1, None),
        Commodity::FusionReactor | Commodity::GaleDrive => (2, None),
        Commodity::AdvancedFusionReactor | Commodity::TempestDrive => (1, Some(crate::power::FUSION_RESEARCH)),
        Commodity::AntimatterReactor | Commodity::MaelstromDrive => (1, Some("mat_foundry_vi_antimatter_synthesis")),
        _ => (u32::MAX, None),
    }
}

/// §hull-ladder: the research a sized part's SIZE needs at a works (heavy:
/// the Cruiser's line; capital: the Dreadnought's), besides its rung's.
pub fn size_gate(c: Commodity) -> Option<&'static str> {
    crate::fuel::size_known_after(c)
}

/// §reactors-drives: the Drive Works' eight recipes — a reactor and a drive
/// of each rung. Rates are slow on purpose (Nov 6 2026: a Drive Works at
/// 0.12/s against the Hub's unbounded demand was a credit faucet): one
/// Fission Reactor every 3 min 20 s at tier I, the others slower in
/// proportion to their inputs. Baskets sit ~20–30% under the Hub price
/// (`market::base_price`). Tunable.
pub const LIGHT_DRIVE_WORKS_PRODUCTS: [Converter; 8] = [
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::FissionReactor,
        rate: 0.005 * OUTPUT_SCALE,
        inputs: &[(Commodity::PrecisionComponents, 3.72), (Commodity::Composites, 2.33), (Commodity::Machinery, 1.40)],
    },
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::ZephyrDrive,
        rate: 0.008 * OUTPUT_SCALE,
        inputs: &[(Commodity::PrecisionComponents, 1.40), (Commodity::Composites, 1.40), (Commodity::Machinery, 1.40)],
    },
    FUSION_REACTOR_CONVERTER,
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::GaleDrive,
        rate: 0.005 * OUTPUT_SCALE,
        inputs: &[(Commodity::PrecisionComponents, 2.79), (Commodity::Composites, 2.33), (Commodity::Titanium, 1.40)],
    },
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::AdvancedFusionReactor,
        rate: 0.003 * OUTPUT_SCALE,
        inputs: &[(Commodity::PrecisionComponents, 10.23), (Commodity::Composites, 4.65), (Commodity::Iridium, 3.72),
            (Commodity::Titanium, 2.33)],
    },
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::TempestDrive,
        rate: 0.004 * OUTPUT_SCALE,
        inputs: &[(Commodity::PrecisionComponents, 4.65), (Commodity::Composites, 2.79), (Commodity::Iridium, 1.40),
            (Commodity::Titanium, 1.40)],
    },
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::AntimatterReactor,
        rate: 0.002 * OUTPUT_SCALE,
        inputs: &[(Commodity::PrecisionComponents, 11.62), (Commodity::Composites, 6.98), (Commodity::Iridium, 3.72),
            (Commodity::ExoticMatter, 2.33)],
    },
    Converter {
        structure: StructureKind::DriveWorks,
        output: Commodity::MaelstromDrive,
        rate: 0.003 * OUTPUT_SCALE,
        inputs: &[(Commodity::PrecisionComponents, 5.58), (Commodity::Composites, 2.79), (Commodity::Iridium, 1.86),
            (Commodity::ExoticMatter, 2.33)],
    },
];

/// The Drive Works' plant-reactor recipe — a Deuterium Fusion Plant's heart.
pub const FUSION_REACTOR_CONVERTER: Converter = Converter {
    structure: StructureKind::DriveWorks,
    output: Commodity::FusionReactor,
    rate: 0.005 * OUTPUT_SCALE,
    // §hull-ladder: baskets ×4.65 with the engine price (1,300 at the Hub).
    inputs: &[(Commodity::PrecisionComponents, 9.30), (Commodity::Composites, 2.80), (Commodity::Iridium, 1.85)],
};

/// §hull-ladder: a heavy part is assembled from 25 light ones, a capital part
/// from 25 heavy — one every 10 and 50 minutes on a line. Tunable.
const fn assembly(structure: StructureKind, output: Commodity, from: &'static [(Commodity, f64)], rate: f64) -> Converter {
    Converter { structure, output, rate, inputs: from }
}
macro_rules! sized_pairs {
    ($(($heavy:ident, $capital:ident, $light:ident)),* $(,)?) => {
        pub const SIZED_DRIVE_WORKS_PRODUCTS: [Converter; 16] = [
            $(assembly(StructureKind::DriveWorks, Commodity::$heavy, &[(Commodity::$light, 25.0)], 1.0 / 600.0),)*
            $(assembly(StructureKind::DriveWorks, Commodity::$capital, &[(Commodity::$heavy, 25.0)], 1.0 / 3000.0),)*
        ];
    };
}
sized_pairs!(
    (HeavyFissionReactor, CapitalFissionReactor, FissionReactor),
    (HeavyFusionReactor, CapitalFusionReactor, FusionReactor),
    (HeavyAdvancedFusionReactor, CapitalAdvancedFusionReactor, AdvancedFusionReactor),
    (HeavyAntimatterReactor, CapitalAntimatterReactor, AntimatterReactor),
    (HeavyZephyrDrive, CapitalZephyrDrive, ZephyrDrive),
    (HeavyGaleDrive, CapitalGaleDrive, GaleDrive),
    (HeavyTempestDrive, CapitalTempestDrive, TempestDrive),
    (HeavyMaelstromDrive, CapitalMaelstromDrive, MaelstromDrive),
);
/// The frame's two sizes, at the Hull Fabricator.
pub const SIZED_HULL_SECTIONS: [Converter; 2] = [
    assembly(StructureKind::HullFabricator, Commodity::HeavyHullSections, &[(Commodity::HullSections, 25.0)], 1.0 / 600.0),
    assembly(StructureKind::HullFabricator, Commodity::CapitalHullSections, &[(Commodity::HeavyHullSections, 25.0)], 1.0 / 3000.0),
];

/// Every Drive Works product: the eight light parts, then the sixteen sized.
pub const DRIVE_WORKS_PRODUCTS: [Converter; 24] = {
    let mut all = [LIGHT_DRIVE_WORKS_PRODUCTS[0]; 24];
    let mut i = 0;
    while i < 8 { all[i] = LIGHT_DRIVE_WORKS_PRODUCTS[i]; i += 1; }
    let mut k = 0;
    while k < 16 { all[8 + k] = SIZED_DRIVE_WORKS_PRODUCTS[k]; k += 1; }
    all
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
        rate: 1.0 * OUTPUT_SCALE,
        inputs: &[(Commodity::MetallicOre, 1.5)],
    },
    // §fuel-overhaul: the Uranium Mill sifts fissile grains out of Ferrite Ore —
    // two Uranium per ore, enough for about ten slow Tiny round trips an hour.
    Converter {
        structure: StructureKind::UraniumMill,
        output: Commodity::Uranium,
        rate: 0.05 * OUTPUT_SCALE,
        inputs: &[(Commodity::MetallicOre, 0.5)],
    },
    Converter {
        structure: StructureKind::ElectronicsFabricator,
        output: Commodity::Electronics,
        rate: 0.5 * OUTPUT_SCALE,
        // §ore-ladder: the dear refined goods are needed in TENTHS of a unit —
        // "a little goes a long way" is what makes a rare-metal haul worth the
        // trip without repricing every end product above it.
        inputs: &[(Commodity::ConductiveMetals, 0.5), (Commodity::RareElements, 0.1), (Commodity::Silicates, 0.6)],
    },
    Converter {
        structure: StructureKind::ChemicalWorks,
        output: Commodity::Polymers,
        rate: 0.8 * OUTPUT_SCALE,
        inputs: &[(Commodity::Volatiles, 1.0), (Commodity::Biomass, 0.8)],
    },
    Converter {
        structure: StructureKind::FuelRefinery,
        output: Commodity::Deuterium,
        rate: 0.8 * OUTPUT_SCALE,
        inputs: &[(Commodity::Volatiles, 1.0)],
    },
    Converter {
        structure: StructureKind::Agroplex,
        output: Commodity::Provisions,
        rate: 1.2 * OUTPUT_SCALE,
        inputs: &[(Commodity::Biomass, 1.0)],
    },
    // §gaia: Living Terraces farm their own gardens — the Agroplex's rate with
    // no Biomass basket. Gaia-only, so the free input is the world's gift.
    Converter {
        structure: StructureKind::LivingTerraces,
        output: Commodity::Provisions,
        rate: 1.2 * OUTPUT_SCALE,
        inputs: &[],
    },
    Converter {
        structure: StructureKind::MachineWorks,
        output: Commodity::Machinery,
        rate: 0.3 * OUTPUT_SCALE,
        inputs: &[
            (Commodity::Alloys, 1.2),
            (Commodity::Electronics, 0.6),
        ],
    },
    Converter {
        structure: StructureKind::ArmamentsComplex,
        output: Commodity::Armaments,
        rate: 0.35 * OUTPUT_SCALE,
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
        rate: 0.30 * OUTPUT_SCALE,
        inputs: &[(Commodity::Alloys, 1.0), (Commodity::Polymers, 0.75), (Commodity::Silicates, 0.4)],
    },
    Converter {
        structure: StructureKind::HullFabricator,
        output: Commodity::HullSections,
        rate: 0.18 * OUTPUT_SCALE,
        inputs: &[(Commodity::Composites, 1.2), (Commodity::Machinery, 0.5), (Commodity::Titanium, 0.4)],
    },
    Converter {
        structure: StructureKind::PrecisionWorks,
        output: Commodity::PrecisionComponents,
        rate: 0.20 * OUTPUT_SCALE,
        inputs: &[(Commodity::Electronics, 1.0), (Commodity::Machinery, 0.5), (Commodity::RareElements, 0.1)],
    },
    // §shipbuilding-overhaul: the Fission Core, the Drive Works' default line
    // (`DRIVE_WORKS_PRODUCTS[0]`). §fuel-overhaul: no furnace fuel (double power).
    DRIVE_WORKS_PRODUCTS[0],
    // §fuel-overhaul: the last tech's factory. Two Deuterium and ten times a
    // factory's power per unit: antimatter is always a net energy loss.
    Converter {
        structure: StructureKind::AntimatterPlant,
        output: Commodity::Antimatter,
        rate: 0.005 * OUTPUT_SCALE,
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
    /// §asteroid-quarry: the system can't pay a quarry's upkeep.
    NoSupplies,
}

impl SuspendReason {
    pub fn slug(self) -> &'static str {
        match self {
            SuspendReason::NoFood => "no_food",
            SuspendReason::NoInputs => "no_inputs",
            SuspendReason::StorageFull => "storage_full",
            SuspendReason::FoodReserve => "food_reserve",
            SuspendReason::NoSupplies => "no_supplies",
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

/// §research-wiring Expert Training: skill points per affine specialist and
/// per expert. A line full of experts reaches ×2.25.
pub const SPECIALIST_SKILL_POINTS: u32 = 3;
pub const EXPERT_SKILL_POINTS: u32 = 5;

/// A line's skill points from its affine crew: the best `tier` of them,
/// experts first.
pub fn skill_points(experts: u32, specialists: u32, tier: u32) -> u32 {
    let e = experts.min(tier);
    e * EXPERT_SKILL_POINTS + specialists.min(tier - e) * SPECIALIST_SKILL_POINTS
}

/// [`skill_factor`] from skill points ([`skill_points`]). A line without
/// experts reads exactly as [`skill_factor`] does.
pub fn skill_factor_points(points: u32, tier: u32) -> f64 {
    if points % SPECIALIST_SKILL_POINTS == 0 && points <= SPECIALIST_SKILL_POINTS * tier {
        return skill_factor(points / SPECIALIST_SKILL_POINTS, tier);
    }
    1.0 + (SPECIALIST_SKILL_MULT - 1.0) * (points.min(EXPERT_SKILL_POINTS * tier) as f64
        / (SPECIALIST_SKILL_POINTS * tier) as f64)
}

/// The FOOD factor of a structure's output — `FoodState::efficiency()` shaped
/// by sector: the primary sector (extraction + Agroplex) never drops below
/// `EXTRACTION_FOOD_FLOOR`; ADVANCED industry (machinery, armaments and components)
/// suspends outright at Critical (precision work stops before the mills do).
pub fn food_factor(kind: StructureKind, state: crate::colony::FoodState) -> f64 {
    food_factor_with(kind, state, 1.0)
}

/// [`food_factor`] with Civic Rations: `rationing` multiplies the Rationing
/// state's efficiency (1.0 without the research).
pub fn food_factor_with(kind: StructureKind, state: crate::colony::FoodState, rationing: f64) -> f64 {
    use crate::colony::FoodState as F;
    let eff = state.efficiency() * if state == F::Rationing { rationing } else { 1.0 };
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

#[cfg(test)]
mod product_tests {
    use super::*;

    /// §shipbuilding-overhaul: the indices the product tables borrow from
    /// `CONVERTERS` point at the works they name.
    #[test]
    fn multi_product_works_list_their_default_first() {
        assert_eq!(HULL_FABRICATOR_PRODUCTS[0].output, Commodity::HullSections);
        assert_eq!(ELECTRONICS_FABRICATOR_PRODUCTS[0].output, Commodity::Electronics);
        assert_eq!(ARMAMENTS_COMPLEX_PRODUCTS[0].output, Commodity::Armaments);
        for (kind, products) in [(StructureKind::HullFabricator, &HULL_FABRICATOR_PRODUCTS[..]),
            (StructureKind::ElectronicsFabricator, &ELECTRONICS_FABRICATOR_PRODUCTS[..]),
            (StructureKind::ArmamentsComplex, &ARMAMENTS_COMPLEX_PRODUCTS[..]),
            (StructureKind::DriveWorks, &DRIVE_WORKS_PRODUCTS[..])]
        {
            assert!(products.iter().all(|c| c.structure == kind), "{kind:?}");
            assert_eq!(assigned_converter(kind, None).unwrap().output, products[0].output);
            assert!(has_product_picker(kind));
            let mut line = Assignment::crew(1);
            line.refining_ore = Some(products[products.len() - 1].output);
            assert_eq!(assigned_converter(kind, Some(&line)).unwrap().output, products[products.len() - 1].output);
            line.refining_ore = Some(Commodity::Biomass);
            assert_eq!(assigned_converter(kind, Some(&line)).unwrap().output, products[0].output, "an unknown product falls back");
        }
        assert!(!has_product_picker(StructureKind::MachineWorks));
        assert!(is_product_of(StructureKind::ArmamentsComplex, Commodity::BeamBattery));
        assert!(!is_product_of(StructureKind::HullFabricator, Commodity::BeamBattery));
        for c in Commodity::PARTS.iter().chain(crate::munitions::MUNITIONS.iter()) {
            assert!(converter_for_output(*c).is_some(), "{c:?} is made somewhere");
        }
        assert!(is_product_of(StructureKind::ArmamentsComplex, Commodity::Torpedoes));
    }
}

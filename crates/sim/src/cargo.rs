//! Cargo carried by convoys. A convoy *broadcasts* its identity and position
//! (the Galactic Convention, §6) but NOT its cargo — cargo contents are only
//! revealed to a player whose sensors are within range of the convoy (the
//! two-tier information model).
//!
//! The industrial web separates five mineable ore families from their refined
//! materials. Raw exports need no refinery; refining draws power and staffed
//! capacity. Legacy Silicates/Rare Elements deposits remain mineable in saves.
//!
//! §fuel-overhaul / §shipbuilding-overhaul: ships burn four fuels, one per
//! warp-core grade. Uranium (milled from Ferrite Ore) feeds a Fission Core,
//! Deuterium (today's Fuel, renamed) a Fusion Core, Helium-3 a Helium Core, and
//! Antimatter a Fold Core, which also folds space for jumps.

use serde::{Deserialize, Serialize};

/// Which rung of the industrial web a commodity sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommodityTier {
    /// Occurs naturally as deposits; extracted.
    Raw,
    /// Made from raws in processing structures.
    Processed,
    /// Manufactured equipment and the hull/drive component chains.
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Commodity {
    // Historical ordering is stable. RareElements/Silicates now classify as
    // refined goods, but direct deposits in older saves remain extractable.
    /// Ferrite Ore in the UI. Retain both historical wire names and cargo.
    #[serde(alias = "ore")]
    MetallicOre,
    RareElements,
    Silicates,
    Volatiles,
    Biomass,
    // ── Processed ────────────────────────────────────────────────────────────
    /// MetallicOre, in a Smelter.
    Alloys,
    Electronics,
    Polymers,
    /// §fuel-overhaul: the full-warp and fusion-plant fuel (formerly "Fuel").
    /// Refined from Volatiles in a Deuterium Refinery, or bought at the Hub.
    #[serde(alias = "fuel")]
    Deuterium,
    /// Biomass-derived only, in an Agroplex.
    Provisions,
    // ── Advanced ─────────────────────────────────────────────────────────────
    Machinery,
    Armaments,
    Composites,
    HullSections,
    PrecisionComponents,
    /// §reactors-drives (docs/reactor-drive-split.md): the Fission Reactor,
    /// rung 1 of the reactor ladder (formerly the Fission Core / Warp Core).
    /// Burns Uranium. Every hull carries one per warp drive
    /// (`ShipKind::engines`).
    FissionReactor,
    // Append only: preserve historical enum ordering as well as wire slugs.
    CupriteOre,
    TitaniumOre,
    CrystallineOre,
    RareMetalOre,
    ConductiveMetals,
    Titanium,
    // §belt-ores: mixed ores that occur only in asteroid belts. Each refines
    // into goods the web already makes, in a different mix (EVE's lesson:
    // many ores, few minerals). Planets keep the original five.
    OlivineOre,
    BorniteOre,
    IlmeniteOre,
    // §pockets: the rare pocket-only ores and the top-end materials they
    // refine into. Found only by prospecting a belt's hidden pockets.
    IridiumOre,
    /// An ice: harvested like Volatiles, refined in the Smelter.
    Helium3Ice,
    VoidCrystal,
    Iridium,
    Helium3,
    ExoticMatter,
    // §fuel-overhaul.
    /// Slow-warp fission fuel, milled from Ferrite Ore.
    Uranium,
    /// Jump fuel. The Authority sells it until Antimatter Synthesis.
    Antimatter,
    /// Rung 2: burns Deuterium (4 energy a unit); the heart of a Deuterium
    /// Fusion Plant (tiers III–IV) too. §reactors-drives: one reactor family
    /// for ships and plants.
    FusionReactor,
    /// Rung 3: burns Helium-3 (24 a unit); the heart of a Helium-3 Fusion
    /// Plant (tiers V–VI) too.
    AdvancedFusionReactor,
    // §shipbuilding-overhaul: the other hull parts.
    /// Every hull's flight computer (Semiconductor Fab).
    NavComputer,
    /// One per 50 units of hold (Hull Fabricator).
    CargoBay,
    /// The light gun: firepower 0.5 (Armaments Complex).
    PulseLaser,
    /// The line gun: firepower 2.0 (Armaments Complex).
    BeamBattery,
    /// Rung 4: burns Antimatter (96 a unit); the heart of the Antimatter Plant.
    AntimatterReactor,
    // §reactors-drives: the four warp drives, by rung. A drive needs a reactor
    // of at least its rung (`fuel::ReactorGrade::powers`).
    /// Rung 1: Slow and Full at rated speed. The charter's drive.
    ZephyrDrive,
    /// Rung 2: ×1.2 speed, Stealth.
    GaleDrive,
    /// Rung 3: ×1.4 speed.
    TempestDrive,
    /// Rung 4: ×1.6 speed, and it jumps.
    MaelstromDrive,
    // §hull-ladder: the heavy and capital sizes of the engine parts and the
    // frame, each ×25 the size below and assembled from 25 of it
    // (`fuel::EngineSize`). Hidden until the Cruiser's / Dreadnought's line.
    HeavyFissionReactor,
    HeavyFusionReactor,
    HeavyAdvancedFusionReactor,
    HeavyAntimatterReactor,
    HeavyZephyrDrive,
    HeavyGaleDrive,
    HeavyTempestDrive,
    HeavyMaelstromDrive,
    HeavyHullSections,
    CapitalFissionReactor,
    CapitalFusionReactor,
    CapitalAdvancedFusionReactor,
    CapitalAntimatterReactor,
    CapitalZephyrDrive,
    CapitalGaleDrive,
    CapitalTempestDrive,
    CapitalMaelstromDrive,
    CapitalHullSections,
    // §shipbuilding-overhaul Phase 5: the munitions (`crate::munitions`).
    /// A Torpedo Rack's rounds: one unit one shot.
    Torpedoes,
    /// A Mass Driver's rounds: one unit one shot.
    Slugs,
    /// A beam gun's focusing crystal: one per gun, it wears.
    BeamCrystals,
    // §modules-as-goods (Nov 8 2026): the ship modules are goods like the
    // parts — one stockpile, the Hub book, holds and freight — one per
    // `crate::module::ModuleKind`, with its slug (`ModuleKind::good`).
    MassDriver,
    TorpedoRack,
    PointDefenseScreen,
    ReflectivePlating,
    WhippleArmor,
    ExtendedTanks,
    ReconSuite,
    CargoPods,
    EscortDatalink,
    FuelTransferRig,
    SurveyDrive,
    NebulaSpectrometer,
    PrismaticLance,
    StasisWeb,
    WarpStabilizer,
    InterdictionField,
    LanceArray,
}

/// §cargo-mass (Nov 7 2026): mass per unit of cargo by class
/// (`Commodity::mass_per_unit`): raw ore, ice and harvest; refined bulk;
/// precision parts and cores; fuels and munitions. The raw figure is the flat
/// per-unit mass every good had before, so ore trips cost what they did.
/// Tunable.
pub const MASS_RAW: f64 = 28.0;
pub const MASS_BULK: f64 = 14.0;
pub const MASS_PARTS: f64 = 7.0;
pub const MASS_SUPPLY: f64 = 4.0;

/// §hold-room (Peter, Nov 10 2026: "Does 1 unit of ore take the same cargo
/// room as 1 unit of semi conductors? ... Shouldn't it?"): hold room is
/// counted in quarters of a room unit, so every class shares one integer.
/// A hold of 50 is 200 quarters.
pub const ROOM_SCALE: u32 = 4;
/// Quarters one unit fills (`Commodity::room_quarters`): ore, ice and raw
/// harvest a whole room unit; refined bulk, Provisions and fuel half; parts,
/// cores, modules, Semiconductors and munitions a quarter. Tunable.
pub const ROOM_RAW: u32 = 4;
pub const ROOM_BULK: u32 = 2;
pub const ROOM_FUEL: u32 = 2;
pub const ROOM_PARTS: u32 = 1;
/// Peter (Nov 10 2026): "Just make the capital parts take 10 volume units":
/// every capital reactor, drive and hull section. A Dreadnought's 97 fill a
/// Large Freighter; a Titan's 981 are two Bulk Freighter loads.
pub const ROOM_CAPITAL: u32 = 10 * ROOM_SCALE;

/// Quarters a manifest fills.
pub fn manifest_quarters<'a>(goods: impl IntoIterator<Item = (&'a Commodity, &'a u32)>) -> u64 {
    goods.into_iter().map(|(c, n)| *n as u64 * c.room_quarters() as u64).sum()
}

impl Commodity {
    pub const ALL: [Commodity; 82] = [
        Commodity::MetallicOre,
        Commodity::RareElements,
        Commodity::Silicates,
        Commodity::Volatiles,
        Commodity::Biomass,
        Commodity::Alloys,
        Commodity::Electronics,
        Commodity::Polymers,
        Commodity::Deuterium,
        Commodity::Provisions,
        Commodity::Machinery,
        Commodity::Armaments,
        Commodity::Composites,
        Commodity::HullSections,
        Commodity::PrecisionComponents,
        Commodity::FissionReactor,
        Commodity::CupriteOre,
        Commodity::TitaniumOre,
        Commodity::CrystallineOre,
        Commodity::RareMetalOre,
        Commodity::ConductiveMetals,
        Commodity::Titanium,
        Commodity::OlivineOre,
        Commodity::BorniteOre,
        Commodity::IlmeniteOre,
        Commodity::IridiumOre,
        Commodity::Helium3Ice,
        Commodity::VoidCrystal,
        Commodity::Iridium,
        Commodity::Helium3,
        Commodity::ExoticMatter,
        Commodity::Uranium,
        Commodity::Antimatter,
        Commodity::FusionReactor,
        Commodity::AdvancedFusionReactor,
        Commodity::NavComputer,
        Commodity::CargoBay,
        Commodity::PulseLaser,
        Commodity::BeamBattery,
        Commodity::Torpedoes,
        Commodity::Slugs,
        Commodity::BeamCrystals,
        Commodity::AntimatterReactor,
        Commodity::ZephyrDrive,
        Commodity::GaleDrive,
        Commodity::TempestDrive,
        Commodity::MaelstromDrive,
        Commodity::HeavyFissionReactor,
        Commodity::HeavyFusionReactor,
        Commodity::HeavyAdvancedFusionReactor,
        Commodity::HeavyAntimatterReactor,
        Commodity::HeavyZephyrDrive,
        Commodity::HeavyGaleDrive,
        Commodity::HeavyTempestDrive,
        Commodity::HeavyMaelstromDrive,
        Commodity::HeavyHullSections,
        Commodity::CapitalFissionReactor,
        Commodity::CapitalFusionReactor,
        Commodity::CapitalAdvancedFusionReactor,
        Commodity::CapitalAntimatterReactor,
        Commodity::CapitalZephyrDrive,
        Commodity::CapitalGaleDrive,
        Commodity::CapitalTempestDrive,
        Commodity::CapitalMaelstromDrive,
        Commodity::CapitalHullSections,
        Commodity::MassDriver,
        Commodity::TorpedoRack,
        Commodity::PointDefenseScreen,
        Commodity::ReflectivePlating,
        Commodity::WhippleArmor,
        Commodity::ExtendedTanks,
        Commodity::ReconSuite,
        Commodity::CargoPods,
        Commodity::EscortDatalink,
        Commodity::FuelTransferRig,
        Commodity::SurveyDrive,
        Commodity::NebulaSpectrometer,
        Commodity::PrismaticLance,
        Commodity::StasisWeb,
        Commodity::WarpStabilizer,
        Commodity::InterdictionField,
        Commodity::LanceArray,
    ];

    /// New deposits contain ores, not already-separated refined materials.
    pub const RAW: [Commodity; 13] = [
        Commodity::MetallicOre,
        Commodity::CupriteOre,
        Commodity::TitaniumOre,
        Commodity::CrystallineOre,
        Commodity::RareMetalOre,
        Commodity::Volatiles,
        Commodity::Biomass,
        Commodity::OlivineOre,
        Commodity::BorniteOre,
        Commodity::IlmeniteOre,
        Commodity::IridiumOre,
        Commodity::Helium3Ice,
        Commodity::VoidCrystal,
    ];

    /// Which rung of the industrial web this commodity sits on.
    pub fn tier(self) -> CommodityTier {
        match self {
            Commodity::MetallicOre
            | Commodity::CupriteOre
            | Commodity::TitaniumOre
            | Commodity::CrystallineOre
            | Commodity::RareMetalOre
            | Commodity::Volatiles
            | Commodity::Biomass
            | Commodity::OlivineOre
            | Commodity::BorniteOre
            | Commodity::IlmeniteOre
            | Commodity::IridiumOre
            | Commodity::Helium3Ice
            | Commodity::VoidCrystal => CommodityTier::Raw,
            Commodity::Alloys
            | Commodity::RareElements
            | Commodity::Silicates
            | Commodity::ConductiveMetals
            | Commodity::Titanium
            | Commodity::Electronics
            | Commodity::Polymers
            | Commodity::Deuterium
            | Commodity::Provisions
            | Commodity::Iridium
            | Commodity::Helium3
            | Commodity::ExoticMatter
            | Commodity::Uranium
            | Commodity::Antimatter => CommodityTier::Processed,
            Commodity::Machinery | Commodity::Armaments | Commodity::Composites
            | Commodity::HullSections | Commodity::PrecisionComponents
            | Commodity::FissionReactor | Commodity::FusionReactor | Commodity::AdvancedFusionReactor
            | Commodity::AntimatterReactor
            | Commodity::ZephyrDrive | Commodity::GaleDrive | Commodity::TempestDrive | Commodity::MaelstromDrive
            | Commodity::HeavyFissionReactor
            | Commodity::HeavyFusionReactor
            | Commodity::HeavyAdvancedFusionReactor
            | Commodity::HeavyAntimatterReactor
            | Commodity::HeavyZephyrDrive
            | Commodity::HeavyGaleDrive
            | Commodity::HeavyTempestDrive
            | Commodity::HeavyMaelstromDrive
            | Commodity::HeavyHullSections
            | Commodity::CapitalFissionReactor
            | Commodity::CapitalFusionReactor
            | Commodity::CapitalAdvancedFusionReactor
            | Commodity::CapitalAntimatterReactor
            | Commodity::CapitalZephyrDrive
            | Commodity::CapitalGaleDrive
            | Commodity::CapitalTempestDrive
            | Commodity::CapitalMaelstromDrive
            | Commodity::CapitalHullSections
            | Commodity::NavComputer | Commodity::CargoBay | Commodity::PulseLaser | Commodity::BeamBattery
            | Commodity::Torpedoes | Commodity::Slugs | Commodity::BeamCrystals
            | Commodity::MassDriver
            | Commodity::TorpedoRack
            | Commodity::PointDefenseScreen
            | Commodity::ReflectivePlating
            | Commodity::WhippleArmor
            | Commodity::ExtendedTanks
            | Commodity::ReconSuite
            | Commodity::CargoPods
            | Commodity::EscortDatalink
            | Commodity::FuelTransferRig
            | Commodity::SurveyDrive
            | Commodity::NebulaSpectrometer
            | Commodity::PrismaticLance
            | Commodity::StasisWeb
            | Commodity::WarpStabilizer
            | Commodity::InterdictionField
            | Commodity::LanceArray
                => CommodityTier::Advanced,
        }
    }

    /// The snake_case wire slug (matches `rename_all = "snake_case"`).
    /// Human name for prose (news, the log); the wire uses [`Self::slug`].
    pub fn name(self) -> &'static str {
        match self {
            Commodity::MetallicOre => "ferrite ore",
            Commodity::RareElements => "rare elements",
            Commodity::Silicates => "silicates",
            Commodity::Volatiles => "volatiles",
            Commodity::Biomass => "biomass",
            Commodity::Alloys => "alloys",
            Commodity::Electronics => "semiconductors",
            Commodity::Polymers => "polymers",
            Commodity::Deuterium => "deuterium",
            Commodity::Provisions => "provisions",
            Commodity::Machinery => "machinery",
            // §shipbuilding-overhaul: the hardware good shows as Weapon Mounts.
            Commodity::Armaments => "weapon mounts",
            Commodity::Composites => "composites",
            Commodity::HullSections => "hull sections",
            Commodity::PrecisionComponents => "precision components",
            Commodity::FissionReactor => "fission reactors",
            Commodity::CupriteOre => "cuprite ore",
            Commodity::TitaniumOre => "titanium ore",
            Commodity::CrystallineOre => "crystalline ore",
            Commodity::RareMetalOre => "rare-metal ore",
            Commodity::ConductiveMetals => "conductive metals",
            Commodity::Titanium => "titanium",
            Commodity::OlivineOre => "olivine ore",
            Commodity::BorniteOre => "bornite ore",
            Commodity::IlmeniteOre => "ilmenite ore",
            Commodity::IridiumOre => "iridium ore",
            Commodity::Helium3Ice => "helium-3 ice",
            Commodity::VoidCrystal => "void crystal",
            Commodity::Iridium => "iridium",
            Commodity::Helium3 => "helium-3",
            Commodity::ExoticMatter => "exotic matter",
            Commodity::Uranium => "uranium",
            Commodity::Antimatter => "antimatter",
            Commodity::FusionReactor => "fusion reactors",
            Commodity::AdvancedFusionReactor => "advanced fusion reactors",
            Commodity::AntimatterReactor => "antimatter reactors",
            Commodity::ZephyrDrive => "zephyr drives",
            Commodity::GaleDrive => "gale drives",
            Commodity::TempestDrive => "tempest drives",
            Commodity::MaelstromDrive => "maelstrom drives",
            Commodity::HeavyFissionReactor => "heavy fission reactors",
            Commodity::HeavyFusionReactor => "heavy fusion reactors",
            Commodity::HeavyAdvancedFusionReactor => "heavy advanced fusion reactors",
            Commodity::HeavyAntimatterReactor => "heavy antimatter reactors",
            Commodity::HeavyZephyrDrive => "heavy zephyr drives",
            Commodity::HeavyGaleDrive => "heavy gale drives",
            Commodity::HeavyTempestDrive => "heavy tempest drives",
            Commodity::HeavyMaelstromDrive => "heavy maelstrom drives",
            Commodity::HeavyHullSections => "heavy hull sections",
            Commodity::CapitalFissionReactor => "capital fission reactors",
            Commodity::CapitalFusionReactor => "capital fusion reactors",
            Commodity::CapitalAdvancedFusionReactor => "capital advanced fusion reactors",
            Commodity::CapitalAntimatterReactor => "capital antimatter reactors",
            Commodity::CapitalZephyrDrive => "capital zephyr drives",
            Commodity::CapitalGaleDrive => "capital gale drives",
            Commodity::CapitalTempestDrive => "capital tempest drives",
            Commodity::CapitalMaelstromDrive => "capital maelstrom drives",
            Commodity::CapitalHullSections => "capital hull sections",
            Commodity::NavComputer => "nav computers",
            Commodity::CargoBay => "cargo bays",
            Commodity::PulseLaser => "pulse lasers",
            Commodity::BeamBattery => "beam batteries",
            Commodity::Torpedoes => "torpedoes",
            Commodity::Slugs => "slugs",
            Commodity::BeamCrystals => "beam crystals",
            Commodity::MassDriver => "mass drivers",
            Commodity::TorpedoRack => "torpedo racks",
            Commodity::PointDefenseScreen => "point-defense screens",
            Commodity::ReflectivePlating => "reflective plating",
            Commodity::WhippleArmor => "whipple armor",
            Commodity::ExtendedTanks => "extended tanks",
            Commodity::ReconSuite => "recon suites",
            Commodity::CargoPods => "cargo pods",
            Commodity::EscortDatalink => "escort datalinks",
            Commodity::FuelTransferRig => "fuel transfer rigs",
            Commodity::SurveyDrive => "survey boosters",
            Commodity::NebulaSpectrometer => "nebula spectrometers",
            Commodity::PrismaticLance => "prismatic lances",
            Commodity::StasisWeb => "stasis webs",
            Commodity::WarpStabilizer => "warp stabilizers",
            Commodity::InterdictionField => "interdiction fields",
            Commodity::LanceArray => "lance arrays",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Commodity::MetallicOre => "metallic_ore",
            Commodity::RareElements => "rare_elements",
            Commodity::Silicates => "silicates",
            Commodity::Volatiles => "volatiles",
            Commodity::Biomass => "biomass",
            Commodity::Alloys => "alloys",
            Commodity::Electronics => "electronics",
            Commodity::Polymers => "polymers",
            Commodity::Deuterium => "deuterium",
            Commodity::Provisions => "provisions",
            Commodity::Machinery => "machinery",
            Commodity::Armaments => "armaments",
            Commodity::Composites => "composites",
            Commodity::HullSections => "hull_sections",
            Commodity::PrecisionComponents => "precision_components",
            Commodity::FissionReactor => "fission_reactor",
            Commodity::CupriteOre => "cuprite_ore",
            Commodity::TitaniumOre => "titanium_ore",
            Commodity::CrystallineOre => "crystalline_ore",
            Commodity::RareMetalOre => "rare_metal_ore",
            Commodity::ConductiveMetals => "conductive_metals",
            Commodity::Titanium => "titanium",
            Commodity::OlivineOre => "olivine_ore",
            Commodity::BorniteOre => "bornite_ore",
            Commodity::IlmeniteOre => "ilmenite_ore",
            Commodity::IridiumOre => "iridium_ore",
            Commodity::Helium3Ice => "helium3_ice",
            Commodity::VoidCrystal => "void_crystal",
            Commodity::Iridium => "iridium",
            Commodity::Helium3 => "helium3",
            Commodity::ExoticMatter => "exotic_matter",
            Commodity::Uranium => "uranium",
            Commodity::Antimatter => "antimatter",
            Commodity::FusionReactor => "fusion_reactor",
            Commodity::AdvancedFusionReactor => "advanced_fusion_reactor",
            Commodity::AntimatterReactor => "antimatter_reactor",
            Commodity::ZephyrDrive => "zephyr_drive",
            Commodity::GaleDrive => "gale_drive",
            Commodity::TempestDrive => "tempest_drive",
            Commodity::MaelstromDrive => "maelstrom_drive",
            Commodity::HeavyFissionReactor => "heavy_fission_reactor",
            Commodity::HeavyFusionReactor => "heavy_fusion_reactor",
            Commodity::HeavyAdvancedFusionReactor => "heavy_advanced_fusion_reactor",
            Commodity::HeavyAntimatterReactor => "heavy_antimatter_reactor",
            Commodity::HeavyZephyrDrive => "heavy_zephyr_drive",
            Commodity::HeavyGaleDrive => "heavy_gale_drive",
            Commodity::HeavyTempestDrive => "heavy_tempest_drive",
            Commodity::HeavyMaelstromDrive => "heavy_maelstrom_drive",
            Commodity::HeavyHullSections => "heavy_hull_sections",
            Commodity::CapitalFissionReactor => "capital_fission_reactor",
            Commodity::CapitalFusionReactor => "capital_fusion_reactor",
            Commodity::CapitalAdvancedFusionReactor => "capital_advanced_fusion_reactor",
            Commodity::CapitalAntimatterReactor => "capital_antimatter_reactor",
            Commodity::CapitalZephyrDrive => "capital_zephyr_drive",
            Commodity::CapitalGaleDrive => "capital_gale_drive",
            Commodity::CapitalTempestDrive => "capital_tempest_drive",
            Commodity::CapitalMaelstromDrive => "capital_maelstrom_drive",
            Commodity::CapitalHullSections => "capital_hull_sections",
            Commodity::NavComputer => "nav_computer",
            Commodity::CargoBay => "cargo_bay",
            Commodity::PulseLaser => "pulse_laser",
            Commodity::BeamBattery => "beam_battery",
            Commodity::Torpedoes => "torpedoes",
            Commodity::Slugs => "slugs",
            Commodity::BeamCrystals => "beam_crystals",
            Commodity::MassDriver => "mass_driver",
            Commodity::TorpedoRack => "torpedo_rack",
            Commodity::PointDefenseScreen => "point_defense_screen",
            Commodity::ReflectivePlating => "reflective_plating",
            Commodity::WhippleArmor => "whipple_armor",
            Commodity::ExtendedTanks => "extended_tanks",
            Commodity::ReconSuite => "recon_suite",
            Commodity::CargoPods => "cargo_pods",
            Commodity::EscortDatalink => "escort_datalink",
            Commodity::FuelTransferRig => "fuel_transfer_rig",
            Commodity::SurveyDrive => "survey_drive",
            Commodity::NebulaSpectrometer => "nebula_spectrometer",
            Commodity::PrismaticLance => "prismatic_lance",
            Commodity::StasisWeb => "stasis_web",
            Commodity::WarpStabilizer => "warp_stabilizer",
            Commodity::InterdictionField => "interdiction_field",
            Commodity::LanceArray => "lance_array",
        }
    }

    /// §reactors-drives: the four reactors, by rung (ships and plants alike).
    pub const REACTORS: [Commodity; 4] = [Commodity::FissionReactor, Commodity::FusionReactor,
        Commodity::AdvancedFusionReactor, Commodity::AntimatterReactor];
    /// The four warp drives, by rung.
    pub const DRIVES: [Commodity; 4] = [Commodity::ZephyrDrive, Commodity::GaleDrive,
        Commodity::TempestDrive, Commodity::MaelstromDrive];
    /// The parts a hull is built from besides its engines (the "kit").
    pub const PARTS: [Commodity; 5] = [Commodity::HullSections, Commodity::NavComputer, Commodity::CargoBay,
        Commodity::PulseLaser, Commodity::BeamBattery];
    /// §modules-as-goods: the module goods, in `crate::module::MODULE_KINDS` order.
    pub const MODULES: [Commodity; 17] = [
        Commodity::MassDriver,
        Commodity::TorpedoRack,
        Commodity::PointDefenseScreen,
        Commodity::ReflectivePlating,
        Commodity::WhippleArmor,
        Commodity::ExtendedTanks,
        Commodity::ReconSuite,
        Commodity::CargoPods,
        Commodity::EscortDatalink,
        Commodity::FuelTransferRig,
        Commodity::SurveyDrive,
        Commodity::NebulaSpectrometer,
        Commodity::PrismaticLance,
        Commodity::StasisWeb,
        Commodity::WarpStabilizer,
        Commodity::InterdictionField,
        Commodity::LanceArray,
    ];

    /// §modules-as-goods: the module this good is, if it is one.
    pub fn module(self) -> Option<crate::module::ModuleKind> {
        crate::module::MODULE_KINDS.into_iter().find(|m| m.good() == self)
    }
    /// A ship module (weapon, armour, control or utility).
    pub fn is_module(self) -> bool { Self::MODULES.contains(&self) }

    /// The light base of a sized part, else the good itself.
    pub fn light_base(self) -> Commodity {
        crate::fuel::sized_part(self).map_or(self, |(_, base)| base)
    }
    /// A reactor of any rung and size.
    pub fn is_reactor(self) -> bool { Self::REACTORS.contains(&self.light_base()) }
    /// A warp drive of any rung and size.
    pub fn is_drive(self) -> bool { Self::DRIVES.contains(&self.light_base()) }
    /// An engine part: a reactor or a drive, any size.
    pub fn is_engine_part(self) -> bool { self.is_reactor() || self.is_drive() }
    /// A hull part: an engine part, a frame of any size or one of the kit parts.
    pub fn is_part(self) -> bool { self.is_engine_part() || Self::PARTS.contains(&self.light_base()) }
    /// The fuels a reactor burns.
    pub fn is_fuel(self) -> bool {
        matches!(self, Self::Uranium | Self::Deuterium | Self::Helium3 | Self::Antimatter)
    }
    /// §shipbuilding-overhaul Phase 5: a magazine good.
    pub fn is_munition(self) -> bool {
        matches!(self, Self::Torpedoes | Self::Slugs | Self::BeamCrystals)
    }
    /// A fuel or a munition: what a fleet carries in a tank or a magazine and
    /// a dock, the Hub or a tender fills.
    pub fn is_supply(self) -> bool { self.is_fuel() || self.is_munition() }

    pub fn is_ore(self) -> bool {
        matches!(self, Self::MetallicOre | Self::CupriteOre | Self::TitaniumOre
            | Self::CrystallineOre | Self::RareMetalOre
            | Self::OlivineOre | Self::BorniteOre | Self::IlmeniteOre
            | Self::IridiumOre | Self::VoidCrystal)
    }

    /// Smelter feedstock: every ore, plus Helium-3 ice, which is harvested
    /// like Volatiles but refined in the Smelter.
    pub fn is_smeltable(self) -> bool {
        self.is_ore() || self == Self::Helium3Ice
    }

    /// §cargo-mass (Nov 7 2026): what one unit weighs in a hold, the mass that
    /// burns fuel (`crate::ship::Fleet::cargo_mass`). Raw ore, ice and bulk
    /// harvest are the heaviest, refined bulk half that, precision parts and
    /// cores a quarter, fuels and munitions lighter still: a hold of ore costs
    /// twice the fuel of a hold of Alloys, and a colony's fuel is cheap to
    /// haul. Hold ROOM is its own rule (`Self::room_quarters`).
    pub fn mass_per_unit(self) -> f64 {
        // §hull-ladder: a heavy part weighs 25 light ones, a capital part 625.
        if let Some((size, base)) = crate::fuel::sized_part(self) {
            if size != crate::fuel::EngineSize::Light { return base.mass_per_unit() * size.factor(); }
        }
        if self.is_supply() {
            return MASS_SUPPLY;
        }
        if self.is_raw() {
            return MASS_RAW;
        }
        if self.is_compact() {
            return MASS_PARTS;
        }
        MASS_BULK
    }

    /// Raw ore, ice and bulk harvest: the heaviest, bulkiest class.
    pub fn is_raw(self) -> bool {
        self.is_ore() || matches!(self, Self::Silicates | Self::Volatiles | Self::Biomass | Self::Helium3Ice)
    }

    /// Precision parts, cores and modules (§modules-as-goods: modules count as
    /// parts), any size.
    pub fn is_compact(self) -> bool {
        self.is_engine_part() || self.is_module() || matches!(self, Self::Electronics | Self::PrecisionComponents
            | Self::NavComputer | Self::PulseLaser | Self::BeamBattery | Self::ExoticMatter)
    }

    /// §hold-room (Nov 10 2026): hold room one unit fills, in quarters
    /// (`ROOM_SCALE`), by the weight classes: a hold of ore is a hold of ore,
    /// the Alloys smelted from it pack twice as tight, parts four times.
    /// Fuel packs as bulk (early Deuterium stays dear to haul); munitions as
    /// parts. A heavy part takes its light part's room; a capital part
    /// `ROOM_CAPITAL`.
    pub fn room_quarters(self) -> u32 {
        if matches!(crate::fuel::sized_part(self), Some((crate::fuel::EngineSize::Capital, _))) {
            return ROOM_CAPITAL;
        }
        if self.is_munition() || self.is_compact() {
            return ROOM_PARTS;
        }
        if self.is_fuel() {
            return ROOM_FUEL;
        }
        if self.is_raw() {
            return ROOM_RAW;
        }
        ROOM_BULK
    }

    /// Hold room one unit fills, in room units (1, 0.5 or 0.25; a capital part 10).
    pub fn room_per_unit(self) -> f64 {
        self.room_quarters() as f64 / ROOM_SCALE as f64
    }

    /// Prose only: stored orders, stockpiles and network identifiers use slug().
    pub fn display_name(self) -> String {
        match self {
            Self::MetallicOre => "ferrite ore".into(),
            Self::RareMetalOre => "rare-metal ore".into(),
            Self::Helium3Ice => "helium-3 ice".into(),
            Self::Helium3 => "helium-3".into(),
            // §charter-v4: shown as Semiconductors; the slug stays.
            Self::Electronics => "semiconductors".into(),
            // §shipbuilding-overhaul: the hardware good shows as Weapon Mounts;
            // the slug stays. The Armaments Complex keeps its name as the
            // category it covers (mounts, guns, and the munitions).
            Self::Armaments => "weapon mounts".into(),
            // §modules-as-goods: the Survey Drive shows as the Survey Booster
            // ("drive" means the warp drive); the slug stays.
            Self::SurveyDrive => "survey booster".into(),
            Self::PointDefenseScreen => "point-defense screen".into(),
            _ => self.slug().replace('_', " "),
        }
    }

    /// Includes legacy direct-mineral deposits; never treats refined metals as ore.
    pub fn is_mineable_mineral(self) -> bool {
        self.is_ore() || matches!(self, Self::Silicates | Self::RareElements)
    }
}

/// A convoy's manifest: what it is hauling and how much.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cargo {
    pub commodity: Commodity,
    pub units: u32,
}

#[cfg(test)]
mod tests {
    #[test]
    fn cargo_mass_classes_order_raw_bulk_parts_supplies() {
        use super::*;
        assert!(MASS_RAW > MASS_BULK && MASS_BULK > MASS_PARTS && MASS_PARTS > MASS_SUPPLY);
        assert_eq!(Commodity::MetallicOre.mass_per_unit(), MASS_RAW);
        assert_eq!(Commodity::Alloys.mass_per_unit(), MASS_BULK);
        assert_eq!(Commodity::Electronics.mass_per_unit(), MASS_PARTS);
        assert_eq!(Commodity::FissionReactor.mass_per_unit(), MASS_PARTS);
        assert_eq!(Commodity::ZephyrDrive.mass_per_unit(), MASS_PARTS);
        assert_eq!(Commodity::Uranium.mass_per_unit(), MASS_SUPPLY);
        assert_eq!(Commodity::Torpedoes.mass_per_unit(), MASS_SUPPLY);
        // Every ore is in the heaviest class: smelt at home, ship the metal.
        for c in Commodity::ALL.iter().filter(|c| c.is_ore()) {
            assert!(c.mass_per_unit() > Commodity::Alloys.mass_per_unit(), "{c:?}");
        }
        // The classes as the client mirrors them (fleet.ts commodityMass).
        let count = |m: f64| Commodity::ALL.iter().filter(|c| c.mass_per_unit() == m).count();
        assert_eq!((count(MASS_RAW), count(MASS_BULK), count(MASS_PARTS), count(MASS_SUPPLY)), (14, 12, 31, 7));
    }

    /// §hold-room (Nov 10 2026): room follows the weight classes, fuel packs
    /// as bulk, munitions as parts, a heavy part takes its light part's and a
    /// capital part ten room units.
    #[test]
    fn hold_room_by_class() {
        use super::*;
        for (c, q) in [(Commodity::MetallicOre, ROOM_RAW), (Commodity::Helium3Ice, ROOM_RAW),
            (Commodity::Alloys, ROOM_BULK), (Commodity::Provisions, ROOM_BULK), (Commodity::HullSections, ROOM_BULK),
            (Commodity::Deuterium, ROOM_FUEL), (Commodity::Helium3, ROOM_FUEL),
            (Commodity::Electronics, ROOM_PARTS), (Commodity::FissionReactor, ROOM_PARTS),
            (Commodity::ZephyrDrive, ROOM_PARTS), (Commodity::MassDriver, ROOM_PARTS), (Commodity::Torpedoes, ROOM_PARTS)] {
            assert_eq!(c.room_quarters(), q, "{c:?}");
        }
        assert_eq!((ROOM_RAW, ROOM_BULK, ROOM_FUEL, ROOM_PARTS, ROOM_CAPITAL),
            (ROOM_SCALE, ROOM_SCALE / 2, ROOM_SCALE / 2, ROOM_SCALE / 4, 10 * ROOM_SCALE));
        for c in Commodity::ALL {
            match crate::fuel::sized_part(c) {
                Some((crate::fuel::EngineSize::Capital, _)) => assert_eq!(c.room_quarters(), ROOM_CAPITAL, "{c:?}: ten room units"),
                Some((_, base)) => assert_eq!(c.room_quarters(), base.room_quarters(), "{c:?} packs as its light part"),
                None => {}
            }
        }
        // The classes as the client mirrors them (fleet.ts commodityRoom).
        let count = |q: u32| Commodity::ALL.iter().filter(|c| c.room_quarters() == q).count();
        assert_eq!((count(ROOM_RAW), count(ROOM_BULK), count(ROOM_PARTS), count(ROOM_CAPITAL)), (14, 17, 42, 9));
        // A Dreadnought's capital parts fill a Large Freighter's 1,000.
        let dreadnought: u64 = crate::build::DREADNOUGHT_RECIPE.costs.iter()
            .filter(|(c, _)| matches!(crate::fuel::sized_part(*c), Some((crate::fuel::EngineSize::Capital, _))))
            .map(|(c, n)| *n as u64 * c.room_quarters() as u64).sum();
        assert_eq!(dreadnought, 970 * ROOM_SCALE as u64);
    }

    use super::*;

    /// The rename keeps OLD snapshots parsing: "ore" (the legacy tag) still
    /// deserialises, and the new tag round-trips.
    #[test]
    fn ore_alias_keeps_old_snapshots_loading() {
        let old: Commodity = serde_json::from_str("\"ore\"").unwrap();
        assert_eq!(old, Commodity::MetallicOre);
        let new = serde_json::to_string(&Commodity::MetallicOre).unwrap();
        assert_eq!(new, "\"metallic_ore\"");
        let back: Commodity = serde_json::from_str(&new).unwrap();
        assert_eq!(back, Commodity::MetallicOre);
        // Every commodity round-trips through its slug form.
        for c in Commodity::ALL {
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(json, format!("\"{}\"", c.slug()));
            assert_eq!(serde_json::from_str::<Commodity>(&json).unwrap(), c);
        }
    }

    /// The tier classification partitions all 45 exactly (13 raw / 14 processed /
    /// 18 advanced), and RAW matches the Raw tier.
    #[test]
    fn tiers_partition_the_catalog() {
        let raw = Commodity::ALL
            .iter()
            .filter(|c| c.tier() == CommodityTier::Raw)
            .count();
        let processed = Commodity::ALL
            .iter()
            .filter(|c| c.tier() == CommodityTier::Processed)
            .count();
        let advanced = Commodity::ALL
            .iter()
            .filter(|c| c.tier() == CommodityTier::Advanced)
            .count();
        assert_eq!((raw, processed, advanced), (13, 14, 55));
        // §modules-as-goods: one good per module kind, on the module's slug.
        for m in crate::module::MODULE_KINDS {
            assert_eq!(m.good().slug(), m.slug());
            assert_eq!(m.good().module(), Some(m));
            assert!(m.good().is_module() && !m.good().is_part());
        }
        for c in crate::munitions::MUNITIONS { assert!(c.is_munition() && c.is_supply() && !c.is_fuel()); }
        for c in Commodity::REACTORS { assert!(c.is_reactor() && c.is_engine_part() && c.is_part() && !c.is_drive()); }
        for c in Commodity::DRIVES { assert!(c.is_drive() && c.is_engine_part() && c.is_part() && !c.is_reactor()); }
        assert!(Commodity::Armaments.display_name() == "weapon mounts" && Commodity::Armaments.slug() == "armaments");
        for c in Commodity::RAW {
            assert_eq!(c.tier(), CommodityTier::Raw);
        }
    }
}

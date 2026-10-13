//! Cargo carried by convoys. A convoy *broadcasts* its identity and position
//! (the Galactic Convention, §6) but NOT its cargo — cargo contents are only
//! revealed to a player whose sensors are within range of the convoy (the
//! two-tier information model).
//!
//! The industrial web separates five mineable ore families from their refined
//! materials. Raw exports need no refinery; refining draws power and staffed
//! capacity. Legacy Silicates/Rare Elements deposits remain mineable in saves.
//!
//! §fuel-overhaul: ships burn three fuels. Uranium (milled from Ferrite Ore)
//! drives slow warp, Deuterium (today's Fuel, renamed) full warp, Helium-3 the
//! long-range fusion burn, and Antimatter folds space for jumps.

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
    /// §fuel-overhaul: the Authority's Warp Core (formerly Drive Assemblies).
    /// Every warp hull needs one; colonies make them only after Fusion Reactors.
    #[serde(alias = "drive_assemblies")]
    WarpCore,
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
    /// The heart of a Fusion Plant (tiers III–VI). Imported until Fusion Reactors.
    FusionCore,
}

impl Commodity {
    pub const ALL: [Commodity; 34] = [
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
        Commodity::WarpCore,
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
        Commodity::FusionCore,
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
            | Commodity::WarpCore | Commodity::FusionCore => CommodityTier::Advanced,
        }
    }

    /// The snake_case wire slug (matches `rename_all = "snake_case"`).
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
            Commodity::WarpCore => "warp_core",
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
            Commodity::FusionCore => "fusion_core",
        }
    }

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

    /// Prose only: stored orders, stockpiles and network identifiers use slug().
    pub fn display_name(self) -> String {
        match self {
            Self::MetallicOre => "ferrite ore".into(),
            Self::RareMetalOre => "rare-metal ore".into(),
            Self::Helium3Ice => "helium-3 ice".into(),
            Self::Helium3 => "helium-3".into(),
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

    /// The tier classification partitions all 34 exactly (13 raw / 14 processed /
    /// 7 advanced), and RAW matches the Raw tier.
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
        assert_eq!((raw, processed, advanced), (13, 14, 7));
        for c in Commodity::RAW {
            assert_eq!(c.tier(), CommodityTier::Raw);
        }
    }
}

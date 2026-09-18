//! Cargo carried by convoys. A convoy *broadcasts* its identity and position
//! (the Galactic Convention, §6) but NOT its cargo — cargo contents are only
//! revealed to a player whose sensors are within range of the convoy (the
//! two-tier information model).
//!
//! The industrial web separates five mineable ore families from their refined
//! materials. Raw exports need no refinery; refining consumes fuel and staffed
//! capacity. Legacy Silicates/Rare Elements deposits remain mineable in saves.

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
    /// MetallicOre + Fuel, in a Smelter.
    Alloys,
    Electronics,
    Polymers,
    /// Volatiles-derived only, in a Fuel Refinery.
    Fuel,
    /// Biomass-derived only, in an Agroplex.
    Provisions,
    // ── Advanced ─────────────────────────────────────────────────────────────
    Machinery,
    Armaments,
    Composites,
    HullSections,
    PrecisionComponents,
    DriveAssemblies,
    // Append only: preserve historical enum ordering as well as wire slugs.
    CupriteOre,
    TitaniumOre,
    CrystallineOre,
    RareMetalOre,
    ConductiveMetals,
    Titanium,
}

impl Commodity {
    pub const ALL: [Commodity; 22] = [
        Commodity::MetallicOre,
        Commodity::RareElements,
        Commodity::Silicates,
        Commodity::Volatiles,
        Commodity::Biomass,
        Commodity::Alloys,
        Commodity::Electronics,
        Commodity::Polymers,
        Commodity::Fuel,
        Commodity::Provisions,
        Commodity::Machinery,
        Commodity::Armaments,
        Commodity::Composites,
        Commodity::HullSections,
        Commodity::PrecisionComponents,
        Commodity::DriveAssemblies,
        Commodity::CupriteOre,
        Commodity::TitaniumOre,
        Commodity::CrystallineOre,
        Commodity::RareMetalOre,
        Commodity::ConductiveMetals,
        Commodity::Titanium,
    ];

    /// New deposits contain ores, not already-separated refined materials.
    pub const RAW: [Commodity; 7] = [
        Commodity::MetallicOre,
        Commodity::CupriteOre,
        Commodity::TitaniumOre,
        Commodity::CrystallineOre,
        Commodity::RareMetalOre,
        Commodity::Volatiles,
        Commodity::Biomass,
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
            | Commodity::Biomass => CommodityTier::Raw,
            Commodity::Alloys
            | Commodity::RareElements
            | Commodity::Silicates
            | Commodity::ConductiveMetals
            | Commodity::Titanium
            | Commodity::Electronics
            | Commodity::Polymers
            | Commodity::Fuel
            | Commodity::Provisions => CommodityTier::Processed,
            Commodity::Machinery | Commodity::Armaments | Commodity::Composites
            | Commodity::HullSections | Commodity::PrecisionComponents
            | Commodity::DriveAssemblies => CommodityTier::Advanced,
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
            Commodity::Fuel => "fuel",
            Commodity::Provisions => "provisions",
            Commodity::Machinery => "machinery",
            Commodity::Armaments => "armaments",
            Commodity::Composites => "composites",
            Commodity::HullSections => "hull_sections",
            Commodity::PrecisionComponents => "precision_components",
            Commodity::DriveAssemblies => "drive_assemblies",
            Commodity::CupriteOre => "cuprite_ore",
            Commodity::TitaniumOre => "titanium_ore",
            Commodity::CrystallineOre => "crystalline_ore",
            Commodity::RareMetalOre => "rare_metal_ore",
            Commodity::ConductiveMetals => "conductive_metals",
            Commodity::Titanium => "titanium",
        }
    }

    pub fn is_ore(self) -> bool {
        matches!(self, Self::MetallicOre | Self::CupriteOre | Self::TitaniumOre
            | Self::CrystallineOre | Self::RareMetalOre)
    }

    /// Prose only: stored orders, stockpiles and network identifiers use slug().
    pub fn display_name(self) -> String {
        match self {
            Self::MetallicOre => "ferrite ore".into(),
            Self::RareMetalOre => "rare-metal ore".into(),
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

    /// The tier classification partitions all 22 exactly (7 raw / 9 processed /
    /// 6 advanced), and RAW matches the Raw tier.
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
        assert_eq!((raw, processed, advanced), (7, 9, 6));
        for c in Commodity::RAW {
            assert_eq!(c.tier(), CommodityTier::Raw);
        }
    }
}

//! Cargo carried by convoys. A convoy *broadcasts* its identity and position
//! (the Galactic Convention, §6) but NOT its cargo — cargo contents are only
//! revealed to a player whose sensors are within range of the convoy (the
//! two-tier information model).
//!
//! §economy: the INDUSTRIAL WEB — 12 commodities in three tiers. Five RAWS occur
//! naturally as deposits (the frontier value gradient draws only from these);
//! five PROCESSED goods are made from raws in processing structures; two
//! ADVANCED goods cap the chains. Every processed/advanced good's base price
//! clears its input basket (test-enforced), so industry beats raw-selling
//! without making raw-selling worthless.

use serde::{Deserialize, Serialize};

/// Which rung of the industrial web a commodity sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommodityTier {
    /// Occurs naturally as deposits; extracted.
    Raw,
    /// Made from raws in processing structures.
    Processed,
    /// Caps the chains (Machinery, Armaments).
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Commodity {
    // ── Raw (occur naturally as deposits) ────────────────────────────────────
    /// Rename of the old `Ore` — the alias keeps old snapshots + wire compat.
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
}

impl Commodity {
    pub const ALL: [Commodity; 12] = [
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
    ];

    /// The five RAWS — deposit generation draws ONLY from these.
    pub const RAW: [Commodity; 5] = [
        Commodity::MetallicOre,
        Commodity::RareElements,
        Commodity::Silicates,
        Commodity::Volatiles,
        Commodity::Biomass,
    ];

    /// Which rung of the industrial web this commodity sits on.
    pub fn tier(self) -> CommodityTier {
        match self {
            Commodity::MetallicOre
            | Commodity::RareElements
            | Commodity::Silicates
            | Commodity::Volatiles
            | Commodity::Biomass => CommodityTier::Raw,
            Commodity::Alloys
            | Commodity::Electronics
            | Commodity::Polymers
            | Commodity::Fuel
            | Commodity::Provisions => CommodityTier::Processed,
            Commodity::Machinery | Commodity::Armaments => CommodityTier::Advanced,
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
        }
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

    /// The tier classification partitions all 12 exactly (5 raw / 5 processed /
    /// 2 advanced), and RAW matches the Raw tier.
    #[test]
    fn tiers_partition_the_twelve() {
        let raw = Commodity::ALL.iter().filter(|c| c.tier() == CommodityTier::Raw).count();
        let processed = Commodity::ALL.iter().filter(|c| c.tier() == CommodityTier::Processed).count();
        let advanced = Commodity::ALL.iter().filter(|c| c.tier() == CommodityTier::Advanced).count();
        assert_eq!((raw, processed, advanced), (5, 5, 2));
        for c in Commodity::RAW {
            assert_eq!(c.tier(), CommodityTier::Raw);
        }
    }
}

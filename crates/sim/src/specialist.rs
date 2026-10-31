//! §economy Part 4 — SPECIALISTS: rare people, not goods. A specialist is a
//! large optional multiplier (`skill` in the factor chain) that must be
//! PHYSICALLY transported — hired at Sol or trained at an Academy, carried by
//! convoy under the two-tier fog rule (identity broadcast, manifest
//! sensor-gated), lost with the ship, kept with a captured colony. Population
//! is the workforce; specialists are the edge.

use serde::{Deserialize, Serialize};

use crate::build::StructureKind;
use crate::ship::ShipKind;

/// The five specialist professions, and (§research-wiring Expert Training)
/// each one's expert tier: trained at an Academy, never hired at Sol.
/// Ordering is stable for BTreeMap determinism; append only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialistKind {
    /// Mineral extraction: the Mining Complex's three deposit families.
    Geologist,
    /// Volatiles end-to-end: harvesting, refining, chemical processing.
    PetrochemicalEngineer,
    /// The food chain: Biomass harvesting and the Agroplex.
    Xenobiologist,
    /// Heavy industry: Smelter, Electronics, Machinery, Armaments.
    IndustrialEngineer,
    /// The yards: Shipyard boost, and Armaments (weapons are ships' work).
    NavalArchitect,
    ExpertGeologist,
    ExpertPetrochemicalEngineer,
    ExpertXenobiologist,
    ExpertIndustrialEngineer,
    ExpertNavalArchitect,
}

impl SpecialistKind {
    pub const ALL: [SpecialistKind; 5] = [
        SpecialistKind::Geologist,
        SpecialistKind::PetrochemicalEngineer,
        SpecialistKind::Xenobiologist,
        SpecialistKind::IndustrialEngineer,
        SpecialistKind::NavalArchitect,
    ];

    /// The expert tiers, in profession order.
    pub const EXPERTS: [SpecialistKind; 5] = [
        SpecialistKind::ExpertGeologist,
        SpecialistKind::ExpertPetrochemicalEngineer,
        SpecialistKind::ExpertXenobiologist,
        SpecialistKind::ExpertIndustrialEngineer,
        SpecialistKind::ExpertNavalArchitect,
    ];

    pub fn is_expert(self) -> bool {
        Self::EXPERTS.contains(&self)
    }

    /// The profession, without its tier.
    pub fn base(self) -> Self {
        match self {
            SpecialistKind::ExpertGeologist => SpecialistKind::Geologist,
            SpecialistKind::ExpertPetrochemicalEngineer => SpecialistKind::PetrochemicalEngineer,
            SpecialistKind::ExpertXenobiologist => SpecialistKind::Xenobiologist,
            SpecialistKind::ExpertIndustrialEngineer => SpecialistKind::IndustrialEngineer,
            SpecialistKind::ExpertNavalArchitect => SpecialistKind::NavalArchitect,
            base => base,
        }
    }

    /// The profession's expert tier.
    pub fn expert(self) -> Self {
        Self::EXPERTS[Self::ALL.iter().position(|k| *k == self.base()).expect("a profession")]
    }

    pub fn slug(self) -> &'static str {
        match self {
            SpecialistKind::Geologist => "geologist",
            SpecialistKind::PetrochemicalEngineer => "petrochemical_engineer",
            SpecialistKind::Xenobiologist => "xenobiologist",
            SpecialistKind::IndustrialEngineer => "industrial_engineer",
            SpecialistKind::NavalArchitect => "naval_architect",
            SpecialistKind::ExpertGeologist => "expert_geologist",
            SpecialistKind::ExpertPetrochemicalEngineer => "expert_petrochemical_engineer",
            SpecialistKind::ExpertXenobiologist => "expert_xenobiologist",
            SpecialistKind::ExpertIndustrialEngineer => "expert_industrial_engineer",
            SpecialistKind::ExpertNavalArchitect => "expert_naval_architect",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            SpecialistKind::Geologist => "Geologist",
            SpecialistKind::PetrochemicalEngineer => "Petrochemical Engineer",
            SpecialistKind::Xenobiologist => "Xenobiologist",
            SpecialistKind::IndustrialEngineer => "Industrial Engineer",
            SpecialistKind::NavalArchitect => "Naval Architect",
            SpecialistKind::ExpertGeologist => "Expert Geologist",
            SpecialistKind::ExpertPetrochemicalEngineer => "Expert Petrochemical Engineer",
            SpecialistKind::ExpertXenobiologist => "Expert Xenobiologist",
            SpecialistKind::ExpertIndustrialEngineer => "Expert Industrial Engineer",
            SpecialistKind::ExpertNavalArchitect => "Expert Naval Architect",
        }
    }

    /// Skill points on an affine line: 3 for a specialist, 5 for an expert
    /// (a full line reaches ×1.75 or ×2.25).
    pub fn skill_points(self) -> u32 {
        if self.is_expert() { crate::production::EXPERT_SKILL_POINTS } else { crate::production::SPECIALIST_SKILL_POINTS }
    }

    /// The AFFINITY table: which structures this profession's skill bonus
    /// applies to. Off-affinity a specialist still counts as a generic crew
    /// member (never a penalty), just without the bonus.
    pub fn affine(self, structure: StructureKind) -> bool {
        use StructureKind as K;
        matches!(
            (self.base(), structure),
            (SpecialistKind::Geologist, K::MiningComplex | K::UraniumMill)
                | (
                    SpecialistKind::PetrochemicalEngineer,
                    K::VolatileHarvester | K::FuelRefinery | K::ChemicalWorks
                )
                | (SpecialistKind::Xenobiologist, K::Bioharvester | K::Agroplex | K::LivingTerraces)
                | (
                    SpecialistKind::IndustrialEngineer,
                    K::Smelter | K::ElectronicsFabricator | K::MachineWorks | K::ArmamentsComplex
                    | K::CompositeWorks | K::HullFabricator | K::PrecisionWorks | K::DriveWorks
                    | K::AntimatterPlant
                )
                | (
                    SpecialistKind::NavalArchitect,
                    K::Shipyard | K::ArmamentsComplex
                    | K::HullFabricator | K::DriveWorks
                )
        )
    }
}

// --- SOURCES ---------------------------------------------------------------------

/// Sol's standing contract price per specialist, any profession (credits).
/// Price-certain, delivery-risky: the personnel convoy from the hub is a
/// normal sub-light, raidable convoy. Tunable.
pub const SPECIALIST_HIRE_COST: f64 = 800.0;

/// Academy training time (build-queue ticks). Deliberately slower than most
/// construction — homegrown talent is the patient road; Sol is the fast one.
pub const ACADEMY_TRAIN_TICKS: u64 = 40 * crate::config::TICK_HZ as u64;

/// What one Academy training course consumes from the local stockpile.
/// Tunable.
pub const ACADEMY_TRAIN_COSTS: &[(crate::cargo::Commodity, f64)] = &[
    (crate::cargo::Commodity::Provisions, 20.0),
    (crate::cargo::Commodity::Electronics, 10.0),
];

/// §research-wiring Expert Training: an expert course, on top of the
/// specialist it raises. Tunable.
pub const EXPERT_TRAIN_COSTS: &[(crate::cargo::Commodity, f64)] = &[
    (crate::cargo::Commodity::Provisions, 30.0),
    (crate::cargo::Commodity::Electronics, 20.0),
    (crate::cargo::Commodity::PrecisionComponents, 5.0),
];

// --- TRANSPORT --------------------------------------------------------------------

/// Passenger berths per SHIP of each kind — only the logistics hulls carry
/// people (warships and scouts have no berths). A fleet's capacity is the sum
/// over its composition. Tunable.
pub fn passenger_capacity(kind: ShipKind) -> u32 {
    match kind {
        ShipKind::Builder => 0, // work crew only — berths are shops
        ShipKind::Convoy => 4,
        ShipKind::TinyFreighter => 1,
        ShipKind::SmallFreighter => 2,
        ShipKind::LargeFreighter => 6,
        ShipKind::HeavyFreighter => 8,
        ShipKind::BulkFreighter => 12,
        ShipKind::Colony => 8,
        // Warships carry crews, not passengers — capitals included (§ladder).
        // §TCA: Authority freighters haul GOODS, not people — personnel ride a
        // corp's own logistics hulls, never the common carrier.
        ShipKind::Transport // marines are not passengers — they ride their own berths
        | ShipKind::Freighter
        | ShipKind::Scout
        | ShipKind::SurveyProbe
        | ShipKind::MiningBarge // its crew works the cutters, not passenger berths
        | ShipKind::MiningBeltStation // a depot crew, not passenger berths
        | ShipKind::CombatFreighter // a prize crew, and a hold for loot
        | ShipKind::Raider
        | ShipKind::Corvette
        | ShipKind::Destroyer
        | ShipKind::Cruiser
        | ShipKind::Battleship
        | ShipKind::Dreadnought
        | ShipKind::Titan => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_producing_structure_has_at_least_one_affine_profession() {
        // Every converter + extraction structure + the Shipyard boost can be
        // specialist-boosted — no dead line in the affinity table.
        let mut producing: Vec<StructureKind> = crate::production::CONVERTERS
            .iter()
            .map(|c| c.structure)
            .collect();
        producing.extend([
            StructureKind::MiningComplex,
            StructureKind::VolatileHarvester,
            StructureKind::Bioharvester,
            StructureKind::Shipyard,
        ]);
        for s in producing {
            assert!(
                SpecialistKind::ALL.iter().any(|k| k.affine(s)),
                "{s:?} has no affine specialist"
            );
        }
    }

    #[test]
    fn slugs_match_serde() {
        for k in SpecialistKind::ALL {
            let json = serde_json::to_string(&k).unwrap();
            assert_eq!(json, format!("\"{}\"", k.slug()));
        }
    }

    #[test]
    fn only_logistics_hulls_carry_passengers() {
        assert!(passenger_capacity(ShipKind::Convoy) > 0);
        assert!(passenger_capacity(ShipKind::Colony) > 0);
        assert_eq!(passenger_capacity(ShipKind::Raider), 0);
        assert_eq!(passenger_capacity(ShipKind::Corvette), 0);
        assert_eq!(passenger_capacity(ShipKind::Scout), 0);
    }
}

//! Opt-in founding camps. Old colonies have no camp and retain their capacities.
use crate::{Commodity, build::Recipe};
use serde::{Deserialize, Serialize};
const HZ: u64 = crate::config::TICK_HZ as u64;

/// Seconds of colony demand a camp keeps in Provisions + Biomass before its
/// Bioharvesters pause. The 250-unit camp warehouse used to fill with food in
/// about two minutes, which starved the mine and blocked the recovered cache.
/// §progression (Nov 11 2026): people eat at `production::OUTPUT_SCALE`, so
/// the reserve counts ten minutes at the tuned rate and keeps its units: at
/// 600 s of today's demand it fell to the 40-unit floor, under the landing
/// crate's 100 Provisions, and held the first Bioharvester from landing.
pub const CAMP_FOOD_RESERVE_S: f64 = 600.0 / crate::production::OUTPUT_SCALE;
/// Floor for the reserve, so a tiny population still keeps a real buffer.
pub const CAMP_FOOD_RESERVE_MIN: f64 = 40.0;
/// A held food line resumes once the reserve falls below this fraction.
pub const CAMP_FOOD_RESUME_FRAC: f64 = 0.75;

/// The food a camp keeps on hand for `demand_per_s` Provisions of upkeep.
pub fn camp_food_reserve(demand_per_s: f64) -> f64 {
    (demand_per_s * CAMP_FOOD_RESERVE_S).max(CAMP_FOOD_RESERVE_MIN)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingProfile {
    MineralHighlands,
    VerdantLowlands,
    CoastalPlains,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// §habitat-rebase (Peter, Nov 1 2026): the camp's Permanent Quarters project
/// is gone. Its Habitat I is a plain Habitat I, housing 8,000 a level.
pub enum SettlementProject {
    Prospect,
    SurfaceWorkings,
    Storehouse,
}

impl SettlementProject {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Prospect => "camp_prospect",
            Self::SurfaceWorkings => "camp_workings",
            Self::Storehouse => "camp_storehouse",
        }
    }
    pub fn recipe(self) -> &'static Recipe {
        use Commodity::*;
        match self {
            Self::Prospect => &Recipe {
                costs: &[],
                build_ticks: 60 * HZ,
            },
            Self::SurfaceWorkings => &Recipe {
                costs: &[(Alloys, 8.0), (Machinery, 2.0)],
                build_ticks: 120 * HZ,
            },
            Self::Storehouse => &Recipe {
                costs: &[(Alloys, 7.0), (Machinery, 1.0)],
                build_ticks: 90 * HZ,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoundingCamp {
    pub profile: FoundingProfile,
    pub completed: std::collections::BTreeSet<SettlementProject>,
}

impl FoundingCamp {
    pub fn new(profile: FoundingProfile) -> Self {
        Self {
            profile,
            completed: Default::default(),
        }
    }
    pub fn done(&self, project: SettlementProject) -> bool {
        self.completed.contains(&project)
    }
}

//! Opt-in founding camps. Old colonies have no camp and retain their capacities.
use crate::{Commodity, build::Recipe};
use serde::{Deserialize, Serialize};
const HZ: u64 = crate::config::TICK_HZ as u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingProfile {
    MineralHighlands,
    VerdantLowlands,
    CoastalPlains,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementProject {
    Prospect,
    SurfaceWorkings,
    Storehouse,
    Quarters,
}

impl SettlementProject {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Prospect => "camp_prospect",
            Self::SurfaceWorkings => "camp_workings",
            Self::Storehouse => "camp_storehouse",
            Self::Quarters => "camp_quarters",
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
            Self::Quarters => &Recipe {
                costs: &[(Alloys, 5.0), (Machinery, 1.0)],
                build_ticks: 120 * HZ,
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

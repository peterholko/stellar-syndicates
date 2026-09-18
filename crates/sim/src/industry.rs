//! Physical logistics and specialized industry. Policies travel to their local
//! executor; all mutable progress is served through the ordinary site/fleet light.
use crate::{BuildKind, Commodity, EntityId, ShipKind, StructureKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_STOPS: usize = 8;
pub const MAX_RESERVATIONS: usize = 8;
pub const OUTPOST_BUILD_S: f64 = 60.0;
pub const SKIM_RANGE_SU: f64 = 1_500.0;
pub const SKIM_FUEL_PER_S: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FreightPort {
    System { id: EntityId },
    Hub,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteStop {
    pub port: FreightPort,
    /// Whole units to transfer on EACH visit, not once per simulation tick.
    pub load: BTreeMap<Commodity, u32>,
    pub unload: BTreeMap<Commodity, u32>,
    #[serde(default)]
    pub sell: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreightRoute {
    pub name: String,
    pub stops: Vec<RouteStop>,
    pub fuel_reserve: f64,
    pub escort: Option<EntityId>,
    pub repeat: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutePhase {
    Dispatch,
    Travelling,
    Unloading,
    Loading,
    Fuel,
    Escort,
    Blocked,
    Complete,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreightRun {
    pub route: FreightRoute,
    pub stop: usize,
    pub phase: RoutePhase,
    pub remaining_load: BTreeMap<Commodity, u32>,
    pub remaining_unload: BTreeMap<Commodity, u32>,
    pub visits: u32,
    #[serde(default)]
    pub hold: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutpostKind {
    Extraction,
    Research,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outpost {
    pub kind: OutpostKind,
    pub body: u32,
    pub commodity: Commodity,
    pub supplied: bool,
    pub rate: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColonyProjectKind {
    OrbitalAssembly,
    AgriculturalExport,
    DeepExtraction,
}
impl ColonyProjectKind {
    pub fn costs(self) -> &'static [(Commodity, f64)] {
        use Commodity::*;
        match self {
            Self::OrbitalAssembly => &[
                (Alloys, 150.0),
                (HullSections, 40.0),
                (PrecisionComponents, 30.0),
                (Machinery, 40.0),
            ],
            Self::AgriculturalExport => &[(Alloys, 70.0), (Polymers, 50.0), (Machinery, 40.0)],
            Self::DeepExtraction => &[(Alloys, 100.0), (DriveAssemblies, 15.0), (Machinery, 60.0)],
        }
    }
    pub fn build_secs(self) -> f64 {
        180.0
    }
    pub fn workers(self) -> u32 {
        3
    }
    pub fn outputs(self) -> &'static [(Commodity, f64)] {
        match self {
            Self::OrbitalAssembly => &[
                (Commodity::HullSections, 0.3),
                (Commodity::DriveAssemblies, 0.12),
            ],
            Self::AgriculturalExport => &[(Commodity::Provisions, 3.0)],
            Self::DeepExtraction => &[], // determined by the actual finite deposit
        }
    }
    pub fn inputs(self) -> &'static [(Commodity, f64)] {
        use Commodity::*;
        // Per operational second. These are new specialized processes, not a
        // blanket multiplier on every colony line. Transport still matters.
        match self {
            Self::OrbitalAssembly => &[(Composites, 0.6), (PrecisionComponents, 0.25), (Fuel, 0.3)],
            Self::AgriculturalExport => &[(Biomass, 2.5), (Polymers, 0.1), (Fuel, 0.05)],
            Self::DeepExtraction => &[(Fuel, 0.15), (Machinery, 0.025)],
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColonyProject {
    pub kind: ColonyProjectKind,
    pub body: u32,
    pub commodity: Commodity,
    pub work: f64,
    pub active: bool,
    pub supplied: bool,
    pub outputs: Vec<(Commodity, f64)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectTarget {
    Cruiser,
    Academy,
    Colony,
    Development { project: ColonyProjectKind },
}
impl ProjectTarget {
    pub fn build(self) -> Option<BuildKind> {
        match self {
            Self::Cruiser => Some(BuildKind::Ship {
                ship: ShipKind::Cruiser,
            }),
            Self::Academy => Some(BuildKind::Upgrade {
                upgrade: StructureKind::Academy,
            }),
            Self::Colony => Some(BuildKind::Ship {
                ship: ShipKind::Colony,
            }),
            _ => None,
        }
    }
    pub fn costs(self) -> &'static [(Commodity, f64)] {
        if let Self::Development { project } = self {
            project.costs()
        } else {
            crate::build::recipe_for(self.build().unwrap()).costs
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reservation {
    pub target: ProjectTarget,
    pub goods: BTreeMap<Commodity, f64>,
}
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteIndustry {
    pub reservations: Vec<Reservation>,
    pub outpost: Option<Outpost>,
    pub projects: Vec<ColonyProject>,
}
impl SiteIndustry {
    pub fn reserved(&self, c: Commodity, except: Option<ProjectTarget>) -> f64 {
        self.reservations
            .iter()
            .filter(|r| Some(r.target) != except)
            .map(|r| r.goods.get(&c).copied().unwrap_or(0.0))
            .sum()
    }
    pub fn workforce(&self) -> u32 {
        self.projects
            .iter()
            .filter(|p| p.active)
            .map(|p| p.kind.workers())
            .sum()
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FleetIndustry {
    Route {
        run: FreightRun,
    },
    Deploy {
        system: EntityId,
        body: u32,
        outpost: OutpostKind,
        commodity: Commodity,
        work: f64,
    },
    Skim {
        system: EntityId,
        harvested: f64,
        cargo_fraction: f64,
        status: String,
    },
}
pub fn outpost_costs() -> &'static [(Commodity, u32)] {
    &[
        (Commodity::Alloys, 30),
        (Commodity::Machinery, 15),
        (Commodity::Electronics, 10),
    ]
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSpec {
    pub kind: ColonyProjectKind,
    pub costs: Vec<(Commodity, f64)>,
    pub inputs: Vec<(Commodity, f64)>,
    pub outputs: Vec<(Commodity, f64)>,
    pub workers: u32,
    pub build_secs: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct IndustryCatalog {
    pub projects: Vec<ProjectSpec>,
    pub outpost_costs: Vec<(Commodity, u32)>,
    pub outpost_build_secs: f64,
    pub skim_fuel_per_s: f64,
    pub max_stops: usize,
}
pub fn catalog() -> IndustryCatalog {
    IndustryCatalog {
        projects: [
            ColonyProjectKind::OrbitalAssembly,
            ColonyProjectKind::AgriculturalExport,
            ColonyProjectKind::DeepExtraction,
        ]
        .into_iter()
        .map(|kind| ProjectSpec {
            kind,
            costs: kind.costs().to_vec(),
            inputs: kind.inputs().to_vec(),
            outputs: kind.outputs().to_vec(),
            workers: kind.workers(),
            build_secs: kind.build_secs(),
        })
        .collect(),
        outpost_costs: outpost_costs().to_vec(),
        outpost_build_secs: OUTPOST_BUILD_S,
        skim_fuel_per_s: SKIM_FUEL_PER_S,
        max_stops: MAX_STOPS,
    }
}

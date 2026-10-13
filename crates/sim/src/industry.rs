//! Physical logistics and specialized industry. Policies travel to their local
//! executor; all mutable progress is served through the ordinary site/fleet light.
use crate::{BuildKind, Commodity, EntityId, ShipKind, StructureKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_STOPS: usize = 8;
/// §fuel-overhaul: the most a depot rule may keep at one port.
pub const DEPOT_MAX_TARGET: u32 = 2_000;
/// §fuel-overhaul: how often the Authority reviews depot rules (sim seconds).
pub const DEPOT_CHECK_SECS: f64 = 30.0;

/// §fuel-overhaul: a FUEL DEPOT standing rule — "keep `target` of `commodity`
/// at `system`". The Authority reviews it at the Hub on its own light-delayed
/// picture of the port, buys the shortfall on the Exchange and books it out on
/// its next freight departure, billing price plus the freight fee. Fuels only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepotRule {
    pub system: EntityId,
    pub commodity: Commodity,
    pub target: u32,
}

/// The goods a depot rule may stock.
pub fn is_depot_fuel(c: Commodity) -> bool {
    matches!(c, Commodity::Uranium | Commodity::Deuterium | Commodity::Helium3 | Commodity::Antimatter)
}
pub const MAX_RESERVATIONS: usize = 8;
pub const OUTPOST_BUILD_S: f64 = 60.0;
pub const SKIM_RANGE_SU: f64 = 1_500.0;
/// §fuel-overhaul: Ramscoop skims Helium-3 at gas giants — the energy of the
/// old 1 Fuel a second.
pub const SKIM_HELIUM3_PER_S: f64 = 1.0 / crate::fuel::HELIUM3_FUEL_EQUIV;
/// §belts: bulk-equivalent ore one Mining Barge cuts per second before research
/// (Deep Bores, Strip Rigs). A dense ore comes out in proportionally fewer
/// units (`production::ore_bulk_ratio`), as from a mine.
pub const BARGE_ORE_PER_S: f64 = 0.25;
/// §belts: every barge working one deposit, whatever corporation it flies for,
/// shares at most this multiple of the deposit's natural rate. Upkeep is cheap,
/// so this is what limits ship mining, and what makes miners compete.
pub const SHIP_YIELD_MULT: f64 = 3.0;
/// §belts phase 6: a corporation's cumulative mining, for the balancing
/// harness and the admin views. Units are whole-or-fractional ore units as
/// they left the ground (dense ores in their own, fewer units). Sim truth:
/// never on a player's wire.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MiningStats {
    /// Cut by Mining Barges, by ore.
    #[serde(default)]
    pub barge: BTreeMap<Commodity, f64>,
    /// Extracted by colony structures on asteroid belts, by ore.
    #[serde(default)]
    pub belt_colony: BTreeMap<Commodity, f64>,
    /// Extracted by colony structures on planets and moons, by ore.
    #[serde(default)]
    pub planet_colony: BTreeMap<Commodity, f64>,
    /// The part of all of the above that came out of found pockets.
    #[serde(default)]
    pub pocket: BTreeMap<Commodity, f64>,
    #[serde(default)]
    pub prospect_passes: u32,
    #[serde(default)]
    pub pockets_found: u32,
    #[serde(default)]
    pub strikes: u32,
    /// Ore freighters took out of stations, by hand or on a route.
    #[serde(default)]
    pub station_hauled: f64,
}

/// §belts: a Mining Belt Station takes this long to anchor, and to unanchor.
pub const ANCHOR_SECS: f64 = 60.0;

/// §belts: where a Mining Belt Station stands in its anchoring. A station
/// flies to the belt's star (`Approaching`), anchors within `SKIM_RANGE_SU` of
/// it (`Anchoring`, then `Anchored`), and takes as long to come free
/// (`Unanchoring`). From `Anchoring` on it cannot move, merge or split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorPhase {
    Approaching,
    Anchoring,
    Anchored,
    Unanchoring,
}

/// §belts: a station's anchor at one belt.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub system: EntityId,
    pub body: u32,
    pub phase: AnchorPhase,
    /// Sim time the current phase began.
    pub since: f64,
}

impl Anchor {
    /// Anchoring, anchored or unanchoring: the station is fixed in place.
    pub fn locked(&self) -> bool {
        self.phase != AnchorPhase::Approaching
    }
}

/// §pockets: a prospecting pass dwells this long within `SKIM_RANGE_SU` of the
/// star, loud like a survey.
pub const PROSPECT_SECS: f64 = 60.0;
/// §pockets: each pass finds each hidden pocket with this chance. Tunable.
pub const POCKET_FIND_CHANCE: f64 = 0.5;
/// §pockets: a barge at work strikes a hidden pocket now and then — mining is
/// slow prospecting. The chance per barge-hour of cutting. Tunable.
pub const STRIKE_CHANCE_PER_BARGE_HOUR: f64 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FreightPort {
    System { id: EntityId },
    Hub,
    /// §belts: one of the owner's Mining Belt Stations. A station stop only
    /// loads: it takes ore from the station's hold.
    Station { id: EntityId },
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
    /// Remember cruise with the route. Old saves/bookings retain Full; new UI
    /// and bot routes explicitly choose Slow, never by rewriting old saves.
    #[serde(default)]
    pub transit: crate::ship::TransitMode,
    /// Optional cash cap per Hub visit, including fees. Purchases fill that
    /// stop's load manifest from available cash after real sales; never credit.
    /// None preserves existing warehouse-only routes.
    #[serde(default)]
    pub shopping_budget: Option<f64>,
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
    #[serde(default)]
    pub shopping_spent: f64,
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
            Self::DeepExtraction => &[(Alloys, 100.0), (WarpCore, 15.0), (Machinery, 60.0)],
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
                (Commodity::WarpCore, 0.12),
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
            // §rare-materials: its Warp Cores carry the Drive Works' Iridium.
            // §fuel-overhaul: no project burns fuel; the Warp Core output needs
            // Fusion Reactors (checked where the project runs).
            Self::OrbitalAssembly => &[(Composites, 0.58), (PrecisionComponents, 0.25), (Iridium, 0.012)],
            Self::AgriculturalExport => &[(Biomass, 2.5), (Polymers, 0.1)],
            Self::DeepExtraction => &[(Machinery, 0.025)],
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
    /// Smelters alone leave these physical floors untouched. Freighters may
    /// export the ore and local ships can refuel; these are not project locks.
    #[serde(default)]
    pub refining_reserve: BTreeMap<Commodity, u32>,
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
    /// §belts: Mining Barges working one ore of a belt. `status` is
    /// travelling, mining, full, depleted, unsafe or claimed.
    Mine {
        system: EntityId,
        body: u32,
        ore: Commodity,
        /// §pockets: working a found pocket of `ore`, not the belt's own.
        #[serde(default)]
        pocket: bool,
        /// §belts: this tick's cut went into the corporation's anchored station.
        #[serde(default)]
        to_station: bool,
        /// Units cut over the job's life, into the stockpile or the hold.
        mined: f64,
        /// The part of a unit carried into the next tick.
        fraction: f64,
        status: String,
    },
    /// §pockets: a Scout or Mining Barge prospecting a charted belt for its
    /// hidden pockets. `status` is travelling, prospecting or unsafe; the job
    /// ends when the pass completes and its report heads home.
    Prospect {
        system: EntityId,
        body: u32,
        /// Seconds of dwell banked on this pass.
        dwell: f64,
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
    pub skim_helium3_per_s: f64,
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
        skim_helium3_per_s: SKIM_HELIUM3_PER_S,
        max_stops: MAX_STOPS,
    }
}

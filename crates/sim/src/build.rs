//! Construction: spending resources to GROW (the Travian-style growth sink, §step1).
//!
//! Recipes spend local goods when a job is queued. Ship hulls then earn work only
//! while their construction yard has assigned workforce; pausing retains that work.
//! Structures take turns in each planet's queue; courses keep independent timers. Construction is
//! server-driven, online or off; recipe costs and base work are deterministic.

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};
use crate::module::ModuleKind;
use crate::ship::ShipKind;

/// What a build job produces on completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BuildKind {
    /// A one-time camp development; shares the planet's ordinary FIFO queue.
    Founding { project: crate::settlement::SettlementProject },
    /// Construct a ship of `ship` kind; it spawns Idle at the building system.
    Ship { ship: ShipKind },
    /// Build/raise a STRUCTURE tier (§economy — the industrial web). The field
    /// keeps its legacy name `upgrade` on the wire; `StructureKind`'s serde
    /// aliases parse legacy slugs, so in-flight pre-economy build jobs complete
    /// as their mapped successor structure.
    Upgrade { upgrade: StructureKind },
    /// §economy Part 4: an Academy TRAINING COURSE — completes into one
    /// resident specialist of `kind` (if the system is still held). Rides the
    /// same build queue; holds no slot, needs no shipyard.
    Train {
        specialist: crate::specialist::SpecialistKind,
    },
    /// A home Academy commissions one corporation officer into reserve duty.
    /// `serial` freezes the deterministic identity at enqueue, so a lost Academy
    /// or snapshot round-trip cannot reshuffle later graduates.
    RecruitCaptain { serial: u32 },
    /// §modules Part B3: manufacture one MODULE — completes into the system's
    /// module ledger (if still held). Needs an Armaments Complex ≥ 1; holds no
    /// slot; rides the same build queue.
    Module { module: ModuleKind },
}

/// §economy: which SLOT POOL a structure consumes. Slot budgets are DERIVED,
/// never stored (same philosophy as the old `dev_slots()` — migration-free by
/// construction): Resource slots come from geology, Industrial and
/// Infrastructure slots from population (see `StarSystem::*_slots`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotPool {
    Resource,
    Industrial,
    Infrastructure,
}

/// §economy: the STRUCTURES of the industrial web — extraction works deposits,
/// processing turns raws into goods, advanced industry caps the chains, support
/// holds the colony together. Replaces the flat `SystemUpgrade`; serde aliases
/// keep every legacy slug parsing (Extractor→MiningComplex, Refinery→
/// FuelRefinery, the rest 1:1), so old snapshots, in-flight build jobs, and old
/// client commands all land on the mapped successor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureKind {
    // ── Extraction (Resource slots) ─────────────────────────────────────────
    /// Works MetallicOre / RareElements / Silicates deposits.
    #[serde(alias = "extractor")]
    MiningComplex,
    /// Works Volatiles deposits.
    VolatileHarvester,
    /// Works Biomass deposits.
    Bioharvester,
    // ── Processing (Industrial slots) ───────────────────────────────────────
    /// MetallicOre → Alloys (§fuel-overhaul: no fuel; double power draw).
    Smelter,
    /// RareElements + Silicates → Electronics.
    ElectronicsFabricator,
    /// Volatiles + Biomass → Polymers.
    ChemicalWorks,
    /// Volatiles → Deuterium. Titled "Deuterium Refinery" (§fuel-overhaul); the
    /// `fuel_refinery` save identity stays.
    #[serde(alias = "refinery")]
    FuelRefinery,
    /// Biomass → Provisions.
    Agroplex,
    // ── Advanced industry (Industrial slots) ────────────────────────────────
    /// Alloys + Electronics → Machinery.
    MachineWorks,
    /// Alloys + Electronics + Polymers → Armaments.
    ArmamentsComplex,
    CompositeWorks,
    HullFabricator,
    PrecisionWorks,
    DriveWorks,
    /// GATES the LIGHT hulls (`yard_for`): Tiny/Small Freighter, Scout/Colony at tier 1,
    /// Medium Freighter/Raider/Corvette at tier 2. Larger freight is built in orbit.
    /// Its tier is also its SLIPWAY COUNT — concurrent hulls (§yards M1).
    Shipyard,
    /// Orbital Shipyard — Large/Heavy/Bulk freight and Destroyer/Cruiser/Battleship.
    /// Keeps the `naval_drydock` save identity. Needs a
    /// Shipyard ≥ 2 on the same system ([`yard_prereq`]): you learn to build
    /// before you build big. Its own tier is its own slip count, independent of
    /// the Shipyard's, so a yard world can lay light and line hulls in parallel.
    NavalDrydock,
    /// §yards: the SUPER-CAPITAL yard — Dreadnought/Titan. Needs an Orbital Shipyard
    /// ≥ 3 here. Deliberately the deepest prerequisite chain in the game: a
    /// capital slipway is a season's investment, supported by a capturable colony.
    CapitalSlipway,
    /// §yards: OUTFITTING AND MAINTENANCE — the yard that fits hulls rather than
    /// laying them. Holds the REFIT work that used to happen at any Shipyard, so
    /// a forward system can be a refit base without being a construction base.
    /// (Module MANUFACTURE stays at the Armaments Complex: armaments make the
    /// crates, the foundry installs them.)
    OrdnanceFoundry,
    // ── Support (Infrastructure slots) ──────────────────────────────────────
    /// Population capacity + workforce slots (§economy Part 2 — the boost/upkeep
    /// semantics retire; capacity is the Habitat's value now).
    Habitat,
    /// §gaia: GAIA WORLDS ONLY. Garden terraces that house people and grow
    /// Provisions without Biomass — a no-input converter crewed one shift per
    /// tier, so a Gaia world feeds itself and exports the surplus.
    LivingTerraces,
    /// Late-game bulk storage, unlocked by Orbital Yards. The old `Depot`
    /// alias still loads existing buildings and paid jobs without re-gating them.
    #[serde(alias = "depot")]
    OrbitalWarehouse,
    /// Standing sensor bubble (unchanged semantics).
    SensorArray,
    /// Static defense (combat semantics + `defense_pool` untouched). LEGACY:
    /// no longer built (the orbital defense station replaced it); pirate
    /// bases and older colonies keep their tiers, which fight as before.
    DefensePlatform,
    /// §orbital-defense: the system's ONE orbital defense station. Its tier is
    /// its LEVEL, Defense Platform (1) to Star Fortress (5); see
    /// [`crate::station`].
    OrbitalDefense,
    /// Trains specialists locally (§economy Part 4) — endogenous supply so Sol
    /// never stays a permanent monopoly.
    Academy,
    /// §ground: THE GARRISON — dug-in ground defense, and the only thing that
    /// makes a siege take longer than the clock says. Fed on Provisions like any
    /// standing force; an UNFED garrison suspends its contribution (nothing is
    /// destroyed, it recovers when supplied). Also the barracks: it builds the
    /// Troop Transports an assault needs (§ground M7).
    ///
    /// Note the name collides conceptually with `Fleet::garrison_fed`, which is a
    /// different thing entirely — an ALLY FLEET stationed at a host system. This
    /// is ground troops on a planet.
    Garrison,
    /// Ground storage. A founding Warehouse I replaces the old invisible base
    /// allowance; all warehouses still contribute to one system stockpile.
    Warehouse,
    /// A one-tier Warehouse attachment. Light-hull utility fitting only; no slot,
    /// manufacturing, military work, or free workforce.
    ServiceBerth,
    /// Planetary administration. Three levels, one free landing module per
    /// settled world; no workforce posting or ordinary development slot.
    CommandCenter,
    /// §power: the planet's POWER PLANT — one per planet, in its own slot (no
    /// pool slot), on any planet, moon, belt or gas giant. §fuel-overhaul:
    /// Solar Array (I–II, burns nothing), Fusion Plant on Deuterium (III–IV) and
    /// on Helium-3 (V–VI). No crew. See [`crate::power`].
    PowerPlant,
    /// §fuel-overhaul: Ferrite Ore → Uranium, the slow-warp fuel. No research:
    /// every home can make its own from day one.
    UraniumMill,
    /// §fuel-overhaul: Deuterium + a fusion world's power → Antimatter. The
    /// last tech (Antimatter Synthesis).
    AntimatterPlant,
}

impl StructureKind {
    /// Corporation research unlocks construction, not the operation of already
    /// built/captured structures. The founding shipyard, mine, food chain,
    /// housing, storage and Academy stay available so research cannot lock its
    /// own prerequisites. Advanced recipes may use imported goods; researching
    /// their factories opens local production, rather than gating the market.
    pub fn research_prerequisite(self) -> Option<&'static str> {
        match self {
            Self::VolatileHarvester | Self::FuelRefinery => Some("prop_bunkerage"),
            Self::Smelter | Self::ChemicalWorks => Some("mat_enrichment"),
            Self::ElectronicsFabricator => Some("comp_signal_libraries"),
            Self::MachineWorks | Self::PrecisionWorks => Some("mat_autoforges"),
            // §fuel-overhaul: local Warp and Fusion Cores wait for Fusion Reactors.
            Self::DriveWorks => Some(crate::power::FUSION_RESEARCH),
            Self::AntimatterPlant => Some("mat_foundry_vi_antimatter_synthesis"),
            Self::ArmamentsComplex => Some("weap_munitions_lines"),
            Self::CompositeWorks | Self::HullFabricator => Some("mat_prefab_construction"),
            Self::NavalDrydock => Some("mat_autoforges"),
            Self::CapitalSlipway => Some("hull_line_vii_dreadnought"),
            Self::OrdnanceFoundry => Some("hull_drydock_efficiency"),
            Self::OrbitalWarehouse => Some("mat_foundry_iv_orbital_yards"),
            Self::SensorArray => Some("comp_sensor_gain"),
            Self::DefensePlatform | Self::OrbitalDefense | Self::Garrison => Some("weap_fire_control"),
            Self::MiningComplex | Self::Bioharvester | Self::Agroplex | Self::Shipyard
            | Self::Habitat | Self::Warehouse | Self::Academy | Self::ServiceBerth
            | Self::CommandCenter | Self::LivingTerraces | Self::PowerPlant
            | Self::UraniumMill => None,
        }
    }

    /// Alternate initial unlocks, not higher-tier unlocks. Preserve the original
    /// military route while allowing civilian industry to build orbital yards.
    pub fn research_alternatives(self) -> &'static [&'static str] {
        match self {
            Self::NavalDrydock => &["hull_modular_berths"],
            _ => &[],
        }
    }

    /// Every kind, in display order. New kinds go LAST: the client seeds a
    /// planet's first building addresses from this catalogue's order.
    pub const ALL: [StructureKind; 32] = [
        StructureKind::MiningComplex,
        StructureKind::VolatileHarvester,
        StructureKind::Bioharvester,
        StructureKind::Smelter,
        StructureKind::ElectronicsFabricator,
        StructureKind::ChemicalWorks,
        StructureKind::FuelRefinery,
        StructureKind::Agroplex,
        StructureKind::MachineWorks,
        StructureKind::ArmamentsComplex,
        StructureKind::CompositeWorks,
        StructureKind::HullFabricator,
        StructureKind::PrecisionWorks,
        StructureKind::DriveWorks,
        StructureKind::Shipyard,
        StructureKind::NavalDrydock,
        StructureKind::CapitalSlipway,
        StructureKind::OrdnanceFoundry,
        StructureKind::Habitat,
        StructureKind::OrbitalWarehouse,
        StructureKind::SensorArray,
        StructureKind::DefensePlatform,
        StructureKind::OrbitalDefense,
        StructureKind::Academy,
        StructureKind::Garrison,
        StructureKind::Warehouse,
        StructureKind::ServiceBerth,
        StructureKind::CommandCenter,
        StructureKind::LivingTerraces,
        StructureKind::PowerPlant,
        StructureKind::UraniumMill,
        StructureKind::AntimatterPlant,
    ];

    /// Which slot pool a built tier of this kind consumes.
    pub fn slot_pool(self) -> SlotPool {
        match self {
            StructureKind::MiningComplex
            | StructureKind::VolatileHarvester
            | StructureKind::Bioharvester => SlotPool::Resource,
            StructureKind::Smelter
            | StructureKind::ElectronicsFabricator
            | StructureKind::ChemicalWorks
            | StructureKind::FuelRefinery
            | StructureKind::MachineWorks
            | StructureKind::ArmamentsComplex
            | StructureKind::CompositeWorks
            | StructureKind::HullFabricator
            | StructureKind::PrecisionWorks
            | StructureKind::DriveWorks
            | StructureKind::UraniumMill
            | StructureKind::AntimatterPlant
            // §yards: the whole yard family is INDUSTRIAL. That pool is already
            // the tightest one (base 2 per body + population tier), which is the
            // point: a shipbuilding world visibly gives up its other industry.
            | StructureKind::Shipyard
            | StructureKind::NavalDrydock
            | StructureKind::CapitalSlipway
            | StructureKind::OrdnanceFoundry => SlotPool::Industrial,
            // §economy Part 3: the AGROPLEX is CIVIC — food security lives in
            // the Infrastructure pool (Habitat + Agroplex = a self-feeding
            // outpost on the base 2 slots, no industrial investment needed).
            // §industrial-headroom: the industrial base is now 2, so a fresh home
            // has a free industrial slot beyond the Shipyard's — a second industry
            // no longer waits on a DEVELOPED colony. The raider gate is purely the
            // Shipyard-tier-2 requirement now, not industrial-slot scarcity.
            StructureKind::Agroplex
            | StructureKind::Habitat
            | StructureKind::LivingTerraces
            | StructureKind::Warehouse
            | StructureKind::ServiceBerth
            | StructureKind::CommandCenter
            // §power: grouped with the civic utilities; it takes no slot.
            | StructureKind::PowerPlant
            | StructureKind::OrbitalWarehouse
            | StructureKind::SensorArray
            | StructureKind::DefensePlatform
            | StructureKind::OrbitalDefense
            | StructureKind::Academy
            // §ground: the garrison is civic infrastructure — a colony defends
            // itself out of the same budget that houses and feeds it.
            | StructureKind::Garrison => SlotPool::Infrastructure,
        }
    }

    /// Offered in the builder. The legacy Defense Platform is not: its tiers
    /// (pirate bases, old colonies) still stand and fight, but stations are
    /// built as `OrbitalDefense` (§orbital-defense).
    pub fn offered(self) -> bool {
        self != Self::DefensePlatform
    }

    /// The snake_case wire slug (matches `rename_all`).
    pub fn slug(self) -> &'static str {
        match self {
            StructureKind::MiningComplex => "mining_complex",
            StructureKind::VolatileHarvester => "volatile_harvester",
            StructureKind::Bioharvester => "bioharvester",
            StructureKind::Smelter => "smelter",
            StructureKind::ElectronicsFabricator => "electronics_fabricator",
            StructureKind::ChemicalWorks => "chemical_works",
            StructureKind::FuelRefinery => "fuel_refinery",
            StructureKind::Agroplex => "agroplex",
            StructureKind::MachineWorks => "machine_works",
            StructureKind::ArmamentsComplex => "armaments_complex",
            StructureKind::CompositeWorks => "composite_works",
            StructureKind::HullFabricator => "hull_fabricator",
            StructureKind::PrecisionWorks => "precision_works",
            StructureKind::DriveWorks => "drive_works",
            StructureKind::Shipyard => "shipyard",
            StructureKind::NavalDrydock => "naval_drydock",
            StructureKind::CapitalSlipway => "capital_slipway",
            StructureKind::OrdnanceFoundry => "ordnance_foundry",
            StructureKind::Habitat => "habitat",
            StructureKind::LivingTerraces => "living_terraces",
            StructureKind::Warehouse => "warehouse",
            StructureKind::ServiceBerth => "service_berth",
            StructureKind::CommandCenter => "command_center",
            StructureKind::OrbitalWarehouse => "orbital_warehouse",
            StructureKind::SensorArray => "sensor_array",
            StructureKind::DefensePlatform => "defense_platform",
            StructureKind::OrbitalDefense => "orbital_defense",
            StructureKind::Academy => "academy",
            StructureKind::Garrison => "garrison",
            StructureKind::PowerPlant => "power_plant",
            StructureKind::UraniumMill => "uranium_mill",
            StructureKind::AntimatterPlant => "antimatter_plant",
        }
    }

    /// Human title for panels / timeline prose.
    pub fn title(self) -> &'static str {
        match self {
            StructureKind::MiningComplex => "Mining Complex",
            StructureKind::VolatileHarvester => "Volatile Harvester",
            StructureKind::Bioharvester => "Bioharvester",
            StructureKind::Smelter => "Smelter",
            StructureKind::ElectronicsFabricator => "Electronics Fabricator",
            StructureKind::ChemicalWorks => "Chemical Works",
            StructureKind::FuelRefinery => "Deuterium Refinery",
            StructureKind::Agroplex => "Agroplex",
            StructureKind::MachineWorks => "Machine Works",
            StructureKind::ArmamentsComplex => "Armaments Complex",
            StructureKind::CompositeWorks => "Composite Works",
            StructureKind::HullFabricator => "Hull Fabricator",
            StructureKind::PrecisionWorks => "Precision Works",
            StructureKind::DriveWorks => "Drive Works",
            StructureKind::Shipyard => "Shipyard",
            StructureKind::NavalDrydock => "Orbital Shipyard",
            StructureKind::CapitalSlipway => "Capital Slipway",
            StructureKind::OrdnanceFoundry => "Ordnance Foundry",
            StructureKind::Habitat => "Habitat",
            StructureKind::LivingTerraces => "Living Terraces",
            StructureKind::Warehouse => "Warehouse",
            StructureKind::ServiceBerth => "Service Berth",
            StructureKind::CommandCenter => "Command Center",
            StructureKind::OrbitalWarehouse => "Orbital Warehouse",
            StructureKind::SensorArray => "Sensor Array",
            StructureKind::DefensePlatform => "Defense Platform",
            StructureKind::OrbitalDefense => "Orbital Defense",
            StructureKind::Academy => "Academy",
            StructureKind::Garrison => "Garrison",
            StructureKind::PowerPlant => "Power Plant",
            StructureKind::UraniumMill => "Uranium Mill",
            StructureKind::AntimatterPlant => "Antimatter Plant",
        }
    }
}

impl StructureKind {
    pub fn uses_slot(self) -> bool {
        !matches!(self, Self::ServiceBerth | Self::CommandCenter | Self::PowerPlant)
    }

    pub fn tier_title(self, tier: u32) -> &'static str {
        if self == Self::OrbitalDefense { return crate::station::level_name(tier.max(1)); }
        if self == Self::PowerPlant {
            return if tier >= crate::power::FUSION_CORE_TIER { "Fusion Plant" } else { "Solar Array" };
        }
        if self != Self::CommandCenter { return self.title(); }
        match tier {
            0 | 1 => "Landing Command",
            2 => "Colony Command Center",
            _ => "Syndicate Headquarters",
        }
    }
}

/// Administration is local to the receiving planet. Never modifies light speed,
/// the corporation's command origin, shipyard work or Academy research output.
pub fn command_construction_mult(tier: u32) -> f64 {
    match tier { 0 | 1 => 1.0, 2 => 0.90, _ => 0.80 }
}
pub fn command_infrastructure_bonus(tier: u32) -> u32 { tier.saturating_sub(1).min(2) }
pub const COMMAND_CENTER_MIN_POPULATION: f64 = 0.001;

pub const COMMAND_CENTER_RECIPES: [Recipe; 3] = [
    Recipe { costs: &[(Commodity::Alloys, 15.0), (Commodity::Machinery, 5.0)], build_ticks: 180 * HZ },
    Recipe { costs: &[(Commodity::Alloys, 40.0), (Commodity::Machinery, 12.0), (Commodity::Electronics, 8.0)], build_ticks: 300 * HZ },
    Recipe { costs: &[(Commodity::Alloys, 120.0), (Commodity::Machinery, 40.0), (Commodity::Electronics, 30.0), (Commodity::Polymers, 20.0)], build_ticks: 900 * HZ },
];

/// Most structures retain their original per-upgrade recipe. Command Center
/// tiers have explicit costs; callers must include pending upgrades in `tier`.
pub fn recipe_for_tier(what: BuildKind, tier: u32) -> &'static Recipe {
    if matches!(what, BuildKind::Upgrade { upgrade: StructureKind::CommandCenter }) {
        &COMMAND_CENTER_RECIPES[tier.clamp(1, 3) as usize - 1]
    } else if matches!(what, BuildKind::Upgrade { upgrade: StructureKind::PowerPlant })
        && tier >= crate::power::FUSION_TIER
    {
        &FUSION_PLANT_RECIPE
    } else if matches!(what, BuildKind::Upgrade { upgrade: StructureKind::PowerPlant })
        && tier >= crate::power::FUSION_CORE_TIER
    {
        &DEUTERIUM_FUSION_PLANT_RECIPE
    } else if matches!(what, BuildKind::Upgrade { upgrade: StructureKind::OrbitalDefense }) {
        &crate::station::LEVELS[tier.clamp(1, crate::station::MAX_LEVEL) as usize - 1].recipe
    } else { recipe_for(what) }
}

/// A queued construction job; ship completion requires earned yard work. Lives on
/// the `World` (not the system) so an ownership flip mid-build is unambiguous: the
/// ship is delivered to whoever PAID (`owner`), even if the system is later lost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildJob {
    /// Monotonic id (from `World.next_build_id`) — stable iteration / determinism.
    pub id: u64,
    /// Who paid; keeps the asset even if the system is later lost.
    pub owner: PlayerId,
    /// Where it spawns (ship) / what it upgrades.
    pub system: EntityId,
    /// §bodies: the BODY this job builds on (ship jobs use that yard's workforce;
    /// courses use the Academy's body). `default` 0 lets
    /// pre-bodies snapshots parse; migration re-sites in-flight jobs.
    #[serde(default)]
    pub body_id: u32,
    pub what: BuildKind,
    /// Freeze the working yard at receipt. Older paid large-freighter orders
    /// have no field and finish at their original ground Shipyard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub construction_yard: Option<StructureKind>,
    /// Receipt/enqueue tick, before any wait for the planet's construction slot.
    /// Absent in legacy saves; a modern waiting structure has this but no start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_tick: Option<u64>,
    /// Actual start of this job's clock (enqueue for ships/courses, activation
    /// for structures). Waiting never counts as construction. Ships retain
    /// earned work separately so a staffing change never rewrites their start.
    /// Legacy saves lack this fact; do not invent it from today's recipe/bonuses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_tick: Option<u64>,
    /// Estimated completion tick; u64::MAX while waiting/paused. For work-based
    /// jobs this is derived from their work, never an independent completion clock.
    pub complete_tick: u64,
    /// Staffed hull/utility work; absent for other jobs and old saves. Legacy ships migrate from their
    /// recorded span on the next step, never from today's recipe or research.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ship_work: Option<BuildWork>,
    /// Structures share one active work slot per (system, body), FIFO by job id.
    /// Rate zero means queued, one means building. Costs/duration freeze at receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_work: Option<BuildWork>,
    /// For a ship build (§FLEETS management v1): the fleet to JOIN on completion
    /// if it's still the owner's and docked at this system — otherwise the new
    /// ship forms its own fleet-of-one. `None` always forms a new fleet.
    /// serde default keeps pre-FLEETS build jobs loading.
    #[serde(default)]
    pub join: Option<EntityId>,
    /// §modules Part B4: the loadout the built SHIP is fitted with on spawn
    /// (modules already debited from the system ledger at enqueue). serde default
    /// = unfitted, so pre-module build jobs complete as stock ships.
    #[serde(default)]
    pub loadout: crate::module::Loadout,
}

impl BuildJob {
    pub fn work_yard(&self) -> Option<StructureKind> {
        match self.what {
            BuildKind::Ship { ship } => Some(self.construction_yard.unwrap_or_else(|| match ship {
                ShipKind::LargeFreighter | ShipKind::HeavyFreighter | ShipKind::BulkFreighter => StructureKind::Shipyard,
                _ => yard_for(ship).0,
            })),
            BuildKind::Module { module } if module.is_utility() => Some(module.workshop()),
            _ => None,
        }
    }

    pub fn work(&self) -> Option<&BuildWork> {
        self.ship_work.as_ref().or(self.structure_work.as_ref())
    }

    pub fn is_queued_structure(&self) -> bool {
        matches!(self.what, BuildKind::Upgrade { .. } | BuildKind::Founding { .. })
            && self.structure_work.as_ref().is_some_and(|work| work.rate == 0.0)
    }
}

/// Piecewise-linear earned work, in unboosted build ticks. Re-anchor ONLY when
/// the yard's rate changes: unchanged ticks need no writes or new full planetary
/// reports. A paused/queued segment has rate zero and preserves completed work.
/// Structures use rate one only at the head of their planet's queue; ships use
/// the operating yard's staffing rate independently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildWork {
    pub required: f64,
    pub completed: f64,
    pub at_tick: u64,
    pub rate: f64,
}

impl BuildWork {
    pub fn completed_at(&self, tick: u64) -> f64 {
        (self.completed + tick.saturating_sub(self.at_tick) as f64 * self.rate)
            .min(self.required)
    }

    pub fn set_rate(&mut self, tick: u64, rate: f64) {
        if self.rate != rate {
            self.completed = self.completed_at(tick);
            self.at_tick = tick;
            self.rate = rate;
        }
    }

    pub fn completion_tick(&self) -> Option<u64> {
        if self.completed >= self.required {
            Some(self.at_tick)
        } else if self.rate > 0.0 {
            Some(self.at_tick.saturating_add(((self.required - self.completed) / self.rate).ceil() as u64))
        } else {
            None
        }
    }
}

/// A queued REFIT. Military work pulls hulls into the foundry and returns them
/// on completion; basic utility work retains the berthed formation and identity.
/// Added crates are reserved at enqueue. In-place removals return their crates
/// only at completion, never while the module is still installed. Rides its
/// OWN small queue parallel to `build_queue`; `#[serde(default)]` empties on the
/// World = zero migration. Detached military hulls remain their fleet owner's
/// property; in-place utility work is cancelled if the fleet loses its dock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefitJob {
    /// Monotonic id (shares `World.next_build_id`) — stable iteration.
    pub id: u64,
    /// Who owns the hulls (keeps them even if the yard's system is later lost).
    pub owner: PlayerId,
    /// The yard's system — where the refitted hulls reappear.
    pub system: EntityId,
    /// The fleet to REJOIN on completion, if it's still the owner's, Idle, and
    /// docked here — otherwise the hulls form a fresh fleet-of-one at the yard.
    pub fleet: EntityId,
    /// The hull kind being refitted.
    pub ship: ShipKind,
    /// The loadout the hulls carry when they rejoin (empty = stripped to stock).
    pub to: crate::module::Loadout,
    /// How many hulls are in the yard.
    pub n: u32,
    /// §roster: THE ACTUAL HULLS in the yard, carrying their remaining health.
    /// A refit changes what a ship carries — it does not mend it. Storing counts
    /// here (as this did before) minted fresh full-health hulls on completion,
    /// which made a refit a free full repair; and because the yard takes the
    /// most-damaged hulls first, it was an optimally-efficient one. The Ordnance
    /// Foundry's repair service is the only thing that restores hull.
    /// `#[serde(default)]` so a pre-roster job in flight still completes (it
    /// falls back to `n` fresh hulls, the old behaviour, exactly once).
    #[serde(default)]
    pub hulls: Vec<crate::ship::Ship>,
    /// Basic utility work leaves hulls berthed under their existing fleet id.
    /// Their old fitting remains observable until completion emits the new one.
    /// This also preserves the captain, manifest and sensor history.
    #[serde(default)]
    pub in_place: bool,
    #[serde(default)]
    pub work: Option<BuildWork>,
    /// A limited warehouse attachment may fit light utility hulls. If its crew
    /// was borrowed from mining, restore that posting when the job ends.
    #[serde(default)]
    pub service: Option<ServiceAssignment>,
    /// Fuel accompanies hulls removed for military work; a refit never refuels.
    #[serde(default)]
    pub fuel: f64,
    /// Absolute sim tick of completion.
    pub complete_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceAssignment {
    pub body: u32,
    pub borrowed: Option<(u32, crate::production::Assignment)>,
}

pub fn service_berth_hull(ship: ShipKind) -> bool {
    matches!(ship, ShipKind::Scout | ShipKind::Raider | ShipKind::TinyFreighter | ShipKind::SmallFreighter)
}

/// One recipe: commodity costs (whole units; the stockpile is f64) + duration in
/// ticks. `'static` const so the whole sink is deterministic and allocation-free.
pub struct Recipe {
    pub costs: &'static [(Commodity, f64)],
    pub build_ticks: u64,
}

// --- TUNABLE RECIPES (the growth-sink knobs) -------------------------------
use crate::config::TICK_HZ;
const HZ: u64 = TICK_HZ as u64;

// Freight progression: early ore sales pay for imported Alloys, Machinery and
// Polymers. Tiny remains the cheapest replacement/secondary-route hull; research
// unlocks larger holds, never an automatic replacement of existing ships.
// Medium introduces Electronics, then Large/Heavy/Bulk introduce Hull Sections,
// Precision Components and Warp Cores. These replace some bulk basics;
// at reference market prices each step costs more outright but less per cargo
// unit. Savings assume useful loads: a half-empty giant is not a free upgrade.
// Capacities, fuel, upkeep and slower heavy-hull speeds live in ship.rs. Tunable.
//
// §fuel-overhaul: no recipe carries fuel (new hulls fill at the yard's dock),
// and every warp hull carries the Authority's Warp Core — one for small hulls,
// more for big ones. The Survey Probe's burn-out drive needs none.

/// Opening freight hull; the first export fits into its 50-unit mixed hold.
pub const TINY_FREIGHTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 20.0), (Commodity::Machinery, 8.0), (Commodity::Polymers, 8.0),
        (Commodity::WarpCore, 1.0)],
    build_ticks: 12 * HZ,
};
pub const SMALL_FREIGHTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 45.0), (Commodity::Machinery, 16.0), (Commodity::Polymers, 16.0),
        (Commodity::WarpCore, 1.0)],
    build_ticks: 24 * HZ,
};
pub const LARGE_FREIGHTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 180.0), (Commodity::Machinery, 45.0), (Commodity::Electronics, 25.0), (Commodity::HullSections, 12.0),
        (Commodity::WarpCore, 3.0)],
    build_ticks: 100 * HZ,
};
pub const HEAVY_FREIGHTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 320.0), (Commodity::Machinery, 70.0), (Commodity::HullSections, 30.0), (Commodity::PrecisionComponents, 15.0),
        (Commodity::WarpCore, 5.0)],
    build_ticks: 180 * HZ,
};
pub const BULK_FREIGHTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 520.0), (Commodity::Machinery, 100.0), (Commodity::HullSections, 60.0),
        (Commodity::PrecisionComponents, 25.0), (Commodity::WarpCore, 16.0)],
    build_ticks: 300 * HZ,
};
/// Legacy Convoy identifier now names the Medium Freighter.
pub const CONVOY_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 90.0),
        (Commodity::Machinery, 28.0),
        (Commodity::Polymers, 20.0),
        (Commodity::Electronics, 10.0),
        (Commodity::WarpCore, 2.0),
    ],
    build_ticks: 50 * HZ,
};
/// Raider (Interceptor): costlier, needs the good frontier materials.
pub const RAIDER_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 20.0),
        (Commodity::Electronics, 12.0),
        (Commodity::Armaments, 15.0),
        (Commodity::WarpCore, 1.0),
    ],
    build_ticks: 10 * HZ,
};
/// §charting: the Survey Probe — a cheap, crewless, one-way drone that charts a
/// system and is spent doing it. Its burn-out drive needs no Warp Core. Tunable.
pub const SURVEY_PROBE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 6.0), (Commodity::Electronics, 4.0)],
    build_ticks: 10 * HZ,
};
/// §belts: the Mining Barge — about a Small Freighter's goods, with the
/// Electronics of its cutters. No research: the founding charter teaches it.
pub const MINING_BARGE_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 40.0),
        (Commodity::Machinery, 20.0),
        (Commodity::Polymers, 12.0),
        (Commodity::Electronics, 6.0),
        (Commodity::WarpCore, 1.0),
    ],
    build_ticks: 30 * HZ,
};
/// §belts: the Mining Belt Station, a Shipyard II hull behind Belt Stations.
pub const MINING_BELT_STATION_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 160.0),
        (Commodity::Machinery, 50.0),
        (Commodity::Polymers, 30.0),
        (Commodity::Electronics, 25.0),
        (Commodity::WarpCore, 2.0),
    ],
    build_ticks: 180 * HZ,
};
/// Scout: the cheap entry unit (cheap enough that a caught scout is an
/// acceptable loss).
pub const SCOUT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 15.0),
        (Commodity::Electronics, 8.0),
        (Commodity::WarpCore, 1.0),
    ],
    build_ticks: 8 * HZ,
};
/// Corvette: **Ore + Alloys** — the dedicated defender; military industry.
pub const CORVETTE_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 25.0),
        (Commodity::Electronics, 12.0),
        (Commodity::Armaments, 12.0),
        (Commodity::WarpCore, 1.0),
    ],
    build_ticks: 14 * HZ,
};
/// Colony Ship: **Ore + Alloys + Provisions** (colonists eat) — absorbs the old
/// instant-claim economics into a physical, raidable investment (§ships part 3).
pub const COLONY_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 45.0),
        (Commodity::Machinery, 15.0),
        (Commodity::Polymers, 20.0),
        (Commodity::Provisions, 30.0),
        (Commodity::WarpCore, 2.0),
    ],
    build_ticks: 30 * HZ,
};
// §ladder: CAPITAL recipes — Rare-Elements-and-Machinery-heavy by design (the
// deep-crust economy is the capital economy; §ore-ladder prices rare elements
// at 100 and sizes these draws in the same credit share as before), and build
// TIMES measured in
// hours-to-days: a capital under construction is a season event and a siege
// target. Combat weight per Armaments spent peaks at Destroyer/Cruiser and
// declines up the ladder (the efficiency invariant, pinned by test). Tunable.
//
// §rare-materials: the three ships of the line also take Iridium, the belt
// pockets' metal, in place of Alloys at equal base value (13 Iridium for 45
// Alloys), so every hull's credit value is unchanged.
const HOUR_TICKS: u64 = 3600 * HZ;
pub const DESTROYER_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 40.0),
        (Commodity::Electronics, 25.0),
        (Commodity::Armaments, 30.0),
        (Commodity::Machinery, 10.0),
        (Commodity::HullSections, 6.0),
        (Commodity::PrecisionComponents, 4.0),
        (Commodity::WarpCore, 2.0),
    ],
    build_ticks: 8 * HOUR_TICKS,
};
pub const CRUISER_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 60.0),
        (Commodity::Electronics, 35.0),
        (Commodity::Armaments, 55.0),
        (Commodity::Machinery, 20.0),
        (Commodity::RareElements, 3.0),
        (Commodity::HullSections, 16.0),
        (Commodity::WarpCore, 7.0),
    ],
    build_ticks: 18 * HOUR_TICKS,
};
pub const BATTLESHIP_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 95.0),
        (Commodity::Iridium, 13.0),
        (Commodity::Electronics, 60.0),
        (Commodity::Armaments, 120.0),
        (Commodity::Machinery, 40.0),
        (Commodity::RareElements, 9.0),
        (Commodity::HullSections, 40.0),
        (Commodity::WarpCore, 14.0),
    ],
    build_ticks: 48 * HOUR_TICKS,
};
pub const DREADNOUGHT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 190.0),
        (Commodity::Iridium, 26.0),
        (Commodity::Electronics, 130.0),
        (Commodity::Armaments, 230.0),
        (Commodity::Machinery, 90.0),
        (Commodity::RareElements, 22.0),
        (Commodity::HullSections, 80.0),
        (Commodity::WarpCore, 31.0),
    ],
    build_ticks: 96 * HOUR_TICKS,
};
pub const TITAN_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 375.0),
        (Commodity::Iridium, 65.0),
        (Commodity::Electronics, 250.0),
        (Commodity::Armaments, 480.0),
        (Commodity::Machinery, 200.0),
        (Commodity::RareElements, 55.0),
        (Commodity::HullSections, 180.0),
        (Commodity::WarpCore, 70.0),
    ],
    build_ticks: 192 * HOUR_TICKS,
};
// §economy Part 5: the FULL industrial-web cost table (design doc). Everything
// advanced needs MACHINERY, and early Machinery comes from Sol — the intended
// loop is extract → sell raws → buy Machinery → build industry → make your own.
// they need Machinery/Electronics, purchasable at the hub (Sol's off-map
// industry lists every good from day one).
pub const MINING_COMPLEX_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 12.0), (Commodity::Alloys, 25.0)],
    build_ticks: 18 * HZ,
};
pub const VOLATILE_HARVESTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 12.0), (Commodity::Alloys, 25.0)],
    build_ticks: 18 * HZ,
};
pub const BIOHARVESTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 12.0), (Commodity::Alloys, 25.0)],
    build_ticks: 18 * HZ,
};
pub const SMELTER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)],
    build_ticks: 20 * HZ,
};
pub const ELECTRONICS_FABRICATOR_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Machinery, 15.0),
        (Commodity::Alloys, 20.0),
        (Commodity::Silicates, 10.0),
    ],
    build_ticks: 20 * HZ,
};
pub const CHEMICAL_WORKS_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)],
    build_ticks: 20 * HZ,
};
pub const FUEL_REFINERY_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)],
    build_ticks: 20 * HZ,
};
pub const AGROPLEX_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)],
    build_ticks: 20 * HZ,
};
pub const MACHINE_WORKS_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Machinery, 20.0),
        (Commodity::Alloys, 40.0),
        (Commodity::Electronics, 15.0),
    ],
    build_ticks: 22 * HZ,
};
pub const ARMAMENTS_COMPLEX_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Machinery, 20.0),
        (Commodity::Alloys, 40.0),
        (Commodity::Electronics, 15.0),
    ],
    build_ticks: 22 * HZ,
};
// §industry-chains: factory bootstrap uses only the existing economy. Component
// costs substitute for heavy-hull bulk inputs at roughly equal base-market
// value, rather than adding a new tax to every ship. All values are tunable.
pub const COMPOSITE_WORKS_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 30.0), (Commodity::Alloys, 45.0), (Commodity::Polymers, 20.0)],
    build_ticks: 30 * HZ,
};
pub const HULL_FABRICATOR_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 40.0), (Commodity::Alloys, 60.0), (Commodity::Electronics, 20.0)],
    build_ticks: 40 * HZ,
};
pub const PRECISION_WORKS_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 30.0), (Commodity::Alloys, 35.0), (Commodity::Electronics, 25.0), (Commodity::RareElements, 3.0)],
    build_ticks: 35 * HZ,
};
pub const DRIVE_WORKS_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 45.0), (Commodity::Alloys, 60.0), (Commodity::Electronics, 30.0)],
    build_ticks: 45 * HZ,
};
pub const SHIPYARD_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Machinery, 20.0),
        (Commodity::Alloys, 40.0),
        (Commodity::Electronics, 15.0),
    ],
    build_ticks: 20 * HZ,
};
// §yards: the yard family climbs steeply — a Drydock is roughly twice a Shipyard
// and a Slipway roughly twice again, with Armaments entering at the Drydock and
// Rare Elements at the Slipway (the capital economy, mirroring the hull recipes).
// The Foundry is the cheap one: outfitting is not construction. All Tunable.
pub const NAVAL_DRYDOCK_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Machinery, 35.0),
        (Commodity::Alloys, 70.0),
        (Commodity::Electronics, 25.0),
        (Commodity::Armaments, 15.0),
    ],
    build_ticks: 30 * HZ,
};
pub const CAPITAL_SLIPWAY_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Machinery, 70.0),
        (Commodity::Alloys, 150.0),
        (Commodity::Electronics, 55.0),
        (Commodity::Armaments, 40.0),
        (Commodity::RareElements, 4.0),
    ],
    build_ticks: 45 * HZ,
};
pub const ORDNANCE_FOUNDRY_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Machinery, 18.0),
        (Commodity::Alloys, 35.0),
        (Commodity::Electronics, 20.0),
    ],
    build_ticks: 18 * HZ,
};
pub const HABITAT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 30.0),
        (Commodity::Polymers, 20.0),
        (Commodity::Machinery, 8.0),
    ],
    build_ticks: 20 * HZ,
};
/// §power, §fuel-overhaul: a Solar Array tier (I–II) — panel fields on an
/// alloy frame, cheap enough to raise beside the first upgrades after the
/// charter.
pub const POWER_PLANT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 30.0),
        (Commodity::Machinery, 12.0),
        (Commodity::Electronics, 8.0),
    ],
    build_ticks: 25 * HZ,
};
/// §fuel-overhaul: a Deuterium Fusion Plant tier (III–IV) — the solar frame
/// plus the Authority's Fusion Core.
pub const DEUTERIUM_FUSION_PLANT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 30.0),
        (Commodity::Machinery, 12.0),
        (Commodity::Electronics, 8.0),
        (Commodity::FusionCore, 1.0),
    ],
    build_ticks: 40 * HZ,
};
/// §power: each Helium-3 fusion tier (V–VI) — reactor walls of Iridium around
/// precision field coils and a Fusion Core.
pub const FUSION_PLANT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 120.0),
        (Commodity::Machinery, 40.0),
        (Commodity::Electronics, 30.0),
        (Commodity::PrecisionComponents, 10.0),
        (Commodity::Iridium, 12.0),
        (Commodity::FusionCore, 1.0),
    ],
    build_ticks: 90 * HZ,
};
/// §fuel-overhaul: the Uranium Mill — a cheap ore-sifting plant any home can
/// raise from the founding kit.
pub const URANIUM_MILL_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 20.0), (Commodity::Machinery, 8.0)],
    build_ticks: 15 * HZ,
};
/// §fuel-overhaul: the Antimatter Plant — containment rings of Exotic Matter
/// around Fusion Cores. The last tech's factory.
pub const ANTIMATTER_PLANT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 200.0),
        (Commodity::Machinery, 80.0),
        (Commodity::Electronics, 60.0),
        (Commodity::PrecisionComponents, 30.0),
        (Commodity::ExoticMatter, 20.0),
        (Commodity::FusionCore, 2.0),
    ],
    build_ticks: 120 * HZ,
};
/// §gaia: grown more than built — living soil (Biomass) and polymer membranes
/// over an alloy frame; slower than a Habitat to raise.
pub const LIVING_TERRACES_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 30.0),
        (Commodity::Polymers, 25.0),
        (Commodity::Biomass, 20.0),
        (Commodity::Machinery, 10.0),
    ],
    build_ticks: 30 * HZ,
};
pub const WAREHOUSE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 30.0), (Commodity::Machinery, 8.0)],
    build_ticks: 15 * HZ,
};
pub const SERVICE_BERTH_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 5.0), (Commodity::Machinery, 3.0)],
    build_ticks: 120 * HZ,
};
/// Bulk orbital storage is a late industrial investment. Tunable.
pub const ORBITAL_WAREHOUSE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 120.0), (Commodity::Machinery, 40.0),
        (Commodity::Electronics, 30.0)],
    build_ticks: 80 * HZ,
};
pub const SENSOR_ARRAY_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Electronics, 18.0), (Commodity::Machinery, 10.0)],
    build_ticks: 18 * HZ,
};
pub const DEFENSE_PLATFORM_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 35.0),
        (Commodity::Electronics, 15.0),
        (Commodity::Armaments, 15.0),
    ],
    build_ticks: 22 * HZ,
};
pub const ACADEMY_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 25.0),
        (Commodity::Electronics, 15.0),
        (Commodity::Provisions, 20.0),
    ],
    build_ticks: 20 * HZ,
};
/// §ground: a Garrison is armaments and people, not heavy industry — cheap
/// enough that any colony can dig in, dear enough that doing it everywhere costs.
/// §ground M7: a trooper is people and their kit, not heavy industry — built at
/// a Garrison, and priced so an invasion is an investment rather than a whim.
pub const TRANSPORT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 40.0),
        (Commodity::Armaments, 30.0),
        (Commodity::Provisions, 40.0),
        (Commodity::WarpCore, 2.0),
    ],
    build_ticks: 24 * HZ,
};
pub const GARRISON_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Armaments, 20.0),
        (Commodity::Alloys, 25.0),
        (Commodity::Provisions, 25.0),
    ],
    build_ticks: 20 * HZ,
};

/// §economy Part 4: one Academy training course (Provisions feed the cohort,
/// Electronics equip the lab). Costs live in `specialist::ACADEMY_TRAIN_COSTS`.
pub const ACADEMY_TRAIN_RECIPE: Recipe = Recipe {
    costs: crate::specialist::ACADEMY_TRAIN_COSTS,
    build_ticks: crate::specialist::ACADEMY_TRAIN_TICKS,
};

/// Officer commissioning is a materially larger Academy commitment than one
/// specialist course. Tunable; the roster cap prevents queue spam from turning
/// commodities into an unlimited officer pool.
pub const CAPTAIN_RECRUIT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Provisions, 40.0),
        (Commodity::Electronics, 20.0),
        (Commodity::Machinery, 10.0),
    ],
    build_ticks: 60 * HZ,
};

// --- §modules Part B3: MODULE RECIPES (manufactured items) --------------------
// Built from Armaments + Electronics, with a real Silicates sink for the glass
// mirrors (Reflective) and a Machinery draw for the heavy spaced armor (Whipple).
// Quicker than a structure — a module is a crate, not a colony. All Tunable.
const MODULE_BUILD_TICKS: u64 = 10 * HZ;
/// §modules Part B4: REFIT duration PER SHIP — a fit swap on an existing hull is
/// quicker than manufacturing the crate, and scales with how many hulls go in the
/// yard at once (n ships ⇒ n × this). Tunable.
pub const REFIT_TICKS_PER_SHIP: u64 = 3 * HZ;
pub const MASS_DRIVER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Armaments, 8.0), (Commodity::Electronics, 4.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const TORPEDO_RACK_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Armaments, 10.0), (Commodity::Electronics, 6.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const POINT_DEFENSE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Armaments, 6.0), (Commodity::Electronics, 8.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const REFLECTIVE_PLATING_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Armaments, 6.0),
        (Commodity::Silicates, 6.0),
        (Commodity::Electronics, 2.0),
    ],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const WHIPPLE_ARMOR_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Armaments, 12.0), (Commodity::Machinery, 2.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const EXTENDED_TANKS_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 12.0), (Commodity::Polymers, 8.0), (Commodity::Machinery, 4.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const RECON_SUITE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Electronics, 12.0), (Commodity::Silicates, 8.0), (Commodity::Machinery, 4.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const CARGO_PODS_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 18.0), (Commodity::Polymers, 12.0), (Commodity::Machinery, 6.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
pub const ESCORT_DATALINK_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Electronics, 16.0), (Commodity::Alloys, 12.0), (Commodity::Machinery, 6.0)],
    build_ticks: MODULE_BUILD_TICKS,
};

pub const FUEL_TRANSFER_RIG_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 12.0), (Commodity::Alloys, 18.0), (Commodity::Polymers, 10.0)],
    build_ticks: MODULE_BUILD_TICKS,
};

pub const SURVEY_DRIVE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::WarpCore, 2.0), (Commodity::Alloys, 12.0)],
    build_ticks: 2 * MODULE_BUILD_TICKS,
};
pub const NEBULA_SPECTROMETER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::PrecisionComponents, 4.0), (Commodity::Electronics, 12.0)],
    build_ticks: 2 * MODULE_BUILD_TICKS,
};
/// §rare-materials: the special modules take Exotic Matter, about a fifth of
/// their value, in place of goods of equal base value.
pub const PRISMATIC_LANCE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::PrecisionComponents, 4.0), (Commodity::Armaments, 8.0), (Commodity::Silicates, 6.0),
        (Commodity::ExoticMatter, 2.0)],
    build_ticks: 2 * MODULE_BUILD_TICKS,
};

/// Control electronics around a field coil.
pub const STASIS_WEB_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Electronics, 8.0), (Commodity::Machinery, 4.0), (Commodity::Armaments, 2.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
/// A hardened drive core: the price of being able to leave.
pub const WARP_STABILIZER_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Machinery, 6.0), (Commodity::Electronics, 4.0), (Commodity::Alloys, 6.0)],
    build_ticks: MODULE_BUILD_TICKS,
};
/// A gravity-well projector: the dearest control module.
pub const INTERDICTION_FIELD_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::PrecisionComponents, 3.0), (Commodity::Electronics, 5.0), (Commodity::Machinery, 6.0),
        (Commodity::ExoticMatter, 2.0)],
    build_ticks: 2 * MODULE_BUILD_TICKS,
};
/// §fuel-overhaul: a fold drive — a Warp Core wrapped in Exotic Matter, and
/// its antimatter bottle.
pub const JUMP_DRIVE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::WarpCore, 1.0), (Commodity::PrecisionComponents, 4.0), (Commodity::ExoticMatter, 3.0)],
    build_ticks: 2 * MODULE_BUILD_TICKS,
};

/// The recipe for one module of `kind`.
pub fn module_recipe(kind: ModuleKind) -> &'static Recipe {
    match kind {
        ModuleKind::MassDriver => &MASS_DRIVER_RECIPE,
        ModuleKind::TorpedoRack => &TORPEDO_RACK_RECIPE,
        ModuleKind::PointDefenseScreen => &POINT_DEFENSE_RECIPE,
        ModuleKind::ReflectivePlating => &REFLECTIVE_PLATING_RECIPE,
        ModuleKind::WhippleArmor => &WHIPPLE_ARMOR_RECIPE,
        ModuleKind::ExtendedTanks => &EXTENDED_TANKS_RECIPE,
        ModuleKind::ReconSuite => &RECON_SUITE_RECIPE,
        ModuleKind::CargoPods => &CARGO_PODS_RECIPE,
        ModuleKind::EscortDatalink => &ESCORT_DATALINK_RECIPE,
        ModuleKind::FuelTransferRig => &FUEL_TRANSFER_RIG_RECIPE,
        ModuleKind::SurveyDrive => &SURVEY_DRIVE_RECIPE,
        ModuleKind::NebulaSpectrometer => &NEBULA_SPECTROMETER_RECIPE,
        ModuleKind::PrismaticLance => &PRISMATIC_LANCE_RECIPE,
        ModuleKind::StasisWeb => &STASIS_WEB_RECIPE,
        ModuleKind::WarpStabilizer => &WARP_STABILIZER_RECIPE,
        ModuleKind::InterdictionField => &INTERDICTION_FIELD_RECIPE,
        ModuleKind::JumpDrive => &JUMP_DRIVE_RECIPE,
    }
}

/// A SENSOR is the expensive one: it is an instrument, and it is what lets you
/// see a rival coming before they arrive.
static DEEP_SPACE_SENSOR_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 60.0),
        (Commodity::Electronics, 120.0),
    ],
    build_ticks: 70 * HZ,
};

/// §emplacements: the CONSTRUCTION SHIP — the crane, not the kit. Machinery-
/// heavy because that is what it is; priced so the hull is a real one-time
/// purchase while each emplacement's kit stays the recurring cost.
static BUILDER_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 60.0),
        (Commodity::Machinery, 25.0),
        (Commodity::WarpCore, 1.0),
    ],
    build_ticks: 50 * HZ,
};

/// §emplacements: THE KIT — what an emplacement costs and how long a builder
/// works at the site. Looked up directly (not through `BuildKind`): these no
/// longer ride the yard queue, a Construction Ship carries them out.
pub fn emplacement_recipe(kind: crate::emplace::EmplacementKind) -> &'static Recipe {
    match kind {
        crate::emplace::EmplacementKind::DeepSpaceSensor => &DEEP_SPACE_SENSOR_RECIPE,
    }
}

pub fn recipe_for(what: BuildKind) -> &'static Recipe {
    match what {
        BuildKind::Ship { ship: ShipKind::TinyFreighter } => &TINY_FREIGHTER_RECIPE,
        BuildKind::Ship { ship: ShipKind::SmallFreighter } => &SMALL_FREIGHTER_RECIPE,
        BuildKind::Ship { ship: ShipKind::LargeFreighter } => &LARGE_FREIGHTER_RECIPE,
        BuildKind::Ship { ship: ShipKind::HeavyFreighter } => &HEAVY_FREIGHTER_RECIPE,
        BuildKind::Ship { ship: ShipKind::BulkFreighter } => &BULK_FREIGHTER_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Builder,
        } => &BUILDER_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Convoy,
        } => &CONVOY_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Raider,
        } => &RAIDER_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Scout,
        } => &SCOUT_RECIPE,
        BuildKind::Ship { ship: ShipKind::SurveyProbe } => &SURVEY_PROBE_RECIPE,
        BuildKind::Ship { ship: ShipKind::MiningBarge } => &MINING_BARGE_RECIPE,
        BuildKind::Ship { ship: ShipKind::MiningBeltStation } => &MINING_BELT_STATION_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Corvette,
        } => &CORVETTE_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Colony,
        } => &COLONY_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Destroyer,
        } => &DESTROYER_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Cruiser,
        } => &CRUISER_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Battleship,
        } => &BATTLESHIP_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Dreadnought,
        } => &DREADNOUGHT_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Titan,
        } => &TITAN_RECIPE,
        BuildKind::Ship {
            ship: ShipKind::Transport,
        } => &TRANSPORT_RECIPE,
        // The Authority Freighter (§TCA) is TCA-only and never buildable: the
        // `BuildShip` handler soft-rejects it (via `ShipKind::is_buildable`) BEFORE
        // any recipe lookup, and no `BuildJob` for one can ever be enqueued, so this
        // arm is genuinely unreachable — it exists only for match exhaustiveness.
        BuildKind::Ship {
            ship: ShipKind::Freighter,
        } => {
            unreachable!("Freighter is TCA-only and never buildable — apply_build guards it")
        }
        BuildKind::Train { .. } => &ACADEMY_TRAIN_RECIPE,
        BuildKind::RecruitCaptain { .. } => &CAPTAIN_RECRUIT_RECIPE,
        BuildKind::Module { module } => module_recipe(module),
        BuildKind::Founding { project } => project.recipe(),
        BuildKind::Upgrade { upgrade } => match upgrade {
            StructureKind::MiningComplex => &MINING_COMPLEX_RECIPE,
            StructureKind::VolatileHarvester => &VOLATILE_HARVESTER_RECIPE,
            StructureKind::Bioharvester => &BIOHARVESTER_RECIPE,
            StructureKind::Smelter => &SMELTER_RECIPE,
            StructureKind::ElectronicsFabricator => &ELECTRONICS_FABRICATOR_RECIPE,
            StructureKind::ChemicalWorks => &CHEMICAL_WORKS_RECIPE,
            StructureKind::FuelRefinery => &FUEL_REFINERY_RECIPE,
            StructureKind::Agroplex => &AGROPLEX_RECIPE,
            StructureKind::MachineWorks => &MACHINE_WORKS_RECIPE,
            StructureKind::ArmamentsComplex => &ARMAMENTS_COMPLEX_RECIPE,
            StructureKind::CompositeWorks => &COMPOSITE_WORKS_RECIPE,
            StructureKind::HullFabricator => &HULL_FABRICATOR_RECIPE,
            StructureKind::PrecisionWorks => &PRECISION_WORKS_RECIPE,
            StructureKind::DriveWorks => &DRIVE_WORKS_RECIPE,
            StructureKind::Shipyard => &SHIPYARD_RECIPE,
            StructureKind::NavalDrydock => &NAVAL_DRYDOCK_RECIPE,
            StructureKind::CapitalSlipway => &CAPITAL_SLIPWAY_RECIPE,
            StructureKind::OrdnanceFoundry => &ORDNANCE_FOUNDRY_RECIPE,
            StructureKind::Habitat => &HABITAT_RECIPE,
            StructureKind::LivingTerraces => &LIVING_TERRACES_RECIPE,
            StructureKind::Warehouse => &WAREHOUSE_RECIPE,
            StructureKind::ServiceBerth => &SERVICE_BERTH_RECIPE,
            StructureKind::CommandCenter => &COMMAND_CENTER_RECIPES[0],
            StructureKind::OrbitalWarehouse => &ORBITAL_WAREHOUSE_RECIPE,
            StructureKind::SensorArray => &SENSOR_ARRAY_RECIPE,
            StructureKind::DefensePlatform => &DEFENSE_PLATFORM_RECIPE,
            StructureKind::OrbitalDefense => &crate::station::LEVELS[0].recipe,
            StructureKind::Academy => &ACADEMY_RECIPE,
            StructureKind::Garrison => &GARRISON_RECIPE,
            StructureKind::PowerPlant => &POWER_PLANT_RECIPE,
            StructureKind::UraniumMill => &URANIUM_MILL_RECIPE,
            StructureKind::AntimatterPlant => &ANTIMATTER_PLANT_RECIPE,
        },
    }
}

// --- DEUTERIUM REFINERY (§buildings step 3b → §economy Part 3) --------------------
// The old REFINERY_RATE/YIELD pair is RETIRED: the refinery is one row of the
// data-driven converter table now (`production::CONVERTERS` — 1.0 Volatiles
// per Deuterium at 0.8/s), staffed and factor-chained like all industry.

// --- HABITAT (§buildings step 3a → §economy Part 2) ------------------------------
// The old per-tier upkeep + fed-boost pair is RETIRED: a Habitat now houses
// POPULATION (capacity `colony::POP_CAP_PER_HABITAT_TIER` per tier), and it is
// the population that eats (`colony::PROVISIONS_PER_MILLION_PER_S`), works, and
// unlocks slots. All colony-life tunables live in `crate::colony`.

// --- DEFENSE PLATFORM (§buildings step 2c) ------------------------------------

/// The protection radius a Defense Platform projects around its system (~60% of
/// a sensor bubble). The platform "senses" exactly its own radius — a raid
/// CONTACT occurring inside it is met by the platform; nothing outside it is
/// affected. Simple, deterministic, and fog-clean (the contact is physically
/// there). Tunable.
pub const DEFENSE_PLATFORM_RADIUS: f64 = 1300.0;
/// DEFENSE WEIGHT of one platform tier in the weighted-strength battle model
/// (§ships part 1). With the raider's attack weight 3, a per-tier duel sits at
/// ratio 3/3 = 1.0 → the even row — exactly the old per-tier RVR duel, so
/// pre-existing platform outcomes are numerically unchanged. Tunable.
pub const PLATFORM_TIER_DEFENSE: f64 = 3.0;

// --- SENSOR ARRAY (§buildings step 2b) ----------------------------------------

/// Bubble radius of a tier-1 array — matches the global ship/CC bubble, so one
/// tier buys a ship's worth of standing vision at the system. Tunable.
///
/// Scaled with the galaxy, exactly as `config.sensor_range` is.
/// Left absolute it would have projected 2,200 against a 110,000 fleet bubble —
/// the building would have been worthless, and the panel that now reports its
/// reach as a number would have been reporting a lie.
pub const SENSOR_ARRAY_BASE: f64 = 2200.0 * crate::config::GALAXY_SCALE;
/// Extra radius per tier past the first (+40% of base) — a tier-2 array outsees
/// any ship. Tunable.
pub const SENSOR_ARRAY_PER_TIER: f64 = 880.0 * crate::config::GALAXY_SCALE;

/// The sensor bubble radius an array of `tier` projects (0 = no array).
pub fn sensor_array_radius(tier: u32) -> f64 {
    if tier == 0 {
        0.0
    } else {
        SENSOR_ARRAY_BASE + SENSOR_ARRAY_PER_TIER * (tier - 1) as f64
    }
}

// --- §economy: POPULATION TIERS (drive the derived slot pools) ------------------

/// Population (millions) at which a colony counts as DEVELOPED — unlocking the
/// second Industrial slot and the third Infrastructure slot. Tunable.
pub const POP_DEVELOPED: f64 = 0.010;
/// Population (millions) at which a colony counts as MAJOR — the third
/// Industrial slot. Tunable.
pub const POP_MAJOR: f64 = 0.050;

/// The population tier: 0 below `POP_DEVELOPED`, 1 from there, 2 at `POP_MAJOR`.
/// Shortages never kill population (§economy Part 2), but physical relocation
/// can shrink a local pool. Existing structures remain grandfathered, so there
/// is still no destructive un-build edge case.
pub fn pop_tier(population: f64) -> u32 {
    if population >= POP_MAJOR {
        2
    } else if population >= POP_DEVELOPED {
        1
    } else {
        0
    }
}

// --- YARD GATING (§buildings step 3 → §yards) ---------------------------------

/// WHICH YARD builds a hull, and at what tier. This replaces the old flat
/// "Shipyard tier 1→6" ladder: the cost of a capital fleet is now SLOTS AND
/// GEOGRAPHY (three co-located structures, each needing the one below it — see
/// [`yard_prereq`]) rather than one very tall building.
///
/// The light hulls keep their exact former gates, so the opening is unchanged:
/// homes generate with Shipyard 1 and still build convoys and scouts turn one,
/// and raiders still cost a second Shipyard tier. Only the capital ladder moves.
/// Tunable.
pub fn yard_for(kind: ShipKind) -> (StructureKind, u32) {
    match kind {
        // Local ground yard: light hulls and freight up to Medium.
        ShipKind::Builder => (StructureKind::Shipyard, 1),
        ShipKind::TinyFreighter | ShipKind::SmallFreighter => (StructureKind::Shipyard, 1),
        ShipKind::Convoy => (StructureKind::Shipyard, 2),
        ShipKind::LargeFreighter => (StructureKind::NavalDrydock, 1),
        ShipKind::HeavyFreighter => (StructureKind::NavalDrydock, 2),
        ShipKind::BulkFreighter => (StructureKind::NavalDrydock, 3),
        ShipKind::Scout => (StructureKind::Shipyard, 1),
        ShipKind::SurveyProbe => (StructureKind::Shipyard, 1),
        ShipKind::MiningBarge => (StructureKind::Shipyard, 1),
        ShipKind::MiningBeltStation => (StructureKind::Shipyard, 2),
        ShipKind::Colony => (StructureKind::Shipyard, 1), // civilian settlement
        ShipKind::Raider => (StructureKind::Shipyard, 2), // military industry is earned
        ShipKind::Corvette => (StructureKind::Shipyard, 2),
        // The line of battle — a dedicated drydock.
        ShipKind::Destroyer => (StructureKind::NavalDrydock, 1),
        ShipKind::Cruiser => (StructureKind::NavalDrydock, 2),
        ShipKind::Battleship => (StructureKind::NavalDrydock, 3),
        // The super-capitals — a slipway, at the end of the deepest chain.
        ShipKind::Dreadnought => (StructureKind::CapitalSlipway, 1),
        ShipKind::Titan => (StructureKind::CapitalSlipway, 2),
        // §ground: troops come from BARRACKS, not slipways — the Garrison that
        // defends a world is the same institution that raises an invasion.
        ShipKind::Transport => (StructureKind::Garrison, 1),
        // §TCA: no yard at any tier can EVER build an Authority freighter — a
        // belt-and-suspenders backstop behind `ShipKind::is_buildable`.
        ShipKind::Freighter => (StructureKind::Shipyard, u32::MAX),
    }
}

/// The structure a YARD needs co-located on the same system before it can be
/// founded, if any. This is what makes the ladder geography: you cannot drop a
/// Capital Slipway on a fresh rock, you grow one. `None` for everything else.
/// Tunable.
pub fn yard_prereq(kind: StructureKind) -> Option<(StructureKind, u32)> {
    match kind {
        StructureKind::NavalDrydock => Some((StructureKind::Shipyard, 2)),
        StructureKind::CapitalSlipway => Some((StructureKind::NavalDrydock, 3)),
        StructureKind::OrdnanceFoundry => Some((StructureKind::Shipyard, 1)),
        _ => None,
    }
}

// --- REPAIR (§roster) ---------------------------------------------------------
// The Ordnance Foundry services damaged hulls. Battle damage PERSISTS per ship
// now, so without a cure attrition would be permanent — this is the relief
// valve, and it is deliberately geographic: a forward foundry keeps a fleet in
// the fight, a rear one costs you the trip home.

/// Hull points restored per second, per Foundry tier, at full staffing. The
/// factor chain (`tier × staffing × skill`) multiplies this exactly like any
/// production line. Sized against the hull table (a Raider is 200, a Corvette
/// 800): a staffed tier-1 foundry patches a mauled corvette in a couple of
/// minutes, a capital in far longer. Tunable — the single pacing knob.
pub const REPAIR_HP_PER_SEC_PER_TIER: f64 = 6.0;

/// Goods consumed per HULL POINT restored. Repair is cheap relative to
/// replacing the hull (that is the point — it should beat rebuilding), but not
/// free: a serviced fleet is a supplied fleet. Tunable.
pub const REPAIR_COST_PER_HP: &[(Commodity, f64)] =
    &[(Commodity::Alloys, 0.01), (Commodity::Machinery, 0.002)];

// --- SLIPWAYS (§yards M1) -----------------------------------------------------
// A yard's TIER is also its THROUGHPUT: how many hulls it can have on the stocks
// at once. Before this, ship jobs were unbounded — a system could lay down any
// number simultaneously, which is precisely why tier meant nothing but a gate.
// Each yard KIND counts its own slips, so a Shipyard 3 + Drydock 2 world runs
// three light hulls and two line warships in parallel.

/// Slips one tier of a yard provides. Tunable — the whole knob for build tempo.
pub const SLIPS_PER_TIER: u32 = 1;

/// How many hulls a yard of `tier` can build at once (0 = no yard, no slips).
pub fn slips_for(tier: u32) -> u32 {
    tier.saturating_mul(SLIPS_PER_TIER)
}

/// The Shipyard tier every HOME system starts with (consuming one development
/// slot) — the bootstrap that avoids a convoy chicken-and-egg stall on turn one.
pub const HOME_SHIPYARD_TIER: u32 = 1;

// --- STRUCTURE TIER CEILING (§industrial-headroom) -------------------------------

/// The ordinary structure ceiling (cost + slots permitting). Research-gated
/// structures require their initial unlock before using this ladder.
/// Tunable.
pub const BASE_MAX_STRUCTURE_TIER: u32 = 4;
/// The ceiling once the owning corporation has researched this structure's
/// Tier-IV/V unlock (`UnlockStructureTier` >= 4 for the kind): the two
/// superlinear prize tiers (5, 6 in `production::TIER_THROUGHPUT`) open up.
/// Tunable.
pub const RESEARCHED_MAX_STRUCTURE_TIER: u32 = 6;

/// The highest tier a structure of `kind` may be raised to. One shared gate for
/// every StructureKind (extraction / processing / habitat / shipyard / …):
/// `research_unlocked_tier` is the best tier this owner's corporation has unlocked
/// for the kind (0 = none, from `research::unlocked_structure_tier`). Without
/// that Tier-IV/V unlock the ceiling is [`BASE_MAX_STRUCTURE_TIER`]; with it,
/// the prize tiers open to [`RESEARCHED_MAX_STRUCTURE_TIER`]. The `kind` arg is
/// used to keep gated structures locked until their first research unlock.
pub fn max_buildable_tier(kind: StructureKind, research_unlocked_tier: u32) -> u32 {
    if kind == StructureKind::ServiceBerth { return 1; }
    // §orbital-defense: each station level is its own research unlock; the
    // legacy platform is no longer built.
    if kind == StructureKind::OrbitalDefense { return research_unlocked_tier.min(crate::station::MAX_LEVEL); }
    if kind == StructureKind::DefensePlatform { return 0; }
    if kind == StructureKind::CommandCenter { return if research_unlocked_tier >= 3 { 3 } else { 2 }; }
    if kind.research_prerequisite().is_some() && research_unlocked_tier == 0 {
        return 0;
    }
    if research_unlocked_tier >= BASE_MAX_STRUCTURE_TIER {
        RESEARCHED_MAX_STRUCTURE_TIER
    } else {
        BASE_MAX_STRUCTURE_TIER
    }
}

// §economy Part 3: EXTRACTOR_RICHNESS_MULT is RETIRED — extraction runs the
// same factor chain as all industry (`production::tier_throughput` on the
// structure tier, × staffing × skill × food), not a compounding multiplier.

// --- DEVELOPMENT SLOTS (§buildings step 1) ----------------------------------
// Every development BUILT (each Mining Complex/Orbital Warehouse/Shipyard tier) consumes ONE slot
// of the system's budget; ships are units, not developments, and consume none.
// Scarcity is the point: maxing Extractors crowds out warehousing/Shipyard, so systems
// must SPECIALIZE ("this one's my extraction colony, THAT one's my shipyard").
// The budget itself derives from geology — see `StarSystem::dev_slots`.

/// Slot budget for a 1-deposit system; each extra deposit adds one slot.
pub const DEV_SLOTS_BASE: u32 = 3;
/// Hard ceiling on any system's slot budget (3-deposit frontier systems hit it).
pub const DEV_SLOTS_MAX: u32 = 5;

// --- STORAGE CAPS (§buildings step 2) ----------------------------------------
// A system's stockpile has a TOTAL capacity (summed across commodities). NEW
// inflow (production accrual, seeds, deliveries) is capped — production simply
// IDLES at the cap; nothing already stored is ever destroyed (async-fair, and
// oversize pre-cap stockpiles are grandfathered). Capacity belongs to actual
// structures, not an invisible allowance added on top of the founding Warehouse.

/// Warehouse I's capacity; preserves the former opening allowance. Tunable.
pub const STORAGE_WAREHOUSE_INITIAL: f64 = 700.0;
/// Each additional ground Warehouse tier adds this much (I–VI). Tunable.
pub const STORAGE_PER_WAREHOUSE_TIER: f64 = 400.0;
/// Late-game orbital bulk storage, additional to ground warehouses. Tunable.
pub const STORAGE_PER_ORBITAL_WAREHOUSE_TIER: f64 = 2_000.0;

pub fn warehouse_capacity(tier: u32) -> f64 {
    if tier == 0 { 0.0 } else {
        STORAGE_WAREHOUSE_INITIAL + STORAGE_PER_WAREHOUSE_TIER * (tier - 1) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_and_orbital_storage_have_separate_progression() {
        use crate::research::{self, ResearchState};
        let mut research = ResearchState::default();
        assert_eq!(warehouse_capacity(0), 0.0);
        for (tier, cap) in [700.0, 1100.0, 1500.0, 1900.0, 2300.0, 2700.0].into_iter().enumerate() {
            assert_eq!(warehouse_capacity(tier as u32 + 1), cap);
        }
        let cap = |state: &ResearchState, kind| max_buildable_tier(kind,
            research::unlocked_structure_tier(state, kind));
        assert_eq!(cap(&research, StructureKind::Warehouse), 4);
        assert_eq!(cap(&research, StructureKind::OrbitalWarehouse), 0);
        research.completed.insert("mat_foundry_iv_arcology_frames".into());
        assert_eq!(cap(&research, StructureKind::Warehouse), 6);
        assert_eq!(cap(&research, StructureKind::OrbitalWarehouse), 0);
        research.completed.insert("mat_foundry_iv_orbital_yards".into());
        assert_eq!(cap(&research, StructureKind::OrbitalWarehouse), 4);
        assert!(STORAGE_PER_ORBITAL_WAREHOUSE_TIER > STORAGE_PER_WAREHOUSE_TIER);
    }

    #[test]
    fn structures_have_research_unlocks_without_locking_the_founding_loop() {
        use StructureKind::*;
        use crate::research::{self, Effect, ResearchState};
        // §gaia: Living Terraces need no research; a Gaia world is their gate.
        let starter = [MiningComplex, Bioharvester, Agroplex, Shipyard, Habitat,
            Warehouse, Academy, LivingTerraces, PowerPlant, UraniumMill];
        let gated = [
            (VolatileHarvester, "prop_bunkerage"), (FuelRefinery, "prop_bunkerage"),
            (Smelter, "mat_enrichment"), (ChemicalWorks, "mat_enrichment"),
            (ElectronicsFabricator, "comp_signal_libraries"),
            (MachineWorks, "mat_autoforges"),
            (ArmamentsComplex, "weap_munitions_lines"),
            (CompositeWorks, "mat_prefab_construction"), (HullFabricator, "mat_prefab_construction"),
            (PrecisionWorks, "mat_autoforges"), (DriveWorks, crate::power::FUSION_RESEARCH),
            (AntimatterPlant, "mat_foundry_vi_antimatter_synthesis"),
            (NavalDrydock, "mat_autoforges"), (CapitalSlipway, "hull_line_vii_dreadnought"),
            (OrdnanceFoundry, "hull_drydock_efficiency"), (SensorArray, "comp_sensor_gain"),
            (OrbitalWarehouse, "mat_foundry_iv_orbital_yards"),
            (Garrison, "weap_fire_control"),
        ];
        // The legacy platform is no longer built; the station climbs its own
        // research ladder (checked below).
        assert_eq!(starter.len() + gated.len() + 4, StructureKind::ALL.len());
        assert_eq!(DefensePlatform.research_prerequisite(), Some("weap_fire_control"));
        assert_eq!(max_buildable_tier(DefensePlatform, 6), 0, "legacy platforms are never built");
        for level in crate::station::LEVELS.iter() {
            let programme = research::programme(level.research).expect("real research");
            assert!(!programme.hidden && programme.effects.contains(&Effect::UnlockStructureTier(OrbitalDefense, level.level)),
                "{}", level.name);
            assert_eq!(max_buildable_tier(OrbitalDefense, level.level), level.level);
        }
        assert_eq!(max_buildable_tier(OrbitalDefense, 0), 0);
        assert_eq!(ServiceBerth.research_prerequisite(), None);
        assert_eq!(max_buildable_tier(ServiceBerth, 6), 1, "attachment has only one tier");
        for kind in starter {
            assert_eq!(kind.research_prerequisite(), None, "{kind:?}: no bootstrap loop");
            assert_eq!(max_buildable_tier(kind, 0), BASE_MAX_STRUCTURE_TIER);
        }
        for (kind, id) in gated {
            assert_eq!(kind.research_prerequisite(), Some(id), "{kind:?}");
            let programme = research::programme(id).expect("real research, not a client-only gate");
            assert!(!programme.hidden, "{id} must be researchable");
            assert!(programme.effects.contains(&Effect::UnlockStructureTier(kind, 1)));
            assert_eq!(max_buildable_tier(kind, 0), 0, "{kind:?} starts locked");
            let mut state = ResearchState::default();
            state.completed.insert(id.into());
            assert_eq!(max_buildable_tier(kind, research::unlocked_structure_tier(&state, kind)),
                BASE_MAX_STRUCTURE_TIER, "{kind:?}: completion unlocks the ordinary tier ladder");
        }
    }

    /// The Depot → Orbital Warehouse rename keeps OLD snapshots parsing: the
    /// legacy `"depot"` slug still deserialises onto the renamed variant (in the
    /// structures map, in queued `Upgrade` build jobs, and in old client commands),
    /// while the new slug round-trips.
    #[test]
    fn depot_alias_keeps_old_snapshots_loading() {
        let old: StructureKind = serde_json::from_str("\"depot\"").unwrap();
        assert_eq!(old, StructureKind::OrbitalWarehouse);
        let new = serde_json::to_string(&StructureKind::OrbitalWarehouse).unwrap();
        assert_eq!(new, "\"orbital_warehouse\"");
        assert_eq!(
            serde_json::from_str::<StructureKind>(&new).unwrap(),
            StructureKind::OrbitalWarehouse
        );
        // A build job queued under the old slug completes as the renamed structure.
        let job: BuildKind =
            serde_json::from_str(r#"{"kind":"upgrade","upgrade":"depot"}"#).unwrap();
        assert_eq!(
            job,
            BuildKind::Upgrade {
                upgrade: StructureKind::OrbitalWarehouse
            }
        );
        // …and the slug() helper agrees with what serde writes.
        assert_eq!(StructureKind::OrbitalWarehouse.slug(), "orbital_warehouse");
    }

    /// §ladder B6.1 — THE EFFICIENCY INVARIANT (load-bearing): combat weight
    /// per Armaments spent PEAKS at Destroyer-or-Cruiser, and the Titan is the
    /// least efficient spend. Capitals buy presence and role, never efficiency
    /// — enforced, not hoped. (Battle rules v12 raised the Dreadnought's
    /// broadside to 15 so it holds its rung between Battleship and Titan in
    /// battle; that lifts it just past the Battleship here, still under the
    /// peak.)
    #[test]
    fn capital_efficiency_peaks_at_destroyer_or_cruiser_and_declines_up() {
        let arm = |k: ShipKind| -> f64 {
            recipe_for(BuildKind::Ship { ship: k })
                .costs
                .iter()
                .find(|(c, _)| *c == Commodity::Armaments)
                .map(|(_, n)| *n)
                .expect("every warship recipe carries Armaments")
        };
        let eff = |k: ShipKind| (k.attack_weight() + k.defense_weight()) / arm(k);
        let d = eff(ShipKind::Destroyer);
        let c = eff(ShipKind::Cruiser);
        let b = eff(ShipKind::Battleship);
        let n = eff(ShipKind::Dreadnought);
        let t = eff(ShipKind::Titan);
        let peak = d.max(c);
        assert!(
            peak >= b && peak >= n && peak >= t,
            "the ladder peaks at Destroyer/Cruiser"
        );
        assert!(
            b > t && n > t,
            "the Titan is the least efficient capital ({b:.4}, {n:.4} > {t:.4})"
        );
        assert!(
            peak > t,
            "a Titan is the LEAST efficient Armaments spend on the ladder"
        );
    }

    #[test]
    fn capital_recipes_and_gates_climb_the_ladder() {
        // §yards: the capital ladder now climbs across TWO yards — the line of
        // battle at the Drydock (1/2/3), the super-capitals at the Slipway (1/2)
        // — instead of one Shipyard running to tier 6. Build times 8h → 8d,
        // strictly rising, are unchanged.
        assert_eq!(
            yard_for(ShipKind::Destroyer),
            (StructureKind::NavalDrydock, 1)
        );
        assert_eq!(
            yard_for(ShipKind::Cruiser),
            (StructureKind::NavalDrydock, 2)
        );
        assert_eq!(
            yard_for(ShipKind::Battleship),
            (StructureKind::NavalDrydock, 3)
        );
        assert_eq!(
            yard_for(ShipKind::Dreadnought),
            (StructureKind::CapitalSlipway, 1)
        );
        assert_eq!(
            yard_for(ShipKind::Titan),
            (StructureKind::CapitalSlipway, 2)
        );
        let ticks = |k: ShipKind| recipe_for(BuildKind::Ship { ship: k }).build_ticks;
        assert_eq!(
            ticks(ShipKind::Destroyer),
            8 * 3600 * HZ,
            "a Destroyer takes 8 hours"
        );
        assert_eq!(
            ticks(ShipKind::Titan),
            192 * 3600 * HZ,
            "a Titan takes 8 days — a season event"
        );
        assert!(
            ticks(ShipKind::Destroyer) < ticks(ShipKind::Cruiser)
                && ticks(ShipKind::Cruiser) < ticks(ShipKind::Battleship)
                && ticks(ShipKind::Battleship) < ticks(ShipKind::Dreadnought)
                && ticks(ShipKind::Dreadnought) < ticks(ShipKind::Titan)
        );
        // Rare-Elements enters at Cruiser and climbs steeply (the capital economy).
        let re = |k: ShipKind| {
            recipe_for(BuildKind::Ship { ship: k })
                .costs
                .iter()
                .find(|(c, _)| *c == Commodity::RareElements)
                .map(|(_, n)| *n)
                .unwrap_or(0.0)
        };
        assert_eq!(re(ShipKind::Destroyer), 0.0);
        assert!(re(ShipKind::Cruiser) > 0.0 && re(ShipKind::Titan) > re(ShipKind::Dreadnought));
    }

    #[test]
    fn tier_ceiling_gates_the_prize_tiers_behind_research() {
        // No unlock (0) → the base cap of 4, exactly where colonies sit today.
        assert_eq!(
            max_buildable_tier(StructureKind::MiningComplex, 0),
            BASE_MAX_STRUCTURE_TIER
        );
        assert_eq!(max_buildable_tier(StructureKind::Smelter, 0), 0);
        assert_eq!(max_buildable_tier(StructureKind::Smelter, 1), 4);
        // A Tier-IV or Tier-V unlock lifts the ceiling to 6 (the two superlinear
        // prize tiers) — for every kind, uniformly.
        assert_eq!(
            max_buildable_tier(StructureKind::MiningComplex, 4),
            RESEARCHED_MAX_STRUCTURE_TIER
        );
        assert_eq!(max_buildable_tier(StructureKind::Habitat, 5), 6);
        assert_eq!(max_buildable_tier(StructureKind::Shipyard, 4), 6);
        // Starter structures retain the free base. Gated structures first
        // require their initial research unlock, then use the same ladder.
        for kind in StructureKind::ALL {
            for unlocked in 0..=5u32 {
                if kind == StructureKind::ServiceBerth {
                    assert_eq!(max_buildable_tier(kind, unlocked), 1);
                } else if kind == StructureKind::DefensePlatform {
                    assert_eq!(max_buildable_tier(kind, unlocked), 0, "legacy");
                } else if kind == StructureKind::OrbitalDefense {
                    assert_eq!(max_buildable_tier(kind, unlocked), unlocked, "one level per unlock");
                } else if kind == StructureKind::CommandCenter {
                    assert_eq!(max_buildable_tier(kind, unlocked), if unlocked >= 3 { 3 } else { 2 });
                } else if kind.research_prerequisite().is_some() && unlocked == 0 {
                    assert_eq!(max_buildable_tier(kind, unlocked), 0);
                } else {
                    assert!(max_buildable_tier(kind, unlocked) >= BASE_MAX_STRUCTURE_TIER);
                }
            }
        }
    }

    /// §rare-materials: the belt pockets' materials have work, each swapped in
    /// at equal base value. Iridium goes into the ships of the line and Drive
    /// Assemblies; Exotic Matter into the special modules and Tier V+ research.
    #[test]
    fn rare_materials_go_into_capitals_drives_special_modules_and_top_research() {
        use crate::research::Field;
        use Commodity as C;
        let has = |costs: &[(C, f64)], good| costs.iter().any(|(g, n)| *g == good && *n > 0.0);
        for ship in crate::ship::ALL_SHIP_KINDS.into_iter().filter(|k| k.is_buildable()) {
            let costs = recipe_for(BuildKind::Ship { ship }).costs;
            assert_eq!(has(costs, C::Iridium), crate::ship::is_siege_anchor(ship), "{ship:?}");
        }
        let drive = crate::production::converter_for(StructureKind::DriveWorks).unwrap();
        assert!(has(drive.inputs, C::Iridium));
        let value = |b: &[(C, f64)]| b.iter().map(|(c, n)| crate::market::base_price(*c) * n).sum::<f64>();
        assert!((value(drive.inputs) - 165.0).abs() < 1e-9, "the Drive Works basket, without its furnace fuel");
        for module in [ModuleKind::PrismaticLance, ModuleKind::InterdictionField] {
            assert!(has(module_recipe(module).costs, C::ExoticMatter), "{module:?}");
        }
        for tier in 1..=8u8 {
            for field in [Field::Propulsion, Field::Materials, Field::Computation, Field::Weapons, Field::Hulls, Field::Life] {
                let basket = crate::research::basket(field, tier);
                assert_eq!(has(&basket, C::ExoticMatter), tier >= 5, "{field:?} {tier}");
                let rare: Vec<_> = basket.iter().copied().filter(|(c, _)| matches!(c, C::RareElements | C::ExoticMatter)).collect();
                let before = if tier >= 3 { 100.0 * 0.003 * (tier - 2) as f64 } else { 0.0 };
                assert!((value(&rare) - before).abs() < 1e-9, "{field:?} {tier}: equal value");
            }
        }
    }

    #[test]
    fn component_chains_feed_heavy_hulls_but_leave_opening_hulls_alone() {
        use Commodity as C;
        // §fuel-overhaul: every warp hull carries the Authority's Warp Core, so
        // the opening economy's components are the colony-made chains.
        let components = [C::Composites, C::HullSections, C::PrecisionComponents];
        for ship in [ShipKind::Raider, ShipKind::Scout, ShipKind::Corvette, ShipKind::Convoy,
            ShipKind::Colony, ShipKind::Builder, ShipKind::Transport] {
            assert!(recipe_for(BuildKind::Ship { ship }).costs.iter()
                .all(|(good, _)| !components.contains(good)), "{ship:?} stays on the opening economy");
        }
        // Substitutions create demand for a supply network, not a hidden large
        // price increase on top of the same old recipe. Base values stay ±1%.
        // §fuel-overhaul: the hulls lost their fuel and the Destroyer gained two
        // Warp Cores.
        for (ship, previous) in [(ShipKind::Destroyer, 5756.0), (ShipKind::Cruiser, 10923.0),
            (ShipKind::Battleship, 23910.0), (ShipKind::Dreadnought, 49265.0), (ShipKind::Titan, 106970.0)] {
            let costs = recipe_for(BuildKind::Ship { ship }).costs;
            assert!(costs.iter().any(|(good, units)| *good == C::HullSections && *units > 0.0));
            let core = if ship == ShipKind::Destroyer { C::PrecisionComponents } else { C::WarpCore };
            assert!(costs.iter().any(|(good, units)| *good == core && *units > 0.0));
            let value: f64 = costs.iter().map(|(good, units)| crate::market::base_price(*good) * units).sum();
            assert!((value / previous - 1.0).abs() < 0.01, "{ship:?}: {value} vs {previous}");
        }
    }
}

//! Construction: spending resources to GROW (the Travian-style growth sink, §step1).
//!
//! Ships and a simple system upgrade ("Extractor") are built by deducting a fixed
//! RECIPE of commodities from the owning system's stockpile and enqueuing a build
//! job that completes after a fixed duration — server-driven, online or off. This
//! is where **Ore and Alloys get their meaning**: they are what you BUILD WITH, not
//! just goods to sell. All recipes/durations are `const` → deterministic; balance is
//! not the goal, a working "production → build" loop is.

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};
use crate::module::ModuleKind;
use crate::ship::ShipKind;

/// What a build job produces on completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BuildKind {
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
    /// MetallicOre + Fuel → Alloys.
    Smelter,
    /// RareElements + Silicates → Electronics.
    ElectronicsFabricator,
    /// Volatiles + Biomass → Polymers.
    ChemicalWorks,
    /// Volatiles → Fuel (the old Refinery, renamed).
    #[serde(alias = "refinery")]
    FuelRefinery,
    /// Biomass → Provisions.
    Agroplex,
    // ── Advanced industry (Industrial slots) ────────────────────────────────
    /// Alloys + Electronics + Fuel → Machinery.
    MachineWorks,
    /// Alloys + Electronics + Polymers → Armaments.
    ArmamentsComplex,
    /// GATES the LIGHT hulls (`yard_for`): Convoy/Scout/Colony at tier 1,
    /// Raider/Corvette at tier 2. Its tier is also its SLIPWAY COUNT — how many
    /// of those hulls it can lay down at once (§yards M1).
    Shipyard,
    /// §yards: the LINE-WARSHIP yard — Destroyer/Cruiser/Battleship. Needs a
    /// Shipyard ≥ 2 on the same system ([`yard_prereq`]): you learn to build
    /// before you build big. Its own tier is its own slip count, independent of
    /// the Shipyard's, so a yard world can lay light and line hulls in parallel.
    NavalDrydock,
    /// §yards: the SUPER-CAPITAL yard — Dreadnought/Titan. Needs a Naval Drydock
    /// ≥ 3 here. Deliberately the deepest prerequisite chain in the game: a
    /// capital slipway is a season's investment, and (unlike a home) it sits on
    /// capturable ground.
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
    /// Raises the system's storage cap — and ONLY that. (Renamed from `Depot`; the
    /// alias keeps old snapshots, in-flight build jobs, and old client commands
    /// parsing, exactly as `extractor` does for MiningComplex.)
    #[serde(alias = "depot")]
    OrbitalWarehouse,
    /// Standing sensor bubble (unchanged semantics).
    SensorArray,
    /// Static defense (combat semantics + `defense_pool` untouched).
    DefensePlatform,
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
}

impl StructureKind {
    /// Every kind, in display order.
    pub const ALL: [StructureKind; 20] = [
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
        StructureKind::Shipyard,
        StructureKind::NavalDrydock,
        StructureKind::CapitalSlipway,
        StructureKind::OrdnanceFoundry,
        StructureKind::Habitat,
        StructureKind::OrbitalWarehouse,
        StructureKind::SensorArray,
        StructureKind::DefensePlatform,
        StructureKind::Academy,
        StructureKind::Garrison,
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
            | StructureKind::OrbitalWarehouse
            | StructureKind::SensorArray
            | StructureKind::DefensePlatform
            | StructureKind::Academy
            // §ground: the garrison is civic infrastructure — a colony defends
            // itself out of the same budget that houses and feeds it.
            | StructureKind::Garrison => SlotPool::Infrastructure,
        }
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
            StructureKind::Shipyard => "shipyard",
            StructureKind::NavalDrydock => "naval_drydock",
            StructureKind::CapitalSlipway => "capital_slipway",
            StructureKind::OrdnanceFoundry => "ordnance_foundry",
            StructureKind::Habitat => "habitat",
            StructureKind::OrbitalWarehouse => "orbital_warehouse",
            StructureKind::SensorArray => "sensor_array",
            StructureKind::DefensePlatform => "defense_platform",
            StructureKind::Academy => "academy",
            StructureKind::Garrison => "garrison",
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
            StructureKind::FuelRefinery => "Fuel Refinery",
            StructureKind::Agroplex => "Agroplex",
            StructureKind::MachineWorks => "Machine Works",
            StructureKind::ArmamentsComplex => "Armaments Complex",
            StructureKind::Shipyard => "Shipyard",
            StructureKind::NavalDrydock => "Naval Drydock",
            StructureKind::CapitalSlipway => "Capital Slipway",
            StructureKind::OrdnanceFoundry => "Ordnance Foundry",
            StructureKind::Habitat => "Habitat",
            StructureKind::OrbitalWarehouse => "Orbital Warehouse",
            StructureKind::SensorArray => "Sensor Array",
            StructureKind::DefensePlatform => "Defense Platform",
            StructureKind::Academy => "Academy",
            StructureKind::Garrison => "Garrison",
        }
    }
}

/// A queued construction job, resolved when `complete_tick` is reached. Lives on
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
    /// §bodies: the BODY this job builds on (structures) or displays at (ship
    /// jobs at the yard's body, courses at the Academy's). `default` 0 lets
    /// pre-bodies snapshots parse; migration re-sites in-flight jobs.
    #[serde(default)]
    pub body_id: u32,
    pub what: BuildKind,
    /// Absolute sim tick of completion.
    pub complete_tick: u64,
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

/// §modules Part B4: a queued REFIT — `n` ships of `ship` were pulled OUT of a
/// docked fleet (so they're safely out of combat while in the yard), their fit
/// swapped to `to`, and they rejoin on completion. The module delta was already
/// reconciled against the system ledger at ENQUEUE (added modules debited, removed
/// modules returned), so completion is pure: re-add the fitted hulls. Rides its
/// OWN small queue parallel to `build_queue`; `#[serde(default)]` empties on the
/// World = zero migration. Ownership of the hulls follows the FLEET OWNER, never
/// the yard's system — so a capture mid-refit still returns them to their owner.
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
    /// Absolute sim tick of completion.
    pub complete_tick: u64,
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

// Note on the distribution: Ore (core) and Alloys (frontier) rarely co-occur in a
// single system, so a recipe needing BOTH can only be built by SHIPPING materials
// between systems (logistics depth). The entry builds therefore use Ore ALONE (any
// ore system — incl. your home — can build them); the advanced Raider needs frontier
// **Alloys + Fuel** (gather them across systems, the §step1 "spread of systems matters").

/// Convoy (bulk hauler): plain **Ore** — cheap, the workhorse you build at home.
pub const CONVOY_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 25.0),
        (Commodity::Machinery, 10.0),
        (Commodity::Polymers, 10.0),
    ],
    build_ticks: 12 * HZ,
};
/// Raider: **Alloys** + **Fuel** — costlier, needs the good frontier materials.
pub const RAIDER_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 20.0),
        (Commodity::Electronics, 12.0),
        (Commodity::Armaments, 15.0),
        (Commodity::Fuel, 10.0),
    ],
    build_ticks: 10 * HZ,
};
/// Scout: cheap **Ore + Fuel** — the entry unit, buildable at the home turn one
/// (cheap enough that a caught scout is an acceptable loss).
pub const SCOUT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 15.0),
        (Commodity::Electronics, 8.0),
        (Commodity::Fuel, 8.0),
    ],
    build_ticks: 8 * HZ,
};
/// Corvette: **Ore + Alloys** — the dedicated defender; military industry.
pub const CORVETTE_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 25.0),
        (Commodity::Electronics, 12.0),
        (Commodity::Armaments, 12.0),
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
        (Commodity::Fuel, 15.0),
    ],
    build_ticks: 30 * HZ,
};
// §ladder: CAPITAL recipes — Rare-Elements-and-Machinery-heavy by design (the
// deep-crust economy is the capital economy), and build TIMES measured in
// hours-to-days: a capital under construction is a season event and a siege
// target. Combat weight per Armaments spent peaks at Destroyer/Cruiser and
// declines up the ladder (the efficiency invariant, pinned by test). Tunable.
const HOUR_TICKS: u64 = 3600 * HZ;
pub const DESTROYER_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 60.0),
        (Commodity::Electronics, 25.0),
        (Commodity::Armaments, 30.0),
        (Commodity::Machinery, 20.0),
        (Commodity::Fuel, 15.0),
    ],
    build_ticks: 8 * HOUR_TICKS,
};
pub const CRUISER_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 120.0),
        (Commodity::Electronics, 50.0),
        (Commodity::Armaments, 55.0),
        (Commodity::Machinery, 45.0),
        (Commodity::RareElements, 12.0),
        (Commodity::Fuel, 30.0),
    ],
    build_ticks: 18 * HOUR_TICKS,
};
pub const BATTLESHIP_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 260.0),
        (Commodity::Electronics, 100.0),
        (Commodity::Armaments, 120.0),
        (Commodity::Machinery, 100.0),
        (Commodity::RareElements, 35.0),
        (Commodity::Fuel, 60.0),
    ],
    build_ticks: 48 * HOUR_TICKS,
};
pub const DREADNOUGHT_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 520.0),
        (Commodity::Electronics, 210.0),
        (Commodity::Armaments, 230.0),
        (Commodity::Machinery, 220.0),
        (Commodity::RareElements, 90.0),
        (Commodity::Fuel, 120.0),
    ],
    build_ticks: 96 * HOUR_TICKS,
};
pub const TITAN_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 1100.0),
        (Commodity::Electronics, 450.0),
        (Commodity::Armaments, 480.0),
        (Commodity::Machinery, 500.0),
        (Commodity::RareElements, 220.0),
        (Commodity::Fuel, 260.0),
    ],
    build_ticks: 192 * HOUR_TICKS,
};
// §economy Part 5: the FULL industrial-web cost table (design doc). Everything
// advanced needs MACHINERY, and early Machinery comes from Sol — the intended
// loop is extract → sell raws → buy Machinery → build industry → make your own.
// they need Machinery/Electronics, purchasable at the hub (Sol's off-map
// industry lists all 12 from day one).
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
        (Commodity::RareElements, 15.0),
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
pub const ORBITAL_WAREHOUSE_RECIPE: Recipe = Recipe {
    costs: &[(Commodity::Alloys, 30.0), (Commodity::Machinery, 8.0)],
    build_ticks: 15 * HZ,
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
        (Commodity::Fuel, 15.0),
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

/// The recipe for one module of `kind`.
pub fn module_recipe(kind: ModuleKind) -> &'static Recipe {
    match kind {
        ModuleKind::MassDriver => &MASS_DRIVER_RECIPE,
        ModuleKind::TorpedoRack => &TORPEDO_RACK_RECIPE,
        ModuleKind::PointDefenseScreen => &POINT_DEFENSE_RECIPE,
        ModuleKind::ReflectivePlating => &REFLECTIVE_PLATING_RECIPE,
        ModuleKind::WhippleArmor => &WHIPPLE_ARMOR_RECIPE,
    }
}

/// A SENSOR is the expensive one: it is an instrument, and it is what lets you
/// see a rival coming before they arrive.
static DEEP_SPACE_SENSOR_RECIPE: Recipe = Recipe {
    costs: &[
        (Commodity::Alloys, 60.0),
        (Commodity::Electronics, 120.0),
        (Commodity::Fuel, 40.0),
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
        (Commodity::Fuel, 20.0),
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
        BuildKind::Module { module } => module_recipe(module),
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
            StructureKind::Shipyard => &SHIPYARD_RECIPE,
            StructureKind::NavalDrydock => &NAVAL_DRYDOCK_RECIPE,
            StructureKind::CapitalSlipway => &CAPITAL_SLIPWAY_RECIPE,
            StructureKind::OrdnanceFoundry => &ORDNANCE_FOUNDRY_RECIPE,
            StructureKind::Habitat => &HABITAT_RECIPE,
            StructureKind::OrbitalWarehouse => &ORBITAL_WAREHOUSE_RECIPE,
            StructureKind::SensorArray => &SENSOR_ARRAY_RECIPE,
            StructureKind::DefensePlatform => &DEFENSE_PLATFORM_RECIPE,
            StructureKind::Academy => &ACADEMY_RECIPE,
            StructureKind::Garrison => &GARRISON_RECIPE,
        },
    }
}

// --- FUEL REFINERY (§buildings step 3b → §economy Part 3) -------------------------
// The old REFINERY_RATE/YIELD pair is RETIRED: the Fuel Refinery is one row of
// the data-driven converter table now (`production::CONVERTERS` — 1.0 Volatiles
// per Fuel at 0.40/s), staffed and factor-chained like all industry.

// --- HABITAT (§buildings step 3a → §economy Part 2) ------------------------------
// The old per-tier upkeep + fed-boost pair is RETIRED: a Habitat now houses
// POPULATION (capacity `colony::POP_CAP_PER_HABITAT_TIER` per tier), and it is
// the population that eats (`colony::PROVISIONS_PER_MILLION_PER_S`), works, and
// unlocks slots. All colony-life tunables live in `crate::colony`.

/// §syndicates Part 3: Provisions consumed per second PER SHIP of an ALLY GARRISON
/// stationed at a host system, drawn from the HOST's own stockpile each tick.
/// Hosting a coalition shield means FEEDING it — a cut supply line UNFEEDS the
/// garrison (its defense contribution suspends until fed; nothing is destroyed).
/// Sized in the Habitat-upkeep ballpark so a modest garrison is affordable but a
/// large one strains a small host. Playtest placeholder. Tunable.
pub const GARRISON_UPKEEP_PER_SHIP: f64 = 0.05;

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
pub const POP_DEVELOPED: f64 = 3.0;
/// Population (millions) at which a colony counts as MAJOR — the third
/// Industrial slot. Tunable.
pub const POP_MAJOR: f64 = 8.0;

/// The population tier: 0 below `POP_DEVELOPED`, 1 from there, 2 at `POP_MAJOR`.
/// Population only ever grows (§economy Part 2), so pools never shrink under a
/// player — no un-build edge case.
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
        // Light + civilian — the Shipyard, exactly as before.
        ShipKind::Builder => (StructureKind::Shipyard, 1),
        ShipKind::Convoy => (StructureKind::Shipyard, 1),
        ShipKind::Scout => (StructureKind::Shipyard, 1),
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

/// The highest tier ANY structure is freely buildable to (cost + slots
/// permitting) with no research. This is where an unresearched colony tops out —
/// exactly where every colony tops out today. Tunable.
pub const BASE_MAX_STRUCTURE_TIER: u32 = 4;
/// The ceiling once the owning syndicate has researched this structure's
/// Tier-IV/V unlock (any `UnlockStructureTier` effect for the kind): the two
/// superlinear prize tiers (5, 6 in `production::TIER_THROUGHPUT`) open up.
/// Tunable.
pub const RESEARCHED_MAX_STRUCTURE_TIER: u32 = 6;

/// The highest tier a structure of `kind` may be raised to. One shared gate for
/// every StructureKind (extraction / processing / habitat / shipyard / …):
/// `research_unlocked_tier` is the best tier this owner's syndicate has unlocked
/// for the kind (0 = none, from `research::unlocked_structure_tier`). Without
/// that Tier-IV/V unlock the ceiling is [`BASE_MAX_STRUCTURE_TIER`]; with it,
/// the prize tiers open to [`RESEARCHED_MAX_STRUCTURE_TIER`]. The `kind` arg is
/// carried for future per-kind ceilings; today the gate is uniform.
pub fn max_buildable_tier(kind: StructureKind, research_unlocked_tier: u32) -> u32 {
    let _ = kind; // uniform across kinds today — wired once, shared by all
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
// oversize pre-cap stockpiles are grandfathered). Orbital Warehouse tiers raise the cap.

/// Base storage capacity of every system (no Orbital Warehouse). Chosen comfortably above the
/// home's 300-unit fuel seed so a fresh corporation starts with headroom, while
/// still filling within minutes of idle production — the "ship it or lose the
/// flow" pressure that gives standing orders a real job. Tunable.
pub const STORAGE_BASE_CAP: f64 = 700.0;
/// Extra capacity per Orbital Warehouse tier. Tunable.
pub const STORAGE_PER_WAREHOUSE_TIER: f64 = 400.0;

#[cfg(test)]
mod tests {
    use super::*;

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
    /// per Armaments spent PEAKS at Destroyer-or-Cruiser and STRICTLY DECLINES
    /// Battleship → Dreadnought → Titan. Capitals buy presence and role, never
    /// efficiency — enforced, not hoped.
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
            b > n && n > t,
            "efficiency strictly declines Battleship → Dreadnought → Titan ({b:.4} > {n:.4} > {t:.4})"
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
        assert_eq!(max_buildable_tier(StructureKind::Smelter, 0), 4);
        // A Tier-IV or Tier-V unlock lifts the ceiling to 6 (the two superlinear
        // prize tiers) — for every kind, uniformly.
        assert_eq!(
            max_buildable_tier(StructureKind::MiningComplex, 4),
            RESEARCHED_MAX_STRUCTURE_TIER
        );
        assert_eq!(max_buildable_tier(StructureKind::Habitat, 5), 6);
        assert_eq!(max_buildable_tier(StructureKind::Shipyard, 4), 6);
        // The prize only ever RAISES the ceiling — never below the free base.
        for kind in StructureKind::ALL {
            for unlocked in 0..=5u32 {
                assert!(max_buildable_tier(kind, unlocked) >= BASE_MAX_STRUCTURE_TIER);
            }
        }
    }
}

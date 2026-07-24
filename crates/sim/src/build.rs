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
use crate::module::ModuleKind;
use crate::ids::{EntityId, PlayerId};
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
    Train { specialist: crate::specialist::SpecialistKind },
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
    /// GATES ship construction (`required_shipyard_tier`), exactly as before.
    Shipyard,
    // ── Support (Infrastructure slots) ──────────────────────────────────────
    /// Population capacity + workforce slots (§economy Part 2 — the boost/upkeep
    /// semantics retire; capacity is the Habitat's value now).
    Habitat,
    /// Raises the storage cap (unchanged semantics).
    Depot,
    /// Standing sensor bubble (unchanged semantics).
    SensorArray,
    /// Static defense (combat semantics + `defense_pool` untouched).
    DefensePlatform,
    /// Trains specialists locally (§economy Part 4) — endogenous supply so Sol
    /// never stays a permanent monopoly.
    Academy,
}

impl StructureKind {
    /// Every kind, in display order.
    pub const ALL: [StructureKind; 16] = [
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
        StructureKind::Habitat,
        StructureKind::Depot,
        StructureKind::SensorArray,
        StructureKind::DefensePlatform,
        StructureKind::Academy,
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
            | StructureKind::Shipyard => SlotPool::Industrial,
            // §economy Part 3: the AGROPLEX is CIVIC — food security lives in
            // the Infrastructure pool (Habitat + Agroplex = a self-feeding
            // outpost on the base 2 slots, no industrial investment needed).
            // §industrial-headroom: the industrial base is now 2, so a fresh home
            // has a free industrial slot beyond the Shipyard's — a second industry
            // no longer waits on a DEVELOPED colony. The raider gate is purely the
            // Shipyard-tier-2 requirement now, not industrial-slot scarcity.
            StructureKind::Agroplex
            | StructureKind::Habitat
            | StructureKind::Depot
            | StructureKind::SensorArray
            | StructureKind::DefensePlatform
            | StructureKind::Academy => SlotPool::Infrastructure,
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
            StructureKind::Habitat => "habitat",
            StructureKind::Depot => "depot",
            StructureKind::SensorArray => "sensor_array",
            StructureKind::DefensePlatform => "defense_platform",
            StructureKind::Academy => "academy",
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
            StructureKind::Habitat => "Habitat",
            StructureKind::Depot => "Depot",
            StructureKind::SensorArray => "Sensor Array",
            StructureKind::DefensePlatform => "Defense Platform",
            StructureKind::Academy => "Academy",
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
pub const CONVOY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 25.0), (Commodity::Machinery, 10.0), (Commodity::Polymers, 10.0)], build_ticks: 12 * HZ };
/// Raider: **Alloys** + **Fuel** — costlier, needs the good frontier materials.
pub const RAIDER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 20.0), (Commodity::Electronics, 12.0), (Commodity::Armaments, 15.0), (Commodity::Fuel, 10.0)], build_ticks: 10 * HZ };
/// Scout: cheap **Ore + Fuel** — the entry unit, buildable at the home turn one
/// (cheap enough that a caught scout is an acceptable loss).
pub const SCOUT_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 15.0), (Commodity::Electronics, 8.0), (Commodity::Fuel, 8.0)], build_ticks: 8 * HZ };
/// Corvette: **Ore + Alloys** — the dedicated defender; military industry.
pub const CORVETTE_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 25.0), (Commodity::Electronics, 12.0), (Commodity::Armaments, 12.0)], build_ticks: 14 * HZ };
/// Colony Ship: **Ore + Alloys + Provisions** (colonists eat) — absorbs the old
/// instant-claim economics into a physical, raidable investment (§ships part 3).
pub const COLONY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 45.0), (Commodity::Machinery, 15.0), (Commodity::Polymers, 20.0), (Commodity::Provisions, 30.0), (Commodity::Fuel, 15.0)], build_ticks: 30 * HZ };
// §ladder: CAPITAL recipes — Rare-Elements-and-Machinery-heavy by design (the
// deep-crust economy is the capital economy), and build TIMES measured in
// hours-to-days: a capital under construction is a season event and a siege
// target. Combat weight per Armaments spent peaks at Destroyer/Cruiser and
// declines up the ladder (the efficiency invariant, pinned by test). Tunable.
const HOUR_TICKS: u64 = 3600 * HZ;
pub const DESTROYER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 60.0), (Commodity::Electronics, 25.0), (Commodity::Armaments, 30.0), (Commodity::Machinery, 20.0), (Commodity::Fuel, 15.0)], build_ticks: 8 * HOUR_TICKS };
pub const CRUISER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 120.0), (Commodity::Electronics, 50.0), (Commodity::Armaments, 55.0), (Commodity::Machinery, 45.0), (Commodity::RareElements, 12.0), (Commodity::Fuel, 30.0)], build_ticks: 18 * HOUR_TICKS };
pub const BATTLESHIP_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 260.0), (Commodity::Electronics, 100.0), (Commodity::Armaments, 120.0), (Commodity::Machinery, 100.0), (Commodity::RareElements, 35.0), (Commodity::Fuel, 60.0)], build_ticks: 48 * HOUR_TICKS };
pub const DREADNOUGHT_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 520.0), (Commodity::Electronics, 210.0), (Commodity::Armaments, 230.0), (Commodity::Machinery, 220.0), (Commodity::RareElements, 90.0), (Commodity::Fuel, 120.0)], build_ticks: 96 * HOUR_TICKS };
pub const TITAN_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 1100.0), (Commodity::Electronics, 450.0), (Commodity::Armaments, 480.0), (Commodity::Machinery, 500.0), (Commodity::RareElements, 220.0), (Commodity::Fuel, 260.0)], build_ticks: 192 * HOUR_TICKS };
// §economy Part 5: the FULL industrial-web cost table (design doc). Everything
// advanced needs MACHINERY, and early Machinery comes from Sol — the intended
// loop is extract → sell raws → buy Machinery → build industry → make your own.
// they need Machinery/Electronics, purchasable at the hub (Sol's off-map
// industry lists all 12 from day one).
pub const MINING_COMPLEX_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 12.0), (Commodity::Alloys, 25.0)], build_ticks: 18 * HZ };
pub const VOLATILE_HARVESTER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 12.0), (Commodity::Alloys, 25.0)], build_ticks: 18 * HZ };
pub const BIOHARVESTER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 12.0), (Commodity::Alloys, 25.0)], build_ticks: 18 * HZ };
pub const SMELTER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)], build_ticks: 20 * HZ };
pub const ELECTRONICS_FABRICATOR_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 20.0), (Commodity::Silicates, 10.0)], build_ticks: 20 * HZ };
pub const CHEMICAL_WORKS_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)], build_ticks: 20 * HZ };
pub const FUEL_REFINERY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)], build_ticks: 20 * HZ };
pub const AGROPLEX_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 15.0), (Commodity::Alloys, 30.0)], build_ticks: 20 * HZ };
pub const MACHINE_WORKS_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 20.0), (Commodity::Alloys, 40.0), (Commodity::Electronics, 15.0)], build_ticks: 22 * HZ };
pub const ARMAMENTS_COMPLEX_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 20.0), (Commodity::Alloys, 40.0), (Commodity::Electronics, 15.0)], build_ticks: 22 * HZ };
pub const SHIPYARD_RECIPE: Recipe = Recipe { costs: &[(Commodity::Machinery, 20.0), (Commodity::Alloys, 40.0), (Commodity::Electronics, 15.0)], build_ticks: 20 * HZ };
pub const HABITAT_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 30.0), (Commodity::Polymers, 20.0), (Commodity::Machinery, 8.0)], build_ticks: 20 * HZ };
pub const DEPOT_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 30.0), (Commodity::Machinery, 8.0)], build_ticks: 15 * HZ };
pub const SENSOR_ARRAY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Electronics, 18.0), (Commodity::Machinery, 10.0)], build_ticks: 18 * HZ };
pub const DEFENSE_PLATFORM_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 35.0), (Commodity::Electronics, 15.0), (Commodity::Armaments, 15.0)], build_ticks: 22 * HZ };
pub const ACADEMY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 25.0), (Commodity::Electronics, 15.0), (Commodity::Provisions, 20.0)], build_ticks: 20 * HZ };

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
pub const MASS_DRIVER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Armaments, 8.0), (Commodity::Electronics, 4.0)], build_ticks: MODULE_BUILD_TICKS };
pub const TORPEDO_RACK_RECIPE: Recipe = Recipe { costs: &[(Commodity::Armaments, 10.0), (Commodity::Electronics, 6.0)], build_ticks: MODULE_BUILD_TICKS };
pub const POINT_DEFENSE_RECIPE: Recipe = Recipe { costs: &[(Commodity::Armaments, 6.0), (Commodity::Electronics, 8.0)], build_ticks: MODULE_BUILD_TICKS };
pub const REFLECTIVE_PLATING_RECIPE: Recipe = Recipe { costs: &[(Commodity::Armaments, 6.0), (Commodity::Silicates, 6.0), (Commodity::Electronics, 2.0)], build_ticks: MODULE_BUILD_TICKS };
pub const WHIPPLE_ARMOR_RECIPE: Recipe = Recipe { costs: &[(Commodity::Armaments, 12.0), (Commodity::Machinery, 2.0)], build_ticks: MODULE_BUILD_TICKS };

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

pub fn recipe_for(what: BuildKind) -> &'static Recipe {
    match what {
        BuildKind::Ship { ship: ShipKind::Convoy } => &CONVOY_RECIPE,
        BuildKind::Ship { ship: ShipKind::Raider } => &RAIDER_RECIPE,
        BuildKind::Ship { ship: ShipKind::Scout } => &SCOUT_RECIPE,
        BuildKind::Ship { ship: ShipKind::Corvette } => &CORVETTE_RECIPE,
        BuildKind::Ship { ship: ShipKind::Colony } => &COLONY_RECIPE,
        BuildKind::Ship { ship: ShipKind::Destroyer } => &DESTROYER_RECIPE,
        BuildKind::Ship { ship: ShipKind::Cruiser } => &CRUISER_RECIPE,
        BuildKind::Ship { ship: ShipKind::Battleship } => &BATTLESHIP_RECIPE,
        BuildKind::Ship { ship: ShipKind::Dreadnought } => &DREADNOUGHT_RECIPE,
        BuildKind::Ship { ship: ShipKind::Titan } => &TITAN_RECIPE,
        // The Authority Freighter (§TCA) is TCA-only and never buildable: the
        // `BuildShip` handler soft-rejects it (via `ShipKind::is_buildable`) BEFORE
        // any recipe lookup, and no `BuildJob` for one can ever be enqueued, so this
        // arm is genuinely unreachable — it exists only for match exhaustiveness.
        BuildKind::Ship { ship: ShipKind::Freighter } => {
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
            StructureKind::Habitat => &HABITAT_RECIPE,
            StructureKind::Depot => &DEPOT_RECIPE,
            StructureKind::SensorArray => &SENSOR_ARRAY_RECIPE,
            StructureKind::DefensePlatform => &DEFENSE_PLATFORM_RECIPE,
            StructureKind::Academy => &ACADEMY_RECIPE,
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
pub const SENSOR_ARRAY_BASE: f64 = 2200.0;
/// Extra radius per tier past the first (+40% of base) — a tier-2 array outsees
/// any ship. Tunable.
pub const SENSOR_ARRAY_PER_TIER: f64 = 880.0;

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

// --- SHIPYARD GATING (§buildings step 3) --------------------------------------

/// The Shipyard tier a system needs to build each ship kind: the workhorse
/// Convoy and the cheap Scout at tier 1, the advanced Raider only at tier 2
/// (military industry must be EARNED). Homes are seeded at tier 1, so convoys
/// AND scouts build turn one. Tunable.
pub fn required_shipyard_tier(kind: ShipKind) -> u32 {
    match kind {
        ShipKind::Convoy => 1,
        ShipKind::Raider => 2,
        ShipKind::Corvette => 2, // military industry, like the raider
        ShipKind::Colony => 1,   // civilian settlement — any yard
        ShipKind::Scout => 1,
        // §ladder: capital yards — tiers 5/6 are themselves research prizes
        // (Line VII/VIII grant the shipyard ceiling with the hull).
        ShipKind::Destroyer => 3,
        ShipKind::Cruiser => 4,
        ShipKind::Battleship => 4,
        ShipKind::Dreadnought => 5,
        ShipKind::Titan => 6,
        // §TCA: no shipyard tier can EVER build an Authority freighter — a
        // belt-and-suspenders backstop behind `ShipKind::is_buildable`.
        ShipKind::Freighter => u32::MAX,
    }
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
// Every development BUILT (each Extractor/Depot/Shipyard tier) consumes ONE slot
// of the system's budget; ships are units, not developments, and consume none.
// Scarcity is the point: maxing Extractors crowds out Depot/Shipyard, so systems
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
// oversize pre-cap stockpiles are grandfathered). Depot tiers raise the cap.

/// Base storage capacity of every system (no Depot). Chosen comfortably above the
/// home's 300-unit fuel seed so a fresh corporation starts with headroom, while
/// still filling within minutes of idle production — the "ship it or lose the
/// flow" pressure that gives standing orders a real job. Tunable.
pub const STORAGE_BASE_CAP: f64 = 700.0;
/// Extra capacity per Depot tier. Tunable.
pub const STORAGE_PER_DEPOT_TIER: f64 = 400.0;

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(peak >= b && peak >= n && peak >= t, "the ladder peaks at Destroyer/Cruiser");
        assert!(b > n && n > t, "efficiency strictly declines Battleship → Dreadnought → Titan ({b:.4} > {n:.4} > {t:.4})");
        assert!(peak > t, "a Titan is the LEAST efficient Armaments spend on the ladder");
    }

    #[test]
    fn capital_recipes_and_gates_climb_the_ladder() {
        // Shipyard gates 3/4/4/5/6 and build times 8h → 8d, strictly rising.
        assert_eq!(required_shipyard_tier(ShipKind::Destroyer), 3);
        assert_eq!(required_shipyard_tier(ShipKind::Cruiser), 4);
        assert_eq!(required_shipyard_tier(ShipKind::Battleship), 4);
        assert_eq!(required_shipyard_tier(ShipKind::Dreadnought), 5);
        assert_eq!(required_shipyard_tier(ShipKind::Titan), 6);
        let ticks = |k: ShipKind| recipe_for(BuildKind::Ship { ship: k }).build_ticks;
        assert_eq!(ticks(ShipKind::Destroyer), 8 * 3600 * HZ, "a Destroyer takes 8 hours");
        assert_eq!(ticks(ShipKind::Titan), 192 * 3600 * HZ, "a Titan takes 8 days — a season event");
        assert!(ticks(ShipKind::Destroyer) < ticks(ShipKind::Cruiser)
            && ticks(ShipKind::Cruiser) < ticks(ShipKind::Battleship)
            && ticks(ShipKind::Battleship) < ticks(ShipKind::Dreadnought)
            && ticks(ShipKind::Dreadnought) < ticks(ShipKind::Titan));
        // Rare-Elements enters at Cruiser and climbs steeply (the capital economy).
        let re = |k: ShipKind| recipe_for(BuildKind::Ship { ship: k }).costs.iter()
            .find(|(c, _)| *c == Commodity::RareElements).map(|(_, n)| *n).unwrap_or(0.0);
        assert_eq!(re(ShipKind::Destroyer), 0.0);
        assert!(re(ShipKind::Cruiser) > 0.0 && re(ShipKind::Titan) > re(ShipKind::Dreadnought));
    }

    #[test]
    fn tier_ceiling_gates_the_prize_tiers_behind_research() {
        // No unlock (0) → the base cap of 4, exactly where colonies sit today.
        assert_eq!(max_buildable_tier(StructureKind::MiningComplex, 0), BASE_MAX_STRUCTURE_TIER);
        assert_eq!(max_buildable_tier(StructureKind::Smelter, 0), 4);
        // A Tier-IV or Tier-V unlock lifts the ceiling to 6 (the two superlinear
        // prize tiers) — for every kind, uniformly.
        assert_eq!(max_buildable_tier(StructureKind::MiningComplex, 4), RESEARCHED_MAX_STRUCTURE_TIER);
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

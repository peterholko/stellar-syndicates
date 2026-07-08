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
use crate::ship::ShipKind;

/// What a build job produces on completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BuildKind {
    /// Construct a ship of `ship` kind; it spawns Idle at the building system.
    Ship { ship: ShipKind },
    /// Apply a system upgrade (raises the system's output / capability).
    Upgrade { upgrade: SystemUpgrade },
}

/// The system developments (STRUCTURE sinks). Kept flat (Travian-style) — no
/// refining chain. Each BUILT tier of any of these consumes one development slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemUpgrade {
    /// Lifts deposit output: `richness · EXTRACTOR_RICHNESS_MULT^tier`.
    Extractor,
    /// Raises the system's STORAGE CAP (§buildings step 2): capacity =
    /// `STORAGE_BASE_CAP + STORAGE_PER_DEPOT_TIER · tier`. Caps create the
    /// "ship it / sell it / spend it before it overflows" logistics pressure.
    Depot,
    /// GATES ship construction (§buildings step 3): a system can only build
    /// ships up to its Shipyard tier (`required_shipyard_tier`). Industrial
    /// geography — your shipyard system becomes strategically important.
    Shipyard,
    /// Projects a per-system SENSOR BUBBLE for the system's owner (§buildings
    /// step 2b): radius `sensor_array_radius(tier)`. Buying VISION — the most
    /// on-identity building in a game about information. Feeds the SAME
    /// coverage model as ship bubbles (detection, cargo reveal, pickets, View).
    SensorArray,
    /// STATIC system defense (§buildings step 2c): within
    /// `DEFENSE_PLATFORM_RADIUS` of the system, a hostile raider committing on
    /// one of the owner's convoys must fight THROUGH the platform (tier =
    /// stationary defender units, resolved by the existing seeded battle) before
    /// it can touch the convoy. Makes PLACE defensible — the fortress
    /// specialization, and the prerequisite for any future siege mechanics.
    DefensePlatform,
    /// POPULATION (§buildings step 3a — the Travian-crop analogue): each tier
    /// BOOSTS the system's total output ×`HABITAT_OUTPUT_MULT` but CONSUMES
    /// Provisions continuously (`HABITAT_UPKEEP_PER_TIER`/s) from the system's
    /// own stockpile — the game's first STANDING consumption. A shortfall makes
    /// the habitat UNFED: the boost suspends (never destroys anything) until
    /// food arrives again. Sustaining boosted frontier output becomes a supply
    /// line rivals can raid.
    Habitat,
    /// FUEL REFINERY (§buildings step 3b): converts the system's stockpiled
    /// **Volatiles → Fuel** continuously (`REFINERY_RATE_PER_TIER`/s per tier at
    /// `REFINERY_YIELD` Fuel per Volatile). Volatiles' job — and forward fuel
    /// production: a refinery near your theater turns a Volatiles supply line
    /// into a fuel depot, easing the fuel-∝-distance operating cost. Idles dry
    /// (soft; nothing destroyed).
    Refinery,
}

/// A queued construction job, resolved when `complete_tick` is reached. Lives on
/// the `World` (not the system) so an ownership flip mid-build is unambiguous: the
/// ship is delivered to whoever PAID (`owner`), even if the system is later lost.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BuildJob {
    /// Monotonic id (from `World.next_build_id`) — stable iteration / determinism.
    pub id: u64,
    /// Who paid; keeps the asset even if the system is later lost.
    pub owner: PlayerId,
    /// Where it spawns (ship) / what it upgrades.
    pub system: EntityId,
    pub what: BuildKind,
    /// Absolute sim tick of completion.
    pub complete_tick: u64,
    /// For a ship build (§FLEETS management v1): the fleet to JOIN on completion
    /// if it's still the owner's and docked at this system — otherwise the new
    /// ship forms its own fleet-of-one. `None` always forms a new fleet.
    /// serde default keeps pre-FLEETS build jobs loading.
    #[serde(default)]
    pub join: Option<EntityId>,
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
pub const CONVOY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 35.0)], build_ticks: 12 * HZ };
/// Raider: **Alloys** + **Fuel** — costlier, needs the good frontier materials.
pub const RAIDER_RECIPE: Recipe = Recipe { costs: &[(Commodity::Alloys, 18.0), (Commodity::Fuel, 12.0)], build_ticks: 10 * HZ };
/// Scout: cheap **Ore + Fuel** — the entry unit, buildable at the home turn one
/// (cheap enough that a caught scout is an acceptable loss).
pub const SCOUT_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 20.0), (Commodity::Fuel, 8.0)], build_ticks: 8 * HZ };
/// Corvette: **Ore + Alloys** — the dedicated defender; military industry.
pub const CORVETTE_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 30.0), (Commodity::Alloys, 15.0)], build_ticks: 14 * HZ };
/// Colony Ship: **Ore + Alloys + Provisions** (colonists eat) — absorbs the old
/// instant-claim economics into a physical, raidable investment (§ships part 3).
pub const COLONY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 60.0), (Commodity::Alloys, 20.0), (Commodity::Provisions, 40.0)], build_ticks: 30 * HZ };
/// Extractor (system development): bulk **Ore** — a structure that grows the system's output.
pub const EXTRACTOR_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 60.0)], build_ticks: 18 * HZ };
/// Depot (system development): light **Ore** — cheaper than an Extractor, so early
/// storage capacity is accessible before income compounds.
pub const DEPOT_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 45.0)], build_ticks: 15 * HZ };
/// Shipyard (system development): **Ore + Alloys** per tier — the Alloys component
/// means expanding military industry needs FRONTIER material shipped in (Ore and
/// Alloys rarely co-occur), reinforcing the industrial geography.
pub const SHIPYARD_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 50.0), (Commodity::Alloys, 10.0)], build_ticks: 20 * HZ };
/// Sensor Array (system development): **Ore + Alloys** — advanced intel
/// infrastructure stays tied to frontier material.
pub const SENSOR_ARRAY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 40.0), (Commodity::Alloys, 15.0)], build_ticks: 18 * HZ };
/// Defense Platform (system development): the priciest development yet —
/// fortification is an INVESTMENT (**Ore + Alloys** per tier).
pub const DEFENSE_PLATFORM_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 55.0), (Commodity::Alloys, 20.0)], build_ticks: 22 * HZ };
/// Habitat (system development): **Ore + Provisions** — food to found a colony.
pub const HABITAT_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 45.0), (Commodity::Provisions, 25.0)], build_ticks: 20 * HZ };
/// Fuel Refinery (system development): **Ore + Alloys** industrial plant.
pub const REFINERY_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 50.0), (Commodity::Alloys, 15.0)], build_ticks: 20 * HZ };

pub fn recipe_for(what: BuildKind) -> &'static Recipe {
    match what {
        BuildKind::Ship { ship: ShipKind::Convoy } => &CONVOY_RECIPE,
        BuildKind::Ship { ship: ShipKind::Raider } => &RAIDER_RECIPE,
        BuildKind::Ship { ship: ShipKind::Scout } => &SCOUT_RECIPE,
        BuildKind::Ship { ship: ShipKind::Corvette } => &CORVETTE_RECIPE,
        BuildKind::Ship { ship: ShipKind::Colony } => &COLONY_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::Extractor } => &EXTRACTOR_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::Depot } => &DEPOT_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::Shipyard } => &SHIPYARD_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::SensorArray } => &SENSOR_ARRAY_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::DefensePlatform } => &DEFENSE_PLATFORM_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::Habitat } => &HABITAT_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::Refinery } => &REFINERY_RECIPE,
    }
}

// --- FUEL REFINERY (§buildings step 3b) -----------------------------------------

/// Volatiles consumed per second PER Refinery tier (input-side rate). Tunable.
pub const REFINERY_RATE_PER_TIER: f64 = 0.5;
/// Fuel produced per Volatile consumed. Slightly LOSSY (< 1) so raw Volatiles
/// trade keeps a niche — refine for logistics, sell for margin. Because the
/// yield is < 1, conversion always SHRINKS the stockpile total, so it can never
/// violate the Depot storage cap (a guard still bounds it for yield ≥ 1
/// tunings). Tunable.
pub const REFINERY_YIELD: f64 = 0.8;

// --- HABITAT (§buildings step 3a) ----------------------------------------------

/// Output multiplier per FED Habitat tier, applied to the system's TOTAL
/// production (compounding, and stacking multiplicatively with the Extractor's
/// per-deposit multiplier). Deliberately smaller than the Extractor's 1.5 — the
/// Habitat's edge is that it boosts ALL deposits, including what Extractors
/// already multiplied. Tunable.
pub const HABITAT_OUTPUT_MULT: f64 = 1.25;
/// Provisions consumed per second PER Habitat tier, drawn from the system's own
/// stockpile each tick. Sized so the HOME's renewable Provisions deposit
/// (0.45 × [0.85, 1.15] ≈ 0.38–0.52/s, un-boosted worst case 0.3825/s)
/// comfortably feeds TWO tiers (2 × 0.15 = 0.30/s) even before the boost — the
/// natural first Habitats are self-sustaining, never a starving home. Tunable.
pub const HABITAT_UPKEEP_PER_TIER: f64 = 0.15;

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
    }
}

/// The Shipyard tier every HOME system starts with (consuming one development
/// slot) — the bootstrap that avoids a convoy chicken-and-egg stall on turn one.
pub const HOME_SHIPYARD_TIER: u32 = 1;

/// Multiplier applied to every deposit's richness PER Extractor tier (compounding):
/// `richness · MULT^tier`. The upgrade payoff. Tunable.
pub const EXTRACTOR_RICHNESS_MULT: f64 = 1.5;

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
pub const STORAGE_BASE_CAP: f64 = 500.0;
/// Extra capacity per Depot tier. Tunable.
pub const STORAGE_PER_DEPOT_TIER: f64 = 400.0;

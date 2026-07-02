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

/// The one simple system development in scope: an Extractor tier that lifts deposit
/// output (the STRUCTURE sink). Kept flat (Travian-style) — no refining chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemUpgrade {
    Extractor,
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
/// Extractor (system development): bulk **Ore** — a structure that grows the system's output.
pub const EXTRACTOR_RECIPE: Recipe = Recipe { costs: &[(Commodity::Ore, 60.0)], build_ticks: 18 * HZ };

pub fn recipe_for(what: BuildKind) -> &'static Recipe {
    match what {
        BuildKind::Ship { ship: ShipKind::Convoy } => &CONVOY_RECIPE,
        BuildKind::Ship { ship: ShipKind::Raider } => &RAIDER_RECIPE,
        BuildKind::Upgrade { upgrade: SystemUpgrade::Extractor } => &EXTRACTOR_RECIPE,
    }
}

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

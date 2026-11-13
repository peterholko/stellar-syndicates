//! Physical civilian migration.
//!
//! Population never appears at a colony because a production tick happened.
//! A colony ship still plants the founding cohort; later workforce cohorts
//! arrive aboard Authority migrant liners, either from the Wormhole Hub's
//! external immigration pool or by internal relocation. The liner is an ordinary,
//! observable, interceptable fleet and the destination is credited only when
//! that hull reaches it.

use serde::{Deserialize, Serialize};

use crate::body::Body;
use crate::galaxy::StarSystem;
use crate::{EntityId, PlayerId};

/// One liner carries one complete workforce cohort: 1,000 people.
pub const MIGRANT_COHORT_PEOPLE: u32 = 1_000;
pub const MIGRANT_COHORT_POP: f64 = MIGRANT_COHORT_PEOPLE as f64 / 1_000_000.0;

/// Base interval between immigration allocations to one corporation. Research,
/// the destination world's appeal, and a priority charter shorten this interval.
/// Physical travel time is additional. Tunable for the compressed playtest.
pub const MIGRATION_BASE_INTERVAL_S: f64 = 360.0;

/// Prevent a very distant holding from filling the map with an unbounded bow
/// wave of passenger hulls. This is a traffic cap, not a population cap.
pub const MIGRATION_MAX_IN_FLIGHT_PER_CORP: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPolicy {
    Closed,
    /// Accept a cohort only on a staffing shortage: no worker free while
    /// built posts still want crews.
    #[default]
    Managed,
    /// Accept cohorts while housing and food permit.
    Open,
    /// Like Open, but selected first and granted a shorter allocation interval.
    Priority,
}

impl MigrationPolicy {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Managed => "managed",
            Self::Open => "open",
            Self::Priority => "priority",
        }
    }

    pub fn selection_weight(self) -> f64 {
        match self {
            Self::Closed => 0.0,
            Self::Managed => 2.0,
            Self::Open => 1.0,
            Self::Priority => 4.0,
        }
    }

    pub fn interval_mult(self) -> f64 {
        match self {
            Self::Priority => 0.75,
            _ => 1.0,
        }
    }
}

/// §migration-status (Nov 1 2026): why a world takes no new liner, the
/// allocation pass's gates in the order [`candidate`] checks them. The
/// Population tab names the first that holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationHold {
    Closed,
    /// Nobody lives here: a colony ship founds it first.
    Unsettled,
    Blockaded,
    /// The system is below Well Supplied.
    Food,
    /// No room for another cohort once the liners bound here land.
    Housing,
    /// Managed: a crew in the system is not posted.
    CrewFree,
    /// Managed: the crews on hand and on their way fill every post.
    Staffed,
}

/// The owner's multipliers an allocation reads (research and syndicate).
#[derive(Debug, Clone, Copy)]
pub struct MigrationMods {
    /// Med Bays × the syndicate Migration bonus: a shorter interval.
    pub speed: f64,
    /// Dense Housing.
    pub housing: f64,
    /// Boom Charters, while a world is under half full.
    pub young: f64,
}

/// A world the allocation pass may send the next liner to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MigrantCandidate {
    pub system: EntityId,
    pub body: u32,
    pub score: f64,
    pub interval_s: f64,
}

/// One world's standing with the allocation pass: the interval and score it
/// would be picked with, or the first gate holding it.
pub fn candidate(sys: &StarSystem, body: &Body, mods: MigrationMods) -> Result<MigrantCandidate, MigrationHold> {
    let policy = body.migration_policy;
    if policy == MigrationPolicy::Closed {
        return Err(MigrationHold::Closed);
    }
    if body.population <= 0.0 {
        return Err(MigrationHold::Unsettled);
    }
    if sys.blockade_held() {
        return Err(MigrationHold::Blockaded);
    }
    if sys.food_state != crate::colony::FoodState::WellSupplied {
        return Err(MigrationHold::Food);
    }
    let cap = body.population_capacity() * mods.housing;
    let reserved = body.population + body.inbound_migrants as f64 / 1_000_000.0;
    if cap - reserved + 1e-12 < MIGRANT_COHORT_POP {
        return Err(MigrationHold::Housing);
    }
    if policy == MigrationPolicy::Managed {
        // §workforce cap: posts can't exceed the workforce, so a shortage is
        // no free crew while posts still want one (built posts stand empty, or
        // a population drop left the posted lines short). Crews already
        // aboard liners bound for the system count as filling them, so one
        // empty post calls one liner.
        let workforce = crate::colony::workforce_units(sys.population());
        let posted = sys.workforce_posted();
        let inbound = sys.bodies.iter().map(|b| b.inbound_migrants).sum::<u32>() / MIGRANT_COHORT_PEOPLE;
        if posted < workforce {
            return Err(MigrationHold::CrewFree);
        }
        if posted.max(sys.workforce_posts()) <= workforce + inbound {
            return Err(MigrationHold::Staffed);
        }
    }
    let appeal = body.population_growth_mult().max(0.25);
    let young = if reserved < cap * 0.5 { mods.young } else { 1.0 };
    let interval_s = MIGRATION_BASE_INTERVAL_S * policy.interval_mult()
        / (appeal * mods.speed * young).max(0.05);
    // Policy is the player's explicit direction. Within a policy, worlds with
    // better living conditions and more free housing win; lower population
    // breaks near-ties toward frontier sites.
    let free_frac = ((cap - reserved) / cap.max(MIGRANT_COHORT_POP)).clamp(0.0, 1.0);
    let score = policy.selection_weight() * 100.0 + appeal * 10.0 + free_frac - reserved;
    Ok(MigrantCandidate { system: sys.id, body: body.id, score, interval_s })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrantLeg {
    Outbound,
    Returning,
}

/// The passenger manifest attached to one physical Authority freighter hull.
/// It is deliberately separate from commodity freight: people are passengers,
/// never a market good, and cannot be loaded into a cargo hold.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MigrantRun {
    pub fleet: EntityId,
    pub owner: PlayerId,
    pub dest: EntityId,
    pub body: u32,
    pub people: u32,
    pub leg: MigrantLeg,
    /// `None` for external immigration from the Wormhole Hub. Internal
    /// relocation records its source so a rejected destination can physically
    /// return the cohort instead of deleting it.
    #[serde(default)]
    pub origin: Option<EntityId>,
    #[serde(default)]
    pub origin_body: Option<u32>,
}

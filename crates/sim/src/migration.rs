//! Physical civilian migration.
//!
//! Population never appears at a colony because a production tick happened.
//! A colony ship still plants the founding cohort; later workforce cohorts
//! arrive aboard Authority migrant liners, either from the Wormhole Hub's
//! external immigration pool or by internal relocation. The liner is an ordinary,
//! observable, interceptable fleet and the destination is credited only when
//! that hull reaches it.

use serde::{Deserialize, Serialize};

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
    /// Accept a cohort only when posted jobs exceed the system workforce.
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

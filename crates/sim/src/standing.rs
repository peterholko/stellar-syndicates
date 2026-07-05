//! Standing logistics orders (§15, §23.2) — constrained, non-scripting rules a
//! player sets that execute AUTOMATICALLY on the server clock, online or off, so a
//! player manages POLICY, not micro. This is the heart of the async-persistent loop.
//!
//! Each order is a per-corporation "logistics ledger" entry: from a source system,
//! ship a commodity to a destination when a TRIGGER condition holds. Firing spawns
//! an ordinary RAIDABLE sub-light convoy via the existing convoy machinery — the
//! lightspeed law is untouched; only the *decision* to dispatch is automated, and
//! rivals still learn of the convoy by delayed light. Two anti-spam gates bound a
//! rule to at most one in-flight convoy, re-evaluated on a fixed cadence, so a
//! permanently-satisfied trigger can never flood the map.
//!
//! Everything here is plain data on the [`crate::world::Corporation`]; it serializes
//! with the world snapshot, so standing orders survive restart and replay
//! deterministically (execution is a pure function of the true world + the tick).

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::EntityId;

/// WHERE a standing order draws from / delivers to. Constrained + enumerable.
/// (`Ord` so it can key the in-flight cargo index used by MaintainAtDest.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Endpoint {
    /// A claimed star system (draws from / deposits into its `stockpile`).
    System { id: EntityId },
    /// The galaxy-centre hub commons (sell on arrival; never a valid source).
    Hub,
    /// The owner's home (deposit into `inventory`; never a valid source).
    Home,
}

/// The trigger condition + how much to move when it fires. A small fixed option
/// set the client renders as dropdowns/sliders — no free-form scripting.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// Capability 1: ship the whole whole-unit stockpile whenever the SOURCE's
    /// stockpile of the commodity reaches `threshold`.
    AboveThreshold { threshold: f64 },
    /// Capability 3: ship `percent`% of the surplus above `floor` (whole units);
    /// `floor` = 0 means "P% of everything", a nonzero floor reserves working stock.
    PercentSurplus { percent: u8, floor: f64 },
    /// Capability 2: keep the DESTINATION's level of the commodity at `>= target`,
    /// shipping the deficit — counting goods already in flight toward the dest so it
    /// doesn't over-ship — capped by what the source actually holds.
    MaintainAtDest { target: f64 },
}

/// Whether a rule is currently executing or paused by the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Active,
    Paused,
}

/// One standing logistics order, identified by a per-corp monotonic `id`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StandingOrder {
    /// Per-corporation id (monotonic). 0 on a create command = "allocate a fresh id".
    pub id: u32,
    /// Where goods are drawn from — must be a `System` the corp owns (validated).
    pub source: Endpoint,
    /// Where goods are sent (System | Hub | Home; not equal to `source`).
    pub dest: Endpoint,
    pub commodity: Commodity,
    pub trigger: Trigger,
    pub status: OrderStatus,
    /// Anti-spam gate 1: the rule is not re-evaluated before this tick (a fixed
    /// cadence, so even a permanently-hot trigger is touched at most once per period).
    #[serde(default)]
    pub next_eval_tick: u64,
    /// Anti-spam gate 2: the single convoy this rule currently has in flight (if
    /// any). The rule will not dispatch again until this convoy leaves the world.
    #[serde(default)]
    pub in_flight: Option<EntityId>,
}

impl Endpoint {
    /// The source system id, if this endpoint is a system.
    pub fn system_id(self) -> Option<EntityId> {
        match self {
            Endpoint::System { id } => Some(id),
            _ => None,
        }
    }
}

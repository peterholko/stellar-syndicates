//! Movement sink: fleets BURN FUEL to move (the Travian-style "armies cost upkeep
//! to march" sink, §step1 part 2). Dispatching an operation (a ship move, a raid,
//! a production/standing convoy) draws Fuel from one of the owner's systems,
//! proportional to distance × fleet mass. A shortfall **LIMITS** the operation
//! (it simply doesn't dispatch — the ship/order/goods are never lost), so the game
//! stays async-fair: an offline, fuel-poor player's fleet idles rather than breaks.
//!
//! All rates are `const` → deterministic and tunable. Balance is not the goal; a
//! working "fuel is the thing that makes a spread of systems matter" loop is.

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;

/// What kind of operation a fuel shortfall held (for the owner-only notice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortfallKind {
    /// A player-issued ship move.
    Move,
    /// A player-issued raid intercept.
    Raid,
    /// A production "ship to hub" convoy.
    Shipment,
}

impl ShortfallKind {
    pub fn label(self) -> &'static str {
        match self {
            ShortfallKind::Move => "fleet move",
            ShortfallKind::Raid => "raid",
            ShortfallKind::Shipment => "shipment",
        }
    }
}

/// Fuel burned per (unit of straight-line distance × unit of fleet mass) at
/// dispatch. Tiny because mass (thousands) × distance (thousands) is large — a
/// loaded convoy crossing the core costs a few dozen units. Tunable.
pub const FUEL_PER_MASS_DISTANCE: f64 = 1.0e-6;

/// Fuel seeded into a new corporation's HOME system stockpile on join — the
/// starting operating reserve, so fleets move from turn one before any
/// fuel-bearing system is claimed. The home produces no fuel, so this is the
/// runway that buys time to expand toward fuel deposits. Tunable.
///
/// §hyperspace: sized against STORAGE, because that is what actually binds it.
/// A system holds 700 units of everything combined and a fresh home already
/// carries ~190 of starter goods, so a seed scaled for range (1,500, clipped to
/// the 510 that would fit) did not buy a runway — it filled the warehouse. Ore
/// came out of the ground with nowhere to go and the colony never built a thing.
///
/// That went unnoticed while fuel was drawn from stockpiles: movement drained it
/// continuously, so the silo emptied itself. Once fleets carry their own, the
/// seed just sits there, and its real cost shows up. This leaves room for the
/// economy to breathe while still covering a couple of full refills.
pub const FUEL_HOME_SEED: f64 = 180.0;

/// The commodity that fuels movement (and so is the one operation kind that is
/// EXEMPT from the charge — a convoy hauling Fuel must move without needing Fuel,
/// or a fuel-starved colony could never be resupplied: a deadlock).
pub const MOVEMENT_FUEL: Commodity = Commodity::Fuel;

/// Tank size per unit of HULL mass.
///
/// Burn is `FUEL_PER_MASS_DISTANCE × mass × base_speed × dt`, so a full tank
/// buys `FUEL_PER_HULL_MASS × factor / FUEL_PER_MASS_DISTANCE` of distance —
/// mass cancels, and range depends only on which layer you fly in. Calibrated
/// so a full formation crosses roughly one galaxy radius in open hyperspace and
/// ten in a lane: enough that a one-way trip out is a decision rather than a
/// formality, and that a rim campaign wants a tanker behind it.
pub const FUEL_PER_HULL_MASS: f64 = 0.035;

/// Fuel a fleet of `mass` burns to traverse `distance`. Deterministic; clamps
/// negatives to zero so a degenerate (already-at-destination) dispatch is free.
/// §hyperspace: fuel burned in ONE TICK by a fleet under way.
///
/// The rate is set by the hull's OWN rated speed — before any medium factor —
/// so the lane does not make your engines efficient, it makes the same
/// engine-seconds cover more ground. Integrating over a leg, `v_base` cancels
/// and the total comes to `k · mass · length / factor`: one cause producing both
/// the speed benefit and the fuel benefit, rather than two bolted-on bonuses.
///
/// Zero when idle, and it picks up the Stealth throttle for free — half speed
/// burns half per second and takes twice as long, netting the same total.
pub fn fuel_tick(mass: f64, base_speed: f64, dt: f64) -> f64 {
    FUEL_PER_MASS_DISTANCE * mass.max(0.0) * base_speed.max(0.0) * dt.max(0.0)
}

pub fn fuel_cost(distance: f64, mass: f64) -> f64 {
    FUEL_PER_MASS_DISTANCE * distance.max(0.0) * mass.max(0.0)
}

//! Movement sink: fleets BURN FUEL to move, out of stores they carry
//! (§fuel-overhaul). Three fuels drive three regimes:
//!
//! - **Uranium** (the fission store) drives Slow warp: half speed, a quarter of
//!   the energy per distance. Every hull but the ships of the line carries one.
//!   Corporations mill it at home from Ferrite Ore.
//! - **Deuterium** (the fusion tank) drives Full warp and Stealth. Helium-3 fills
//!   the same tank after Fusion Reactors: one unit of space, six of energy.
//! - **Antimatter** (the bottle on jump-capable hulls) folds space for jumps.
//!
//! A fleet that runs one store dry under way switches to the other; it holds
//! position (never breaks) only when both are empty. All rates are `const`.

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;

/// What kind of operation a fuel shortfall held (for the owner-only notice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortfallKind {
    /// A player-issued ship move.
    Move,
    /// A discrete jump-drive relocation.
    Jump,
    /// A player-issued raid intercept.
    Raid,
    /// A production "ship to hub" convoy.
    Shipment,
}

impl ShortfallKind {
    pub fn label(self) -> &'static str {
        match self {
            ShortfallKind::Move => "fleet move",
            ShortfallKind::Jump => "jump",
            ShortfallKind::Raid => "raid",
            ShortfallKind::Shipment => "shipment",
        }
    }
}

/// Fuel burned per (unit of straight-line distance × unit of fleet mass) at
/// dispatch. Tiny because mass (thousands) × distance (thousands) is large — a
/// loaded convoy crossing the core costs a few dozen units. Tunable.
pub const FUEL_PER_MASS_DISTANCE: f64 = 1.0e-6;

/// §fuel-overhaul: the founding kit's fuel, seeded into the HOME stockpile on
/// join: Uranium for slow freight until the home's mill runs, and a little
/// Deuterium for a full-speed trip when time is critical. Tunable.
///
/// Sized against STORAGE, because that is what actually binds it: with the
/// Uranium Mill's goods and a Warp Core added to the kit, the fuel seed shrinks
/// from the old 60 Fuel to 30, so the kit holds as many units as before. The
/// founding fleet arrives with full stores of its own.
/// A system holds 700 units of everything combined and a fresh home already
/// carries ~190 of starter goods, so a seed scaled for range (1,500, clipped to
/// the 510 that would fit) did not buy a runway — it filled the warehouse. Ore
/// came out of the ground with nowhere to go and the colony never built a thing.
///
/// That went unnoticed while fuel was drawn from stockpiles: movement drained it
/// continuously, so the silo emptied itself. Once fleets carry their own, the
/// seed just sits there, and its real cost shows up. This leaves room for the
/// economy to breathe while still covering a couple of full refills.
pub const URANIUM_HOME_SEED: f64 = 20.0;
pub const DEUTERIUM_HOME_SEED: f64 = 10.0;

/// The fusion tank's base fuel: Full warp, Stealth and Fusion Plants III–IV.
/// Standing-order convoys hauling it skip the tank pre-check, so a dry colony
/// can always be resupplied.
pub const MOVEMENT_FUEL: Commodity = Commodity::Deuterium;
/// The fission store's fuel: Slow warp.
pub const FISSION_FUEL: Commodity = Commodity::Uranium;
/// The bottle's fuel: jumps.
pub const JUMP_FUEL: Commodity = Commodity::Antimatter;

/// §fuel-overhaul: one unit of Helium-3 holds this much Deuterium of energy in
/// one unit of tank space. Per unit of mass D–He3 fusion yields about four times
/// D–D, and nearly all of it charged; D–D loses about a third to neutrons, so
/// the usable ratio is about six. Burning it needs Fusion Reactors. Tunable.
pub const HELIUM3_FUEL_EQUIV: f64 = 6.0;

/// Slow warp (the Uranium regime) runs at this fraction of formation speed…
pub const SLOW_SPEED_FRACTION: f64 = 0.5;
/// …and burns this fraction of Full's energy per distance. A slow trip on
/// Uranium (5.6 Cr) therefore costs what the old Economy cruise did. Tunable.
pub const SLOW_FUEL_MULT: f64 = 0.25;

/// §fuel-overhaul: the Fusion Freight research: player freighters burn this
/// fraction of the Deuterium (and Helium-3) per distance, so full-speed freight
/// costs what slow freight does. Tunable.
pub const FUSION_FREIGHT_MULT: f64 = 0.1;

/// Antimatter bottle per unit of hull mass on a jump-capable hull, at least
/// [`ANTIMATTER_BOTTLE_MIN`] per hull: two full-range jumps. Tunable.
pub const ANTIMATTER_PER_HULL_MASS: f64 = 0.01;
pub const ANTIMATTER_BOTTLE_MIN: f64 = 1.0;

/// Tank size per unit of HULL mass.
///
/// Burn is `FUEL_PER_MASS_DISTANCE × mass × base_speed × dt`, so a full tank
/// buys `FUEL_PER_HULL_MASS × factor / FUEL_PER_MASS_DISTANCE` of distance —
/// mass cancels, and range depends on the active cruise regime. Calibrated so a
/// full formation can cross roughly two galaxy radii in warp: enough that a
/// campaign still has to account for its return fuel.
pub const FUEL_PER_HULL_MASS: f64 = 0.035;

/// Fuel a fleet of `mass` burns to traverse `distance`. Deterministic; clamps
/// negatives to zero so a degenerate (already-at-destination) dispatch is free.
/// Fuel burned in ONE TICK by a fleet under way.
///
/// The rate is set by the hull's OWN rated speed — before any medium factor —
/// so warp makes the same engine-seconds cover more ground. Integrating over a
/// leg, `v_base` cancels
/// and the total comes to `k · mass · length / factor`: one cause producing both
/// the speed benefit and the fuel benefit, rather than two bolted-on bonuses.
///
/// Zero when idle, and it picks up the Stealth throttle for free — half speed
/// burns half per second and takes twice as long, netting the same total.
/// Slow warp applies its separate per-distance multiplier at the fleet
/// caller; this primitive remains unchanged for Full, Stealth and combat hulls.
pub fn fuel_tick(mass: f64, base_speed: f64, dt: f64) -> f64 {
    FUEL_PER_MASS_DISTANCE * mass.max(0.0) * base_speed.max(0.0) * dt.max(0.0)
}

pub fn fuel_cost(distance: f64, mass: f64) -> f64 {
    FUEL_PER_MASS_DISTANCE * distance.max(0.0) * mass.max(0.0)
}

/// Conservative dispatch estimate, shared by onboard freight and CC planning.
/// Inputs are public well positions and the caller's known mass/modifier; this
/// never reads a remote fleet, depot or market. Include the hub in `wells`.
/// Merge overlapping wells so they are charged once. A small transition budget
/// covers spooling/turning; the 15% margin is an estimate, not extra physical burn.
pub fn route_estimate(from: crate::Vec2, to: crate::Vec2, mass: f64,
    consumption: f64, wells: impl IntoIterator<Item = crate::Vec2>) -> f64
{
    let distance = from.distance(to);
    if distance < 1.0 { return 0.0; }
    let direction = (to - from) / distance;
    let radius = crate::transit::HYPERLIMIT;
    let mut intervals = Vec::new();
    for center in wells {
        let offset = center - from;
        let along = offset.dot(direction);
        let across = (offset.length_sq() - along * along).max(0.0);
        if across < radius * radius {
            let half = (radius * radius - across).sqrt();
            let a = (along - half).max(0.0);
            let b = (along + half).min(distance);
            if b > a { intervals.push((a, b)); }
        }
    }
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut inside, mut end, mut crossings) = (0.0, -1.0_f64, 0);
    for (a, b) in intervals {
        if a > end { crossings += 1; }
        inside += (b - a.max(end)).max(0.0);
        end = end.max(b);
    }
    let engine_distance = inside + (distance - inside) / crate::transit::WARP_FACTOR
        + 250.0 * (1 + crossings) as f64;
    fuel_cost(engine_distance, mass) * consumption.max(0.0) * 1.15
}

#[cfg(test)]
mod planning_tests {
    use super::*;
    use crate::Vec2;

    #[test]
    fn hub_well_and_cargo_are_in_the_dispatch_budget() {
        let home = Vec2::new(70_000.0, 0.0);
        let hub = Vec2::ZERO;
        let one = route_estimate(home, hub, 1500.0, 1.0, [home]);
        let both = route_estimate(home, hub, 1500.0, 1.0, [home, hub]);
        assert!(both > one);
        assert_eq!(both, route_estimate(home, hub, 1500.0, 1.0, [home, hub, hub]));
        assert!(route_estimate(home, hub, 2000.0, 1.0, [home, hub]) > both);
        assert_eq!(route_estimate(home, home, 1500.0, 1.0, [home]), 0.0);
    }
}

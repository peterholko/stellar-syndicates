//! THE TERRAN CHARTER AUTHORITY (§9, §TCA) — the home-galaxy body on the far side
//! of the wormhole that issued every corporation's charter. It operates the
//! CHARTERHOUSE (the hub station and its Exchange) and a scheduled common-carrier
//! FREIGHT service to the colonies.
//!
//! The Authority is a NEUTRAL sentinel faction (owner [`crate::ids::PlayerId::TCA`],
//! mirroring the PIRATE sentinel): it owns physical [`crate::ship::ShipKind::Freighter`]
//! fleets but is never a real [`crate::world::Corporation`] — it holds no territory,
//! places no orders, and never appears in rankings or valuation. It projects no
//! force beyond the wormhole's vicinity; protection of its own hulls is retributive,
//! not preventive — and that (standing, citations, enforcement) is PHASE 2. In
//! Phase 1 a freighter kill is consequence-free.
//!
//! This module holds the freight TUNABLES (mirroring the `pirate.rs` block — playtest
//! placeholders; the mechanics are the deliverable) and the freight DATA MODEL
//! ([`Shipment`], [`FreightRun`]) that rides on the [`crate::world::World`]. The
//! scheduler, booking, and run resolution are pure functions in `world.rs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};

// --- TUNABLE TCA FREIGHT BLOCK (playtest placeholders — mechanics are the deliverable) ---

/// Seconds between scheduled freight DEPARTURES per destination. A departure at
/// every multiple of this fires one freighter per destination that has queued
/// shipments in either direction. (Production: scale up alongside the battle
/// timescale, exactly as `battle_target_secs` scales — a longer real game wants
/// rarer, larger convoys.)
pub const TCA_DEPARTURE_PERIOD: f64 = 120.0;

/// Max units a single corporation may load per destination per departure. Bookings
/// beyond this don't reject — they roll forward FIFO to later departures.
pub const TCA_SHIPMENT_CAP: u32 = 400;

/// A destination system with ANY Depot tier gets its per-departure cap multiplied
/// by this (flat v1; per-tier scaling deferred — a Depot means "the Authority runs
/// bigger, cheaper lifts to a place that can receive them").
pub const TCA_DEPOT_CAP_MULT: f64 = 2.0;

/// …and its freight fee discounted by this multiplier (flat v1).
pub const TCA_DEPOT_FEE_MULT: f64 = 0.75;

/// The AD-VALOREM part of the freight fee: this fraction of the cargo's market
/// value (at booking time) is charged. A pure credit SINK — destroyed, never paid
/// to anyone, never refunded (Phase 1 has no TCA treasury).
pub const TCA_FREIGHT_FEE_FRAC: f64 = 0.06;

/// The DISTANCE part of the freight fee: credits per cargo unit per sim-unit of
/// hub→destination distance. Long hauls cost more (the far colonies pay for reach).
pub const TCA_FREIGHT_FEE_PER_UNIT_DIST: f64 = 1.0e-4;

/// THE CHARTERHOUSE SOVEREIGNTY BUBBLE (§TCA Part 4): within this radius of the
/// hub no engagement may OPEN — contact resolution skips pairs inside it, and
/// `Intercept`/`Attack` orders whose target sits inside soft-reject. Fleeing into
/// the bubble is SANCTUARY, by design.
pub const TCA_SOVEREIGN_RADIUS: f64 = 900.0;

/// Interaction radius for player-convoy HUB/SYSTEM load & unload (§TCA Part 5): a
/// fleet must be within this of the Charterhouse (or the owned system's star) to
/// move goods across the warehouse/stockpile boundary. Sized like a short docking
/// approach; playtest placeholder.
pub const LOGISTICS_RANGE: f64 = 260.0;

/// The freight fee for a booking, given the cargo `units`, the per-unit
/// `booking_price` (the standing Exchange price at booking time), the hub→dest
/// `dist`, and whether the destination has a Depot. A PURE function (the whole
/// sink is deterministic + testable). Never negative.
pub fn freight_fee(units: u32, booking_price: f64, dist: f64, has_depot: bool) -> f64 {
    let u = units as f64;
    let value_fee = u * booking_price * TCA_FREIGHT_FEE_FRAC;
    let dist_fee = u * dist * TCA_FREIGHT_FEE_PER_UNIT_DIST;
    let raw = value_fee + dist_fee;
    let fee = if has_depot { raw * TCA_DEPOT_FEE_MULT } else { raw };
    fee.max(0.0)
}

/// The per-corporation, per-departure unit cap for a destination — the base cap,
/// doubled if the destination has a Depot.
pub fn shipment_cap(has_depot: bool) -> u32 {
    if has_depot {
        (TCA_SHIPMENT_CAP as f64 * TCA_DEPOT_CAP_MULT) as u32
    } else {
        TCA_SHIPMENT_CAP
    }
}

/// Deterministic id for a queued/aboard [`Shipment`], allocated from a monotonic
/// counter on the [`crate::world::World`]. A plain small counter (never near
/// 2^53), so a bare JSON number is precise on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ShipmentId(pub u64);

impl std::fmt::Display for ShipmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "F{}", self.0)
    }
}

/// The DIRECTION a booked shipment moves goods, relative to the Charterhouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShipmentDir {
    /// Warehouse → a destination system's stockpile (an EXPORT from the hub).
    Outbound,
    /// A destination system's stockpile → warehouse (an IMPORT to the hub);
    /// `sell_on_arrival` may auto-sell it at the Exchange on landing.
    Inbound,
}

/// Which LEG of its round trip a freighter is flying.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunLeg {
    /// Hub → destination (carrying the Outbound shipments; collects Inbound on arrival).
    Outbound,
    /// Destination → hub (carrying the Inbound shipments home).
    Returning,
}

/// A booked freight shipment — one corporation's one commodity moving one
/// direction on the Authority's scheduled service. Escrowed at booking (its goods
/// have left the warehouse/stockpile but are not yet aboard a freighter); it waits
/// in [`crate::world::World::freight_queue`] until the next departure loads it, then
/// rides inside a [`FreightRun`]. The `fee_paid` is a pure sink already destroyed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Shipment {
    pub id: ShipmentId,
    pub owner: PlayerId,
    /// The destination (Outbound) or origin (Inbound) system — always corp-owned
    /// at booking time. For Inbound, the pickup system.
    pub system: EntityId,
    pub direction: ShipmentDir,
    pub commodity: Commodity,
    pub units: u32,
    /// The freight fee already charged and destroyed at booking (never refunded).
    pub fee_paid: f64,
    /// Sim-time the booking was made (FIFO tie-break + timeline detail).
    pub booked_at: f64,
    /// INBOUND only: sell the goods at the Exchange on arrival at the hub (at the
    /// arrival-tick standing price). Ignored for Outbound.
    #[serde(default)]
    pub sell_on_arrival: bool,
}

/// A physical freighter run: one [`crate::ship::ShipKind::Freighter`] fleet (owned
/// by [`PlayerId::TCA`]) carrying many owners' many shipments to one destination and
/// back. The MANIFEST rides HERE, not in `Fleet.cargo` — a freighter carries a mix
/// of owners/commodities, whereas `Fleet.cargo` stays the single-commodity
/// player-convoy field. Keyed in [`crate::world::World::freight_runs`] by the
/// freighter fleet id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreightRun {
    /// The freighter fleet flying this run (the map/combat entity).
    pub fleet: EntityId,
    /// The one destination system this run serves.
    pub dest: EntityId,
    pub leg: RunLeg,
    /// The aboard shipments, in load order (FIFO). Ids index `shipments`.
    pub manifest: Vec<ShipmentId>,
    /// The aboard shipment records, keyed by id (deterministic iteration).
    pub shipments: BTreeMap<ShipmentId, Shipment>,
}

impl FreightRun {
    /// Total units of `commodity` owned by `owner` currently aboard (manifest view).
    pub fn units_for(&self, owner: PlayerId, commodity: Commodity) -> u32 {
        self.shipments
            .values()
            .filter(|s| s.owner == owner && s.commodity == commodity)
            .map(|s| s.units)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_has_value_and_distance_parts_and_depot_discount() {
        // 100 units, price 10, dist 5000, no depot.
        // value = 100*10*0.06 = 60; dist = 100*5000*1e-4 = 50; raw = 110.
        let raw = freight_fee(100, 10.0, 5000.0, false);
        assert!((raw - 110.0).abs() < 1e-9, "got {raw}");
        // A depot discounts the WHOLE fee by 0.75.
        let disc = freight_fee(100, 10.0, 5000.0, true);
        assert!((disc - 110.0 * 0.75).abs() < 1e-9, "got {disc}");
        // Zero units is a zero fee (no free lunch, no negative either).
        assert_eq!(freight_fee(0, 10.0, 5000.0, false), 0.0);
    }

    /// THE WIRE CONTRACT: the View exposes the fee's INPUTS (`TCA_FREIGHT_FEE_FRAC`,
    /// `TCA_FREIGHT_FEE_PER_UNIT_DIST`, `TCA_DEPOT_FEE_MULT`) plus each destination's
    /// distance, and the client prices a lot from them. This asserts that
    /// recomposition is EXACTLY what the sim charges, so the two can never drift.
    #[test]
    fn exposed_terms_reproduce_the_charged_fee() {
        for &(units, price, dist, depot) in &[
            (100u32, 10.0, 5000.0, false),
            (37, 22.5, 1234.0, true),
            (1, 0.5, 0.0, true),
            (4096, 62.0, 7999.0, false),
        ] {
            // What a client computes from the exposed inputs…
            let per_unit = price * TCA_FREIGHT_FEE_FRAC + dist * TCA_FREIGHT_FEE_PER_UNIT_DIST;
            let raw = units as f64 * per_unit;
            let client = if depot { raw * TCA_DEPOT_FEE_MULT } else { raw };
            // …must equal what the Authority actually charges.
            let server = freight_fee(units, price, dist, depot);
            assert!(
                (client - server).abs() < 1e-9,
                "client priced {client}, server charged {server} (units {units}, price {price}, dist {dist}, depot {depot})"
            );
        }
    }

    #[test]
    fn depot_doubles_the_cap() {
        assert_eq!(shipment_cap(false), TCA_SHIPMENT_CAP);
        assert_eq!(shipment_cap(true), TCA_SHIPMENT_CAP * 2);
    }

    #[test]
    fn shipment_and_run_round_trip_through_serde() {
        let s = Shipment {
            id: ShipmentId(7),
            owner: PlayerId(42),
            system: EntityId(3),
            direction: ShipmentDir::Inbound,
            commodity: Commodity::Alloys,
            units: 120,
            fee_paid: 33.5,
            booked_at: 12.0,
            sell_on_arrival: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Shipment = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);

        let mut shipments = BTreeMap::new();
        shipments.insert(s.id, s);
        let run = FreightRun {
            fleet: EntityId(99),
            dest: EntityId(3),
            leg: RunLeg::Returning,
            manifest: vec![s.id],
            shipments,
        };
        let json = serde_json::to_string(&run).unwrap();
        let back: FreightRun = serde_json::from_str(&json).unwrap();
        assert_eq!(back.units_for(PlayerId(42), Commodity::Alloys), 120);
        assert_eq!(back.leg, RunLeg::Returning);
    }
}

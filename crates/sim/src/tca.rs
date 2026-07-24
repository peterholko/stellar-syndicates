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
//! not preventive — it runs no patrols and posts no escorts, and the frontier stays
//! lawless. What it does instead is REMEMBER and PRICE: charter standing, citations
//! that travel at c, escalating band consequences, and scripted enforcement
//! expeditions (§TCA Phase 2, the law block below).
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

// --- TUNABLE TCA LAW BLOCK (Phase 2 — playtest placeholders) ----------------
// PHILOSOPHY: this is PRICED OUTLAWRY, not prohibition. Every consequence below
// is a cost a player can knowingly pay, never a wall. If a band ever makes
// attacking Authority freight strictly irrational, the TUNING is wrong — the
// mechanics are still the deliverable.

/// Standing every charter is issued with, and the ceiling regen climbs back to.
pub const TCA_STANDING_START: f64 = 100.0;
pub const TCA_STANDING_MAX: f64 = 100.0;

/// Standing lost per INCIDENT — one Authority freighter raided or destroyed —
/// charged in FULL to every participating corporation (no splitting: three corps
/// jumping one freighter each answer for the freighter).
pub const TCA_STANDING_LOSS_PER_INCIDENT: f64 = 10.0;

/// Standing lost per ENFORCEMENT fleet destroyed. Fighting the law raises the
/// reinstatement bill: the culprit is already in the bottom band, so this deepens
/// the cost without needing any new mechanic.
pub const TCA_STANDING_LOSS_ENFORCEMENT: f64 = 20.0;

// The BAND THRESHOLDS. Status is a PURE FUNCTION of standing (never stored), so
// there is no cached copy to desync. At the default loss these are 1 / 4 / 8 / 12
// incidents. Band 1 is deliberately tuition-cheap: one curious kill costs a
// tariff, never a battlefleet.
/// Below this you are SANCTIONED (tariffs + an Exchange penalty fee).
pub const TCA_SANCTIONED_BELOW: f64 = 100.0;
/// At or below this your freight bookings are SUSPENDED.
pub const TCA_SUSPENDED_AT: f64 = 60.0;
/// At or below this your Exchange charter is REVOKED.
pub const TCA_REVOKED_AT: f64 = 20.0;
/// At or below this you are PROSCRIBED — the Authority sends expeditions.
pub const TCA_PROSCRIBED_AT: f64 = -20.0;

/// Ceiling of the freight-fee multiplier, reached at [`TCA_REVOKED_AT`]. Linear
/// from 1.0 at full standing; clamped at both ends.
pub const TCA_TARIFF_MULT_MAX: f64 = 3.0;

/// Ceiling of the Exchange PENALTY FEE (a fraction of trade value), reached at
/// [`TCA_REVOKED_AT`]. Linear from 0.0 at full standing — a corporation in GOOD
/// STANDING pays exactly NOTHING. That is the whole point of making it
/// penalty-only: the §economy clearing invariants (and their tests) are untouched.
pub const TCA_MARKET_PENALTY_FEE_MAX: f64 = 0.10;

/// Standing regained per second, unconditionally, in every band. At playtest
/// scale one incident heals in ~8 minutes. (Production: scale down alongside the
/// battle timescale, like the other rates.)
pub const TCA_STANDING_REGEN_PER_SEC: f64 = 0.02;

/// Credits per standing point bought back through `PayReinstatement` — a pure
/// sink, like the freight fee.
pub const TCA_REINSTATE_FEE_PER_POINT: f64 = 25.0;

/// Seconds between enforcement expeditions against a corporation that STAYS
/// proscribed (measured from the end of the previous one).
pub const TCA_ENFORCEMENT_PERIOD: f64 = 420.0;
/// How long an expedition holds station before withdrawing. Deliberately SHORTER
/// than a siege could ever run (`SIEGE_DURATION_BATTLE_MULT × battle_target_secs`),
/// so the Authority costs a proscribed corp economy-time and never a colony —
/// `enforcement_withdraws_before_a_siege_could_capture` asserts it.
pub const TCA_ENFORCEMENT_DURATION: f64 = 180.0;
/// Hulls in an expedition.
pub const TCA_ENFORCEMENT_SHIPS: u32 = 6;

/// THE CHARTER STATUS BANDS. Derived from standing, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharterStatus {
    /// Full standing: no tariff, no penalty fee, nothing withheld.
    GoodStanding,
    /// A citation on the record: freight tariffs and an Exchange penalty fee.
    Sanctioned,
    /// …and the Authority will take no NEW freight booking from you. Shipments
    /// already queued or aboard still complete — it honors contracts it took.
    Suspended,
    /// …and the Exchange is closed to you. Resting orders are grandfathered, and
    /// your warehouse is still yours to fetch from; you simply can't trade here.
    Revoked,
    /// …and the Authority sends ENFORCEMENT EXPEDITIONS.
    Proscribed,
}

impl CharterStatus {
    /// The player-facing name of the band.
    pub fn title(self) -> &'static str {
        match self {
            CharterStatus::GoodStanding => "Good Standing",
            CharterStatus::Sanctioned => "Sanctioned",
            CharterStatus::Suspended => "Suspended",
            CharterStatus::Revoked => "Revoked",
            CharterStatus::Proscribed => "Proscribed",
        }
    }
    /// Is the Exchange closed to this band?
    pub fn exchange_closed(self) -> bool {
        matches!(self, CharterStatus::Revoked | CharterStatus::Proscribed)
    }
    /// Is the Authority refusing NEW freight bookings from this band?
    pub fn freight_suspended(self) -> bool {
        matches!(self, CharterStatus::Suspended | CharterStatus::Revoked | CharterStatus::Proscribed)
    }
}

/// THE ONE derivation of charter status from standing (§TCA Phase 2). Pure and
/// threshold-ordered — worst band first, so the boundaries are unambiguous:
/// a threshold value belongs to the HARSHER band for the `_AT` thresholds, and
/// `Sanctioned` starts strictly BELOW full standing.
pub fn charter_status(standing: f64) -> CharterStatus {
    if standing <= TCA_PROSCRIBED_AT {
        CharterStatus::Proscribed
    } else if standing <= TCA_REVOKED_AT {
        CharterStatus::Revoked
    } else if standing <= TCA_SUSPENDED_AT {
        CharterStatus::Suspended
    } else if standing < TCA_SANCTIONED_BELOW {
        CharterStatus::Sanctioned
    } else {
        CharterStatus::GoodStanding
    }
}

/// How far a corporation has fallen from full standing, as a 0..1 fraction of the
/// way down to [`TCA_REVOKED_AT`] (where both penalties max out). Clamped, so a
/// corp below Revoked doesn't keep escalating — the expeditions are the deeper
/// band's answer, not an ever-steeper fee.
fn penalty_ramp(standing: f64) -> f64 {
    let span = TCA_STANDING_MAX - TCA_REVOKED_AT;
    if span <= 0.0 {
        return 0.0;
    }
    ((TCA_STANDING_MAX - standing) / span).clamp(0.0, 1.0)
}

/// The FREIGHT TARIFF multiplier at a given standing: 1.0 at full standing,
/// rising linearly to [`TCA_TARIFF_MULT_MAX`] at [`TCA_REVOKED_AT`] and clamped
/// there. Good standing costs exactly the Phase 1 fee.
pub fn tariff_mult(standing: f64) -> f64 {
    1.0 + (TCA_TARIFF_MULT_MAX - 1.0) * penalty_ramp(standing)
}

/// The EXCHANGE PENALTY FEE at a given standing, as a fraction of trade value:
/// 0.0 at full standing, rising linearly to [`TCA_MARKET_PENALTY_FEE_MAX`] at
/// [`TCA_REVOKED_AT`]. A corporation in GOOD STANDING pays EXACTLY ZERO — the fee
/// is penalty-only precisely so the §economy clearing invariants are untouched.
pub fn market_penalty_frac(standing: f64) -> f64 {
    TCA_MARKET_PENALTY_FEE_MAX * penalty_ramp(standing)
}

/// The band ladder for DISPLAY: `(title, the standing at which this band begins
/// as standing falls)`. One source of truth, so the thresholds are never
/// duplicated in TypeScript.
///
/// Read it as the client draws it: *Good Standing at 100; Sanctioned below 100;
/// Suspended at 60; Revoked at 20; Proscribed at −20.* Note the inclusivity is
/// not uniform — `Sanctioned` begins STRICTLY below full standing, while the
/// three `_AT` bands include their threshold. `charter_status` is the authority;
/// this is the label set, and `the_status_ladder_matches_charter_status` pins the
/// two together.
pub fn status_ladder() -> [(&'static str, f64); 5] {
    [
        (CharterStatus::GoodStanding.title(), TCA_STANDING_MAX),
        (CharterStatus::Sanctioned.title(), TCA_SANCTIONED_BELOW),
        (CharterStatus::Suspended.title(), TCA_SUSPENDED_AT),
        (CharterStatus::Revoked.title(), TCA_REVOKED_AT),
        (CharterStatus::Proscribed.title(), TCA_PROSCRIBED_AT),
    ]
}

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

/// What a corporation is cited FOR (§TCA Phase 2). The Authority protects ONLY
/// its own hulls — player-versus-player raiding is nothing to do with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CitationOffense {
    /// An Authority freighter was RAIDED — its manifest plundered.
    FreightRaided,
    /// An Authority freighter was DESTROYED outright.
    FreightDestroyed,
    /// An Authority ENFORCEMENT vessel was destroyed. Fighting the law costs more
    /// than robbing it.
    EnforcementDestroyed,
}

impl CitationOffense {
    /// The bulletin's wording for this offense.
    pub fn title(self) -> &'static str {
        match self {
            CitationOffense::FreightRaided => "piracy against chartered freight",
            CitationOffense::FreightDestroyed => "destruction of chartered freight",
            CitationOffense::EnforcementDestroyed => "armed resistance to Authority enforcement",
        }
    }
}

/// AN INCIDENT IN FLIGHT (§TCA Phase 2). Nothing happens at the scene: the
/// Authority learns of an offense only when its LIGHT reaches the Charterhouse,
/// and only then does standing move and the public citation issue. A spree deep
/// on the frontier therefore drags a visible light-cone of consequences toward
/// the map's centre behind the culprit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    /// Where the offense happened (the light source).
    pub pos: crate::math::Vec2,
    /// Sim-time it happened.
    pub occurred_at: f64,
    /// Sim-time its light reaches the hub — when standing moves and the bulletin
    /// issues. Precomputed at record time so the pipeline is a pure comparison.
    pub arrive_at: f64,
    /// Every participating corporation. Each is charged the FULL flat loss — three
    /// corps jumping one freighter each answer for the freighter. `BTreeSet` keeps
    /// the charge order deterministic.
    pub culprits: std::collections::BTreeSet<PlayerId>,
    pub offense: CitationOffense,
    /// Standing docked from each culprit when this lands.
    pub loss: f64,
}

/// AN ENFORCEMENT EXPEDITION (§TCA Phase 2): a scripted Authority squadron sent
/// against a PROSCRIBED corporation. It reuses the ordinary fleet + blockade
/// machinery through the sentinel owner — no new AI, no new combat rules, no
/// capture and no colonization. It is fightable (destroy it and it ends early,
/// at the cost of a graver citation) and waitable (it withdraws on its own).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expedition {
    /// The Authority squadron flying it.
    pub fleet: EntityId,
    /// The proscribed corporation it answers.
    pub target_corp: PlayerId,
    /// The system it takes station on.
    pub system: EntityId,
    /// Sim-time it launched.
    pub since: f64,
    /// Sim-time it first reached station (`None` while still inbound) — the clock
    /// `TCA_ENFORCEMENT_DURATION` runs against.
    #[serde(default)]
    pub on_station_since: Option<f64>,
    /// Set once it has been ordered home (recalled or time-served), so the world
    /// doesn't re-issue the order every tick.
    #[serde(default)]
    pub withdrawing: bool,
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

    /// The band ladder, checked EXACTLY at every boundary — the one place a
    /// tuning edit could silently move the law.
    #[test]
    fn charter_status_bands_at_their_exact_boundaries() {
        use CharterStatus::*;
        // Full standing (and anything above, defensively) is good standing.
        assert_eq!(charter_status(TCA_STANDING_MAX), GoodStanding);
        assert_eq!(charter_status(TCA_SANCTIONED_BELOW), GoodStanding);
        assert_eq!(charter_status(1e9), GoodStanding);
        // One hair below full standing is already a citation on the record.
        assert_eq!(charter_status(TCA_SANCTIONED_BELOW - 1e-9), Sanctioned);
        assert_eq!(charter_status(TCA_SUSPENDED_AT + 1e-9), Sanctioned);
        // The `_AT` thresholds belong to the HARSHER band.
        assert_eq!(charter_status(TCA_SUSPENDED_AT), Suspended);
        assert_eq!(charter_status(TCA_REVOKED_AT + 1e-9), Suspended);
        assert_eq!(charter_status(TCA_REVOKED_AT), Revoked);
        assert_eq!(charter_status(TCA_PROSCRIBED_AT + 1e-9), Revoked);
        assert_eq!(charter_status(TCA_PROSCRIBED_AT), Proscribed);
        assert_eq!(charter_status(-1e9), Proscribed);
        // The ladder is monotonic: harsher standing never yields a softer band.
        let mut prev = charter_status(200.0);
        let mut s = 200.0;
        while s > -200.0 {
            let cur = charter_status(s);
            assert!(cur >= prev, "band ladder must be monotonic at {s}");
            prev = cur;
            s -= 0.5;
        }
    }

    /// The default thresholds are the documented 1 / 4 / 8 / 12 incidents.
    #[test]
    fn default_thresholds_are_the_documented_incident_counts() {
        let after = |n: u32| TCA_STANDING_START - n as f64 * TCA_STANDING_LOSS_PER_INCIDENT;
        assert_eq!(charter_status(after(0)), CharterStatus::GoodStanding);
        assert_eq!(charter_status(after(1)), CharterStatus::Sanctioned, "one kill = tuition, not a battlefleet");
        assert_eq!(charter_status(after(3)), CharterStatus::Sanctioned);
        assert_eq!(charter_status(after(4)), CharterStatus::Suspended);
        assert_eq!(charter_status(after(7)), CharterStatus::Suspended);
        assert_eq!(charter_status(after(8)), CharterStatus::Revoked);
        assert_eq!(charter_status(after(11)), CharterStatus::Revoked);
        assert_eq!(charter_status(after(12)), CharterStatus::Proscribed);
    }

    /// GOOD STANDING PAYS NOTHING. This is the invariant that keeps the §economy
    /// clearing tests untouched: the penalty is penalty-only, never a base fee.
    #[test]
    fn good_standing_pays_no_tariff_and_no_penalty() {
        assert_eq!(tariff_mult(TCA_STANDING_MAX), 1.0);
        assert_eq!(market_penalty_frac(TCA_STANDING_MAX), 0.0);
        // …and above the ceiling too (defensive: regen clamps, but be safe).
        assert_eq!(tariff_mult(1e9), 1.0);
        assert_eq!(market_penalty_frac(1e9), 0.0);
        // The freight fee at full standing is EXACTLY the Phase 1 fee.
        let base = freight_fee(100, 10.0, 5000.0, false);
        assert_eq!(base * tariff_mult(TCA_STANDING_MAX), base);
    }

    /// Both penalties ramp linearly to their ceiling at Revoked, then CLAMP —
    /// the deeper bands answer with expeditions, not an ever-steeper bill.
    #[test]
    fn penalties_ramp_linearly_then_clamp_at_revoked() {
        assert!((tariff_mult(TCA_REVOKED_AT) - TCA_TARIFF_MULT_MAX).abs() < 1e-9);
        assert!((market_penalty_frac(TCA_REVOKED_AT) - TCA_MARKET_PENALTY_FEE_MAX).abs() < 1e-9);
        // Clamped below Revoked (including deep into Proscribed).
        assert!((tariff_mult(TCA_PROSCRIBED_AT - 500.0) - TCA_TARIFF_MULT_MAX).abs() < 1e-9);
        assert!((market_penalty_frac(-1e6) - TCA_MARKET_PENALTY_FEE_MAX).abs() < 1e-9);
        // Half way down the ramp is half the extra tariff and half the fee.
        let mid = TCA_STANDING_MAX - (TCA_STANDING_MAX - TCA_REVOKED_AT) / 2.0;
        assert!((tariff_mult(mid) - (1.0 + (TCA_TARIFF_MULT_MAX - 1.0) / 2.0)).abs() < 1e-9);
        assert!((market_penalty_frac(mid) - TCA_MARKET_PENALTY_FEE_MAX / 2.0).abs() < 1e-9);
        // Monotonic: worse standing never costs less.
        let mut s = TCA_STANDING_MAX;
        while s > TCA_PROSCRIBED_AT {
            assert!(tariff_mult(s - 1.0) >= tariff_mult(s) - 1e-12);
            assert!(market_penalty_frac(s - 1.0) >= market_penalty_frac(s) - 1e-12);
            s -= 1.0;
        }
    }

    /// The exported display ladder matches the real derivation, so the client's
    /// labels can never drift from the law. Encodes the one non-uniform edge:
    /// `Sanctioned` BEGINS strictly below full standing, the three `_AT` bands
    /// include their threshold.
    #[test]
    fn the_status_ladder_matches_charter_status() {
        let ladder = status_ladder();
        // Titles are the five bands, worsening in order.
        let expect = [
            CharterStatus::GoodStanding,
            CharterStatus::Sanctioned,
            CharterStatus::Suspended,
            CharterStatus::Revoked,
            CharterStatus::Proscribed,
        ];
        for (row, want) in ladder.iter().zip(expect) {
            assert_eq!(row.0, want.title(), "ladder titles are the bands in order");
        }
        // Good Standing begins AT the ceiling; Sanctioned strictly below it.
        assert_eq!(charter_status(ladder[0].1), CharterStatus::GoodStanding);
        assert_eq!(charter_status(ladder[1].1 - 1e-9), CharterStatus::Sanctioned);
        // The three `_AT` bands include their own threshold exactly.
        for i in 2..5 {
            assert_eq!(charter_status(ladder[i].1).title(), ladder[i].0, "band at threshold {}", ladder[i].1);
        }
        // Thresholds never ascend, so the ladder reads top-to-bottom. The first
        // two rows deliberately SHARE the ceiling value — "Good Standing at 100,
        // Sanctioned below 100" is one number read two ways — so only the bands
        // below that are strictly separated.
        for i in 1..5 {
            assert!(ladder[i].1 <= ladder[i - 1].1, "ladder thresholds must not ascend");
        }
        for i in 2..5 {
            assert!(ladder[i].1 < ladder[i - 1].1, "the lower bands are strictly separated");
        }
    }

    #[test]
    fn status_helpers_agree_with_the_bands() {
        use CharterStatus::*;
        for (st, closed, susp) in [
            (GoodStanding, false, false),
            (Sanctioned, false, false),
            (Suspended, false, true),
            (Revoked, true, true),
            (Proscribed, true, true),
        ] {
            assert_eq!(st.exchange_closed(), closed, "{st:?}");
            assert_eq!(st.freight_suspended(), susp, "{st:?}");
            assert!(!st.title().is_empty());
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

//! Ships: the mobile entities of the galaxy.
//!
//! Two types embody the §7 convoy-vs-raider dial — not a special rule, just
//! different acceleration / top-speed. Convoys are slow and heavy; raiders are
//! fast and light and can run a convoy down. (The lane mass-reduction effect of
//! §7/§10 lands in a later milestone.)
//!
//! Every ship moves under flip-and-burn and acts on a standing **order**; the
//! world advances each ship once per tick. There is no real-time piloting — the
//! async-native, lightspeed-bound design demands standing orders, not micro.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Cargo;
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::movement::advance_toward;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShipKind {
    /// Slow, heavy hauler — the largest ship in the game (§7). Carries trade.
    Convoy,
    /// Fast, light interceptor. Cuts chords across open space to run convoys
    /// down.
    Raider,
    /// The dedicated DEFENDER (§ships part 2): moderate mass (slower than a
    /// raider, faster than a convoy), no cargo, DEFENSE-heavy in the weighted
    /// battle model. It CANNOT raid (raiding is the raider's verb — crisp
    /// roles); it protects by SCREENING: any friendly corvette near a raid
    /// contact on a civilian ship duels the raider first (escort when shadowing
    /// a convoy, garrison when parked at an owned system — standing, offline).
    /// BROADCASTS under the Convention: a declared escort DETERS (a dark
    /// defender would just be a raider with extra steps).
    Corvette,
    /// The SETTLEMENT ship (§ships part 3): the HEAVIEST hull flying — slow,
    /// expensive to fuel — with no cargo bay (it IS the cargo: colonists +
    /// infrastructure). Claiming is now PHYSICAL: send it to an unclaimed
    /// system; on arrival ownership transfers and the ship is CONSUMED (it
    /// becomes the colony). BROADCASTS under the Convention (a declared
    /// civilian settlement vessel — and your expansion is telegraphed,
    /// raidable, escortable). Destroyed in transit = colonists lost.
    Colony,
    /// The ACTIVE-INTEL ship (§scout): the lightest hull in the game — fastest
    /// to accelerate, cheapest to fuel — with NO cargo capacity and negligible
    /// combat strength (in any engagement it is simply destroyed; its defense is
    /// speed and darkness, not armor). Runs DARK like a raider, projects an
    /// oversized sensor bubble (`sensor_mult`), and near rival systems captures
    /// timestamped intel snapshots of their fortifications.
    Scout,
}

/// Mass added per unit of cargo carried. A fully-loaded convoy is meaningfully
/// heavier than an empty one, so it accelerates noticeably worse (a = F/m) — your
/// richest shipments are also the most sluggish. Tunable.
pub const CARGO_MASS_PER_UNIT: f64 = 28.0;

impl ShipKind {
    /// Hull (empty) MASS, m₀. Trade convoys are ORDERS OF MAGNITUDE more massive
    /// than raiders (here ~22×), which is what makes them ponderous — the
    /// acceleration asymmetry emerges from this, not from hand-set accel consts.
    /// The scout is the LIGHTEST hull (mass drives both acceleration and the
    /// fuel-∝-mass trip cost, so light = fast AND cheap to run).
    pub fn hull_mass(self) -> f64 {
        match self {
            ShipKind::Convoy => 4500.0,
            ShipKind::Raider => 200.0,
            ShipKind::Corvette => 800.0,
            ShipKind::Colony => 6000.0, // the heaviest hull — fuel-∝-mass bites
            ShipKind::Scout => 80.0,
        }
    }

    /// CONSTANT cruise speed (sim units / s), GDD §14.1 — there is no
    /// acceleration; a ship travels at exactly this speed (the fleet moves at its
    /// slowest member's, [`Fleet::max_speed`]). All stay well below `c` (= 300)
    /// so relativity is respected — nothing outruns its own light. Ordering
    /// preserves the old relative feel (scout > raider > corvette > convoy >
    /// colony).
    ///
    /// CALIBRATION (migration-gentle): magnitudes are set so a representative
    /// galaxy-crossing trip (~8000 su, the 4-player galaxy radius) takes about as
    /// long as the old flip-and-burn did — whose accel ramp meant its AVERAGE
    /// speed was well under the max cap. Convoy anchors it: old convoy (a=1.5,
    /// cap 48) crossed 8000 su in ≈199 s; constant 40 gives 8000/40 = 200 s. The
    /// other kinds keep the old max-speed RATIOS off that anchor, so raider/
    /// convoy chase dynamics and pacing hold (raider 8000 su: old ≈78 s, new 80 s;
    /// colony old ≈233 s, new 242 s). Tunable.
    pub fn max_speed(self) -> f64 {
        match self {
            ShipKind::Convoy => 40.0,
            ShipKind::Raider => 100.0,
            ShipKind::Corvette => 65.0, // keeps station with convoys, can't chase raiders
            ShipKind::Colony => 33.0, // slowest — the long, visible voyage
            ShipKind::Scout => 115.0, // the fastest thing flying — still < c/2
        }
    }

    /// Whether this kind BROADCASTS under the Convention (visible galaxy-wide,
    /// light-delayed). Convoys do; raiders and scouts run DARK — visible only
    /// inside a rival's sensor coverage. One source of truth for the View's
    /// gating (a broadcasting spy would be useless).
    pub fn broadcasts(self) -> bool {
        // Convoys (trade), corvettes (a DECLARED escort deters), and colony
        // ships (a declared civilian settlement vessel — expansion is
        // telegraphed) broadcast; raiders and scouts run dark.
        matches!(self, ShipKind::Convoy | ShipKind::Corvette | ShipKind::Colony)
    }

    /// Multiplier on `config.sensor_range` for the sensor bubble THIS ship
    /// projects into its owner's coverage union. The scout's whole point:
    /// `SCOUT_SENSOR_MULT` × the standard bubble — mobile vision that out-sees
    /// any other ship. Tunable.
    pub fn sensor_mult(self) -> f64 {
        match self {
            ShipKind::Scout => SCOUT_SENSOR_MULT,
            _ => 1.0,
        }
    }

    // --- COMBAT STRENGTHS (§ships part 1, GDD §26.2 spirit) -------------------
    // Battles are weighted-strength contests, not unit counts: each side's
    // strength is a SUM of these per-kind weights, and the seeded outcome table
    // (`world::outcome_probs`) is a function of the attack/defense RATIO. The
    // weights re-express today's exact outcomes in the new units (see the
    // anchor-point notes on `outcome_probs`), so pre-existing raid results are
    // numerically unchanged. All tunable.

    /// Offensive weight when this kind is the AGGRESSOR in an engagement.
    pub fn attack_weight(self) -> f64 {
        match self {
            ShipKind::Raider => 3.0,   // the hunter
            ShipKind::Corvette => 1.0, // guards; barely bites back
            ShipKind::Convoy => 0.0,   // civilians don't attack
            ShipKind::Colony => 0.0,   // colonists, not soldiers
            ShipKind::Scout => 0.0,    // dies if engaged — speed is its armor
        }
    }

    /// Defensive weight when this kind is ATTACKED (or screening a defender).
    pub fn defense_weight(self) -> f64 {
        match self {
            ShipKind::Raider => 2.0,
            ShipKind::Corvette => 4.0, // the armored screen — built to be attacked
            ShipKind::Convoy => 1.0,
            ShipKind::Colony => 1.0, // a fat civilian hull — escort it
            ShipKind::Scout => 0.0, // no armor at all
        }
    }

    /// Whether this kind counts toward doctrine's local FORCE-RATIO assessments
    /// (weighted strength, not head-count). Non-combatants (convoys, scouts) are
    /// excluded exactly as the old raider-count was — so raider-only worlds see
    /// identical ratios.
    pub fn is_combatant(self) -> bool {
        matches!(self, ShipKind::Raider | ShipKind::Corvette)
    }

    /// A combatant's weight in force-ratio comparisons (attack + defense — its
    /// total fighting presence). Equal-kind fleets produce the same ratios as
    /// the old head-count.
    pub fn combat_weight(self) -> f64 {
        self.attack_weight() + self.defense_weight()
    }

    /// HULL (§FLEETS Part 2 — Lanchester): the damage a kind's pool must absorb
    /// to destroy ONE ship of it. Derived from its defense weight (`defense ×
    /// [`crate::combat::HULL_PER_DEFENSE`]`) with a small floor so even a
    /// zero-defense scout can be attritted (it just dies fast — speed was its
    /// armor). Tunable via the combat block.
    pub fn hull(self) -> f64 {
        (self.defense_weight() * crate::combat::HULL_PER_DEFENSE).max(crate::combat::HULL_MIN)
    }
}

/// The FLAGSHIP precedence (GDD §13.1): a fleet is DRAWN and named for its
/// most-significant member — colony first (the point of the whole voyage),
/// then convoy (trade), corvette (escort), raider (teeth), scout (eyes). A
/// fleet-of-one resolves to that ship's own kind, so nothing changes for the
/// N=1 world. Highest precedence first.
pub const FLAGSHIP_PRECEDENCE: [ShipKind; 5] = [
    ShipKind::Colony,
    ShipKind::Convoy,
    ShipKind::Corvette,
    ShipKind::Raider,
    ShipKind::Scout,
];

/// All ship kinds, in a fixed deterministic order (composition iteration,
/// damage-pool distribution, report ordering). Kept in sync with [`ShipKind`].
pub const ALL_SHIP_KINDS: [ShipKind; 5] = [
    ShipKind::Convoy,
    ShipKind::Raider,
    ShipKind::Corvette,
    ShipKind::Colony,
    ShipKind::Scout,
];

/// The fastest flying speed across every ship kind — the single number the
/// light-game invariant ([`crate::config::SimConfig::light_ratio`]) is measured
/// against: `c` must comfortably outrun even the quickest hull so information
/// and orders can, in principle, overtake any raider. Recomputed from the speed
/// table so a future speed edit can't silently outrun light unnoticed.
pub fn fastest_ship_speed() -> f64 {
    ALL_SHIP_KINDS
        .iter()
        .map(|k| k.max_speed())
        .fold(0.0_f64, f64::max)
}

/// An ESTIMATED-SIZE BUCKET for a fleet seen through the fog (GDD §13.1 intel
/// ladder). A far observer of a broadcasting hammer knows roughly HOW BIG it is
/// — never the exact count, and never what's IN it (that needs sensor coverage).
///
/// Buckets, not ± ranges, on purpose: an exact N can't be inverted out of a
/// bucket the way it could from "±2". Thresholds are tunable but must only ever
/// WIDEN the estimate (the fog-leak tests assert the class is never narrower
/// than the true count warrants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountClass {
    One,
    TwoToThree,
    FourToSeven,
    EightToFifteen,
    SixteenToThirty,
    ThirtyOnePlus,
}

impl CountClass {
    /// The deterministic bucket for an exact total count. Tunable thresholds:
    /// `1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+`.
    pub fn from_count(n: u32) -> Self {
        match n {
            0..=1 => CountClass::One,
            2..=3 => CountClass::TwoToThree,
            4..=7 => CountClass::FourToSeven,
            8..=15 => CountClass::EightToFifteen,
            16..=30 => CountClass::SixteenToThirty,
            _ => CountClass::ThirtyOnePlus,
        }
    }

    /// The human-facing label ("est. 4–7 ships").
    pub fn label(self) -> &'static str {
        match self {
            CountClass::One => "1",
            CountClass::TwoToThree => "2–3",
            CountClass::FourToSeven => "4–7",
            CountClass::EightToFifteen => "8–15",
            CountClass::SixteenToThirty => "16–30",
            CountClass::ThirtyOnePlus => "31+",
        }
    }

    /// A representative count for the STALE-INTEL calculator (Part 3): when an
    /// observer has only the bucket (target out of sensor coverage), the
    /// projected battle assumes this many ships of "typical" composition. It is
    /// deliberately the bucket MIDPOINT, never the true count (leak-checked).
    pub fn midpoint(self) -> u32 {
        match self {
            CountClass::One => 1,
            CountClass::TwoToThree => 2,
            CountClass::FourToSeven => 5,
            CountClass::EightToFifteen => 11,
            CountClass::SixteenToThirty => 23,
            CountClass::ThirtyOnePlus => 40,
        }
    }
}

/// Radius (sim units) within which an arriving COLONY SHIP settles an
/// unclaimed system (§ships part 3) — matches the raid contact radius, so
/// "arrival" means the same thing everywhere. Tunable.
pub const COLONY_CLAIM_RADIUS: f64 = 80.0;

/// Radius (sim units) within which a friendly CORVETTE screens a raid contact
/// on a civilian ship (§ships part 2): shadowing a convoy = escort; parked at
/// an owned system = garrison (same reach as the Defense Platform, so a
/// garrisoned corvette covers the whole protected zone). One rule, both roles.
/// Tunable.
pub const CORVETTE_PROTECT_RADIUS: f64 = 1300.0;

/// The scout's sensor-bubble multiplier over the standard ship bubble (its
/// entire reason to exist: 1.5 × 2200 = 3300 su — out-seeing a tier-1 Sensor
/// Array). Tunable.
pub const SCOUT_SENSOR_MULT: f64 = 1.5;

/// Range (sim units) at which a SCOUT passing a RIVAL-owned system captures an
/// intel snapshot of its fortifications (§scout part 2). ≈ the Defense
/// Platform's protection radius — close enough to be engageable, so scouting a
/// defended system is a risk. Tunable.
pub const SCOUT_INTEL_RANGE: f64 = 1300.0;

/// A scout that stays parked in range keeps its snapshot fresh SILENTLY; the
/// owner-only "Scout report" notice re-fires only when a snapshot had gone
/// stale by this much (the scout left and returned) or the observed tiers
/// changed. Anti-spam. Tunable.
pub const SCOUT_INTEL_RENOTIFY_S: f64 = 60.0;

/// Seconds a patrolling ship waits at each waypoint before moving on.
const PATROL_DWELL: f64 = 2.5;

/// A fleet's TRANSIT THROTTLE (§Part 4): the stealth-vs-speed choice. `Full`
/// (default, behavior-preserving) runs at the formation speed and lights up the
/// fleet's signature at flank; `Stealth` creeps at `STEALTH_FRACTION` of it (~2×
/// trip time) to stay quiet. Applies to MoveTo/Patrol; pursuit is always Full
/// (v1). serde default = Full keeps old snapshots loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitMode {
    #[default]
    Full,
    Stealth,
}

impl TransitMode {
    /// The fraction of formation speed this mode travels at.
    pub fn fraction(self) -> f64 {
        match self {
            TransitMode::Full => 1.0,
            TransitMode::Stealth => crate::detection::STEALTH_FRACTION,
        }
    }
}

/// A fleet's standing order — what it does without further input. Orders are
/// FLEET-LEVEL (GDD §13.1): the whole formation moves, intercepts, and holds as
/// one entity. A fleet-of-one behaves exactly as the old single ship did.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FleetOrder {
    /// At rest, no goal.
    Idle,
    /// Flip-and-burn to a fixed point, then go [`FleetOrder::Idle`].
    MoveTo { dest: Vec2 },
    /// Cycle forever through a list of waypoints, dwelling briefly at each.
    /// (M2 demo behaviour so the shared world is visibly alive; real
    /// player-issued orders arrive in M4/M5.)
    Patrol {
        waypoints: Vec<Vec2>,
        index: usize,
        /// Sim time until which the ship holds at the current waypoint.
        dwell_until: f64,
    },
    /// Autonomously pursue a target ship to intercept (§8). Resolved by the
    /// world in true space (contact → convoy lost; target reaches safety →
    /// raid fails). Pursuit steering lives in [`crate::movement::pursue_step`]
    /// (proportional steer-and-correct) and is driven by the world (it needs the
    /// target's state).
    Intercept { target: EntityId },
    /// BLOCKADE a rival system (§contestable-territory Part 1): fly to the
    /// system and take STATION on it, strangling its logistics. `station` is the
    /// target system's position (static, captured at issue time) so the
    /// self-contained advance can steer to it without a world lookup; `system`
    /// names the target for the world's blockade resolution. On arrival the
    /// fleet HOLDS on station (keeps this order — it does not go Idle), and the
    /// world's standing-defense engages it as any hostile contact.
    Blockade { system: EntityId, station: Vec2 },
}

/// What a trade convoy does when it reaches its destination (§9). A buy spawns a
/// delivery convoy (hub → home) that deposits cargo on arrival; a sell spawns a
/// convoy (home → hub) that sells the cargo at the price-on-arrival; a standing
/// logistics order (§15) can also spawn a convoy that deposits cargo into another
/// system's stockpile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeMission {
    DeliverHome,
    SellAtHub,
    /// Deposit the cargo into the destination system's stockpile on arrival (a
    /// system→system supply convoy; §15). Cargo is lost if the destination is no
    /// longer owned by the convoy's owner when it arrives (no gifting rivals).
    DeliverToSystem { system: EntityId },
}

/// A patrolling raider's AUTONOMOUS defensive sortie (§5.1, Pillar 1): it has
/// broken off its patrol on its own to intercept a hostile raider threatening a
/// friendly convoy, and will resume the saved `patrol` route once the threat is
/// gone. Its presence also marks this Intercept as defensive (not a player's
/// manual raid), so the world's standing-doctrine logic owns its lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefenseEngagement {
    pub target: EntityId,
    pub patrol: Vec<Vec2>,
}

/// A FLEET: the map/sim unit (GDD §13.1). One or more ships of mixed kinds
/// moving, fighting, and being observed as a SINGLE entity. A fleet-of-one is
/// the N=1 case and behaves exactly as the old single [`Ship`]-per-unit world.
///
/// `composition` is a deterministic `BTreeMap` (id-sorted kind iteration) of how
/// many of each kind ride in the formation. It is never empty for a live fleet —
/// an emptied fleet is removed from the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fleet {
    pub id: EntityId,
    pub owner: PlayerId,
    /// How many of each ship kind ride in this formation (deterministic order).
    pub composition: BTreeMap<ShipKind, u32>,
    pub pos: Vec2,
    pub vel: Vec2,
    pub order: FleetOrder,
    /// Cargo carried (convoys only; raiders carry none). Broadcast withholds
    /// this — it is revealed by sensor range, not by the Convention. Capacity
    /// scales with the number of convoys aboard; existing single-convoy rules
    /// are the N=1 case, unchanged.
    pub cargo: Option<Cargo>,
    /// If set, this is a trade convoy fleet that resolves on arrival (§9).
    pub mission: Option<TradeMission>,
    /// If set, this fleet is on an AUTONOMOUS defensive intercept (it broke off
    /// patrol to engage a threat) — server-driven standing doctrine, runs whether
    /// or not the owner is connected.
    #[serde(default)]
    pub defense: Option<DefenseEngagement>,
    /// COLONY fleets only (§ships part 3): the "arrived at an already-claimed
    /// system" notice has been sent for the current hold, so it isn't re-sent
    /// every tick. Cleared whenever the fleet moves again. serde default = false.
    #[serde(default)]
    pub notified_held: bool,
    /// Per-kind DAMAGE POOLS accumulated in an ongoing engagement (Part 2,
    /// Lanchester attrition). Empty when not/never engaged; serde default keeps
    /// old snapshots loading. A kind's ships die whole once its pool ≥ its hull,
    /// carrying the remainder forward.
    #[serde(default)]
    pub damage: BTreeMap<ShipKind, f64>,
    /// TRANSIT THROTTLE (§Part 4): Full (default) or Stealth. Governs move speed
    /// and, via the retarded velocity, the fleet's detection signature.
    #[serde(default)]
    pub transit: TransitMode,
}

impl Fleet {
    /// Build a FLEET-OF-ONE — the migration/spawn primitive. Every place the old
    /// world made a `Ship::new(...)` makes a `Fleet::single(...)`, so the N=1
    /// world is byte-for-byte the same behaviour.
    pub fn single(
        id: EntityId,
        owner: PlayerId,
        kind: ShipKind,
        pos: Vec2,
        order: FleetOrder,
        cargo: Option<Cargo>,
    ) -> Self {
        let mut composition = BTreeMap::new();
        composition.insert(kind, 1);
        Fleet {
            id,
            owner,
            composition,
            pos,
            vel: Vec2::ZERO,
            order,
            cargo,
            mission: None,
            defense: None,
            notified_held: false,
            damage: BTreeMap::new(),
            transit: TransitMode::Full,
        }
    }

    /// How many ships of `kind` ride in this fleet (0 if none).
    pub fn count(&self, kind: ShipKind) -> u32 {
        self.composition.get(&kind).copied().unwrap_or(0)
    }

    /// Does the fleet contain at least one ship of `kind`?
    pub fn contains(&self, kind: ShipKind) -> bool {
        self.count(kind) > 0
    }

    /// Total ship count across all kinds.
    pub fn total_count(&self) -> u32 {
        self.composition.values().copied().sum()
    }

    /// The estimated-size bucket a fog observer sees (never the exact count).
    pub fn count_class(&self) -> CountClass {
        CountClass::from_count(self.total_count())
    }

    /// Add `n` ships of `kind` to the composition.
    pub fn add(&mut self, kind: ShipKind, n: u32) {
        if n > 0 {
            *self.composition.entry(kind).or_insert(0) += n;
        }
    }

    /// Remove up to `n` ships of `kind`, dropping the entry when it hits zero.
    /// Returns how many were actually removed.
    pub fn remove(&mut self, kind: ShipKind, n: u32) -> u32 {
        let have = self.count(kind);
        let take = have.min(n);
        if take == 0 {
            return 0;
        }
        if take == have {
            self.composition.remove(&kind);
            self.damage.remove(&kind);
        } else {
            self.composition.insert(kind, have - take);
        }
        take
    }

    /// Remove exactly one ship of `kind` (e.g. a colony consumed on claim).
    /// Returns true if one was present and removed.
    pub fn remove_one(&mut self, kind: ShipKind) -> bool {
        self.remove(kind, 1) == 1
    }

    /// True once the fleet has no ships left — it should be removed from the world.
    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }

    /// The kind this fleet is DRAWN and named for (flagship precedence). For a
    /// fleet-of-one this is simply that ship's kind.
    pub fn flagship_kind(&self) -> ShipKind {
        for k in FLAGSHIP_PRECEDENCE {
            if self.contains(k) {
                return k;
            }
        }
        // A live fleet is never empty; fall back defensively.
        ShipKind::Scout
    }

    /// A fleet BROADCASTS (Convention, visible galaxy-wide) if ANY member kind
    /// broadcasts — you cannot hide a freighter by parking a raider beside it.
    /// A fleet of only raiders and/or scouts runs DARK.
    pub fn broadcasts(&self) -> bool {
        self.composition.keys().any(|k| k.broadcasts())
    }

    /// The best sensor bubble this fleet projects into its owner's coverage —
    /// the MAX `sensor_mult` among its members (a scout aboard extends vision).
    pub fn sensor_mult(&self) -> f64 {
        self.composition
            .keys()
            .map(|k| k.sensor_mult())
            .fold(1.0_f64, f64::max)
    }

    /// Total EMPTY-HULL mass = Σ hull_mass(kind) × count.
    pub fn hull_mass(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.hull_mass() * *n as f64)
            .sum()
    }

    /// Cargo mass carried by the fleet (§7).
    pub fn cargo_mass(&self) -> f64 {
        self.cargo.map(|c| c.units as f64 * CARGO_MASS_PER_UNIT).unwrap_or(0.0)
    }

    /// Total mass = Σ hull + cargo (§7). Drives fuel-∝-distance×mass exactly as
    /// before; a fleet-of-one convoy with cargo reduces to the old `Ship::mass`.
    pub fn mass(&self) -> f64 {
        self.hull_mass() + self.cargo_mass()
    }

    /// The fleet's TRANSIT speed = formation speed × the throttle fraction
    /// (§Part 4). Full = formation speed (behavior-preserving); Stealth creeps.
    pub fn transit_speed(&self) -> f64 {
        self.max_speed() * self.transit.fraction()
    }

    /// The fleet's detection SIGNATURE (§Part 4) at its CURRENT velocity — the
    /// authoritative-side value (the View recomputes the same from retarded
    /// samples). Dark fleets only; a broadcaster's is unused.
    pub fn signature(&self) -> f64 {
        crate::detection::signature(&self.composition, self.vel.length(), self.max_speed())
    }

    /// FORMATION speed (GDD §14.2): the SLOWEST member sets the pace — the
    /// minimum constant `speed(kind)` among present kinds. For a fleet-of-one this
    /// is that ship's own speed; a hammer carrying a colony ship crawls at the
    /// colony's pace, telegraphing itself by physics. Cargo does NOT slow a fleet
    /// (constant-speed model, §14.1) — it costs FUEL (mass), not time.
    pub fn max_speed(&self) -> f64 {
        self.composition
            .keys()
            .map(|k| k.max_speed())
            .fold(f64::INFINITY, f64::min)
    }

    /// Total offensive weight = Σ attack_weight(kind) × count.
    pub fn attack_power(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.attack_weight() * *n as f64)
            .sum()
    }

    /// Total defensive weight = Σ defense_weight(kind) × count.
    pub fn defense_power(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.defense_weight() * *n as f64)
            .sum()
    }

    /// Total combat weight (force-ratio presence) = Σ combat_weight(kind) × count.
    /// Non-combatant kinds contribute their (small) defense weight only, exactly
    /// as head-count comparisons did for the N=1 world.
    pub fn combat_weight(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.combat_weight() * *n as f64)
            .sum()
    }

    /// Whether the fleet carries any teeth (a combatant kind) for doctrine
    /// force-ratio assessments.
    pub fn is_combatant(&self) -> bool {
        self.composition.keys().any(|k| k.is_combatant())
    }

    /// Advance this fleet one timestep at simulation time `time`. Moves at the
    /// FORMATION constant speed (slowest member sets the pace), scaled by the
    /// transit throttle (§Part 4 — Full/Stealth).
    pub fn advance(&mut self, time: f64, dt: f64) {
        let speed = self.transit_speed();

        match &mut self.order {
            FleetOrder::Idle => {
                // Holds station. (Already at rest.)
                self.vel = Vec2::ZERO;
            }
            FleetOrder::MoveTo { dest } => {
                let step = advance_toward(self.pos, *dest, speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    self.order = FleetOrder::Idle;
                }
            }
            FleetOrder::Blockade { station, .. } => {
                // Fly to station, then HOLD there (keep the Blockade order — the
                // world reads on-station presence as an active blockade; going
                // Idle would drop it). Once arrived, advance_toward returns the
                // station point at zero velocity, so it simply holds each tick.
                let step = advance_toward(self.pos, *station, speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
            }
            FleetOrder::Patrol {
                waypoints,
                index,
                dwell_until,
            } => {
                if waypoints.is_empty() {
                    self.vel = Vec2::ZERO;
                    return;
                }
                if time < *dwell_until {
                    self.vel = Vec2::ZERO;
                    return;
                }
                let dest = waypoints[*index % waypoints.len()];
                let step = advance_toward(self.pos, dest, speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    *dwell_until = time + PATROL_DWELL;
                    *index = (*index + 1) % waypoints.len();
                }
            }
            // Interception is driven by the world (it needs the target's state),
            // so there is nothing to do in the self-contained per-fleet advance.
            FleetOrder::Intercept { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{Cargo, Commodity};
    use crate::ids::{EntityId, PlayerId};
    use crate::math::Vec2;

    fn fleet(comp: &[(ShipKind, u32)], cargo: Option<Cargo>) -> Fleet {
        let mut f = Fleet::single(EntityId(1), PlayerId(1), ShipKind::Scout, Vec2::ZERO, FleetOrder::Idle, cargo);
        f.composition.clear();
        for (k, n) in comp {
            f.add(*k, *n);
        }
        f
    }

    #[test]
    fn fleet_of_one_matches_the_old_single_ship_exactly() {
        // A convoy fleet-of-one moves at the convoy's constant speed (§14.1) —
        // cargo affects fuel (mass), not speed.
        let cargo = Some(Cargo { commodity: Commodity::Ore, units: 100 });
        let f = fleet(&[(ShipKind::Convoy, 1)], cargo);
        assert_eq!(f.max_speed(), ShipKind::Convoy.max_speed(), "fleet-of-one speed == its kind's speed");
        assert_eq!(f.flagship_kind(), ShipKind::Convoy);
        assert_eq!(f.total_count(), 1);
    }

    #[test]
    fn formation_speed_is_set_by_the_slowest_member() {
        // A hammer (raider) carrying a colony ship lumbers at the COLONY's pace.
        let f = fleet(&[(ShipKind::Raider, 3), (ShipKind::Colony, 1)], None);
        assert_eq!(f.max_speed(), ShipKind::Colony.max_speed(), "slowest member sets the formation speed");
        // Raider alone is far faster — proving the formation penalty.
        let raider = fleet(&[(ShipKind::Raider, 1)], None);
        assert!(raider.max_speed() > f.max_speed(), "an unencumbered raider is faster");
    }

    #[test]
    fn mass_and_fuel_sum_over_the_whole_convoy_count() {
        let cargo = Some(Cargo { commodity: Commodity::Ore, units: 50 });
        let f = fleet(&[(ShipKind::Convoy, 3)], cargo);
        let expected = 3.0 * ShipKind::Convoy.hull_mass() + 50.0 * CARGO_MASS_PER_UNIT;
        assert!((f.mass() - expected).abs() < 1e-9, "mass = Σ hull×count + cargo");
        // Fuel ∝ distance × total mass, so a 3-convoy fleet burns 3× a 1-convoy
        // fleet's hull share over the same leg (cargo held equal).
        let one = fleet(&[(ShipKind::Convoy, 1)], None);
        let three = fleet(&[(ShipKind::Convoy, 3)], None);
        let d = 1000.0;
        assert!((crate::fuel::fuel_cost(d, three.mass()) - 3.0 * crate::fuel::fuel_cost(d, one.mass())).abs() < 1e-6);
    }

    #[test]
    fn broadcasts_if_any_member_broadcasts() {
        // You cannot hide a freighter by parking a raider beside it.
        assert!(fleet(&[(ShipKind::Raider, 2), (ShipKind::Convoy, 1)], None).broadcasts());
        assert!(fleet(&[(ShipKind::Corvette, 1)], None).broadcasts());
        // Raiders and/or scouts only → dark.
        assert!(!fleet(&[(ShipKind::Raider, 3)], None).broadcasts());
        assert!(!fleet(&[(ShipKind::Raider, 2), (ShipKind::Scout, 1)], None).broadcasts());
    }

    #[test]
    fn flagship_follows_precedence_colony_convoy_corvette_raider_scout() {
        assert_eq!(fleet(&[(ShipKind::Convoy, 1), (ShipKind::Colony, 1)], None).flagship_kind(), ShipKind::Colony);
        assert_eq!(fleet(&[(ShipKind::Convoy, 1), (ShipKind::Corvette, 2)], None).flagship_kind(), ShipKind::Convoy);
        assert_eq!(fleet(&[(ShipKind::Raider, 5), (ShipKind::Scout, 1)], None).flagship_kind(), ShipKind::Raider);
        assert_eq!(fleet(&[(ShipKind::Scout, 2)], None).flagship_kind(), ShipKind::Scout);
    }

    #[test]
    fn count_class_buckets_are_deterministic_and_never_narrower_than_the_count() {
        // The exact bucket at each threshold edge.
        let cases = [
            (1, CountClass::One),
            (2, CountClass::TwoToThree),
            (3, CountClass::TwoToThree),
            (4, CountClass::FourToSeven),
            (7, CountClass::FourToSeven),
            (8, CountClass::EightToFifteen),
            (15, CountClass::EightToFifteen),
            (16, CountClass::SixteenToThirty),
            (30, CountClass::SixteenToThirty),
            (31, CountClass::ThirtyOnePlus),
            (999, CountClass::ThirtyOnePlus),
        ];
        for (n, class) in cases {
            assert_eq!(CountClass::from_count(n), class, "n={n}");
            // The bucket never rules the true count OUT (leak-safety invariant).
            let lo_hi = match class {
                CountClass::One => (1, 1),
                CountClass::TwoToThree => (2, 3),
                CountClass::FourToSeven => (4, 7),
                CountClass::EightToFifteen => (8, 15),
                CountClass::SixteenToThirty => (16, 30),
                CountClass::ThirtyOnePlus => (31, u32::MAX),
            };
            assert!(n >= lo_hi.0 && n <= lo_hi.1, "count must lie inside its own bucket");
        }
    }

    #[test]
    fn stealth_transit_halves_speed_and_quiets_the_signature() {
        let mut f = fleet(&[(ShipKind::Raider, 1)], None);
        // Full speed → the detection anchor (signature 1.0).
        f.vel = Vec2::new(f.max_speed(), 0.0);
        let full_sig = f.signature();
        assert!((full_sig - 1.0).abs() < 1e-9, "a lone raider at full speed is the 1.0 anchor");
        assert_eq!(f.transit_speed(), f.max_speed(), "Full transit = formation speed");
        // Stealth halves the move speed and, at that speed, quiets the signature.
        f.transit = TransitMode::Stealth;
        assert!((f.transit_speed() - f.max_speed() * 0.5).abs() < 1e-9, "Stealth creeps at STEALTH_FRACTION");
        f.vel = Vec2::new(f.transit_speed(), 0.0);
        assert!(f.signature() < full_sig, "creeping is quieter than flank speed");
    }

    #[test]
    fn combat_power_sums_over_composition() {
        let f = fleet(&[(ShipKind::Raider, 2), (ShipKind::Corvette, 1)], None);
        assert_eq!(f.attack_power(), 2.0 * 3.0 + 1.0); // raiders(3) + corvette(1)
        assert_eq!(f.defense_power(), 2.0 * 2.0 + 4.0); // raiders(2) + corvette(4)
        assert!(f.is_combatant());
        assert!(!fleet(&[(ShipKind::Convoy, 4)], None).is_combatant());
    }
}

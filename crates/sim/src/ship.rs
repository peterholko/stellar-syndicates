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

use serde::{Deserialize, Serialize};

use crate::cargo::Cargo;
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::movement::flip_and_burn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Engine THRUST (force, F). Acceleration is NOT set directly — it's derived
    /// as `a = F / m` (see [`Ship::accel`]). Convoys have somewhat more thrust
    /// (bigger engines) but vastly more mass, so they still accelerate far worse.
    ///
    /// Tuning note: values are deliberately LOW so the build-up to speed, the
    /// flip-and-burn, and the convoy-vs-raider nimbleness gap are all *watchable*
    /// at the current galaxy scale — a chase plays out over tens of seconds, not
    /// an instant. With these consts an empty raider accelerates at
    /// `2200/200 = 11` su/s² and an empty convoy at `6750/4500 = 1.5` su/s² (a
    /// loaded one ~0.86), so the raider visibly darts while the convoy lumbers.
    pub fn thrust(self) -> f64 {
        match self {
            ShipKind::Convoy => 6750.0,
            ShipKind::Raider => 2200.0,
            // 4000/800 = 5 su/s² — nimbler than a convoy, no match for a raider.
            ShipKind::Corvette => 4000.0,
            // 7200/6000 = 1.2 su/s² — the most ponderous thing in space.
            ShipKind::Colony => 7200.0,
            // Small engine, tiny hull: 1400/80 = 17.5 su/s² — the dartiest ship.
            ShipKind::Scout => 1400.0,
        }
    }

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

    /// Cruise speed cap (sim units / s). All stay well below `c` (= 300) so
    /// relativity is respected — nothing outruns its own light. Acceleration
    /// (above) ramps velocity up to this cap.
    pub fn max_speed(self) -> f64 {
        match self {
            ShipKind::Convoy => 48.0,
            ShipKind::Raider => 120.0,
            ShipKind::Corvette => 80.0, // keeps station with convoys, can't chase raiders
            ShipKind::Colony => 40.0, // slower than a convoy — the long, visible voyage
            ShipKind::Scout => 140.0, // the fastest thing flying — still < c/2
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

/// A ship's standing order — what it does without further input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ShipOrder {
    /// At rest, no goal.
    Idle,
    /// Flip-and-burn to a fixed point, then go [`ShipOrder::Idle`].
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ship {
    pub id: EntityId,
    pub owner: PlayerId,
    pub kind: ShipKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub order: ShipOrder,
    /// Cargo carried (convoys only; raiders carry none). Broadcast withholds
    /// this — it is revealed by sensor range, not by the Convention.
    pub cargo: Option<Cargo>,
    /// If set, this is a trade convoy that resolves on arrival (§9).
    pub mission: Option<TradeMission>,
    /// If set, this raider is on an AUTONOMOUS defensive intercept (it broke off
    /// patrol to engage a threat) — server-driven standing doctrine, runs whether
    /// or not the owner is connected.
    #[serde(default)]
    pub defense: Option<DefenseEngagement>,
    /// COLONY ships only (§ships part 3): the "arrived at an already-claimed
    /// system" notice has been sent for the current hold, so it isn't re-sent
    /// every tick. Cleared whenever the ship moves again. serde default = false.
    #[serde(default)]
    pub notified_held: bool,
}

impl Ship {
    pub fn new(
        id: EntityId,
        owner: PlayerId,
        kind: ShipKind,
        pos: Vec2,
        order: ShipOrder,
        cargo: Option<Cargo>,
    ) -> Self {
        Ship {
            id,
            owner,
            kind,
            pos,
            vel: Vec2::ZERO,
            order,
            cargo,
            mission: None,
            defense: None,
            notified_held: false,
        }
    }

    /// Total mass = hull + cargo (§7). A loaded ship is heavier, so slower to
    /// accelerate. Recomputed from current cargo, so dropping/gaining cargo
    /// changes how the ship handles.
    pub fn mass(&self) -> f64 {
        let cargo = self.cargo.map(|c| c.units as f64 * CARGO_MASS_PER_UNIT).unwrap_or(0.0);
        self.kind.hull_mass() + cargo
    }

    /// Acceleration DERIVED from thrust and mass: `a = F / m` (§7). Higher mass
    /// (bigger hull, fuller hold) → weaker acceleration for the same thrust. This
    /// is the single source of the raider-vs-convoy nimbleness asymmetry and of
    /// the loaded-convoy penalty — not a hand-set per-kind acceleration.
    pub fn accel(&self) -> f64 {
        self.kind.thrust() / self.mass()
    }

    /// Advance this ship one timestep at simulation time `time`.
    pub fn advance(&mut self, time: f64, dt: f64) {
        let accel = self.accel();
        let max_speed = self.kind.max_speed();

        match &mut self.order {
            ShipOrder::Idle => {
                // Holds station. (Drag-free space; already at rest.)
                self.vel = Vec2::ZERO;
            }
            ShipOrder::MoveTo { dest } => {
                let step = flip_and_burn(self.pos, self.vel, *dest, accel, max_speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    self.order = ShipOrder::Idle;
                }
            }
            ShipOrder::Patrol {
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
                let step = flip_and_burn(self.pos, self.vel, dest, accel, max_speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    *dwell_until = time + PATROL_DWELL;
                    *index = (*index + 1) % waypoints.len();
                }
            }
            // Interception is driven by the world (it needs the target's state),
            // so there is nothing to do in the self-contained per-ship advance.
            ShipOrder::Intercept { .. } => {}
        }
    }
}

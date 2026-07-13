//! PIRATE ENCLAVES (§pirates) — a deterministic, seeded NEUTRAL hostile faction
//! that fills the empty dark between player collisions with ambient danger, safe
//! combat practice, and objectives that don't require farming another human.
//!
//! An [`Enclave`] is a hidden base at an unclaimed mid-ring system. It stays DARK
//! until a scout snapshots it (like fortifications), periodically launches a dark
//! raider PACK (owned by the [`crate::ids::PlayerId::PIRATE`] sentinel — so it
//! reuses ALL the fleet/combat/raid code by owner comparison) that hunts
//! BROADCASTING convoys within its radius, escalates on a slow clock if ignored,
//! and is suppressed by ASSAULTING the base (a platform-equivalent defense pool ∝
//! tier). Pirates STEAL (raid brevity) — they never siege, never capture — so the
//! standing defense handles them fully offline; loss rates are bounded by the same
//! raid caps as players.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::EntityId;

// --- TUNABLE PIRATE BLOCK (playtest placeholders — mechanics are the deliverable) ---
/// How many hidden enclaves to seed at generation.
pub const PIRATE_ENCLAVE_COUNT: usize = 3;
/// No enclave is seeded within this of ANY home slot (keeps piracy off the doorstep).
pub const PIRATE_HOME_EXCLUSION: f64 = 2600.0;
/// Enclaves live in this frontier band (0 = inner margin, 1 = rim) — the MID ring.
pub const PIRATE_RING_LO: f64 = 0.30;
pub const PIRATE_RING_HI: f64 = 0.72;
/// Cap on the escalation tier.
pub const PIRATE_MAX_TIER: u32 = 3;
/// The base's platform-equivalent DEFENSE tiers = `tier × this` (grinding these to
/// 0 in an assault destroys the base). Reuses the Defense-Platform combat model.
pub const PIRATE_DEFENSE_PER_TIER: u32 = 2;
/// Raiders in a launched pack = `tier × this`. At `1`, a fresh enclave opens with
/// a LONE bandit (tier 1 → 1 raider) and only grows into a real pack (2, then 3)
/// if it's left to escalate — the Civ-barbarian ramp: weak first contact, a
/// serious threat only when ignored. Keeps the raider's own combat stats (and the
/// PvP counter-triangle) untouched — this scales the PIRATE pack, not the hull.
pub const PIRATE_PACK_PER_TIER: u32 = 1;
/// Seconds before an enclave launches its FIRST-EVER pack (seeded at generation).
/// Deliberately long: nothing hunts the galaxy during the opening minutes, so a
/// founding corp's first convoys reach the hub unmolested. The steady 90 s cadence
/// (`PIRATE_LAUNCH_PERIOD`) only takes over after this initial delay.
pub const PIRATE_FIRST_LAUNCH_SECS: f64 = 300.0;
/// NEW-PLAYER GRACE: a corp's broadcasting convoys are INVISIBLE to pirate hunting
/// for this long after the corp JOINS (keyed on `Corporation.joined_tick`, not
/// wall-clock game time). This is what protects a LATECOMER who drops into an
/// already-escalated galaxy — they get the same undefended-onboarding window a
/// founder gets, measured from their own join. Established corps past the window
/// are hunted normally.
pub const PIRATE_GRACE_SECS: f64 = 240.0;
/// Hunting radius = base + per-tier (wider reach as the enclave escalates).
pub const PIRATE_HUNT_RADIUS_BASE: f64 = 2600.0;
pub const PIRATE_HUNT_RADIUS_PER_TIER: f64 = 900.0;
/// Seconds between pack launches (one pack out per enclave at a time).
pub const PIRATE_LAUNCH_PERIOD: f64 = 90.0;
/// Seconds between escalation-tier growths while UNsuppressed.
pub const PIRATE_GROW_PERIOD: f64 = 300.0;
/// After a base is destroyed, this long DORMANT before a weaker (tier-1) respawn.
pub const PIRATE_DORMANCY: f64 = 600.0;
/// A player war-fleet stationed (Idle) within this of an ACTIVE enclave opens an
/// assault on the base (the "attack the defended site" gesture).
pub const PIRATE_ASSAULT_RADIUS: f64 = 220.0;

/// The hunting radius at a given tier.
pub fn hunt_radius(tier: u32) -> f64 {
    PIRATE_HUNT_RADIUS_BASE + PIRATE_HUNT_RADIUS_PER_TIER * (tier.saturating_sub(1) as f64)
}
/// The base's platform-equivalent defense tiers at a given enclave tier.
pub fn base_defense_tiers(tier: u32) -> u32 {
    tier * PIRATE_DEFENSE_PER_TIER
}
/// The raider count a pack launches at a given tier (≥ 1).
pub fn pack_size(tier: u32) -> u32 {
    (tier * PIRATE_PACK_PER_TIER).max(1)
}

/// A hidden pirate base at an unclaimed system. Its schedules are seeded at
/// generation (deterministic: same seed → same piracy). Its platform-equivalent
/// defense lives on the host `StarSystem.defense_tier` (so the assault reuses the
/// Defense-Platform combat verbatim); THIS carries the AI state + loot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enclave {
    /// The unclaimed system this base sits at (`owner` stays `None` — dark until
    /// scouted; existence is DISCOVERED via scouting + raids, never announced).
    pub system: EntityId,
    /// Escalation tier (1..=`PIRATE_MAX_TIER`); grows on the slow clock if ignored.
    pub tier: u32,
    /// Loot returned by packs — the prize an assault victor seizes.
    #[serde(default)]
    pub plunder: BTreeMap<Commodity, u32>,
    /// Sim-time of the next pack launch (seeded, staggered per enclave).
    pub next_launch_at: f64,
    /// Sim-time of the next escalation growth.
    pub next_grow_at: f64,
    /// `0.0` = active; `> now` = suppressed/dormant (a weaker base respawns after).
    #[serde(default)]
    pub dormant_until: f64,
    /// The current pack fleet id (out raiding or home), if one is deployed.
    #[serde(default)]
    pub pack: Option<EntityId>,
}

impl Enclave {
    /// Whether the enclave is ACTIVE (not in post-suppression dormancy) at `now`.
    pub fn active(&self, now: f64) -> bool {
        now >= self.dormant_until
    }
}

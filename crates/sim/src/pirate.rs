//! PIRATE ENCLAVES (§pirates) — a deterministic, seeded NEUTRAL hostile faction
//! that fills the empty dark between player collisions with ambient danger, safe
//! combat practice, and objectives that don't require farming another human.
//!
//! An [`Enclave`] is a hidden base at an unclaimed mid-ring system. It stays DARK
//! until a scout snapshots it (like fortifications), periodically launches a dark
//! raider PACK (owned by the [`crate::ids::PlayerId::PIRATE`] sentinel — so it
//! reuses ALL the fleet/combat/raid code by owner comparison) that hunts
//! corporate convoys within its local hunting radius and periodically raids an
//! unprotected home stockpile. It escalates on a slow clock if ignored,
//! and is suppressed by ASSAULTING the base (a platform-equivalent defense pool ∝
//! tier). Home raids are short, defended blockades with a capped theft allowance
//! and a protected stock floor — never capture or bombardment. Standing defenses
//! fight while the owner is offline; warships and platforms can take real losses.
//! Fortified depots and regional strongholds are separate fixed-strength sites:
//! stationary fitted garrisons, no escalation, and permanent clearance that
//! opens their host system for settlement. Their plunder must be hauled home.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Recognizable, fixed opponents, never generated to counter a player's loadout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PirateFaction { Ashwake, Ironclad, Rift }
impl PirateFaction {
    pub fn for_base(id: crate::EntityId) -> Self {
        match id.0 % 3 { 0 => Self::Ashwake, 1 => Self::Ironclad, _ => Self::Rift }
    }
    pub fn name(self) -> &'static str { match self {
        Self::Ashwake => "Ashwake Corsairs", Self::Ironclad => "Ironclad Reclaimers", Self::Rift => "Rift Stalkers",
    } }
    pub fn counter(self) -> &'static str { match self {
        Self::Ashwake => "Torpedo salvos; bring point-defense screens and prioritize missile ships.",
        Self::Ironclad => "Slow armored scavengers; torpedoes bypass their plating. Avoid a beam slugging match.",
        Self::Rift => "Fast driver ambushers; use Whipple Armor and screen transports. They retreat at 50% hull.",
    } }
    pub fn mission(self) -> crate::doctrine::MissionProfile {
        use crate::doctrine::*;
        MissionProfile { priority: TargetPriority::Transports, screening: ScreeningRole::Automatic,
            withdrawal: if self == Self::Rift { DamageWithdrawal::Hull50 } else { DamageWithdrawal::Never } }
    }
    pub fn patrol(self, count: u32) -> Vec<(crate::ShipKind, crate::Loadout, u32)> {
        use crate::{ShipKind as H, ModuleKind as M, Loadout};
        let (hull, fit) = match self {
            Self::Ashwake => (H::Raider, vec![M::TorpedoRack]),
            Self::Ironclad => (H::Corvette, vec![M::ReflectivePlating, M::WhippleArmor]),
            Self::Rift => (H::Raider, vec![M::MassDriver]),
        };
        vec![(hull, Loadout::new(fit), count.max(1))]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatObjective { DisableFireControl, DisableSupply, HoldExtraction, BreakBlockade, ProtectEvacuation }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportSite {
    pub id: crate::EntityId,
    pub pos: crate::Vec2,
    pub objective: CombatObjective,
    pub guards: Vec<crate::EntityId>,
    pub disabled_at: Option<f64>,
    pub disabled_by: Option<crate::PlayerId>,
    pub effect_at: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseNetwork {
    pub faction: PirateFaction,
    pub supports: Vec<SupportSite>,
    pub patrol: Option<crate::EntityId>,
    pub next_patrol_at: f64,
    pub lost_at: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Campaign {
    pub bases: std::collections::BTreeMap<crate::EntityId, BaseNetwork>,
    /// Immutable mission actors survive expired contracts; renewal never clones them.
    pub missions: std::collections::BTreeMap<crate::OperationId, ObjectiveActors>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectiveActors {
    pub pirates: Vec<crate::EntityId>,
    pub evacuees: Option<crate::EntityId>,
    pub launched: bool,
    pub arrived: bool,
}

pub const CAMPAIGN_PATROL_COST: u32 = 12;
pub const CAMPAIGN_PATROL_PERIOD_S: f64 = 300.0;
pub const EXTRACTION_HOLD_S: u32 = 60;

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};

/// Home raids respect founder protection and cannot start during the first
/// thirty sim minutes, even for an older corporation without a founding record.
pub const HOME_RAID_GRACE_S: f64 = 30.0 * 60.0;
/// Minimum warning lead before departure, AFTER the pirate's broadcast reaches
/// the target's CC. Travel adds further response time; no teleporting attackers.
pub const HOME_RAID_WARNING_S: f64 = 90.0;
/// Per-victim rest between expeditions (also refreshed when a raid breaks off).
pub const HOME_RAID_COOLDOWN_S: f64 = 15.0 * 60.0;
/// A smash-and-grab cannot hold orbit indefinitely. Scales with battle pacing.
pub const HOME_RAID_HOLD_BATTLE_MULT: f64 = 2.0;
pub const HOME_RAID_MAX_LOOT: u32 = 60;
pub const HOME_RAID_STOCK_FRAC: f64 = 0.10;

/// One physical pack's home-raid mission. This is private simulation state,
/// never a client countdown. The ordinary fleet/history and delayed notices
/// expose only arrived light. Persist every phase: loading a save must neither
/// reissue the warning nor refill the theft allowance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeRaid {
    pub base: EntityId,
    pub owner: PlayerId,
    pub system: EntityId,
    pub pos: crate::math::Vec2,
    pub depart_at: f64,
    pub arrived_at: Option<f64>,
    /// Frozen at first arrival; replenishing/importing cannot enlarge this raid.
    pub allowance: BTreeMap<Commodity, u32>,
    pub stolen: BTreeMap<Commodity, u32>,
    pub loading_credit: f64,
    pub returning: bool,
    /// First loss of the source base. The pack may react only inside this
    /// report's light cone; turning a remote fleet instantly would leak the loss.
    #[serde(default)]
    pub source_lost_at: Option<f64>,
    #[serde(default)]
    pub counter_raid_offered: bool,
}

// --- TUNABLE PIRATE BLOCK (playtest placeholders — mechanics are the deliverable) ---
/// How many hidden enclaves to seed at generation.
pub const PIRATE_ENCLAVE_COUNT: usize = 3;
/// Fixed objectives, separate from the three repeatable, escalating hideouts.
/// These tags travel in the existing scout snapshot, never from live site truth.
pub const DEPOT_TIER: u32 = 4;
pub const STRONGHOLD_TIER: u32 = 5;
pub fn permanent_site(tier: u32) -> bool { tier >= DEPOT_TIER }
pub fn site_name(tier: u32) -> &'static str {
    match tier { DEPOT_TIER => "Fortified pirate depot", STRONGHOLD_TIER => "Regional stronghold", _ => "Privateer hideout" }
}

/// Published, fixed encounter fittings: research can counter them; bringing a
/// better fleet never silently increases the opposition. All hulls start whole.
pub fn formation(tier: u32) -> Vec<(crate::ship::ShipKind, crate::module::Loadout, u32)> {
    use crate::ship::ShipKind::*;
    use crate::module::{Loadout, ModuleKind::*};
    match tier {
        DEPOT_TIER => vec![(Destroyer, Loadout::new(vec![MassDriver, ReflectivePlating]), 1),
            (Corvette, Loadout::new(vec![PointDefenseScreen, ReflectivePlating]), 1)],
        STRONGHOLD_TIER => vec![(Cruiser, Loadout::new(vec![MassDriver, WhippleArmor, ReflectivePlating]), 1),
            (Destroyer, Loadout::new(vec![TorpedoRack, ReflectivePlating]), 1),
            (Corvette, Loadout::new(vec![PointDefenseScreen, ReflectivePlating]), 2)],
        _ => vec![(Raider, Loadout::default(), pack_size(tier))],
    }
}

pub fn patrol_fitting(variant: u8) -> crate::module::Loadout {
    use crate::module::{Loadout, ModuleKind::*};
    Loadout::new(match variant % 3 {
        0 => vec![MassDriver], 1 => vec![TorpedoRack], _ => vec![ReflectivePlating],
    })
}
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
/// Deliberately long: no AMBIENT enclave hunts the galaxy during the opening
/// minutes. The founding programme owns its one damaged, scripted Convoy threat;
/// the steady 90 s cadence only takes over after this initial delay.
pub const PIRATE_FIRST_LAUNCH_SECS: f64 = 300.0;
/// NEW-PLAYER GRACE: a corp's convoys are invisible to pirate hunting
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
/// Finite wreckage at an ambient hideout, even if its raid stole nothing.
/// Created once at clearance, recovered physically; never a remote home payout.
pub const HIDEOUT_WRECK_ALLOYS_PER_TIER: u32 = 8;

/// Authored, finite equipment caches. These are useful combinations of real
/// fittings, not a new rarity multiplier: a breacher still needs torpedo cover,
/// and a screening ship still sacrifices gun damage. Only permanent sites mint
/// them, once on clearance; ordinary respawning hideouts cannot farm dossiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitePrize { BreacherCache, ScreenCache }

impl SitePrize {
    pub fn for_tier(tier: u32) -> Option<Self> {
        match tier { DEPOT_TIER => Some(Self::BreacherCache), STRONGHOLD_TIER => Some(Self::ScreenCache), _ => None }
    }

    pub fn title(self) -> &'static str {
        match self { Self::BreacherCache => "Breacher cache", Self::ScreenCache => "Fleet-screen cache" }
    }

    pub fn modules(self) -> BTreeMap<crate::module::ModuleKind, u32> {
        use crate::module::ModuleKind::*;
        match self {
            Self::BreacherCache => BTreeMap::from([(MassDriver, 2), (WhippleArmor, 2)]),
            Self::ScreenCache => BTreeMap::from([(PointDefenseScreen, 4), (ReflectivePlating, 4)]),
        }
    }

    pub fn programme(self) -> &'static str {
        match self { Self::BreacherCache => "hull_line_v_cruiser", Self::ScreenCache => "hull_line_vi_battleship" }
    }

    /// Tunable: one dossier offsets 30% of this programme only. Normal gates,
    /// the remaining Academy work and the shipbuilding recipe still apply.
    pub const DOSSIER_FRACTION: f64 = crate::research::DOSSIER_WORK_FRACTION;

    pub fn summary(self) -> &'static str {
        match self {
            Self::BreacherCache => "2 Mass Drivers + 2 Whipple Armor: outfit two breachers; no torpedo cover. Cruiser Hull dossier: 30% research work.",
            Self::ScreenCache => "4 Point-Defense Screens + 4 Reflective Plating: outfit four escorts; reduced gun damage. Battleship dossier: 30% research work.",
        }
    }
}
/// A player war-fleet stationed (Idle) within this of an ACTIVE enclave opens an
/// assault on the base (the "attack the defended site" gesture).
pub const PIRATE_ASSAULT_RADIUS: f64 = 220.0;

/// The hunting radius at a given tier.
pub fn hunt_radius(tier: u32) -> f64 {
    PIRATE_HUNT_RADIUS_BASE + PIRATE_HUNT_RADIUS_PER_TIER * (tier.saturating_sub(1) as f64)
}
/// The base's platform-equivalent defense tiers at a given enclave tier.
pub fn base_defense_tiers(tier: u32) -> u32 {
    match tier { DEPOT_TIER => 3, STRONGHOLD_TIER => 6, _ => tier * PIRATE_DEFENSE_PER_TIER }
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
    /// 1..=3 are escalating hideouts; 4/5 identify fixed depot/stronghold templates.
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
    /// Heavy objectives stay cleared across save/load. Legacy hideouts retain
    /// their old dormant/respawn cycle; no new state is inferred from a clock.
    #[serde(default)]
    pub cleared: bool,
}

impl Enclave {
    /// Whether the enclave is ACTIVE (not in post-suppression dormancy) at `now`.
    pub fn active(&self, now: f64) -> bool {
        !self.cleared && now >= self.dormant_until
    }
}

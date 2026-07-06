//! Fleet doctrine (§16 / async-automation Layer 2): a corporation's standing
//! combat & logistics policy.
//!
//! Like standing logistics orders ([`crate::standing`]), doctrine is the async
//! answer to "I can't babysit my ships": a CONSTRAINED menu (no scripting) that
//! the server runs every tick, ONLINE OR OFF. A player who sets `EngageWeaker` +
//! `RetreatThreshold::Half` and logs out still has their pickets hunt the weak,
//! decline the unwinnable, retreat when reinforcements turn a fight, and re-route
//! supply when a frontier system is lost — all decided server-side from the same
//! fog-respecting sensing the autonomous defence already uses.
//!
//! Setting doctrine is INSTANT local administration: it mutates only the corp's
//! own private policy and reveals nothing to rivals. The SHIPS it commands stay
//! sub-light, raidable, and light-revealed like any other — presence confers no
//! advantage, the async-persistent invariant.
//!
//! Every default is TODAY'S behaviour, so a corp that never touches its doctrine
//! plays exactly as before (the additive constraint): `DefensiveOnly` + `Never` +
//! `GuardNearest` + `Drop` == the pre-Layer-2 autonomous defence and supply loop.

use serde::{Deserialize, Serialize};

/// When an autonomous picket raider breaks off patrol to engage a hostile it can
/// SENSE. Ordered most-passive → most-aggressive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementPolicy {
    /// Never engage autonomously. Pickets still escort/hold per [`EscortPolicy`],
    /// but never break off to fight — pure preservation (you still raid by hand).
    Avoid,
    /// Engage only a hostile on an intercept course toward a friendly convoy the
    /// picket is guarding. The default — today's autonomous-defence behaviour.
    #[default]
    DefensiveOnly,
    /// Defensive, PLUS proactively hunt a sensed hostile — but only when the local
    /// force ratio favours you (you outnumber the enemy raiders in your sensor
    /// bubble). Opportunistic aggression.
    EngageWeaker,
    /// Hunt ANY hostile raider you can sense, regardless of odds or whether it
    /// threatens a convoy. Aggressive area denial.
    EngageAny,
}

impl EngagementPolicy {
    /// Whether a [`EngagementPosture::WeaponsFree`] fleet may COMMIT to a target
    /// it has picked, given the local weighted force ratio (`f` = friendly, `h` =
    /// hostile combatant weight in the fleet's own bubble). The POSTURE picks WHO;
    /// this composes the corp policy's WHETHER, so a favourable-only doctrine
    /// shadows an unfavourable contact instead of suiciding into it:
    ///   * `Avoid` vetoes ALL autonomous offense (the posture can't override a
    ///     doctrine that says "never engage autonomously");
    ///   * `DefensiveOnly` / `EngageWeaker` commit only when FAVOURABLE (`f > h`);
    ///   * `EngageAny` commits at any odds.
    ///
    /// The retreat-threshold gate is applied separately by the caller (unchanged),
    /// so `EngageAny` + a retreat threshold still declines a losing fight.
    pub fn weapons_free_commits(self, f: f64, h: f64) -> bool {
        match self {
            EngagementPolicy::Avoid => false,
            EngagementPolicy::DefensiveOnly | EngagementPolicy::EngageWeaker => f > h,
            EngagementPolicy::EngageAny => true,
        }
    }
}

/// A per-FLEET engagement POSTURE (§offensive-orders): standing aggression a
/// player delegates to ONE fleet in advance — the on-theme answer to command lag,
/// since a distant fleet can act on its OWN local detection without a
/// command-center round trip. It COMPOSES with the corp [`FleetDoctrine`] (it does
/// not replace it): the posture picks WHO a fleet pursues, while the doctrine's
/// [`EngagementPolicy`] force-ratio + [`RetreatThreshold`] still decide WHETHER.
/// Ordered most-passive → most-aggressive. serde default = `Passive`, so every
/// existing fleet loads unchanged (byte-preserving).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementPosture {
    /// Fight only if engaged; take no autonomous offensive action. Defers to the
    /// corp doctrine for any defensive picketing. The default — today's behaviour.
    #[default]
    Passive,
    /// Defensive posture — the existing picket behaviour (defend a guarded
    /// asset/station); no proactive hunting. Equivalent to Passive for the new
    /// offensive layer; named for intent and future per-fleet defensive control.
    Defensive,
    /// WEAPONS-FREE: on detecting any rival fleet within the fleet's OWN sensor
    /// bubble, auto-commit an intercept — a lone convoy is raided (cargo seized),
    /// anything armed is attacked (full battle) — gated by the composed doctrine
    /// above. Requires the fleet to carry a raider (strike capability).
    WeaponsFree,
}

/// The friendly force-ratio at or below which a committing / engaged picket
/// withdraws home instead of fighting. The ratio is friendly ÷ (friendly +
/// hostile) raiders within the picket's own sensor bubble (it counts itself); it
/// is checked BEFORE committing and again WHILE engaged, so enemy reinforcements
/// can tip a fight and trigger a retreat mid-engagement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetreatThreshold {
    /// Retreat only when heavily outnumbered (ratio < 0.25, worse than ~1:3).
    Quarter,
    /// Retreat when outnumbered at all (ratio < 0.50).
    Half,
    /// Hold only with a clear edge — retreat unless ratio ≥ 0.75 (~3:1).
    ThreeQuarter,
    /// Never retreat for odds — fight regardless. The default (today's behaviour).
    #[default]
    Never,
}

impl RetreatThreshold {
    /// Minimum friendly force-ratio required to commit / stay engaged. `None`
    /// means "never retreat for odds" (no gate at all).
    pub fn min_ratio(self) -> Option<f64> {
        match self {
            RetreatThreshold::Quarter => Some(0.25),
            RetreatThreshold::Half => Some(0.50),
            RetreatThreshold::ThreeQuarter => Some(0.75),
            RetreatThreshold::Never => None,
        }
    }
}

/// What a patrolling picket adopts as its charge — the convoy it shadows, and (for
/// [`EngagementPolicy::DefensiveOnly`]) the convoy it defends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscortPolicy {
    /// Shadow the nearest friendly convoy in range. The default (today's behaviour).
    #[default]
    GuardNearest,
    /// Shadow the most-laden friendly convoy in range — protect your richest
    /// shipments first.
    GuardRichest,
    /// Don't shadow anything; hold the patrol route the player set (a fixed
    /// chokepoint picket). Still defends a convoy that passes through its bubble.
    HoldStation,
}

/// What an automated supply convoy does when its destination is no longer valid
/// on arrival (the corp no longer owns the destination system — it was lost or
/// taken mid-transit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DestinationInvalidPolicy {
    /// Cargo is lost — the frontier risk of automated supply. The default.
    #[default]
    Drop,
    /// Re-route the convoy home (still raidable on the return leg); deposits into
    /// home inventory on arrival.
    ReturnHome,
    /// Re-route the convoy to the hub (still raidable); sells at the
    /// price-on-arrival.
    SellAtHub,
}

/// A corporation's standing combat & logistics doctrine. Pure menu of enums, so
/// it is `Copy` + `Eq` and trivially deterministic. Every field defaults to the
/// pre-Layer-2 behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FleetDoctrine {
    #[serde(default)]
    pub engagement: EngagementPolicy,
    #[serde(default)]
    pub retreat: RetreatThreshold,
    #[serde(default)]
    pub escort: EscortPolicy,
    #[serde(default)]
    pub destination_invalid: DestinationInvalidPolicy,
}

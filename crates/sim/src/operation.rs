//! Repeatable contracts, competitive objectives, and syndicate operations.
//!
//! One typed engine connects existing combat, survey, logistics, Authority,
//! strategic-node and syndicate events. Ground-truth progress is distinct from
//! [`KnownOperation`]: remote progress queues a report from the event site and
//! only that arrived copy is served to a corporation. This preserves the game's
//! central rule while avoiding one bespoke progress clock per contract type.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, OperationId, PlayerId, SyndicateId};
use crate::math::Vec2;

pub const OPERATION_REFRESH_S: f64 = 30.0;
pub const PRIVATE_OFFER_LIFETIME_S: f64 = 15.0 * 60.0;
pub const ACTIVE_CONTRACT_LIFETIME_S: f64 = 30.0 * 60.0;
pub const SALVAGE_LIFETIME_S: f64 = 10.0 * 60.0;
pub const SALVAGE_RECOVERY_RADIUS: f64 = 300.0;
pub const ESCORT_RADIUS: f64 = 1_200.0;
pub const CONTROL_HOLD_S: f64 = 3.0 * 60.0;
/// Optional beginner jobs repeat, but never flood the founding checklist.
pub const FOLLOW_UP_COOLDOWN_S: f64 = 10.0 * 60.0;
pub const FOLLOW_UP_PIRATE_SPEED_MULT: f64 = 0.50;
pub const FOLLOW_UP_PIRATE_DAMAGE_MULT: f64 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowUpKind { Escort, Salvage, Production, Patrol, DangerousFreight, GuardedSalvage, Depot, Stronghold,
    CounterRaidTrace, CounterRaidAssault, CounterRaidRecovery, SiteRecovery, CombatPreparation }

/// Private linkage, not a new intelligence channel. Each stage publishes an
/// immutable offer only through its own report from the physical event site.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CounterRaid {
    pub raid: EntityId,
    pub home: EntityId,
    pub source: EntityId,
}

/// Published contract terms, not a reading of unseen enemy strength. Immutable
/// after posting; actual progress and enemy sightings use their usual light.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationBriefing {
    pub follow_up: FollowUpKind,
    pub title: String,
    pub difficulty: String,
    pub suitable_fleets: String,
    pub summary: String,
    /// Fixed public contract variant, not a reading of the player's strength.
    #[serde(default)]
    pub variant: u8,
}

/// Physical encounter bookkeeping, deliberately NOT included in OperationView.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FollowUpEncounter {
    pub protected_fleet: Option<EntityId>,
    pub staged_at_home: bool,
    pub launched: bool,
    pub pirates: Vec<EntityId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Offered,
    Active,
    Completed,
    Failed,
    Expired,
    Abandoned,
}

impl OperationState {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Expired | Self::Abandoned
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationIssuer {
    Authority,
    Market,
    SurveyOffice,
    SalvageOffice,
    RegionalCouncil,
    Syndicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum OperationScope {
    Private { player: PlayerId },
    Public,
    Syndicate { syndicate: SyndicateId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationKind {
    /// Frozen scout/Authority report. Progress is still served from KnownOperation.
    CombatObjective { objective: crate::pirate::CombatObjective, site: EntityId, pos: Vec2,
        destination: Option<Vec2> },
    PrivateerPatrol { pos: Vec2 },
    PirateBounty {
        system: EntityId,
        tier: u32,
    },
    SurveyExpedition {
        system: EntityId,
    },
    MarketDelivery {
        commodity: Commodity,
        units: u32,
    },
    RescueSalvage {
        pos: Vec2,
        commodity: Commodity,
        units: u32,
        source_fleet: EntityId,
    },
    /// Immutable terms of a finite cache at a cleared permanent pirate site.
    /// Equipment is loaded on the recovering hull; only its data can radio home.
    PrizeRecovery { pos: Vec2, system: EntityId, prize: crate::pirate::SitePrize },
    ConvoyEscort {
        protected_fleet: EntityId,
        destination: Vec2,
    },
    /// Player chooses the Freighter and its guard; neither is conjured or moved
    /// by accepting. Guard and travel remain ordinary delayed fleet commands.
    FreightEscort { origin: Vec2, destination: Vec2 },
    AuthorityEnforcement {
        target: PlayerId,
    },
    StrategicControl {
        system: EntityId,
    },
    RegionalMandate {
        region: Vec2,
        radius: f64,
    },
    SyndicateMegaproject {
        system: EntityId,
        stage: u8,
    },
}

impl OperationKind {
    pub fn target_pos(&self, systems: &[(EntityId, Vec2)], hub: Vec2) -> Vec2 {
        let system_pos = |id| {
            systems
                .iter()
                .find_map(|(sid, pos)| (*sid == id).then_some(*pos))
                .unwrap_or(hub)
        };
        match *self {
            Self::CombatObjective { pos, .. } => pos,
            Self::PirateBounty { system, .. }
            | Self::SurveyExpedition { system }
            | Self::StrategicControl { system }
            | Self::SyndicateMegaproject { system, .. } => system_pos(system),
            Self::MarketDelivery { .. } => hub,
            Self::RescueSalvage { pos, .. } | Self::PrizeRecovery { pos, .. } | Self::PrivateerPatrol { pos } => pos,
            Self::ConvoyEscort { destination, .. } | Self::FreightEscort { destination, .. } => destination,
            Self::AuthorityEnforcement { .. } => hub,
            Self::RegionalMandate { region, .. } => region,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct OperationReward {
    pub credits: f64,
    pub authority_standing: f64,
    pub captain_xp: u32,
    pub research_insight: f64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Contribution {
    pub progress: u32,
    pub goods: u32,
    pub combat: u32,
    pub exploration: u32,
    pub escort: u32,
}

impl Contribution {
    pub fn score(self) -> u64 {
        self.progress as u64
            + self.goods as u64
            + self.combat as u64
            + self.exploration as u64
            + self.escort as u64
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KnownOperation {
    pub state: OperationState,
    pub progress: u32,
    pub goal: u32,
    pub stage: u8,
    /// Emission time of the report whose picture this is.
    pub reported_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: OperationId,
    pub issuer: OperationIssuer,
    pub scope: OperationScope,
    pub kind: OperationKind,
    pub state: OperationState,
    pub offered_at: f64,
    pub starts_at: f64,
    pub expires_at: f64,
    pub goal: u32,
    pub progress: u32,
    pub reward: OperationReward,
    #[serde(default)]
    pub briefing: Option<OperationBriefing>,
    #[serde(default)]
    pub encounter: Option<FollowUpEncounter>,
    #[serde(default)]
    pub counter_raid: Option<CounterRaid>,
    #[serde(default)]
    pub participants: BTreeSet<PlayerId>,
    #[serde(default)]
    pub assigned_fleets: BTreeMap<PlayerId, EntityId>,
    #[serde(default)]
    pub contributions: BTreeMap<PlayerId, Contribution>,
    #[serde(default)]
    pub known: BTreeMap<PlayerId, KnownOperation>,
    #[serde(default)]
    pub completed_at: Option<f64>,
    #[serde(default)]
    pub winner: Option<PlayerId>,
    /// Rewards are credited only when the completion report reaches each
    /// recipient. Keeping this ledger on the operation prevents a remote wallet,
    /// research bar, or captain promotion from revealing an unseen outcome.
    #[serde(default)]
    pub rewards_paid: BTreeSet<PlayerId>,
    #[serde(default)]
    pub stage: u8,
    #[serde(default)]
    pub hold_since: Option<f64>,
}

impl Operation {
    pub fn is_visible_to(&self, player: PlayerId) -> bool {
        self.known.contains_key(&player)
    }

    pub fn accepts(&self, player: PlayerId, syndicate: Option<SyndicateId>) -> bool {
        !self.state.terminal()
            && match self.scope {
                OperationScope::Private { player: owner } => owner == player,
                OperationScope::Public => true,
                OperationScope::Syndicate { syndicate: sid } => syndicate == Some(sid),
            }
    }

    pub fn contribution_mut(&mut self, player: PlayerId) -> &mut Contribution {
        self.contributions.entry(player).or_default()
    }

    pub fn knowledge(&self) -> KnownOperation {
        KnownOperation {
            state: self.state,
            progress: self.progress,
            goal: self.goal,
            stage: self.stage,
            reported_at: self.completed_at.unwrap_or(self.offered_at),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOperationReport {
    pub operation: OperationId,
    pub recipient: PlayerId,
    pub snapshot: KnownOperation,
    pub arrive_at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MidgameStage {
    #[default]
    HomeDevelopment,
    Exploration,
    Specialization,
    FirstColony,
    TradeNetwork,
    ContestedExpansion,
    RegionalPower,
}

impl MidgameStage {
    pub fn ordinal(self) -> u8 {
        match self {
            Self::HomeDevelopment => 0,
            Self::Exploration => 1,
            Self::Specialization => 2,
            Self::FirstColony => 3,
            Self::TradeNetwork => 4,
            Self::ContestedExpansion => 5,
            Self::RegionalPower => 6,
        }
    }
}

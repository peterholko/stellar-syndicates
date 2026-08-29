//! Formal corporation-to-corporation diplomacy.
//!
//! Diplomacy is ground-truth policy plus a separate, light-delayed known picture.
//! A declaration never grants an FTL alert: the target learns it after the
//! command-center-to-command-center wavefront, and territorial war cannot begin
//! before that notice plus [`WAR_NOTICE_GRACE_S`]. Mutual treaties take effect
//! when accepted because both parties consent; the acceptor's response still
//! travels back before the proposer sees it in their panel.

use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;

/// Playtest notice window after a declaration reaches its target. Production
/// pacing can lengthen this without changing the mechanic.
pub const WAR_NOTICE_GRACE_S: f64 = 60.0;
/// Treaty proposals expire so an ancient offer cannot be accepted after the
/// strategic picture that produced it has disappeared.
pub const TREATY_OFFER_LIFETIME_S: f64 = 15.0 * 60.0;
/// Former syndicate members cannot attack one another during separation.
pub const SYNDICATE_SEPARATION_S: f64 = 5.0 * 60.0;
/// A right of reprisal created by neutral-space aggression.
pub const REPRISAL_S: f64 = 10.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RelationState {
    #[default]
    Neutral,
    NonAggression,
    War,
    Ceasefire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatyKind {
    NonAggression,
    Ceasefire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiplomacyNoticeKind {
    Proposal,
    Accepted,
    Rejected,
    Declaration,
    Activated,
    Cancelled,
    Separation,
    Reprisal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreatyProposal {
    pub id: u64,
    pub from: PlayerId,
    pub to: PlayerId,
    pub kind: TreatyKind,
    pub proposed_at: f64,
    pub expires_at: f64,
    /// The target may not accept before the proposal's light reaches its CC.
    pub visible_to_target_at: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PendingWar {
    pub declarer: PlayerId,
    pub declared_at: f64,
    pub target_notified_at: f64,
    pub activates_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiplomaticRelation {
    pub a: PlayerId,
    pub b: PlayerId,
    #[serde(default)]
    pub state: RelationState,
    #[serde(default)]
    pub since: f64,
    #[serde(default)]
    pub pending_war: Option<PendingWar>,
    #[serde(default)]
    pub separation_until: f64,
    #[serde(default)]
    pub pending_cancel_at: Option<f64>,
    /// Corp allowed to retaliate without first declaring war, and until when.
    #[serde(default)]
    pub reprisal_for: Option<(PlayerId, f64)>,
    #[serde(default)]
    pub known_reprisal_until_a: Option<f64>,
    #[serde(default)]
    pub known_reprisal_until_b: Option<f64>,
    #[serde(default)]
    pub known_separation_until_a: f64,
    #[serde(default)]
    pub known_separation_until_b: f64,
    /// Each participant's last ARRIVED picture. These may differ while a message
    /// is in flight; serving reads them, never `state` directly.
    #[serde(default)]
    pub known_a: RelationState,
    #[serde(default)]
    pub known_b: RelationState,
}

impl DiplomaticRelation {
    pub fn new(a: PlayerId, b: PlayerId, now: f64) -> Self {
        debug_assert!(a < b);
        Self {
            a,
            b,
            state: RelationState::Neutral,
            since: now,
            pending_war: None,
            separation_until: 0.0,
            pending_cancel_at: None,
            reprisal_for: None,
            known_reprisal_until_a: None,
            known_reprisal_until_b: None,
            known_separation_until_a: 0.0,
            known_separation_until_b: 0.0,
            known_a: RelationState::Neutral,
            known_b: RelationState::Neutral,
        }
    }

    pub fn other(&self, player: PlayerId) -> Option<PlayerId> {
        (player == self.a)
            .then_some(self.b)
            .or_else(|| (player == self.b).then_some(self.a))
    }

    pub fn known_by(&self, player: PlayerId) -> RelationState {
        if player == self.a {
            self.known_a
        } else if player == self.b {
            self.known_b
        } else {
            RelationState::Neutral
        }
    }

    pub fn set_known(&mut self, player: PlayerId, state: RelationState) {
        if player == self.a {
            self.known_a = state;
        } else if player == self.b {
            self.known_b = state;
        }
    }

    pub fn known_reprisal_until(&self, player: PlayerId) -> Option<f64> {
        if player == self.a {
            self.known_reprisal_until_a
        } else if player == self.b {
            self.known_reprisal_until_b
        } else {
            None
        }
    }

    pub fn set_known_reprisal_until(&mut self, player: PlayerId, until: f64) {
        if player == self.a {
            self.known_reprisal_until_a = Some(until);
        } else if player == self.b {
            self.known_reprisal_until_b = Some(until);
        }
    }

    pub fn known_separation_until(&self, player: PlayerId) -> f64 {
        if player == self.a {
            self.known_separation_until_a
        } else if player == self.b {
            self.known_separation_until_b
        } else {
            0.0
        }
    }

    pub fn set_known_separation_until(&mut self, player: PlayerId, until: f64) {
        if player == self.a {
            self.known_separation_until_a = until;
        } else if player == self.b {
            self.known_separation_until_b = until;
        }
    }

    pub fn protected(&self, now: f64) -> bool {
        self.separation_until > now
            || matches!(self.state, RelationState::NonAggression | RelationState::Ceasefire)
    }

    pub fn territorial_war_for(&self, actor: PlayerId, now: f64) -> bool {
        self.state == RelationState::War
            || self
                .reprisal_for
                .is_some_and(|(holder, until)| holder == actor && until > now)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDiplomacyNotice {
    pub recipient: PlayerId,
    pub other: PlayerId,
    pub state: RelationState,
    pub kind: DiplomacyNoticeKind,
    pub arrive_at: f64,
}

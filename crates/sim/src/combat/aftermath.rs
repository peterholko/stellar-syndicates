//! Frozen, owner-only battle results. These are observers, never combat inputs.
//! The server selects ONE side and releases it with the conclusion's light;
//! current fleets, repairs and later promotions must not rewrite this history.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::{EntityId, ShipKind};
use crate::captain::{CaptainPortrait, CaptainSighting};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BattleAftermath {
    pub survivors: Vec<SurvivingFleet>,
    /// Encounter bounty earned, not a second credit payment. Tutorial payout
    /// remains in the existing light-gated founding programme; ordinary fights
    /// do not manufacture credits. Separate contract rewards remain contracts.
    pub bounty_credits: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurvivingFleet {
    pub fleet_id: EntityId,
    pub kind: ShipKind,
    pub composition: BTreeMap<ShipKind, u32>,
    /// HP-weighted remaining hull, 0..1, at battle end (or at withdrawal).
    pub hull: f64,
    pub withdrew: bool,
    pub guard_target: Option<EntityId>,
    pub captain: Option<BattleCaptainGain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleCaptainGain {
    pub id: u32,
    pub name: String,
    pub portrait: CaptainPortrait,
    pub before: CaptainSighting,
    pub after: CaptainSighting,
}

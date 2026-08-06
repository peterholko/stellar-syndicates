//! The new-corporation founding programme.
//!
//! This is authoritative simulation state, not a client checklist. Milestones
//! advance from things that actually happened in the world, so reconnecting or
//! playing from another client cannot skip the opening lessons.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::EntityId;

/// Earliest point at which a corporation may deliberately end founder safety.
pub const FOUNDER_PROTECTION_MIN_S: f64 = 30.0 * 60.0;
/// Founder safety is never permanent, even if the programme is abandoned.
pub const FOUNDER_PROTECTION_MAX_S: f64 = 24.0 * 60.0 * 60.0;

/// The Academy does real work for the first programme, but the founding grant
/// leaves only this much Tier-I throughput to fund. At the fast 4× playtest
/// pacing a staffed Tier-I Academy therefore produces the first result in about
/// three wall minutes; later programmes use the ordinary season-scale costs.
pub const FIRST_RESEARCH_REMAINING_S: f64 = 12.0 * 60.0;

/// Three legible opening choices surfaced by the guide. They remain ordinary
/// catalogue programmes: the player may ignore them and choose any open Tier I.
pub const RECOMMENDED_FIRST_RESEARCH: [&str; 3] =
    ["prop_drive_tuning", "mat_deep_bores", "life_med_bays"];

/// The ordered opening arc. Each stage teaches one system before unlocking the
/// next; expansion arrives only after the player has built, travelled, fought,
/// hauled, traded, researched, compared two surveys, and committed a colony.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingStage {
    BuildShipyard,
    LeaveHomeWell,
    DefeatPrivateer,
    BuildMine,
    BuildConvoy,
    FirstSale,
    BuildAcademy,
    FirstResearch,
    BuildScout,
    #[serde(alias = "survey_system")]
    SurveyCandidates,
    BuildColony,
    EstablishColony,
    Complete,
}

impl Default for FoundingStage {
    fn default() -> Self {
        Self::Complete
    }
}

/// Persistent per-corporation progress. `Default` is deliberately complete and
/// disabled: old snapshots must not be pushed backwards into onboarding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundingProgram {
    #[serde(default)]
    pub stage: FoundingStage,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub started_at: f64,
    #[serde(default)]
    pub interceptor: Option<EntityId>,
    #[serde(default)]
    pub privateer: Option<EntityId>,
    /// When evidence of the Interceptor crossing the home well reaches the CC.
    #[serde(default)]
    pub departure_report_at: Option<f64>,
    #[serde(default)]
    pub privateer_report_at: Option<f64>,
    #[serde(default)]
    pub reward_granted: bool,
    /// When the first Market Hub sale receipt reaches the command center.
    #[serde(default)]
    pub sale_report_at: Option<f64>,
    /// A player-owned Convoy has physically delivered goods to the Market Hub.
    /// An instant sale of bounty inventory cannot satisfy the hauling lesson.
    #[serde(default)]
    pub market_haul_completed: bool,
    #[serde(default)]
    pub initial_surveyed: usize,
    /// The two nearby, contrasting systems assigned to this founding chapter:
    /// population-led first, mineral-led second. Their exact geology remains
    /// survey-gated; the ids merely make the tutorial objective deterministic.
    #[serde(default)]
    pub survey_candidates: Vec<EntityId>,
    #[serde(default)]
    pub initial_owned_systems: usize,
    #[serde(default)]
    pub initial_research_completed: usize,
    /// The first programme gets one transparent founding grant; never repeated
    /// by changing queues, reconnecting, or joining/leaving a syndicate. The
    /// grant covers the first programme's basket as well as most of its clock.
    #[serde(default)]
    pub research_grant_applied: bool,
    /// The first two prospect reports authorize one exact Colony Ship kit at
    /// the Market Warehouse. The player still has to move it home and build it.
    #[serde(default)]
    pub colony_kit_granted: bool,
    /// Sim timestamps at which each authoritative milestone first became true.
    /// This is the live-play and headless balance instrument: no UI estimates.
    #[serde(default)]
    pub milestones: BTreeMap<FoundingStage, f64>,
    #[serde(default)]
    pub protection_broken: bool,
}

impl Default for FoundingProgram {
    fn default() -> Self {
        Self {
            stage: FoundingStage::Complete,
            enabled: false,
            started_at: 0.0,
            interceptor: None,
            privateer: None,
            departure_report_at: None,
            privateer_report_at: None,
            reward_granted: false,
            sale_report_at: None,
            market_haul_completed: false,
            initial_surveyed: 0,
            survey_candidates: Vec::new(),
            initial_owned_systems: 0,
            initial_research_completed: 0,
            research_grant_applied: false,
            colony_kit_granted: false,
            milestones: BTreeMap::new(),
            protection_broken: false,
        }
    }
}

impl FoundingProgram {
    pub fn new(
        now: f64,
        initial_surveyed: usize,
        initial_owned_systems: usize,
        survey_candidates: Vec<EntityId>,
    ) -> Self {
        let mut programme = Self {
            stage: FoundingStage::BuildShipyard,
            enabled: true,
            started_at: now,
            initial_surveyed,
            initial_owned_systems,
            survey_candidates,
            ..Self::default()
        };
        programme
            .milestones
            .insert(FoundingStage::BuildShipyard, now);
        programme
    }

    /// Advance once and stamp the fact. Keeping this mutation in one helper is
    /// what makes live telemetry and the headless harness read the same clock.
    pub fn set_stage(&mut self, next: FoundingStage, now: f64) {
        if self.stage == next {
            return;
        }
        self.stage = next;
        self.milestones.entry(next).or_insert(now);
    }

    pub fn expansion_unlocked(&self) -> bool {
        !self.enabled
            || matches!(
                self.stage,
                FoundingStage::BuildColony
                    | FoundingStage::EstablishColony
                    | FoundingStage::Complete
            )
    }

    /// Founder safety has a minimum clock and also cannot be voluntarily ended
    /// before the privateer outcome's light reaches the command center. It then
    /// lasts until the player chooses aggression, with a hard 24-hour ceiling.
    pub fn protected(&self, now: f64) -> bool {
        self.enabled && !self.protection_broken && now < self.started_at + FOUNDER_PROTECTION_MAX_S
    }

    pub fn may_break_protection(&self, now: f64) -> bool {
        self.enabled
            && now >= self.started_at + FOUNDER_PROTECTION_MIN_S
            && self.privateer_report_at.is_some_and(|at| now >= at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn founder_safety_ends_only_by_eligible_aggression_or_the_hard_ceiling() {
        let mut programme = FoundingProgram::new(10.0, 0, 1, Vec::new());
        let after_min = programme.started_at + FOUNDER_PROTECTION_MIN_S + 1.0;

        assert!(programme.protected(after_min));
        assert!(!programme.may_break_protection(after_min));

        programme.privateer_report_at = Some(after_min - 1.0);
        assert!(programme.protected(after_min));
        assert!(programme.may_break_protection(after_min));

        programme.protection_broken = true;
        assert!(!programme.protected(after_min));

        programme.protection_broken = false;
        assert!(!programme.protected(programme.started_at + FOUNDER_PROTECTION_MAX_S));
    }
}

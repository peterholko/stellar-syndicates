//! The new-corporation founding programme.
//!
//! This is authoritative simulation state, not a client checklist. Milestones
//! advance from things that actually happened in the world, so reconnecting or
//! playing from another client cannot skip the opening lessons.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{cargo::Commodity, ids::EntityId};

/// Earliest point at which a corporation may deliberately end founder safety.
pub const FOUNDER_PROTECTION_MIN_S: f64 = 30.0 * 60.0;
/// Founder safety always lasts at least this long.
pub const FOUNDER_PROTECTION_BASE_S: f64 = 24.0 * 60.0 * 60.0;
/// Once the programme completes, safety continues this long (never shorter
/// than the base window), so a founder who plays once a day is not exposed the
/// moment they finish.
pub const FOUNDER_PROTECTION_AFTER_COMPLETE_S: f64 = 24.0 * 60.0 * 60.0;
/// Founder safety is never permanent, even if the programme is abandoned.
pub const FOUNDER_PROTECTION_MAX_S: f64 = 48.0 * 60.0 * 60.0;

/// §founding R3: the local-opening Privateer appears this far ahead along the
/// guarded Freighter's route (at most 60% of the leg) and this far to its side:
/// inside ordinary sensors, so the approach is visible and the escort meets it.
pub const LOCAL_PRIVATEER_AHEAD_SU: f64 = 9_000.0;
pub const LOCAL_PRIVATEER_OFFSET_SU: f64 = 6_000.0;

/// §founding R6: the Authority's small charter stipend for first reaching a
/// charter milestone, named for the step just finished. Credits are money,
/// never conjured goods; the physical economy stays the game. The Rogue
/// Privateer pays its own bounty instead.
pub fn charter_stipend(stage: FoundingStage) -> Option<(&'static str, f64)> {
    Some(match stage {
        FoundingStage::PrepareExport => ("Mine online", 300.0),
        FoundingStage::DefeatPrivateer => ("First shipment under way", 500.0),
        FoundingStage::FirstResearch => ("Shipyard staffed", 400.0),
        FoundingStage::SurveyCandidates => ("Research under way", 300.0),
        FoundingStage::BuildBarge => ("Nearby system charted", 500.0),
        FoundingStage::MineBelt => ("Mining Barge launched", 300.0),
        FoundingStage::HaulOre => ("Belt mining under way", 300.0),
        FoundingStage::Complete => ("Founding charter fulfilled", 800.0),
        _ => return None,
    })
}

/// The Academy does real work for the first programme, but the founding grant
/// leaves only this much Tier-I throughput to fund. At the fast 4× playtest
/// pacing a staffed Tier-I Academy therefore produces the first result in about
/// three wall minutes; later programmes use the ordinary season-scale costs.
pub const FIRST_RESEARCH_REMAINING_S: f64 = 12.0 * 60.0;

/// The Rogue Privateer pays money, never a conjured construction manifest. This
/// is the rounded base-market value of the retired mixed-goods bounty plus its
/// old cash award: enough purchasing power for the founding imports, while the
/// player still has to choose, buy, freight, and receive every manufactured good.
pub const PRIVATEER_CREDIT_BOUNTY: f64 = 4_500.0;

/// The tutorial threat starts well downrange. Its lateral offset and slow
/// convergence make the ordinary 20,000-su Freighter contact a useful warning
/// rather than a one-second ambush at 4× pacing.
pub const PRIVATEER_LEAD_SU: f64 = 45_000.0;
/// Lateral displacement from the Freighter's actual outbound route. The threat
/// must first turn and converge instead of spawning directly in its path.
pub const PRIVATEER_ROUTE_OFFSET_SU: f64 = 25_000.0;
/// A limping tutorial Raider: barely faster than a Convoy, less than half the
/// starting Interceptor's speed. Ordinary enclave pirates remain untouched.
pub const PRIVATEER_SPEED_MULT: f64 = 0.45;
/// Start undamaged so players see combat take the privateer from full hull
/// to destruction. Tutorial safety comes from weak weapons, not missing HP.
/// Ordinary enclave pirates are unaffected.
pub const PRIVATEER_HULL_FRAC: f64 = 1.0;
/// Its improvised weapons are deliberately weak. A caught Freighter therefore
/// takes damage over time instead of being erased by the ordinary raid burst,
/// leaving a real relief window for the player's Interceptor. Full starting
/// hull gives it staying power without increasing its damage per hit.
pub const PRIVATEER_DAMAGE_MULT: f64 = 0.05;

/// Three legible opening choices surfaced by the guide. They remain ordinary
/// catalogue programmes: the player may ignore them and choose any open Tier I.
pub const RECOMMENDED_FIRST_RESEARCH: [&str; 3] =
    ["comp_shadow_iv_recon_suite", "prop_expedition_iii_extended_tanks", "mat_deep_bores"];

/// The founding charter (v3), nine steps in three chapters:
/// - Landfall: BuildMine → PrepareExport (the first escorted shipment) →
///   DefeatPrivateer (it ambushes that shipment).
/// - Industry: BuildShipyard → FirstResearch (research started at a staffed
///   Academy) → SurveyCandidates (chart the local stop with a probe).
/// - Reach: BuildBarge → MineBelt → HaulOre (the barge's ore delivered).
///
/// BuildConvoy remains the lost-starter recovery step. Keep enum order stable
/// for saved identities: later stages are appended, not inserted in play order.
/// Retired stages (the Scout, the beacon, the investment choice, colonies)
/// advance to their successors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingStage {
    BuildShipyard,
    BuildMine,
    BuildConvoy,
    /// Prepare the opening export. The first meaningful departure by ANY owned
    /// Freighter starts the privateer lesson regardless of its manifest; the
    /// Ferrite Ore sale receipt remains a separate later milestone.
    #[serde(alias = "leave_home_well")]
    ExportProduction,
    DefeatPrivateer,
    /// Wait for a Ferrite Ore sale receipt to reach the command center.
    #[serde(alias = "first_sale")]
    CompleteExport,
    BuildAcademy,
    FirstResearch,
    BuildScout,
    #[serde(alias = "survey_system")]
    SurveyCandidates,
    BuildColony,
    EstablishColony,
    Complete,
    BuildSecondFreighter,
    /// Two suggested investments, not a permanent class choice. Either a second
    /// owned freight hull or an operating researched Smelter completes the chapter.
    GrowBusiness,
    /// Version-two opening: a short, local recovery before the Hub voyage.
    LocalRecovery,
    PrepareExport,
    /// §belts: build a Mining Barge, once the surveys have charted a belt.
    BuildBarge,
    /// §belts: send it to the charted belt; done when command hears that it
    /// is cutting ore there.
    MineBelt,
    /// §charter v3: bring the barge's ore home and unload it, or sell it at
    /// the Hub; done when command hears it was delivered.
    HaulOre,
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
    /// Old saves retain their existing arc. Only first joins opt into v2.
    #[serde(default)]
    pub local_opening: bool,
    #[serde(default)]
    pub recovery_site: Option<EntityId>,
    #[serde(default)]
    pub scout: Option<EntityId>,
    /// Arrival at command, not the time the cache was physically recovered.
    #[serde(default)]
    pub recovery_report_at: Option<f64>,
    #[serde(default)]
    pub export_dispatched_at: Option<f64>,
    #[serde(default)]
    pub local_ready_report_at: Option<f64>,
    #[serde(default)]
    pub stage: FoundingStage,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub started_at: f64,
    #[serde(default)]
    pub interceptor: Option<EntityId>,
    /// Initially the granted Tiny Freighter; rebound to whichever owned
    /// Freighter actually departs first and becomes the privateer's target.
    #[serde(default)]
    pub convoy: Option<EntityId>,
    #[serde(default)]
    pub privateer: Option<EntityId>,
    /// Legacy standalone-flight report, retained for snapshot compatibility.
    #[serde(default)]
    pub departure_report_at: Option<f64>,
    #[serde(default)]
    pub privateer_report_at: Option<f64>,
    #[serde(default)]
    pub reward_granted: bool,
    /// When the first Ferrite Ore sale receipt reaches the command center.
    /// Retains its save-field name so old snapshots migrate without a custom pass.
    #[serde(default)]
    pub sale_report_at: Option<f64>,
    /// Sale-report arrivals. Old saves may also contain Provisions; only Metallic
    /// Ore is required now. Progress follows receipt light, never Market-Hub truth.
    #[serde(default)]
    pub opening_export_reports: BTreeMap<Commodity, f64>,
    /// Legacy delivery ledger, retained for snapshot compatibility. Completing
    /// the export now requires only an executed Ore sale, not a separate delivery.
    #[serde(default)]
    pub opening_export_deliveries: BTreeSet<Commodity>,
    /// Legacy v1 physical-haul bit. Retained only for snapshot compatibility;
    /// new corporations use the Ferrite Ore sale receipt above.
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
    /// Legacy colony-kit ledger. New completions grant no kit; already-awarded
    /// goods and this flag survive saves unchanged.
    #[serde(default)]
    pub colony_kit_granted: bool,
    /// Sim timestamps at which each authoritative milestone first became true.
    /// This is the live-play and headless balance instrument: no UI estimates.
    #[serde(default)]
    pub milestones: BTreeMap<FoundingStage, f64>,
    #[serde(default)]
    pub protection_broken: bool,
    /// §founding R3: the Authority's one replacement starter Freighter.
    #[serde(default)]
    pub freighter_replaced: bool,
    /// §founding R6: charter stipends already paid, by milestone.
    #[serde(default)]
    pub stipends: BTreeMap<FoundingStage, f64>,
    /// §founding R7: the latest hull that turned back from the recovery site
    /// because of a threat, and when that report reaches command. Cleared
    /// when a hull is sent to the site again.
    #[serde(default)]
    pub turned_back: Option<(EntityId, f64)>,
    /// §belts: the charted belt (system, belt body) the barge lesson points
    /// at, chosen when the surveys finish: a surveyed prospect's belt first.
    #[serde(default)]
    pub belt_target: Option<(EntityId, u32)>,
    /// §belts: when the report that a barge is cutting ore at a belt away
    /// from home reaches the command center.
    #[serde(default)]
    pub belt_report_at: Option<f64>,
    /// §charter v3: when the report that a barge delivered its ore (to a
    /// stockpile or the Market warehouse) reaches the command center.
    #[serde(default)]
    pub haul_report_at: Option<f64>,
}

impl Default for FoundingProgram {
    fn default() -> Self {
        Self {
            local_opening: false,
            recovery_site: None,
            scout: None,
            recovery_report_at: None,
            export_dispatched_at: None,
            local_ready_report_at: None,
            stage: FoundingStage::Complete,
            enabled: false,
            started_at: 0.0,
            interceptor: None,
            convoy: None,
            privateer: None,
            departure_report_at: None,
            privateer_report_at: None,
            reward_granted: false,
            sale_report_at: None,
            opening_export_reports: BTreeMap::new(),
            opening_export_deliveries: BTreeSet::new(),
            market_haul_completed: false,
            initial_surveyed: 0,
            survey_candidates: Vec::new(),
            initial_owned_systems: 0,
            initial_research_completed: 0,
            research_grant_applied: false,
            colony_kit_granted: false,
            milestones: BTreeMap::new(),
            protection_broken: false,
            freighter_replaced: false,
            stipends: BTreeMap::new(),
            turned_back: None,
            belt_target: None,
            belt_report_at: None,
            haul_report_at: None,
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
            stage: FoundingStage::BuildMine,
            enabled: true,
            started_at: now,
            initial_surveyed,
            initial_owned_systems,
            survey_candidates,
            ..Self::default()
        };
        programme
            .milestones
            .insert(FoundingStage::BuildMine, now);
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
                    | FoundingStage::BuildBarge
                    | FoundingStage::MineBelt
                    | FoundingStage::HaulOre
                    | FoundingStage::Complete
            )
    }

    /// Founder safety has a minimum clock and also cannot be voluntarily ended
    /// before the privateer outcome's light reaches the command center. It then
    /// lasts until the player chooses aggression or [`Self::protection_until`].
    pub fn protected(&self, now: f64) -> bool {
        self.enabled && !self.protection_broken && now < self.protection_until()
    }

    /// When founder safety ends on its own: 24 hours after completing the
    /// programme (never before the 24-hour base), and always by the 48-hour
    /// ceiling. An unfinished programme keeps the full ceiling.
    pub fn protection_until(&self) -> f64 {
        let ceiling = self.started_at + FOUNDER_PROTECTION_MAX_S;
        match self.milestones.get(&FoundingStage::Complete) {
            Some(done) => (done + FOUNDER_PROTECTION_AFTER_COMPLETE_S)
                .max(self.started_at + FOUNDER_PROTECTION_BASE_S)
                .min(ceiling),
            None => ceiling,
        }
    }

    /// §founding R8: the Landfall chapter, from settling until the first
    /// guarded export leaves home. Contract offers stay few and patient here.
    pub fn landfall_focus(&self) -> bool {
        self.enabled
            && self.local_opening
            && self.export_dispatched_at.is_none()
            && self.stage != FoundingStage::Complete
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

    #[test]
    fn founder_safety_outlasts_the_programme_by_a_day_within_the_ceiling() {
        let hour = 3600.0;
        let mut programme = FoundingProgram::new(0.0, 0, 1, Vec::new());
        assert_eq!(programme.protection_until(), 48.0 * hour, "unfinished: full ceiling");
        programme.set_stage(FoundingStage::Complete, 2.0 * hour);
        assert_eq!(programme.protection_until(), 26.0 * hour, "done early: a day after completion");
        assert!(programme.protected(25.0 * hour) && !programme.protected(26.0 * hour));
        let mut late = FoundingProgram::new(0.0, 0, 1, Vec::new());
        late.set_stage(FoundingStage::Complete, 40.0 * hour);
        assert_eq!(late.protection_until(), 48.0 * hour, "never past the ceiling");
    }
}

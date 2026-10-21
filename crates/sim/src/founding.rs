//! The new-corporation founding programme.
//!
//! This is authoritative simulation state, not a client checklist. Milestones
//! advance from things that actually happened in the world, so reconnecting or
//! playing from another client cannot skip the opening lessons.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{build::StructureKind, cargo::Commodity, ids::EntityId, math::Vec2};

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

/// §charter-v5: the raid. The privateer sets out from beyond the colony's
/// sensors on its den's side, this many sensor ranges out (never past the den
/// itself), so it is seen within seconds of the mine coming online.
pub const RAID_START_SENSOR_RANGES: f64 = 1.25;
/// It is over the colony once inside its gravity well's edge, and loads this
/// many units a second: what it takes visibly drains in a quarter-minute.
pub const RAID_LOOT_PER_SEC: f64 = 4.0;
/// What it takes: every unit of these in the home stockpile.
pub const RAID_TAKES: [Commodity; 2] = [Commodity::Electronics, Commodity::Armaments];
/// It never holds orbit longer than this, whatever stops it loading.
pub const RAID_MAX_HOLD_S: f64 = 90.0;
/// After unloading at its den, it lurks this share of the way from the den
/// toward the colony: beyond the colony's sensors, but tracked.
pub const RAID_LURK_FRACTION: f64 = 0.35;

/// §founding R6: the Authority's small charter stipend for first reaching a
/// charter milestone, named for the step just finished. Credits are money,
/// never conjured goods; the physical economy stays the game. The Rogue
/// Privateer pays its own bounty instead. §charter-v5: the stipends before the
/// bounty leave the founder about 800 Cr short of buying back what the raid
/// took (50 semiconductors and 15 Armaments at the trader's markup), so the
/// ore has to be sold at the trader's discount.
pub fn charter_stipend(stage: FoundingStage) -> Option<(&'static str, f64)> {
    Some(match stage {
        FoundingStage::BuildMine => ("Colony fed and housed", 300.0),
        FoundingStage::PirateRaid => ("Mine online", 300.0),
        FoundingStage::BuildInterceptor => ("Shipyard staffed", 400.0),
        FoundingStage::DefeatPrivateer => ("Interceptor launched", 300.0),
        FoundingStage::PrepareExport => ("Privateer den eliminated", 600.0),
        FoundingStage::ExpandMining => ("Research under way", 300.0),
        FoundingStage::SurveyCandidates => ("Mining expanded", 400.0),
        FoundingStage::BuildBarge => ("Nearby system charted", 500.0),
        FoundingStage::MineBelt => ("Mining Barge launched", 300.0),
        FoundingStage::HaulOre => ("Belt mining under way", 300.0),
        FoundingStage::Complete => ("Founding charter fulfilled", 800.0),
        _ => return None,
    })
}

/// §charter-v5: the kit's semiconductors, all of which the raid takes. Tunable.
pub const KIT_SEMICONDUCTORS: f64 = 40.0;
/// §charter-v5: the kit's Armaments: one Interceptor's worth, which the raid
/// takes too. Tunable.
pub const KIT_ARMAMENTS: f64 = 15.0;
/// §charter-v4: the kit's Provisions: about five and a half minutes of food
/// for the 6,000 settlers while the farms go up, plus the Academy's 20. Tunable.
pub const KIT_PROVISIONS: f64 = 100.0;

/// §charter-v4: the privateer den. Its garrison flies the privateer's weak
/// guns: three raiders at three-quarter hull are a real fight that one
/// Interceptor wins (measured over eleven seeds: 27–54 s, at worst 45% hull
/// left). It has no defense platforms; one platform tier kills a lone
/// Interceptor. Tunable.
pub const DEN_GARRISON_RAIDERS: u32 = 3;
pub const DEN_GARRISON_HULL_FRAC: f64 = 0.75;
/// Goods left at the den when it falls, for a Freighter to recover.
pub const DEN_PLUNDER: [(crate::cargo::Commodity, u32); 2] =
    [(crate::cargo::Commodity::Alloys, 40), (crate::cargo::Commodity::Electronics, 15)];
/// The den's belt is rich: its rolled deposits times this. Tunable.
pub const DEN_BELT_RICHNESS_MULT: f64 = 1.8;
/// Destroying the privateer promotes the captain aboard the killing fleet: its
/// experience is topped up to this level's (the kill alone earns 85 of 100).
pub const PRIVATEER_PROMOTION_LEVEL: u8 = 2;

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

/// The founding charter (v5, `docs/founding-charter.md`), eighteen steps in
/// five chapters ([`FoundingStage::STEPS`]):
/// - Landfall: BuildBioharvester → BuildHabitat → BuildAgroplex → BuildMine.
/// - Raid: PirateRaid (the privateer takes the kit's semiconductors and
///   Armaments) → SellOre (to the free trader, which docks after the raid) →
///   PowerShipyard → BuildShipyard.
/// - Pirates: BuildInterceptor → DefeatPrivateer → EliminateDen (its base at
///   the local belt, holding what the raid took).
/// - Expansion: PrepareExport (the first ore to the Market Hub, now the lane
///   is safe) → FirstResearch → ExpandMining (a powered mine on another home
///   world) → SurveyCandidates.
/// - Reach: BuildBarge → MineBelt → HaulOre.
///
/// Keep enum order stable for saved identities: later stages are appended,
/// not inserted in play order. Retired stages (the Scout, the beacon, the
/// investment choice, colonies, the counterfeit, the Freighter export) advance
/// to their successors.
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
    /// §charter-v5: "Export your ore", after the den: done when the report of
    /// the first Ferrite Ore leaving for the Market Hub reaches command.
    PrepareExport,
    /// §belts: build a Mining Barge, once the surveys have charted a belt.
    BuildBarge,
    /// §belts: send it to the charted belt; done when command hears that it
    /// is cutting ore there.
    MineBelt,
    /// §charter v3: bring the barge's ore home and unload it, or sell it at
    /// the Hub; done when command hears it was delivered.
    HaulOre,
    /// §charter-v4: build and staff the first Bioharvester.
    BuildBioharvester,
    /// §charter-v4: build the first Habitat.
    BuildHabitat,
    /// §charter-v4: build and staff the first Agroplex.
    BuildAgroplex,
    /// §charter-v4: the Shipyard needs more than the Command Center's reactor:
    /// build a Solar Array.
    PowerShipyard,
    /// Retired (§charter-v5): the counterfeit's "buy real semiconductors".
    BuySemiconductors,
    /// Retired (§charter-v5): the first Freighter.
    BuildFreighter,
    /// §charter-v4: build an Interceptor (at Shipyard II).
    BuildInterceptor,
    /// §charter-v4: a powered, staffed mine on another world of the home system.
    ExpandMining,
    /// §charter-v4: the dead privateer's log named its den, a pirate base at
    /// the home's local belt; done when the report of its clearance reaches
    /// command.
    EliminateDen,
    /// §charter-v5: the privateer raids the colony as the mine comes online;
    /// done when the report of what it took reaches command.
    PirateRaid,
    /// §charter-v5: sell Ferrite Ore to the free trader (or it has gone).
    SellOre,
}

impl FoundingStage {
    /// §charter-v5: the charter's steps in play order (Complete follows).
    pub const STEPS: [FoundingStage; 18] = [
        Self::BuildBioharvester,
        Self::BuildHabitat,
        Self::BuildAgroplex,
        Self::BuildMine,
        Self::PirateRaid,
        Self::SellOre,
        Self::PowerShipyard,
        Self::BuildShipyard,
        Self::BuildInterceptor,
        Self::DefeatPrivateer,
        Self::EliminateDen,
        Self::PrepareExport,
        Self::FirstResearch,
        Self::ExpandMining,
        Self::SurveyCandidates,
        Self::BuildBarge,
        Self::MineBelt,
        Self::HaulOre,
    ];

    /// The 1-based step, 19 for Complete; retired stages count as the step
    /// they hand on to.
    pub fn step(self) -> usize {
        let live = match self {
            Self::Complete | Self::BuildColony | Self::EstablishColony => return Self::STEPS.len() + 1,
            Self::BuySemiconductors => Self::PowerShipyard,
            Self::BuildFreighter | Self::BuildConvoy => Self::BuildInterceptor,
            Self::LocalRecovery | Self::ExportProduction => Self::PrepareExport,
            Self::CompleteExport | Self::GrowBusiness | Self::BuildSecondFreighter => Self::BuildShipyard,
            Self::BuildAcademy => Self::FirstResearch,
            Self::BuildScout => Self::SurveyCandidates,
            other => other,
        };
        Self::STEPS.iter().position(|s| *s == live).map_or(Self::STEPS.len() + 1, |i| i + 1)
    }
}

impl Default for FoundingStage {
    fn default() -> Self {
        Self::Complete
    }
}

/// §first-session (Oct 15 2026): the charter's opening buildings rise in
/// [`OPENING_BUILD_SECS`] while the charter runs: level I of each on the
/// landing world (the home's Command Center world), whatever its construction
/// speed, so the first session isn't spent waiting on them. Tunable.
pub const OPENING_BUILD_SECS: f64 = 10.0;
pub const OPENING_BUILDS: [StructureKind; 5] = [StructureKind::MiningComplex, StructureKind::Bioharvester,
    StructureKind::Habitat, StructureKind::Agroplex, StructureKind::PowerPlant];

/// §charter-v5: where the raiding privateer is in its raid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaidPhase {
    /// Flying in on the colony.
    Inbound,
    /// Over the colony, loading what it takes.
    Looting,
    /// Flying home to its den with the loot.
    Returning,
    /// Unloaded; lurking between its den and the colony.
    Lurking,
}

/// §charter-v5: the privateer's one raid on a founder's colony. Persisted, so
/// a reload neither repeats it nor refills what it may take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoundingRaid {
    pub phase: RaidPhase,
    /// Where it heads home to: its den, or its starting point without one.
    pub base: Vec2,
    /// Where it lurks once home.
    pub lurk: Vec2,
    pub arrived_at: Option<f64>,
    /// What it may still take, frozen when it arrives over the colony.
    #[serde(default)]
    pub allowance: BTreeMap<Commodity, u32>,
    #[serde(default)]
    pub stolen: BTreeMap<Commodity, u32>,
    #[serde(default)]
    pub loading_credit: f64,
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
    /// §merchant: the founding merchant has been called (once per charter).
    #[serde(default)]
    pub merchant_called: bool,
    /// §charter-v5: the privateer's raid on the colony, once launched.
    #[serde(default)]
    pub raid: Option<FoundingRaid>,
    /// §charter-v5: when command first sees the raiding privateer; from then
    /// on the colony tracks it.
    #[serde(default)]
    pub raid_seen_at: Option<f64>,
    /// §charter-v5: when the report of what the raid took reaches command.
    #[serde(default)]
    pub raid_report_at: Option<f64>,
    /// §charter-v5: Ferrite Ore sold to the free trader.
    #[serde(default)]
    pub ore_sold: u32,
    /// §charter-v4: the system of this corporation's privateer den, once the
    /// privateer has appeared (its den with it).
    #[serde(default)]
    pub den: Option<EntityId>,
    /// §charter-v4: when the report that the den fell reaches command.
    #[serde(default)]
    pub den_report_at: Option<f64>,
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
            merchant_called: false,
            raid: None,
            raid_seen_at: None,
            raid_report_at: None,
            ore_sold: 0,
            den: None,
            den_report_at: None,
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
        let first = FoundingStage::STEPS[0];
        let mut programme = Self {
            stage: first,
            enabled: true,
            started_at: now,
            initial_surveyed,
            initial_owned_systems,
            survey_candidates,
            ..Self::default()
        };
        programme.milestones.insert(first, now);
        programme
    }

    /// §charter-v4: the charter's land grant covers only the landing world
    /// until "Expand mining operations" opens the rest of the home system.
    pub fn land_grant_open(&self) -> bool {
        !self.enabled || !self.local_opening || self.stage.step() >= FoundingStage::ExpandMining.step()
    }

    /// The stipend this programme earns on first reaching `stage`
    /// ([`charter_stipend`]); a home without a privateer den earns none for one.
    pub fn stipend(&self, stage: FoundingStage) -> Option<(&'static str, f64)> {
        if stage == FoundingStage::PrepareExport && self.den.is_none() {
            return None;
        }
        charter_stipend(stage)
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

    /// §founding R8: the Landfall chapter, from settling until the raid's
    /// report (§charter-v5; before it, the first export) reaches command.
    /// Contract offers stay few and patient here.
    pub fn landfall_focus(&self) -> bool {
        self.enabled
            && self.local_opening
            && self.raid_report_at.is_none()
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

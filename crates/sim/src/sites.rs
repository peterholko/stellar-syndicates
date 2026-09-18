//! Finite, discoverable places between stars. Truth never goes on the public
//! chart: the server sends only a corporation's arrived, immutable SiteReports.
use crate::{EntityId, PlayerId, cargo::Commodity, math::Vec2, module::ModuleKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SITE_RANGE: f64 = 300.0;
pub const STUDY_SECONDS: f64 = 20.0;
pub const RECOVERY_SECONDS: f64 = 30.0;
pub const RESTORE_SECONDS: f64 = 60.0;
/// A Scout's passive search for stationary signatures, not a new combat sensor.
pub const SCOUT_CONTACT_RANGE: f64 = 30_000.0;
/// Local navigation lookout for expedition safety (Tunable), not a shared
/// combat sensor that reveals rival positions to command center.
pub const EXPEDITION_LOOKOUT_RANGE: f64 = 20_000.0;
pub const EXPEDITION_ESCAPE_DISTANCE: f64 = 25_000.0;
pub const SENSING_PERIOD: f64 = 1.0;
pub const RESTORE_MACHINERY: u32 = 12;
pub const RESTORE_ELECTRONICS: u32 = 8;
pub const DEEP_STUDY_SECONDS: f64 = 60.0;
pub const EXTRACTION_SECONDS: f64 = 45.0;
pub const JOURNAL_NOTE_CHARS: usize = 400;
pub const JOURNAL_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    Derelict,
    Station,
    Asteroids,
    Anomaly,
    Precursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpeditionTask {
    Investigate,
    Recover,
    Restore,
    /// Explicit choices: ordinary travel never opts into deep/risky work.
    Study,
    Extract,
}

impl ExpeditionTask {
    pub fn seconds(self) -> f64 {
        match self {
            Self::Investigate => STUDY_SECONDS,
            Self::Recover => RECOVERY_SECONDS,
            Self::Restore => RESTORE_SECONDS,
            Self::Study => DEEP_STUDY_SECONDS,
            Self::Extract => EXTRACTION_SECONDS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyRequirement { Scout, ResearchTeam, FuelledFreighter, ShieldedFreighter }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteOpportunity {
    pub task: ExpeditionTask,
    pub requirement: StudyRequirement,
    pub seconds: f64,
    pub costs: BTreeMap<Commodity, u32>,
    /// A finite deep cache, separate from the freely accessible surface cargo.
    pub cargo: BTreeMap<Commodity, u32>,
    pub blueprint: Option<ModuleKind>,
    /// Only the existence of a lead is advertised; coordinates require study.
    pub has_lead: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryLead {
    pub site: EntityId,
    pub pos: Vec2,
    pub clue: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalKind { Site, System }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: EntityId,
    pub kind: JournalKind,
    pub pinned: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpeditionAssignment {
    pub site: EntityId,
    pub task: ExpeditionTask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteDetails {
    pub kind: SiteKind,
    pub name: String,
    pub cargo: BTreeMap<Commodity, u32>,
    pub modules: BTreeMap<ModuleKind, u32>,
    /// A one-time data grant per corporation, not a free technology or gate bypass.
    pub programme: String,
    pub research_fraction: f64,
    pub restored_by: Option<PlayerId>,
    #[serde(default)]
    pub environment: Option<crate::nebula::NebulaKind>,
    #[serde(default)]
    pub opportunity: Option<SiteOpportunity>,
    /// Per-recipient results, filled when freezing a report, never by the UI.
    #[serde(default)]
    pub studied: bool,
    #[serde(default)]
    pub lead: Option<DiscoveryLead>,
    #[serde(default)]
    pub guarded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteReport {
    pub id: EntityId,
    pub pos: Vec2,
    pub reported_at: f64,
    /// None means an unidentified contact. No kind, loot, owner or depletion leaks.
    pub details: Option<SiteDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationSite {
    pub id: EntityId,
    pub pos: Vec2,
    pub details: SiteDetails,
    pub revision: u64,
    /// Physical outpost, never part of an unidentified contact's wire report.
    #[serde(default)]
    pub restored_sensor: Option<EntityId>,
    pub surveyed: BTreeSet<PlayerId>,
    pub study_paid: BTreeSet<PlayerId>,
    pub known: BTreeMap<PlayerId, SiteReport>,
    /// Dedupes observations, not knowledge. A queued report may still be in flight.
    pub emitted: BTreeMap<PlayerId, (u64, bool)>,
    /// Hidden authored linkage. It enters a report only after on-site study.
    #[serde(default)]
    pub followup: Option<DiscoveryLead>,
    #[serde(default)]
    pub studied: BTreeSet<PlayerId>,
    /// Persistent, ordinary physical ships. Loading a save never respawns them.
    #[serde(default)]
    pub guardians: Vec<EntityId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSiteReport {
    pub recipient: PlayerId,
    pub arrival: f64,
    pub snapshot: SiteReport,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Exploration {
    pub generated: bool,
    /// Each home gets four nearby, off-system contacts, including overflow joins.
    pub seeded_homes: BTreeSet<EntityId>,
    /// Utility reward migration is append-only: never reroll a discovered or
    /// emptied wreck in a saved galaxy.
    #[serde(default)]
    pub utility_seeded_homes: BTreeSet<EntityId>,
    /// One later freight-equipment cache per home, appended without changing
    /// any existing site's position, contents or received survey report.
    #[serde(default)]
    pub cargo_seeded_homes: BTreeSet<EntityId>,
    pub sites: BTreeMap<EntityId, ExplorationSite>,
    pub pending: Vec<PendingSiteReport>,
    pub next_sensing_at: f64,
    #[serde(default)]
    pub chains_seeded_homes: BTreeSet<EntityId>,
    #[serde(default)]
    pub nebula_activities_seeded: BTreeSet<u32>,
    /// Private command-center annotations, not observations of the world.
    #[serde(default)]
    pub journal: BTreeMap<PlayerId, BTreeMap<EntityId, JournalEntry>>,
    /// Change-gates the reliable journal section; notes never ride 10 Hz Views.
    #[serde(default)]
    pub journal_versions: BTreeMap<PlayerId, u64>,
}

impl Exploration {
    pub fn journal_version_for(&self, player: PlayerId) -> u64 {
        self.journal_versions.get(&player).copied().unwrap_or(0)
    }
    pub fn journal_for(&self, player: PlayerId) -> Vec<JournalEntry> {
        self.journal.get(&player).map(|entries| entries.values().cloned().collect()).unwrap_or_default()
    }
    pub fn reports_for(&self, player: PlayerId) -> Vec<SiteReport> {
        self.sites
            .values()
            .filter_map(|s| s.known.get(&player).cloned())
            .collect()
    }
}

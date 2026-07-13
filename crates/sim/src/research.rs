//! §research — PROGRAMME BOARDS WITH SCHOOLS (v6). A syndicate-wide tech layer:
//! six FIELDS, each Tier I (open) → Tier II (field verb gate) → two SCHOOLS
//! (own verb gate, Tiers III–V). NO EXCLUSIVITY — every programme is
//! researchable by every syndicate; identity is the ORDER you chose on a
//! one-at-a-time continuous clock.
//!
//! This module owns the FRAMEWORK (R1): the catalog data model, the tier-gate
//! table, the tunable cost/basket/affinity tables, and the PURE availability /
//! completion logic. The distributed clock (R2), the verb counters (R3), and
//! the effect wiring (R4) live at their sim sites and read from here.
//!
//! Determinism & compat (design law 1): the catalog is `const`; all keyed
//! syndicate state is `BTreeMap`/`BTreeSet`; `ResearchState` is `#[serde(default)]`
//! on the `Syndicate`, so old snapshots load with empty research and tick clean.
//! Programme ids are `&'static str` in the catalog but `String` in serialized
//! state (a `&'static str` can't be deserialized), bridged by string equality.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::build::StructureKind;
use crate::cargo::Commodity;
use crate::ids::EntityId;
use crate::module::ModuleKind;
use crate::ship::ShipKind;
use crate::specialist::SpecialistKind;

/// A programme's stable slug. `&'static str` in the catalog; `String` in state.
pub type ProgrammeId = String;

// ─────────────────────────────────────────────────────────────────────────────
// FIELDS & SCHOOLS
// ─────────────────────────────────────────────────────────────────────────────

/// The six research FIELDS (boards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Propulsion,
    Materials,
    Computation,
    Weapons,
    Hulls,
    Life,
}

/// The twelve SCHOOLS — two per field, each a Tier-III–V ladder behind its own
/// verb gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum School {
    LineHaul,       // Propulsion
    Expedition,     // Propulsion
    DeepCrust,      // Materials
    Foundry,        // Materials
    Watch,          // Computation
    Shadow,         // Computation
    Strike,         // Weapons
    Countermeasures, // Weapons
    Line,           // Hulls
    Corsair,        // Hulls
    Growth,         // Life
    Talent,         // Life
}

impl School {
    /// The field this school belongs to (a school only opens within its field).
    pub fn field(self) -> Field {
        match self {
            School::LineHaul | School::Expedition => Field::Propulsion,
            School::DeepCrust | School::Foundry => Field::Materials,
            School::Watch | School::Shadow => Field::Computation,
            School::Strike | School::Countermeasures => Field::Weapons,
            School::Line | School::Corsair => Field::Hulls,
            School::Growth | School::Talent => Field::Life,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// VERBS & METRICS (gate inputs — R3 feeds them)
// ─────────────────────────────────────────────────────────────────────────────

/// CUMULATIVE, corp-wide counters — a syndicate's biography. Incremented when
/// the SIM resolves the underlying event (design law 4: the sim is the ledger).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    LyFlown,
    WarshipLyFlown,
    ConvoyDeliveries,
    UnitsExtracted,
    UnitsProcessed,
    UnitsThroughIndustry,
    SystemsScouted,
    RivalFleetsObserved,
    BattlesFought,
    BattlesWon,
    HullMassDestroyed,
    DamageAbsorbed,
    SuccessfulRaids,
    WarshipsCommissioned,
    PopulationGrown,
    SpecialistsTrained,
}

/// INSTANTANEOUS STATE metrics (read each check, not accumulated).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    TotalPopulation,
    WellSuppliedSystems,
}

// ─────────────────────────────────────────────────────────────────────────────
// EFFECT KEYS — ModKeys (multiplicative/additive tuners) and Caps (capabilities)
// ─────────────────────────────────────────────────────────────────────────────

/// A tunable EFFECT KEY. Each names its single application site (R4a). Most are
/// MULTIPLICATIVE (default 1.0, factors multiply); the two bucket keys are
/// ADDITIVE integer steps (default 0.0, steps sum) — see [`ModKey::is_additive`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModKey {
    // Propulsion / logistics
    SpeedAll,
    SpeedWarship,
    FuelCapacity,
    FuelConsumption,
    FuelOffensive,
    ConvoyCargo,
    ConvoySpeed,
    ColonySeedPop,
    ColonyCost,
    ColonyBuildTime,
    // Materials / fabrication
    ExtractionRate,
    ExtractionMoons,
    ProcessingYield,
    MachineryInputs,
    AgroplexYield,
    AgroplexInputs,
    DepotCap,
    StructureBuildTime,
    WarshipBuildTime,
    WarshipCost,
    ModuleBuildTime,
    ModuleCost,
    RefitTime,
    // Weapons (damage + counter constants — law 5)
    BeamDmg,
    DriverDmg,
    TorpDmg,
    OpeningRoundDmg,
    PdIntercept,
    ReflectBlunt,
    WhippleBlunt,
    DmgVsPlatforms,
    // Hulls / combat survivability
    HullMass,
    DamagePoolDepth,
    RearmRate,
    AnchoredDmgTaken,
    RaiderDisengageExposure,
    RaidSteal,
    EscortedConvoyRaidDmg,
    WolfpackPerStack,
    // Computation / sensors
    SensorRadius,
    SensorRange,
    BucketFineness,   // additive: rival fleets read +N class finer to you
    BucketCoarseness, // additive: your fleets read +N class coarser to rivals
    KnowledgeLadderRate,
    // Life / habitation
    PopGrowth,
    HabitatCap,
    RationingFloor,
    ProvisionsUse,
    GrowthBelowHalf,
    // Talent
    TrainingTime,
    AcademyConcurrent,
    SpecialistHousing,
}

impl ModKey {
    /// The two bucket keys aggregate ADDITIVELY (integer class steps, base 0);
    /// every other key aggregates MULTIPLICATIVELY (factors, base 1.0).
    pub fn is_additive(self) -> bool {
        matches!(self, ModKey::BucketFineness | ModKey::BucketCoarseness)
    }
}

/// A CAPABILITY FLAG — a boolean unlock with a single enforcement point (R4c).
/// Enforcement of `SalvageRigs`/`BoardingParties` is deferred (their catalog
/// entries ship `hidden`); the flags exist so the enum is stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cap {
    // Logistics
    StandingTriggers,
    AutonomousFreight,
    MassStreams,
    FleetTenders,
    UnderwayRefit,
    Ramscoop,
    // Materials
    TraceRefining,
    SlagReclamation,
    CrownVein,
    CrownProject,
    FlashForges,
    ProspectingCharters,
    MantleTaps,
    // Life
    Xenoacclimation,
    // Computation / sensors
    SurveyCorps,
    StandingWatch,
    PanopticonCatalog,
    EcmEmitters,
    SignatureMimicry,
    CounterIntel,
    TrafficAnalysis,
    WakeAnalysis,
    BattleArchives,
    // Weapons / platforms
    PlatformLances,
    PlatformNetfire,
    GrandBatteries,
    FirstStrike,
    // Hulls
    FortressDoctrine,
    // Talent / population
    ExpertTraining,
    FoundersInstitutes,
    BroadCurricula,
    TwinCampuses,
    CryoBerths,
    GenerationCharters,
    // Deferred-enforcement (hidden entries)
    SalvageRigs,
    BoardingParties,
}

// ─────────────────────────────────────────────────────────────────────────────
// GATES & EFFECTS
// ─────────────────────────────────────────────────────────────────────────────

/// A TIER GATE — the verb/metric condition to enter a tier. Lives per
/// (field, school, tier) in [`tier_gate`], never per programme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gate {
    /// Tier I (and IV/V, which are ladder-governed): no verb gate.
    None,
    /// A cumulative verb counter must reach the threshold.
    Cumulative(Verb, f64),
    /// An instantaneous state metric must reach the threshold.
    State(Metric, f64),
    /// A state metric must hold ≥ threshold continuously for `secs` (Life V).
    Sustained(Metric, f64, u64),
}

/// What COMPLETING a programme does. `Mods` carries a slice so a single
/// programme can tune several keys (e.g. Munitions Lines: build-time AND cost).
/// Never serialized — it lives only in the `const` catalog; state stores ids.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    Mods(&'static [(ModKey, f64)]),
    UnlockModule(ModuleKind),
    UnlockHull(ShipKind),
    UnlockStructureTier(StructureKind, u32),
    Flag(Cap),
}

/// A live target a capability designates (Crown Project body, Mass Stream pair,
/// Signature Mimicry fleet). Set via `SetDesignation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignationTarget {
    Body(EntityId),
    SystemPair(EntityId, EntityId),
    Fleet(EntityId),
}

/// One authored PROGRAMME (a catalog node).
#[derive(Debug, Clone, Copy)]
pub struct Programme {
    pub id: &'static str,
    pub field: Field,
    /// `None` = a shared Tier I/II programme; `Some` = a school Tier III–V one.
    pub school: Option<School>,
    pub tier: u8,
    pub name: &'static str,
    pub blurb: &'static str,
    pub effect: Effect,
    /// Hidden entries never render and are never researchable in v1 (Salvage
    /// Rigs, Boarding Parties — enforcement deferred to their own handoffs).
    pub hidden: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// SYNDICATE RESEARCH STATE (serde-default on `Syndicate`)
// ─────────────────────────────────────────────────────────────────────────────

/// A syndicate's whole research picture. All keyed maps are `BTreeMap`/
/// `BTreeSet` (deterministic) and every field is serde-default (old snaps load
/// empty). Owner-only in the view (design law 3).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResearchState {
    /// The programme the clock is accruing into (front of the queue when set).
    #[serde(default)]
    pub active: Option<ProgrammeId>,
    /// The queue-ahead list (the design's primary strategic verb).
    #[serde(default)]
    pub queue: Vec<ProgrammeId>,
    /// Throughput-seconds accrued into `active`.
    #[serde(default)]
    pub progress: f64,
    /// Completed programme ids (drives availability + the `mods` layer).
    #[serde(default)]
    pub completed: BTreeSet<ProgrammeId>,
    /// Cumulative verb counters.
    #[serde(default)]
    pub verbs: BTreeMap<Verb, f64>,
    /// For `Sustained` gates: sim-time (secs, floored) since a metric became
    /// continuously satisfied. Absent = not currently satisfied. Keyed by metric
    /// (v1 has one Sustained gate; a string/tuple key would break JSON).
    #[serde(default)]
    pub sustained_since: BTreeMap<Metric, u64>,
    /// Capability designations (Crown Project body, Mass Stream pair, etc.).
    #[serde(default)]
    pub designations: BTreeMap<Cap, DesignationTarget>,
    /// §research R2: latched STALL state — true while an available active
    /// programme has no staffed Academy contributing. Drives the fire-once
    /// `ResearchStalled`/`ResearchResumed` events. serde-default false.
    #[serde(default)]
    pub stalled: bool,
    /// §research R3: DISTINCT rival/pirate fleet ids a member has ever detected
    /// (the `RivalFleetsObserved` verb = this set's len; dedupes re-sightings).
    #[serde(default)]
    pub observed_fleets: BTreeSet<EntityId>,
    /// §research R3: DISTINCT systems a member has first advanced the knowledge
    /// ladder on (the `SystemsScouted` verb = this set's len; dedupes revisits).
    #[serde(default)]
    pub scouted_systems: BTreeSet<EntityId>,
}

impl ResearchState {
    /// The current value of a cumulative verb (0 if never incremented).
    pub fn verb(&self, v: Verb) -> f64 {
        self.verbs.get(&v).copied().unwrap_or(0.0)
    }

    /// Add to a cumulative verb (R3 hook sites call this).
    pub fn add_verb(&mut self, v: Verb, amount: f64) {
        if amount != 0.0 {
            *self.verbs.entry(v).or_insert(0.0) += amount;
        }
    }

    /// Has this syndicate completed `id`?
    pub fn has(&self, id: &str) -> bool {
        self.completed.contains(id)
    }

    /// Replace the queue (already-validated ids), promoting the front to
    /// `active` if the clock is currently idle. The queue-ahead command path.
    pub fn set_queue(&mut self, queue: Vec<ProgrammeId>) {
        self.queue = queue;
        if self.active.is_none() && !self.queue.is_empty() {
            self.active = Some(self.queue.remove(0));
        }
    }

    /// If the active programme is fully funded, COMPLETE it: record it, carry
    /// any progress overflow to the next queue entry, and promote that entry to
    /// active. Returns the completed id (the caller emits the event + the effect
    /// is realized lazily via [`mods`]). `None` if nothing completed this call.
    pub fn try_complete(&mut self) -> Option<ProgrammeId> {
        let active = self.active.clone()?;
        let cost = cost_of(&active);
        if self.progress + 1e-6 < cost {
            return None;
        }
        self.completed.insert(active.clone());
        self.progress -= cost; // carry the overflow into the next programme
        self.active = if self.queue.is_empty() { None } else { Some(self.queue.remove(0)) };
        if self.active.is_none() {
            self.progress = 0.0; // nothing to carry into
        }
        Some(active)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TIER-GATE TABLE (per field / school / tier)
// ─────────────────────────────────────────────────────────────────────────────

/// The verb gate to ENTER a tier. Tier I is always open; Tier II gates on the
/// FIELD verb; Tier III gates on the SCHOOL verb; Tiers IV–V are governed by the
/// ladder rule alone (their school gate, being cumulative, is already met). All
/// thresholds are `Tunable`, sized to season length (design law 5/8).
pub fn tier_gate(field: Field, school: Option<School>, tier: u8) -> Gate {
    // Life · GROWTH V is the endurance capstone: on top of the ladder predecessor,
    // hold ≥ 5 WellSupplied systems continuously for 7 days (the one Sustained gate).
    if let (Field::Life, Some(School::Growth), 5) = (field, school, tier) {
        return Gate::Sustained(Metric::WellSuppliedSystems, 5.0, 7 * 24 * 3600);
    }
    match tier {
        1 => Gate::None,
        2 => field_gate(field),
        3 => school.map(school_gate).unwrap_or(Gate::None),
        _ => Gate::None, // IV, V — ladder-governed
    }
}

/// The FIELD Tier-II verb gate. Tunable.
fn field_gate(field: Field) -> Gate {
    match field {
        Field::Propulsion => Gate::Cumulative(Verb::LyFlown, 200.0),
        Field::Materials => Gate::Cumulative(Verb::UnitsThroughIndustry, 10_000.0),
        Field::Computation => Gate::Cumulative(Verb::SystemsScouted, 10.0),
        Field::Weapons => Gate::Cumulative(Verb::BattlesFought, 5.0),
        Field::Hulls => Gate::Cumulative(Verb::WarshipsCommissioned, 15.0),
        // Population is carried in MILLIONS internally (`Body::population`), so the
        // "5M grown" design target is 5.0 in that unit; ditto the 20M Growth gate.
        Field::Life => Gate::Cumulative(Verb::PopulationGrown, 5.0),
    }
}

/// The SCHOOL Tier-III verb gate (school-flavored biography). Tunable.
fn school_gate(school: School) -> Gate {
    match school {
        School::LineHaul => Gate::Cumulative(Verb::ConvoyDeliveries, 30.0),
        School::Expedition => Gate::Cumulative(Verb::WarshipLyFlown, 800.0),
        School::DeepCrust => Gate::Cumulative(Verb::UnitsExtracted, 15_000.0),
        School::Foundry => Gate::Cumulative(Verb::UnitsProcessed, 12_000.0),
        School::Watch => Gate::Cumulative(Verb::SystemsScouted, 25.0),
        School::Shadow => Gate::Cumulative(Verb::RivalFleetsObserved, 15.0),
        School::Strike => Gate::Cumulative(Verb::HullMassDestroyed, 150.0),
        School::Countermeasures => Gate::Cumulative(Verb::DamageAbsorbed, 150.0),
        School::Line => Gate::Cumulative(Verb::BattlesWon, 8.0),
        School::Corsair => Gate::Cumulative(Verb::SuccessfulRaids, 8.0),
        School::Growth => Gate::State(Metric::TotalPopulation, 20.0),
        School::Talent => Gate::Cumulative(Verb::SpecialistsTrained, 8.0),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TUNABLE COST & BASKET & AFFINITY TABLES
// ─────────────────────────────────────────────────────────────────────────────

const HOUR: f64 = 3600.0;

/// Sim-units per "light-year" for the distance verbs (`LyFlown`/`WarshipLyFlown`).
/// One galaxy crossing (~8000 su) ≈ 200 ly, so the Propulsion II gate (200 ly) is
/// about one empire's worth of cumulative travel. Tunable.
pub const SU_PER_LY: f64 = 40.0;

/// Cost of a programme of `tier` in THROUGHPUT-SECONDS (at reference rate 1.0):
/// a steep curve so late convergence never happens inside a season (law 6/8).
/// T1 2h · T2 8h · T3 24h · T4 72h · T5 168h. Tunable.
pub fn tier_cost_secs(tier: u8) -> f64 {
    match tier {
        1 => 2.0 * HOUR,
        2 => 8.0 * HOUR,
        3 => 24.0 * HOUR,
        4 => 72.0 * HOUR,
        _ => 168.0 * HOUR,
    }
}

/// The funding BASKET drawn per RATE-SECOND for a `field`/`tier` programme —
/// each contributing Academy drips its share from its own stockpile (R2).
/// Escalates Electronics-light → Rare-Elements-heavy, field-flavored (Weapons
/// wants Armaments, Life wants Biomass/Provisions, Hulls wants Machinery). All
/// `Tunable`. Units per rate-second (small; integrated over the cost seconds).
pub fn basket(field: Field, tier: u8) -> Vec<(Commodity, f64)> {
    let mut b: Vec<(Commodity, f64)> = Vec::new();
    // Base: Electronics on every programme, growing with tier.
    b.push((Commodity::Electronics, 0.010 * tier as f64));
    // Machinery enters at Tier II.
    if tier >= 2 {
        b.push((Commodity::Machinery, 0.004 * (tier - 1) as f64));
    }
    // Rare Elements enter at Tier III, heavy at IV–V.
    if tier >= 3 {
        b.push((Commodity::RareElements, 0.003 * (tier - 2) as f64));
    }
    // Field flavor.
    match field {
        Field::Weapons => b.push((Commodity::Armaments, 0.006 * tier as f64)),
        Field::Life => {
            b.push((Commodity::Biomass, 0.004 * tier as f64));
            b.push((Commodity::Provisions, 0.003 * tier as f64));
        }
        Field::Hulls => b.push((Commodity::Machinery, 0.006 * tier as f64)),
        _ => {}
    }
    b
}

/// SPECIALIST AFFINITY per field — a matching resident specialist posted to an
/// Academy's research line multiplies its rate (the 1.75× skill factor). Tunable.
pub fn field_affinity(field: Field) -> &'static [SpecialistKind] {
    match field {
        Field::Propulsion => &[SpecialistKind::PetrochemicalEngineer],
        Field::Materials => &[SpecialistKind::Geologist, SpecialistKind::IndustrialEngineer],
        Field::Computation => &[SpecialistKind::IndustrialEngineer],
        Field::Weapons => &[SpecialistKind::NavalArchitect, SpecialistKind::IndustrialEngineer],
        Field::Hulls => &[SpecialistKind::NavalArchitect],
        Field::Life => &[SpecialistKind::Xenobiologist],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CATALOG — the full six-board, ~108-programme translation of the v6 design
// tables (docs/research-programme-boards-v6.md). Each field: Tier I (open) →
// Tier II (field gate) → two schools × Tiers III–V. Effects use the existing
// keys: `Mods` (mult/additive tuners), `Flag` (capability), `UnlockStructureTier`
// (tier IV/V of an existing structure). A handful of NEW-CONTENT prizes — the two
// prestige hulls (Destroyer/Cruiser), the utility modules (Extended Tanks / Recon
// Suite / Escort Datalink / Lance Array / Blockade-runner refit), and a couple of
// view/explore features — ship as `Effect::Mods(&[])` PLACEHOLDERS: researchable,
// on the tree, blurb-described, but inert until the "full pass" adds the ShipKind/
// ModuleKind content and wires them (they can't be expressed with today's enums).
// ─────────────────────────────────────────────────────────────────────────────

/// A no-op effect for programmes whose prize is NEW CONTENT not yet in the sim
/// (new hulls / utility modules / view features). Keeps the tree complete; the
/// full pass swaps these for real `UnlockHull`/`UnlockModule`/`Flag` effects.
const PENDING: Effect = Effect::Mods(&[]);

pub const CATALOG: &[Programme] = &[
    // ═══════════════════ 1 · PROPULSION & LOGISTICS ═══════════════════
    // Shared I (open)
    Programme { id: "prop_drive_tuning", field: Field::Propulsion, school: None, tier: 1,
        name: "Drive Tuning", blurb: "+10% speed, all ship kinds.",
        effect: Effect::Mods(&[(ModKey::SpeedAll, 1.10)]), hidden: false },
    Programme { id: "prop_bunkerage", field: Field::Propulsion, school: None, tier: 1,
        name: "Bunkerage", blurb: "+25% fuel capacity.",
        effect: Effect::Mods(&[(ModKey::FuelCapacity, 1.25)]), hidden: false },
    Programme { id: "prop_freight_frames", field: Field::Propulsion, school: None, tier: 1,
        name: "Freight Frames", blurb: "+20% convoy cargo.",
        effect: Effect::Mods(&[(ModKey::ConvoyCargo, 1.20)]), hidden: false },
    // Shared II (gate: 200 ly flown)
    Programme { id: "prop_efficient_burns", field: Field::Propulsion, school: None, tier: 2,
        name: "Efficient Burns", blurb: "−25% fuel consumption.",
        effect: Effect::Mods(&[(ModKey::FuelConsumption, 0.75)]), hidden: false },
    Programme { id: "prop_heavy_lifters", field: Field::Propulsion, school: None, tier: 2,
        name: "Heavy Lifters", blurb: "Colony ships seed +50% population.",
        effect: Effect::Mods(&[(ModKey::ColonySeedPop, 1.50)]), hidden: false },
    Programme { id: "prop_military_drives", field: Field::Propulsion, school: None, tier: 2,
        name: "Military Drives", blurb: "+20% warship speed.",
        effect: Effect::Mods(&[(ModKey::SpeedWarship, 1.20)]), hidden: false },
    // ⑂ LINE HAUL (gate: 30 convoy deliveries)
    Programme { id: "prop_line_logistics_doctrine", field: Field::Propulsion, school: Some(School::LineHaul), tier: 3,
        name: "Logistics Doctrine", blurb: "Standing-order triggers: thresholds, conditionals.",
        effect: Effect::Flag(Cap::StandingTriggers), hidden: false },
    Programme { id: "prop_line_express_charters", field: Field::Propulsion, school: Some(School::LineHaul), tier: 3,
        name: "Express Charters", blurb: "+25% convoy speed.",
        effect: Effect::Mods(&[(ModKey::ConvoySpeed, 1.25)]), hidden: false },
    Programme { id: "prop_line_drop_berths", field: Field::Propulsion, school: Some(School::LineHaul), tier: 4,
        name: "Drop Berths", blurb: "Colony ships −30% cost and build time.",
        effect: Effect::Mods(&[(ModKey::ColonyCost, 0.70), (ModKey::ColonyBuildTime, 0.70)]), hidden: false },
    Programme { id: "prop_line_bulk_charters", field: Field::Propulsion, school: Some(School::LineHaul), tier: 4,
        name: "Bulk Charters", blurb: "+40% convoy cargo.",
        effect: Effect::Mods(&[(ModKey::ConvoyCargo, 1.40)]), hidden: false },
    Programme { id: "prop_line_autonomous_freight", field: Field::Propulsion, school: Some(School::LineHaul), tier: 5,
        name: "Autonomous Freight", blurb: "Convoys chain multi-leg standing routes without CC round-trips.",
        effect: Effect::Flag(Cap::AutonomousFreight), hidden: false },
    Programme { id: "prop_line_mass_streams", field: Field::Propulsion, school: Some(School::LineHaul), tier: 5,
        name: "Mass Streams", blurb: "One designated owned-pair route: +40% speed (rivals can learn + interdict).",
        effect: Effect::Flag(Cap::MassStreams), hidden: false },
    // ⑂ EXPEDITION (gate: 800 ly flown by warships)
    Programme { id: "prop_expedition_iii_extended_tanks", field: Field::Propulsion, school: Some(School::Expedition), tier: 3,
        name: "Extended Tanks", blurb: "Utility module: +fuel capacity (arrives with the utility-module pass).",
        effect: PENDING, hidden: false },
    Programme { id: "prop_expedition_iii_torch_regime", field: Field::Propulsion, school: Some(School::Expedition), tier: 3,
        name: "Torch Regime", blurb: "+30% warship speed, +40% burn — loud and thirsty.",
        effect: Effect::Mods(&[(ModKey::SpeedWarship, 1.30), (ModKey::FuelConsumption, 1.40)]), hidden: false },
    Programme { id: "prop_expedition_iv_fleet_tenders", field: Field::Propulsion, school: Some(School::Expedition), tier: 4,
        name: "Fleet Tenders", blurb: "Convoys refuel fleets underway.",
        effect: Effect::Flag(Cap::FleetTenders), hidden: false },
    Programme { id: "prop_expedition_iv_long_patrol", field: Field::Propulsion, school: Some(School::Expedition), tier: 4,
        name: "Long Patrol", blurb: "Warship fuel consumption −30%.",
        effect: Effect::Mods(&[(ModKey::FuelOffensive, 0.70)]), hidden: false },
    Programme { id: "prop_expedition_v_ramscoop", field: Field::Propulsion, school: Some(School::Expedition), tier: 5,
        name: "Ramscoop Skimming", blurb: "Ships regain fuel transiting gas-giant systems.",
        effect: Effect::Flag(Cap::Ramscoop), hidden: false },
    Programme { id: "prop_expedition_v_underway_refit", field: Field::Propulsion, school: Some(School::Expedition), tier: 5,
        name: "Underway Refit", blurb: "Fleet Tenders perform module refits in the field.",
        effect: Effect::Flag(Cap::UnderwayRefit), hidden: false },

    // ═══════════════════ 2 · MATERIALS & FABRICATION ═══════════════════
    // Shared I
    Programme { id: "mat_deep_bores", field: Field::Materials, school: None, tier: 1,
        name: "Deep Bores", blurb: "+15% extraction.",
        effect: Effect::Mods(&[(ModKey::ExtractionRate, 1.15)]), hidden: false },
    Programme { id: "mat_enrichment", field: Field::Materials, school: None, tier: 1,
        name: "Enrichment", blurb: "+15% processing yield.",
        effect: Effect::Mods(&[(ModKey::ProcessingYield, 1.15)]), hidden: false },
    Programme { id: "mat_bulk_storage", field: Field::Materials, school: None, tier: 1,
        name: "Bulk Storage", blurb: "+50% Depot caps.",
        effect: Effect::Mods(&[(ModKey::DepotCap, 1.50)]), hidden: false },
    // Shared II (gate: 10,000 units through industry)
    Programme { id: "mat_beneficiation", field: Field::Materials, school: None, tier: 2,
        name: "Beneficiation", blurb: "Poor deposits gain a richness floor (arrives with the extraction pass).",
        effect: PENDING, hidden: false },
    Programme { id: "mat_prefab_construction", field: Field::Materials, school: None, tier: 2,
        name: "Prefab Construction", blurb: "−25% structure build time.",
        effect: Effect::Mods(&[(ModKey::StructureBuildTime, 0.75)]), hidden: false },
    Programme { id: "mat_autoforges", field: Field::Materials, school: None, tier: 2,
        name: "Autoforges", blurb: "Machinery recipe −20% inputs.",
        effect: Effect::Mods(&[(ModKey::MachineryInputs, 0.80)]), hidden: false },
    // ⑂ DEEP CRUST (gate: 15,000 raw units extracted)
    Programme { id: "mat_deepcrust_iii_tier4_extraction", field: Field::Materials, school: Some(School::DeepCrust), tier: 3,
        name: "Tier-IV Extraction", blurb: "Extraction structures reach tier IV.",
        effect: Effect::UnlockStructureTier(StructureKind::MiningComplex, 4), hidden: false },
    Programme { id: "mat_deepcrust_iii_strip_rigs", field: Field::Materials, school: Some(School::DeepCrust), tier: 3,
        name: "Strip Rigs", blurb: "+25% extraction on moons.",
        effect: Effect::Mods(&[(ModKey::ExtractionMoons, 1.25)]), hidden: false },
    Programme { id: "mat_deepcrust_iv_trace_refining", field: Field::Materials, school: Some(School::DeepCrust), tier: 4,
        name: "Trace Refining", blurb: "Mining Complexes yield 5% Rare Elements on metallic deposits.",
        effect: Effect::Flag(Cap::TraceRefining), hidden: false },
    Programme { id: "mat_deepcrust_iv_prospecting_charters", field: Field::Materials, school: Some(School::DeepCrust), tier: 4,
        name: "Prospecting Charters", blurb: "New colonies start at full deposit knowledge.",
        effect: Effect::Flag(Cap::ProspectingCharters), hidden: false },
    Programme { id: "mat_deepcrust_v_mantle_taps", field: Field::Materials, school: Some(School::DeepCrust), tier: 5,
        name: "Mantle Taps", blurb: "Extraction ignores accessibility penalties.",
        effect: Effect::Flag(Cap::MantleTaps), hidden: false },
    Programme { id: "mat_deepcrust_v_crown_vein", field: Field::Materials, school: Some(School::DeepCrust), tier: 5,
        name: "Crown Vein", blurb: "One designated body: deposits +1 richness band.",
        effect: Effect::Flag(Cap::CrownVein), hidden: false },
    // ⑂ FOUNDRY (gate: 12,000 units processed)
    Programme { id: "mat_foundry_iii_tier4_processors", field: Field::Materials, school: Some(School::Foundry), tier: 3,
        name: "Tier-IV Processors", blurb: "Processing structures reach tier IV.",
        effect: Effect::UnlockStructureTier(StructureKind::Smelter, 4), hidden: false },
    Programme { id: "mat_foundry_iii_slag_reclamation", field: Field::Materials, school: Some(School::Foundry), tier: 3,
        name: "Slag Reclamation", blurb: "Smelters emit 10% bonus Silicates.",
        effect: Effect::Flag(Cap::SlagReclamation), hidden: false },
    Programme { id: "mat_foundry_iv_orbital_yards", field: Field::Materials, school: Some(School::Foundry), tier: 4,
        name: "Orbital Yards", blurb: "Shipyard tier IV.",
        effect: Effect::UnlockStructureTier(StructureKind::Shipyard, 4), hidden: false },
    Programme { id: "mat_foundry_iv_arcology_frames", field: Field::Materials, school: Some(School::Foundry), tier: 4,
        name: "Arcology Frames", blurb: "Habitat tier IV.",
        effect: Effect::UnlockStructureTier(StructureKind::Habitat, 4), hidden: false },
    Programme { id: "mat_foundry_v_flash_forges", field: Field::Materials, school: Some(School::Foundry), tier: 5,
        name: "Flash Forges", blurb: "Systems run 2 concurrent structure builds.",
        effect: Effect::Flag(Cap::FlashForges), hidden: false },
    Programme { id: "mat_foundry_v_crown_project", field: Field::Materials, school: Some(School::Foundry), tier: 5,
        name: "Crown Project", blurb: "One designated body gains +1 slot in every pool.",
        effect: Effect::Flag(Cap::CrownProject), hidden: false },

    // ═══════════════════ 3 · COMPUTATION & SENSORS ═══════════════════
    // Shared I
    Programme { id: "comp_sensor_gain", field: Field::Computation, school: None, tier: 1,
        name: "Sensor Gain", blurb: "+15% sensor radius.",
        effect: Effect::Mods(&[(ModKey::SensorRadius, 1.15)]), hidden: false },
    Programme { id: "comp_signal_libraries", field: Field::Computation, school: None, tier: 1,
        name: "Signal Libraries", blurb: "Rival fleets bucket one class finer to you.",
        effect: Effect::Mods(&[(ModKey::BucketFineness, 1.0)]), hidden: false },
    Programme { id: "comp_survey_protocols", field: Field::Computation, school: None, tier: 1,
        name: "Survey Protocols", blurb: "Deposit ladder advances faster.",
        effect: Effect::Mods(&[(ModKey::KnowledgeLadderRate, 1.25)]), hidden: false },
    // Shared II (gate: 10 systems scouted)
    Programme { id: "comp_predictive_plots", field: Field::Computation, school: None, tier: 2,
        name: "Predictive Plots", blurb: "Stale-intel confidence bands (view feature; arrives with the intel pass).",
        effect: PENDING, hidden: false },
    Programme { id: "comp_deep_space_arrays", field: Field::Computation, school: None, tier: 2,
        name: "Deep-Space Arrays", blurb: "Sensor Array tier IV.",
        effect: Effect::UnlockStructureTier(StructureKind::SensorArray, 4), hidden: false },
    Programme { id: "comp_gravimetric_survey", field: Field::Computation, school: None, tier: 2,
        name: "Gravimetric Survey", blurb: "Deposits read one R-level deeper at range (arrives with the explore pass).",
        effect: PENDING, hidden: false },
    // ⑂ WATCH (gate: 25 systems scouted)
    Programme { id: "comp_watch_iii_picket_networks", field: Field::Computation, school: Some(School::Watch), tier: 3,
        name: "Picket Networks", blurb: "Owned sensor radii +25%; overlaps form surveilled corridors.",
        effect: Effect::Mods(&[(ModKey::SensorRadius, 1.25)]), hidden: false },
    Programme { id: "comp_watch_iii_long_baselines", field: Field::Computation, school: Some(School::Watch), tier: 3,
        name: "Long Baselines", blurb: "+15% sensor range.",
        effect: Effect::Mods(&[(ModKey::SensorRange, 1.15)]), hidden: false },
    Programme { id: "comp_watch_iv_battle_archives", field: Field::Computation, school: Some(School::Watch), tier: 4,
        name: "Battle Archives", blurb: "Third-party battle records gain bucketed kill detail.",
        effect: Effect::Flag(Cap::BattleArchives), hidden: false },
    Programme { id: "comp_watch_iv_standing_watch", field: Field::Computation, school: Some(School::Watch), tier: 4,
        name: "Standing Watch", blurb: "Contacts persist as tracked estimates after leaving coverage.",
        effect: Effect::Flag(Cap::StandingWatch), hidden: false },
    Programme { id: "comp_watch_v_panopticon", field: Field::Computation, school: Some(School::Watch), tier: 5,
        name: "Panopticon Catalog", blurb: "CC auto-catalogs every battle flash whose light reaches any asset.",
        effect: Effect::Flag(Cap::PanopticonCatalog), hidden: false },
    Programme { id: "comp_watch_v_survey_corps", field: Field::Computation, school: Some(School::Watch), tier: 5,
        name: "Survey Corps", blurb: "Scouts auto-advance the deposit ladder in-system.",
        effect: Effect::Flag(Cap::SurveyCorps), hidden: false },
    // ⑂ SHADOW (gate: 15 distinct rival or pirate fleets observed)
    Programme { id: "comp_shadow_iii_ecm_emitters", field: Field::Computation, school: Some(School::Shadow), tier: 3,
        name: "ECM Emitters", blurb: "Your fleets bucket one class coarser — sensor warfare; the combat matrix untouched.",
        effect: Effect::Mods(&[(ModKey::BucketCoarseness, 1.0)]), hidden: false },
    Programme { id: "comp_shadow_iii_traffic_analysis", field: Field::Computation, school: Some(School::Shadow), tier: 3,
        name: "Traffic Analysis", blurb: "Contacts reveal convoy manifest class: goods / personnel / modules.",
        effect: Effect::Flag(Cap::TrafficAnalysis), hidden: false },
    Programme { id: "comp_shadow_iv_wake_analysis", field: Field::Computation, school: Some(School::Shadow), tier: 4,
        name: "Wake Analysis", blurb: "Observed fleets show estimated fuel state and origin bearing.",
        effect: Effect::Flag(Cap::WakeAnalysis), hidden: false },
    Programme { id: "comp_shadow_iv_recon_suite", field: Field::Computation, school: Some(School::Shadow), tier: 4,
        name: "Recon Suite", blurb: "Utility module: +sensor radius (arrives with the utility-module pass).",
        effect: PENDING, hidden: false },
    Programme { id: "comp_shadow_v_signature_mimicry", field: Field::Computation, school: Some(School::Shadow), tier: 5,
        name: "Signature Mimicry", blurb: "A designated fleet reads one count-class smaller and can present as a convoy until it engages.",
        effect: Effect::Flag(Cap::SignatureMimicry), hidden: false },
    Programme { id: "comp_shadow_v_counter_intel", field: Field::Computation, school: Some(School::Shadow), tier: 5,
        name: "Counter-Intelligence", blurb: "Rival Signal Libraries / Traffic / Wake analysis read null against you.",
        effect: Effect::Flag(Cap::CounterIntel), hidden: false },

    // ═══════════════════ 4 · WEAPONS & ORDNANCE ═══════════════════
    // Shared I
    Programme { id: "weap_fire_control", field: Field::Weapons, school: None, tier: 1,
        name: "Fire Control", blurb: "+10% beam damage.",
        effect: Effect::Mods(&[(ModKey::BeamDmg, 1.10)]), hidden: false },
    Programme { id: "weap_magnetic_accelerators", field: Field::Weapons, school: None, tier: 1,
        name: "Magnetic Accelerators", blurb: "+10% driver damage.",
        effect: Effect::Mods(&[(ModKey::DriverDmg, 1.10)]), hidden: false },
    Programme { id: "weap_warhead_yields", field: Field::Weapons, school: None, tier: 1,
        name: "Warhead Yields", blurb: "+10% torpedo damage.",
        effect: Effect::Mods(&[(ModKey::TorpDmg, 1.10)]), hidden: false },
    // Shared II (gate: 5 battles fought)
    Programme { id: "weap_hardened_magazines", field: Field::Weapons, school: None, tier: 2,
        name: "Hardened Magazines", blurb: "Warship Armaments cost −15%.",
        effect: Effect::Mods(&[(ModKey::WarshipCost, 0.85)]), hidden: false },
    Programme { id: "weap_munitions_lines", field: Field::Weapons, school: None, tier: 2,
        name: "Munitions Lines", blurb: "Modules −25% build time, −15% cost.",
        effect: Effect::Mods(&[(ModKey::ModuleBuildTime, 0.75), (ModKey::ModuleCost, 0.85)]), hidden: false },
    Programme { id: "weap_fire_discipline", field: Field::Weapons, school: None, tier: 2,
        name: "Fire Discipline", blurb: "Your opening-round damage +10%.",
        effect: Effect::Mods(&[(ModKey::OpeningRoundDmg, 1.10)]), hidden: false },
    // ⑂ STRIKE SYSTEMS (gate: 150 hull-mass destroyed)
    Programme { id: "weap_strike_iii_lance_array", field: Field::Weapons, school: Some(School::Strike), tier: 3,
        name: "Lance Array", blurb: "Heavy-beam module, doubly vulnerable to Reflective (arrives with the weapon-module pass).",
        effect: PENDING, hidden: false },
    Programme { id: "weap_strike_iii_breaching_ordnance", field: Field::Weapons, school: Some(School::Strike), tier: 3,
        name: "Breaching Ordnance", blurb: "+25% damage to Defense Platforms.",
        effect: Effect::Mods(&[(ModKey::DmgVsPlatforms, 1.25)]), hidden: false },
    Programme { id: "weap_strike_iv_platform_lances", field: Field::Weapons, school: Some(School::Strike), tier: 4,
        name: "Platform Lances", blurb: "Defense Platforms mount Lance-profile beam fire.",
        effect: Effect::Flag(Cap::PlatformLances), hidden: false },
    Programme { id: "weap_strike_iv_penetrator_slugs", field: Field::Weapons, school: Some(School::Strike), tier: 4,
        name: "Penetrator Slugs", blurb: "+15% driver damage.",
        effect: Effect::Mods(&[(ModKey::DriverDmg, 1.15)]), hidden: false },
    Programme { id: "weap_strike_v_first_strike", field: Field::Weapons, school: Some(School::Strike), tier: 5,
        name: "First Strike", blurb: "Attacking side's opening round +50%.",
        effect: Effect::Flag(Cap::FirstStrike), hidden: false },
    Programme { id: "weap_strike_v_overpressure", field: Field::Weapons, school: Some(School::Strike), tier: 5,
        name: "Overpressure Warheads", blurb: "Torpedo kills splash 10% into the victim's stack (arrives with the combat pass).",
        effect: PENDING, hidden: false },
    // ⑂ COUNTERMEASURES (gate: absorb 150 hull-mass of damage)
    Programme { id: "weap_cm_iii_flak_doctrine", field: Field::Weapons, school: Some(School::Countermeasures), tier: 3,
        name: "Flak Doctrine", blurb: "Point-Defense interception 0.60 → 0.75.",
        effect: Effect::Mods(&[(ModKey::PdIntercept, 0.75 / 0.60)]), hidden: false },
    Programme { id: "weap_cm_iii_ablative_refits", field: Field::Weapons, school: Some(School::Countermeasures), tier: 3,
        name: "Ablative Refits", blurb: "Reflective blunt 0.35 → 0.45.",
        effect: Effect::Mods(&[(ModKey::ReflectBlunt, 0.45 / 0.35)]), hidden: false },
    Programme { id: "weap_cm_iv_platform_netfire", field: Field::Weapons, school: Some(School::Countermeasures), tier: 4,
        name: "Platform Netfire", blurb: "Defense Platforms project PD interception over the defending side.",
        effect: Effect::Flag(Cap::PlatformNetfire), hidden: false },
    Programme { id: "weap_cm_iv_spall_liners", field: Field::Weapons, school: Some(School::Countermeasures), tier: 4,
        name: "Spall Liners", blurb: "Whipple blunt 0.45 → 0.55.",
        effect: Effect::Mods(&[(ModKey::WhippleBlunt, 0.55 / 0.45)]), hidden: false },
    Programme { id: "weap_cm_v_grand_batteries", field: Field::Weapons, school: Some(School::Countermeasures), tier: 5,
        name: "Grand Batteries", blurb: "Defense Platform V; defended side gains an opening-round alpha.",
        effect: Effect::Flag(Cap::GrandBatteries), hidden: false },
    Programme { id: "weap_cm_v_citadel_compartments", field: Field::Weapons, school: Some(School::Countermeasures), tier: 5,
        name: "Citadel Compartments", blurb: "Your warships' damage pools run 20% deeper.",
        effect: Effect::Mods(&[(ModKey::DamagePoolDepth, 1.20)]), hidden: false },

    // ═══════════════════ 5 · HULLS ═══════════════════
    // Shared I
    Programme { id: "hull_drydock_efficiency", field: Field::Hulls, school: None, tier: 1,
        name: "Drydock Efficiency", blurb: "Warship build time −20%.",
        effect: Effect::Mods(&[(ModKey::WarshipBuildTime, 0.80)]), hidden: false },
    Programme { id: "hull_reinforced_frames", field: Field::Hulls, school: None, tier: 1,
        name: "Reinforced Frames", blurb: "Warship hull mass +10% — tougher in the attrition math.",
        effect: Effect::Mods(&[(ModKey::HullMass, 1.10)]), hidden: false },
    Programme { id: "hull_slipway_standards", field: Field::Hulls, school: None, tier: 1,
        name: "Slipway Standards", blurb: "Warship goods cost −10%.",
        effect: Effect::Mods(&[(ModKey::WarshipCost, 0.90)]), hidden: false },
    // Shared II (gate: 15 warships commissioned)
    Programme { id: "hull_rapid_rearm", field: Field::Hulls, school: None, tier: 2,
        name: "Rapid Rearm", blurb: "Damage pools recover faster at friendly Shipyards.",
        effect: Effect::Mods(&[(ModKey::RearmRate, 1.25)]), hidden: false },
    Programme { id: "hull_modular_berths", field: Field::Hulls, school: None, tier: 2,
        name: "Modular Berths", blurb: "Refits −50% time.",
        effect: Effect::Mods(&[(ModKey::RefitTime, 0.50)]), hidden: false },
    Programme { id: "hull_compartmentalization", field: Field::Hulls, school: None, tier: 2,
        name: "Compartmentalization", blurb: "Warship damage pools run 10% deeper.",
        effect: Effect::Mods(&[(ModKey::DamagePoolDepth, 1.10)]), hidden: false },
    // ⑂ LINE — heavy displacement (gate: 8 battles won)
    Programme { id: "hull_line_iii_hardened_anchorage", field: Field::Hulls, school: Some(School::Line), tier: 3,
        name: "Hardened Anchorage", blurb: "Anchored fleets at owned systems take −15% damage.",
        effect: Effect::Mods(&[(ModKey::AnchoredDmgTaken, 0.85)]), hidden: false },
    Programme { id: "hull_line_iii_escort_datalink", field: Field::Hulls, school: Some(School::Line), tier: 3,
        name: "Escort Datalink", blurb: "Corvette utility module: +screening (arrives with the utility-module pass).",
        effect: PENDING, hidden: false },
    Programme { id: "hull_line_iv_destroyer", field: Field::Hulls, school: Some(School::Line), tier: 4,
        name: "Destroyer Hull", blurb: "Heavy combatant: 3 module slots, slow, Armaments+Machinery-hungry (arrives with the hull pass).",
        effect: PENDING, hidden: false },
    Programme { id: "hull_line_iv_fleet_escorts", field: Field::Hulls, school: Some(School::Line), tier: 4,
        name: "Fleet Escorts", blurb: "Convoys under Corvette escort take −25% raid damage.",
        effect: Effect::Mods(&[(ModKey::EscortedConvoyRaidDmg, 0.75)]), hidden: false },
    Programme { id: "hull_line_v_cruiser", field: Field::Hulls, school: Some(School::Line), tier: 5,
        name: "Cruiser Hull", blurb: "The season's prestige ship: 4 module slots, slowest combatant (arrives with the hull pass).",
        effect: PENDING, hidden: false },
    Programme { id: "hull_line_v_fortress_doctrine", field: Field::Hulls, school: Some(School::Line), tier: 5,
        name: "Fortress Doctrine", blurb: "Friendly fleets disengage without exposure at your defended systems.",
        effect: Effect::Flag(Cap::FortressDoctrine), hidden: false },
    // ⑂ CORSAIR — light displacement (gate: 8 successful raids)
    Programme { id: "hull_corsair_iii_wolfpack", field: Field::Hulls, school: Some(School::Corsair), tier: 3,
        name: "Wolfpack Doctrine", blurb: "Raider stacks +15% attack per additional raider stack, capped ×2.",
        effect: Effect::Mods(&[(ModKey::WolfpackPerStack, 1.15)]), hidden: false },
    Programme { id: "hull_corsair_iii_prize_holds", field: Field::Hulls, school: Some(School::Corsair), tier: 3,
        name: "Prize Holds", blurb: "Raids steal +50% cargo.",
        effect: Effect::Mods(&[(ModKey::RaidSteal, 1.50)]), hidden: false },
    Programme { id: "hull_corsair_iv_slip_anchors", field: Field::Hulls, school: Some(School::Corsair), tier: 4,
        name: "Slip Anchors", blurb: "Raiders disengage with −50% exposure.",
        effect: Effect::Mods(&[(ModKey::RaiderDisengageExposure, 0.50)]), hidden: false },
    Programme { id: "hull_corsair_iv_blockade_runners", field: Field::Hulls, school: Some(School::Corsair), tier: 4,
        name: "Blockade Runners", blurb: "Convoy refit variant: +30% speed, −20% cargo (arrives with the refit pass).",
        effect: PENDING, hidden: false },
    Programme { id: "hull_corsair_v_salvage_rigs", field: Field::Hulls, school: Some(School::Corsair), tier: 5,
        name: "Salvage Rigs", blurb: "Wreck module salvage (deferred).",
        effect: Effect::Flag(Cap::SalvageRigs), hidden: true },
    Programme { id: "hull_corsair_v_boarding_parties", field: Field::Hulls, school: Some(School::Corsair), tier: 5,
        name: "Boarding Parties", blurb: "Capture intercepted cargo (deferred).",
        effect: Effect::Flag(Cap::BoardingParties), hidden: true },

    // ═══════════════════ 6 · LIFE & HABITATION ═══════════════════
    // Shared I
    Programme { id: "life_hydroponics", field: Field::Life, school: None, tier: 1,
        name: "Hydroponics", blurb: "+15% Agroplex output.",
        effect: Effect::Mods(&[(ModKey::AgroplexYield, 1.15)]), hidden: false },
    Programme { id: "life_med_bays", field: Field::Life, school: None, tier: 1,
        name: "Med Bays", blurb: "+10% population growth.",
        effect: Effect::Mods(&[(ModKey::PopGrowth, 1.10)]), hidden: false },
    Programme { id: "life_dense_housing", field: Field::Life, school: None, tier: 1,
        name: "Dense Housing", blurb: "+25% Habitat capacity.",
        effect: Effect::Mods(&[(ModKey::HabitatCap, 1.25)]), hidden: false },
    // Shared II (gate: 5M population grown)
    Programme { id: "life_civic_rations", field: Field::Life, school: None, tier: 2,
        name: "Civic Rations", blurb: "Rationing penalty 85% → 92%.",
        effect: Effect::Mods(&[(ModKey::RationingFloor, 0.92 / 0.85)]), hidden: false },
    Programme { id: "life_cryo_berths", field: Field::Life, school: None, tier: 2,
        name: "Cryo Berths", blurb: "Colony ships arrive at half capacity.",
        effect: Effect::Flag(Cap::CryoBerths), hidden: false },
    Programme { id: "life_gene_crops", field: Field::Life, school: None, tier: 2,
        name: "Gene-Tailored Crops", blurb: "Agroplex +25% output, −20% Biomass input.",
        effect: Effect::Mods(&[(ModKey::AgroplexYield, 1.25), (ModKey::AgroplexInputs, 0.80)]), hidden: false },
    // ⑂ GROWTH — settlement (gate: 20M total population)
    Programme { id: "life_growth_iii_orbital_habitats", field: Field::Life, school: Some(School::Growth), tier: 3,
        name: "Orbital Habitats", blurb: "Habitat buildable on station bodies (arrives with the habitat pass).",
        effect: PENDING, hidden: false },
    Programme { id: "life_growth_iii_boom_charters", field: Field::Life, school: Some(School::Growth), tier: 3,
        name: "Boom Charters", blurb: "+20% growth on bodies below half capacity.",
        effect: Effect::Mods(&[(ModKey::GrowthBelowHalf, 1.20)]), hidden: false },
    Programme { id: "life_growth_iv_xenoacclimation", field: Field::Life, school: Some(School::Growth), tier: 4,
        name: "Xenoacclimation", blurb: "Marginal body kinds become colonizable.",
        effect: Effect::Flag(Cap::Xenoacclimation), hidden: false },
    Programme { id: "life_growth_iv_closed_loop", field: Field::Life, school: Some(School::Growth), tier: 4,
        name: "Closed-Loop Ecology", blurb: "Provisions consumption −30%.",
        effect: Effect::Mods(&[(ModKey::ProvisionsUse, 0.70)]), hidden: false },
    Programme { id: "life_growth_v_arcologies", field: Field::Life, school: Some(School::Growth), tier: 5,
        name: "Arcologies", blurb: "Habitat tier V.",
        effect: Effect::UnlockStructureTier(StructureKind::Habitat, 5), hidden: false },
    Programme { id: "life_growth_v_generation_charters", field: Field::Life, school: Some(School::Growth), tier: 5,
        name: "Generation Charters", blurb: "Colony ships carry one specialist and arrive WellSupplied.",
        effect: Effect::Flag(Cap::GenerationCharters), hidden: false },
    // ⑂ TALENT — institutional (gate: 8 specialists trained)
    Programme { id: "life_talent_iii_expert_training", field: Field::Life, school: Some(School::Talent), tier: 3,
        name: "Expert Training", blurb: "Unlocks the 2.25× expert specialist tier.",
        effect: Effect::Flag(Cap::ExpertTraining), hidden: false },
    Programme { id: "life_talent_iii_deep_careers", field: Field::Life, school: Some(School::Talent), tier: 3,
        name: "Deep Careers", blurb: "Specialist training time −40%.",
        effect: Effect::Mods(&[(ModKey::TrainingTime, 0.60)]), hidden: false },
    Programme { id: "life_talent_iv_academy_iv", field: Field::Life, school: Some(School::Talent), tier: 4,
        name: "Academy IV", blurb: "Academy tier IV.",
        effect: Effect::UnlockStructureTier(StructureKind::Academy, 4), hidden: false },
    Programme { id: "life_talent_iv_broad_curricula", field: Field::Life, school: Some(School::Talent), tier: 4,
        name: "Broad Curricula", blurb: "Academies run 2 concurrent training jobs.",
        effect: Effect::Flag(Cap::BroadCurricula), hidden: false },
    Programme { id: "life_talent_v_twin_campuses", field: Field::Life, school: Some(School::Talent), tier: 5,
        name: "Twin Campuses", blurb: "Each Habitat tier houses +1 specialist.",
        effect: Effect::Flag(Cap::TwinCampuses), hidden: false },
    Programme { id: "life_talent_v_founders_institutes", field: Field::Life, school: Some(School::Talent), tier: 5,
        name: "Founders' Institutes", blurb: "Newly trained specialists start expert.",
        effect: Effect::Flag(Cap::FoundersInstitutes), hidden: false },
];

/// Look up a catalog programme by id.
pub fn programme(id: &str) -> Option<&'static Programme> {
    CATALOG.iter().find(|p| p.id == id)
}

/// Every VISIBLE (non-hidden) programme id — the researchable universe.
pub fn visible_ids() -> impl Iterator<Item = &'static str> {
    CATALOG.iter().filter(|p| !p.hidden).map(|p| p.id)
}

// ─────────────────────────────────────────────────────────────────────────────
// AVAILABILITY & GATE LOGIC (pure)
// ─────────────────────────────────────────────────────────────────────────────

/// Is `gate` currently satisfied, given the syndicate's verbs, the live metric
/// reader, and the current sim time?
pub fn gate_met(
    gate: &Gate,
    state: &ResearchState,
    metric: &dyn Fn(Metric) -> f64,
    now: f64,
) -> bool {
    match gate {
        Gate::None => true,
        Gate::Cumulative(v, t) => state.verb(*v) + 1e-9 >= *t,
        Gate::State(m, t) => metric(*m) + 1e-9 >= *t,
        Gate::Sustained(m, _t, secs) => state
            .sustained_since
            .get(m)
            .is_some_and(|since| now - *since as f64 + 1e-9 >= *secs as f64),
    }
}

/// Does completed programme `q` satisfy the LADDER predecessor of `p`? A tier is
/// opened by any Tier-(N−1) completion on the same ladder: Tier II/III chain off
/// the field's SHARED ladder; Tiers IV/V chain off the SAME SCHOOL.
fn is_predecessor(q: &Programme, p: &Programme) -> bool {
    if q.tier + 1 != p.tier || q.field != p.field {
        return false;
    }
    match p.tier {
        2 | 3 => q.school.is_none(), // shared Tier I/II opens Tier II/III
        _ => q.school == p.school,   // same-school chain for IV/V
    }
}

/// Is programme `id` researchable now for this syndicate? Pure: Tier I is always
/// open (hidden entries never are); higher tiers need the tier gate met AND a
/// completed Tier-(N−1) programme on the ladder.
pub fn is_available(
    id: &str,
    state: &ResearchState,
    metric: &dyn Fn(Metric) -> f64,
    now: f64,
) -> bool {
    let Some(p) = programme(id) else { return false };
    if p.hidden {
        return false;
    }
    if state.has(id) {
        return false; // already researched
    }
    if p.tier == 1 {
        return true;
    }
    let gate = tier_gate(p.field, p.school, p.tier);
    if !gate_met(&gate, state, metric, now) {
        return false;
    }
    state
        .completed
        .iter()
        .filter_map(|cid| programme(cid))
        .any(|q| is_predecessor(q, p))
}

/// The throughput-seconds cost of `id` (0 for an unknown id).
pub fn cost_of(id: &str) -> f64 {
    programme(id).map(|p| tier_cost_secs(p.tier)).unwrap_or(0.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// THE `mods` LOOKUP LAYER (R4 reads it; completion just updates `completed`)
// ─────────────────────────────────────────────────────────────────────────────

/// The aggregate EFFECT MODS a syndicate's completed research grants: for every
/// completed `Effect::Mods`, fold its factors in — multiplicatively for normal
/// keys (base 1.0), additively for the bucket keys (base 0.0). Recomputed on
/// demand (design decision #5: effects are lazy over `completed`, never stored),
/// so completion is instant and galaxy-wide.
pub fn mods(state: &ResearchState) -> BTreeMap<ModKey, f64> {
    let mut m: BTreeMap<ModKey, f64> = BTreeMap::new();
    for id in &state.completed {
        if let Some(Effect::Mods(list)) = programme(id).map(|p| p.effect) {
            for (key, factor) in list {
                if key.is_additive() {
                    *m.entry(*key).or_insert(0.0) += *factor;
                } else {
                    *m.entry(*key).or_insert(1.0) *= *factor;
                }
            }
        }
    }
    m
}

/// A single mod key's value for a syndicate (identity default: 1.0 multiplicative,
/// 0.0 additive). The convenience R4 sites call.
pub fn mod_of(state: &ResearchState, key: ModKey) -> f64 {
    mods(state).get(&key).copied().unwrap_or(if key.is_additive() { 0.0 } else { 1.0 })
}

/// Has this syndicate unlocked the given capability flag?
pub fn has_flag(state: &ResearchState, cap: Cap) -> bool {
    state.completed.iter().any(|id| programme(id).map(|p| p.effect) == Some(Effect::Flag(cap)))
}

/// Has this syndicate unlocked the given hull?
pub fn has_hull(state: &ResearchState, kind: ShipKind) -> bool {
    state.completed.iter().any(|id| programme(id).map(|p| p.effect) == Some(Effect::UnlockHull(kind)))
}

/// Has this syndicate unlocked the given module kind?
pub fn has_module(state: &ResearchState, kind: ModuleKind) -> bool {
    state.completed.iter().any(|id| programme(id).map(|p| p.effect) == Some(Effect::UnlockModule(kind)))
}

/// The best UNLOCKED tier for `kind` from research (0 = none granted). A site
/// takes `max(base_tier, this)` to gate tier-IV/V builds.
pub fn unlocked_structure_tier(state: &ResearchState, kind: StructureKind) -> u32 {
    state
        .completed
        .iter()
        .filter_map(|id| match programme(id).map(|p| p.effect) {
            Some(Effect::UnlockStructureTier(k, t)) if k == kind => Some(t),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metrics reader that reports nothing satisfied (for gate tests).
    fn no_metrics(_: Metric) -> f64 {
        0.0
    }

    #[test]
    fn catalog_is_well_formed() {
        let mut seen = BTreeSet::new();
        for p in CATALOG {
            assert!(seen.insert(p.id), "duplicate catalog id {}", p.id);
            assert!((1..=5).contains(&p.tier), "{} has bad tier {}", p.id, p.tier);
            // Tier I/II are shared (school None); III–V belong to a school of the field.
            if p.tier <= 2 {
                assert!(p.school.is_none(), "{} tier {} must be shared", p.id, p.tier);
            } else {
                let s = p.school.expect("school tier needs a school");
                assert_eq!(s.field(), p.field, "{}'s school is in the wrong field", p.id);
            }
        }
    }

    #[test]
    fn catalog_is_the_full_six_board_tree() {
        // 6 fields × (3 shared-I + 3 shared-II + two schools × (2+2+2)) = 6 × 18 = 108.
        assert_eq!(CATALOG.len(), 108, "the full v6 tree is 108 programmes");
        for field in [Field::Propulsion, Field::Materials, Field::Computation, Field::Weapons, Field::Hulls, Field::Life] {
            let of_field: Vec<&Programme> = CATALOG.iter().filter(|p| p.field == field).collect();
            assert_eq!(of_field.len(), 18, "{field:?} has 18 programmes");
            // Exactly two schools, each a III/IV/V ladder of 2+2+2.
            let schools: BTreeSet<School> = of_field.iter().filter_map(|p| p.school).collect();
            assert_eq!(schools.len(), 2, "{field:?} forks into exactly two schools");
            for s in schools {
                for t in 3..=5u8 {
                    let n = of_field.iter().filter(|p| p.school == Some(s) && p.tier == t).count();
                    assert_eq!(n, 2, "{s:?} tier {t} has two programmes");
                }
            }
            // Three shared programmes at each of tiers I and II.
            for t in 1..=2u8 {
                let n = of_field.iter().filter(|p| p.school.is_none() && p.tier == t).count();
                assert_eq!(n, 3, "{field:?} shared tier {t} has three programmes");
            }
        }
        // Exactly the two documented hidden entries.
        let hidden: Vec<&str> = CATALOG.iter().filter(|p| p.hidden).map(|p| p.id).collect();
        assert_eq!(hidden, vec!["hull_corsair_v_salvage_rigs", "hull_corsair_v_boarding_parties"]);
        // Every catalog id resolves and every non-hidden one is visible.
        assert_eq!(visible_ids().count(), 106);
    }

    #[test]
    fn tier_one_is_always_open_and_hidden_never() {
        let st = ResearchState::default();
        assert!(is_available("prop_drive_tuning", &st, &no_metrics, 0.0), "Tier I opens immediately");
        assert!(!is_available("hull_corsair_v_salvage_rigs", &st, &no_metrics, 0.0), "hidden is never researchable");
        assert!(!is_available("nonexistent", &st, &no_metrics, 0.0));
    }

    #[test]
    fn tier_two_needs_the_field_gate_and_a_tier_one_completion() {
        let mut st = ResearchState::default();
        // Military Drives (Prop II) needs a Prop-I completion AND 200 ly flown.
        assert!(!is_available("prop_military_drives", &st, &no_metrics, 0.0), "gated + no predecessor");
        st.completed.insert("prop_drive_tuning".into()); // a Tier-I of the field
        assert!(!is_available("prop_military_drives", &st, &no_metrics, 0.0), "predecessor alone isn't enough");
        st.add_verb(Verb::LyFlown, 200.0);
        assert!(is_available("prop_military_drives", &st, &no_metrics, 0.0), "gate + predecessor opens it");
    }

    #[test]
    fn ladder_rule_deep_rush_skips_siblings_but_they_remain_researchable() {
        let mut st = ResearchState::default();
        // Rush the LINE HAUL school: complete a Prop-II, meet the school gate,
        // complete Express Charters (III), then Drop Berths (IV) opens — without
        // having done the OTHER Tier-III sibling.
        st.completed.insert("prop_drive_tuning".into());
        st.add_verb(Verb::LyFlown, 200.0);
        st.completed.insert("prop_military_drives".into()); // a shared Tier-II
        st.add_verb(Verb::ConvoyDeliveries, 30.0); // LINE HAUL gate
        assert!(is_available("prop_line_express_charters", &st, &no_metrics, 0.0), "school III opens");
        st.completed.insert("prop_line_express_charters".into());
        assert!(is_available("prop_line_drop_berths", &st, &no_metrics, 0.0), "IV opens off a III completion");
        assert!(!is_available("prop_line_autonomous_freight", &st, &no_metrics, 0.0), "V still needs a IV first");
    }

    #[test]
    fn state_and_sustained_gates() {
        let mut st = ResearchState::default();
        st.completed.insert("mat_deep_bores".into()); // unrelated, just to have some state
        // GROWTH school gate is a State(TotalPopulation ≥ 20M) — 20.0 in the
        // internal millions unit.
        let pop20 = |m: Metric| if m == Metric::TotalPopulation { 20.0 } else { 0.0 };
        // Need a Life Tier-II predecessor + the state gate; simulate the predecessor.
        st.completed.insert("life_shared_ii".into()); // not in catalog → no predecessor effect
        assert!(!is_available("life_growth_iii_boom_charters", &st, &pop20, 0.0), "no real Life-II predecessor yet");
        // Sustained gate shape: satisfied only after the duration elapses.
        let g = Gate::Sustained(Metric::WellSuppliedSystems, 5.0, 100);
        assert!(!gate_met(&g, &st, &no_metrics, 50.0), "not yet satisfied");
        st.sustained_since.insert(Metric::WellSuppliedSystems, 0);
        assert!(!gate_met(&g, &st, &no_metrics, 50.0), "50s < 100s window");
        assert!(gate_met(&g, &st, &no_metrics, 100.0), "window elapsed");
    }

    #[test]
    fn mods_fold_multiplicatively_and_only_for_completed() {
        let mut st = ResearchState::default();
        assert!((mod_of(&st, ModKey::SpeedAll) - 1.0).abs() < 1e-9, "identity default 1.0");
        st.completed.insert("prop_drive_tuning".into()); // SpeedAll ×1.10
        assert!((mod_of(&st, ModKey::SpeedAll) - 1.10).abs() < 1e-9);
        // Compound effect: Drop Berths tunes two keys.
        st.completed.insert("prop_line_drop_berths".into());
        assert!((mod_of(&st, ModKey::ColonyCost) - 0.70).abs() < 1e-9);
        assert!((mod_of(&st, ModKey::ColonyBuildTime) - 0.70).abs() < 1e-9);
    }

    #[test]
    fn set_queue_promotes_and_completion_advances() {
        let mut st = ResearchState::default();
        st.set_queue(vec!["prop_drive_tuning".into(), "prop_bunkerage".into()]);
        assert_eq!(st.active.as_deref(), Some("prop_drive_tuning"), "front promoted to active");
        assert_eq!(st.queue, vec!["prop_bunkerage".to_string()]);
        // Not funded → no completion.
        assert!(st.try_complete().is_none());
        // Fund past the T1 cost with a little overflow.
        st.progress = tier_cost_secs(1) + 500.0;
        let done = st.try_complete();
        assert_eq!(done.as_deref(), Some("prop_drive_tuning"), "completes the funded active");
        assert!(st.has("prop_drive_tuning"));
        assert_eq!(st.active.as_deref(), Some("prop_bunkerage"), "queue auto-advances");
        assert!((st.progress - 500.0).abs() < 1e-6, "overflow carries to the next");
        // Empty the queue: completing the last leaves active None, progress 0.
        st.progress = tier_cost_secs(1);
        st.try_complete();
        assert!(st.active.is_none() && st.queue.is_empty());
        assert_eq!(st.progress, 0.0);
    }

    #[test]
    fn unlock_lookups() {
        let mut st = ResearchState::default();
        assert_eq!(unlocked_structure_tier(&st, StructureKind::Shipyard), 0);
        assert!(!has_flag(&st, Cap::AutonomousFreight));
        st.completed.insert("mat_foundry_iv_orbital_yards".into()); // Shipyard IV unlock
        st.completed.insert("prop_line_autonomous_freight".into()); // a capability flag
        assert_eq!(unlocked_structure_tier(&st, StructureKind::Shipyard), 4);
        assert!(has_flag(&st, Cap::AutonomousFreight));
        assert!(!has_flag(&st, Cap::SalvageRigs), "hidden entry never completed");
        // The UnlockHull lookup compiles + is identity-empty until R4b adds hulls.
        assert!(!has_hull(&st, ShipKind::Corvette));
    }
}

//! EXPLORATION FOG (§explore) — the deposit knowledge ladder.
//!
//! Deposits used to be public knowledge (a full-galaxy geology dump at join).
//! This module fogs them behind a LADDER — the composition-ladder idiom (§13.1)
//! applied to geology:
//!
//! * **R0/R1 — public, free**: position, name, star type, and a richness BAND
//!   (Poor / Fair / Rich) — the spectral read anyone can take from home. The
//!   band is a pure function of a system's STATIC deposits, so it is the same
//!   for every corp and never changes: safe to ship in the once-at-join galaxy.
//! * **R2/R3 — Surveyed (corp knowledge, permanent)**: exact deposits,
//!   planetary grades/features, and the system economic trait. Static survey
//!   data never stales. Lives in
//!   [`crate::world::Corporation::surveyed`], gated in the server View.
//!
//! The band VALUE weights are the market's fixed BOOTSTRAP PRICE ANCHORS
//! ([`crate::market::base_price`] — Provisions 6 · Ore 8 · Fuel 10 · Volatiles 18
//! · Alloys 26), NOT live prices: the band must be static and public, and those
//! anchors are already the one fixed per-commodity value table (the client
//! mirrors them as `COMMODITY_VALUE`; `claim_cost_for` uses them too). One
//! source of truth — a second weight table would only drift.
//!
//! Band THRESHOLDS are the terciles of `band_value` across all systems, computed
//! once at galaxy generation and stored on the `World` (deposits never change, so
//! neither do the terciles). Pre-feature snapshots load with the serde default
//! (0,0) and are healed by `World::fixup_after_load` — a pure recompute.

use serde::{Deserialize, Serialize};

use crate::body::{Body, BodySize, BodySpecial, Environment};
use crate::build::StructureKind;
use crate::cargo::Commodity;
use crate::galaxy::{Deposit, StarSystem};

// ── Tunables (playtest values; every one a named constant) ──────────────────────

/// Systems within this radius of a corp's HOME are pre-surveyed at join — your
/// starting valley is known; the frontier isn't. Tunable.
pub const SURVEY_INITIAL_RADIUS: f64 = 1200.0;

/// §explore Part 2 — the SURVEY order (the scout's second job). The fleet must
/// close to within this range of the star to survey (hulls on-site only — no
/// remote surveying). Tunable.
pub const SURVEY_RANGE: f64 = 120.0;

/// The DWELL: how long the fleet must hold on-site, uninterrupted, for the
/// survey to complete. All-or-nothing — leaving range or entering an engagement
/// aborts with no partial credit. Tunable.
pub const SURVEY_SECS: f64 = 20.0;

/// ACTIVE SENSING IS LOUD: while dwelling, the fleet's detection signature is
/// multiplied by this (> 1 — seen from farther). Applies during the dwell window
/// ONLY, through the one shared `detection::signature` path — the risk price of
/// knowledge. Tunable.
pub const SURVEY_SIGNATURE_FACTOR: f64 = 1.5;

// ── §explore Part 3: surveyed economic TRAITS ──────────────────────────────────

/// Fraction of systems carrying exactly ONE economic trait (uniform over the five),
/// rolled at galaxy generation from an isolated seeded stream. Tunable.
pub const TRAIT_FRACTION: f64 = 0.25;

/// Bonus Vein: that commodity's deposit richness ×this in accrual. Tunable.
pub const BONUS_VEIN_MULT: f64 = 1.5;

/// Deep Deposits: base richness ×this — but the Extractor multiplier applies as
/// `^(tier−1)` (the FIRST Extractor tier is wasted breaking through). Tunable.
pub const DEEP_DEPOSITS_BASE_MULT: f64 = 1.5;

/// Unstable Geology: development (upgrade) recipe costs ×this here. Tunable.
pub const UNSTABLE_COST_MULT: f64 = 1.25;

/// Volatile Pockets: Refinery output ×this here. Tunable.
pub const VOLATILE_REFINERY_MULT: f64 = 1.3;

/// Precursor Cache: a ONE-TIME Alloys grant to the stockpile at claim completion
/// (latched — pays exactly once, ever, including across capture). Tunable.
pub const PRECURSOR_ALLOYS: f64 = 40.0;

/// Lucky ground may make ONE natural production leg spectacular, but geology,
/// a matching body special and a system trait may never multiply into an
/// all-purpose runaway. Staffing, tier throughput, specialists and research sit
/// outside this cap: investment still scales a jackpot after discovery.
pub const NATURAL_SPECIALTY_CAP: f64 = 3.0;

/// The first rung at which a survey calls a site a meaningful colony role.
pub const ROLE_USEFUL_MIN: f64 = 1.30;
/// Top-decile target band: a discovery strong enough to reconsider build order.
pub const ROLE_EXCEPTIONAL_MIN: f64 = 1.80;
/// Rare single-specialty jackpot; the natural geography cap lands at ×3.
pub const ROLE_JACKPOT_MIN: f64 = 2.50;

/// A system economic trait — revealed by survey or ownership. Effects are
/// ALWAYS-ON ground truth, applying regardless of when a corporation learned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SystemTrait {
    /// One commodity's vein runs deep — its richness ×`BONUS_VEIN_MULT`.
    BonusVein { commodity: crate::cargo::Commodity },
    /// Richer ground, harder to open: base ×`DEEP_DEPOSITS_BASE_MULT`, but the
    /// Extractor multiplier applies as `^(tier−1)` — tier 1 is wasted.
    DeepDeposits,
    /// The lemon: development recipe costs ×`UNSTABLE_COST_MULT` here.
    UnstableGeology,
    /// Refinery output ×`VOLATILE_REFINERY_MULT` here.
    VolatilePockets,
    /// A one-time `PRECURSOR_ALLOYS` stockpile grant at claim (latched).
    PrecursorCache,
}

impl SystemTrait {
    /// Stable machine slug (the surveyed-or-owner wire form).
    pub fn slug(self) -> &'static str {
        match self {
            SystemTrait::BonusVein { .. } => "bonus_vein",
            SystemTrait::DeepDeposits => "deep_deposits",
            SystemTrait::UnstableGeology => "unstable_geology",
            SystemTrait::VolatilePockets => "volatile_pockets",
            SystemTrait::PrecursorCache => "precursor_cache",
        }
    }

    /// Human title for the reveal notice / panel line.
    pub fn title(self) -> &'static str {
        match self {
            SystemTrait::BonusVein { .. } => "Bonus Vein",
            SystemTrait::DeepDeposits => "Deep Deposits",
            SystemTrait::UnstableGeology => "Unstable Geology",
            SystemTrait::VolatilePockets => "Volatile Pockets",
            SystemTrait::PrecursorCache => "Precursor Cache",
        }
    }
}

/// One legible use for a surveyed colony. These are derived from authoritative
/// planetary facts, never stored: changing a tuning immediately changes both
/// production and the survey's explanation of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColonyRole {
    PopulationWorld,
    MiningWorld,
    FuelComplex,
    ElectronicsCenter,
    AgriculturalExporter,
    ShipbuildingCenter,
    StrategicOutpost,
}

impl ColonyRole {
    pub fn slug(self) -> &'static str {
        match self {
            Self::PopulationWorld => "population_world",
            Self::MiningWorld => "mining_world",
            Self::FuelComplex => "fuel_complex",
            Self::ElectronicsCenter => "electronics_center",
            Self::AgriculturalExporter => "agricultural_exporter",
            Self::ShipbuildingCenter => "shipbuilding_center",
            Self::StrategicOutpost => "strategic_outpost",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::PopulationWorld => "Population World",
            Self::MiningWorld => "Mining World",
            Self::FuelComplex => "Fuel Complex",
            Self::ElectronicsCenter => "Electronics Center",
            Self::AgriculturalExporter => "Agricultural Exporter",
            Self::ShipbuildingCenter => "Shipbuilding Center",
            Self::StrategicOutpost => "Strategic Outpost",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpportunityTier {
    Useful,
    Exceptional,
    Jackpot,
}

impl OpportunityTier {
    pub fn for_score(score: f64) -> Option<Self> {
        if score >= ROLE_JACKPOT_MIN {
            Some(Self::Jackpot)
        } else if score >= ROLE_EXCEPTIONAL_MIN {
            Some(Self::Exceptional)
        } else if score >= ROLE_USEFUL_MIN {
            Some(Self::Useful)
        } else {
            None
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::Exceptional => "exceptional",
            Self::Jackpot => "jackpot",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColonyOpportunity {
    pub role: ColonyRole,
    pub tier: OpportunityTier,
    pub score: f64,
    pub body_id: Option<u32>,
    pub body_name: Option<String>,
    pub reason: String,
}

/// The natural extraction multiplier for a deposit's location: body geology +
/// matching special + matching system trait. This is the ONE stacking path used
/// by production, the owner factor-chain readout and survey opportunity scores.
pub fn extraction_site_mult(body: &Body, resource: Commodity, trait_: Option<SystemTrait>) -> f64 {
    let trait_mult = match trait_ {
        Some(SystemTrait::BonusVein { commodity }) if commodity == resource => BONUS_VEIN_MULT,
        Some(SystemTrait::DeepDeposits) => DEEP_DEPOSITS_BASE_MULT,
        _ => 1.0,
    };
    (body.extraction_mult(resource) * trait_mult).min(NATURAL_SPECIALTY_CAP)
}

/// Natural tier-I output for one deposit, capped against the home reference.
/// Raw richness participates in the rating, so a truly rich frontier vein is
/// exciting; no lucky combination can exceed ×3 before player investment.
pub fn natural_extraction_rate(body: &Body, deposit: &Deposit, trait_: Option<SystemTrait>) -> f64 {
    (deposit.richness * extraction_site_mult(body, deposit.resource, trait_))
        .min(crate::galaxy::DEPOSIT_BASE_RICHNESS * NATURAL_SPECIALTY_CAP)
}

/// Converter-site yield, including the one system trait that affects refining.
pub fn converter_site_mult(body: &Body, kind: StructureKind, trait_: Option<SystemTrait>) -> f64 {
    let system =
        if kind == StructureKind::FuelRefinery && trait_ == Some(SystemTrait::VolatilePockets) {
            VOLATILE_REFINERY_MULT
        } else {
            1.0
        };
    (body.converter_yield_mult(kind) * system).min(NATURAL_SPECIALTY_CAP)
}

fn population_score(body: &Body) -> f64 {
    let home_capacity =
        BodySize::Large.habitat_capacity_mult() * Environment::Terran.habitat_capacity_mult();
    let capacity = body.habitat_capacity_mult() / home_capacity;
    let growth = body.population_growth_mult() / Environment::Terran.growth_mult();
    let food = Environment::Terran.provisions_mult() / body.provisions_mult();
    let fertile = if body.profile.special == Some(BodySpecial::FertileBiosphere) {
        1.20
    } else {
        1.0
    };
    (0.50 * capacity + 0.30 * growth + 0.20 * food) * fertile
}

fn strongest_deposit(
    body: &Body,
    trait_: Option<SystemTrait>,
    predicate: impl Fn(Commodity) -> bool,
) -> Option<(Commodity, f64)> {
    body.deposits
        .iter()
        .filter(|d| predicate(d.resource))
        .map(|d| {
            (
                d.resource,
                natural_extraction_rate(body, d, trait_) / crate::galaxy::DEPOSIT_BASE_RICHNESS,
            )
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Derive the strongest distinct colony roles in a surveyed system. Scores are
/// normalized to the reliable home baseline and capped at ×3. `jump_neighbors`
/// is public chart geometry; it gives otherwise ordinary systems a positional
/// role without pretending their rocks became richer.
pub fn colony_opportunities(system: &StarSystem, jump_neighbors: usize) -> Vec<ColonyOpportunity> {
    let mut best: std::collections::BTreeMap<ColonyRole, ColonyOpportunity> =
        std::collections::BTreeMap::new();
    let mut consider = |role: ColonyRole, score: f64, body: Option<&Body>, reason: String| {
        let score = score.min(NATURAL_SPECIALTY_CAP);
        let Some(tier) = OpportunityTier::for_score(score) else {
            return;
        };
        let candidate = ColonyOpportunity {
            role,
            tier,
            score,
            body_id: body.map(|b| b.id),
            body_name: body.map(|b| b.name.clone()),
            reason,
        };
        if best.get(&role).is_none_or(|old| score > old.score) {
            best.insert(role, candidate);
        }
    };

    for body in &system.bodies {
        let pop = population_score(body);
        consider(
            ColonyRole::PopulationWorld,
            pop,
            Some(body),
            format!(
                "{} {} world: habitat ×{:.2}, growth ×{:.2}, provisions ×{:.2}",
                body.profile.size.slug(),
                body.profile.environment.slug(),
                body.habitat_capacity_mult(),
                body.population_growth_mult(),
                body.provisions_mult(),
            ),
        );

        if let Some((resource, score)) = strongest_deposit(body, system.trait_, |c| {
            matches!(c, Commodity::MetallicOre | Commodity::RareElements)
        }) {
            consider(
                ColonyRole::MiningWorld,
                score,
                Some(body),
                format!(
                    "{} on {} geology yields ×{score:.2} of the home reference",
                    resource.slug(),
                    body.profile.geology.slug(),
                ),
            );
        }

        if let Some((_, extraction)) =
            strongest_deposit(body, system.trait_, |c| c == Commodity::Volatiles)
        {
            let refining = converter_site_mult(body, StructureKind::FuelRefinery, system.trait_);
            let score = (extraction * refining).min(NATURAL_SPECIALTY_CAP);
            consider(
                ColonyRole::FuelComplex,
                score,
                Some(body),
                format!("Volatiles ×{extraction:.2} feeding local Fuel refining ×{refining:.2}",),
            );
        }

        if let Some((resource, extraction)) = strongest_deposit(body, system.trait_, |c| {
            matches!(c, Commodity::Silicates | Commodity::RareElements)
        }) {
            let fabrication =
                converter_site_mult(body, StructureKind::ElectronicsFabricator, system.trait_);
            let score = (extraction * fabrication).min(NATURAL_SPECIALTY_CAP);
            consider(
                ColonyRole::ElectronicsCenter,
                score,
                Some(body),
                format!(
                    "{} ×{extraction:.2} with Electronics fabrication ×{fabrication:.2}",
                    resource.slug(),
                ),
            );
        }

        if let Some((_, extraction)) =
            strongest_deposit(body, system.trait_, |c| c == Commodity::Biomass)
        {
            let provisions = converter_site_mult(body, StructureKind::Agroplex, system.trait_);
            let climate = Environment::Terran.provisions_mult() / body.provisions_mult();
            let score = (extraction * provisions * climate).min(NATURAL_SPECIALTY_CAP);
            consider(
                ColonyRole::AgriculturalExporter,
                score,
                Some(body),
                format!(
                    "Biomass ×{extraction:.2}, Agroplex ×{provisions:.2}, climate ×{climate:.2}",
                ),
            );
        }

        if body.profile.special == Some(BodySpecial::LowGravity) {
            let build_speed = 1.0 / (body.ship_build_time_mult() * body.construction_time_mult());
            let slot_support = 1.0 + 0.18 * body.base_industrial_slots().saturating_sub(1) as f64;
            let score = (build_speed * slot_support).min(NATURAL_SPECIALTY_CAP);
            consider(
                ColonyRole::ShipbuildingCenter,
                score,
                Some(body),
                format!(
                    "Low gravity with {} industrial slots: hull tempo ×{build_speed:.2}",
                    body.base_industrial_slots(),
                ),
            );
        }
    }

    // Two reachable chart nodes make a real bridge rather than a dead-end
    // colony. Location can earn a useful role, never an economic jackpot.
    if jump_neighbors >= 2 {
        let score = (1.30 + 0.10 * jump_neighbors.saturating_sub(2).min(3) as f64).min(1.60);
        consider(
            ColonyRole::StrategicOutpost,
            score,
            None,
            format!("{jump_neighbors} systems within one jump: staging, sensors and defense"),
        );
    }

    let mut out: Vec<_> = best.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.role.cmp(&b.role))
    });
    out
}

/// The public richness BAND — the spectral read (R1). Poor / Fair / Rich by the
/// terciles of the galaxy's system values. Should predict ~70% of a system's
/// worth; the survey buys the rest (the exact composition + the trait gamble).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RichnessBand {
    Poor,
    Fair,
    Rich,
}

impl RichnessBand {
    /// Stable machine slug (the wire form the client keys sizing/labels off).
    pub fn slug(self) -> &'static str {
        match self {
            RichnessBand::Poor => "poor",
            RichnessBand::Fair => "fair",
            RichnessBand::Rich => "rich",
        }
    }
}

/// A system's STATIC value scalar: `Σ dep.richness × base_price(commodity)` —
/// richness only (reserves deplete but richness doesn't), weighted by the fixed
/// bootstrap anchors. Pure + deterministic; the single banding input.
pub fn band_value(deposits: &[Deposit]) -> f64 {
    band_value_iter(deposits.iter())
}

/// §bodies: the iterator form — deposits live on bodies now, so callers sum
/// over `StarSystem::all_deposits()`.
pub fn band_value_iter<'a>(deposits: impl Iterator<Item = &'a Deposit>) -> f64 {
    deposits
        .map(|d| d.richness * crate::market::base_price(d.resource))
        .sum()
}

/// Bucket a system value against the stored tercile thresholds `(lo, hi)`.
pub fn band_for(value: f64, lo: f64, hi: f64) -> RichnessBand {
    if value <= lo {
        RichnessBand::Poor
    } else if value <= hi {
        RichnessBand::Fair
    } else {
        RichnessBand::Rich
    }
}

/// The tercile thresholds `(lo, hi)` of `band_value` across `systems` — the
/// bottom third are Poor, the middle third Fair, the top third Rich. Sorted by
/// total order (values are finite sums of finite products); deterministic for a
/// given galaxy. Empty/small galaxies degrade gracefully (everything Rich).
pub fn band_thresholds(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let mut v: Vec<f64> = values.collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if v.is_empty() {
        return (0.0, 0.0);
    }
    let n = v.len();
    let lo = v[(n - 1) / 3];
    let hi = v[(2 * (n - 1)) / 3];
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::Commodity;

    fn dep(resource: Commodity, richness: f64) -> Deposit {
        Deposit {
            resource,
            richness,
            reserves: None,
            accessibility: 0.5,
        }
    }

    /// band_value mirrors the bootstrap anchors (Σ richness × base_price).
    #[test]
    fn band_value_uses_the_bootstrap_anchors() {
        let deps = vec![
            dep(Commodity::MetallicOre, 2.0),
            dep(Commodity::Alloys, 1.0),
        ];
        assert_eq!(band_value(&deps), 2.0 * 8.0 + 26.0);
        assert_eq!(band_value(&[]), 0.0);
    }

    /// Terciles split a spread of values into thirds; banding follows them.
    #[test]
    fn terciles_split_into_thirds() {
        // Nine values 1..=9 → lo = v[2] = 3, hi = v[5] = 6.
        let (lo, hi) = band_thresholds((1..=9).map(|x| x as f64));
        assert_eq!((lo, hi), (3.0, 6.0));
        assert_eq!(band_for(2.0, lo, hi), RichnessBand::Poor);
        assert_eq!(band_for(3.0, lo, hi), RichnessBand::Poor); // inclusive lo
        assert_eq!(band_for(5.0, lo, hi), RichnessBand::Fair);
        assert_eq!(band_for(9.0, lo, hi), RichnessBand::Rich);
    }
}

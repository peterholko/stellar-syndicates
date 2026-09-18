//! Static galaxy geography: the hub, procedurally-placed star systems, and the
//! ring of home anchors (§4). Generated deterministically from the seed.
//!
//! "No discrete zones": systems are scattered continuously across one radial
//! space, the hub fixed at the centre, homes distributed around a ring as
//! bright spots. Resources/claims hang off systems in later milestones.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};
use crate::market::base_price;
use crate::math::Vec2;
use crate::rng::Rng;

/// A single extractable resource concentration on a star system (adapted from
/// Stellar Charters' "deposits on bodies", simplified to hang directly off the
/// system — no planet/body hierarchy yet). A claimed system's deposits produce
/// their `resource` continuously into the system's stockpile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deposit {
    /// A commodity that already trades on the hub Exchange.
    pub resource: Commodity,
    /// Units produced per second at full extraction.
    pub richness: f64,
    /// Remaining reserves; `None` = renewable (never depletes). Finite deposits
    /// run dry: §ore-ladder rolls the two scarce ores finite (`roll_reserves`)
    /// so a rim claim is a rush, not a permanent faucet; bulk ores renew.
    pub reserves: Option<f64>,
    /// 0..1 difficulty (deeper = harder). A field for later extractor-tier
    /// gating; it does NOT gate anything yet.
    pub accessibility: f64,
}

/// A procedurally-placed star system. `pos`, `name`, `deposits`, and `claim_cost`
/// are static geography (known to all). `owner`/`claimed_at`/`stockpile` are
/// *dynamic* state: a claim is an event at `pos`/`claimed_at`, so its reveal to
/// rivals must respect light delay (enforced by the server's view filter), and a
/// player's accumulated production is private to them.
/// serde default for `StarSystem::garrison_fed` (old snapshots load fed).
fn default_true_sys() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StarSystem {
    pub id: EntityId,
    pub pos: Vec2,
    pub name: String,
    /// §bodies: the system's PLANETS AND MOONS — first-class sim entities.
    /// Deposits, structures, population, and assignments live ON bodies now;
    /// every system-level number is a derived sum. Empty on a pre-bodies
    /// snapshot until `migrate_to_bodies` folds the legacy fields in.
    #[serde(default)]
    pub bodies: Vec<crate::body::Body>,
    /// DEPRECATED §bodies: the legacy SYSTEM-scoped deposit list — a parse-only
    /// shell consumed by `migrate_to_bodies` (deposits belong to bodies now).
    #[serde(default, rename = "deposits")]
    pub legacy_deposits: Vec<Deposit>,
    /// DEPRECATED (§ships part 3): the old instant-claim credit cost. Claiming
    /// is now PHYSICAL (a Colony Ship's recipe absorbs the economics), so this
    /// charges nothing and gates nothing. Kept on the struct/wire for snapshot
    /// compatibility and as a ready-made "system value" scalar (a future colony
    /// overhead / valuation knob). Still generated per-system.
    #[serde(default)]
    pub claim_cost: f64,
    /// Owning corporation, once claimed (light-gated to rivals by the view filter).
    #[serde(default)]
    pub owner: Option<PlayerId>,
    /// Sim time at which the system was claimed (None while unowned) — the event
    /// time whose light gates the reveal of `owner` to other players.
    #[serde(default)]
    pub claimed_at: Option<f64>,
    /// Production accumulated at the system, awaiting a convoy to the hub.
    #[serde(default)]
    pub stockpile: BTreeMap<Commodity, f64>,
    #[serde(default)]
    pub industry: crate::industry::SiteIndustry,
    /// §modules Part B3: the MODULE ledger — manufactured modules pooled at this
    /// system (crates in the yard's warehouse, not per-body). Fitted onto ships
    /// at build/refit, shipped by convoy, traded at Sol. `#[serde(default)]` so
    /// pre-module snapshots load with an empty ledger.
    #[serde(default)]
    pub modules: BTreeMap<crate::module::ModuleKind, u32>,
    /// Number of Extractor upgrades built here (§step1 structure sink). Scales
    /// every deposit's richness by `EXTRACTOR_RICHNESS_MULT^tier` in accrual.
    #[serde(default, rename = "extractor_tier")]
    pub legacy_extractor_tier: u32,
    /// Number of Orbital Warehouse tiers built here (§buildings step 2). Each tier raises
    /// the system's storage cap by `STORAGE_PER_ORBITAL_WAREHOUSE_TIER`. `default` = 0 on old
    /// snapshots (migration grants Warehouse I; oversize stockpiles are grandfathered —
    /// the cap blocks NEW inflow only, it never destroys what's stored).
    #[serde(default, rename = "depot_tier")]
    pub legacy_depot_tier: u32,
    /// Number of Shipyard upgrades built here (§buildings step 3). Gates ship
    /// construction: Tiny/Small Freighter need tier ≥ 1, Medium/Interceptor ≥ 2
    /// (`required_shipyard_tier`).
    /// HOME systems generate at tier 1 (the turn-one convoy bootstrap).
    #[serde(default, rename = "shipyard_tier")]
    pub legacy_shipyard_tier: u32,
    /// Number of Sensor Array upgrades built here (§buildings step 2b). An owned
    /// system with tier ≥ 1 projects a standing sensor bubble for its OWNER
    /// (radius `sensor_array_radius(tier)`), feeding the same coverage model as
    /// ship bubbles. Owner-only in the View, like every tier.
    #[serde(default, rename = "sensor_tier")]
    pub legacy_sensor_tier: u32,
    /// Number of Defense Platform tiers standing here (§buildings step 2c). A
    /// hostile raider making contact with one of the owner's convoys within
    /// `DEFENSE_PLATFORM_RADIUS` must fight through `tier` stationary defender
    /// units first (seeded battles). Tiers can be LOST in those engagements
    /// (damage), so this can go down as well as up; the system itself is never
    /// destroyed. Owner-only in the View — a rival learns a platform exists only
    /// through engagement outcomes (delayed battle reports).
    #[serde(default, rename = "defense_tier")]
    pub legacy_defense_tier: u32,
    /// The platform's accumulated DAMAGE POOL (§FLEETS Part 2 Lanchester): a tier
    /// dies when this fills a `PLATFORM_TIER_HULL`, carrying the remainder. serde
    /// default keeps pre-Lanchester snapshots loading (a fresh, undamaged pool).
    #[serde(default)]
    pub defense_pool: f64,
    /// Number of Habitat tiers here (§buildings step 3a). When FED, boosts the
    /// system's total output ×`HABITAT_OUTPUT_MULT^tier`; consumes
    /// `HABITAT_UPKEEP_PER_TIER`/s of Provisions from this stockpile. Owner-only.
    #[serde(default, rename = "habitat_tier")]
    pub legacy_habitat_tier: u32,
    /// §economy Part 2: the colony's FOOD STATE on the 4-rung ladder (replaces
    /// the old binary `habitat_fed`). Recomputed every tick for owned systems
    /// from stock coverage vs population demand; hunger only SUSPENDS
    /// (efficiency drops, immigration pauses) — nothing is destroyed, nobody dies
    /// (async-fair). Owner-only in the View. `default` WellSupplied is right
    /// for old snapshots (population defaults 0 = no demand) and corrected on
    /// the first tick regardless; the old `habitat_fed` key is simply ignored.
    #[serde(default)]
    pub food_state: crate::colony::FoodState,
    /// Number of Fuel Refinery tiers here (§buildings step 3b). Converts
    /// stockpiled Volatiles → Fuel at `REFINERY_RATE_PER_TIER · tier`/s
    /// (`REFINERY_YIELD` Fuel per Volatile); idles dry. Owner-only in the View.
    #[serde(default, rename = "refinery_tier")]
    pub legacy_refinery_tier: u32,
    /// BLOCKADE state (§contestable-territory Part 1): `Some` while ≥1 hostile
    /// fleet holds station here. Recomputed every tick by `resolve_blockades`
    /// from on-station fleet presence — persisted so a mid-blockade snapshot
    /// keeps the (unbroken) `since` / siege clock. `default` None (no blockade).
    #[serde(default)]
    pub blockade: Option<Blockade>,
    /// §TCA: the PREVIOUS blockade window `(since, lifted_at)` — a two-state
    /// history so a DISTANT observer can answer "was this system blockaded at
    /// retarded time T?" across the light-delay window. The Market Hub uses it
    /// to decide whether to accept freight bookings on its own (light-delayed)
    /// knowledge: it keeps refusing until the LIFT's light reaches the hub, and
    /// keeps accepting until the ONSET's light does. Mirrors the two-state
    /// `Corporation::syndicate_prev` pattern. `default` None (never blockaded).
    #[serde(default)]
    pub blockade_prev: Option<(f64, f64)>,
    /// §explore Part 3: the system's surveyable economic trait. Effects are
    /// always-on ground truth. Seeded at generation
    /// (`TRAIT_FRACTION` of systems, an isolated stream). `default` None — a
    /// pre-feature galaxy simply has none (acceptable; new generations do).
    #[serde(default)]
    pub trait_: Option<crate::explore::SystemTrait>,
    /// §ground: is the dug-in GARRISON currently fed? Recomputed every tick from
    /// this system's own Provisions (like `food_state`). An UNFED garrison
    /// suspends — it stretches no siege clock and resists no landing — but no
    /// tier is ever lost and it recovers the tick supply returns. Owner-only in
    /// the View. `default` true so pre-garrison snapshots load untroubled.
    #[serde(default = "default_true_sys")]
    pub garrison_fed: bool,
    /// §ground M6: BOMBARDMENT SUPPRESSION, 0..1 — the fraction of the garrison
    /// currently pinned down by orbital fire. Decays back to 0 when the guns
    /// stop, so it is a window a besieger must exploit, never a permanent loss:
    /// no tier is destroyed and no population is touched. `default` 0.
    #[serde(default)]
    pub garrison_suppression: f64,
    /// §explore Part 3: the Precursor Cache has PAID (latched — exactly once,
    /// ever; deliberately NOT reset on capture, so a flip can't re-mint it).
    #[serde(default)]
    pub cache_claimed: bool,
    /// DEPRECATED §bodies: the legacy SYSTEM-scoped structure map — a parse-only
    /// shell (the flat tier fields above fold into it, then `migrate_to_bodies`
    /// sites everything onto bodies and zeroes it).
    #[serde(default, rename = "structures")]
    pub legacy_structures: BTreeMap<crate::build::StructureKind, u32>,
    /// DEPRECATED §bodies: the legacy SYSTEM-scoped population — a parse-only
    /// shell folded onto the Habitat's body by `migrate_to_bodies`.
    #[serde(default, rename = "population")]
    pub legacy_population: f64,
    /// DEPRECATED §bodies: the legacy SYSTEM-scoped assignments — a parse-only
    /// shell re-homed onto their structures' bodies by `migrate_to_bodies`.
    #[serde(default, rename = "assignments")]
    pub legacy_assignments: BTreeMap<crate::build::StructureKind, crate::production::Assignment>,
    /// §economy Part 4: the RESIDENT SPECIALIST POOL (kind → headcount) —
    /// hired from Sol, trained at an Academy, delivered by convoy. Posted to
    /// lines via assignments; conquest KEEPS them with the system (people
    /// outlast the flag). Owner-only in the View.
    #[serde(default)]
    pub specialists: BTreeMap<crate::specialist::SpecialistKind, u32>,
}

/// The live BLOCKADE at a system (§contestable-territory). Recomputed each tick
/// from fleet presence; persisted so an unbroken blockade's clocks survive a
/// snapshot. `siege_since` is populated in Part 2 (siege→capture).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Blockade {
    /// The blockading corporation (the badge / capture attribution; there may be
    /// several on-station fleets — this is the earliest-arrived owner).
    pub by: PlayerId,
    /// Sim-time the current UNBROKEN blockade began (any full lift resets it).
    pub since: f64,
    /// Sim-time the SIEGE conditions (defenses suppressed + no garrison, under an
    /// unbroken blockade) first held — the capture clock's start. `None` until
    /// the siege can progress; reset whenever a condition breaks (§Part 2).
    #[serde(default)]
    pub siege_since: Option<f64>,
}

impl StarSystem {
    /// Physical goods remain in storage. Earmarks protect them from exports,
    /// factory inputs and competing projects. Population life support and
    /// hostile plunder still access physical stock, rather than this budget.
    pub fn free_stock(&self, c: Commodity) -> f64 {
        self.project_stock(c, None)
    }
    pub fn project_stock(&self, c: Commodity, target: Option<crate::industry::ProjectTarget>) -> f64 {
        (self.stockpile.get(&c).copied().unwrap_or(0.0) - self.industry.reserved(c, target)).max(0.0)
    }
    /// Whether this system can be claimed (no current owner).
    pub fn is_unclaimed(&self) -> bool {
        self.owner.is_none()
    }

    // --- §bodies: DERIVED system reads (bodies are the store; every
    // system-level number the player sees is a sum or max over them). --------

    /// Every deposit in the system, walking bodies in roster order.
    pub fn all_deposits(&self) -> impl Iterator<Item = &Deposit> {
        self.bodies.iter().flat_map(|b| b.deposits.iter())
    }

    /// Total system POPULATION — the sum of every body's (millions).
    pub fn population(&self) -> f64 {
        self.bodies.iter().map(|b| b.population).sum()
    }

    /// The BEST tier of a structure kind anywhere in the system — the read for
    /// capability GATES (a ship builds at the best yard; the sensor bubble is
    /// the best array's; an Academy anywhere teaches).
    pub fn tier(&self, kind: crate::build::StructureKind) -> u32 {
        self.bodies.iter().map(|b| b.tier(kind)).max().unwrap_or(0)
    }

    /// The SUMMED tiers of a kind across bodies — the read for quantities that
    /// STACK (Orbital Warehouse storage capacity, Defense Platform strength).
    pub fn tier_sum(&self, kind: crate::build::StructureKind) -> u32 {
        self.bodies.iter().map(|b| b.tier(kind)).sum()
    }

    /// §bodies: write a kind's tier. Reads went per-body; the writers that
    /// remain system-scoped (combat platform losses, pirate base seeding,
    /// capture halving fallbacks, tests) target the body that HOLDS the kind
    /// (highest tier first), else the kind's natural SITE, else the primary.
    /// The write is total: it sets the SYSTEM total to `tier` by zeroing other
    /// holders — matching the old single-store semantics those callers assume.
    pub fn set_tier(&mut self, kind: crate::build::StructureKind, tier: u32) {
        if self.bodies.is_empty() {
            // Pre-migration shell (or a bare test fixture): keep the legacy map
            // coherent so `migrate_to_bodies` sites it later.
            if tier == 0 {
                self.legacy_structures.remove(&kind);
            } else {
                self.legacy_structures.insert(kind, tier);
            }
            return;
        }
        let target = self
            .bodies
            .iter()
            .filter(|b| b.tier(kind) > 0)
            .max_by_key(|b| b.tier(kind))
            .map(|b| b.id)
            .or_else(|| self.site_for(kind))
            .unwrap_or(self.bodies[0].id);
        for b in self.bodies.iter_mut() {
            if b.id == target {
                b.set_tier(kind, tier);
            } else if b.tier(kind) > 0 {
                b.set_tier(kind, 0);
            }
        }
    }

    /// §bodies: the natural SITE for a structure kind — the Part-5 siting
    /// rules, shared by migration, the home bootstrap, and system-scoped
    /// writers. Returns a body id (None only for an empty roster).
    pub fn site_for(&self, kind: crate::build::StructureKind) -> Option<u32> {
        use crate::build::StructureKind as K;
        if self.bodies.is_empty() {
            return None;
        }
        let planets: Vec<&crate::body::Body> =
            self.bodies.iter().filter(|b| b.parent.is_none()).collect();
        let primary = planets.first().map(|b| b.id).or(Some(self.bodies[0].id));
        let richest_matching = |k: K| {
            self.bodies
                .iter()
                .flat_map(|b| b.deposits.iter().map(move |d| (b, d)))
                .filter(|(_, d)| crate::production::extraction_structure(d.resource) == Some(k))
                .max_by(|a, b| a.1.richness.partial_cmp(&b.1.richness).expect("finite"))
                .map(|(b, _)| b.id)
        };
        let volatiles_body = self
            .bodies
            .iter()
            .find(|b| {
                b.deposits
                    .iter()
                    .any(|d| d.resource == crate::cargo::Commodity::Volatiles)
            })
            .map(|b| b.id)
            .or_else(|| {
                planets
                    .iter()
                    .find(|b| b.kind == crate::body::BodyKind::GasGiant)
                    .map(|b| b.id)
            })
            .or(primary);
        let habitable_body = self
            .bodies
            .iter()
            .find(|b| b.habitable)
            .map(|b| b.id)
            .or_else(|| {
                planets
                    .iter()
                    .find(|b| {
                        matches!(
                            b.kind,
                            crate::body::BodyKind::Terrestrial | crate::body::BodyKind::Ocean
                        )
                    })
                    .map(|b| b.id)
            })
            .or(primary);
        let industrial_body = planets
            .iter()
            .find(|b| {
                !b.habitable
                    && !matches!(
                        b.kind,
                        crate::body::BodyKind::GasGiant | crate::body::BodyKind::Ice
                    )
            })
            .map(|b| b.id)
            .or(primary);
        let outermost = planets.last().map(|b| b.id).or(primary);
        match kind {
            K::MiningComplex | K::VolatileHarvester | K::Bioharvester => {
                richest_matching(kind).or(match kind {
                    K::VolatileHarvester => volatiles_body,
                    K::Bioharvester => habitable_body,
                    _ => industrial_body,
                })
            }
            K::FuelRefinery | K::ChemicalWorks => volatiles_body,
            // §ground: the garrison sits where the people are — you defend a
            // populated world, not a bare rock.
            K::Habitat | K::Agroplex | K::Academy | K::Garrison => habitable_body,
            K::Warehouse => {
                if let Some(b) = self.bodies.iter().filter(|b| b.tier(K::Warehouse) > 0)
                    .max_by_key(|b| b.tier(K::Warehouse)) {
                    return Some(b.id); // unspecified tier-ups stay at the existing store
                }
                // Ground storage needs a surface and should not overfill the
                // founding food world's Infrastructure pool. Existing full
                // colonies may be grandfathered during the one-time migration.
                let ground: Vec<_> = self.bodies.iter().filter(|b|
                    b.kind != crate::body::BodyKind::GasGiant).collect();
                ground.iter().find(|b| b.pool_slots_built(crate::build::SlotPool::Infrastructure)
                    < b.pool_slots(crate::build::SlotPool::Infrastructure))
                    .or_else(|| ground.first()).map(|b| b.id).or(primary)
            }
            K::Smelter
            | K::ElectronicsFabricator
            | K::MachineWorks
            | K::ArmamentsComplex
            | K::CompositeWorks
            | K::HullFabricator
            | K::PrecisionWorks
            | K::DriveWorks => {
                industrial_body
            }
            // §yards: the whole yard family auto-sites with the Shipyard, on the
            // system's primary body — a shipbuilding world builds its ladder in
            // one place, and the drydock's crews are the shipyard's neighbours.
            K::Shipyard
            | K::NavalDrydock
            | K::CapitalSlipway
            | K::OrdnanceFoundry
            | K::OrbitalWarehouse
            | K::DefensePlatform => primary,
            K::SensorArray => outermost,
        }
    }

    /// §bodies: seed `millions` of population onto the system's natural
    /// habitable body (colony landings, home bootstraps, save migration, tests).
    pub fn seed_population(&mut self, millions: f64) {
        let Some(target) = self.site_for(crate::build::StructureKind::Habitat) else {
            self.legacy_population += millions; // pre-migration shell
            return;
        };
        if let Some(b) = self.bodies.iter_mut().find(|b| b.id == target) {
            b.population += millions;
        }
    }

    /// §bodies: REPLACE this system's geology for tests/tools — regenerates
    /// the roster from `deposits` (the shared generator), then RE-SITES any
    /// structures/assignments already present and re-seeds the population
    /// (structures placed before a geology change keep working).
    pub fn set_test_deposits(&mut self, deposits: Vec<Deposit>) {
        let mut structures: BTreeMap<crate::build::StructureKind, u32> = BTreeMap::new();
        let mut assignments: BTreeMap<crate::build::StructureKind, crate::production::Assignment> =
            BTreeMap::new();
        let mut pop = 0.0;
        for b in &self.bodies {
            for (k, t) in &b.structures {
                *structures.entry(*k).or_insert(0) += t;
            }
            for (k, a) in &b.assignments {
                assignments.insert(*k, a.clone());
            }
            pop += b.population;
        }
        self.bodies = crate::body::generate_bodies(&self.id.0.to_string(), &self.name, &deposits);
        for (kind, tier) in structures {
            let target = self.site_for(kind).unwrap_or(0);
            if let Some(b) = self.bodies.iter_mut().find(|b| b.id == target) {
                b.set_tier(kind, tier);
            }
        }
        for (kind, asg) in assignments {
            if let Some(b) = self.bodies.iter_mut().find(|b| b.tier(kind) > 0) {
                b.assignments.insert(kind, asg);
            }
        }
        if pop > 0.0 {
            self.seed_population(pop);
        }
    }

    /// §bodies: append ONE deposit onto an affinity-matching body (tests /
    /// tools). Falls back to the first body; a bodiless shell keeps it legacy.
    pub fn add_test_deposit(&mut self, d: Deposit) {
        if self.bodies.is_empty() {
            self.legacy_deposits.push(d);
            return;
        }
        let kind = crate::production::extraction_structure(d.resource);
        let idx = self
            .bodies
            .iter()
            .position(|b| match kind {
                Some(crate::build::StructureKind::Bioharvester) => b.habitable,
                Some(crate::build::StructureKind::VolatileHarvester) => matches!(
                    b.kind,
                    crate::body::BodyKind::Ice | crate::body::BodyKind::GasGiant
                ),
                _ => matches!(
                    b.kind,
                    crate::body::BodyKind::Rocky | crate::body::BodyKind::Terrestrial
                ),
            })
            .unwrap_or(0);
        self.bodies[idx].deposits.push(d);
    }

    /// §bodies: zero every body's population, then seed `millions` on the
    /// natural habitable body (test/tool shorthand for "the colony IS this big").
    pub fn set_population(&mut self, millions: f64) {
        for b in self.bodies.iter_mut() {
            b.population = 0.0;
        }
        self.seed_population(millions);
    }

    /// §bodies: post an assignment on the body HOLDING `kind` (highest tier
    /// first) — the pre-bodies call shape, for tests and simple tools.
    pub fn assign(
        &mut self,
        kind: crate::build::StructureKind,
        asg: crate::production::Assignment,
    ) {
        if let Some(b) = self
            .bodies
            .iter_mut()
            .filter(|b| b.tier(kind) > 0)
            .max_by_key(|b| b.tier(kind))
        {
            b.assignments.insert(kind, asg);
        }
    }

    /// §bodies: the assignment on the body holding `kind`, if any.
    pub fn assignment(
        &self,
        kind: crate::build::StructureKind,
    ) -> Option<&crate::production::Assignment> {
        self.bodies
            .iter()
            .filter(|b| b.tier(kind) > 0)
            .max_by_key(|b| b.tier(kind))
            .and_then(|b| b.assignments.get(&kind))
            .or_else(|| self.bodies.iter().find_map(|b| b.assignments.get(&kind)))
    }

    /// §economy: fold the LEGACY flat tier fields into the legacy structure
    /// map (Extractor → MiningComplex, Refinery → FuelRefinery, the rest 1:1),
    /// zeroing the carriers. Idempotent; `migrate_to_bodies` then sites the
    /// map onto bodies. `defense_pool` and combat semantics ride along.
    pub fn fold_legacy_structures(&mut self) {
        use crate::build::StructureKind as K;
        let folds = [
            (
                std::mem::take(&mut self.legacy_extractor_tier),
                K::MiningComplex,
            ),
            (
                std::mem::take(&mut self.legacy_depot_tier),
                K::OrbitalWarehouse,
            ),
            (std::mem::take(&mut self.legacy_shipyard_tier), K::Shipyard),
            (std::mem::take(&mut self.legacy_sensor_tier), K::SensorArray),
            (
                std::mem::take(&mut self.legacy_defense_tier),
                K::DefensePlatform,
            ),
            (std::mem::take(&mut self.legacy_habitat_tier), K::Habitat),
            (
                std::mem::take(&mut self.legacy_refinery_tier),
                K::FuelRefinery,
            ),
        ];
        for (legacy, kind) in folds {
            if legacy > 0 {
                let cur = self.legacy_structures.get(&kind).copied().unwrap_or(0);
                self.legacy_structures.insert(kind, cur + legacy);
            }
        }
    }

    /// §bodies: MIGRATE this system onto its body roster — idempotent (a
    /// system with bodies passes through untouched). Generates the ported
    /// roster from the legacy deposit list (layout-preserving), distributes
    /// the deposits, sites the legacy structures, seeds the population onto
    /// the Habitat's body, and re-homes assignments with their structures.
    pub fn migrate_to_bodies(&mut self) {
        if self.bodies.is_empty() {
            let deposits = std::mem::take(&mut self.legacy_deposits);
            self.bodies =
                crate::body::generate_bodies(&self.id.0.to_string(), &self.name, &deposits);
        } else if !self.legacy_deposits.is_empty() {
            // A mixed-era state: bodies exist but a legacy deposit list is
            // still riding along — distribute it by affinity (nothing lost).
            for d in std::mem::take(&mut self.legacy_deposits) {
                self.add_test_deposit(d);
            }
        }
        // §planetary-identity: pre-feature body rosters carried no independent
        // size/environment/geology. Fill those profiles deterministically from
        // stable ids; this never moves or rerolls any owned economic state.
        let sid = self.id.0.to_string();
        for body in &mut self.bodies {
            body.ensure_profile(&sid);
        }
        // Site every legacy structure per the shared rules.
        let structures = std::mem::take(&mut self.legacy_structures);
        for (kind, tier) in structures {
            if tier == 0 {
                continue;
            }
            let target = self.site_for(kind).unwrap_or(0);
            if let Some(b) = self.bodies.iter_mut().find(|b| b.id == target) {
                let cur = b.tier(kind);
                b.set_tier(kind, cur + tier);
            }
        }
        // Population lands on the Habitat's body.
        let pop = std::mem::take(&mut self.legacy_population);
        if pop > 0.0 {
            self.seed_population(pop);
        }
        // Assignments re-home with their structures.
        let assignments = std::mem::take(&mut self.legacy_assignments);
        for (kind, asg) in assignments {
            if let Some(b) = self.bodies.iter_mut().find(|b| b.tier(kind) > 0) {
                b.assignments.insert(kind, asg);
            }
        }
    }

    /// Seed one legible, population-led expansion candidate. New galaxies vary
    /// the proposition deterministically: a huge Gaia garden, a fertile Terran
    /// exporter, or a fertile Gaia hybrid. Deposits and geology remain whatever
    /// they rolled, so the same onboarding guarantee does not prescribe the same
    /// production chain in every opening.
    pub fn guarantee_garden_world(&mut self) {
        let Some(body) = self
            .bodies
            .iter_mut()
            .filter(|b| b.parent.is_none() && b.kind != crate::body::BodyKind::GasGiant)
            .max_by_key(|b| b.profile.size)
        else {
            return;
        };
        let has_biomass = body
            .deposits
            .iter()
            .any(|d| d.resource == Commodity::Biomass);
        match self.id.0 % 3 {
            0 => {
                body.profile.size = crate::body::BodySize::Huge;
                body.profile.environment = crate::body::Environment::Gaia;
            }
            1 => {
                body.profile.size = crate::body::BodySize::Huge;
                if has_biomass {
                    body.profile.environment = crate::body::Environment::Terran;
                    body.profile.special = Some(crate::body::BodySpecial::FertileBiosphere);
                } else {
                    // Preserve the population guarantee without inventing the
                    // Biomass deposit a fertile exporter would require.
                    body.profile.environment = crate::body::Environment::Gaia;
                }
            }
            _ => {
                if has_biomass {
                    body.profile.size = body.profile.size.max(crate::body::BodySize::Large);
                    body.profile.environment = crate::body::Environment::Gaia;
                    body.profile.special = Some(crate::body::BodySpecial::FertileBiosphere);
                } else {
                    body.profile.size = crate::body::BodySize::Huge;
                    body.profile.environment = crate::body::Environment::Gaia;
                }
            }
        }
        body.habitable = true;
    }

    /// Seed the contrasting nearby opportunity: a hostile, Ultra Rich mineral
    /// body. It still needs the system's actual mineral deposit, construction,
    /// population and supply chain — this guarantees a reason to expand, not a
    /// free functioning colony.
    pub fn guarantee_industrial_world(&mut self) {
        let mineral = |d: &&Deposit| {
            d.resource.is_mineable_mineral()
        };
        let target = self
            .bodies
            .iter()
            .position(|b| b.deposits.iter().filter(mineral).next().is_some())
            .or_else(|| {
                self.bodies
                    .iter()
                    .position(|b| b.kind != crate::body::BodyKind::GasGiant)
            });
        let Some(body) = target.and_then(|i| self.bodies.get_mut(i)) else {
            return;
        };
        // Two equal-value headaches: a compact hostile mine, or a larger airless
        // industrial shelf. Deposit type and any rolled special remain seeded.
        if self.id.0.is_multiple_of(2) {
            body.profile.size = body.profile.size.max(crate::body::BodySize::Medium);
            body.profile.environment = crate::body::Environment::Hostile;
        } else {
            body.profile.size = body.profile.size.max(crate::body::BodySize::Large);
            body.profile.environment = crate::body::Environment::Uninhabitable;
        }
        body.profile.geology = crate::body::Geology::UltraRich;
        body.habitable = false;
    }

    /// §bodies: the SUMMED slot pools across bodies (the system panel's
    /// "industrial 4/7 across 5 bodies" readout; gating is per body).
    pub fn resource_slots(&self) -> u32 {
        self.bodies.iter().map(|b| b.resource_slots()).sum()
    }

    pub fn industrial_slots(&self) -> u32 {
        self.bodies.iter().map(|b| b.industrial_slots()).sum()
    }

    pub fn infrastructure_slots(&self) -> u32 {
        self.bodies.iter().map(|b| b.infrastructure_slots()).sum()
    }

    /// The summed slot budget of one pool.
    pub fn pool_slots(&self, pool: crate::build::SlotPool) -> u32 {
        self.bodies.iter().map(|b| b.pool_slots(pool)).sum()
    }

    /// Summed slots of one pool consumed across bodies (breadth per body).
    pub fn pool_slots_built(&self, pool: crate::build::SlotPool) -> u32 {
        self.bodies.iter().map(|b| b.pool_slots_built(pool)).sum()
    }

    /// §economy Part 3: total workforce crews POSTED across every body's
    /// assignments (labor is ONE system pool — it commutes inside the well).
    pub fn workforce_posted(&self) -> u32 {
        self.bodies
            .iter()
            .flat_map(|b| b.assignments.values())
            .map(|a| a.workers)
            .sum::<u32>() + self.industry.workforce()
    }

    /// §economy Part 3: the SYSTEM-wide staffing share — the one workforce
    /// pool (Σ body populations) diluted across every posted crew on every
    /// body, uniformly (fair, legible, deadlock-free).
    pub fn staffing_share(&self) -> f64 {
        let posted = self.workforce_posted();
        if posted == 0 {
            return 1.0;
        }
        (crate::colony::workforce_units(self.population()) as f64 / posted as f64).min(1.0)
    }

    /// §economy Part 4: the EFFECTIVE specialists on every line this tick —
    /// keyed `(body id, structure)` now; the resident pool stays SYSTEM-scoped
    /// and is walked in deterministic (body id, kind) order, non-destructively.
    pub fn effective_specialists(
        &self,
    ) -> BTreeMap<(u32, crate::build::StructureKind), (u32, u32)> {
        let mut pool_left = self.specialists.clone();
        let mut out = BTreeMap::new();
        for b in &self.bodies {
            for (kind, asg) in &b.assignments {
                let (mut crew, mut matched) = (0u32, 0u32);
                for (&sk, &n) in &asg.specialists {
                    let left = pool_left.entry(sk).or_insert(0);
                    let take = n.min(*left);
                    *left -= take;
                    crew += take;
                    if sk.affine(*kind) {
                        matched += take;
                    }
                }
                out.insert((b.id, *kind), (crew, matched));
            }
        }
        out
    }

    /// §economy Part 3: the STAFFING factor of one BODY's line —
    /// `(crew/tier) · share` (crew = workers + posted specialists).
    pub fn staffing_factor(&self, body_id: u32, kind: crate::build::StructureKind) -> f64 {
        let Some(b) = self.bodies.iter().find(|b| b.id == body_id) else {
            return 0.0;
        };
        let tier = b.tier(kind);
        if tier == 0 {
            return 0.0;
        }
        let workers = b.assignments.get(&kind).map(|a| a.workers).unwrap_or(0);
        let spec_crew = self
            .effective_specialists()
            .get(&(body_id, kind))
            .map(|(c, _)| *c)
            .unwrap_or(0);
        ((workers + spec_crew).min(tier) as f64 / tier as f64) * self.staffing_share()
    }

    /// §economy Part 4: the SKILL factor of one BODY's line.
    pub fn skill_factor(&self, body_id: u32, kind: crate::build::StructureKind) -> f64 {
        let tier = self
            .bodies
            .iter()
            .find(|b| b.id == body_id)
            .map(|b| b.tier(kind))
            .unwrap_or(0);
        let (_, matched) = self
            .effective_specialists()
            .get(&(body_id, kind))
            .copied()
            .unwrap_or((0, 0));
        crate::production::skill_factor(matched, tier)
    }

    /// LEGACY single-budget readouts, now sums over the three pools — keeps the
    /// existing wire fields (`slots_used`/`slots_total`) meaningful until the
    /// per-pool client panel lands (Part 6/7).
    pub fn dev_slots(&self) -> u32 {
        self.resource_slots() + self.industrial_slots() + self.infrastructure_slots()
    }

    /// Development slots already CONSUMED here (all pools) — one per distinct
    /// built structure (see `pool_slots_built`).
    pub fn dev_slots_built(&self) -> u32 {
        self.bodies
            .iter()
            .map(|b| b.structures.values().filter(|t| **t >= 1).count() as u32)
            .sum()
    }

    /// §ground: the garrison strength actually standing right now — its tier,
    /// zeroed if unfed, and reduced by whatever bombardment has pinned down.
    /// THE one derivation: the siege clock and any landing both read this, so a
    /// besieger's guns and a defender's larder move the same number.
    pub fn effective_garrison(&self) -> f64 {
        if !self.garrison_fed {
            return 0.0;
        }
        let tier = self.tier_sum(crate::build::StructureKind::Garrison) as f64;
        (tier * (1.0 - self.garrison_suppression.clamp(0.0, 1.0))).max(0.0)
    }

    /// The sensor bubble this system projects FOR ITS OWNER (0 without an array).
    pub fn sensor_bubble(&self) -> f64 {
        crate::build::sensor_array_radius(self.tier(crate::build::StructureKind::SensorArray))
    }

    /// Founding supplies include one ground Warehouse I, not an invisible base
    /// allowance. Buildings on every body share this single system stockpile.
    /// Losing capacity blocks new inflow; it never deletes stored goods.
    pub fn storage_cap(&self) -> f64 {
        let ground: f64 = if self.bodies.is_empty() {
            crate::build::warehouse_capacity(self.tier(crate::build::StructureKind::Warehouse))
        } else {
            self.bodies.iter().map(|b| crate::build::warehouse_capacity(
                b.tier(crate::build::StructureKind::Warehouse))).sum()
        };
        ground + crate::build::STORAGE_PER_ORBITAL_WAREHOUSE_TIER
                * self.tier_sum(crate::build::StructureKind::OrbitalWarehouse) as f64
    }

    /// Founding kit / old-save migration only, never a per-tick repair. A lost
    /// warehouse is not silently rebuilt. Keep any existing tiers and inventory.
    pub(crate) fn seed_warehouse(&mut self) {
        if self.tier(crate::build::StructureKind::Warehouse) == 0 {
            self.set_tier(crate::build::StructureKind::Warehouse, 1);
        }
    }

    /// Total units currently stored (summed across commodities) — what the cap
    /// measures against.
    pub fn storage_used(&self) -> f64 {
        self.stockpile.values().sum()
    }

    /// Remaining storage headroom (0 when at/over cap — e.g. a grandfathered
    /// oversize stockpile from before caps existed).
    pub fn storage_headroom(&self) -> f64 {
        (self.storage_cap() - self.storage_used()).max(0.0)
    }
}

/// One of the pre-generated home-anchor slots arranged around a ring. Assigned
/// to a player on join; a player commands from their home anchor (§6).
///
/// `pos` is static geography (known to all). `owner`/`claimed_at` are *dynamic*
/// state: a claim is an event at `pos` and time `claimed_at`, so its reveal to
/// other players must respect light delay (enforced by the view filter), or it
/// would leak a rival's presence faster than light.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeSlot {
    pub pos: Vec2,
    pub owner: Option<PlayerId>,
    /// Sim time at which this slot was claimed (None while unowned).
    pub claimed_at: Option<f64>,
    /// The developed HOME STAR SYSTEM co-located at this slot, granted to the
    /// player who takes the slot (Travian/OGame convention: you begin owning a
    /// home settlement). Generated with the galaxy; `None` only in pre-feature
    /// snapshots. The command center sits at this system's position.
    #[serde(default)]
    pub system: Option<EntityId>,
    /// Two deterministic nearby founding prospects: population-led first,
    /// mineral-led second. Exact properties remain survey-gated to each player.
    #[serde(default)]
    pub founding_opportunities: Vec<EntityId>,
}

/// §ore-ladder: the RAW deposit table — what a system's deposits can be, how
/// often, and where. Deposits are drawn ONLY from raws (processed/advanced
/// goods are MADE, never mined). Each row is `(commodity, weight, lo, hi)`: the
/// commodity is eligible while the system's frontier factor lies in `[lo, hi]`,
/// and its draw weight ramps in over [`RAW_RAMP`] past `lo` and out over the
/// same distance before `hi`, so borders are soft. Rarity comes from the
/// WEIGHTS and WINDOWS together, never from the price ladder alone: the bulk
/// ores are common everywhere, Titanium ore opens in the outer half, Rare-metal
/// ore only past the pirate ring. Galaxy-wide deposit shares this produces
/// (area-uniform stars, rim systems rolling more deposits): Ferrite ~26%,
/// Volatiles ~15%, Crystalline ~16%, Biomass ~8%, Cuprite ~18%, Titanium ~11%,
/// Rare-metal ~6% — a default four-player galaxy holds about five Rare-metal
/// deposits in total. The ordering is test-pinned. Tunable.
pub const RAW_DEPOSIT_TABLE: [(Commodity, f64, f64, f64); 7] = [
    (Commodity::MetallicOre, 1.00, 0.00, 1.00),
    (Commodity::Volatiles, 0.55, 0.00, 1.00),
    (Commodity::CrystallineOre, 1.70, 0.00, 0.80),
    (Commodity::Biomass, 0.70, 0.00, 0.85),
    (Commodity::CupriteOre, 0.72, 0.20, 1.00),
    (Commodity::TitaniumOre, 0.48, TITANIUM_MIN_FRONTIER, 1.00),
    (Commodity::RareMetalOre, 0.40, RARE_METAL_MIN_FRONTIER, 1.00),
];
/// Titanium ore opens in the outer half of the disk.
pub const TITANIUM_MIN_FRONTIER: f64 = 0.45;
/// Rare-metal ore lies beyond the pirate ring ([`crate::pirate::PIRATE_RING_HI`]
/// is 0.72), so every haul home crosses hunted space.
pub const RARE_METAL_MIN_FRONTIER: f64 = 0.70;
/// Width of the soft border at each window edge.
const RAW_RAMP: f64 = 0.15;

/// A commodity's draw weight at a frontier factor (0 outside its window).
pub fn raw_deposit_weight(resource: Commodity, frontier: f64) -> f64 {
    let Some(&(_, weight, lo, hi)) = RAW_DEPOSIT_TABLE.iter().find(|row| row.0 == resource)
    else {
        return 0.0;
    };
    if frontier < lo || frontier > hi {
        return 0.0;
    }
    let ramp_in = if lo > 0.0 { ((frontier - lo) / RAW_RAMP).min(1.0) } else { 1.0 };
    let ramp_out = if hi < 1.0 { ((hi - frontier) / RAW_RAMP).min(1.0) } else { 1.0 };
    weight * ramp_in * ramp_out
}

/// Draw one raw deposit commodity for a system at `frontier` from the rows
/// `eligible` admits (one RNG draw). Ferrite is eligible everywhere, so the
/// ore-only draw the sparse stars use can never come up empty.
pub fn roll_raw_deposit_where(
    rng: &mut Rng,
    frontier: f64,
    eligible: impl Fn(Commodity) -> bool,
) -> Commodity {
    let frontier = frontier.clamp(0.0, 1.0);
    let weight = |c: Commodity| if eligible(c) { raw_deposit_weight(c, frontier) } else { 0.0 };
    let total: f64 = RAW_DEPOSIT_TABLE.iter().map(|row| weight(row.0)).sum();
    let mut pick = rng.next_f64() * total;
    for &(resource, ..) in &RAW_DEPOSIT_TABLE {
        pick -= weight(resource);
        if pick <= 0.0 && weight(resource) > 0.0 {
            return resource;
        }
    }
    Commodity::MetallicOre
}

/// Draw one raw deposit commodity for a system at `frontier` (one RNG draw).
pub fn roll_raw_deposit(rng: &mut Rng, frontier: f64) -> Commodity {
    roll_raw_deposit_where(rng, frontier, |_| true)
}

/// §ore-ladder: the two scarce ores are FINITE — a rim claim is a rush, not a
/// permanent faucet. Sized in BULK-equivalent units and scaled by the ore's
/// unit ratio (§ore-density), so at a base worker's ~0.45/s content rate a
/// seam lasts twelve to twenty-five hours of extraction before staffing, tiers
/// and geology shorten it. Bulk ores stay renewable. Tunable.
pub const FINITE_RESERVES_LO: f64 = 20_000.0;
pub const FINITE_RESERVES_HI: f64 = 40_000.0;

/// §ore-ladder: the scarce ores' seams run RICHER than the bulk ores' (they are
/// rare, finite and far — the find has to be worth the trip). This also holds
/// the organic survey pacing where the old tier ladder left it: the pinned
/// 10–15% build-order-changing target lost about a point when Rare-metal ore
/// stopped being half the rim, and the richer seam gives it back. Tunable.
pub const SCARCE_VEIN_MULT: f64 = 1.20;

/// The richness premium a freshly rolled deposit of `resource` carries.
pub fn vein_mult(resource: Commodity) -> f64 {
    match resource {
        Commodity::TitaniumOre | Commodity::RareMetalOre => SCARCE_VEIN_MULT,
        _ => 1.0,
    }
}

/// Reserves for a freshly rolled deposit: finite for the scarce ores, `None`
/// (renewable) for everything else. `vein` is the deposit's richness roll
/// normalised to `0..1` — a richer seam is also a deeper one — so sizing the
/// reserve costs no RNG draw of its own and the generator's stream (and every
/// seeded star position after it) stays exactly where it was.
pub fn reserves_for(resource: Commodity, vein: f64) -> Option<f64> {
    match resource {
        Commodity::TitaniumOre | Commodity::RareMetalOre => {
            let vein = vein.clamp(0.0, 1.0);
            let bulk_units = FINITE_RESERVES_LO + (FINITE_RESERVES_HI - FINITE_RESERVES_LO) * vein;
            Some((bulk_units * crate::production::ore_bulk_ratio(resource)).round())
        }
        _ => None,
    }
}

/// Base extraction rate (units/sec) a deposit produces; scaled up toward the
/// frontier. Tunable — balance is not the goal, a working loop is.
/// The ordinary home-site extraction reference. Survey opportunity ratings use
/// this same anchor, so a displayed `×2` means twice a baseline home worker's
/// natural deposit output before staffing, tiers, specialists, food or research.
pub const DEPOSIT_BASE_RICHNESS: f64 = 0.45;
/// Claim cost = `CLAIM_BASE` + `CLAIM_VALUE_K` × the system's value-rate
/// (Σ richness·base_price), so richer frontier systems cost more to claim.
const CLAIM_BASE: f64 = 600.0;
const CLAIM_VALUE_K: f64 = 45.0;

/// Generate `count` star systems uniformly over the galaxy disk (area-uniform
/// via the √u radius trick), keeping a clear margin around the hub. Each system
/// gets resource deposits whose richness and value rise toward the rim — the
/// GDD's distance/value gradient: the best production is out in the dangerous,
/// fog-blind frontier (§4).
pub fn generate_systems(
    rng: &mut Rng,
    radius: f64,
    count: u32,
    names: &[String],
    alloc: &mut dyn FnMut() -> EntityId,
) -> Vec<StarSystem> {
    let mut systems = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        // Area-uniform radius in [0.12R, 0.96R].
        let u = rng.next_f64().sqrt();
        let r = radius * (0.12 + 0.84 * u);
        let theta = rng.range(0.0, std::f64::consts::TAU);
        let pos = Vec2::from_polar(theta, r);
        let id = alloc();
        // §naming: the caller supplies a galaxy-unique, seed-shuffled name list.
        let name = names[i].clone();
        // Frontier factor in [0,1]: 0 at the inner margin, 1 at the rim.
        let frontier = u; // == (r/radius - 0.12) / 0.84, monotonic in distance
        let deposits = generate_deposits(rng, frontier);
        // §bodies: NEW systems are born with their roster — deposits are
        // rolled first (the frontier gradient is untouched), then placed onto
        // affinity-correct bodies by the shared generator.
        let bodies = crate::body::generate_bodies(&id.0.to_string(), &name, &deposits);
        systems.push(unowned_system(id, pos, name, bodies, claim_cost_for(&deposits)));
    }
    systems
}

/// Shared empty administration for both full colony systems and sparse
/// exploration systems. The caller owns the body roster; no filler planets or
/// hidden resource rolls occur here.
pub(crate) fn unowned_system(
    id: EntityId,
    pos: Vec2,
    name: String,
    bodies: Vec<crate::body::Body>,
    claim_cost: f64,
) -> StarSystem {
    StarSystem {
        id,
        pos,
        name,
        bodies,
        legacy_deposits: Vec::new(),
        claim_cost,
        owner: None,
        claimed_at: None,
        stockpile: BTreeMap::new(),
        industry: Default::default(),
        modules: BTreeMap::new(),
        legacy_extractor_tier: 0,
        legacy_depot_tier: 0,
        legacy_shipyard_tier: 0, // frontier systems must EARN their shipyards
        legacy_sensor_tier: 0,
        legacy_defense_tier: 0,
        defense_pool: 0.0,
        legacy_habitat_tier: 0,
        food_state: crate::colony::FoodState::default(),
        legacy_refinery_tier: 0,
        blockade: None,
        garrison_fed: true,
        garrison_suppression: 0.0,
        blockade_prev: None,
        trait_: None,
        cache_claimed: false,
        legacy_structures: BTreeMap::new(),
        legacy_population: 0.0,
        legacy_assignments: BTreeMap::new(),
        specialists: BTreeMap::new(),
    }
}

/// Deterministically generate a system's deposits from its frontier factor:
/// more deposits, richer, and drawn from the rarity table at that distance —
/// the scarce ores only open toward the rim, and carry FINITE reserves. Two
/// draws per deposit, as the tier ladder took, so a seed's star chart is
/// unchanged by the table: only what lies under each star moved.
fn generate_deposits(rng: &mut Rng, frontier: f64) -> Vec<Deposit> {
    // 1 deposit near the hub, up to 3 at the rim.
    let n = (1.0 + frontier * 2.0 + rng.range(0.0, 0.9))
        .floor()
        .clamp(1.0, 3.0) as usize;
    let mut deposits = Vec::with_capacity(n);
    for _ in 0..n {
        let resource = roll_raw_deposit(rng, frontier);
        // Richness rises toward the frontier, but remains a SITE advantage
        // rather than a universal frontier jackpot. Extra deposits and rarer
        // commodities already make the rim valuable; this narrower band leaves
        // Rich/Ultra Rich geology and matching specials room to create the
        // memorable ×1.8–3 specialty discoveries.
        let vein = rng.range(0.80, 1.20);
        let richness =
            DEPOSIT_BASE_RICHNESS * (0.70 + 0.55 * frontier) * vein * vein_mult(resource);
        let reserves = reserves_for(resource, (vein - 0.80) / 0.40);
        deposits.push(Deposit {
            resource,
            richness,
            reserves,
            accessibility: frontier,
        });
    }
    deposits
}

/// A deposit's VALUE RATE: credits per second at full natural extraction —
/// richness (the content rate) × the ore's unit ratio (§ore-density) × its
/// reference price. The single scalar behind claim costs, survey bands and
/// system worth, so a dense ore is valued by what it earns, not by its price tag.
pub fn deposit_value_rate(d: &Deposit) -> f64 {
    d.richness * crate::production::ore_bulk_ratio(d.resource) * base_price(d.resource)
}

/// The credit cost to claim a system, from the total value-rate of its deposits
/// (Σ [`deposit_value_rate`]). Richer/more-valuable frontier systems cost more.
pub fn claim_cost_for(deposits: &[Deposit]) -> f64 {
    let value_rate: f64 = deposits.iter().map(deposit_value_rate).sum();
    CLAIM_BASE + CLAIM_VALUE_K * value_rate
}

/// Generate `count` home-anchor slots evenly spaced around a ring at
/// `ring_frac · radius`, with small seeded jitter so they aren't perfectly
/// regular.
pub const HOME_SLOT_RADIAL_JITTER_FRAC: f64 = 0.08;

pub fn generate_home_slots(
    rng: &mut Rng,
    radius: f64,
    ring_frac: f64,
    count: u32,
) -> Vec<HomeSlot> {
    let count = count.max(1);
    let base = radius * ring_frac;
    let mut slots = Vec::with_capacity(count as usize);
    for i in 0..count {
        let base_angle = std::f64::consts::TAU * (i as f64) / (count as f64);
        // Jitter angle by up to ±¼ of the slot spacing, radius by ±8%.
        let ang_jitter = rng.range(-1.0, 1.0) * (std::f64::consts::TAU / count as f64) * 0.25;
        let r_jitter =
            base * rng.range(-HOME_SLOT_RADIAL_JITTER_FRAC, HOME_SLOT_RADIAL_JITTER_FRAC);
        let pos = Vec2::from_polar(base_angle + ang_jitter, base + r_jitter);
        slots.push(HomeSlot {
            pos,
            owner: None,
            claimed_at: None,
            system: None, // set when the co-located home system is generated
            founding_opportunities: Vec::new(),
        });
    }
    slots
}

/// XORed into the seed so each home system's geology is deterministic but
/// independent of the frontier-system RNG stream (so changing `system_count`
/// never shifts home geology, and home generation never perturbs the frontier
/// or the world's live event RNG).
const HOME_SYSTEM_MAGIC: u64 = 0x484F_4D45_5359_5354; // "HOMESYST"

/// A developed but MODEST starter geology: two renewable deposits in the cheap,
/// steady commodities (Provisions + Ore) at low richness. A reliable home base
/// that produces from turn one — deliberately weaker than the dangerous frontier,
/// so expansion outward stays the reward/risk (the distance/value gradient holds).
fn generate_home_deposits(rng: &mut Rng) -> Vec<Deposit> {
    // §economy: the direct successors of the old Provisions + Ore pair — the
    // home extracts BIOMASS (→ Provisions via the Agroplex) and METALLIC ORE
    // (→ Alloys via a Smelter), at the same modest richnesses.
    vec![
        Deposit {
            resource: Commodity::Biomass,
            richness: DEPOSIT_BASE_RICHNESS * rng.range(0.85, 1.15),
            reserves: None,
            accessibility: 0.1,
        },
        Deposit {
            resource: Commodity::MetallicOre,
            richness: DEPOSIT_BASE_RICHNESS * rng.range(0.7, 1.0),
            reserves: None,
            accessibility: 0.1,
        },
    ]
}

/// Starting construction supplies, shared by fresh generation and first join
/// into an unused home slot from an older galaxy. Fuel is added on join.
pub(crate) fn home_starting_stockpile() -> BTreeMap<Commodity, f64> {
    [
        (Commodity::Provisions, crate::colony::HOME_PROVISIONS_SEED),
        (Commodity::Machinery, 15.0),
        (Commodity::Alloys, 30.0),
    ].into_iter().collect()
}

/// One developed home star system, co-located at `pos`, with modest seeded
/// geology keyed by home `index` (so it's reproducible and independent of the
/// frontier stream). `owner`/`claimed_at` are left `None` — ownership is granted
/// to the player on join (free; the command center sits here).
pub fn generate_home_system(
    seed: u64,
    index: usize,
    id: EntityId,
    pos: Vec2,
    name: String,
) -> StarSystem {
    let mut rng =
        Rng::new(seed ^ HOME_SYSTEM_MAGIC ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let deposits = generate_home_deposits(&mut rng);
    let claim_cost = claim_cost_for(&deposits);
    // §naming: the caller supplies a galaxy-unique name; geology stays keyed to
    // `index` (its own re-seeded stream), untouched by the naming change.
    // §bodies: the home is born with its roster; the bootstrap then SITES its
    // structures on the right bodies via the shared rules.
    let mut bodies = crate::body::generate_bodies(&id.0.to_string(), &name, &deposits);
    // Homes are deliberately reliable rather than jackpot rolls. Their garden
    // body is a roomy Terran baseline; their starter ore is Average. The nearby
    // expansion pair supplies the exciting contrast.
    if let Some(b) = bodies
        .iter_mut()
        .find(|b| b.deposits.iter().any(|d| d.resource == Commodity::Biomass))
    {
        b.profile.size = crate::body::BodySize::Large;
        b.profile.environment = crate::body::Environment::Terran;
        b.profile.geology = crate::body::Geology::Average;
        b.profile.special = None;
        b.habitable = true;
    }
    if let Some(b) = bodies.iter_mut().find(|b| {
        b.deposits
            .iter()
            .any(|d| d.resource == Commodity::MetallicOre)
    }) {
        b.profile.geology = crate::body::Geology::Average;
    }
    let mut sys = StarSystem {
        id,
        pos,
        name,
        bodies,
        legacy_deposits: Vec::new(),
        claim_cost,
        owner: None,
        claimed_at: None,
        // Enough for Mining Complex I plus a small buffer. The granted Tiny
        // Freighter earns the manufactured imports for a Shipyard and more hulls;
        // their construction materials are not prepaid in the founding stockpile.
        stockpile: home_starting_stockpile(),
        industry: Default::default(),
        modules: BTreeMap::new(),
        legacy_extractor_tier: 0,
        legacy_depot_tier: 0,
        legacy_shipyard_tier: 0,
        legacy_sensor_tier: 0,
        legacy_defense_tier: 0,
        defense_pool: 0.0,
        legacy_habitat_tier: 0,
        food_state: crate::colony::FoodState::default(),
        legacy_refinery_tier: 0,
        blockade: None,
        garrison_fed: true,
        garrison_suppression: 0.0,
        blockade_prev: None,
        trait_: None,
        cache_claimed: false,
        legacy_structures: BTreeMap::new(),
        legacy_population: 0.0,
        legacy_assignments: BTreeMap::new(),
        specialists: BTreeMap::new(),
    };
    // HOME BOOTSTRAP (§buildings step 3 → §economy Part 3 → §bodies): a home
    // begins as a small food-secure settlement. The ore mine starts the export
    // loop; the Shipyard is a later reinvestment, not pre-granted infrastructure.
    let bootstrap = [
        (crate::build::StructureKind::Bioharvester, 1),
        (crate::build::StructureKind::Agroplex, 1),
        (crate::build::StructureKind::Habitat, 1),
        (crate::build::StructureKind::Warehouse, 1),
    ];
    for (kind, tier) in bootstrap {
        let target = sys.site_for(kind).unwrap_or(0);
        if let Some(b) = sys.bodies.iter_mut().find(|b| b.id == target) {
            b.set_tier(kind, tier);
        }
    }
    sys.seed_population(crate::colony::HOME_FOUNDING_POP);
    // Both starting cohorts staff the food chain. Posting a mining job shares
    // that workforce under the normal staffing rule and attracts Managed migrants;
    // it does not require a third, silently granted cohort.
    for kind in [
        crate::build::StructureKind::Bioharvester,
        crate::build::StructureKind::Agroplex,
    ] {
        if let Some(b) = sys.bodies.iter_mut().find(|b| b.tier(kind) > 0) {
            b.assignments
                .insert(kind, crate::production::Assignment::crew(1));
        }
    }
    sys
}

/// One home star system per home slot, co-located with each slot — the developed
/// home bases players begin owning. Ids drawn from the shared allocator so they
/// stay unique; geology is deterministic per home index.
pub fn generate_home_systems(
    seed: u64,
    slots: &[HomeSlot],
    names: &[String],
    alloc: &mut dyn FnMut() -> EntityId,
) -> Vec<StarSystem> {
    slots
        .iter()
        .enumerate()
        .map(|(i, slot)| generate_home_system(seed, i, alloc(), slot.pos, names[i].clone()))
        .collect()
}

/// A curated pool of evocative, pronounceable ONE-WORD system names — real star /
/// catalogue names, public-domain mythological figures (Slavic, Norse, Greek,
/// Roman, Egyptian, Mesopotamian), and frontier/industrial words fitting a
/// corporate-expansion setting. Short, unambiguous spoken aloud, and visually
/// distinct at a glance. Drawn WITHOUT REPLACEMENT per galaxy so no two systems
/// collide (see [`shuffled_system_names`]). No trademarked or invented names.
pub const SYSTEM_NAMES: &[&str] = &[
    // ── Stars & catalogue names ──
    "Vega",
    "Altair",
    "Rigel",
    "Mizar",
    "Antares",
    "Deneb",
    "Fomalhaut",
    "Alcyone",
    "Canopus",
    "Sirius",
    "Procyon",
    "Capella",
    "Arcturus",
    "Aldebaran",
    "Bellatrix",
    "Betelgeuse",
    "Spica",
    "Regulus",
    "Pollux",
    "Castor",
    "Alnilam",
    "Alnitak",
    "Saiph",
    "Mintaka",
    "Naos",
    "Adhara",
    "Wezen",
    "Alhena",
    "Elnath",
    "Menkar",
    "Hamal",
    "Sheratan",
    "Mirach",
    "Almach",
    "Algol",
    "Merak",
    "Dubhe",
    "Phecda",
    "Megrez",
    "Alioth",
    "Alkaid",
    "Kochab",
    "Polaris",
    "Thuban",
    "Etamin",
    "Rastaban",
    "Zosma",
    "Denebola",
    "Alphard",
    "Gomeisa",
    "Markab",
    "Scheat",
    "Algenib",
    "Enif",
    "Skat",
    "Diphda",
    "Achernar",
    "Acamar",
    "Alnair",
    "Peacock",
    "Atria",
    "Gacrux",
    "Acrux",
    "Mimosa",
    "Hadar",
    "Shaula",
    "Sargas",
    "Nunki",
    "Ascella",
    "Albireo",
    "Tarazed",
    "Alshain",
    "Rukbat",
    "Nashira",
    "Dabih",
    "Wasat",
    "Tejat",
    "Propus",
    "Mebsuta",
    "Zaurak",
    "Cursa",
    "Keid",
    "Rana",
    "Sadr",
    "Gienah",
    "Ruchbah",
    "Segin",
    "Caph",
    "Achird",
    "Sabik",
    "Izar",
    "Seginus",
    "Nekkar",
    "Alphecca",
    // ── Slavic ──
    "Veles",
    "Perun",
    "Morana",
    "Svarog",
    "Dazhbog",
    "Mokosh",
    "Stribog",
    "Chernobog",
    "Belobog",
    "Lada",
    "Jarilo",
    "Zorya",
    "Simargl",
    "Radegast",
    "Triglav",
    "Vesna",
    // ── Norse ──
    "Njord",
    "Freya",
    "Freyr",
    "Odin",
    "Tyr",
    "Baldr",
    "Heimdall",
    "Loki",
    "Frigg",
    "Idun",
    "Bragi",
    "Vidar",
    "Vali",
    "Forseti",
    "Ullr",
    "Skadi",
    "Nanna",
    "Sif",
    "Hel",
    "Fenrir",
    "Aegir",
    "Nidhogg",
    "Ymir",
    "Mimir",
    "Kvasir",
    "Surtr",
    "Sleipnir",
    "Bifrost",
    "Asgard",
    "Vanaheim",
    // ── Greek ──
    "Hyperion",
    "Nemesis",
    "Helios",
    "Selene",
    "Nyx",
    "Erebus",
    "Gaia",
    "Cronus",
    "Rhea",
    "Themis",
    "Tethys",
    "Oceanus",
    "Iapetus",
    "Atlas",
    "Prometheus",
    "Pallas",
    "Astraeus",
    "Leto",
    "Asteria",
    "Metis",
    "Dione",
    "Phoebe",
    "Theia",
    "Hecate",
    "Kratos",
    "Styx",
    "Eris",
    "Thanatos",
    "Hypnos",
    "Cerberus",
    // ── Roman ──
    "Janus",
    "Vesta",
    "Ceres",
    "Juno",
    "Minerva",
    "Vulcan",
    "Bellona",
    "Fortuna",
    "Quirinus",
    "Faunus",
    "Silvanus",
    "Pomona",
    "Concordia",
    "Aurora",
    // ── Egyptian ──
    "Anubis",
    "Horus",
    "Osiris",
    "Isis",
    "Thoth",
    "Sobek",
    "Sekhmet",
    "Bastet",
    "Ptah",
    "Hathor",
    "Khonsu",
    "Amun",
    "Aten",
    "Maat",
    "Neith",
    "Wadjet",
    "Apophis",
    "Wepwawet",
    // ── Mesopotamian ──
    "Ereshkigal",
    "Nergal",
    "Marduk",
    "Enlil",
    "Enki",
    "Inanna",
    "Ishtar",
    "Tiamat",
    "Anshar",
    "Ninhursag",
    "Shamash",
    "Ninurta",
    "Nabu",
    "Dumuzi",
    "Pazuzu",
    "Gilgamesh",
    // ── Frontier & industrial ──
    "Anvil",
    "Kiln",
    "Tally",
    "Bulwark",
    "Ember",
    "Lattice",
    "Quarry",
    "Reckoning",
    "Forge",
    "Crucible",
    "Foundry",
    "Bellows",
    "Girder",
    "Rivet",
    "Gantry",
    "Derrick",
    "Sluice",
    "Ballast",
    "Slag",
    "Cinder",
    "Ingot",
    "Tithe",
    "Ledger",
    "Sable",
    "Cairn",
    "Beacon",
    "Palisade",
    "Rampart",
    "Bastion",
    "Redoubt",
    "Keystone",
    "Millstone",
    "Whetstone",
    "Lodestone",
    "Flint",
    "Tinder",
    "Ashfall",
    "Cistern",
    "Conduit",
    "Spindle",
    "Loom",
    "Hearth",
    "Furnace",
    "Temper",
    "Quench",
    "Pinion",
    "Ratchet",
    "Flywheel",
    "Piston",
    "Prospect",
    "Placer",
    "Tailings",
];

/// Deterministic overflow suffixes for a galaxy larger than [`SYSTEM_NAMES`] — a
/// second (third…) pass appends these so a bare name is never repeated.
const NAME_OVERFLOW_SUFFIXES: &[&str] = &[
    "Reach", "Deep", "Verge", "Expanse", "Reef", "Drift", "Marches", "Hollow",
];

/// `needed` galaxy-unique system names: the curated [`SYSTEM_NAMES`] pool shuffled
/// IN PLACE by the SEEDED `rng` (Fisher–Yates — no new RNG stream, no non-seeded
/// randomness), then handed out in order. If the galaxy needs more names than the
/// pool holds, deterministic suffixed passes ("Vega Reach", … then "Vega Deep", …)
/// extend it so generation never repeats a bare name and never panics.
pub fn shuffled_system_names(rng: &mut Rng, needed: usize) -> Vec<String> {
    let mut base: Vec<&'static str> = SYSTEM_NAMES.to_vec();
    for i in (1..base.len()).rev() {
        // j ∈ [0, i] (next_f64 is [0,1); .min(i) guards the impossible 1.0).
        let j = ((rng.next_f64() * (i as f64 + 1.0)).floor() as usize).min(i);
        base.swap(i, j);
    }
    let mut out: Vec<String> = base.iter().map(|s| (*s).to_string()).collect();
    let mut pass = 0usize;
    while out.len() < needed {
        let suffix = NAME_OVERFLOW_SUFFIXES[pass % NAME_OVERFLOW_SUFFIXES.len()];
        let cycle = pass / NAME_OVERFLOW_SUFFIXES.len(); // 0 for the first pass round
        for name in &base {
            out.push(if cycle == 0 {
                format!("{name} {suffix}")
            } else {
                // Beyond one full round of suffixes, disambiguate by cycle number.
                format!("{name} {suffix} {}", cycle + 1)
            });
            if out.len() >= needed {
                break;
            }
        }
        pass += 1;
    }
    out
}

/// Pick a galaxy-unique name for a home system minted AT RUNTIME (an over-capacity
/// or regenerated home), avoiding every name already `taken`. Deterministic:
/// re-seeds the SAME shuffle from the world `seed` (exactly as home geology does)
/// and returns the first free entry — never non-seeded, never a repeat, never a
/// panic (the shuffle always yields more distinct names than the taken set).
pub fn pick_unused_name(seed: u64, taken: &BTreeSet<String>) -> String {
    let mut rng = Rng::new(seed);
    let names = shuffled_system_names(&mut rng, taken.len() + SYSTEM_NAMES.len());
    names
        .into_iter()
        .find(|n| !taken.contains(n))
        .expect("a full pool beyond the taken set always leaves a free name")
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn system_name_pool_has_no_duplicates() {
        let set: BTreeSet<&&str> = SYSTEM_NAMES.iter().collect();
        assert_eq!(
            set.len(),
            SYSTEM_NAMES.len(),
            "the curated pool must be collision-free"
        );
        assert!(
            SYSTEM_NAMES.len() >= 200,
            "pool is ~250 names, got {}",
            SYSTEM_NAMES.len()
        );
        // Names are one word (no whitespace) so the overflow-suffix pass reads clean.
        for n in SYSTEM_NAMES {
            assert!(!n.contains(' '), "{n} should be one word");
            assert!(!n.is_empty());
        }
    }

    #[test]
    fn shuffled_names_are_deterministic_unique_and_never_short() {
        // Same seed → identical shuffle + hand-out order (determinism is law).
        let a = shuffled_system_names(&mut Rng::new(777), 120);
        let b = shuffled_system_names(&mut Rng::new(777), 120);
        assert_eq!(a, b, "same seed reproduces the same names");
        // A different seed generally reorders (not a hard guarantee, but expected).
        let c = shuffled_system_names(&mut Rng::new(778), 120);
        assert_ne!(a, c, "a different seed shuffles differently");
        // No duplicates within a galaxy's names.
        let set: BTreeSet<&String> = a.iter().collect();
        assert_eq!(set.len(), a.len(), "no two systems share a name");
    }

    #[test]
    fn overflow_past_the_pool_extends_without_repeats_or_panic() {
        // Demand far more than the pool holds — several suffix passes.
        let n = SYSTEM_NAMES.len() * 3 + 17;
        let names = shuffled_system_names(&mut Rng::new(4242), n);
        assert_eq!(names.len(), n);
        let set: BTreeSet<&String> = names.iter().collect();
        assert_eq!(set.len(), n, "overflow suffixes never repeat a name");
        // The first pool-worth are bare names; the next are suffixed.
        assert!(
            names[SYSTEM_NAMES.len()].contains(' '),
            "overflow names carry a suffix"
        );
    }

    #[test]
    fn pick_unused_name_avoids_taken_and_is_deterministic() {
        let mut taken: BTreeSet<String> = shuffled_system_names(&mut Rng::new(9), 50)
            .into_iter()
            .collect();
        let a = pick_unused_name(9, &taken);
        let b = pick_unused_name(9, &taken);
        assert_eq!(a, b, "deterministic for a fixed (seed, taken)");
        assert!(!taken.contains(&a), "never collides with an in-use name");
        // Adding it advances the pick to a new free name.
        taken.insert(a.clone());
        let next = pick_unused_name(9, &taken);
        assert_ne!(next, a);
        assert!(!taken.contains(&next));
    }
}

#[cfg(test)]
mod deposit_tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Area-uniform stars, each rolled by the live generator: the galaxy-wide
    /// deposit shares the table actually produces.
    fn galaxy_shares(seed: u64) -> (BTreeMap<Commodity, f64>, Vec<(f64, Deposit)>) {
        let mut rng = Rng::new(seed);
        let mut count: BTreeMap<Commodity, f64> = BTreeMap::new();
        let mut rolled = Vec::new();
        let mut total = 0.0;
        for _ in 0..20_000 {
            let frontier = rng.next_f64().sqrt();
            for d in generate_deposits(&mut rng, frontier) {
                *count.entry(d.resource).or_default() += 1.0;
                total += 1.0;
                rolled.push((frontier, d));
            }
        }
        for v in count.values_mut() {
            *v /= total;
        }
        (count, rolled)
    }

    /// §ore-ladder: scarcity is REAL in the generator — the price ladder and
    /// the deposit shares point the same way, and can never silently invert.
    #[test]
    fn rarer_ores_are_rarer_deposits() {
        let (share, _) = galaxy_shares(11);
        let s = |c: Commodity| share.get(&c).copied().unwrap_or(0.0);
        assert!(s(Commodity::RareMetalOre) < s(Commodity::TitaniumOre));
        assert!(s(Commodity::TitaniumOre) < s(Commodity::CupriteOre));
        assert!(s(Commodity::CupriteOre) < s(Commodity::MetallicOre));
        assert!(s(Commodity::RareMetalOre) < 0.08, "rare metal {:.3}", s(Commodity::RareMetalOre));
        assert!(s(Commodity::CrystallineOre) > 0.10, "crystalline {:.3}", s(Commodity::CrystallineOre));
        assert!(s(Commodity::MetallicOre) > 0.22, "ferrite {:.3}", s(Commodity::MetallicOre));
        for c in Commodity::RAW {
            assert!(s(c) > 0.03, "{c:?} must still occur: {:.3}", s(c));
        }
    }

    /// The scarce ores are gated to the rim: no Titanium ore inside the inner
    /// half, no Rare-metal ore short of the pirate ring.
    #[test]
    fn scarce_ores_only_open_toward_the_rim() {
        let (_, rolled) = galaxy_shares(12);
        for (frontier, d) in &rolled {
            match d.resource {
                Commodity::RareMetalOre => assert!(*frontier >= RARE_METAL_MIN_FRONTIER),
                Commodity::TitaniumOre => assert!(*frontier >= TITANIUM_MIN_FRONTIER),
                _ => {}
            }
        }
        assert!(rolled.iter().any(|(f, d)| d.resource == Commodity::RareMetalOre && *f > 0.9));
        assert_eq!(raw_deposit_weight(Commodity::RareMetalOre, 0.5), 0.0);
        assert!(raw_deposit_weight(Commodity::RareMetalOre, 1.0) > 0.0);
        assert!(raw_deposit_weight(Commodity::MetallicOre, 0.0) > 0.0);
        assert!(raw_deposit_weight(Commodity::MetallicOre, 1.0) > 0.0);
    }

    /// The two scarce ores carry finite reserves in the tuned band, deeper for
    /// a richer vein; everything else stays renewable. Sparse stars share the
    /// ore-only draw.
    #[test]
    fn scarce_ores_are_finite_and_bulk_ores_renew() {
        let (_, rolled) = galaxy_shares(13);
        for (_, d) in &rolled {
            match d.resource {
                Commodity::TitaniumOre | Commodity::RareMetalOre => {
                    let r = d.reserves.expect("scarce ore is finite");
                    let ratio = crate::production::ore_bulk_ratio(d.resource);
                    let (lo, hi) = (FINITE_RESERVES_LO * ratio, FINITE_RESERVES_HI * ratio);
                    assert!((lo - 1.0..=hi + 1.0).contains(&r), "{r} outside {lo}..{hi}");
                }
                _ => assert_eq!(d.reserves, None, "{:?} renews", d.resource),
            }
        }
        let ratio = crate::production::ore_bulk_ratio;
        assert_eq!(
            reserves_for(Commodity::RareMetalOre, 0.0),
            Some((FINITE_RESERVES_LO * ratio(Commodity::RareMetalOre)).round())
        );
        assert_eq!(
            reserves_for(Commodity::TitaniumOre, 1.0),
            Some((FINITE_RESERVES_HI * ratio(Commodity::TitaniumOre)).round())
        );
        // A dense seam holds the same HOURS of extraction in fewer, richer units.
        assert!(reserves_for(Commodity::RareMetalOre, 1.0) < reserves_for(Commodity::TitaniumOre, 1.0));
        assert_eq!(reserves_for(Commodity::MetallicOre, 0.5), None);
        let mut rng = Rng::new(14);
        for _ in 0..500 {
            let frontier = rng.next_f64();
            let ore = roll_raw_deposit_where(&mut rng, frontier, |c| c.is_ore());
            assert!(ore.is_ore(), "{ore:?}");
        }
    }
}

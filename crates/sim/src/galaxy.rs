//! Static galaxy geography: the hub, procedurally-placed star systems, and the
//! ring of home anchors (§4). Generated deterministically from the seed.
//!
//! "No discrete zones": systems are scattered continuously across one radial
//! space, the hub fixed at the centre, homes distributed around a ring as
//! bright spots. Resources/claims hang off systems in later milestones.

use std::collections::BTreeMap;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    /// A commodity that already trades on the hub Exchange.
    pub resource: Commodity,
    /// Units produced per second at full extraction.
    pub richness: f64,
    /// Remaining reserves; `None` = renewable (never depletes). Finite deposits
    /// run dry — kept simple for the alpha by generating renewable deposits.
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Number of Extractor upgrades built here (§step1 structure sink). Scales
    /// every deposit's richness by `EXTRACTOR_RICHNESS_MULT^tier` in accrual.
    #[serde(default, rename = "extractor_tier")]
    pub legacy_extractor_tier: u32,
    /// Number of Depot upgrades built here (§buildings step 2). Each tier raises
    /// the system's storage cap by `STORAGE_PER_DEPOT_TIER`. `default` = 0 on old
    /// snapshots (they get the base cap; oversize stockpiles are grandfathered —
    /// the cap blocks NEW inflow only, it never destroys what's stored).
    #[serde(default, rename = "depot_tier")]
    pub legacy_depot_tier: u32,
    /// Number of Shipyard upgrades built here (§buildings step 3). Gates ship
    /// construction: Convoy needs tier ≥ 1, Raider ≥ 2 (`required_shipyard_tier`).
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
    /// (efficiency drops, growth stops) — nothing is destroyed, nobody dies
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
    /// §explore Part 3: the system's HIDDEN TRAIT (R3) — revealed only by
    /// ownership; effects are always-on ground truth. Seeded at generation
    /// (`TRAIT_FRACTION` of systems, an isolated stream). `default` None — a
    /// pre-feature galaxy simply has none (acceptable; new generations do).
    #[serde(default)]
    pub trait_: Option<crate::explore::SystemTrait>,
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    /// STACK (Depot storage capacity, Defense Platform strength).
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
        let planets: Vec<&crate::body::Body> = self.bodies.iter().filter(|b| b.parent.is_none()).collect();
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
            .find(|b| b.deposits.iter().any(|d| d.resource == crate::cargo::Commodity::Volatiles))
            .map(|b| b.id)
            .or_else(|| planets.iter().find(|b| b.kind == crate::body::BodyKind::GasGiant).map(|b| b.id))
            .or(primary);
        let habitable_body = self
            .bodies
            .iter()
            .find(|b| b.habitable)
            .map(|b| b.id)
            .or_else(|| {
                planets
                    .iter()
                    .find(|b| matches!(b.kind, crate::body::BodyKind::Terrestrial | crate::body::BodyKind::Ocean))
                    .map(|b| b.id)
            })
            .or(primary);
        let industrial_body = planets
            .iter()
            .find(|b| !b.habitable && !matches!(b.kind, crate::body::BodyKind::GasGiant | crate::body::BodyKind::Ice))
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
            K::Habitat | K::Agroplex | K::Academy => habitable_body,
            K::Smelter | K::ElectronicsFabricator | K::MachineWorks | K::ArmamentsComplex => industrial_body,
            K::Shipyard | K::Depot | K::DefensePlatform => primary,
            K::SensorArray => outermost,
        }
    }

    /// §bodies: seed `millions` of population onto the system's natural
    /// habitable body (colony landings, home bootstraps, migration, tests).
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
        let mut assignments: BTreeMap<crate::build::StructureKind, crate::production::Assignment> = BTreeMap::new();
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
                Some(crate::build::StructureKind::VolatileHarvester) => matches!(b.kind, crate::body::BodyKind::Ice | crate::body::BodyKind::GasGiant),
                _ => matches!(b.kind, crate::body::BodyKind::Rocky | crate::body::BodyKind::Terrestrial),
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
    pub fn assign(&mut self, kind: crate::build::StructureKind, asg: crate::production::Assignment) {
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
    pub fn assignment(&self, kind: crate::build::StructureKind) -> Option<&crate::production::Assignment> {
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
            (std::mem::take(&mut self.legacy_extractor_tier), K::MiningComplex),
            (std::mem::take(&mut self.legacy_depot_tier), K::Depot),
            (std::mem::take(&mut self.legacy_shipyard_tier), K::Shipyard),
            (std::mem::take(&mut self.legacy_sensor_tier), K::SensorArray),
            (std::mem::take(&mut self.legacy_defense_tier), K::DefensePlatform),
            (std::mem::take(&mut self.legacy_habitat_tier), K::Habitat),
            (std::mem::take(&mut self.legacy_refinery_tier), K::FuelRefinery),
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
            self.bodies = crate::body::generate_bodies(&self.id.0.to_string(), &self.name, &deposits);
        } else if !self.legacy_deposits.is_empty() {
            // A mixed-era state: bodies exist but a legacy deposit list is
            // still riding along — distribute it by affinity (nothing lost).
            for d in std::mem::take(&mut self.legacy_deposits) {
                self.add_test_deposit(d);
            }
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
            if let Some(b) = self
                .bodies
                .iter_mut()
                .find(|b| b.tier(kind) > 0)
            {
                b.assignments.insert(kind, asg);
            }
        }
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
            .sum()
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
        let Some(b) = self.bodies.iter().find(|b| b.id == body_id) else { return 0.0 };
        let tier = b.tier(kind);
        if tier == 0 {
            return 0.0;
        }
        let workers = b.assignments.get(&kind).map(|a| a.workers).unwrap_or(0);
        let spec_crew = self.effective_specialists().get(&(body_id, kind)).map(|(c, _)| *c).unwrap_or(0);
        ((workers + spec_crew).min(tier) as f64 / tier as f64) * self.staffing_share()
    }

    /// §economy Part 4: the SKILL factor of one BODY's line.
    pub fn skill_factor(&self, body_id: u32, kind: crate::build::StructureKind) -> f64 {
        let tier = self.bodies.iter().find(|b| b.id == body_id).map(|b| b.tier(kind)).unwrap_or(0);
        let (_, matched) = self.effective_specialists().get(&(body_id, kind)).copied().unwrap_or((0, 0));
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
        self.bodies.iter().map(|b| b.structures.values().filter(|t| **t >= 1).count() as u32).sum()
    }

    /// The sensor bubble this system projects FOR ITS OWNER (0 without an array).
    pub fn sensor_bubble(&self) -> f64 {
        crate::build::sensor_array_radius(self.tier(crate::build::StructureKind::SensorArray))
    }

    /// This system's TOTAL storage capacity (§buildings step 2): a base every
    /// system has, plus a chunk per Depot tier. New inflow is capped at this;
    /// what's already stored is never destroyed.
    pub fn storage_cap(&self) -> f64 {
        crate::build::STORAGE_BASE_CAP
            + crate::build::STORAGE_PER_DEPOT_TIER * self.tier(crate::build::StructureKind::Depot) as f64
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
}

/// §economy: the RAW commodity ladder, cheapest → frontier-most (by base
/// price). Deposits are drawn ONLY from raws (processed/advanced goods are
/// MADE, never mined), biased by distance from the hub — near-hub systems hold
/// common/cheap raws, the frontier holds Rare Elements and rich Volatiles (§4).
const RAW_VALUE_TIER: [Commodity; 5] = [
    Commodity::Biomass,
    Commodity::Silicates,
    Commodity::MetallicOre,
    Commodity::Volatiles,
    Commodity::RareElements,
];

/// Base extraction rate (units/sec) a deposit produces; scaled up toward the
/// frontier. Tunable — balance is not the goal, a working loop is.
const DEPOSIT_BASE_RICHNESS: f64 = 0.45;
/// Claim cost = `CLAIM_BASE` + `CLAIM_VALUE_K` × the system's value-rate
/// (Σ richness·base_price), so richer frontier systems cost more to claim.
const CLAIM_BASE: f64 = 600.0;
const CLAIM_VALUE_K: f64 = 45.0;

/// Generate `count` star systems uniformly over the galaxy disk (area-uniform
/// via the √u radius trick), keeping a clear margin around the hub. Each system
/// gets resource deposits whose richness and value rise toward the rim — the
/// GDD's distance/value gradient: the best production is out in the dangerous,
/// fog-blind frontier (§4).
pub fn generate_systems(rng: &mut Rng, radius: f64, count: u32, alloc: &mut dyn FnMut() -> EntityId) -> Vec<StarSystem> {
    let mut systems = Vec::with_capacity(count as usize);
    for _ in 0..count {
        // Area-uniform radius in [0.12R, 0.96R].
        let u = rng.next_f64().sqrt();
        let r = radius * (0.12 + 0.84 * u);
        let theta = rng.range(0.0, std::f64::consts::TAU);
        let pos = Vec2::from_polar(theta, r);
        let id = alloc();
        let name = system_name(rng);
        // Frontier factor in [0,1]: 0 at the inner margin, 1 at the rim.
        let frontier = u; // == (r/radius - 0.12) / 0.84, monotonic in distance
        let deposits = generate_deposits(rng, frontier);
        let claim_cost = claim_cost_for(&deposits);
        // §bodies: NEW systems are born with their roster — deposits are
        // rolled first (the frontier gradient is untouched), then placed onto
        // affinity-correct bodies by the shared generator.
        let bodies = crate::body::generate_bodies(&id.0.to_string(), &name, &deposits);
        systems.push(StarSystem {
            id,
            pos,
            name,
            bodies,
            legacy_deposits: Vec::new(),
            claim_cost,
            owner: None,
            claimed_at: None,
            stockpile: BTreeMap::new(),
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
            trait_: None,
            cache_claimed: false,
            legacy_structures: BTreeMap::new(),
            legacy_population: 0.0,
            legacy_assignments: BTreeMap::new(),
            specialists: BTreeMap::new(),
        });
    }
    systems
}

/// Deterministically generate a system's deposits from its frontier factor:
/// more deposits, richer, and skewed toward valuable commodities the farther out
/// it sits. Renewable (no depletion) for the alpha.
fn generate_deposits(rng: &mut Rng, frontier: f64) -> Vec<Deposit> {
    // 1 deposit near the hub, up to 3 at the rim.
    let n = (1.0 + frontier * 2.0 + rng.range(0.0, 0.9)).floor().clamp(1.0, 3.0) as usize;
    let mut deposits = Vec::with_capacity(n);
    for _ in 0..n {
        // Pick a commodity tier centred on the frontier (cheap near hub, valuable
        // at the rim) with seeded spread.
        let center = frontier * (RAW_VALUE_TIER.len() - 1) as f64;
        let idx = (center + rng.range(-1.1, 1.1)).round().clamp(0.0, (RAW_VALUE_TIER.len() - 1) as f64) as usize;
        let resource = RAW_VALUE_TIER[idx];
        // Richness rises toward the frontier, jittered.
        let richness = DEPOSIT_BASE_RICHNESS * (0.5 + 1.7 * frontier) * rng.range(0.6, 1.4);
        deposits.push(Deposit {
            resource,
            richness,
            reserves: None, // renewable for the alpha
            accessibility: frontier,
        });
    }
    deposits
}

/// The credit cost to claim a system, from the total value-rate of its deposits
/// (Σ richness·base_price). Richer/more-valuable frontier systems cost more.
pub fn claim_cost_for(deposits: &[Deposit]) -> f64 {
    let value_rate: f64 = deposits.iter().map(|d| d.richness * base_price(d.resource)).sum();
    CLAIM_BASE + CLAIM_VALUE_K * value_rate
}

/// Generate `count` home-anchor slots evenly spaced around a ring at
/// `ring_frac · radius`, with small seeded jitter so they aren't perfectly
/// regular.
pub fn generate_home_slots(rng: &mut Rng, radius: f64, ring_frac: f64, count: u32) -> Vec<HomeSlot> {
    let count = count.max(1);
    let base = radius * ring_frac;
    let mut slots = Vec::with_capacity(count as usize);
    for i in 0..count {
        let base_angle = std::f64::consts::TAU * (i as f64) / (count as f64);
        // Jitter angle by up to ±¼ of the slot spacing, radius by ±8%.
        let ang_jitter = rng.range(-1.0, 1.0) * (std::f64::consts::TAU / count as f64) * 0.25;
        let r_jitter = base * rng.range(-0.08, 0.08);
        let pos = Vec2::from_polar(base_angle + ang_jitter, base + r_jitter);
        slots.push(HomeSlot {
            pos,
            owner: None,
            claimed_at: None,
            system: None, // set when the co-located home system is generated
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

/// One developed home star system, co-located at `pos`, with modest seeded
/// geology keyed by home `index` (so it's reproducible and independent of the
/// frontier stream). `owner`/`claimed_at` are left `None` — ownership is granted
/// to the player on join (free; the command center sits here).
pub fn generate_home_system(seed: u64, index: usize, id: EntityId, pos: Vec2) -> StarSystem {
    let mut rng = Rng::new(seed ^ HOME_SYSTEM_MAGIC ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let deposits = generate_home_deposits(&mut rng);
    let claim_cost = claim_cost_for(&deposits);
    let name = system_name(&mut rng);
    // §bodies: the home is born with its roster; the bootstrap then SITES its
    // structures on the right bodies via the shared rules.
    let bodies = crate::body::generate_bodies(&id.0.to_string(), &name, &deposits);
    let mut sys = StarSystem {
        id,
        pos,
        name,
        bodies,
        legacy_deposits: Vec::new(),
        claim_cost,
        owner: None,
        claimed_at: None,
        // §economy Part 3+5 bootstrap stock: a standing Provisions buffer (the
        // food ladder starts Well Supplied while the farm chain spins up) plus
        // the STARTER KIT — enough Machinery/Alloys/Polymers that the first
        // Depot/Habitat/industry doesn't require a market round-trip. (The
        // Fuel movement seed lands on join.) All Tunable.
        stockpile: [
            (Commodity::Provisions, crate::colony::HOME_PROVISIONS_SEED),
            (Commodity::Machinery, 40.0),
            (Commodity::Alloys, 60.0),
            (Commodity::Polymers, 30.0),
        ]
        .into_iter()
        .collect(),
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
        trait_: None,
        cache_claimed: false,
        legacy_structures: BTreeMap::new(),
        legacy_population: 0.0,
        legacy_assignments: BTreeMap::new(),
        specialists: BTreeMap::new(),
    };
    // HOME BOOTSTRAP (§buildings step 3 → §economy Part 3 → §bodies): a home
    // is born a WORKING developed colony — Shipyard (convoys turn one), the
    // extraction its geology calls for, the Agroplex, the Habitat with 2.0M
    // colonists — each structure SITED on its natural body (the mine on the
    // ore body, the farm chain + Habitat on the habitable world, the yard
    // over the primary), pre-staffed on those bodies.
    let bootstrap = [
        (crate::build::StructureKind::Shipyard, crate::build::HOME_SHIPYARD_TIER),
        (crate::build::StructureKind::Bioharvester, 1),
        (crate::build::StructureKind::MiningComplex, 1),
        (crate::build::StructureKind::Agroplex, 1),
        (crate::build::StructureKind::Habitat, 1),
    ];
    for (kind, tier) in bootstrap {
        let target = sys.site_for(kind).unwrap_or(0);
        if let Some(b) = sys.bodies.iter_mut().find(|b| b.id == target) {
            b.set_tier(kind, tier);
        }
    }
    sys.seed_population(crate::colony::HOME_FOUNDING_POP);
    // Pre-staffed: the food chain + the mine (the Shipyard boost crew is the
    // player's first staffing decision once population grows).
    for kind in [
        crate::build::StructureKind::Bioharvester,
        crate::build::StructureKind::MiningComplex,
        crate::build::StructureKind::Agroplex,
    ] {
        if let Some(b) = sys.bodies.iter_mut().find(|b| b.tier(kind) > 0) {
            b.assignments.insert(kind, crate::production::Assignment::crew(1));
        }
    }
    sys
}

/// One home star system per home slot, co-located with each slot — the developed
/// home bases players begin owning. Ids drawn from the shared allocator so they
/// stay unique; geology is deterministic per home index.
pub fn generate_home_systems(seed: u64, slots: &[HomeSlot], alloc: &mut dyn FnMut() -> EntityId) -> Vec<StarSystem> {
    slots
        .iter()
        .enumerate()
        .map(|(i, slot)| generate_home_system(seed, i, alloc(), slot.pos))
        .collect()
}

/// A short catalogue-style designation, e.g. "KX-417".
fn system_name(rng: &mut Rng) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTVWXYZ";
    let a = LETTERS[(rng.next_u64() as usize) % LETTERS.len()] as char;
    let b = LETTERS[(rng.next_u64() as usize) % LETTERS.len()] as char;
    let num = 100 + (rng.next_u64() % 900);
    format!("{a}{b}-{num}")
}

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
    /// Resource deposits (static geology). Richer/more valuable toward the rim.
    #[serde(default)]
    pub deposits: Vec<Deposit>,
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
    /// §economy: the system's STRUCTURES (kind → built tier) — the ONE keyed map
    /// that replaces the flat per-building tier fields above. Those legacy fields
    /// stay as deprecated parse-only carriers and are folded in by
    /// [`StarSystem::fold_legacy_structures`] on load. Owner-only in the View.
    #[serde(default)]
    pub structures: BTreeMap<crate::build::StructureKind, u32>,
    /// §economy Part 2: colony POPULATION in millions. Grows toward Habitat
    /// capacity when well-supplied; NEVER decreases. Drives the Industrial /
    /// Infrastructure slot pools via `pop_tier`. Dormant (0) until Part 3 wires
    /// growth/consumption. Owner-only in the View.
    #[serde(default)]
    pub population: f64,
    /// §economy Part 3: standing PRODUCTION ASSIGNMENTS — workforce crews
    /// posted per structure kind (extraction, converters, Shipyard boost).
    /// Nothing produces unstaffed. Owner-only in the View; `default` empty is
    /// right for old snapshots (Part 7's migration seeds sensible defaults).
    #[serde(default)]
    pub assignments: BTreeMap<crate::build::StructureKind, crate::production::Assignment>,
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

    /// §economy: the built tier of a structure kind (0 = none). THE tier read —
    /// every consumer goes through this (the legacy flat fields are parse-only).
    pub fn tier(&self, kind: crate::build::StructureKind) -> u32 {
        self.structures.get(&kind).copied().unwrap_or(0)
    }

    /// §economy: set a structure's tier (0 removes the entry — the map stays
    /// minimal and deterministic).
    pub fn set_tier(&mut self, kind: crate::build::StructureKind, tier: u32) {
        if tier == 0 {
            self.structures.remove(&kind);
        } else {
            self.structures.insert(kind, tier);
        }
    }

    /// §economy: fold the LEGACY flat tier fields into `structures` (Extractor →
    /// MiningComplex, Refinery → FuelRefinery, the rest 1:1), zeroing the legacy
    /// carriers. Idempotent (zeroed fields fold nothing); called on snapshot load
    /// so a pre-economy world keeps every built tier. `defense_pool` and the
    /// combat semantics ride along untouched.
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
                let cur = self.tier(kind);
                self.set_tier(kind, cur + legacy);
            }
        }
    }

    /// §economy: the three DERIVED slot pools (never stored — the old
    /// `dev_slots()` philosophy; migration-free by construction).
    /// RESOURCE slots come from geology: one per deposit, clamped 1..=4.
    pub fn resource_slots(&self) -> u32 {
        (self.deposits.len() as u32).clamp(1, 4)
    }

    /// INDUSTRIAL slots come from population: 1 / 2 / 3 by `pop_tier`.
    pub fn industrial_slots(&self) -> u32 {
        1 + crate::build::pop_tier(self.population)
    }

    /// INFRASTRUCTURE slots: 2 / 3 / 3 by `pop_tier`.
    pub fn infrastructure_slots(&self) -> u32 {
        2 + (crate::build::pop_tier(self.population) >= 1) as u32
    }

    /// The slot budget of one pool.
    pub fn pool_slots(&self, pool: crate::build::SlotPool) -> u32 {
        match pool {
            crate::build::SlotPool::Resource => self.resource_slots(),
            crate::build::SlotPool::Industrial => self.industrial_slots(),
            crate::build::SlotPool::Infrastructure => self.infrastructure_slots(),
        }
    }

    /// Slots of one pool already CONSUMED — one per DISTINCT built structure
    /// (§economy Part 3): a slot is a FOOTPRINT, and tiers go DEEP on the same
    /// slot (that's the throughput ladder's whole job). Slots bound BREADTH —
    /// how many kinds a colony runs — never depth; otherwise a 2-deposit home
    /// could never upgrade its mine at all. In-progress jobs founding a NEW
    /// structure also hold a slot (the World's `pool_slots_pending`).
    pub fn pool_slots_built(&self, pool: crate::build::SlotPool) -> u32 {
        self.structures
            .iter()
            .filter(|(k, t)| k.slot_pool() == pool && **t >= 1)
            .count() as u32
    }

    /// §economy Part 3: total workforce crews POSTED across all assignments.
    pub fn workforce_posted(&self) -> u32 {
        self.assignments.values().map(|a| a.workers).sum()
    }

    /// §economy Part 3: the colony-wide STAFFING SHARE — when the posting
    /// exceeds what the population fields, every line dilutes by the same
    /// fraction (fair, legible, and deadlock-free: no line ever starves
    /// another outright). 1.0 when fully covered or nothing is posted.
    pub fn staffing_share(&self) -> f64 {
        let posted = self.workforce_posted();
        if posted == 0 {
            return 1.0;
        }
        (crate::colony::workforce_units(self.population) as f64 / posted as f64).min(1.0)
    }

    /// §economy Part 4: the EFFECTIVE specialists on every line this tick —
    /// per structure `(crew, matched)`: how many posted specialists actually
    /// work it (clamped by the resident pool, walked in deterministic BTreeMap
    /// order so a shrunken pool degrades the same way everywhere) and how many
    /// of those are AFFINE (drive the skill factor). Non-destructive: stored
    /// postings are untouched, so a returning specialist re-validates free.
    pub fn effective_specialists(
        &self,
    ) -> BTreeMap<crate::build::StructureKind, (u32, u32)> {
        let mut pool_left = self.specialists.clone();
        let mut out = BTreeMap::new();
        for (kind, asg) in &self.assignments {
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
            out.insert(*kind, (crew, matched));
        }
        out
    }

    /// §economy Part 3: the STAFFING factor of one structure's line —
    /// `(crew/tier) · share` where crew = generic workers + posted specialists
    /// (a specialist always works, affinity or not — never a penalty): a
    /// tier-N plant wants N crews for full throughput; posting fewer
    /// under-crews it, over-posting the colony dilutes everyone. 0.0 when
    /// nothing is posted (unstaffed = idle).
    pub fn staffing_factor(&self, kind: crate::build::StructureKind) -> f64 {
        let tier = self.tier(kind);
        if tier == 0 {
            return 0.0;
        }
        let workers = self.assignments.get(&kind).map(|a| a.workers).unwrap_or(0);
        let spec_crew = self.effective_specialists().get(&kind).map(|(c, _)| *c).unwrap_or(0);
        ((workers + spec_crew).min(tier) as f64 / tier as f64) * self.staffing_share()
    }

    /// §economy Part 4: the SKILL factor of one structure's line (1.0 bare,
    /// up to `SPECIALIST_SKILL_MULT` fully affine-staffed).
    pub fn skill_factor(&self, kind: crate::build::StructureKind) -> f64 {
        let (_, matched) = self.effective_specialists().get(&kind).copied().unwrap_or((0, 0));
        crate::production::skill_factor(matched, self.tier(kind))
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
        self.structures.values().filter(|t| **t >= 1).count() as u32
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
        systems.push(StarSystem {
            id,
            pos,
            name,
            deposits,
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
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
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
    StarSystem {
        id,
        pos,
        name: system_name(&mut rng),
        deposits,
        claim_cost,
        owner: None,
        claimed_at: None,
        // A standing Provisions buffer so the food ladder starts Well Supplied
        // while the seeded farm chain spins up (§economy Part 3 bootstrap).
        stockpile: [(Commodity::Provisions, crate::colony::HOME_PROVISIONS_SEED)]
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
        // HOME BOOTSTRAP (§buildings step 3 → §economy Part 3): a home is born
        // a WORKING developed colony — Shipyard (convoys turn one), the two
        // extraction structures its geology calls for, the Agroplex that turns
        // Biomass into food, and the Habitat housing 2.0M colonists. Every
        // slot pool is born exactly FULL (Resource 2/2, Industrial 1/1, Infra
        // 2/2): all expansion runs through population growth, by design.
        structures: [
            (crate::build::StructureKind::Shipyard, crate::build::HOME_SHIPYARD_TIER),
            (crate::build::StructureKind::Bioharvester, 1),
            (crate::build::StructureKind::MiningComplex, 1),
            (crate::build::StructureKind::Agroplex, 1),
            (crate::build::StructureKind::Habitat, 1),
        ]
        .into_iter()
        .collect(),
        population: crate::colony::HOME_FOUNDING_POP,
        // Pre-staffed: the food chain + the mine (the Shipyard boost crew is
        // the player's first staffing decision once population grows).
        assignments: [
            (crate::build::StructureKind::Bioharvester, crate::production::Assignment::crew(1)),
            (crate::build::StructureKind::MiningComplex, crate::production::Assignment::crew(1)),
            (crate::build::StructureKind::Agroplex, crate::production::Assignment::crew(1)),
        ]
        .into_iter()
        .collect(),
        specialists: BTreeMap::new(),
    }
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

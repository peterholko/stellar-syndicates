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
    /// Credit cost to claim this system (scales with its production value).
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
    #[serde(default)]
    pub extractor_tier: u32,
}

impl StarSystem {
    /// Whether this system can be claimed (no current owner).
    pub fn is_unclaimed(&self) -> bool {
        self.owner.is_none()
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

/// Commodities ordered cheapest → most valuable (by base price). Deposits are
/// drawn from this ladder biased by distance from the hub, so near-hub systems
/// hold common/cheap resources and the frontier holds the valuable ones (§4).
const VALUE_TIER: [Commodity; 5] = [
    Commodity::Provisions,
    Commodity::Ore,
    Commodity::Fuel,
    Commodity::Volatiles,
    Commodity::Alloys,
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
            extractor_tier: 0,
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
        let center = frontier * (VALUE_TIER.len() - 1) as f64;
        let idx = (center + rng.range(-1.1, 1.1)).round().clamp(0.0, 4.0) as usize;
        let resource = VALUE_TIER[idx];
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
    vec![
        Deposit {
            resource: Commodity::Provisions,
            richness: DEPOSIT_BASE_RICHNESS * rng.range(0.85, 1.15),
            reserves: None,
            accessibility: 0.1,
        },
        Deposit {
            resource: Commodity::Ore,
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
        stockpile: BTreeMap::new(),
        extractor_tier: 0,
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

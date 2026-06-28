//! Static solar-system geography (§4): the sun at the centre, a habitable planet
//! (the market), inner + outer asteroid belts, and the player starting asteroids
//! (mining stations). Generated deterministically from the seed.
//!
//! Bodies are placed at their AU distances (× [`AU`]), so light-delay falls out
//! physically from `c`: minutes near the inner system, hours at the Kuiper edge.
//! Every body carries real **orbital parameters** (semi-major axis, Kepler period,
//! phase) — but orbital MOTION is frozen for now (positions static); turning it on
//! later is a single config change. (`StarSystem` is the historical name for a
//! celestial body; it now models an asteroid or planet — see [`BodyKind`].)

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::config::{SimConfig, AU};
use crate::ids::{EntityId, PlayerId};
use crate::market::base_price;
use crate::math::Vec2;
use crate::rng::Rng;

/// What kind of celestial body this is. Asteroids are claimable mining targets;
/// the habitable planet is the market (not claimable). Modelled as a body so more
/// habitable planets can be generated later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BodyKind {
    #[default]
    Asteroid,
    Planet,
}

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

/// A celestial body (asteroid or habitable planet). `pos`, `name`, `body`,
/// `deposits`, `claim_cost`, and the orbital parameters are static geography
/// (known to all). `owner`/`claimed_at`/`stockpile` are *dynamic* state: a claim
/// is an event at `pos`/`claimed_at`, so its reveal to rivals respects light delay
/// (view filter), and a player's accumulated mining output is private to them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarSystem {
    pub id: EntityId,
    pub pos: Vec2,
    pub name: String,
    /// Asteroid (claimable, mines ore) vs habitable planet (the market).
    #[serde(default)]
    pub body: BodyKind,
    /// Resource deposits (static geology). Richer/more valuable toward the rim.
    #[serde(default)]
    pub deposits: Vec<Deposit>,
    /// Credit cost to claim/operate this asteroid (scales with its mining value).
    #[serde(default)]
    pub claim_cost: f64,
    /// Owning corporation, once claimed (light-gated to rivals by the view filter).
    #[serde(default)]
    pub owner: Option<PlayerId>,
    /// Sim time at which it was claimed (None while unowned) — the event time
    /// whose light gates the reveal of `owner` to other players.
    #[serde(default)]
    pub claimed_at: Option<f64>,
    /// Mining output accumulated here, awaiting a hauler to the habitable planet.
    #[serde(default)]
    pub stockpile: BTreeMap<Commodity, f64>,

    // --- Orbital parameters (Kepler). Motion is FROZEN for now (positions are
    //     static); these make turning orbits on later a single config change. ---
    /// Semi-major axis in AU (its orbital distance from the sun).
    #[serde(default)]
    pub semi_major_au: f64,
    /// Orbital period in years, from Kepler's third law (`T = a^1.5`) — outer
    /// bodies orbit slower.
    #[serde(default)]
    pub orbital_period_years: f64,
    /// Current orbital phase (radians). With motion off, the body sits here.
    #[serde(default)]
    pub orbital_phase: f64,
}

impl StarSystem {
    /// Whether this body can be claimed (an unowned asteroid; planets are the
    /// market and never claimable).
    pub fn is_claimable(&self) -> bool {
        self.body == BodyKind::Asteroid && self.owner.is_none()
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

/// AU bounds of the belts and the value gradient (tunable).
const PLANET_AU: (f64, f64) = (0.9, 1.1); // habitable planet, ~1 AU (the market)
const INNER_BELT_AU: (f64, f64) = (2.0, 3.0); // accessible, lower-value
const OUTER_BELT_AU: (f64, f64) = (30.0, 40.0); // Kuiper frontier, richer
/// AU distances used to normalise a body's frontier (value) factor in [0,1].
const FRONTIER_INNER_AU: f64 = 1.0;
const FRONTIER_OUTER_AU: f64 = 40.0;

/// A generated solar system: the bodies (planet + asteroids), the market location
/// (the habitable planet — `hub`), and the player starting-asteroid positions.
pub struct SolarSystem {
    pub bodies: Vec<StarSystem>,
    pub hub: Vec2,
    pub starts: Vec<Vec2>,
}

/// Generate the solar system deterministically from the seed (§4): one habitable
/// planet at ~1 AU (the market), an inner belt (~2–3 AU, lower value, ~minutes of
/// light), an outer/Kuiper belt (~30–40 AU, richer/higher value, ~hours of light),
/// and `max_players` spaced starting-asteroid mining stations. Deposit richness &
/// value rise toward the frontier — the best ore is out in the dangerous, fog-blind
/// dark. Counts come from the config; all distances are real-inspired, not exact.
pub fn generate_solar_system(rng: &mut Rng, config: &SimConfig, alloc: &mut dyn FnMut() -> EntityId) -> SolarSystem {
    let tau = std::f64::consts::TAU;
    let mut bodies = Vec::new();

    // 1. Habitable planet at ~1 AU — the market (hub). A body, so more habitable
    //    planets/markets can be generated later.
    let p_theta = rng.range(0.0, tau);
    let p_au = rng.range(PLANET_AU.0, PLANET_AU.1);
    let planet_pos = Vec2::from_polar(p_theta, p_au * AU);
    bodies.push(StarSystem {
        id: alloc(),
        pos: planet_pos,
        name: planet_name(rng),
        body: BodyKind::Planet,
        deposits: Vec::new(),
        claim_cost: 0.0,
        owner: None,
        claimed_at: None,
        stockpile: BTreeMap::new(),
        semi_major_au: p_au,
        orbital_period_years: p_au.powf(1.5),
        orbital_phase: p_theta,
    });

    // 2. Inner belt, then 3. outer belt.
    for _ in 0..config.inner_belt {
        let au = rng.range(INNER_BELT_AU.0, INNER_BELT_AU.1);
        let theta = rng.range(0.0, tau);
        bodies.push(asteroid(rng, alloc(), au, theta));
    }
    for _ in 0..config.outer_belt {
        let au = rng.range(OUTER_BELT_AU.0, OUTER_BELT_AU.1);
        let theta = rng.range(0.0, tau);
        bodies.push(asteroid(rng, alloc(), au, theta));
    }

    // 4. Player starting asteroids at ~start_orbit_au, EVENLY SPACED around the
    //    sun so players don't begin on top of each other.
    let n = config.max_players.max(1);
    let mut starts = Vec::new();
    for i in 0..n {
        let theta = tau * (i as f64) / (n as f64) + rng.range(-0.15, 0.15);
        let au = config.start_orbit_au + rng.range(-0.3, 0.3);
        let a = asteroid(rng, alloc(), au, theta);
        starts.push(a.pos);
        bodies.push(a);
    }

    SolarSystem { bodies, hub: planet_pos, starts }
}

/// A claimable asteroid at `au` / `theta`, with seeded deposits whose value rises
/// toward the frontier and frozen Kepler orbital parameters.
fn asteroid(rng: &mut Rng, id: EntityId, au: f64, theta: f64) -> StarSystem {
    let pos = Vec2::from_polar(theta, au * AU);
    let frontier = ((au - FRONTIER_INNER_AU) / (FRONTIER_OUTER_AU - FRONTIER_INNER_AU)).clamp(0.0, 1.0);
    let deposits = generate_deposits(rng, frontier);
    let claim_cost = claim_cost_for(&deposits);
    StarSystem {
        id,
        pos,
        name: asteroid_name(rng),
        body: BodyKind::Asteroid,
        deposits,
        claim_cost,
        owner: None,
        claimed_at: None,
        stockpile: BTreeMap::new(),
        semi_major_au: au,
        orbital_period_years: au.powf(1.5),
        orbital_phase: theta,
    }
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

/// A short minor-planet-style designation, e.g. "KX-417".
fn asteroid_name(rng: &mut Rng) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTVWXYZ";
    let a = LETTERS[(rng.next_u64() as usize) % LETTERS.len()] as char;
    let b = LETTERS[(rng.next_u64() as usize) % LETTERS.len()] as char;
    let num = 100 + (rng.next_u64() % 900);
    format!("{a}{b}-{num}")
}

/// A name for the habitable planet (the market world).
fn planet_name(rng: &mut Rng) -> String {
    const NAMES: &[&str] = &["Meridian", "Verdance", "Halcyon", "Aurelia", "Concord", "Elysia", "Solace", "Terranova"];
    NAMES[(rng.next_u64() as usize) % NAMES.len()].to_string()
}

//! Static galaxy geography: the hub, procedurally-placed star systems, and the
//! ring of home anchors (§4). Generated deterministically from the seed.
//!
//! "No discrete zones": systems are scattered continuously across one radial
//! space, the hub fixed at the centre, homes distributed around a ring as
//! bright spots. Resources/claims hang off systems in later milestones.

use serde::{Deserialize, Serialize};

use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::rng::Rng;

/// A procedurally-placed star system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarSystem {
    pub id: EntityId,
    pub pos: Vec2,
    pub name: String,
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

/// Generate `count` star systems uniformly over the galaxy disk (area-uniform
/// via the √u radius trick), keeping a clear margin around the hub.
pub fn generate_systems(rng: &mut Rng, radius: f64, count: u32, alloc: &mut dyn FnMut() -> EntityId) -> Vec<StarSystem> {
    let mut systems = Vec::with_capacity(count as usize);
    for _ in 0..count {
        // Area-uniform radius in [0.12R, 0.96R].
        let r = radius * (0.12 + 0.84 * rng.next_f64().sqrt());
        let theta = rng.range(0.0, std::f64::consts::TAU);
        let pos = Vec2::from_polar(theta, r);
        let id = alloc();
        let name = system_name(rng);
        systems.push(StarSystem { id, pos, name });
    }
    systems
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
        });
    }
    slots
}

/// A short catalogue-style designation, e.g. "KX-417".
fn system_name(rng: &mut Rng) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTVWXYZ";
    let a = LETTERS[(rng.next_u64() as usize) % LETTERS.len()] as char;
    let b = LETTERS[(rng.next_u64() as usize) % LETTERS.len()] as char;
    let num = 100 + (rng.next_u64() % 900);
    format!("{a}{b}-{num}")
}

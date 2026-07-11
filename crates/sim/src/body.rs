//! §bodies — PLANETS ARE REAL. A star system's planets and moons are
//! first-class sim entities now: structures are built ON a body, deposits
//! BELONG to a body, population lives on bodies (in their Habitats), and
//! production is staffed per body. What stays pooled at the SYSTEM
//! (deliberate, Tunable-flagged design choices): the stockpile (one logistics
//! node — convoys dock at systems), the workforce + specialist pools (labor
//! commutes freely inside a gravity well), the food state (pooled Provisions
//! vs the summed population), and deposit KNOWLEDGE (the explore ladder stays
//! system-scoped; R2 now reveals deposits with their body placement).
//!
//! ROSTER GENERATION is a faithful port of the client's old
//! `buildVisualSystem()` — same FNV-1a hash, same mulberry32 stream, same
//! draw ORDER (including draws whose values are cosmetic and discarded here),
//! so migrated systems keep the exact layouts players have already seen.
//! The client now consumes this roster from the wire and re-derives only
//! cosmetics (orbit radii, art variants, colors) from body ids.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::galaxy::Deposit;

/// A body's physical class — the sim-relevant collapse of the client's seven
/// visual kinds (desert/lava/barren are all `Rocky`; the client re-derives a
/// visual variant from the body id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    Rocky,
    Terrestrial,
    Ocean,
    Ice,
    GasGiant,
}

impl BodyKind {
    pub fn slug(self) -> &'static str {
        match self {
            BodyKind::Rocky => "rocky",
            BodyKind::Terrestrial => "terrestrial",
            BodyKind::Ocean => "ocean",
            BodyKind::Ice => "ice",
            BodyKind::GasGiant => "gas_giant",
        }
    }
}

/// One planet or moon. `id` is stable within its system (assigned in the
/// final inner→outer roster order, moons after all planets); the sim owns
/// names now ("Veles II", moons "Veles IIa").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub id: u32,
    pub name: String,
    pub kind: BodyKind,
    /// Moons point at their planet.
    #[serde(default)]
    pub parent: Option<u32>,
    pub habitable: bool,
    /// The deposits ON this body — extraction structures here require a
    /// matching one (real now, not a visual association).
    #[serde(default)]
    pub deposits: Vec<Deposit>,
    /// Structures built ON this body (kind → tier). The same kind may exist
    /// on several bodies of one system now.
    #[serde(default)]
    pub structures: BTreeMap<crate::build::StructureKind, u32>,
    /// Population living on THIS body (in its Habitats). Grows per body
    /// toward this body's habitat capacity; NEVER decreases (async-fair).
    #[serde(default)]
    pub population: f64,
    /// Production assignments on THIS body's structures. Staffing draws the
    /// SHARED system workforce pool (labor commutes inside the well).
    #[serde(default)]
    pub assignments: BTreeMap<crate::build::StructureKind, crate::production::Assignment>,
}

impl Body {
    pub fn tier(&self, kind: crate::build::StructureKind) -> u32 {
        self.structures.get(&kind).copied().unwrap_or(0)
    }

    pub fn set_tier(&mut self, kind: crate::build::StructureKind, tier: u32) {
        if tier == 0 {
            self.structures.remove(&kind);
        } else {
            self.structures.insert(kind, tier);
        }
    }

    /// Does this body carry a deposit the given EXTRACTION structure works?
    pub fn has_deposit_for(&self, kind: crate::build::StructureKind) -> bool {
        self.deposits
            .iter()
            .any(|d| crate::production::extraction_structure(d.resource) == Some(kind))
    }
}

// --- PER-BODY SLOT POOLS (derived, never stored — the same law as ever) --------

/// Per-BODY population tiers — scaled down from the system thresholds (a body
/// develops on its own curve; the system's industrial weight is the sum).
/// Tunable.
pub const BODY_POP_DEVELOPED: f64 = 1.5;
pub const BODY_POP_MAJOR: f64 = 4.0;

pub fn body_pop_tier(population: f64) -> u32 {
    if population >= BODY_POP_MAJOR {
        2
    } else if population >= BODY_POP_DEVELOPED {
        1
    } else {
        0
    }
}

impl Body {
    /// RESOURCE slots: one per deposit on this body, at most 4. A body with no
    /// deposits hosts no extraction (min with the deposit count — unlike the
    /// old system pool there is no floor of 1: bare rocks stay bare).
    pub fn resource_slots(&self) -> u32 {
        (self.deposits.len() as u32).min(4)
    }

    /// INDUSTRIAL slots: gas giants host none (nowhere to stand); every other
    /// body starts with 1 and grows with ITS population tier.
    pub fn industrial_slots(&self) -> u32 {
        let base = if self.kind == BodyKind::GasGiant { 0 } else { 1 };
        base + body_pop_tier(self.population)
    }

    /// INFRASTRUCTURE slots: 1, +1 if habitable, +1 once developed.
    pub fn infrastructure_slots(&self) -> u32 {
        1 + self.habitable as u32 + (body_pop_tier(self.population) >= 1) as u32
    }

    pub fn pool_slots(&self, pool: crate::build::SlotPool) -> u32 {
        match pool {
            crate::build::SlotPool::Resource => self.resource_slots(),
            crate::build::SlotPool::Industrial => self.industrial_slots(),
            crate::build::SlotPool::Infrastructure => self.infrastructure_slots(),
        }
    }

    /// Slots of one pool consumed here — one per DISTINCT built structure
    /// (breadth, not depth: tiers deepen in place).
    pub fn pool_slots_built(&self, pool: crate::build::SlotPool) -> u32 {
        self.structures
            .iter()
            .filter(|(k, t)| k.slot_pool() == pool && **t >= 1)
            .count() as u32
    }
}

// --- ROSTER GENERATION (the ported client algorithm) ----------------------------

/// FNV-1a over the id string — bit-identical to the client's `hashId`.
fn hash_id(id: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// mulberry32 — bit-identical to the client's stream (i32 wrapping adds,
/// `Math.imul` semantics, `>>> 0` reinterpretation, `/ 2^32`).
struct Mulberry32(u32);
impl Mulberry32 {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
    fn pick<'a, T>(&mut self, arr: &'a [T]) -> &'a T {
        &arr[(self.next() * arr.len() as f64).floor() as usize % arr.len()]
    }
}

/// The client's seven VISUAL kinds — used internally so the deposit-affinity
/// picks and every rng draw match the old generator exactly; collapsed to
/// [`BodyKind`] for storage.
#[derive(Clone, Copy, PartialEq)]
enum VisualKind {
    Terrestrial,
    Desert,
    Ocean,
    Ice,
    GasGiant,
    Lava,
    Barren,
}

impl VisualKind {
    fn collapse(self) -> BodyKind {
        match self {
            VisualKind::Terrestrial => BodyKind::Terrestrial,
            VisualKind::Ocean => BodyKind::Ocean,
            VisualKind::Ice => BodyKind::Ice,
            VisualKind::GasGiant => BodyKind::GasGiant,
            VisualKind::Desert | VisualKind::Lava | VisualKind::Barren => BodyKind::Rocky,
        }
    }
}

/// Deposit → the kinds of world it belongs on (the client's DEP_KINDS,
/// verbatim — this affinity is mandatory: volatiles→ice/gas, biomass→
/// habitable/ocean, minerals→rocky).
fn dep_kinds(c: Commodity) -> &'static [VisualKind] {
    use VisualKind as V;
    match c {
        Commodity::MetallicOre => &[V::Barren, V::Desert, V::Terrestrial],
        Commodity::RareElements => &[V::Lava, V::Barren],
        Commodity::Silicates => &[V::Desert, V::Barren],
        Commodity::Volatiles => &[V::Ice],
        Commodity::Biomass => &[V::Ocean, V::Terrestrial],
        Commodity::Alloys => &[V::Barren, V::Desert],
        Commodity::Electronics => &[V::Barren],
        Commodity::Polymers => &[V::Barren],
        Commodity::Fuel => &[V::GasGiant],
        Commodity::Provisions => &[V::Ocean, V::Terrestrial],
        Commodity::Machinery => &[V::Barren],
        Commodity::Armaments => &[V::Barren],
    }
}

const FILLER_KINDS: [VisualKind; 7] = [
    VisualKind::Terrestrial,
    VisualKind::Desert,
    VisualKind::Barren,
    VisualKind::Lava,
    VisualKind::Ice,
    VisualKind::GasGiant,
    VisualKind::Ocean,
];

const ROMAN: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];

/// Generate the authoritative body roster for a system — the ported client
/// algorithm, drawing the SAME rng sequence in the SAME order (cosmetic draws
/// included and discarded) so pre-migration layouts survive byte-for-byte:
/// kinds, habitability, deposit placement, moon structure, order, and names.
/// Deposits are MOVED onto the bodies they land on.
pub fn generate_bodies(system_id: &str, system_name: &str, deposits: &[Deposit]) -> Vec<Body> {
    struct Moon {
        deposits: Vec<Deposit>,
    }
    struct Planet {
        kind: VisualKind,
        habitable: bool,
        deposits: Vec<Deposit>,
        moons: Vec<Moon>,
        orbit: f64,
    }
    let mut rng = Mulberry32(hash_id(system_id) ^ 0x5eed1a7);
    let mut planets: Vec<Planet> = Vec::new();

    // 1. Each known deposit gets a home body; volatiles prefer an icy MOON of
    //    a gas giant, else an ice world.
    let mut gas_giant: Option<usize> = None;
    let mut volatiles: Vec<Deposit> = Vec::new();
    for d in deposits {
        if d.resource == Commodity::Volatiles {
            volatiles.push(d.clone());
            continue;
        }
        let kind = *rng.pick(dep_kinds(d.resource));
        let habitable = d.resource == Commodity::Biomass || d.resource == Commodity::Provisions;
        planets.push(Planet { kind, habitable, deposits: vec![d.clone()], moons: Vec::new(), orbit: 0.0 });
        if kind == VisualKind::GasGiant {
            gas_giant = Some(planets.len() - 1);
        }
    }
    for d in volatiles {
        if let Some(gi) = gas_giant {
            // The client drew orbitRadius, radius, angle for the moon — three
            // draws we must consume to stay in step (values are cosmetic).
            let _ = rng.next();
            let _ = rng.next();
            let _ = rng.next();
            planets[gi].moons.push(Moon { deposits: vec![d] });
        } else {
            planets.push(Planet { kind: VisualKind::Ice, habitable: false, deposits: vec![d], moons: Vec::new(), orbit: 0.0 });
        }
    }

    // 2. Fill to 3–8 decorative planets.
    let target = (deposits.len() + 1 + (rng.next() * 3.0).floor() as usize).clamp(3, 8);
    while planets.len() < target {
        let kind = *rng.pick(&FILLER_KINDS);
        planets.push(Planet { kind, habitable: false, deposits: Vec::new(), moons: Vec::new(), orbit: 0.0 });
    }

    // 3. Deterministic Fisher–Yates shuffle, then the orbit draws (orbit is
    //    kept ONLY to reproduce the final sort; radius/angle/moon-position
    //    draws are consumed and discarded).
    let len = planets.len();
    for i in (1..len).rev() {
        let j = (rng.next() * (i + 1) as f64).floor() as usize;
        planets.swap(i, j);
    }
    let n = planets.len();
    for (i, p) in planets.iter_mut().enumerate() {
        let base = 0.2 + 0.75 * if n == 1 { 0.5 } else { i as f64 / (n - 1) as f64 };
        p.orbit = (base + (rng.next() - 0.5) * 0.03).min(0.96);
        let _ = rng.next(); // angle
        let _ = rng.next(); // radiusForKind (one draw for every kind)
        let moon_count = if p.kind == VisualKind::GasGiant {
            1 + (rng.next() * 2.0).floor() as usize
        } else if rng.next() < 0.22 {
            1
        } else {
            0
        };
        for _ in 0..moon_count {
            let _ = rng.next(); // moon orbitRadius
            let _ = rng.next(); // moon radius
            let _ = rng.next(); // moon angle
            p.moons.push(Moon { deposits: Vec::new() });
        }
    }
    planets.sort_by(|a, b| a.orbit.partial_cmp(&b.orbit).expect("orbits are finite"));

    // 4. Flatten to Bodies: planets in inner→outer order get ids 0..n, then
    //    moons (walk order: each planet's moons after it) continue the ids.
    let mut bodies: Vec<Body> = Vec::new();
    let mut moon_queue: Vec<(u32, usize, Vec<Deposit>)> = Vec::new(); // (parent id, letter idx, deposits)
    for (i, p) in planets.iter_mut().enumerate() {
        let id = i as u32;
        let name = format!(
            "{} {}",
            system_name,
            ROMAN.get(i).copied().map(str::to_string).unwrap_or_else(|| (i + 1).to_string())
        );
        bodies.push(Body {
            id,
            name,
            kind: p.kind.collapse(),
            parent: None,
            habitable: p.habitable,
            deposits: std::mem::take(&mut p.deposits),
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
        });
        for (k, m) in p.moons.iter_mut().enumerate() {
            moon_queue.push((id, k, std::mem::take(&mut m.deposits)));
        }
    }
    let mut next_id = planets.len() as u32;
    for (parent, k, deps) in moon_queue {
        let pname = bodies[parent as usize].name.clone();
        bodies.push(Body {
            id: next_id,
            name: format!("{}{}", pname, (b'a' + (k as u8 % 26)) as char),
            kind: BodyKind::Ice, // the walk forced moons to ice — kept
            parent: Some(parent),
            habitable: false,
            deposits: deps,
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
        });
        next_id += 1;
    }
    bodies
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep(c: Commodity, r: f64) -> Deposit {
        Deposit { resource: c, richness: r, reserves: None, accessibility: 0.1 }
    }

    #[test]
    fn roster_generation_is_deterministic() {
        let deps = vec![dep(Commodity::Biomass, 0.4), dep(Commodity::MetallicOre, 0.35), dep(Commodity::Volatiles, 0.3)];
        let a = generate_bodies("42", "Veles", &deps);
        let b = generate_bodies("42", "Veles", &deps);
        assert_eq!(serde_json::to_string(&a).unwrap(), serde_json::to_string(&b).unwrap());
        assert!(a.len() >= 3, "filled to at least 3 planets");
    }

    #[test]
    fn deposits_land_on_affine_bodies_and_all_survive() {
        let deps = vec![
            dep(Commodity::Biomass, 0.4),
            dep(Commodity::MetallicOre, 0.35),
            dep(Commodity::Volatiles, 0.3),
            dep(Commodity::RareElements, 0.5),
            dep(Commodity::Silicates, 0.2),
        ];
        let bodies = generate_bodies("7", "Krsnik", &deps);
        let placed: usize = bodies.iter().map(|b| b.deposits.len()).sum();
        assert_eq!(placed, deps.len(), "every deposit lands on exactly one body");
        for b in &bodies {
            for d in &b.deposits {
                match d.resource {
                    Commodity::Volatiles => assert!(matches!(b.kind, BodyKind::Ice | BodyKind::GasGiant) || b.parent.is_some(), "volatiles on ice/gas/moon"),
                    Commodity::Biomass => assert!(b.habitable, "biomass bodies are habitable"),
                    Commodity::MetallicOre | Commodity::RareElements | Commodity::Silicates =>
                        assert!(matches!(b.kind, BodyKind::Rocky | BodyKind::Terrestrial), "minerals on rocky worlds"),
                    _ => {}
                }
            }
        }
        // Names: planets carry roman numerals in id order; moons letter off
        // their parent.
        let planets: Vec<&Body> = bodies.iter().filter(|b| b.parent.is_none()).collect();
        assert!(planets[0].name.ends_with(" I"));
        for m in bodies.iter().filter(|b| b.parent.is_some()) {
            let p = &bodies[m.parent.unwrap() as usize];
            assert!(m.name.starts_with(&p.name), "moon named off its parent");
        }
    }

    #[test]
    fn slot_pools_derive_per_body() {
        let mut b = Body {
            id: 0, name: "X I".into(), kind: BodyKind::Rocky, parent: None, habitable: false,
            deposits: vec![dep(Commodity::MetallicOre, 0.4)],
            structures: BTreeMap::new(), population: 0.0, assignments: BTreeMap::new(),
        };
        assert_eq!(b.resource_slots(), 1);
        assert_eq!(b.industrial_slots(), 1);
        assert_eq!(b.infrastructure_slots(), 1, "not habitable, undeveloped");
        b.population = BODY_POP_DEVELOPED;
        assert_eq!(b.industrial_slots(), 2);
        assert_eq!(b.infrastructure_slots(), 2);
        b.kind = BodyKind::GasGiant;
        assert_eq!(b.industrial_slots(), 1, "gas giants have no base industrial slot");
        b.deposits.clear();
        assert_eq!(b.resource_slots(), 0, "a bare rock hosts no extraction");
    }
}

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

/// A world's physical scale. Size is independent of environment and geology:
/// a huge barren planet can be mineral-poor, while a tiny airless moon can be
/// the richest rock in the sector. It governs settlement capacity and the
/// amount of industrial ground available, not deposit composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodySize {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
}

impl Default for BodySize {
    fn default() -> Self {
        Self::Medium
    }
}

impl BodySize {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Huge => "huge",
        }
    }

    pub fn habitat_capacity_mult(self) -> f64 {
        match self {
            Self::Tiny => 0.60,
            Self::Small => 0.80,
            Self::Medium => 1.00,
            Self::Large => 1.25,
            Self::Huge => 1.50,
        }
    }

    fn industrial_base(self) -> u32 {
        match self {
            Self::Tiny => 1,
            Self::Small | Self::Medium => 2,
            Self::Large => 3,
            Self::Huge => 4,
        }
    }
}

/// The cost of making a world into a place where people can thrive. This is
/// deliberately independent from mineral wealth: atmosphere is a settlement
/// question, never an ore-quality proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Gaia,
    Terran,
    Marginal,
    Hostile,
    Uninhabitable,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Marginal
    }
}

impl Environment {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Gaia => "gaia",
            Self::Terran => "terran",
            Self::Marginal => "marginal",
            Self::Hostile => "hostile",
            Self::Uninhabitable => "uninhabitable",
        }
    }

    pub fn naturally_habitable(self) -> bool {
        matches!(self, Self::Gaia | Self::Terran)
    }

    pub fn habitat_capacity_mult(self) -> f64 {
        match self {
            Self::Gaia => 1.50,
            Self::Terran => 1.25,
            Self::Marginal => 1.00,
            Self::Hostile => 0.75,
            Self::Uninhabitable => 0.50,
        }
    }

    pub fn growth_mult(self) -> f64 {
        match self {
            Self::Gaia => 1.50,
            Self::Terran => 1.15,
            Self::Marginal => 0.85,
            Self::Hostile => 0.55,
            Self::Uninhabitable => 0.25,
        }
    }

    pub fn provisions_mult(self) -> f64 {
        match self {
            Self::Gaia => 0.80,
            Self::Terran => 1.00,
            Self::Marginal => 1.10,
            Self::Hostile => 1.25,
            Self::Uninhabitable => 1.40,
        }
    }

    pub fn construction_time_mult(self) -> f64 {
        match self {
            Self::Gaia => 0.90,
            Self::Terran => 1.00,
            Self::Marginal => 1.10,
            Self::Hostile => 1.25,
            Self::Uninhabitable => 1.40,
        }
    }
}

/// Mineral abundance, independent of atmosphere and body kind. This is the
/// Master-of-Orion-style expansion axis: an airless moon may legitimately be
/// Ultra Rich and worth supplying from a distant garden world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Geology {
    UltraPoor,
    Poor,
    Average,
    Rich,
    UltraRich,
}

impl Default for Geology {
    fn default() -> Self {
        Self::Average
    }
}

impl Geology {
    pub fn slug(self) -> &'static str {
        match self {
            Self::UltraPoor => "ultra_poor",
            Self::Poor => "poor",
            Self::Average => "average",
            Self::Rich => "rich",
            Self::UltraRich => "ultra_rich",
        }
    }

    pub fn mineral_extraction_mult(self) -> f64 {
        match self {
            Self::UltraPoor => 0.50,
            Self::Poor => 0.75,
            Self::Average => 1.00,
            Self::Rich => 1.50,
            Self::UltraRich => 2.25,
        }
    }
}

/// A surveyed, body-level industrial opportunity. Specials alter an existing
/// leg of the twelve-good web rather than minting a parallel commodity system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodySpecial {
    LowGravity,
    VolcanicMantle,
    HydrocarbonSeas,
    CrystallineCrust,
    FertileBiosphere,
    PrecursorRuins,
}

impl BodySpecial {
    pub fn slug(self) -> &'static str {
        match self {
            Self::LowGravity => "low_gravity",
            Self::VolcanicMantle => "volcanic_mantle",
            Self::HydrocarbonSeas => "hydrocarbon_seas",
            Self::CrystallineCrust => "crystalline_crust",
            Self::FertileBiosphere => "fertile_biosphere",
            Self::PrecursorRuins => "precursor_ruins",
        }
    }

    pub fn effect(self) -> &'static str {
        match self {
            Self::LowGravity => "Ship construction time −20%",
            Self::VolcanicMantle => {
                "Metallic Ore and Rare Elements extraction ×1.20; Smelter yield ×1.20"
            }
            Self::HydrocarbonSeas => "Volatiles extraction ×1.35; Fuel and Polymer yield ×1.20",
            Self::CrystallineCrust => "Silicates and Rare Elements extraction ×1.35",
            Self::FertileBiosphere => "Biomass extraction ×1.35; Provisions yield ×1.20",
            Self::PrecursorRuins => "Electronics yield ×1.25",
        }
    }
}

/// The independent identity of one body. `version == 0` is a pre-feature save;
/// [`Body::ensure_profile`] deterministically fills it from stable ids without
/// moving deposits, structures, assignments, or population.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlanetaryProfile {
    #[serde(default)]
    pub version: u8,
    #[serde(default)]
    pub size: BodySize,
    #[serde(default)]
    pub environment: Environment,
    #[serde(default)]
    pub geology: Geology,
    #[serde(default)]
    pub special: Option<BodySpecial>,
}

impl Default for PlanetaryProfile {
    fn default() -> Self {
        Self {
            version: 0,
            size: BodySize::Medium,
            environment: Environment::Marginal,
            geology: Geology::Average,
            special: None,
        }
    }
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
/// names now — planets by Roman orbital position ("Veles II"), moons with a
/// hyphenated letter ("Veles II-a").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    pub id: u32,
    pub name: String,
    pub kind: BodyKind,
    /// Moons point at their planet.
    #[serde(default)]
    pub parent: Option<u32>,
    pub habitable: bool,
    /// Size / environment are public astronomy; geology + the special ride the
    /// survey ladder. Kept together so old snapshots need one neutral default.
    #[serde(default)]
    pub profile: PlanetaryProfile,
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

    /// Fill a pre-profile body from an isolated, stable stream. Deposit type is
    /// never an input to size/environment/geology, so a planet's habitability
    /// cannot silently predetermine its mineral value. Specials alone inspect
    /// deposits because their promise must apply to something the body can use.
    pub fn ensure_profile(&mut self, system_id: &str) {
        if self.profile.version != 0 {
            return;
        }
        let mut rng = Mulberry32(hash_id(&format!("{system_id}:{}", self.id)) ^ 0x504c_414e);
        let size_roll = rng.next();
        let size = if self.parent.is_some() {
            if size_roll < 0.72 {
                BodySize::Tiny
            } else {
                BodySize::Small
            }
        } else if self.kind == BodyKind::GasGiant {
            if size_roll < 0.35 {
                BodySize::Large
            } else {
                BodySize::Huge
            }
        } else if size_roll < 0.12 {
            BodySize::Tiny
        } else if size_roll < 0.37 {
            BodySize::Small
        } else if size_roll < 0.72 {
            BodySize::Medium
        } else if size_roll < 0.92 {
            BodySize::Large
        } else {
            BodySize::Huge
        };

        let e = rng.next();
        let environment = match self.kind {
            BodyKind::Terrestrial => {
                if e < 0.07 {
                    Environment::Gaia
                } else if e < 0.62 {
                    Environment::Terran
                } else if e < 0.86 {
                    Environment::Marginal
                } else if e < 0.96 {
                    Environment::Hostile
                } else {
                    Environment::Uninhabitable
                }
            }
            BodyKind::Ocean => {
                if e < 0.12 {
                    Environment::Gaia
                } else if e < 0.75 {
                    Environment::Terran
                } else if e < 0.92 {
                    Environment::Marginal
                } else {
                    Environment::Hostile
                }
            }
            BodyKind::Rocky => {
                if e < 0.08 {
                    Environment::Marginal
                } else if e < 0.48 {
                    Environment::Hostile
                } else {
                    Environment::Uninhabitable
                }
            }
            BodyKind::Ice => {
                if e < 0.04 {
                    Environment::Marginal
                } else if e < 0.34 {
                    Environment::Hostile
                } else {
                    Environment::Uninhabitable
                }
            }
            BodyKind::GasGiant => Environment::Uninhabitable,
        };

        // An intentionally independent roll: atmosphere never constrains ore.
        let g = rng.next();
        // Good ground is supposed to be a DISCOVERY, not the default texture:
        // 8% Ultra Poor · 20% Poor · 65.5% Average · 5% Rich · 1.5% Ultra Rich.
        // The nearby onboarding mine is promoted separately, so organic surveys
        // can hold the 10–15% build-order-changing target across whole systems.
        let geology = if g < 0.08 {
            Geology::UltraPoor
        } else if g < 0.28 {
            Geology::Poor
        } else if g < 0.935 {
            Geology::Average
        } else if g < 0.985 {
            Geology::Rich
        } else {
            Geology::UltraRich
        };

        let special = if rng.next() < 0.20 {
            let mut eligible = Vec::new();
            // Low-density Large/Huge worlds are rare but valuable because they
            // combine the ship-time bonus with enough industrial room to form a
            // genuine shipbuilding colony instead of a one-slot curiosity.
            if self.kind != BodyKind::GasGiant {
                eligible.push(BodySpecial::LowGravity);
            }
            if self.kind == BodyKind::Rocky {
                eligible.push(BodySpecial::VolcanicMantle);
            }
            if matches!(
                self.kind,
                BodyKind::Ocean | BodyKind::Ice | BodyKind::GasGiant
            ) || self
                .deposits
                .iter()
                .any(|d| d.resource == Commodity::Volatiles)
            {
                eligible.push(BodySpecial::HydrocarbonSeas);
            }
            if self.kind != BodyKind::GasGiant {
                eligible.push(BodySpecial::CrystallineCrust);
                eligible.push(BodySpecial::PrecursorRuins);
            }
            if environment.naturally_habitable()
                && self
                    .deposits
                    .iter()
                    .any(|d| d.resource == Commodity::Biomass)
            {
                eligible.push(BodySpecial::FertileBiosphere);
            }
            (!eligible.is_empty()).then(|| *rng.pick(&eligible))
        } else {
            None
        };

        self.profile = PlanetaryProfile {
            version: 1,
            size,
            environment,
            geology,
            special,
        };
        self.habitable = environment.naturally_habitable();
    }

    pub fn habitat_capacity_mult(&self) -> f64 {
        self.profile.size.habitat_capacity_mult() * self.profile.environment.habitat_capacity_mult()
    }

    pub fn population_growth_mult(&self) -> f64 {
        self.profile.environment.growth_mult()
    }

    pub fn provisions_mult(&self) -> f64 {
        self.profile.environment.provisions_mult()
    }

    pub fn construction_time_mult(&self) -> f64 {
        self.profile.environment.construction_time_mult()
    }

    pub fn extraction_mult(&self, resource: Commodity) -> f64 {
        let mineral = matches!(
            resource,
            Commodity::MetallicOre | Commodity::Silicates | Commodity::RareElements
        );
        let geology = if mineral {
            self.profile.geology.mineral_extraction_mult()
        } else {
            1.0
        };
        let special = match (self.profile.special, resource) {
            (
                Some(BodySpecial::VolcanicMantle),
                Commodity::MetallicOre | Commodity::RareElements,
            ) => 1.20,
            (Some(BodySpecial::HydrocarbonSeas), Commodity::Volatiles) => 1.35,
            (
                Some(BodySpecial::CrystallineCrust),
                Commodity::Silicates | Commodity::RareElements,
            ) => 1.35,
            (Some(BodySpecial::FertileBiosphere), Commodity::Biomass) => 1.35,
            _ => 1.0,
        };
        geology * special
    }

    pub fn converter_yield_mult(&self, kind: crate::build::StructureKind) -> f64 {
        use crate::build::StructureKind as K;
        match (self.profile.special, kind) {
            (Some(BodySpecial::VolcanicMantle), K::Smelter) => 1.20,
            (Some(BodySpecial::HydrocarbonSeas), K::FuelRefinery | K::ChemicalWorks) => 1.20,
            (Some(BodySpecial::FertileBiosphere), K::Agroplex) => 1.20,
            (Some(BodySpecial::PrecursorRuins), K::ElectronicsFabricator) => 1.25,
            _ => 1.0,
        }
    }

    pub fn ship_build_time_mult(&self) -> f64 {
        if self.profile.special == Some(BodySpecial::LowGravity) {
            0.80
        } else {
            1.0
        }
    }
}

// --- PER-BODY SLOT POOLS (derived, never stored — the same law as ever) --------

/// Per-BODY population tiers — scaled down from the system thresholds (a body
/// develops on its own curve; the system's industrial weight is the sum).
/// Tunable.
pub const BODY_POP_DEVELOPED: f64 = 0.010;
pub const BODY_POP_MAJOR: f64 = 0.050;

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
    /// body starts with 2 and grows with ITS population tier — so even a fresh
    /// colony has industrial breathing room (2) and a major world runs 4.
    pub fn industrial_slots(&self) -> u32 {
        self.base_industrial_slots() + body_pop_tier(self.population)
    }

    pub fn base_industrial_slots(&self) -> u32 {
        if self.kind == BodyKind::GasGiant {
            0
        } else {
            self.profile.size.industrial_base()
        }
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

/// The display numeral for a planet at orbital position `i` (0-based, inner→outer):
/// Roman I, II, III…, falling back to Arabic past X for a rare deep system.
pub fn planet_numeral(i: usize) -> String {
    const ROMAN: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    ROMAN
        .get(i)
        .map(|s| s.to_string())
        .unwrap_or_else(|| (i + 1).to_string())
}

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
        planets.push(Planet {
            kind,
            habitable,
            deposits: vec![d.clone()],
            moons: Vec::new(),
            orbit: 0.0,
        });
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
            planets.push(Planet {
                kind: VisualKind::Ice,
                habitable: false,
                deposits: vec![d],
                moons: Vec::new(),
                orbit: 0.0,
            });
        }
    }

    // 2. Fill to 3–8 decorative planets.
    let target = (deposits.len() + 1 + (rng.next() * 3.0).floor() as usize).clamp(3, 8);
    while planets.len() < target {
        let kind = *rng.pick(&FILLER_KINDS);
        planets.push(Planet {
            kind,
            habitable: false,
            deposits: Vec::new(),
            moons: Vec::new(),
            orbit: 0.0,
        });
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
        let base = 0.2
            + 0.75
                * if n == 1 {
                    0.5
                } else {
                    i as f64 / (n - 1) as f64
                };
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
            p.moons.push(Moon {
                deposits: Vec::new(),
            });
        }
    }
    planets.sort_by(|a, b| a.orbit.partial_cmp(&b.orbit).expect("orbits are finite"));

    // 4. Flatten to Bodies: planets in inner→outer order get ids 0..n, then
    //    moons (walk order: each planet's moons after it) continue the ids.
    let mut bodies: Vec<Body> = Vec::new();
    let mut moon_queue: Vec<(u32, usize, Vec<Deposit>)> = Vec::new(); // (parent id, letter idx, deposits)
    for (i, p) in planets.iter_mut().enumerate() {
        let id = i as u32;
        // Planets take ROMAN numerals by orbital position, inner→outer (the sort
        // above): "Veles I", "Veles II", "Veles III".
        let name = format!("{} {}", system_name, planet_numeral(i));
        bodies.push(Body {
            id,
            name,
            kind: p.kind.collapse(),
            parent: None,
            habitable: p.habitable,
            profile: PlanetaryProfile::default(),
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
            // Moons keep the letter suffix but gain a hyphen: "Veles 2-a", "Veles 2-b".
            name: format!("{}-{}", pname, (b'a' + (k as u8 % 26)) as char),
            kind: BodyKind::Ice, // the walk forced moons to ice — kept
            parent: Some(parent),
            habitable: false,
            profile: PlanetaryProfile::default(),
            deposits: deps,
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
        });
        next_id += 1;
    }
    for body in &mut bodies {
        body.ensure_profile(system_id);
    }
    bodies
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dep(c: Commodity, r: f64) -> Deposit {
        Deposit {
            resource: c,
            richness: r,
            reserves: None,
            accessibility: 0.1,
        }
    }

    #[test]
    fn roster_generation_is_deterministic() {
        let deps = vec![
            dep(Commodity::Biomass, 0.4),
            dep(Commodity::MetallicOre, 0.35),
            dep(Commodity::Volatiles, 0.3),
        ];
        let a = generate_bodies("42", "Veles", &deps);
        let b = generate_bodies("42", "Veles", &deps);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
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
        assert_eq!(
            placed,
            deps.len(),
            "every deposit lands on exactly one body"
        );
        for b in &bodies {
            for d in &b.deposits {
                match d.resource {
                    Commodity::Volatiles => assert!(
                        matches!(b.kind, BodyKind::Ice | BodyKind::GasGiant) || b.parent.is_some(),
                        "volatiles on ice/gas/moon"
                    ),
                    Commodity::Biomass => assert!(
                        matches!(b.kind, BodyKind::Terrestrial | BodyKind::Ocean),
                        "biomass has a terrestrial/ocean home even when its atmosphere is hostile to us"
                    ),
                    Commodity::MetallicOre | Commodity::RareElements | Commodity::Silicates => {
                        assert!(
                            matches!(b.kind, BodyKind::Rocky | BodyKind::Terrestrial),
                            "minerals on rocky worlds"
                        )
                    }
                    _ => {}
                }
            }
        }
        // Names: planets carry ROMAN numerals by orbital position (inner = I);
        // moons take a hyphenated letter off their parent ("… II-a").
        let planets: Vec<&Body> = bodies.iter().filter(|b| b.parent.is_none()).collect();
        assert!(
            planets[0].name.ends_with(" I"),
            "inner planet is I, got {}",
            planets[0].name
        );
        for m in bodies.iter().filter(|b| b.parent.is_some()) {
            let p = &bodies[m.parent.unwrap() as usize];
            assert!(
                m.name.starts_with(&format!("{}-", p.name)),
                "moon named off its parent with a hyphen: {}",
                m.name
            );
        }
    }

    #[test]
    fn slot_pools_derive_per_body() {
        let mut b = Body {
            id: 0,
            name: "X I".into(),
            kind: BodyKind::Rocky,
            parent: None,
            habitable: false,
            profile: PlanetaryProfile::default(),
            deposits: vec![dep(Commodity::MetallicOre, 0.4)],
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
        };
        assert_eq!(b.resource_slots(), 1);
        assert_eq!(
            b.industrial_slots(),
            2,
            "non-gas base is 2 (industrial headroom)"
        );
        assert_eq!(b.infrastructure_slots(), 1, "not habitable, undeveloped");
        b.population = BODY_POP_DEVELOPED;
        assert_eq!(b.industrial_slots(), 3, "base 2 + one pop tier");
        assert_eq!(b.infrastructure_slots(), 2);
        b.kind = BodyKind::GasGiant;
        assert_eq!(
            b.industrial_slots(),
            1,
            "gas giants have no base industrial slot (0 + one pop tier)"
        );
        b.deposits.clear();
        assert_eq!(b.resource_slots(), 0, "a bare rock hosts no extraction");
    }

    #[test]
    fn industrial_slots_have_headroom() {
        // §industrial-headroom: base went 1 → 2 for non-gas bodies, so capacity
        // only ever GROWS — a fresh colony already runs two industries, a major
        // world runs four, and gas giants are unchanged (still 0 + pop tier).
        let mut b = Body {
            id: 0,
            name: "Head I".into(),
            kind: BodyKind::Terrestrial,
            parent: None,
            habitable: true,
            profile: PlanetaryProfile::default(),
            deposits: vec![],
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
        };
        assert_eq!(
            b.industrial_slots(),
            2,
            "a fresh non-gas colony starts with 2 industrial slots"
        );
        assert!(b.industrial_slots() >= 2);
        b.population = BODY_POP_DEVELOPED; // pop tier 1
        assert_eq!(b.industrial_slots(), 3);
        b.population = BODY_POP_MAJOR; // pop tier 2 — max
        assert_eq!(
            b.industrial_slots(),
            4,
            "a max-pop world runs four industries"
        );
        // Gas giants keep a 0 base: nowhere to stand, only pop lifts them.
        b.kind = BodyKind::GasGiant;
        assert_eq!(b.industrial_slots(), 2, "gas giant = 0 base + 2 pop tiers");
        b.population = 0.0;
        assert_eq!(
            b.industrial_slots(),
            0,
            "a fresh gas giant hosts no industry"
        );
    }

    #[test]
    fn atmosphere_never_determines_mineral_grade() {
        let mut barren = Body {
            id: 0,
            name: "Test I".into(),
            kind: BodyKind::Rocky,
            parent: None,
            habitable: false,
            profile: PlanetaryProfile::default(),
            deposits: vec![dep(Commodity::MetallicOre, 0.4)],
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
        };
        let mut found = None;
        for id in 0..2_000 {
            let mut candidate = barren.clone();
            candidate.profile = PlanetaryProfile::default();
            candidate.ensure_profile(&id.to_string());
            if matches!(
                candidate.profile.environment,
                Environment::Hostile | Environment::Uninhabitable
            ) && candidate.profile.geology == Geology::UltraRich
            {
                found = Some(candidate);
                break;
            }
        }
        let rich = found.expect("the independent rolls produce airless Ultra Rich worlds");
        assert_eq!(rich.profile.geology.mineral_extraction_mult(), 2.25);

        // Deposit composition is not an input to the three identity rolls.
        let mut other_deposit = barren.clone();
        barren.ensure_profile("same-id");
        other_deposit.deposits = vec![dep(Commodity::Biomass, 0.4)];
        other_deposit.ensure_profile("same-id");
        assert_eq!(barren.profile.size, other_deposit.profile.size);
        assert_eq!(
            barren.profile.environment,
            other_deposit.profile.environment
        );
        assert_eq!(barren.profile.geology, other_deposit.profile.geology);
    }

    #[test]
    fn planetary_profile_drives_legible_economic_factors() {
        let b = Body {
            id: 0,
            name: "Forge I".into(),
            kind: BodyKind::Rocky,
            parent: None,
            habitable: true,
            profile: PlanetaryProfile {
                version: 1,
                size: BodySize::Huge,
                environment: Environment::Gaia,
                geology: Geology::UltraRich,
                special: Some(BodySpecial::VolcanicMantle),
            },
            deposits: vec![dep(Commodity::MetallicOre, 0.4)],
            structures: BTreeMap::new(),
            population: 0.0,
            assignments: BTreeMap::new(),
        };
        assert!((b.habitat_capacity_mult() - 2.25).abs() < 1e-9);
        assert!((b.population_growth_mult() - 1.50).abs() < 1e-9);
        assert!((b.extraction_mult(Commodity::MetallicOre) - 2.70).abs() < 1e-9);
        assert_eq!(b.extraction_mult(Commodity::Biomass), 1.0);
        assert_eq!(
            b.converter_yield_mult(crate::build::StructureKind::Smelter),
            1.20
        );
    }

    #[test]
    fn pre_profile_body_migrates_without_moving_economic_state() {
        let raw = r#"{
            "id":4,"name":"Old IV","kind":"rocky","parent":null,
            "habitable":false,"deposits":[],"structures":{},
            "population":1.25,"assignments":{}
        }"#;
        let mut body: Body = serde_json::from_str(raw).unwrap();
        assert_eq!(body.profile.version, 0);
        body.ensure_profile("77");
        assert_eq!(body.profile.version, 1);
        assert_eq!(body.population, 1.25, "migration preserves population");
        assert!(body.deposits.is_empty(), "migration never rerolls deposits");
        let once = serde_json::to_string(&body.profile).unwrap();
        body.ensure_profile("different");
        assert_eq!(
            serde_json::to_string(&body.profile).unwrap(),
            once,
            "idempotent"
        );
    }
}

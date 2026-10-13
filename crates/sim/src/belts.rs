//! §belts — ASTEROID BELTS ARE REAL (`docs/asteroid-belt-mining-plan.md`).
//!
//! Every galaxy rolls its own belts from a keyed stream per system, so no
//! star, planet or deposit roll moves. A belt is a body
//! ([`BodyKind::AsteroidBelt`]) carrying lean deposits of its type's ores,
//! including the three ores planets never have (Olivine, Bornite, Ilmenite).
//! The server withholds a belt from a corporation until it surveys the
//! system; a colony mines it with an ordinary Mining Complex.
//!
//! §pockets: each belt also hides 0–2 small, finite, rich POCKETS of its
//! type's pocket ores (a shattered world 1–3), rolled from a stream of their
//! own. Only prospecting finds them, and a find is private to the finder.

use crate::body::{BeltKind, BeltShape, Body, BodyKind, PlanetaryProfile};
use crate::cargo::Commodity;
use crate::galaxy::{Deposit, StarSystem, DEPOSIT_BASE_RICHNESS};
use crate::math::Vec2;
use crate::rng::Rng;

/// Belts are LEAN: their ore comes on top of the planets'. Tunable.
pub const BELT_RICHNESS_MULT: f64 = 0.6;
/// A shattered world's belt is the remains of a planet: no lean factor.
pub const SHATTERED_RICHNESS_MULT: f64 = 1.0;
/// Belts per system: 10% two, 50% one, 40% none. Tunable.
pub const TWO_BELTS: f64 = 0.10;
pub const ONE_BELT: f64 = 0.50;
/// The home belt, identical for every player: below the home Ferrite floor
/// (0.315), so the founding mine still lands on the Ferrite world.
pub const HOME_BELT_RICHNESS: f64 = 0.25;
/// Soft border at each frontier window edge, as for planets.
const RAMP: f64 = 0.15;
/// The picture: belt orbit radius and band width in System View units.
const RADIUS_LO: f64 = 0.30;
const RADIUS_HI: f64 = 0.85;
const WIDTH_LO: f64 = 0.02;
const WIDTH_HI: f64 = 0.05;
/// Two belts in one system keep at least this far apart.
const BELT_GAP: f64 = 0.15;

/// One ore row: (ore, weight, frontier lo, frontier hi).
pub type OreRow = (Commodity, f64, f64, f64);

const STONY: &[OreRow] = &[
    (Commodity::CrystallineOre, 1.0, 0.0, 0.80),
    (Commodity::OlivineOre, 1.0, 0.0, 1.0),
    (Commodity::MetallicOre, 0.6, 0.0, 1.0),
];
const METALLIC: &[OreRow] = &[
    (Commodity::MetallicOre, 1.0, 0.0, 1.0),
    (Commodity::CupriteOre, 0.8, 0.20, 1.0),
    (Commodity::BorniteOre, 0.7, 0.25, 1.0),
    (Commodity::IlmeniteOre, 0.6, 0.30, 1.0),
];
const ICY: &[OreRow] = &[(Commodity::Volatiles, 1.0, 0.0, 1.0)];
const SHATTERED: &[OreRow] = &[
    (Commodity::MetallicOre, 0.6, 0.0, 1.0),
    (Commodity::CupriteOre, 0.8, 0.20, 1.0),
    (Commodity::BorniteOre, 0.7, 0.25, 1.0),
    (Commodity::IlmeniteOre, 0.8, 0.30, 1.0),
];

const KINDS: [BeltKind; 4] = [BeltKind::Stony, BeltKind::Metallic, BeltKind::Icy, BeltKind::Shattered];

/// §pockets: what each belt type can hide. Frontier windows hold here too, so
/// the scarce ores stay out on the rim. Tunable.
const STONY_POCKETS: &[OreRow] = &[
    (Commodity::CupriteOre, 1.0, 0.0, 1.0),
    (Commodity::BorniteOre, 0.7, 0.0, 1.0),
];
const METALLIC_POCKETS: &[OreRow] = &[
    (Commodity::TitaniumOre, 1.0, 0.45, 1.0),
    (Commodity::IridiumOre, 0.7, 0.60, 1.0),
    (Commodity::RareMetalOre, 0.5, 0.70, 1.0),
];
/// Rich Volatiles: a pocket of the belt's own ice, far richer than the belt.
const ICY_POCKETS: &[OreRow] = &[
    (Commodity::Volatiles, 1.0, 0.0, 1.0),
    (Commodity::Helium3Ice, 0.8, 0.55, 1.0),
];
const SHATTERED_POCKETS: &[OreRow] = &[
    (Commodity::TitaniumOre, 1.0, 0.45, 1.0),
    (Commodity::IridiumOre, 0.8, 0.60, 1.0),
    (Commodity::RareMetalOre, 0.6, 0.70, 1.0),
    (Commodity::VoidCrystal, 0.6, 0.65, 1.0),
];

/// §pockets: a pocket's richness is the planet formula with a richer vein
/// (×1.0–1.5) and no lean factor. Tunable.
pub const POCKET_VEIN_LO: f64 = 1.0;
pub const POCKET_VEIN_HI: f64 = 1.5;
/// §pockets: a pocket holds a quarter of a finite planet seam: 5–10k
/// bulk-equivalent units, scaled by the ore's unit ratio. Tunable.
pub const POCKET_RESERVE_SHARE: f64 = 0.25;
/// §pockets: every home belt hides the same Cuprite pocket, so prospecting
/// pays off the first time. Below the home Ferrite floor, like the belt.
pub const HOME_POCKET_RICHNESS: f64 = 0.30;
pub const HOME_POCKET_RESERVES: f64 = 3_000.0;

/// The base ores a belt type can carry. Scarce ores never appear here: in
/// belts they come only as hidden pockets (a later phase).
pub fn ores(kind: BeltKind) -> &'static [OreRow] {
    match kind {
        BeltKind::Stony => STONY,
        BeltKind::Metallic => METALLIC,
        BeltKind::Icy => ICY,
        BeltKind::Shattered => SHATTERED,
    }
}

/// §pockets: the ores a belt type can hide.
pub fn pocket_ores(kind: BeltKind) -> &'static [OreRow] {
    match kind {
        BeltKind::Stony => STONY_POCKETS,
        BeltKind::Metallic => METALLIC_POCKETS,
        BeltKind::Icy => ICY_POCKETS,
        BeltKind::Shattered => SHATTERED_POCKETS,
    }
}

/// A row's draw weight at a frontier value (0 outside its window).
pub fn ore_weight(row: OreRow, frontier: f64) -> f64 {
    let (_, weight, lo, hi) = row;
    if frontier < lo || frontier > hi {
        return 0.0;
    }
    let ramp_in = if lo > 0.0 { ((frontier - lo) / RAMP).min(1.0) } else { 1.0 };
    let ramp_out = if hi < 1.0 { ((hi - frontier) / RAMP).min(1.0) } else { 1.0 };
    weight * ramp_in * ramp_out
}

/// How likely each belt type is at a frontier value. Stony rocks dominate the
/// core, metal grows outward, and a shattered world only exists past the
/// middle of the disk — a few per galaxy. Tunable.
pub fn type_weight(kind: BeltKind, frontier: f64) -> f64 {
    let f = frontier.clamp(0.0, 1.0);
    match kind {
        BeltKind::Stony => 0.50 - 0.25 * f,
        BeltKind::Metallic => 0.20 + 0.20 * f,
        BeltKind::Icy => 0.30 - 0.05 * f,
        BeltKind::Shattered => {
            if f < 0.45 { 0.0 } else { 0.08 * ((f - 0.45) / RAMP).min(1.0) }
        }
    }
}

/// Frontier from position: 0 at the inner margin (0.12 R), 1 at the rim.
pub fn frontier_at(pos: Vec2, galaxy_radius: f64) -> f64 {
    ((pos.length() / galaxy_radius - 0.12) / 0.84).clamp(0.0, 1.0)
}

fn pick_kind(rng: &mut Rng, frontier: f64) -> BeltKind {
    let total: f64 = KINDS.iter().map(|k| type_weight(*k, frontier)).sum();
    let mut pick = rng.next_f64() * total;
    for kind in KINDS {
        pick -= type_weight(kind, frontier);
        if pick <= 0.0 && type_weight(kind, frontier) > 0.0 {
            return kind;
        }
    }
    BeltKind::Stony
}

fn pick_ore(rng: &mut Rng, kind: BeltKind, frontier: f64, taken: &[Commodity]) -> Option<Commodity> {
    pick_row(rng, ores(kind), frontier, taken)
}

fn pick_row(rng: &mut Rng, rows: &[OreRow], frontier: f64, taken: &[Commodity]) -> Option<Commodity> {
    let weight = |row: &OreRow| if taken.contains(&row.0) { 0.0 } else { ore_weight(*row, frontier) };
    let total: f64 = rows.iter().map(weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = rng.next_f64() * total;
    for row in rows {
        pick -= weight(row);
        if pick <= 0.0 && weight(row) > 0.0 {
            return Some(row.0);
        }
    }
    rows.iter().find(|row| weight(row) > 0.0).map(|row| row.0)
}

/// §pockets: a pocket's reserves, a quarter of a finite planet seam, from its
/// vein roll normalised to `0..1` (a richer pocket is also a deeper one).
pub fn pocket_reserves(resource: Commodity, vein: f64) -> f64 {
    let vein = vein.clamp(0.0, 1.0);
    let bulk = crate::galaxy::FINITE_RESERVES_LO
        + (crate::galaxy::FINITE_RESERVES_HI - crate::galaxy::FINITE_RESERVES_LO) * vein;
    (bulk * POCKET_RESERVE_SHARE * crate::production::ore_bulk_ratio(resource)).round()
}

/// §pockets: roll one belt's hidden pockets. 40% none, 45% one, 15% two; a
/// shattered world 1–3; a bare belt on a sparse star rarely any. Never two of
/// one ore. The count is drawn even when no ore fits the frontier, so the
/// stream's shape never depends on it.
pub fn roll_pockets(rng: &mut Rng, kind: BeltKind, frontier: f64, bare: bool) -> Vec<Deposit> {
    let roll = rng.next_f64();
    let count = if bare {
        usize::from(roll < 0.15)
    } else if kind == BeltKind::Shattered {
        if roll < 0.40 { 1 } else if roll < 0.80 { 2 } else { 3 }
    } else if roll < 0.40 {
        0
    } else if roll < 0.85 {
        1
    } else {
        2
    };
    let mut pockets: Vec<Deposit> = Vec::new();
    for _ in 0..count {
        let taken: Vec<Commodity> = pockets.iter().map(|d| d.resource).collect();
        let Some(resource) = pick_row(rng, pocket_ores(kind), frontier, &taken) else { break };
        let vein = rng.range(POCKET_VEIN_LO, POCKET_VEIN_HI);
        pockets.push(Deposit {
            resource,
            richness: DEPOSIT_BASE_RICHNESS * (0.70 + 0.55 * frontier) * vein * crate::galaxy::vein_mult(resource),
            reserves: Some(pocket_reserves(resource, (vein - POCKET_VEIN_LO) / (POCKET_VEIN_HI - POCKET_VEIN_LO))),
            accessibility: frontier,
        });
    }
    pockets
}

/// §pockets: the home belt's pocket, the same for every player.
pub fn home_pocket() -> Deposit {
    Deposit {
        resource: Commodity::CupriteOre,
        richness: HOME_POCKET_RICHNESS,
        reserves: Some(HOME_POCKET_RESERVES),
        accessibility: 0.1,
    }
}

/// One rolled belt: its type and picture, and its deposits.
#[derive(Debug, Clone, PartialEq)]
pub struct RolledBelt {
    pub shape: BeltShape,
    pub deposits: Vec<Deposit>,
}

/// A belt type's deposits: one ore, plus a second (a different ore) with a
/// chance equal to the frontier value. Icy belts carry Volatiles alone.
fn roll_deposits(rng: &mut Rng, kind: BeltKind, frontier: f64) -> Vec<Deposit> {
    let second = kind != BeltKind::Icy && rng.next_f64() < frontier;
    let mut deposits: Vec<Deposit> = Vec::new();
    for _ in 0..1 + second as usize {
        let taken: Vec<Commodity> = deposits.iter().map(|d| d.resource).collect();
        let Some(resource) = pick_ore(rng, kind, frontier, &taken) else { break };
        let vein = rng.range(0.80, 1.20);
        let lean = if kind == BeltKind::Shattered { SHATTERED_RICHNESS_MULT } else { BELT_RICHNESS_MULT };
        deposits.push(Deposit {
            resource,
            richness: DEPOSIT_BASE_RICHNESS * (0.70 + 0.55 * frontier) * vein
                * crate::galaxy::vein_mult(resource) * lean,
            reserves: crate::galaxy::reserves_for(resource, (vein - 0.80) / 0.40),
            accessibility: frontier,
        });
    }
    deposits
}

/// Roll one system's belts. `with_deposits` is false for sparse exploration
/// stars, whose belts are bare rock. Sorted inner to outer.
pub fn roll_belts(rng: &mut Rng, frontier: f64, with_deposits: bool) -> Vec<RolledBelt> {
    let count_roll = rng.next_f64();
    let count = if count_roll < TWO_BELTS {
        2
    } else if count_roll < TWO_BELTS + ONE_BELT {
        1
    } else {
        0
    };
    let mut belts: Vec<RolledBelt> = Vec::new();
    for _ in 0..count {
        let mut radius = rng.range(RADIUS_LO, RADIUS_HI);
        if let Some(first) = belts.first() {
            let r0 = first.shape.radius;
            if (radius - r0).abs() < BELT_GAP {
                // Move to the roomier side of the first belt.
                radius = if r0 - RADIUS_LO > RADIUS_HI - r0 { r0 - BELT_GAP } else { r0 + BELT_GAP };
            }
        }
        let width = rng.range(WIDTH_LO, WIDTH_HI);
        let kind = pick_kind(rng, frontier);
        let deposits = if with_deposits { roll_deposits(rng, kind, frontier) } else { Vec::new() };
        belts.push(RolledBelt { shape: BeltShape { kind, radius, width }, deposits });
    }
    belts.sort_by(|a, b| a.shape.radius.total_cmp(&b.shape.radius));
    belts
}

/// §belts: the belt of a home's rocky local survey stop, which the founding
/// charter's barge lesson mines once the opening surveys have charted it:
/// Stony or Metallic, with ore, though the stop is otherwise sparse.
pub fn prospect_belt(rng: &mut Rng, frontier: f64) -> RolledBelt {
    let stony = type_weight(BeltKind::Stony, frontier);
    let total = stony + type_weight(BeltKind::Metallic, frontier);
    let kind = if total <= 0.0 || rng.next_f64() * total < stony {
        BeltKind::Stony
    } else {
        BeltKind::Metallic
    };
    let radius = rng.range(RADIUS_LO, RADIUS_HI);
    let width = rng.range(WIDTH_LO, WIDTH_HI);
    let deposits = roll_deposits(rng, kind, frontier);
    RolledBelt { shape: BeltShape { kind, radius, width }, deposits }
}

/// The home belt: the same Stony belt of Crystalline for every player.
pub fn home_belt() -> RolledBelt {
    RolledBelt {
        shape: BeltShape { kind: BeltKind::Stony, radius: 0.62, width: 0.035 },
        deposits: vec![Deposit {
            resource: Commodity::CrystallineOre,
            richness: HOME_BELT_RICHNESS,
            reserves: None,
            accessibility: 0.1,
        }],
    }
}

/// Append `belts` to `system` as bodies, with ids after every existing body so
/// planets and moons keep theirs. A system that already has belts is left
/// alone.
pub fn add_belts(system: &mut StarSystem, belts: Vec<RolledBelt>) {
    if system.bodies.iter().any(Body::is_belt) {
        return;
    }
    let count = belts.len();
    let mut next_id = system.bodies.iter().map(|b| b.id + 1).max().unwrap_or(0);
    let system_id = system.id.0.to_string();
    for (i, belt) in belts.into_iter().enumerate() {
        let name = if count > 1 && i + 1 == count {
            format!("{} Outer Belt", system.name)
        } else {
            format!("{} Belt", system.name)
        };
        let mut body = Body {
            founding_camp: None,
            id: next_id,
            name,
            kind: BodyKind::AsteroidBelt,
            parent: None,
            belt: Some(belt.shape),
            habitable: false,
            profile: PlanetaryProfile::default(),
            deposits: belt.deposits,
            pockets: Vec::new(),
            prospected: Default::default(),
            structures: Default::default(),
            population: 0.0,
            migration_policy: Default::default(),
            inbound_migrants: 0,
            assignments: Default::default(),
            grid: Default::default(),
        };
        body.ensure_profile(&system_id);
        system.bodies.push(body);
        next_id += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn belts_roll_from_their_own_stream_with_ores_inside_their_windows() {
        let mut counts = [0usize; 3];
        let mut kinds = std::collections::BTreeMap::new();
        for id in 0..4000u64 {
            let frontier = (id % 101) as f64 / 100.0;
            let mut rng = Rng::keyed_id(7, "belts-v1", id);
            let belts = roll_belts(&mut rng, frontier, true);
            counts[belts.len()] += 1;
            for belt in &belts {
                *kinds.entry(belt.shape.kind).or_insert(0usize) += 1;
                assert!((RADIUS_LO - BELT_GAP..=RADIUS_HI + BELT_GAP).contains(&belt.shape.radius));
                assert!(!belt.deposits.is_empty(), "every full-system belt carries ore");
                if belt.shape.kind == BeltKind::Shattered {
                    assert!(frontier >= 0.45, "shattered worlds sit past the middle");
                }
                for d in &belt.deposits {
                    let row = ores(belt.shape.kind).iter().find(|r| r.0 == d.resource)
                        .expect("a belt only carries its own type's ores");
                    assert!(ore_weight(*row, frontier) > 0.0, "{:?} outside its window", d.resource);
                    assert!(!matches!(d.resource, Commodity::TitaniumOre | Commodity::RareMetalOre),
                        "scarce ores only come as pockets");
                    assert!(d.reserves.is_none(), "base belt ores renew");
                }
            }
            if belts.len() == 2 {
                assert!(belts[1].shape.radius - belts[0].shape.radius >= BELT_GAP - 1e-9);
            }
        }
        let share = |n: usize| n as f64 / 4000.0;
        assert!((share(counts[0]) - 0.40).abs() < 0.03, "none {:.3}", share(counts[0]));
        assert!((share(counts[1]) - 0.50).abs() < 0.03, "one {:.3}", share(counts[1]));
        assert!((share(counts[2]) - 0.10).abs() < 0.02, "two {:.3}", share(counts[2]));
        assert!(kinds.len() == 4, "every type occurs: {kinds:?}");
    }

    #[test]
    fn sparse_belts_are_bare_and_rolls_are_deterministic() {
        let a = roll_belts(&mut Rng::keyed_id(9, "belts-v1", 33), 0.6, true);
        let b = roll_belts(&mut Rng::keyed_id(9, "belts-v1", 33), 0.6, true);
        assert_eq!(a, b);
        for id in 0..200 {
            for belt in roll_belts(&mut Rng::keyed_id(9, "belts-v1", id), 0.5, false) {
                assert!(belt.deposits.is_empty());
            }
        }
    }
}

//! NEBULAE — static deep-space terrain between star systems.
//!
//! Regions are public geography generated from their own keyed RNG stream. The
//! picture and the rules share the same rotated ellipse, so a fleet crosses the
//! visible edge at exactly the point where its sensor/signature/jump modifier
//! changes. Effects plug into the existing detection and jump choke points; no
//! parallel visibility model is introduced.

use serde::{Deserialize, Serialize};

use crate::{EntityId, Rng, Vec2};

/// One of each is generated in the alpha galaxy so every terrain rule is
/// present in every playtest. The enum order is stable generation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NebulaKind {
    MolecularCloud,
    IonNebula,
    DustCloud,
    SupernovaRemnant,
    PrecursorCloud,
}

impl NebulaKind {
    pub const ALL: [Self; 5] = [
        Self::MolecularCloud,
        Self::IonNebula,
        Self::DustCloud,
        Self::SupernovaRemnant,
        Self::PrecursorCloud,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::MolecularCloud => "Nacre Molecular Cloud",
            Self::IonNebula => "Vesper Ion Nebula",
            Self::DustCloud => "Obsidian Dust Cloud",
            Self::SupernovaRemnant => "Cinderwake Remnant",
            Self::PrecursorCloud => "Orison Precursor Cloud",
        }
    }

    /// Multiplier applied to a DARK fleet's detection signature at the target.
    /// A supernova remnant is a hazard because radiation makes traffic louder.
    pub fn signature_mult(self) -> f64 {
        match self {
            Self::MolecularCloud => 0.65,
            Self::DustCloud => 0.35,
            Self::SupernovaRemnant => 1.50,
            Self::IonNebula | Self::PrecursorCloud => 1.0,
        }
    }

    /// Multiplier applied to a sensor source whose emitter sits in the region.
    pub fn sensor_mult(self) -> f64 {
        match self {
            Self::IonNebula => 0.55,
            _ => 1.0,
        }
    }

    /// A jump spooled from precursor space can reach farther. Destination-only
    /// presence grants nothing: the field shapes the drive at its origin.
    pub fn jump_range_mult(self) -> f64 {
        match self {
            Self::PrecursorCloud => 1.25,
            _ => 1.0,
        }
    }
}

/// A rotated elliptical terrain region. The generated texture is deliberately
/// wispy, but this ellipse is the honest mechanical boundary and hit geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NebulaRegion {
    pub id: u32,
    pub kind: NebulaKind,
    pub name: String,
    pub center: Vec2,
    pub radius_x: f64,
    pub radius_y: f64,
    /// Clockwise screen/world rotation in radians.
    pub rotation: f64,
    /// Stable visual seed for future shader/noise variation.
    pub seed: u64,
}

impl NebulaRegion {
    /// Squared normalized distance in the region's local ellipse frame.
    pub fn normalized_distance_sq(&self, pos: Vec2) -> f64 {
        let d = pos - self.center;
        let (sin, cos) = self.rotation.sin_cos();
        let x = d.x * cos + d.y * sin;
        let y = -d.x * sin + d.y * cos;
        (x / self.radius_x).powi(2) + (y / self.radius_y).powi(2)
    }

    pub fn contains(&self, pos: Vec2) -> bool {
        self.normalized_distance_sq(pos) <= 1.0 + 1e-12
    }
}

/// Generate exactly one region of every kind. Each candidate is anchored near
/// a non-home system but offset into deep space: the object reads as BETWEEN
/// stars while still creating surveyable economic consequences at its fringe.
pub fn generate_nebulas(
    seed: u64,
    galaxy_radius: f64,
    systems: &[(EntityId, Vec2)],
    home_systems: &[EntityId],
) -> Vec<NebulaRegion> {
    let mut rng = Rng::keyed(seed, "nebulas-v1");
    let anchors: Vec<Vec2> = systems
        .iter()
        .filter(|(id, _)| !home_systems.contains(id))
        .map(|(_, pos)| *pos)
        .collect();
    if anchors.is_empty() {
        return Vec::new();
    }

    let mut regions: Vec<NebulaRegion> = Vec::with_capacity(NebulaKind::ALL.len());
    for (index, kind) in NebulaKind::ALL.into_iter().enumerate() {
        let mut chosen = None;
        for _ in 0..192 {
            let anchor = anchors[(rng.next_u64() as usize) % anchors.len()];
            let radius_x = rng.range(42_000.0, 68_000.0).min(galaxy_radius * 0.19);
            let radius_y = radius_x * rng.range(0.56, 0.82);
            let rotation = rng.range(0.0, std::f64::consts::TAU);
            let offset = Vec2::from_polar(
                rng.range(0.0, std::f64::consts::TAU),
                radius_y * rng.range(0.22, 0.48),
            );
            let center = anchor + offset;
            if center.length() + radius_x > galaxy_radius * 0.97 {
                continue;
            }
            if center.length() < galaxy_radius * 0.16 {
                continue; // keep the Market Hub readable
            }
            if regions
                .iter()
                .any(|other| other.center.distance(center) < (other.radius_y + radius_y) * 0.82)
            {
                continue;
            }
            let candidate = NebulaRegion {
                id: index as u32 + 1,
                kind,
                name: kind.title().to_string(),
                center,
                radius_x,
                radius_y,
                rotation,
                seed: rng.next_u64(),
            };
            // The anchor should remain inside despite the offset; this assertion
            // is also the resource-bias guarantee used by world generation.
            if candidate.contains(anchor) {
                chosen = Some(candidate);
                break;
            }
        }
        if let Some(region) = chosen {
            regions.push(region);
        }
    }
    regions
}

/// Multipliers compose in the unlikely overlap sliver. Clamps prevent a future
/// density increase from making a fleet perfectly invisible or infinitely loud.
pub fn signature_factor_at(regions: &[NebulaRegion], pos: Vec2) -> f64 {
    regions
        .iter()
        .filter(|region| region.contains(pos))
        .fold(1.0, |factor, region| factor * region.kind.signature_mult())
        .clamp(0.15, 3.0)
}

pub fn sensor_factor_at(regions: &[NebulaRegion], pos: Vec2) -> f64 {
    regions
        .iter()
        .filter(|region| region.contains(pos))
        .fold(1.0, |factor, region| factor * region.kind.sensor_mult())
        .clamp(0.20, 1.0)
}

pub fn jump_range_factor_at(regions: &[NebulaRegion], pos: Vec2) -> f64 {
    regions
        .iter()
        .filter(|region| region.contains(pos))
        .fold(1.0_f64, |factor, region| {
            factor.max(region.kind.jump_range_mult())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_complete_and_anchored_between_stars() {
        let systems: Vec<_> = (0..24)
            .map(|i| {
                let angle = i as f64 / 24.0 * std::f64::consts::TAU;
                (
                    EntityId(i + 1),
                    Vec2::from_polar(angle, 170_000.0 + i as f64 * 3_000.0),
                )
            })
            .collect();
        let a = generate_nebulas(77, 400_000.0, &systems, &[EntityId(1)]);
        let b = generate_nebulas(77, 400_000.0, &systems, &[EntityId(1)]);
        assert_eq!(a, b);
        assert_eq!(a.len(), NebulaKind::ALL.len());
        for kind in NebulaKind::ALL {
            assert_eq!(a.iter().filter(|region| region.kind == kind).count(), 1);
        }
        assert!(
            a.iter()
                .all(|region| systems.iter().any(|(_, pos)| region.contains(*pos)))
        );
        assert!(a.iter().all(|region| region.center.length() > 1.0));
    }

    #[test]
    fn terrain_modifiers_change_only_their_declared_channel() {
        let region = |kind| NebulaRegion {
            id: 1,
            kind,
            name: String::new(),
            center: Vec2::ZERO,
            radius_x: 100.0,
            radius_y: 50.0,
            rotation: 0.0,
            seed: 0,
        };
        let inside = Vec2::new(20.0, 0.0);
        assert_eq!(
            signature_factor_at(&[region(NebulaKind::DustCloud)], inside),
            0.35
        );
        assert_eq!(
            sensor_factor_at(&[region(NebulaKind::IonNebula)], inside),
            0.55
        );
        assert_eq!(
            jump_range_factor_at(&[region(NebulaKind::PrecursorCloud)], inside),
            1.25
        );
        assert_eq!(
            signature_factor_at(&[region(NebulaKind::DustCloud)], Vec2::new(101.0, 0.0)),
            1.0
        );
    }

    #[test]
    fn ordinary_galaxies_ship_all_five_regions_and_their_survey_hooks() {
        for seed in 0..8 {
            let world = crate::World::new(crate::SimConfig::for_players(seed, 4));
            assert_eq!(world.nebulas.len(), NebulaKind::ALL.len(), "seed {seed}");
            let region = |kind| {
                world
                    .nebulas
                    .iter()
                    .find(|region| region.kind == kind)
                    .unwrap()
            };
            let systems_in = |kind| {
                let region = region(kind);
                world
                    .systems
                    .iter()
                    .filter(move |system| region.contains(system.pos))
            };
            assert!(systems_in(NebulaKind::MolecularCloud).any(|system| {
                system
                    .all_deposits()
                    .any(|deposit| deposit.resource == crate::Commodity::Volatiles)
            }));
            assert!(systems_in(NebulaKind::SupernovaRemnant).any(|system| {
                system
                    .all_deposits()
                    .any(|deposit| deposit.resource == crate::Commodity::RareMetalOre)
            }));
            assert!(systems_in(NebulaKind::PrecursorCloud).any(|system| {
                system
                    .bodies
                    .iter()
                    .any(|body| body.profile.special == Some(crate::BodySpecial::PrecursorRuins))
            }));
        }
    }
}

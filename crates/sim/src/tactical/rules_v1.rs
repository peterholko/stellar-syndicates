//! Combat-facing balance tables and RNG frozen with the v1 kernel. Do not
//! reconnect these to the mutable ship/module catalog when tuning a new version.

use crate::module::{DamageType, Family, Loadout, ModuleKind};
use crate::ship::ShipKind;
use serde::{Deserialize, Serialize};

pub fn hull_mass(kind: ShipKind) -> f64 {
    match kind {
        ShipKind::Convoy => 4500.0,
        ShipKind::Builder => 2500.0,
        ShipKind::Raider => 200.0,
        ShipKind::Corvette => 800.0,
        ShipKind::Colony => 6000.0,
        ShipKind::Scout => 80.0,
        ShipKind::Destroyer => 2000.0,
        ShipKind::Cruiser => 4000.0,
        ShipKind::Battleship => 8000.0,
        ShipKind::Dreadnought => 16000.0,
        ShipKind::Titan => 32000.0,
        ShipKind::Transport => 7000.0,
        ShipKind::Freighter => 6000.0,
    }
}

pub fn max_speed(kind: ShipKind) -> f64 {
    match kind {
        ShipKind::Convoy => 40.0,
        ShipKind::Builder => 35.0,
        ShipKind::Raider => 100.0,
        ShipKind::Corvette => 65.0,
        ShipKind::Colony => 33.0,
        ShipKind::Scout => 115.0,
        ShipKind::Destroyer => 55.0,
        ShipKind::Cruiser => 45.0,
        ShipKind::Battleship => 36.0,
        ShipKind::Dreadnought => 29.0,
        ShipKind::Titan => 23.0,
        ShipKind::Transport => 30.0,
        ShipKind::Freighter => 32.0,
    }
}

pub fn attack_weight(kind: ShipKind) -> f64 {
    match kind {
        ShipKind::Raider => 3.0,
        ShipKind::Corvette => 1.0,
        ShipKind::Destroyer => 2.4,
        ShipKind::Cruiser => 4.5,
        ShipKind::Battleship => 8.0,
        ShipKind::Dreadnought => 12.0,
        ShipKind::Titan => 24.0,
        _ => 0.0,
    }
}

pub fn hull_affinity(kind: ShipKind, family: Family) -> f64 {
    match (kind, family) {
        (ShipKind::Raider, Family::Torpedo) => 1.25,
        (ShipKind::Corvette, Family::Interception) => 1.25,
        (ShipKind::Destroyer, Family::Beam) => 1.20,
        (ShipKind::Cruiser, Family::Protection) => 1.20,
        (ShipKind::Battleship, Family::Driver) => 1.20,
        (ShipKind::Dreadnought, Family::Interception) => 1.30,
        (ShipKind::Titan, Family::Beam | Family::Driver | Family::Torpedo) => 1.10,
        _ => 1.0,
    }
}

pub fn offense(loadout: &Loadout) -> (DamageType, f64) {
    let count = |kind| loadout.modules().iter().filter(|k| **k == kind).count() as f64;
    if count(ModuleKind::TorpedoRack) > 0.0 {
        (DamageType::Torpedo, 1.6 * count(ModuleKind::TorpedoRack))
    } else if count(ModuleKind::MassDriver) > 0.0 {
        (DamageType::Driver, 1.3 * count(ModuleKind::MassDriver))
    } else if count(ModuleKind::PointDefenseScreen) > 0.0 {
        (DamageType::Beam, 0.5)
    } else {
        (DamageType::Beam, 1.0)
    }
}

pub fn featured(kind: ShipKind) -> bool {
    matches!(
        kind,
        ShipKind::Destroyer
            | ShipKind::Cruiser
            | ShipKind::Battleship
            | ShipKind::Dreadnought
            | ShipKind::Titan
    )
}

/// Same SplitMix64 stream/serialized shape as the original battle engine,
/// isolated here so changing the galaxy RNG cannot rewrite an old battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub(crate) fn state_bits(&self) -> u64 {
        self.state
    }
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn next_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) / ((1u64 << 53) as f64)
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
}

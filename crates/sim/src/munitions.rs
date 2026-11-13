//! §shipbuilding-overhaul Phase 5: MUNITIONS (docs/shipbuilding-overhaul-plan.md
//! §8). Fighting gets a supply line. Three goods, one per damage type, made at
//! the Armaments Complex after Munitions Lines:
//!
//! - **Torpedoes** feed Torpedo Racks: one unit one shot; they exhaust.
//! - **Slugs** feed Mass Drivers: one unit one shot; they exhaust.
//! - **Beam Crystals** sit in every beam gun (the kit's Pulse Lasers and Beam
//!   Batteries, a Lance Array, a Point-Defense Screen): one per gun, and they
//!   WEAR rather than run out — a worn crystal fires at [`CRYSTAL_FLOOR`] of a
//!   fresh one, never zero, so a fleet is never disarmed.
//!
//! A fleet carries one MAGAZINE per kind, filled free at owned or allied docks
//! from the stockpile, bought at the Hub, topped up in the field from a
//! freighter's hold like fuel. The granularity lives inside the battle (rules
//! v17): the engine fires one torpedo per rack per round and decrements; the
//! player sees a magazine bar, never a missile. A dry rack goes silent and the
//! hull falls back to its kit guns. NPC fleets (pirates, the Authority) carry
//! no magazine and are exempt, like sentinels from upkeep.

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::module::{DamageType, Loadout, ModuleKind};
use crate::ship::ShipKind;

/// The three munitions, in damage-type order.
pub const MUNITIONS: [Commodity; 3] = [Commodity::BeamCrystals, Commodity::Slugs, Commodity::Torpedoes];

/// Rounds a Torpedo Rack's magazine holds. Tunable.
pub const TORPEDOES_PER_RACK: f64 = 40.0;
/// Rounds a Mass Driver's magazine holds. Tunable.
pub const SLUGS_PER_DRIVER: f64 = 60.0;
/// Crystals a beam gun carries: one.
pub const CRYSTALS_PER_GUN: f64 = 1.0;
/// How much of its crystal one beam gun's shot wears away: a thousand shots
/// take a gun from fresh to worn. Tunable.
pub const CRYSTAL_WEAR_PER_SHOT: f64 = 0.001;
/// A worn crystal still fires at this share of a fresh one. Tunable.
pub const CRYSTAL_FLOOR: f64 = 0.5;

/// The munition a weapon family fires.
pub fn munition_for(family: DamageType) -> Commodity {
    match family {
        DamageType::Beam => Commodity::BeamCrystals,
        DamageType::Driver => Commodity::Slugs,
        DamageType::Torpedo => Commodity::Torpedoes,
    }
}

/// The Torpedo Racks a fit carries.
pub fn racks(fit: &Loadout) -> f64 {
    fit.modules().iter().filter(|m| **m == ModuleKind::TorpedoRack).count() as f64
}

/// The Mass Drivers a fit carries.
pub fn drivers(fit: &Loadout) -> f64 {
    fit.modules().iter().filter(|m| **m == ModuleKind::MassDriver).count() as f64
}

/// The beam guns a hull fires with, each holding one crystal: the kit's Pulse
/// Lasers and Beam Batteries plus every beam module aboard (a Lance Array, a
/// Prismatic Lance, a Point-Defense Screen's beam).
pub fn beam_guns(kind: ShipKind, fit: &Loadout) -> f64 {
    let (pulse, beam) = kind.guns();
    f64::from(pulse + beam) + fit.modules().iter()
        .filter(|m| matches!(m, ModuleKind::LanceArray | ModuleKind::PrismaticLance | ModuleKind::PointDefenseScreen))
        .count() as f64
}

/// The family a munition feeds.
pub fn family_of(c: Commodity) -> Option<DamageType> {
    match c {
        Commodity::BeamCrystals => Some(DamageType::Beam),
        Commodity::Slugs => Some(DamageType::Driver),
        Commodity::Torpedoes => Some(DamageType::Torpedo),
        _ => None,
    }
}

/// A fleet's magazines, as the battle engine carries them (rules v17) and the
/// world fills them. `crystal_capacity` is the beam guns aboard; `crystals` is
/// what is left of them.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Magazine {
    pub torpedoes: f64,
    pub slugs: f64,
    pub crystals: f64,
    pub crystal_capacity: f64,
}

impl Magazine {
    /// No round left for this family: the rack goes silent and the hull falls
    /// back to its beams. Beams never run dry.
    pub fn dry(&self, family: DamageType) -> bool {
        match family {
            DamageType::Torpedo => self.torpedoes < 1.0,
            DamageType::Driver => self.slugs < 1.0,
            DamageType::Beam => false,
        }
    }

    /// Fire one shot of `family` from a hull with `guns` beam guns: a torpedo
    /// or slug leaves the magazine, the beams wear their crystals. Returns
    /// false when there was no round.
    pub fn fire(&mut self, family: DamageType, guns: f64) -> bool {
        match family {
            DamageType::Torpedo => {
                if self.torpedoes < 1.0 { return false; }
                self.torpedoes -= 1.0;
            }
            DamageType::Driver => {
                if self.slugs < 1.0 { return false; }
                self.slugs -= 1.0;
            }
            DamageType::Beam => {
                self.crystals = (self.crystals - CRYSTAL_WEAR_PER_SHOT * guns.max(0.0)).max(0.0);
            }
        }
        true
    }

    /// The beams' damage multiplier from the state of their crystals: fresh
    /// fires at 1, worn at [`CRYSTAL_FLOOR`]. A hull with no beam guns is 1.
    pub fn crystal_factor(&self) -> f64 {
        if self.crystal_capacity <= 1e-9 {
            return 1.0;
        }
        let fresh = (self.crystals / self.crystal_capacity).clamp(0.0, 1.0);
        CRYSTAL_FLOOR + (1.0 - CRYSTAL_FLOOR) * fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_exhaust_and_crystals_wear_to_a_floor() {
        let mut m = Magazine { torpedoes: 1.0, slugs: 0.0, crystals: 2.0, crystal_capacity: 2.0 };
        assert!(!m.dry(DamageType::Torpedo) && m.dry(DamageType::Driver) && !m.dry(DamageType::Beam));
        assert!(m.fire(DamageType::Torpedo, 2.0) && m.dry(DamageType::Torpedo), "one torpedo, one shot");
        assert!(!m.fire(DamageType::Torpedo, 2.0) && !m.fire(DamageType::Driver, 2.0), "a dry rack fires nothing");
        assert_eq!(m.crystal_factor(), 1.0);
        for _ in 0..1_000 { assert!(m.fire(DamageType::Beam, 2.0), "beams never run dry"); }
        assert_eq!(m.crystals, 0.0);
        assert_eq!(m.crystal_factor(), CRYSTAL_FLOOR, "worn crystals still fire at half");
        assert_eq!(Magazine::default().crystal_factor(), 1.0, "no beam guns, no penalty");
        for c in MUNITIONS { assert_eq!(munition_for(family_of(c).unwrap()), c); }
        let bare = Loadout::default();
        assert_eq!(beam_guns(ShipKind::Corvette, &bare), 2.0, "the Corvette's two Pulse Lasers");
        assert_eq!(beam_guns(ShipKind::Destroyer, &Loadout::new(vec![ModuleKind::PointDefenseScreen])), 3.0);
        assert_eq!(beam_guns(ShipKind::Freighter, &bare), 0.0, "no guns, no crystals");
        assert_eq!(racks(&Loadout::new(vec![ModuleKind::TorpedoRack, ModuleKind::TorpedoRack])), 2.0);
    }
}

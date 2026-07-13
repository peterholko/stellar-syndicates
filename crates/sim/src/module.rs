//! Ship MODULES — the MoO2-shaped equipment layer (§modules Part B). Few
//! modules, qualitative HARD counters, no quality tiers: a clean matrix where
//! every weapon has exactly ONE counter.
//!
//! * **Reflective Plating** stops BEAMS (mirrored ablative facing — the only
//!   thing that stops light is reflecting and ablating it).
//! * **Whipple Armor** stops DRIVERS (spaced kinetic armor shattering slugs).
//! * **Point-Defense Screen** stops TORPEDOES (and pays for it out of a weapon
//!   slot, so screening costs offense).
//!
//! Torpedoes ignore both armors — their counter is INTERCEPTION. Beams travel
//! at c: nothing intercepts or jams a weapon that arrives with its own light,
//! so their only counter is reflection. ECM is deliberately ABSENT from v1 so
//! the counter matrix stays one-to-one (PD owns anti-torpedo alone).
//!
//! Modules are manufactured items (§Part B3): built from Armaments + Electronics
//! (+ a real Silicates sink for the glass mirrors), shipped by raidable convoy,
//! installed at Shipyards. This module owns the CATALOG + the damage-typing math
//! primitives; `combat.rs` folds them into the pooled Lanchester pipeline and
//! `ship.rs` carries loadouts on fleets.

use std::fmt;

use serde::{Deserialize, Serialize};

// --- TUNABLE MODULE BLOCK (the counter-triangle knobs) ----------------------
// All `Tunable`: modules SHIFT matchups, never the clock — the equal-reference
// duration calibration (`combat::DMG_RATE_CALIBRATION`) must still pass with
// default (UNFITTED) fleets, which these constants leave untouched.
/// Mass Driver offense: a driver salvo hits harder than a stock beam, but is
/// opt-in and answerable by Whipple armor.
pub const DRIVER_MULT: f64 = 1.3;
/// Torpedo Rack offense: the hardest single hit — but slow, and interceptable.
pub const TORP_MULT: f64 = 1.6;
/// A Point-Defense ship trades offense for screening: its beam runs at half.
pub const PD_ATTACK: f64 = 0.5;
/// Side interception: incoming torpedo damage is cut by `PD_INTERCEPT × pd_share`,
/// where `pd_share` is the PD-fitted hull fraction of the defending side.
pub const PD_INTERCEPT: f64 = 0.6;
/// Reflective Plating blunts BEAM damage into the fitted stack by this fraction.
pub const REFLECT_BLUNT: f64 = 0.35;
/// Whipple Armor blunts DRIVER damage into the fitted stack by this fraction —
/// hotter than Reflective's, because drivers are opt-in while every ship fires
/// beams. Tune both against the meta, not each other.
pub const WHIPPLE_BLUNT: f64 = 0.45;

/// §modules Part B3: how many module CRATES one convoy hauls in a `TransferModules`
/// run. Crates are dense compared to people, so a hauler moves a healthy batch —
/// enough to fit out a small squadron in one raidable crossing. Tunable.
pub const MODULE_CONVOY_BERTHS: u32 = 12;

/// The three damage TYPES a weapon deals. Each has exactly one counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageType {
    /// Laser clusters — the stock fit on every combatant. Travel at c;
    /// counter = Reflective Plating (reflection is the only thing that stops it).
    Beam,
    /// Mass Driver slugs — counter = Whipple Armor.
    Driver,
    /// Torpedoes — ignore armor; counter = Point-Defense interception.
    Torpedo,
}

/// One equipment MODULE. Each does exactly ONE legible thing (MoO2 discipline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleKind {
    /// Weapon: fires DRIVERS at `DRIVER_MULT`.
    MassDriver,
    /// Weapon: fires TORPEDOES at `TORP_MULT`.
    TorpedoRack,
    /// Weapon + defense: fires BEAM at `PD_ATTACK`, and contributes side
    /// torpedo interception (the anti-torpedo counter — it costs a weapon slot).
    PointDefenseScreen,
    /// Armor: blunts incoming BEAM into this ship (`REFLECT_BLUNT`).
    ReflectivePlating,
    /// Armor: blunts incoming DRIVER into this ship (`WHIPPLE_BLUNT`).
    WhippleArmor,
}

/// Every module kind, in a fixed deterministic order (menus, iteration).
pub const MODULE_KINDS: [ModuleKind; 5] = [
    ModuleKind::MassDriver,
    ModuleKind::TorpedoRack,
    ModuleKind::PointDefenseScreen,
    ModuleKind::ReflectivePlating,
    ModuleKind::WhippleArmor,
];

impl ModuleKind {
    /// Stable wire slug (matches serde `rename_all = "snake_case"`).
    pub fn slug(self) -> &'static str {
        match self {
            ModuleKind::MassDriver => "mass_driver",
            ModuleKind::TorpedoRack => "torpedo_rack",
            ModuleKind::PointDefenseScreen => "point_defense_screen",
            ModuleKind::ReflectivePlating => "reflective_plating",
            ModuleKind::WhippleArmor => "whipple_armor",
        }
    }

    pub fn from_slug(s: &str) -> Option<Self> {
        MODULE_KINDS.into_iter().find(|m| m.slug() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            ModuleKind::MassDriver => "Mass Driver",
            ModuleKind::TorpedoRack => "Torpedo Rack",
            ModuleKind::PointDefenseScreen => "Point-Defense Screen",
            ModuleKind::ReflectivePlating => "Reflective Plating",
            ModuleKind::WhippleArmor => "Whipple Armor",
        }
    }

    /// Is this a WEAPON module (determines the ship's offense type)?
    pub fn is_weapon(self) -> bool {
        matches!(
            self,
            ModuleKind::MassDriver | ModuleKind::TorpedoRack | ModuleKind::PointDefenseScreen
        )
    }
}

impl fmt::Display for ModuleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// A ship's LOADOUT: a canonical (sorted, deduped-by-slot) list of ≤ `slots`
/// modules. `""` (empty) is the default UNFITTED ship — the stock beam brawler,
/// stored implicitly (never in the loadout map). The wire form is the slugs
/// joined by `+` ("mass_driver+whipple_armor"), which is also the map key so
/// stacks group deterministically.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Loadout(Vec<ModuleKind>);

impl Loadout {
    /// Build a canonical loadout from any module list (sorted; order-insensitive
    /// so "driver+whipple" == "whipple+driver"). Does NOT enforce slot count —
    /// the build/refit validators do that against the hull's `module_slots`.
    pub fn new(mut mods: Vec<ModuleKind>) -> Self {
        mods.sort();
        Loadout(mods)
    }

    pub fn modules(&self) -> &[ModuleKind] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The wire/map key — slugs joined by `+`, or `""` for an unfitted ship.
    pub fn key(&self) -> String {
        self.0.iter().map(|m| m.slug()).collect::<Vec<_>>().join("+")
    }

    /// Parse a key back to a canonical loadout. Unknown slugs are dropped (a
    /// forward-compat guard); `""` → the unfitted default.
    pub fn from_key(key: &str) -> Self {
        if key.is_empty() {
            return Loadout::default();
        }
        Loadout::new(key.split('+').filter_map(ModuleKind::from_slug).collect())
    }

    /// The ship's OFFENSE: `(damage_type, multiplier)`. The WEAPON module
    /// decides it (torpedo > driver > point-defense-beam), else a stock beam.
    /// Armor modules never change offense.
    pub fn offense(&self) -> (DamageType, f64) {
        if self.0.contains(&ModuleKind::TorpedoRack) {
            (DamageType::Torpedo, TORP_MULT)
        } else if self.0.contains(&ModuleKind::MassDriver) {
            (DamageType::Driver, DRIVER_MULT)
        } else if self.0.contains(&ModuleKind::PointDefenseScreen) {
            (DamageType::Beam, PD_ATTACK)
        } else {
            (DamageType::Beam, 1.0)
        }
    }

    /// Blunts BEAM damage into this ship (Reflective Plating).
    pub fn reflects(&self) -> bool {
        self.0.contains(&ModuleKind::ReflectivePlating)
    }

    /// Blunts DRIVER damage into this ship (Whipple Armor).
    pub fn whipples(&self) -> bool {
        self.0.contains(&ModuleKind::WhippleArmor)
    }

    /// Contributes to the side's TORPEDO interception (Point-Defense Screen).
    pub fn has_pd(&self) -> bool {
        self.0.contains(&ModuleKind::PointDefenseScreen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_round_trips_and_canonicalizes() {
        let a = Loadout::new(vec![ModuleKind::WhippleArmor, ModuleKind::MassDriver]);
        let b = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]);
        assert_eq!(a, b, "order-insensitive canonical form");
        assert_eq!(a.key(), "mass_driver+whipple_armor");
        assert_eq!(Loadout::from_key(&a.key()), a, "key round-trips");
        assert_eq!(Loadout::from_key(""), Loadout::default(), "empty key = unfitted");
        assert!(Loadout::default().is_empty());
    }

    #[test]
    fn offense_type_is_the_weapon_module() {
        assert_eq!(Loadout::default().offense(), (DamageType::Beam, 1.0));
        assert_eq!(Loadout::new(vec![ModuleKind::MassDriver]).offense(), (DamageType::Driver, DRIVER_MULT));
        assert_eq!(Loadout::new(vec![ModuleKind::TorpedoRack]).offense(), (DamageType::Torpedo, TORP_MULT));
        assert_eq!(Loadout::new(vec![ModuleKind::PointDefenseScreen]).offense(), (DamageType::Beam, PD_ATTACK));
        // Armor alone doesn't change offense; a weapon + armor keeps the weapon.
        assert_eq!(Loadout::new(vec![ModuleKind::ReflectivePlating]).offense(), (DamageType::Beam, 1.0));
        assert_eq!(Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]).offense(), (DamageType::Driver, DRIVER_MULT));
    }

    #[test]
    fn mitigation_flags() {
        let l = Loadout::new(vec![ModuleKind::ReflectivePlating]);
        assert!(l.reflects() && !l.whipples() && !l.has_pd());
        let w = Loadout::new(vec![ModuleKind::WhippleArmor]);
        assert!(w.whipples() && !w.reflects());
        let p = Loadout::new(vec![ModuleKind::PointDefenseScreen]);
        assert!(p.has_pd());
    }
}

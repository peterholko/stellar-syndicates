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
//! Modules are physical manufactured items, shipped by raidable freighters.
//! Military crates use an Armaments Complex and refit at an Ordnance Foundry;
//! utilities manufacture and refit at a staffed Shipyard I. This module owns
//! the CATALOG + the damage-typing math
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
// §tactical supersession: the side-level `PD_INTERCEPT` share-math is DELETED.
// PD is LITERAL now — each PD-fitted ship rolls intercepts against torpedoes
// crossing its screen bubble ([`crate::tactical::PD_ROLL_BASE`] × Interception
// affinity × the Flak-research mod, wired through `SideMods::flak_mult`).
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

/// §modules Part B3 (Sol hub): Sol SELLS modules to players at this multiple of
/// the module's goods-recipe VALUE (its commodity cost priced at Sol's standing
/// market). The 2× premium means local manufacture at the module's workshop is
/// always cheaper — Sol is the BOOTSTRAP / fallback, never the efficient path.
/// Tunable.
pub const MODULE_BUY_MULT: f64 = 2.0;
/// §modules Part B3 (Sol hub): Sol BUYS modules back at this multiple of the same
/// recipe value — a deliberately steep 4× round-trip spread (buy 2×, sell 0.5×)
/// so modules are for FITTING, not arbitrage. Tunable.
pub const MODULE_SELL_MULT: f64 = 0.5;

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
    /// Utility: additional tank volume on this hull, not free fuel.
    ExtendedTanks,
    /// Utility: better local contacts and expedition threat detection.
    ReconSuite,
    /// Utility: double the fitted player Freighter's commodity hold.
    CargoPods,
    /// Utility fire control: a Corvette's fitted PD screens its guarded fleet.
    EscortDatalink,
    /// Utility: pump physical Fuel cargo into another owned fleet's tanks.
    FuelTransferRig,
    /// Discovery blueprint: faster Scout travel at the expense of tank volume.
    SurveyDrive,
    /// Discovery blueprint: stationary-site sensing, no combat sensor bubble.
    NebulaSpectrometer,
    /// Discovery blueprint: powerful beam, three fitting points; mirrors counter it.
    PrismaticLance,
}

/// Every module kind, in a fixed deterministic order (menus, iteration).
pub const MODULE_KINDS: [ModuleKind; 13] = [
    ModuleKind::MassDriver,
    ModuleKind::TorpedoRack,
    ModuleKind::PointDefenseScreen,
    ModuleKind::ReflectivePlating,
    ModuleKind::WhippleArmor,
    ModuleKind::ExtendedTanks,
    ModuleKind::ReconSuite,
    ModuleKind::CargoPods,
    ModuleKind::EscortDatalink,
    ModuleKind::FuelTransferRig,
    ModuleKind::SurveyDrive,
    ModuleKind::NebulaSpectrometer,
    ModuleKind::PrismaticLance,
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
            ModuleKind::ExtendedTanks => "extended_tanks",
            ModuleKind::ReconSuite => "recon_suite",
            ModuleKind::CargoPods => "cargo_pods",
            ModuleKind::EscortDatalink => "escort_datalink",
            ModuleKind::FuelTransferRig => "fuel_transfer_rig",
            ModuleKind::SurveyDrive => "survey_drive",
            ModuleKind::NebulaSpectrometer => "nebula_spectrometer",
            ModuleKind::PrismaticLance => "prismatic_lance",
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
            ModuleKind::ExtendedTanks => "Extended Tanks",
            ModuleKind::ReconSuite => "Recon Suite",
            ModuleKind::CargoPods => "Cargo Pods",
            ModuleKind::EscortDatalink => "Escort Datalink",
            ModuleKind::FuelTransferRig => "Fuel Transfer Rig",
            ModuleKind::SurveyDrive => "Survey Drive",
            ModuleKind::NebulaSpectrometer => "Nebula Spectrometer",
            ModuleKind::PrismaticLance => "Prismatic Lance",
        }
    }

    /// Is this a WEAPON module (determines the ship's offense type)?
    pub fn is_weapon(self) -> bool {
        matches!(
            self,
            ModuleKind::MassDriver | ModuleKind::TorpedoRack | ModuleKind::PointDefenseScreen | ModuleKind::PrismaticLance
        )
    }

    pub fn is_utility(self) -> bool {
        matches!(self, Self::ExtendedTanks | Self::ReconSuite | Self::CargoPods | Self::EscortDatalink | Self::FuelTransferRig | Self::SurveyDrive | Self::NebulaSpectrometer)
    }

    pub fn blueprint_only(self) -> bool {
        matches!(self, Self::SurveyDrive | Self::NebulaSpectrometer | Self::PrismaticLance)
    }

    pub fn fits_hull(self, kind: crate::ship::ShipKind) -> bool {
        use crate::ship::ShipKind;
        match self {
            Self::ExtendedTanks => matches!(kind, ShipKind::Scout | ShipKind::Raider | ShipKind::Corvette) || kind.is_player_freighter(),
            Self::ReconSuite => matches!(kind, ShipKind::Scout | ShipKind::Raider),
            Self::CargoPods | Self::FuelTransferRig => kind.is_player_freighter(),
            Self::EscortDatalink => kind == ShipKind::Corvette,
            Self::SurveyDrive | Self::NebulaSpectrometer => kind == ShipKind::Scout,
            // The player's freighter gains a utility berth, never a hardpoint.
            _ => !kind.is_player_freighter(),
        }
    }

    pub fn workshop(self) -> crate::build::StructureKind {
        if self.is_utility() { crate::build::StructureKind::Shipyard }
        else { crate::build::StructureKind::ArmamentsComplex }
    }

    /// §fitting: the module's FITTING-POINT cost — the capacity a hull spends to
    /// carry it. Depth is ALLOCATION, not marks: no numbered tiers, ever. The
    /// costs are tuned so the classic trade-offs bind on subcapitals (a torpedo
    /// Corvette can't also take heavy armor; a torpedo Raider is a glass
    /// cannon) — see `ship::fitting_points` for the per-hull budgets. Tunable.
    pub fn fitting_cost(self) -> u32 {
        match self {
            ModuleKind::MassDriver => 2,
            ModuleKind::TorpedoRack => 3,
            ModuleKind::PointDefenseScreen => 2,
            ModuleKind::ReflectivePlating => 2,
            ModuleKind::WhippleArmor => 3,
            ModuleKind::PrismaticLance => 3,
            ModuleKind::SurveyDrive | ModuleKind::NebulaSpectrometer => 2,
            ModuleKind::ExtendedTanks | ModuleKind::ReconSuite | ModuleKind::CargoPods | ModuleKind::EscortDatalink | ModuleKind::FuelTransferRig => 2,
        }
    }

    /// §fitting: the module's FAMILY — the axis hull AFFINITIES key on (a hull
    /// is good at a *kind of fighting*, not at a specific module).
    pub fn family(self) -> Family {
        match self {
            ModuleKind::MassDriver => Family::Driver,
            ModuleKind::TorpedoRack => Family::Torpedo,
            ModuleKind::PrismaticLance => Family::Beam,
            ModuleKind::SurveyDrive | ModuleKind::NebulaSpectrometer => Family::Utility,
            // PD is BOTH a (beam) weapon and the interception screen; its
            // affinity axis is the screening job — the weapon side rides the
            // Beam family through `Loadout::offense()`.
            ModuleKind::PointDefenseScreen => Family::Interception,
            ModuleKind::ReflectivePlating | ModuleKind::WhippleArmor => Family::Protection,
            ModuleKind::ExtendedTanks | ModuleKind::ReconSuite | ModuleKind::CargoPods | ModuleKind::EscortDatalink | ModuleKind::FuelTransferRig => Family::Utility,
        }
    }
}

/// §fitting: module FAMILIES — the axes hull affinities scale. The three weapon
/// families are the damage types; Interception is PD's screening contribution;
/// Protection is the armor pair's mitigation; Utility supplies local support
/// (exploration, tank/hold capacity or escort fire control), never faster reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Beam,
    Driver,
    Torpedo,
    Interception,
    Protection,
    Utility,
}

/// The weapon FAMILY of a damage type (for affinity lookups on the offense side).
pub fn weapon_family(ty: DamageType) -> Family {
    match ty {
        DamageType::Beam => Family::Beam,
        DamageType::Driver => Family::Driver,
        DamageType::Torpedo => Family::Torpedo,
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
        self.0
            .iter()
            .map(|m| m.slug())
            .collect::<Vec<_>>()
            .join("+")
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
    /// decides it (torpedo > driver > prismatic beam > point-defense beam), else a stock beam.
    /// Armor modules never change offense. §fitting: DUPLICATES of the chosen
    /// weapon stack LINEARLY (2× TorpedoRack fires at 2×TORP_MULT) — the
    /// fitting budget is the brake, not a uniqueness rule. A single copy is
    /// bit-identical to the pre-fitting model.
    pub fn offense(&self) -> (DamageType, f64) {
        let count = |k: ModuleKind| self.0.iter().filter(|m| **m == k).count() as f64;
        if self.0.contains(&ModuleKind::TorpedoRack) {
            (
                DamageType::Torpedo,
                TORP_MULT * count(ModuleKind::TorpedoRack),
            )
        } else if self.0.contains(&ModuleKind::MassDriver) {
            (
                DamageType::Driver,
                DRIVER_MULT * count(ModuleKind::MassDriver),
            )
        } else if self.0.contains(&ModuleKind::PrismaticLance) {
            (DamageType::Beam, 1.45 * count(ModuleKind::PrismaticLance))
        } else if self.0.contains(&ModuleKind::PointDefenseScreen) {
            // PD's beam does NOT stack — extra screens add interception
            // presence, not gunnery (the weapon side is the trade-off).
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

    /// Total FITTING-POINT cost. Military duplicates can stack; each utility
    /// is limited to one per hull as well as consuming slots and points.
    pub fn fitting_cost(&self) -> u32 {
        self.0.iter().map(|m| m.fitting_cost()).sum()
    }

    /// §fitting: is this loadout LEGAL on `kind`? Both constraints at once —
    /// the hull's module SLOTS and its FITTING-POINT budget. Enforced at build,
    /// refit, and fit-save; deliberately NOT on snapshot load (grandfathering:
    /// pre-fitting stacks that exceed a new budget keep flying — budgets bind
    /// only when a player next builds or refits).
    pub fn validate(&self, kind: crate::ship::ShipKind) -> bool {
        self.0.len() as u32 <= kind.module_slots()
            && self.fitting_cost() <= crate::ship::fitting_points(kind)
            && (!self.has_datalink() || self.has_pd())
            && self.0.iter().all(|m| m.fits_hull(kind)
                && (!m.is_utility() || self.0.iter().filter(|other| *other == m).count() == 1))
    }

    pub fn has_recon(&self) -> bool { self.0.contains(&ModuleKind::ReconSuite) }

    pub fn has_datalink(&self) -> bool { self.0.contains(&ModuleKind::EscortDatalink) }

    /// Tunable: +75% on the equipped hull only. Identical duplicates cannot stack.
    pub fn fuel_capacity_mult(&self) -> f64 {
        let tanks = if self.0.contains(&ModuleKind::ExtendedTanks) { 1.75 } else { 1.0 };
        tanks * if self.0.contains(&ModuleKind::SurveyDrive) { 0.6 } else { 1.0 }
    }

    pub fn speed_mult(&self) -> f64 {
        if self.0.contains(&ModuleKind::SurveyDrive) { 1.25 } else { 1.0 }
    }

    pub fn has_spectrometer(&self) -> bool { self.0.contains(&ModuleKind::NebulaSpectrometer) }

    /// Tunable: double commodity capacity on the fitted Freighter only. Its
    /// single utility slot makes this a choice against Extended Tanks.
    pub fn cargo_capacity_mult(&self) -> u32 {
        if self.0.contains(&ModuleKind::CargoPods) { 2 } else { 1 }
    }

    /// Only changed modules determine the work: a utility swap may retain an
    /// existing weapon, but cannot install/remove that weapon at a basic yard.
    pub fn utility_change_to(&self, to: &Self) -> bool {
        MODULE_KINDS.into_iter().filter(|m| !m.is_utility()).all(|m|
            self.0.iter().filter(|v| **v == m).count() == to.0.iter().filter(|v| **v == m).count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torpedo_corvette_cannot_take_heavy_armor() {
        // §fitting relationship 1: Torp(3) + Whipple(3) = 6 > the Corvette's 5.
        let lo = Loadout::new(vec![ModuleKind::TorpedoRack, ModuleKind::WhippleArmor]);
        assert_eq!(lo.fitting_cost(), 6);
        assert!(
            !lo.validate(crate::ship::ShipKind::Corvette),
            "torpedo Corvette can't also armor up"
        );
    }

    #[test]
    fn driver_whipple_corvette_is_the_classic_brawler() {
        // §fitting relationship 2: Driver(2) + Whipple(3) = 5 fits EXACTLY.
        let lo = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]);
        assert_eq!(
            lo.fitting_cost(),
            crate::ship::fitting_points(crate::ship::ShipKind::Corvette)
        );
        assert!(lo.validate(crate::ship::ShipKind::Corvette));
    }

    #[test]
    fn torpedo_raider_is_a_glass_cannon() {
        // §fitting relationship 3: Torp(3) on the Raider's 4 leaves 1 point —
        // no module costs 1 today, so the rack flies alone. Intended.
        let torp = Loadout::new(vec![ModuleKind::TorpedoRack]);
        assert!(torp.validate(crate::ship::ShipKind::Raider));
        for extra in MODULE_KINDS {
            let lo = Loadout::new(vec![ModuleKind::TorpedoRack, extra]);
            assert!(
                !lo.validate(crate::ship::ShipKind::Raider),
                "torp + {extra:?} must overflow the Raider budget"
            );
        }
    }

    #[test]
    fn validate_binds_both_slots_and_budget() {
        use crate::ship::ShipKind;
        // Slots bind even when the budget would allow: the Scout has 1 slot.
        let two_cheap = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::ReflectivePlating]);
        assert!(two_cheap.fitting_cost() <= crate::ship::fitting_points(ShipKind::Corvette));
        assert!(
            !two_cheap.validate(ShipKind::Scout),
            "2 modules > the Scout's 1 slot"
        );
        assert!(Loadout::new(vec![ModuleKind::MassDriver]).validate(ShipKind::Scout));
        // Logistics hulls carry nothing (0 slots) regardless of budget.
        assert!(!Loadout::new(vec![ModuleKind::MassDriver]).validate(ShipKind::Convoy));
        // The empty loadout is legal everywhere (stock is always allowed).
        assert!(Loadout::default().validate(ShipKind::Convoy));
        assert!(Loadout::default().validate(ShipKind::Corvette));
    }

    #[test]
    fn key_round_trips_and_canonicalizes() {
        let a = Loadout::new(vec![ModuleKind::WhippleArmor, ModuleKind::MassDriver]);
        let b = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]);
        assert_eq!(a, b, "order-insensitive canonical form");
        assert_eq!(a.key(), "mass_driver+whipple_armor");
        assert_eq!(Loadout::from_key(&a.key()), a, "key round-trips");
        assert_eq!(
            Loadout::from_key(""),
            Loadout::default(),
            "empty key = unfitted"
        );
        assert!(Loadout::default().is_empty());
    }

    #[test]
    fn offense_type_is_the_weapon_module() {
        assert_eq!(Loadout::default().offense(), (DamageType::Beam, 1.0));
        assert_eq!(
            Loadout::new(vec![ModuleKind::MassDriver]).offense(),
            (DamageType::Driver, DRIVER_MULT)
        );
        assert_eq!(
            Loadout::new(vec![ModuleKind::TorpedoRack]).offense(),
            (DamageType::Torpedo, TORP_MULT)
        );
        assert_eq!(
            Loadout::new(vec![ModuleKind::PointDefenseScreen]).offense(),
            (DamageType::Beam, PD_ATTACK)
        );
        // Armor alone doesn't change offense; a weapon + armor keeps the weapon.
        assert_eq!(
            Loadout::new(vec![ModuleKind::ReflectivePlating]).offense(),
            (DamageType::Beam, 1.0)
        );
        assert_eq!(
            Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::WhippleArmor]).offense(),
            (DamageType::Driver, DRIVER_MULT)
        );
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

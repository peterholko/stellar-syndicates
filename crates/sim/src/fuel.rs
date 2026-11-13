//! Movement sink: fleets BURN FUEL to move, out of stores they carry
//! (§fuel-overhaul, §shipbuilding-overhaul, §reactors-drives). Every hull
//! carries engines — a REACTOR feeding a WARP DRIVE, one pair per
//! `ShipKind::engines()` — and the reactor's rung names the fuel:
//!
//! - **Fission Reactor** burns Uranium (1 energy a unit). Corporations mill it
//!   at home from Ferrite Ore. The charter's rung.
//! - **Fusion Reactor** burns Deuterium (4 a unit).
//! - **Advanced Fusion Reactor** burns Helium-3 (24 a unit).
//! - **Antimatter Reactor** burns Antimatter (96 a unit).
//!
//! The drive — Zephyr, Gale, Tempest, Maelstrom — sets the speed (×1 / 1.2 /
//! 1.4 / 1.6) and what the hull can do (Stealth from a Gale, the jump on a
//! Maelstrom), and needs a reactor of at least its rung. Speed is a THROTTLE on
//! any engine: Full at rated speed, Slow at half speed for a quarter of the
//! energy per distance. A fleet carries one tank per fuel kind aboard, sized in
//! ENERGY per unit of hull mass, doubling a reactor rung. A fleet is fuelled
//! while every kind aboard has fuel; otherwise it holds position (never
//! breaks). All rates are `const`.

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;

/// What kind of operation a fuel shortfall held (for the owner-only notice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortfallKind {
    /// A player-issued ship move.
    Move,
    /// A discrete jump-drive relocation.
    Jump,
    /// A player-issued raid intercept.
    Raid,
    /// A production "ship to hub" convoy.
    Shipment,
}

impl ShortfallKind {
    pub fn label(self) -> &'static str {
        match self {
            ShortfallKind::Move => "fleet move",
            ShortfallKind::Jump => "jump",
            ShortfallKind::Raid => "raid",
            ShortfallKind::Shipment => "shipment",
        }
    }
}

/// Fuel burned per (unit of straight-line distance × unit of fleet mass) at
/// dispatch. Tiny because mass (thousands) × distance (thousands) is large — a
/// loaded convoy crossing the core costs a few dozen units. Tunable.
pub const FUEL_PER_MASS_DISTANCE: f64 = 1.0e-6;

/// §fuel-overhaul: the founding kit's fuel, seeded into the HOME stockpile on
/// join: Uranium for slow freight until the home's mill runs, and a little
/// Deuterium for a full-speed trip when time is critical. Tunable.
///
/// Sized against STORAGE, because that is what actually binds it: with the
/// Uranium Mill's goods and a Warp Core added to the kit, the fuel seed shrinks
/// from the old 60 Fuel to 30, so the kit holds as many units as before. The
/// founding fleet arrives with full stores of its own.
/// A system holds 700 units of everything combined and a fresh home already
/// carries ~190 of starter goods, so a seed scaled for range (1,500, clipped to
/// the 510 that would fit) did not buy a runway — it filled the warehouse. Ore
/// came out of the ground with nowhere to go and the colony never built a thing.
///
/// That went unnoticed while fuel was drawn from stockpiles: movement drained it
/// continuously, so the silo emptied itself. Once fleets carry their own, the
/// seed just sits there, and its real cost shows up. This leaves room for the
/// economy to breathe while still covering a couple of full refills.
/// §shipbuilding-overhaul (Nov 6 2026): no Deuterium seed — no charter hull is
/// a fusion hull.
pub const URANIUM_HOME_SEED: f64 = 20.0;

/// The Fusion Core's fuel, and Fusion Plants III–IV's. Standing-order convoys
/// hauling it skip the tank pre-check, so a dry colony can always be resupplied.
pub const MOVEMENT_FUEL: Commodity = Commodity::Deuterium;
/// The Fission Core's fuel.
pub const FISSION_FUEL: Commodity = Commodity::Uranium;
/// The Fold Core's fuel: cruise and jumps.
pub const JUMP_FUEL: Commodity = Commodity::Antimatter;

/// §fuel-overhaul: one unit of Helium-3 holds this many Deuterium of energy.
/// Per unit of mass D–He3 fusion yields about four times D–D, and nearly all of
/// it charged; D–D loses about a third to neutrons, so the usable ratio is about
/// six. §shipbuilding-overhaul: the Helium Core's density is this times the
/// Fusion Core's. Tunable.
pub const HELIUM3_FUEL_EQUIV: f64 = 6.0;

/// Slow runs at this fraction of formation speed…
pub const SLOW_SPEED_FRACTION: f64 = 0.5;
/// …and burns this fraction of Full's energy per distance, on any core. A slow
/// trip on Uranium (5.6 Cr) therefore costs what the old Economy cruise did;
/// Full "just burns four times more" (Peter, Nov 6 2026). Tunable.
pub const SLOW_FUEL_MULT: f64 = 0.25;

/// §reactors-drives (docs/reactor-drive-split.md): the REACTOR a hull's
/// engines run on, by rung. One fuel per rung; the rung is chosen at build and
/// changed by a yard refit (`Command::RefitEngines`). The order is the ladder.
/// A reactor powers any drive of its rung or below ([`Self::powers`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactorGrade {
    #[default]
    Fission,
    Fusion,
    /// The Advanced Fusion Reactor, on Helium-3.
    Helium,
    Antimatter,
}

/// The research that makes rung `rung` (2–4) known, buyable and buildable:
/// Bunkerage (Propulsion I) for Fusion, Helium-3 Cycle for Helium, Antimatter
/// Synthesis for Antimatter. Rung 1 needs none. Reactor, drive and fuel of a
/// rung arrive together, so nothing is ever shown that nothing aboard can
/// power (Peter, Nov 7 2026: "the player should not even know about the other
/// fuel types until it has been unlocked from research").
pub fn rung_programme(rung: u8) -> Option<&'static str> {
    match rung {
        2 => Some("prop_bunkerage"),
        3 => Some(crate::power::FUSION_RESEARCH),
        4 => Some("mat_foundry_vi_antimatter_synthesis"),
        _ => None,
    }
}

/// The research a good waits for, if any: a reactor, a drive or a fuel of
/// rung 2–4. Everything else is known from the start.
pub fn known_after(c: Commodity) -> Option<&'static str> {
    // A sized part waits for its light base's rung (and its size, see
    // `size_known_after`).
    let c = sized_part(c).map_or(c, |(_, base)| base);
    if let Some(g) = ReactorGrade::of_commodity(c).or_else(|| ReactorGrade::burning(c)) {
        return rung_programme(g.rung());
    }
    Drive::of_commodity(c).and_then(|d| rung_programme(d.rung()))
}

impl ReactorGrade {
    pub const ALL: [ReactorGrade; 4] = [ReactorGrade::Fission, ReactorGrade::Fusion, ReactorGrade::Helium, ReactorGrade::Antimatter];

    /// 1–4.
    pub fn rung(self) -> u8 {
        match self {
            ReactorGrade::Fission => 1,
            ReactorGrade::Fusion => 2,
            ReactorGrade::Helium => 3,
            ReactorGrade::Antimatter => 4,
        }
    }

    pub fn of_rung(rung: u8) -> Option<ReactorGrade> {
        Self::ALL.into_iter().find(|g| g.rung() == rung)
    }

    /// The fuel this reactor burns.
    pub fn fuel(self) -> Commodity {
        match self {
            ReactorGrade::Fission => Commodity::Uranium,
            ReactorGrade::Fusion => Commodity::Deuterium,
            ReactorGrade::Helium => Commodity::Helium3,
            ReactorGrade::Antimatter => Commodity::Antimatter,
        }
    }

    /// The reactor good a hull of this rung is built from.
    pub fn commodity(self) -> Commodity {
        match self {
            ReactorGrade::Fission => Commodity::FissionReactor,
            ReactorGrade::Fusion => Commodity::FusionReactor,
            ReactorGrade::Helium => Commodity::AdvancedFusionReactor,
            ReactorGrade::Antimatter => Commodity::AntimatterReactor,
        }
    }

    pub fn of_commodity(c: Commodity) -> Option<ReactorGrade> {
        Self::ALL.into_iter().find(|g| g.commodity() == c)
    }

    /// The rung whose reactor burns this fuel.
    pub fn burning(fuel: Commodity) -> Option<ReactorGrade> {
        Self::ALL.into_iter().find(|g| g.fuel() == fuel)
    }

    /// ENERGY a unit of this rung's fuel holds, in Uranium. Chosen so a fission
    /// hull on Slow burns exactly what slow warp burned before the overhaul
    /// (SLOW_FUEL_MULT recast as Uranium 1 : Deuterium 4), Helium-3 keeps its
    /// ×6 over Deuterium, and Antimatter ×4 over that. Tunable.
    pub fn energy_per_unit(self) -> f64 {
        match self {
            ReactorGrade::Fission => 1.0,
            ReactorGrade::Fusion => 4.0,
            ReactorGrade::Helium => 4.0 * HELIUM3_FUEL_EQUIV,
            ReactorGrade::Antimatter => 16.0 * HELIUM3_FUEL_EQUIV,
        }
    }

    /// ENERGY aboard per unit of hull mass, relative to Fission: a better
    /// reactor stores more as well as denser. Doubles a rung, so a matched
    /// reactor-and-drive pair gains range as well as speed (`Drive::energy_mult`
    /// grows ×1.44–2.56 up the ladder). Tunable.
    pub fn energy_aboard(self) -> f64 {
        match self {
            ReactorGrade::Fission => 1.0,
            ReactorGrade::Fusion => 2.0,
            ReactorGrade::Helium => 4.0,
            ReactorGrade::Antimatter => 8.0,
        }
    }

    /// Tank size per unit of HULL mass, in units of this rung's fuel:
    /// `FUEL_PER_HULL_MASS` of Uranium-energy × `energy_aboard`, in the fuel's
    /// own units. An Antimatter tank is at least its jump bottle
    /// (`ANTIMATTER_PER_HULL_MASS`).
    pub fn tank_per_hull_mass(self) -> f64 {
        let cruise = FUEL_PER_HULL_MASS * self.energy_aboard() / self.energy_per_unit();
        if self == ReactorGrade::Antimatter { cruise.max(ANTIMATTER_PER_HULL_MASS) } else { cruise }
    }

    /// The power rule: a reactor runs a drive of its rung or below.
    pub fn powers(self, drive: Drive) -> bool { self.rung() >= drive.rung() }

    /// The best drive this reactor can run.
    pub fn best_drive(self) -> Drive {
        Drive::of_rung(self.rung()).unwrap_or_default()
    }

    pub fn label(self) -> &'static str {
        match self {
            ReactorGrade::Fission => "Fission Reactor",
            ReactorGrade::Fusion => "Fusion Reactor",
            ReactorGrade::Helium => "Advanced Fusion Reactor",
            ReactorGrade::Antimatter => "Antimatter Reactor",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            ReactorGrade::Fission => "fission",
            ReactorGrade::Fusion => "fusion",
            ReactorGrade::Helium => "helium",
            ReactorGrade::Antimatter => "antimatter",
        }
    }
}

/// §reactors-drives: the WARP DRIVE a hull's engines turn, by rung — the winds
/// of the age of sail the charter and privateer lore borrow, rising in force
/// (Peter, Nov 7 2026: "4 levels of warp drives … something more creative"
/// than level 1–4). A drive needs a reactor of at least its rung. Speed rises
/// a rung and energy a distance rises with the square of it, the law the
/// throttle already obeys, so a faster drive is a hungrier one and "needs more
/// power" is literal. Chosen at build, changed by a yard refit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Drive {
    #[default]
    Zephyr,
    Gale,
    Tempest,
    Maelstrom,
}

impl Drive {
    pub const ALL: [Drive; 4] = [Drive::Zephyr, Drive::Gale, Drive::Tempest, Drive::Maelstrom];

    /// 1–4.
    pub fn rung(self) -> u8 {
        match self {
            Drive::Zephyr => 1,
            Drive::Gale => 2,
            Drive::Tempest => 3,
            Drive::Maelstrom => 4,
        }
    }

    pub fn of_rung(rung: u8) -> Option<Drive> {
        Self::ALL.into_iter().find(|d| d.rung() == rung)
    }

    /// The drive good a hull of this rung is built from.
    pub fn commodity(self) -> Commodity {
        match self {
            Drive::Zephyr => Commodity::ZephyrDrive,
            Drive::Gale => Commodity::GaleDrive,
            Drive::Tempest => Commodity::TempestDrive,
            Drive::Maelstrom => Commodity::MaelstromDrive,
        }
    }

    pub fn of_commodity(c: Commodity) -> Option<Drive> {
        Self::ALL.into_iter().find(|d| d.commodity() == c)
    }

    /// The multiplier on rated speed. The ladder stops where the light-game
    /// invariant does: a Scout (115 su/s) on a Maelstrom must stay under
    /// `c / C_SPEED_RATIO` = 200 su/s (`config::SimConfig`). Tunable within that.
    pub fn speed_mult(self) -> f64 {
        match self {
            Drive::Zephyr => 1.0,
            Drive::Gale => 1.2,
            Drive::Tempest => 1.4,
            Drive::Maelstrom => 1.6,
        }
    }

    /// The multiplier on ENERGY a distance: the square of the speed, as Slow is
    /// half the speed for a quarter of the burn.
    pub fn energy_mult(self) -> f64 {
        self.speed_mult() * self.speed_mult()
    }

    /// The lowest reactor that powers this drive.
    pub fn min_reactor(self) -> ReactorGrade {
        ReactorGrade::of_rung(self.rung()).unwrap_or_default()
    }

    /// Stealth needs a Gale or better: the first upgrade buys something
    /// besides speed.
    pub fn can_stealth(self) -> bool { self >= Drive::Gale }

    /// Only a Maelstrom Drive jumps.
    pub fn jumps(self) -> bool { self == Drive::Maelstrom }

    pub fn label(self) -> &'static str {
        match self {
            Drive::Zephyr => "Zephyr Drive",
            Drive::Gale => "Gale Drive",
            Drive::Tempest => "Tempest Drive",
            Drive::Maelstrom => "Maelstrom Drive",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Drive::Zephyr => "zephyr",
            Drive::Gale => "gale",
            Drive::Tempest => "tempest",
            Drive::Maelstrom => "maelstrom",
        }
    }
}

/// A hull's engine: its reactor and its drive.
pub type Engine = (ReactorGrade, Drive);

/// §hull-ladder (docs/hull-ladder-plan.md): the SIZE of a hull's engines and
/// frame — light, heavy or capital, each ×25 the one below in price and mass,
/// assembled at the works from 25 of the size below. A hull's size is fixed
/// by its kind (`ShipKind::engine_size`); the rung (fuel) is chosen at build.
/// Like EVE's small / large / capital components: a Titan's hundred capital
/// engines are 62,500 light engines' worth of Drive Works time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSize {
    #[default]
    Light,
    Heavy,
    Capital,
}

/// Parts assembled into one of the next size.
pub const SIZE_ASSEMBLY: u32 = 25;
/// §hull-ladder: the assembly's own margin, a step. A heavy part is priced
/// at its 25 light parts and two percent; a capital part at its 25 heavy
/// parts and two percent again (so the Drive Works clears its basket, as
/// every converter must). Mass takes no fee.
pub const SIZE_ASSEMBLY_FEE: f64 = 0.02;

impl EngineSize {
    pub const ALL: [EngineSize; 3] = [EngineSize::Light, EngineSize::Heavy, EngineSize::Capital];

    /// Mass (and parts assembled) relative to a light part.
    pub fn factor(self) -> f64 {
        match self {
            EngineSize::Light => 1.0,
            EngineSize::Heavy => SIZE_ASSEMBLY as f64,
            EngineSize::Capital => (SIZE_ASSEMBLY * SIZE_ASSEMBLY) as f64,
        }
    }

    /// Price relative to a light part: the parts assembled and the
    /// assembly's fee a step (`SIZE_ASSEMBLY_FEE`).
    pub fn price_factor(self) -> f64 {
        let steps = match self {
            EngineSize::Light => 0,
            EngineSize::Heavy => 1,
            EngineSize::Capital => 2,
        };
        self.factor() * (1.0 + SIZE_ASSEMBLY_FEE).powi(steps)
    }

    /// The size one step down, for the assembly recipe.
    pub fn below(self) -> Option<EngineSize> {
        match self {
            EngineSize::Light => None,
            EngineSize::Heavy => Some(EngineSize::Light),
            EngineSize::Capital => Some(EngineSize::Heavy),
        }
    }

    /// The research that makes a size known, buyable and buildable: heavy
    /// parts with the Cruiser's line, capital parts with the Dreadnought's.
    /// Light parts need none.
    pub fn programme(self) -> Option<&'static str> {
        match self {
            EngineSize::Light => None,
            EngineSize::Heavy => Some("hull_line_v_cruiser"),
            EngineSize::Capital => Some("hull_line_vii_dreadnought"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EngineSize::Light => "Light",
            EngineSize::Heavy => "Heavy",
            EngineSize::Capital => "Capital",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            EngineSize::Light => "light",
            EngineSize::Heavy => "heavy",
            EngineSize::Capital => "capital",
        }
    }

    /// The reactor good of `grade` at this size.
    pub fn reactor(self, grade: ReactorGrade) -> Commodity {
        use Commodity as C;
        match (self, grade) {
            (EngineSize::Light, g) => g.commodity(),
            (EngineSize::Heavy, ReactorGrade::Fission) => C::HeavyFissionReactor,
            (EngineSize::Heavy, ReactorGrade::Fusion) => C::HeavyFusionReactor,
            (EngineSize::Heavy, ReactorGrade::Helium) => C::HeavyAdvancedFusionReactor,
            (EngineSize::Heavy, ReactorGrade::Antimatter) => C::HeavyAntimatterReactor,
            (EngineSize::Capital, ReactorGrade::Fission) => C::CapitalFissionReactor,
            (EngineSize::Capital, ReactorGrade::Fusion) => C::CapitalFusionReactor,
            (EngineSize::Capital, ReactorGrade::Helium) => C::CapitalAdvancedFusionReactor,
            (EngineSize::Capital, ReactorGrade::Antimatter) => C::CapitalAntimatterReactor,
        }
    }

    /// The drive good of `drive` at this size.
    pub fn drive(self, drive: Drive) -> Commodity {
        use Commodity as C;
        match (self, drive) {
            (EngineSize::Light, d) => d.commodity(),
            (EngineSize::Heavy, Drive::Zephyr) => C::HeavyZephyrDrive,
            (EngineSize::Heavy, Drive::Gale) => C::HeavyGaleDrive,
            (EngineSize::Heavy, Drive::Tempest) => C::HeavyTempestDrive,
            (EngineSize::Heavy, Drive::Maelstrom) => C::HeavyMaelstromDrive,
            (EngineSize::Capital, Drive::Zephyr) => C::CapitalZephyrDrive,
            (EngineSize::Capital, Drive::Gale) => C::CapitalGaleDrive,
            (EngineSize::Capital, Drive::Tempest) => C::CapitalTempestDrive,
            (EngineSize::Capital, Drive::Maelstrom) => C::CapitalMaelstromDrive,
        }
    }

    /// The frame good at this size.
    pub fn hull_sections(self) -> Commodity {
        match self {
            EngineSize::Light => Commodity::HullSections,
            EngineSize::Heavy => Commodity::HeavyHullSections,
            EngineSize::Capital => Commodity::CapitalHullSections,
        }
    }
}

/// A sized part's size and its light base good: `(size, base)`; None for
/// anything else.
pub fn sized_part(c: Commodity) -> Option<(EngineSize, Commodity)> {
    for size in EngineSize::ALL {
        for g in ReactorGrade::ALL {
            if size.reactor(g) == c { return Some((size, g.commodity())); }
        }
        for d in Drive::ALL {
            if size.drive(d) == c { return Some((size, d.commodity())); }
        }
        if size.hull_sections() == c { return Some((size, Commodity::HullSections)); }
    }
    None
}

/// The research a part's SIZE waits for (None for light parts and everything
/// else). A heavy rung-3 part waits for both this and `known_after`.
pub fn size_known_after(c: Commodity) -> Option<&'static str> {
    sized_part(c).and_then(|(size, _)| size.programme())
}

/// The engine scripted spawns and tests use for a rung: the reactor with the
/// best drive it powers.
pub fn engine_of(reactor: ReactorGrade) -> Engine { (reactor, reactor.best_drive()) }

/// The ENERGY (in Uranium) a fleet of `mass` burns in one tick at `base_speed`
/// on `Full`; Slow applies `SLOW_FUEL_MULT`. Fuel units follow from the grade's
/// `energy_per_unit`.
pub fn energy_tick(mass: f64, base_speed: f64, dt: f64) -> f64 {
    fuel_tick(mass, base_speed, dt)
}

/// §fuel-overhaul: the Fusion Freight research: player freighters burn this
/// fraction of the Deuterium (and Helium-3) per distance, so full-speed freight
/// costs what slow freight does. Tunable.
pub const FUSION_FREIGHT_MULT: f64 = 0.1;

/// Antimatter bottle per unit of hull mass on a Fold hull, at least
/// [`ANTIMATTER_BOTTLE_MIN`] per hull: two full-range jumps. Tunable.
pub const ANTIMATTER_PER_HULL_MASS: f64 = 0.01;
pub const ANTIMATTER_BOTTLE_MIN: f64 = 1.0;

/// Tank size per unit of HULL mass.
///
/// Burn is `FUEL_PER_MASS_DISTANCE × mass × base_speed × dt`, so a full tank
/// buys `FUEL_PER_HULL_MASS × factor / FUEL_PER_MASS_DISTANCE` of distance —
/// mass cancels, and range depends on the active cruise regime. Calibrated so a
/// full formation can cross roughly two galaxy radii in warp: enough that a
/// campaign still has to account for its return fuel.
pub const FUEL_PER_HULL_MASS: f64 = 0.035;

/// Fuel a fleet of `mass` burns to traverse `distance`. Deterministic; clamps
/// negatives to zero so a degenerate (already-at-destination) dispatch is free.
/// Fuel burned in ONE TICK by a fleet under way.
///
/// The rate is set by the hull's OWN rated speed — before any medium factor —
/// so warp makes the same engine-seconds cover more ground. Integrating over a
/// leg, `v_base` cancels
/// and the total comes to `k · mass · length / factor`: one cause producing both
/// the speed benefit and the fuel benefit, rather than two bolted-on bonuses.
///
/// Zero when idle, and it picks up the Stealth throttle for free — half speed
/// burns half per second and takes twice as long, netting the same total.
/// Slow warp applies its separate per-distance multiplier at the fleet
/// caller; this primitive remains unchanged for Full, Stealth and combat hulls.
pub fn fuel_tick(mass: f64, base_speed: f64, dt: f64) -> f64 {
    FUEL_PER_MASS_DISTANCE * mass.max(0.0) * base_speed.max(0.0) * dt.max(0.0)
}

pub fn fuel_cost(distance: f64, mass: f64) -> f64 {
    FUEL_PER_MASS_DISTANCE * distance.max(0.0) * mass.max(0.0)
}

/// Conservative dispatch estimate, shared by onboard freight and CC planning.
/// Inputs are public well positions and the caller's known mass/modifier; this
/// never reads a remote fleet, depot or market. Include the hub in `wells`.
/// Merge overlapping wells so they are charged once. A small transition budget
/// covers spooling/turning; the 15% margin is an estimate, not extra physical burn.
pub fn route_estimate(from: crate::Vec2, to: crate::Vec2, mass: f64,
    consumption: f64, wells: impl IntoIterator<Item = crate::Vec2>) -> f64
{
    let distance = from.distance(to);
    if distance < 1.0 { return 0.0; }
    let (inside, crossings) = crate::transit::well_legs(from, to, wells);
    let engine_distance = inside + (distance - inside) / crate::transit::WARP_FACTOR
        + 250.0 * (1 + crossings) as f64;
    fuel_cost(engine_distance, mass) * consumption.max(0.0) * 1.15
}

#[cfg(test)]
mod planning_tests {
    use super::*;
    use crate::Vec2;

    #[test]
    fn hub_well_and_cargo_are_in_the_dispatch_budget() {
        let home = Vec2::new(70_000.0, 0.0);
        let hub = Vec2::ZERO;
        let one = route_estimate(home, hub, 1500.0, 1.0, [home]);
        let both = route_estimate(home, hub, 1500.0, 1.0, [home, hub]);
        assert!(both > one);
        assert_eq!(both, route_estimate(home, hub, 1500.0, 1.0, [home, hub, hub]));
        assert!(route_estimate(home, hub, 2000.0, 1.0, [home, hub]) > both);
        assert_eq!(route_estimate(home, home, 1500.0, 1.0, [home]), 0.0);
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    #[test]
    fn rungs_name_their_fuel_reactor_and_drive_and_keep_todays_slow_uranium_burn() {
        for g in ReactorGrade::ALL {
            assert_eq!(ReactorGrade::of_commodity(g.commodity()), Some(g));
            assert_eq!(ReactorGrade::burning(g.fuel()), Some(g));
            assert!(g.fuel().is_fuel() && g.commodity().is_reactor());
            let d = g.best_drive();
            assert_eq!(d.rung(), g.rung());
            assert_eq!(Drive::of_commodity(d.commodity()), Some(d));
            assert!(d.commodity().is_drive() && g.powers(d));
            assert_eq!(known_after(g.commodity()), rung_programme(g.rung()));
            assert_eq!(known_after(d.commodity()), rung_programme(g.rung()));
            assert_eq!(known_after(g.fuel()), rung_programme(g.rung()));
        }
        assert_eq!(ReactorGrade::Fission.energy_per_unit(), 1.0);
        assert_eq!(ReactorGrade::Fusion.energy_per_unit(), 1.0 / SLOW_FUEL_MULT);
        assert_eq!(ReactorGrade::Helium.energy_per_unit(), 4.0 * HELIUM3_FUEL_EQUIV);
        assert_eq!(ReactorGrade::Antimatter.energy_per_unit(), 96.0);
        // A Fission hull's tank is today's; energy aboard doubles a rung…
        assert_eq!(ReactorGrade::Fission.tank_per_hull_mass(), FUEL_PER_HULL_MASS);
        for g in [ReactorGrade::Fission, ReactorGrade::Fusion, ReactorGrade::Helium] {
            assert!((g.tank_per_hull_mass() * g.energy_per_unit() - FUEL_PER_HULL_MASS * g.energy_aboard()).abs() < 1e-12);
        }
        // …and the Antimatter tank is at least the jump bottle.
        assert_eq!(ReactorGrade::Antimatter.tank_per_hull_mass(), ANTIMATTER_PER_HULL_MASS);
        assert!(ReactorGrade::Fission < ReactorGrade::Fusion && ReactorGrade::Helium < ReactorGrade::Antimatter);
    }

    #[test]
    fn the_power_rule_and_the_drive_ladder() {
        // A Fusion Reactor runs a Zephyr or a Gale, never a Tempest (Peter: "the
        // player could run a fusion reactor with a warp drive level 1").
        assert!(ReactorGrade::Fusion.powers(Drive::Zephyr) && ReactorGrade::Fusion.powers(Drive::Gale));
        assert!(!ReactorGrade::Fusion.powers(Drive::Tempest) && !ReactorGrade::Fission.powers(Drive::Gale));
        assert!(ReactorGrade::Antimatter.powers(Drive::Maelstrom));
        assert!(!Drive::Zephyr.can_stealth() && Drive::Gale.can_stealth());
        assert!(Drive::Maelstrom.jumps() && !Drive::Tempest.jumps());
        assert_eq!(Drive::Zephyr.speed_mult(), 1.0);
        for d in Drive::ALL {
            assert!((d.energy_mult() - d.speed_mult().powi(2)).abs() < 1e-12);
        }
        assert!(Drive::Zephyr.speed_mult() < Drive::Gale.speed_mult()
            && Drive::Gale.speed_mult() < Drive::Tempest.speed_mult()
            && Drive::Tempest.speed_mult() < Drive::Maelstrom.speed_mult());
        // Rung 1 is known from the start; the rest wait for one programme each.
        assert_eq!(known_after(Commodity::FissionReactor), None);
        assert_eq!(known_after(Commodity::ZephyrDrive), None);
        assert_eq!(known_after(Commodity::Uranium), None);
        assert_eq!(known_after(Commodity::Deuterium), Some("prop_bunkerage"));
        assert_eq!(known_after(Commodity::Alloys), None);
    }
}

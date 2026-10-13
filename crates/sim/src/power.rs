//! §power: PER-PLANET POWER GRIDS (`docs/power-plants-plan.md`).
//!
//! Every planet (moons, belts and gas giants included) runs its own grid.
//! Mines, factories, yards and the Academy draw power; civic structures —
//! food, housing, storage, the Command Center, berths, garrisons, defence and
//! sensors — never do, so a power failure can't starve a colony or silence
//! its guns. Every planet supplies a small base of its own; past that it needs
//! its Power Plant: one per planet, in its own slot. §fuel-overhaul: tiers
//! I–II are Solar Arrays and burn nothing; III–IV are Fusion Plants on
//! Deuterium (with the Authority's Fusion Core); V–VI fuse Helium-3 after
//! Fusion Reactors. Factories burn no fuel; they draw power, and the heat-heavy
//! works (Smelter, Machine Works, Drive Works) draw double. When demand outruns
//! supply, every powered structure on the planet slows by the same share. The
//! base never fails: no deadlock. All numbers are Tunable.

use serde::{Deserialize, Serialize};

use crate::build::StructureKind;
use crate::cargo::Commodity;

/// Every planet's own supply, free and fuel-free (MW).
pub const BASE_MW: f64 = 3.0;
/// A Power Plant's output per unit of its tier throughput (MW).
pub const PLANT_MW_PER_THROUGHPUT: f64 = 8.0;
/// Deuterium a Fusion Plant (III–IV) burns per MW-second it supplies above the
/// base (18 a MWh, the old combustion rate).
pub const FUEL_PER_MW_S: f64 = 0.005;
/// §fuel-overhaul: the first Fusion Plant tier (Deuterium, a Fusion Core per
/// tier). Below it the plant is a Solar Array and burns nothing.
pub const FUSION_CORE_TIER: u32 = 3;
/// The first Helium-3 fusion tier.
pub const FUSION_TIER: u32 = 5;
/// The research that opens the Helium-3 tiers (the plant's tier V–VI unlock),
/// lets fusion tanks take Helium-3 and opens the Drive Works.
pub const FUSION_RESEARCH: &str = "mat_foundry_iv_fusion_reactors";
/// §fuel-overhaul: the heat-heavy works draw this multiple of their tier
/// throughput in MW, in place of the furnace fuel they used to burn.
pub const HEAT_HEAVY_DRAW: f64 = 2.0;
/// §fuel-overhaul: an Antimatter Plant draws ten times a factory: making
/// antimatter is always a net energy loss.
pub const ANTIMATTER_PLANT_DRAW: f64 = 10.0;
/// Power below this counts as a brownout (latched notice).
const BROWNOUT_BELOW: f64 = 0.999;

/// Does a running structure of this kind draw power?
pub fn powered(kind: StructureKind) -> bool {
    use StructureKind as K;
    match kind {
        K::MiningComplex
        | K::VolatileHarvester
        | K::Smelter
        | K::ElectronicsFabricator
        | K::ChemicalWorks
        | K::FuelRefinery
        | K::MachineWorks
        | K::ArmamentsComplex
        | K::CompositeWorks
        | K::HullFabricator
        | K::PrecisionWorks
        | K::DriveWorks
        | K::UraniumMill
        | K::AntimatterPlant
        | K::Shipyard
        | K::NavalDrydock
        | K::CapitalSlipway
        | K::OrdnanceFoundry
        | K::Academy => true,
        K::Bioharvester
        | K::Agroplex
        | K::LivingTerraces
        | K::Habitat
        | K::OrbitalWarehouse
        | K::Warehouse
        | K::SensorArray
        | K::DefensePlatform
        | K::OrbitalDefense
        | K::Garrison
        | K::ServiceBerth
        | K::CommandCenter
        | K::PowerPlant => false,
    }
}

/// The MW a fully crewed structure of `kind` at `tier` draws: its tier
/// throughput, so power follows work, times its heat factor.
pub fn draw_mw(kind: StructureKind, tier: u32) -> f64 {
    if !powered(kind) || tier == 0 {
        return 0.0;
    }
    let factor = match kind {
        StructureKind::Smelter | StructureKind::MachineWorks | StructureKind::DriveWorks => HEAT_HEAVY_DRAW,
        StructureKind::AntimatterPlant => ANTIMATTER_PLANT_DRAW,
        _ => 1.0,
    };
    crate::production::tier_throughput(tier) * factor
}

/// A Power Plant's output at `tier` (0 with none).
pub fn plant_mw(tier: u32) -> f64 {
    if tier == 0 { 0.0 } else { PLANT_MW_PER_THROUGHPUT * crate::production::tier_throughput(tier) }
}

/// §fuel-overhaul: the plant's free SOLAR output. A Fusion Plant keeps its
/// Solar Array II fields, so upgrading never trades free power for fuel: the
/// reactor burns only for output above them, and a dry reactor falls back to
/// them rather than to the planet's base.
pub fn solar_mw(tier: u32) -> f64 {
    plant_mw(tier.min(FUSION_CORE_TIER - 1))
}

/// What a plant at `tier` burns: nothing on solar (I–II), Deuterium at
/// III–IV, Helium-3 at V–VI.
pub fn plant_fuel(tier: u32) -> Option<Commodity> {
    if tier >= FUSION_TIER {
        Some(Commodity::Helium3)
    } else if tier >= FUSION_CORE_TIER {
        Some(Commodity::Deuterium)
    } else { None }
}

/// Units of the plant's fuel per MW-second: Helium-3 at a sixth of the
/// Deuterium volume (its energy ratio), nothing on solar.
pub fn burn_per_mw_s(tier: u32) -> f64 {
    match plant_fuel(tier) {
        Some(Commodity::Helium3) => FUEL_PER_MW_S / crate::fuel::HELIUM3_FUEL_EQUIV,
        Some(_) => FUEL_PER_MW_S,
        None => 0.0,
    }
}

/// One planet's grid, recomputed every tick before anything produces.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    /// MW its running structures draw (crewed share of each draw).
    pub demand: f64,
    /// MW it can supply: the base plus what the plant's fuel covers.
    pub supply: f64,
    /// min(1, supply ÷ demand): the factor on every powered structure here.
    pub power: f64,
    /// The plant's burn, units of its fuel per second, at this load.
    pub burn: f64,
    /// The plant ran short of fuel this tick.
    pub no_fuel: bool,
    /// Latched brownout state (for the owner's notices).
    pub brownout: bool,
}

impl Default for Grid {
    fn default() -> Self {
        Self { demand: 0.0, supply: BASE_MW, power: 1.0, burn: 0.0, no_fuel: false, brownout: false }
    }
}

/// The grid's arithmetic. `demand` is the planet's draw this tick,
/// `plant_tier` its Power Plant, `fuel_stock` the system's free stock of that
/// plant's fuel and `dt` the tick. Returns the grid and the units burned.
pub fn settle(demand: f64, plant_tier: u32, fuel_stock: f64, dt: f64) -> (Grid, f64) {
    let capacity = plant_mw(plant_tier);
    let solar = solar_mw(plant_tier);
    let load = (demand - BASE_MW).clamp(0.0, capacity);
    let per_mw = burn_per_mw_s(plant_tier) * dt;
    let need = (load - solar).max(0.0) * per_mw;
    let (supplied, burned, no_fuel) = if need <= fuel_stock.max(0.0) + 1e-12 {
        (load, need, false)
    } else {
        let burned = fuel_stock.max(0.0);
        (solar.min(load) + if per_mw > 0.0 { burned / per_mw } else { 0.0 }, burned, true)
    };
    let supply = BASE_MW + if no_fuel { supplied } else { capacity };
    // Whole percent, rounded down: what the panels show, and steady enough
    // that a yard's work rate doesn't change every tick.
    let power = if demand <= 1e-12 { 1.0 } else { (((BASE_MW + supplied) / demand).min(1.0) * 100.0).floor() / 100.0 };
    (Grid { demand, supply, power, burn: if dt > 0.0 { burned / dt } else { 0.0 }, no_fuel, brownout: false }, burned)
}

/// Is this power level a brownout? With the previous latch, so a hairline
/// recovery doesn't flicker the notice.
pub fn browning_out(power: f64, was: bool) -> bool {
    if was { power < 1.0 - 1e-9 } else { power < BROWNOUT_BELOW }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_covers_light_work_and_solar_burns_nothing() {
        let (g, burned) = settle(2.0, 0, 0.0, 1.0);
        assert_eq!((g.power, burned), (1.0, 0.0), "tier I work runs on the base");
        let (g, _) = settle(4.4, 0, 100.0, 1.0);
        assert_eq!(g.power, 0.68, "no plant: the base's share, in whole percent");
        let (g, burned) = settle(4.4, 1, 0.0, 1.0);
        assert_eq!((g.power, burned, g.no_fuel), (1.0, 0.0, false), "a Solar Array needs no fuel");
        assert!((g.supply - (BASE_MW + 8.0)).abs() < 1e-12);
        assert_eq!(plant_fuel(2), None);
    }

    #[test]
    fn fusion_burns_deuterium_then_helium3_and_short_fuel_supplies_what_it_covers() {
        let (g, burned) = settle(33.4, 3, 100.0, 1.0);
        assert_eq!(g.power, 1.0);
        assert!((burned - (30.4 - 17.6) * FUEL_PER_MW_S).abs() < 1e-12,
            "burn follows the load above the base and the kept solar fields");
        assert_eq!(plant_fuel(3), Some(Commodity::Deuterium));
        let (g, burned) = settle(40.0, 3, 0.02, 1.0);
        assert!(g.no_fuel && (burned - 0.02).abs() < 1e-12);
        assert_eq!(g.power, 0.61, "(3 + 17.6 solar + 4 fuelled) / 40, rounded down");
        let (g, _) = settle(100.0, 4, 1e9, 1.0);
        assert_eq!(g.power, 0.51, "Deuterium tops out at a tier IV plant");
        assert_eq!(plant_fuel(5), Some(Commodity::Helium3));
        let (g, burned) = settle(60.0, 5, 1e9, 1.0);
        assert_eq!(g.power, 1.0);
        assert!((burned - (57.0 - 17.6) * FUEL_PER_MW_S / crate::fuel::HELIUM3_FUEL_EQUIV).abs() < 1e-12);
    }

    #[test]
    fn upgrading_solar_to_fusion_never_costs_free_power() {
        // A 20 MW planet runs free on Solar II. On Fusion III the same load
        // still runs free — the reactor burns only above the solar fields —
        // and a dry reactor falls back to them, not to the base.
        let (solar, burned) = settle(20.0, 2, 0.0, 1.0);
        assert_eq!((solar.power, burned), (1.0, 0.0));
        let (fusion, burned) = settle(20.0, 3, 0.0, 1.0);
        assert_eq!((fusion.power, burned, fusion.no_fuel), (1.0, 0.0, false));
        let (dry, _) = settle(30.0, 3, 0.0, 1.0);
        assert!(dry.no_fuel && (dry.supply - (BASE_MW + solar_mw(3))).abs() < 1e-12);
        assert_eq!(dry.power, 0.68, "(3 + 17.6) / 30, rounded down");
        for tier in 3..=6 {
            assert!((solar_mw(tier) - plant_mw(2)).abs() < 1e-12);
        }
    }

    #[test]
    fn heat_heavy_works_draw_double_and_the_antimatter_plant_ten_times() {
        assert_eq!(draw_mw(StructureKind::Smelter, 1), 2.0);
        assert_eq!(draw_mw(StructureKind::UraniumMill, 1), 1.0);
        assert_eq!(draw_mw(StructureKind::AntimatterPlant, 1), 10.0);
    }

    #[test]
    fn food_housing_and_defence_never_draw_power() {
        for kind in StructureKind::ALL {
            let civic = matches!(kind, StructureKind::Bioharvester | StructureKind::Agroplex
                | StructureKind::LivingTerraces | StructureKind::Habitat | StructureKind::Warehouse
                | StructureKind::OrbitalWarehouse | StructureKind::CommandCenter | StructureKind::ServiceBerth
                | StructureKind::Garrison | StructureKind::OrbitalDefense | StructureKind::DefensePlatform
                | StructureKind::SensorArray | StructureKind::PowerPlant);
            assert_eq!(powered(kind), !civic, "{kind:?}");
        }
    }
}

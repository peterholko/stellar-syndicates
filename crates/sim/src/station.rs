//! §orbital-defense: THE STATION LADDER. Every system can raise one orbital
//! defense station, level by level, from a Defense Platform to a Star
//! Fortress (the `OrbitalDefense` structure; its tier is the level). Each
//! level is its own construction project, gated by research and by the
//! system's yards, and the station keeps fighting at its old level while the
//! next one builds.
//!
//! A station fights through its HARDPOINTS: one at level 1, six at level 5.
//! Each is a stationary battery in battle with its own hull and its own fit
//! (beam by default; drivers, torpedoes, armour or a web from module crates
//! at an Ordnance Foundry). A station whose hardpoints are all knocked out is
//! SILENCED: it no longer contests a blockade, and the siege can run. Its
//! hardpoints repair between battles, never under a held blockade, and a
//! captured station comes over one level down with every hardpoint out.
//!
//! Sizing: a station gets about twice the hull and twice the firepower of
//! the warships its goods would buy, the price of never moving: it is a
//! huge, still target, it fires few big guns at moving hulls, and it can
//! never help anywhere else. Measured (`tactical::v15` probe), fleets worth
//! a station's goods beat it about half the time if they are siege-class
//! capitals and about a third of the time as cruisers; interceptors and
//! destroyers almost never. The Star Fortress is the capstone, worth about
//! two and a half Titans: a lone Titan never takes it, a Titan with three
//! dreadnoughts (the same goods) wins about half the time, a third more
//! goods wins nearly always. A corporation may raise one.
//!
//! Legacy Defense Platform tiers (pirate bases, old colonies) keep their own
//! combat model and are no longer built.

use serde::{Deserialize, Serialize};

use crate::build::{Recipe, StructureKind};
use crate::cargo::Commodity;
use crate::module::{Loadout, ModuleKind};

/// The highest station level: the Star Fortress.
pub const MAX_LEVEL: u32 = 5;

/// A hardpoint's fitting budget: three slots and eight fitting points, a
/// destroyer's.
pub const MOUNT_SLOTS: usize = 3;
pub const MOUNT_POINTS: u32 = 8;

/// A knocked-out or damaged hardpoint repairs in this many siege clocks,
/// half that with an Ordnance Foundry in the system.
pub const REPAIR_SIEGE_CLOCKS: f64 = 1.0;
pub const FOUNDRY_REPAIR_MULT: f64 = 2.0;
/// A knocked-out hardpoint stays offline until repaired to this share of its
/// hull: it doesn't limp back into the next fight at one hit point.
pub const ONLINE_HEALTH: f64 = 0.25;

const HZ: u64 = crate::config::TICK_HZ as u64;
const HOUR_TICKS: u64 = 3600 * HZ;

/// One level of the ladder.
pub struct Level {
    pub level: u32,
    pub name: &'static str,
    /// Hardpoints at this level, each a battery in battle.
    pub hardpoints: u32,
    /// Hull of ONE hardpoint.
    pub hull: f64,
    /// Attack weight of ONE hardpoint (a corvette's is 1, a titan's 24).
    pub aw: f64,
    /// Weapon reach multiple of every hardpoint (ships run 0.6–1.2).
    pub reach: f64,
    /// The research that unlocks raising a station to this level.
    pub research: &'static str,
    /// A yard the system must already hold, at this tier or better.
    pub yard: Option<(StructureKind, u32)>,
    /// The cost of raising the station TO this level (not cumulative).
    pub recipe: Recipe,
    pub description: &'static str,
}

pub const LEVELS: [Level; 5] = [
    Level {
        level: 1,
        name: "Defense Platform",
        hardpoints: 1,
        hull: 2_400.0,
        aw: 3.0,
        reach: 1.0,
        research: "weap_fire_control",
        yard: None,
        recipe: Recipe {
            costs: &[(Commodity::Alloys, 35.0), (Commodity::Electronics, 15.0), (Commodity::Armaments, 15.0)],
            build_ticks: 22 * HZ,
        },
        description: "One beam battery in orbit. Joins every battle within 1,300 su of the system and keeps a blockade contested while it fires.",
    },
    Level {
        level: 2,
        name: "Orbital Battery",
        hardpoints: 2,
        hull: 2_000.0,
        aw: 2.0,
        reach: 1.0,
        research: "weap_orbital_batteries",
        yard: None,
        recipe: Recipe {
            costs: &[
                (Commodity::Alloys, 60.0),
                (Commodity::Electronics, 25.0),
                (Commodity::Armaments, 25.0),
                (Commodity::Machinery, 10.0),
            ],
            build_ticks: HOUR_TICKS,
        },
        description: "Two hardpoints, each fitted with its own battery at an Ordnance Foundry.",
    },
    Level {
        level: 3,
        name: "Defense Station",
        hardpoints: 3,
        hull: 3_000.0,
        aw: 3.0,
        reach: 1.0,
        research: "weap_cm_iii_defense_stations",
        yard: Some((StructureKind::OrdnanceFoundry, 1)),
        recipe: Recipe {
            costs: &[
                (Commodity::Alloys, 90.0),
                (Commodity::Electronics, 40.0),
                (Commodity::Armaments, 50.0),
                (Commodity::Machinery, 20.0),
                (Commodity::Composites, 20.0),
                (Commodity::PrecisionComponents, 6.0),
            ],
            build_ticks: 6 * HOUR_TICKS,
        },
        description: "Three armed arms around the core. Needs an Ordnance Foundry in the system.",
    },
    Level {
        level: 4,
        name: "Citadel",
        hardpoints: 4,
        hull: 12_000.0,
        aw: 6.5,
        reach: 1.1,
        research: "weap_cm_v_grand_batteries",
        yard: Some((StructureKind::NavalDrydock, 1)),
        recipe: Recipe {
            costs: &[
                (Commodity::Alloys, 300.0),
                (Commodity::Electronics, 150.0),
                (Commodity::Armaments, 250.0),
                (Commodity::Machinery, 80.0),
                (Commodity::Composites, 80.0),
                (Commodity::HullSections, 60.0),
                (Commodity::PrecisionComponents, 20.0),
            ],
            build_ticks: 48 * HOUR_TICKS,
        },
        description: "A star fort: four heavy batteries behind an armored wall, a Titan's hull for a dreadnought's goods. Needs an Orbital Shipyard in the system.",
    },
    Level {
        level: 5,
        name: "Star Fortress",
        hardpoints: 6,
        hull: 32_000.0,
        aw: 22.0,
        reach: 1.2,
        research: "weap_cm_v_star_fortress",
        yard: Some((StructureKind::CapitalSlipway, 1)),
        recipe: Recipe {
            costs: &[
                (Commodity::Alloys, 1_200.0),
                (Commodity::Electronics, 600.0),
                (Commodity::Armaments, 1_000.0),
                (Commodity::Machinery, 300.0),
                (Commodity::Composites, 400.0),
                (Commodity::HullSections, 300.0),
                (Commodity::PrecisionComponents, 120.0),
                (Commodity::Titanium, 200.0),
                (Commodity::RareElements, 100.0),
            ],
            build_ticks: 192 * HOUR_TICKS,
        },
        description: "The capstone: six heavy batteries on a ring several times a Citadel's size, more than a Titan can take alone. One per corporation; needs a Capital Slipway in the system.",
    },
];

/// The ladder entry for `level` (1–5).
pub fn level(level: u32) -> Option<&'static Level> {
    LEVELS.get((level as usize).checked_sub(1)?)
}

/// The station's name at `level`, or the structure's for no station.
pub fn level_name(level: u32) -> &'static str {
    self::level(level).map_or("Orbital Defense", |l| l.name)
}

/// One hardpoint as the system keeps it between battles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mount {
    /// The battery's fit, as a loadout key ("" = the stock beam battery).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fit: String,
    /// 0..1 of its hull.
    pub health: f64,
    /// Knocked out: silent until repaired to [`ONLINE_HEALTH`].
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub offline: bool,
}

impl Default for Mount {
    fn default() -> Self {
        Mount { fit: String::new(), health: 1.0, offline: false }
    }
}

impl Mount {
    /// Firing (or able to): not knocked out.
    pub fn standing(&self) -> bool {
        !self.offline && self.health > 0.0
    }

    /// Knocked out in battle (or captured silenced).
    pub fn knock_out(&mut self) {
        self.health = 0.0;
        self.offline = true;
    }

    /// Repair by `amount` of its hull; back online at [`ONLINE_HEALTH`].
    pub fn repair(&mut self, amount: f64) {
        self.health = (self.health + amount).min(1.0);
        if self.offline && self.health >= ONLINE_HEALTH {
            self.offline = false;
        }
    }
}

/// A hardpoint as it enters a battle.
#[derive(Debug, Clone, PartialEq)]
pub struct MountSpec {
    pub index: u8,
    pub level: u8,
    pub hp: f64,
    pub max_hp: f64,
    pub aw: f64,
    pub reach: f64,
    pub stack: String,
}

/// The battle roster of a station at `level` with these hardpoints: only
/// the ones still standing fight.
pub fn mount_specs(level: u32, mounts: &[Mount]) -> Vec<MountSpec> {
    let Some(l) = self::level(level) else { return Vec::new() };
    mounts
        .iter()
        .take(l.hardpoints as usize)
        .enumerate()
        .filter(|(_, m)| m.standing())
        .map(|(i, m)| MountSpec {
            index: i as u8,
            level: l.level as u8,
            hp: (m.health.min(1.0) * l.hull).max(1.0),
            max_hp: l.hull,
            aw: l.aw,
            reach: l.reach,
            stack: m.fit.clone(),
        })
        .collect()
}

/// Whether `module` may be fitted to a hardpoint at all: weapons, armour and
/// webs. Point defense comes with every hardpoint; utilities, stabilizers
/// and the destroyer's interdiction field are ship equipment. A Prismatic
/// Lance needs Platform Lances research (and no crate).
pub fn mount_module_allowed(module: ModuleKind, lances: bool) -> bool {
    match module {
        ModuleKind::MassDriver
        | ModuleKind::TorpedoRack
        | ModuleKind::ReflectivePlating
        | ModuleKind::WhippleArmor
        | ModuleKind::StasisWeb => true,
        ModuleKind::PrismaticLance => lances,
        _ => false,
    }
}

/// Whether `fit` fits one hardpoint: allowed modules, three slots, eight
/// fitting points.
pub fn mount_fit_valid(fit: &Loadout, lances: bool) -> bool {
    fit.len() <= MOUNT_SLOTS
        && fit.fitting_cost() <= MOUNT_POINTS
        && fit.modules().iter().all(|m| mount_module_allowed(*m, lances))
}

/// The modules a refit from `from` to `to` adds and removes, by kind. A
/// lance mounted through Platform Lances research neither takes nor returns
/// a crate.
pub fn fit_delta(from: &Loadout, to: &Loadout) -> (Vec<(ModuleKind, u32)>, Vec<(ModuleKind, u32)>) {
    let count = |l: &Loadout, k: ModuleKind| l.modules().iter().filter(|m| **m == k).count() as u32;
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for k in crate::module::MODULE_KINDS {
        if k == ModuleKind::PrismaticLance {
            continue;
        }
        let (a, b) = (count(from, k), count(to, k));
        if b > a {
            added.push((k, b - a));
        } else if a > b {
            removed.push((k, a - b));
        }
    }
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(l: &Level) -> f64 {
        l.recipe.costs.iter().map(|(c, n)| crate::market::base_price(*c) * n).sum()
    }

    /// The ladder climbs like the hull ladder: each level more hull and more
    /// firepower than the last, the capstone beyond a Titan on both.
    #[test]
    fn the_ladder_climbs_past_a_titan() {
        let total = |l: &Level| l.hull * l.hardpoints as f64;
        let fire = |l: &Level| l.aw * l.hardpoints as f64;
        for w in LEVELS.windows(2) {
            assert!(total(&w[1]) > total(&w[0]) && fire(&w[1]) > fire(&w[0]), "{} → {}", w[0].name, w[1].name);
            assert!(w[1].recipe.build_ticks > w[0].recipe.build_ticks);
        }
        let titan = 32_000.0; // a Titan's hull in battle (tactical::rules_v1::hull_mass)
        let fortress = &LEVELS[4];
        assert!(total(fortress) >= 3.0 * titan, "three Titans of hull");
        assert!(fire(fortress) >= 2.0 * crate::ship::ShipKind::Titan.attack_weight(), "two Titans of guns");
        let cumulative: f64 = LEVELS.iter().map(value).sum();
        let titan_value: f64 = crate::build::recipe_for(crate::build::BuildKind::Ship { ship: crate::ship::ShipKind::Titan })
            .costs.iter().map(|(c, n)| crate::market::base_price(*c) * n).sum();
        assert!((2.0..3.0).contains(&(cumulative / titan_value)), "about two and a half Titans: {}", cumulative / titan_value);
    }

    #[test]
    fn hardpoints_take_weapons_armour_and_webs_only() {
        let fit = |m: Vec<ModuleKind>| Loadout::new(m);
        assert!(mount_fit_valid(&fit(vec![ModuleKind::MassDriver, ModuleKind::MassDriver, ModuleKind::WhippleArmor]), false));
        assert!(mount_fit_valid(&fit(vec![ModuleKind::TorpedoRack, ModuleKind::TorpedoRack]), false));
        assert!(!mount_fit_valid(&fit(vec![ModuleKind::TorpedoRack, ModuleKind::TorpedoRack, ModuleKind::TorpedoRack]), false),
            "nine fitting points");
        assert!(!mount_fit_valid(&fit(vec![ModuleKind::PointDefenseScreen]), false), "every hardpoint has point defense");
        assert!(!mount_fit_valid(&fit(vec![ModuleKind::WarpStabilizer]), false));
        assert!(!mount_fit_valid(&fit(vec![ModuleKind::PrismaticLance]), false));
        assert!(mount_fit_valid(&fit(vec![ModuleKind::PrismaticLance]), true), "Platform Lances");
        let (added, removed) = fit_delta(&fit(vec![ModuleKind::MassDriver]), &fit(vec![ModuleKind::TorpedoRack, ModuleKind::PrismaticLance]));
        assert_eq!(added, vec![(ModuleKind::TorpedoRack, 1)]);
        assert_eq!(removed, vec![(ModuleKind::MassDriver, 1)]);
    }

    #[test]
    fn only_standing_hardpoints_fight() {
        let mounts = vec![
            Mount { fit: "mass_driver".into(), health: 0.5, offline: false },
            Mount { fit: String::new(), health: 0.4, offline: true },
        ];
        let specs = mount_specs(2, &mounts);
        assert_eq!(specs.len(), 1);
        assert_eq!((specs[0].index, specs[0].stack.as_str()), (0, "mass_driver"));
        assert!((specs[0].hp - 1_000.0).abs() < 1e-9 && specs[0].max_hp == 2_000.0);
    }
}

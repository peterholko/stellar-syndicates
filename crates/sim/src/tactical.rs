//! Versioned tactical combat and private deterministic battle recordings.
//!
//! New battles use v16 deterministic math (every sin, cos, atan2 and powf is
//! software, so a battle fights the same in every build and on every
//! platform; see `Math`), v15 orbital stations (a station fights through fitted
//! hardpoints; battleships and up besiege it; Breaching Ordnance, Platform
//! Netfire and Fortress Doctrine take effect), v14 home ground (a battle at a
//! system is fought at its world: terrain, platforms grouped at the holder's
//! anchor, reinforcements from where they arrive), v13 tracking and fortune (capital
//! weapons can't track interceptors; each side rolls its fortune per
//! battle), v12 gunnery (one
//! shot per weapon, battle lines no wider than their guns reach, heavier
//! dreadnoughts and armour), v11 roles for
//! small ships (interceptor tackle, stasis webs, destroyer light batteries),
//! v10 fleet formations, v9 hull bodies,
//! wreckage and slow big-ship turning, v8 weaving line and capital hulls, v7 hull-sized weapon
//! reach, v6 mission profiles, v5 discovery weapons, v4 escort datalinks and v3
//! maneuvers. Keep old rules:
//! a saved battle's version selects its original rules, not the
//! newest balance. The server alone reconstructs archives; seeds/state are
//! never part of the player protocol.

pub mod replay;
mod rules_v1;
mod v1;
mod v2;
mod v3;
mod v4;
mod v5;
mod v6;
mod v7;
mod v8;
mod v9;
mod v10;
mod v11;
mod v12;
mod v13;
mod v14;
mod v15;
mod v16;
#[cfg(test)]
mod fingerprint;

pub use v1::*;
pub use v9::{body_radius, turn_rate, wreck_pieces, Wreck, LARGE_HULL_MASS, MAX_WRECK_HULLS};
pub use v10::{FormationFrame, Shape};
pub use v11::{
    battery_armor, battery_to_hit, stabilizers, BATTERY_ARMOR_EXP, BATTERY_ARMOR_MASS, BATTERY_DAMAGE, BATTERY_SHOTS,
    BATTERY_SPEED_SENS, INTERCEPTOR_GUNS, TACKLE_RANGE, WEB_RANGE, WEB_SPEED,
};
pub use v12::{guns, LINE_REACH, REFLECT_BLUNT, WHIPPLE_BLUNT};
pub use v13::{fortune, tracks_poorly, CAPITAL_TRACKING, FORTUNE, TACKLER_TRACKING, TRACKING_MASS};
pub use v14::{WorldBody, WORLD_OFFSET, WORLD_RADIUS};
pub use v15::{Mount, NETFIRE_RADIUS, SIEGE_BONUS};
pub use v16::{project_distribution, project_distribution_at_world, project_distribution_with_missions, simulate_engagement};

/// Captured once at battle open. Missing in pre-v2 saves means v1, including
/// an already-running fight; loading a save must never rebalance its replay.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Rules {
    #[default]
    V1,
    V2,
    /// One private, frozen maneuver seed per engagement, not per frame or viewer.
    V3 { maneuver_seed: u64 },
    /// Local datalink fire control; the rest of the kernel retains v3 motion.
    V4 { maneuver_seed: u64 },
    /// Discovery weapons; v4 steering, escort fire control and all old fittings unchanged.
    V5 { maneuver_seed: u64 },
    V6 { maneuver_seed: u64 },
    /// Hull-sized weapon reach; v6 missions and everything older unchanged.
    V7 { maneuver_seed: u64 },
    /// Line and capital hulls keep weaving instead of parking; v7 otherwise.
    V8 { maneuver_seed: u64 },
    /// Hulls are bodies that keep clear; large hulls leave physical wreckage.
    V9 { maneuver_seed: u64 },
    /// Fleets hold formations (battle line, escort ring, wedge); v9 otherwise.
    V10 { maneuver_seed: u64 },
    /// Interceptors tackle, webs slow, destroyers fire light batteries,
    /// interceptor guns are light; v10 otherwise.
    V11 { maneuver_seed: u64 },
    /// One shot per weapon, lines no wider than their reach, dreadnought
    /// attack 15, Reflective 50% and Whipple 60%; v11 otherwise.
    V12 { maneuver_seed: u64 },
    /// Capital weapons can't track interceptors; each side's fortune scales
    /// its damage for the battle; v12 otherwise.
    V13 { maneuver_seed: u64 },
    /// Battles at a system are fought at its world (terrain, platforms
    /// grouped at the holder's anchor, reinforcements from where they arrive);
    /// v13 otherwise.
    V14 { maneuver_seed: u64 },
    /// Station hardpoints fight as fitted, the siege class hits stations
    /// harder, and Breaching Ordnance, Platform Netfire and Fortress Doctrine
    /// take effect; v14 otherwise.
    V15 { maneuver_seed: u64 },
    /// Every sin, cos, atan2 and powf in software (`Math::Soft`); v15
    /// otherwise.
    V16 { maneuver_seed: u64 },
}

impl Rules {
    fn is_v1(&self) -> bool { *self == Self::V1 }

    fn math(self) -> Math {
        if matches!(self, Rules::V16 { .. }) { Math::Soft } else { Math::Platform }
    }
}

/// How a rules version evaluates sin, cos, atan2 and powf.
///
/// Rules v1–v15 call the platform's libm as the compiler leaves those calls,
/// and the compiler doesn't leave them alone: an optimized build on Apple
/// platforms fuses a sin and a cos of one angle into `__sincos_stret` and
/// turns a constant `powf(x, 0.5)` into `sqrt`, each a last-bit difference
/// for about one input in seven hundred. So their battles fight differently
/// in debug and release builds, and an archive replays exactly only in the
/// kind of build that recorded it. `Platform` keeps every one of those calls
/// exactly as written, so the archives still replay. Rules v16 and later use
/// `Soft` (`crate::dmath`): the same bits in every build, on every platform.
///
/// Write a `Platform` arm exactly as the original expression (a paired sin
/// and cos stays one expression), or its release-build bits move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Math {
    Platform,
    Soft,
}

impl Math {
    #[inline(always)]
    pub fn sin_cos(self, x: f64) -> (f64, f64) {
        match self {
            Math::Platform => x.sin_cos(),
            Math::Soft => crate::dmath::sin_cos(x),
        }
    }

    #[inline(always)]
    pub fn atan2(self, y: f64, x: f64) -> f64 {
        match self {
            Math::Platform => y.atan2(x),
            Math::Soft => crate::dmath::atan2(y, x),
        }
    }

    #[inline(always)]
    pub fn powf(self, x: f64, e: f64) -> f64 {
        match self {
            Math::Platform => x.powf(e),
            Math::Soft => crate::dmath::powf(x, e),
        }
    }

    #[inline(always)]
    pub fn polar(self, angle: f64, radius: f64) -> crate::math::Vec2 {
        match self {
            Math::Platform => crate::math::Vec2::from_polar(angle, radius),
            Math::Soft => crate::dmath::polar(angle, radius),
        }
    }
}

impl TacticalState {
    /// Add the other side's stationary support BEFORE the first recording
    /// checkpoint. Use the archived constructor's exact platform HP/weapons;
    /// no changes to old rules, random streams, or pre-existing battle rosters.
    pub fn add_initial_attacker_platforms(&mut self, tiers: u32, pool: f64) {
        if tiers == 0 { return; }
        assert_eq!(self.step, 0);
        let support = Self::open(0, 0, &[], &[], tiers, pool, self.bearing);
        for mut platform in support.combatants {
            platform.cid = self.next_cid;
            self.next_cid += 1;
            platform.side = 0;
            platform.pos = platform.pos + self.bearing * SPAWN_DIST;
            self.combatants.push(platform);
        }
    }

    pub fn platform_tiers_for(&self, side: u8) -> u32 {
        self.combatants.iter().filter(|c| c.platform && c.side == side && c.hp > 0.0).count() as u32
    }

    pub fn platform_pool_for(&self, side: u8) -> f64 {
        self.combatants.iter().filter(|c| c.platform && c.side == side && c.hp > 0.0)
            .map(|c| (c.max_hp - c.hp).max(0.0)).sum()
    }

    pub fn rules_version(&self) -> u32 {
        match self.rules {
            Rules::V1 => 1,
            Rules::V2 => 2,
            Rules::V3 { .. } => 3,
            Rules::V4 { .. } => 4,
            Rules::V5 { .. } => 5,
            Rules::V6 { .. } => 6,
            Rules::V7 { .. } => 7,
            Rules::V8 { .. } => 8,
            Rules::V9 { .. } => 9,
            Rules::V10 { .. } => 10,
            Rules::V11 { .. } => 11,
            Rules::V12 { .. } => 12,
            Rules::V13 { .. } => 13,
            Rules::V14 { .. } => 14,
            Rules::V15 { .. } => 15,
            Rules::V16 { .. } => 16,
        }
    }

    /// How this battle's rules evaluate sin, cos, atan2 and powf.
    pub fn math(&self) -> Math {
        self.rules.math()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_combat_tables_match_the_current_ship_and_module_catalog() {
        // When changing live balance, introduce/select the next combat version
        // and update this ACTIVE-version check. Never retune archived v1.
        for kind in crate::ship::ALL_SHIP_KINDS {
            assert_eq!(super::rules_v1::hull_mass(kind), kind.hull_mass());
            assert_eq!(super::rules_v1::max_speed(kind), kind.max_speed());
            assert_eq!(super::v12::attack_weight(kind), kind.attack_weight());
            assert_eq!(
                super::rules_v1::featured(kind),
                kind.is_combatant() && crate::ship::requires_hull_unlock(kind)
            );
            for family in [
                crate::module::Family::Beam,
                crate::module::Family::Driver,
                crate::module::Family::Torpedo,
                crate::module::Family::Interception,
                crate::module::Family::Protection,
            ] {
                assert_eq!(
                    super::rules_v1::hull_affinity(kind, family),
                    crate::ship::hull_affinity(kind, family)
                );
            }
        }
        assert_eq!(super::v12::REFLECT_BLUNT, crate::module::REFLECT_BLUNT);
        assert_eq!(super::v12::WHIPPLE_BLUNT, crate::module::WHIPPLE_BLUNT);
        for first in crate::module::MODULE_KINDS {
            for second in crate::module::MODULE_KINDS {
                let fit = crate::Loadout::new(vec![first, second]);
                assert_eq!(super::v5::offense(&fit), fit.offense());
            }
        }
        for seed in [0, 1, 123, u64::MAX] {
            let mut frozen = super::rules_v1::Rng::new(seed);
            let mut live = crate::rng::Rng::new(seed);
            for _ in 0..100 {
                assert_eq!(frozen.next_u64(), live.next_u64());
            }
        }
    }
}

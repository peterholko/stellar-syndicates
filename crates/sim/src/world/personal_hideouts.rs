//! §player-styles (Peter, Oct 31 2026): pirate hideouts near home. Once the
//! charter is done, a pirate crew sets up at one of a founder's doorstep stars.
//! It is cleared like the den (an idle warship within reach opens the
//! assault), and its loot becomes a private salvage contract. The next crew
//! comes a few hours later, a little stronger. One left alone for a day sends
//! its raiders after its founder's freighters and barges outside Laser Battery
//! cover. Humans only.
use serde::{Deserialize, Serialize};

use super::*;
use crate::pirate::Enclave;

/// How far from home a hideout sets up. Tunable.
pub const HIDEOUT_DOORSTEP_SU: f64 = 25_000.0;
/// The next crew arrives this long after a hideout is cleared, for the first
/// three; then [`HIDEOUT_GAP_LATE_S`]. Tunable.
pub const HIDEOUT_GAP_EARLY_S: f64 = 2.0 * 3_600.0;
pub const HIDEOUT_GAP_LATE_S: f64 = 6.0 * 3_600.0;
/// A hideout left alone this long sends its raiders out. Tunable.
pub const HIDEOUT_IGNORED_S: f64 = 24.0 * 3_600.0;
/// How far from it those raiders hunt their founder's ships. Tunable.
pub const HIDEOUT_HUNT_SU: f64 = 30_000.0;
/// Its guards' guns (the den's privateer has 0.05, the uncle 0.28): at 0.1
/// each tier falls to its count of Interceptors on every seed tried (at 0.15 a
/// lone Interceptor lost two tier-1 fights in five). Tunable.
pub const HIDEOUT_DAMAGE_MULT: f64 = 0.1;

/// A founder's personal hideouts: the one standing, how many have fallen, and
/// when the next crew arrives.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PersonalHideouts {
    pub site: Option<EntityId>,
    pub cleared: u32,
    pub next_at: Option<f64>,
}

/// The tier of the `n`th hideout (1-based): tier 1 for the first two, tier 2
/// for the next two, tier 3 after.
pub fn hideout_tier(n: u32) -> u32 {
    match n {
        0..=2 => 1,
        3..=4 => 2,
        _ => 3,
    }
}

/// Its guards: raiders, and the share of their hull they start with.
pub fn hideout_guards(tier: u32) -> (u32, f64) {
    match tier {
        1 => (3, 0.75),
        2 => (4, 1.0),
        _ => (6, 1.0),
    }
}

/// What it holds: about 3,000, 6,000 and 10,000 Cr at base prices, goods a
/// young colony doesn't make yet.
pub fn hideout_loot(tier: u32) -> BTreeMap<crate::cargo::Commodity, u32> {
    use crate::cargo::Commodity as C;
    match tier {
        1 => [(C::Machinery, 30), (C::Electronics, 35)].into_iter().collect(),
        2 => [(C::Machinery, 40), (C::Electronics, 40), (C::Armaments, 40)].into_iter().collect(),
        _ => [(C::Machinery, 40), (C::Armaments, 40), (C::TitaniumOre, 30), (C::RareElements, 20)].into_iter().collect(),
    }
}

impl World {
    /// Raise each finished founder's next hideout when its time comes, and
    /// note the fall of the one standing.
    pub(super) fn tick_personal_hideouts(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let owners: Vec<PlayerId> = self.players.iter()
            .filter(|(id, c)| !id.is_bot() && c.founding.local_opening
                && c.founding.stage == crate::founding::FoundingStage::Complete && !c.founding.fallen(now))
            .map(|(id, _)| *id)
            .collect();
        for owner in owners {
            let h = self.players[&owner].hideouts.clone();
            if let Some(site) = h.site {
                if self.enclaves.get(&site).is_none_or(|e| e.cleared) {
                    // Fallen: its loot is already a salvage contract, and the
                    // founder's map drops it.
                    self.enclaves.remove(&site);
                    if let Some(pos) = self.systems.iter().find(|s| s.id == site).map(|s| s.pos)
                        && let Some(corp) = self.players.get_mut(&owner)
                    {
                        corp.intel.insert(site, IntelSnapshot { defense_tier: 0, shipyard_tier: 0, enclave_tier: 0,
                            garrison_tier: 0, station_level: 0, observed_at: now, pos });
                    }
                    let cleared = h.cleared + 1;
                    let gap = if cleared <= 3 { HIDEOUT_GAP_EARLY_S } else { HIDEOUT_GAP_LATE_S };
                    self.players.get_mut(&owner).unwrap().hideouts =
                        PersonalHideouts { site: None, cleared, next_at: Some(now + gap) };
                }
                continue;
            }
            if h.next_at.is_none_or(|at| now >= at) {
                self.spawn_personal_hideout(owner, h.cleared, events);
            }
        }
    }

    /// The next crew sets up at one of the founder's doorstep stars, moving
    /// round them as each falls, and command learns where at once.
    fn spawn_personal_hideout(&mut self, owner: PlayerId, cleared: u32, events: &mut Vec<Event>) {
        let Some(corp) = self.players.get(&owner) else { return };
        let home = corp.home;
        let den = corp.founding.den;
        let homes: std::collections::BTreeSet<EntityId> = self.home_slots.iter().filter_map(|h| h.system).collect();
        let mut stars: Vec<(f64, EntityId, Vec2)> = self.systems.iter()
            .filter(|s| s.owner.is_none() && s.pos.distance(self.hub) > HUB_SAFE_RADIUS && Some(s.id) != den
                && !homes.contains(&s.id)
                && !self.enclaves.contains_key(&s.id) && s.pos.distance(home) <= HIDEOUT_DOORSTEP_SU)
            .map(|s| (s.pos.distance(home), s.id, s.pos))
            .collect();
        stars.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let Some(&(_, system, pos)) = stars.get(cleared as usize % stars.len().max(1)) else {
            // No star in reach: try again later rather than every tick.
            self.players.get_mut(&owner).unwrap().hideouts.next_at = Some(self.time + HIDEOUT_GAP_EARLY_S);
            return;
        };
        let tier = hideout_tier(cleared + 1);
        let (raiders, hull) = hideout_guards(tier);
        let id = self.alloc_entity_id();
        let mut guards = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, pos, FleetOrder::Idle, None);
        guards.reset_to(ShipKind::Raider, raiders);
        guards.fill_stores();
        guards.hideout_crew = true;
        for ship in &mut guards.ships {
            ship.hp = ship.max_hp() * hull;
        }
        self.fleets.insert(id, guards);
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == system) {
            sys.set_tier(crate::build::StructureKind::DefensePlatform, 0);
            sys.defense_pool = 0.0;
        }
        self.enclaves.insert(system, Enclave {
            system,
            tier,
            plunder: hideout_loot(tier),
            next_launch_at: self.time + HIDEOUT_IGNORED_S,
            // Never; finite, so the world still saves as JSON.
            next_grow_at: f64::MAX,
            dormant_until: 0.0,
            pack: Some(id),
            cleared: false,
            founder_den: false,
            for_player: Some(owner),
        });
        let now = self.time;
        let corp = self.players.get_mut(&owner).unwrap();
        corp.hideouts.site = Some(system);
        corp.intel.insert(system, IntelSnapshot { defense_tier: 0, shipyard_tier: 0, enclave_tier: tier,
            garrison_tier: 0, station_level: 0, observed_at: now, pos });
        events.push(Event::new(now, EventPayload::PirateHideoutSpotted { owner, system, tier, pos }));
    }

    /// An ignored hideout's raiders hunt their founder's freighters and barges
    /// outside Laser Battery cover, never anyone else's.
    pub(super) fn launch_hideout_pack(&mut self, pid: EntityId, owner: PlayerId, base_pos: Vec2,
        covered: &dyn Fn(Vec2) -> bool) -> bool
    {
        let shielded = self.pvp_shielded(owner);
        let target = self.fleets.iter()
            .filter(|(_, f)| f.owner == owner && !shielded && !covered(f.pos)
                && (f.flagship_kind().is_freighter_hull() || f.contains(ShipKind::MiningBarge))
                && f.pos.distance(base_pos) <= HIDEOUT_HUNT_SU)
            .min_by(|a, b| base_pos.distance(a.1.pos).total_cmp(&base_pos.distance(b.1.pos)).then(a.0.cmp(b.0)))
            .map(|(id, _)| *id);
        let Some(tid) = target else { return false };
        if let Some(f) = self.fleets.get_mut(&pid) {
            f.order = FleetOrder::Intercept { target: tid };
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hideouts_grow_a_tier_every_two_and_carry_more_loot() {
        let tiers: Vec<u32> = (1..=6).map(hideout_tier).collect();
        assert_eq!(tiers, vec![1, 1, 2, 2, 3, 3]);
        let worth = |tier| hideout_loot(tier).iter().map(|(c, n)| crate::market::base_price(*c) * *n as f64).sum::<f64>();
        assert!((2_900.0..3_200.0).contains(&worth(1)), "{}", worth(1));
        assert!((5_800.0..6_300.0).contains(&worth(2)), "{}", worth(2));
        assert!((9_700.0..10_300.0).contains(&worth(3)), "{}", worth(3));
    }
}

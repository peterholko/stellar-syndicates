//! §charter-crates (Peter, Oct 31 2026): the charter's supplies, released a
//! chapter at a time and collected from the guide (`founding::CharterCrate`).
use super::*;
use crate::founding::CharterCrate;

impl World {
    /// Collect a released charter crate into the home stockpile. It lands
    /// whole, past the storage cap if it must: production waits for room.
    pub(super) fn collect_charter_crate(&mut self, owner: PlayerId, which: CharterCrate, events: &mut Vec<Event>) {
        let Some(home) = self.players.get(&owner).filter(|c| c.founding.crate_waiting(which)).and_then(|c| c.home_system)
        else {
            return;
        };
        let Some(sys) = self.systems.iter_mut().find(|s| s.id == home && s.owner == Some(owner)) else {
            return;
        };
        for &(good, n) in which.goods() {
            *sys.stockpile.entry(good).or_insert(0.0) += n;
        }
        if let Some(corp) = self.players.get_mut(&owner) {
            corp.founding.crates_collected.insert(which);
        }
        events.push(Event::new(self.time, EventPayload::CharterCrateCollected { owner, charter_crate: which }));
    }

    /// Bots collect each crate as it is released; a finished charter opens
    /// any still sealed.
    pub(super) fn open_due_charter_crates(&mut self, events: &mut Vec<Event>) {
        let due: Vec<_> = self.players.iter()
            .filter(|(id, c)| id.is_bot() || c.founding.stage == crate::founding::FoundingStage::Complete)
            .flat_map(|(id, c)| CharterCrate::ALL.into_iter().filter(|k| c.founding.crate_waiting(*k)).map(move |k| (*id, k)))
            .collect();
        for (owner, which) in due {
            self.collect_charter_crate(owner, which, events);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::StructureKind as K;
    use crate::cargo::Commodity as C;
    use crate::founding::{FoundingStage as S, CharterCrate as Crate};

    fn founded() -> (World, PlayerId, EntityId) {
        let mut w = World::new(SimConfig::for_players(2026, 4));
        let owner = PlayerId(91_401);
        w.step(&[Command::AddPlayer { id: owner, name: "Crates".into() }]);
        let home = w.players[&owner].home_system.unwrap();
        (w, owner, home)
    }

    fn stock(w: &World, sys: EntityId, c: C) -> f64 {
        w.systems.iter().find(|s| s.id == sys).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
    }

    /// Every crate carries what its builds cost, but the Semiconductors and
    /// Armaments the trader and the Market Hub sell back.
    #[test]
    fn each_crate_carries_its_builds() {
        for c in Crate::ALL {
            let mut need: BTreeMap<C, f64> = BTreeMap::new();
            for &what in c.builds() {
                for &(good, n) in crate::build::recipe_for_tier(what, 1).costs.iter() {
                    *need.entry(good).or_default() += n;
                }
            }
            let has = c.stock();
            for (good, n) in need {
                if matches!(good, C::Electronics | C::Armaments) { continue; }
                // §reactors-drives: the barge's reactors are the den's
                // recovered six and its drives landed with the colony, not the
                // Reach crate's.
                if good.is_engine_part() && c == Crate::Reach { continue; }
                assert!(has.get(&good).copied().unwrap_or(0.0) >= n, "{c:?} crate: {good:?} {n} for its builds");
            }
        }
        let all = crate::founding::charter_supplies();
        // §shipbuilding-overhaul: the Interceptor's one, the Tiny Freighter's
        // two and the barge's three, all at landing — the raid's prize.
        assert_eq!(all[&C::FissionReactor], 6.0, "the charter's six engines");
        assert_eq!(all[&C::PulseLaser], 6.0, "the Interceptor's guns land with the colony");
        // §charter-v6: the raid's and step 4's Solar Array's.
        assert_eq!(all[&C::Electronics], crate::founding::KIT_SEMICONDUCTORS + crate::founding::PLANT_SEMICONDUCTORS);
        assert_eq!(crate::build::POWER_PLANT_RECIPE.costs.iter().find(|(c, _)| *c == C::Electronics).map(|(_, n)| *n),
            Some(crate::founding::PLANT_SEMICONDUCTORS), "the kit carries the Solar Array's semiconductors");
    }

    #[test]
    fn the_landing_holds_only_the_landfall_crate() {
        let (w, owner, home) = founded();
        let f = &w.players[&owner].founding;
        assert!(f.crates_collected.contains(&Crate::Landfall), "open at landing");
        assert!(Crate::ALL[1..].iter().all(|c| !f.crate_waiting(*c)), "nothing else released before step 24");
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        // §smelt-from-landing (Nov 6 2026): the Smelter's 45 units ride in
        // the holds too, a little over half; §sell-alloys (Nov 11 2026): and
        // the trader's ten.
        assert!(s.storage_used() <= 0.56 * s.storage_cap(), "room for the mine: {:.0} of {:.0}", s.storage_used(), s.storage_cap());
        // Peter (Nov 4 2026): the Laser Battery's, Solar Array's and
        // Shipyard's goods too; no crate after the raid. Nov 6: the Smelter's.
        // §sell-alloys (Nov 11 2026): and the trader's sample at step 9.
        assert_eq!(stock(&w, home, C::Alloys), 235.0 + crate::merchant::BUY_CAP as f64);
        // Peter (Nov 5 2026): no Pirates or Expansion crate; the first
        // Interceptor's and the Tiny Freighter's cores land with the colony.
        assert_eq!(stock(&w, home, C::FissionReactor), 6.0, "the charter's six Fission Cores");
    }

    #[test]
    fn a_crate_is_collected_once_its_step_releases_it_even_into_full_stores() {
        let (mut w, owner, home) = founded();
        w.step(&[Command::CollectCharterCrate { player_id: owner, charter_crate: Crate::Reach }]);
        assert_eq!(stock(&w, home, C::Alloys), 245.0, "sealed until step 24");
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildBarge, w.time);
        assert!(w.players[&owner].founding.crate_waiting(Crate::Reach));
        let cap = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(C::MetallicOre, cap);
        let events = w.step(&[Command::CollectCharterCrate { player_id: owner, charter_crate: Crate::Reach }]);
        assert!(events.iter().any(|e| matches!(e.payload,
            EventPayload::CharterCrateCollected { owner: o, charter_crate: Crate::Reach } if o == owner)));
        assert_eq!(stock(&w, home, C::Alloys), 285.0, "past the cap: a crate always opens");
        w.step(&[Command::CollectCharterCrate { player_id: owner, charter_crate: Crate::Reach }]);
        assert_eq!(stock(&w, home, C::Alloys), 285.0, "once");
    }

    #[test]
    fn a_finished_charter_opens_what_is_left_and_bots_never_wait() {
        let (mut w, owner, home) = founded();
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::Complete, w.time);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.crates_collected.len(), Crate::ALL.len());
        let supplies = crate::founding::charter_supplies();
        assert!(stock(&w, home, C::FissionReactor) >= supplies[&C::FissionReactor], "every crate's goods");
        let bot = PlayerId(PlayerId::BOT_BASE | 3);
        w.step(&[Command::AddPlayer { id: bot, name: "Bot".into() }]);
        w.players.get_mut(&bot).unwrap().founding.set_stage(S::BuildBarge, w.time);
        w.step(&[]);
        let collected = &w.players[&bot].founding.crates_collected;
        assert!(collected.contains(&Crate::Reach));
        assert!(!collected.contains(&Crate::Vendetta), "not before its step");
    }

    /// "Secure your stores": a Warehouse and a Vault at home, and the Vault
    /// only from this step.
    #[test]
    fn the_charter_secures_its_stores_before_the_uncle() {
        let (mut w, owner, home) = founded();
        let f = &w.players[&owner].founding;
        assert!(!f.allows(K::Vault), "no Vault before step 22");
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::SecureStores, w.time);
        assert!(w.players[&owner].founding.allows(K::Vault));
        let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let land = s.bodies.iter().find(|b| b.tier(K::CommandCenter) > 0).unwrap().id;
        s.bodies.iter_mut().find(|b| b.id == land).unwrap().set_tier(K::Warehouse, 1);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::SecureStores, "the Vault too");
        w.systems.iter_mut().find(|s| s.id == home).unwrap()
            .bodies.iter_mut().find(|b| b.id == land).unwrap().set_tier(K::Vault, 1);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::Vendetta);
    }
}

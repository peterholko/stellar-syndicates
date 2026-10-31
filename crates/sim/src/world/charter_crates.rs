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
                assert!(has.get(&good).copied().unwrap_or(0.0) >= n, "{c:?} crate: {good:?} {n} for its builds");
            }
        }
        let all = crate::founding::charter_supplies();
        assert_eq!(all[&C::WarpCore], 5.0, "the Tiny Freighter, three Interceptors and the barge");
        assert_eq!(all[&C::Electronics], crate::founding::KIT_SEMICONDUCTORS, "only the raid's");
    }

    #[test]
    fn the_landing_holds_only_the_landfall_crate() {
        let (w, owner, home) = founded();
        let f = &w.players[&owner].founding;
        assert!(f.crates_collected.contains(&Crate::Landfall), "open at landing");
        assert!(Crate::ALL[1..].iter().all(|c| !f.crate_waiting(*c)), "nothing else released before step 6");
        let s = w.systems.iter().find(|s| s.id == home).unwrap();
        assert!(s.storage_used() <= 0.4 * s.storage_cap(), "room for the mine: {:.0} of {:.0}", s.storage_used(), s.storage_cap());
        assert_eq!(stock(&w, home, C::Alloys), 110.0);
        assert_eq!(stock(&w, home, C::WarpCore), 0.0, "the first hull's core comes with its chapter");
    }

    #[test]
    fn a_crate_is_collected_once_its_step_releases_it_even_into_full_stores() {
        let (mut w, owner, home) = founded();
        w.step(&[Command::CollectCharterCrate { player_id: owner, charter_crate: Crate::Raid }]);
        assert_eq!(stock(&w, home, C::Alloys), 110.0, "sealed until step 6");
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::BuildGunPlatform, w.time);
        assert!(w.players[&owner].founding.crate_waiting(Crate::Raid));
        let cap = w.systems.iter().find(|s| s.id == home).unwrap().storage_cap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(C::MetallicOre, cap);
        let events = w.step(&[Command::CollectCharterCrate { player_id: owner, charter_crate: Crate::Raid }]);
        assert!(events.iter().any(|e| matches!(e.payload,
            EventPayload::CharterCrateCollected { owner: o, charter_crate: Crate::Raid } if o == owner)));
        assert_eq!(stock(&w, home, C::Alloys), 205.0, "past the cap: a crate always opens");
        w.step(&[Command::CollectCharterCrate { player_id: owner, charter_crate: Crate::Raid }]);
        assert_eq!(stock(&w, home, C::Alloys), 205.0, "once");
    }

    #[test]
    fn a_finished_charter_opens_what_is_left_and_bots_never_wait() {
        let (mut w, owner, home) = founded();
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::Complete, w.time);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.crates_collected.len(), Crate::ALL.len());
        let supplies = crate::founding::charter_supplies();
        assert!(stock(&w, home, C::WarpCore) >= supplies[&C::WarpCore], "every crate's goods");
        let bot = PlayerId(PlayerId::BOT_BASE | 3);
        w.step(&[Command::AddPlayer { id: bot, name: "Bot".into() }]);
        w.players.get_mut(&bot).unwrap().founding.set_stage(S::PrepareExport, w.time);
        w.step(&[]);
        let collected = &w.players[&bot].founding.crates_collected;
        assert!([Crate::Raid, Crate::Pirates, Crate::Expansion].iter().all(|c| collected.contains(c)));
        assert!(!collected.contains(&Crate::Reach), "not before its step");
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

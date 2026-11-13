//! §charter-v4: the privateer den. The Rogue Privateer flies out of a pirate
//! base in the home's den system ([`crate::galaxy::HomeSlot::den_system`]), a
//! local survey stop holding only a rich asteroid belt. The den rises with the
//! privateer, the privateer's log names it, and a lone Interceptor can destroy
//! its weak garrison. It stays cleared, and its belt becomes the charter's.
//! §den-contract: the Authority's contract on it pays for its fall.
use super::*;
use crate::pirate::Enclave;

impl World {
    /// This corporation's den system, from its home slot.
    pub(super) fn founding_den_system(&self, owner: PlayerId) -> Option<EntityId> {
        let home = self.players.get(&owner)?.home_system?;
        self.home_slots.iter().find(|h| h.system == Some(home))?.den_system
    }

    /// Raise the den with the privateer, once per charter: an idle garrison of
    /// weak raiders holding the den system, with no defense platforms. Homes
    /// without a den system (overflow homes) skip the den step.
    pub(super) fn ensure_founding_den(&mut self, owner: PlayerId) {
        let Some(corp) = self.players.get(&owner) else {
            return;
        };
        if corp.founding.den.is_some() || !corp.founding.enabled || !corp.founding.local_opening {
            return;
        }
        let Some(den) = self.founding_den_system(owner) else {
            return;
        };
        let home = corp.home_system.and_then(|h| self.systems.iter().find(|s| s.id == h)).map(|s| s.pos);
        let Some(sys) = self.systems.iter().find(|s| s.id == den && s.owner.is_none()) else {
            return;
        };
        // §den-in-belt (Peter, Nov 9 2026: "why the battle at the pirate den
        // occurred at the star instead of their pirate den in the asteroid
        // belt"): the den holds a point of its belt, the side facing the
        // colony, where an Interceptor flying in from home comes in.
        let (pos, at_world) = match sys.first_belt() {
            Some(belt) => (sys.belt_point(belt, home.unwrap_or(sys.pos)),
                Some(crate::orbits::AtWorld { system: den, body: belt })),
            None => (sys.pos, None),
        };
        if !self.enclaves.contains_key(&den) {
            let id = self.alloc_entity_id();
            let mut garrison = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, pos, FleetOrder::Idle, None);
            garrison.at_world = at_world;
            garrison.reset_to(ShipKind::Raider, crate::founding::DEN_GARRISON_RAIDERS);
            garrison.fill_stores();
            // The privateer's crew: its weak guns and slow hulls.
            garrison.founding_privateer = true;
            for ship in &mut garrison.ships {
                ship.hp = ship.max_hp() * crate::founding::DEN_GARRISON_HULL_FRAC;
            }
            self.fleets.insert(id, garrison);
            if let Some(sys) = self.systems.iter_mut().find(|s| s.id == den) {
                sys.set_tier(crate::build::StructureKind::DefensePlatform, 0);
                sys.defense_pool = 0.0;
            }
            self.enclaves.insert(den, Enclave {
                system: den,
                tier: 1,
                // §stash: it holds what the raid takes; §den-stores and,
                // from the day it rises, what the charter's next steps need.
                plunder: crate::founding::DEN_STORES.into_iter().collect(),
                // Never; finite, so the world still saves as JSON.
                next_launch_at: f64::MAX,
                next_grow_at: f64::MAX,
                dormant_until: 0.0,
                pack: Some(id),
                cleared: false,
                founder_den: true,
                for_player: None,
            });
        }
        self.players.get_mut(&owner).unwrap().founding.den = Some(den);
    }

    /// The den becomes known. §den-retreat (Nov 12 2026): as the hunt begins,
    /// since the privateer went home to it; or, if he died first, from his
    /// log, where and when he died.
    pub(super) fn reveal_founding_den(&mut self, owner: PlayerId, at: f64, pos: Vec2) {
        let Some(den) = self.players.get(&owner).and_then(|c| c.founding.den) else {
            return;
        };
        if self.enclaves.get(&den).is_none_or(|e| e.cleared) {
            return;
        }
        if let Some(corp) = self.players.get_mut(&owner) {
            corp.intel.insert(den, IntelSnapshot { defense_tier: 0, shipyard_tier: 0, enclave_tier: 1,
                garrison_tier: 0, station_level: 0, observed_at: at, pos });
        }
    }

    /// §den-contract (Peter, Nov 5 2026: "destroying the privateer provides
    /// coordinates to the pirate den and it is noticed that there is an
    /// Authority contract to eliminate the pirate's den by his name"): once
    /// per den, with the log that names it, the Authority's contract on it,
    /// named for his family. Private, and taken on at once (the colony holding
    /// the coordinates is the contractor), so its first report home is the
    /// news of it. It never lapses and can't be dropped: the den's fall
    /// completes it like any bounty (`process_operation_events`), and it pays
    /// [`crate::founding::DEN_CONTRACT_CREDITS`] when that report reaches
    /// command, where the den's charter stipend used to.
    pub(super) fn post_founding_den_contract(&mut self, owner: PlayerId, origin: Vec2, out: &mut Vec<Event>) {
        use crate::operation::{FollowUpKind, OperationBriefing, OperationIssuer, OperationKind, OperationReward,
            OperationScope, OperationState};
        let Some(den) = self.players.get(&owner).and_then(|c| c.founding.den) else {
            return;
        };
        if self.enclaves.get(&den).is_none_or(|e| e.cleared) || self.operations.values().any(|o| o.is_founder_den()
            && matches!(o.scope, OperationScope::Private { player } if player == owner))
        {
            return;
        }
        let at = self.systems.iter().find(|s| s.id == den).map_or_else(String::new, |s| s.name.clone());
        let (title, whose) = self.pirate_family(owner).map_or_else(
            || ("Eliminate the privateer den".to_string(), "The privateer's base".to_string()),
            |p| (format!("Eliminate the {} den", p.surname()), format!("{}'s base", p.nephew_name())));
        let reward = OperationReward { credits: crate::founding::DEN_CONTRACT_CREDITS,
            authority_standing: crate::founding::DEN_CONTRACT_STANDING, captain_xp: 40, research_insight: 0.0 };
        // A private contract posts as an offer; this one is taken on before
        // its report leaves. Never lapses; finite, so the world still saves
        // as JSON.
        let id = self.insert_operation(OperationIssuer::Authority, OperationScope::Private { player: owner },
            OperationKind::PirateBounty { system: den, tier: 1 }, 1, reward, origin, f64::MAX, &mut Vec::new());
        self.pending_operation_reports.retain(|r| r.operation != id);
        let o = self.operations.get_mut(&id).expect("just posted");
        o.state = OperationState::Active;
        o.participants.insert(owner);
        o.contributions.entry(owner).or_default();
        // §den-retreat: he went home to it, so he fights there with its crew.
        o.briefing = Some(OperationBriefing { follow_up: FollowUpKind::FounderDen, title,
            difficulty: "Three raiders".into(), suitable_fleets: "One Interceptor".into(), variant: 0,
            summary: format!("{whose} at {at}. No defenses.") });
        self.queue_operation_report(id, &[owner], origin, out);
    }

    /// §privateer-contract (Peter, Nov 9 2026: "The Authority Advisor gets a
    /// transmission from the Authority that there just happens to be a
    /// contract out to eliminate the Pirate. Can we introduce the player to
    /// the Contracts at this point?"): as the hunt begins, the Authority's
    /// contract on the privateer, by his name, at his last known position.
    /// Private and offered: the founder takes it on in Operations, the
    /// charter's first contract. Posted at the colony (the advisor's channel),
    /// so it is news at once. It never lapses and can't be dropped; the kill
    /// claims it, taken or not (`complete_founding_privateer_contract`), and
    /// its report pays [`crate::founding::PRIVATEER_CREDIT_BOUNTY`] where the
    /// bounty's stipend did. `announce` false: posted with the kill itself,
    /// so its offer is never news.
    pub(super) fn post_founding_privateer_contract(&mut self, owner: PlayerId, announce: bool,
        out: &mut Vec<Event>) -> Option<OperationId>
    {
        use crate::operation::{FollowUpKind, OperationBriefing, OperationIssuer, OperationKind, OperationReward,
            OperationScope};
        if let Some(id) = self.founding_privateer_contract(owner) {
            return Some(id);
        }
        let corp = self.players.get(&owner)?;
        let f = &corp.founding;
        if !f.enabled || f.privateer.is_none() {
            return None;
        }
        let origin = corp.command_center;
        let last_seen = f.raid.as_ref().map(|r| r.lurk)
            .or_else(|| f.privateer.and_then(|id| self.fleets.get(&id)).map(|g| g.pos))
            .unwrap_or(origin);
        // §den-retreat: home at his den with its crew; without a den, lying in wait nearby.
        let den = f.den.and_then(|d| self.systems.iter().find(|s| s.id == d)).map(|s| s.name.clone());
        let (title, who) = self.pirate_family(owner).map_or_else(
            || ("Eliminate the privateer".to_string(), "The privateer".to_string()),
            |p| (format!("Eliminate {}", p.nephew_name()), p.nephew_name()));
        let reward = OperationReward { credits: crate::founding::PRIVATEER_CREDIT_BOUNTY,
            authority_standing: 0.0, captain_xp: 0, research_insight: 0.0 };
        let mut quiet = Vec::new();
        let id = self.insert_operation(OperationIssuer::Authority, OperationScope::Private { player: owner },
            OperationKind::PrivateerPatrol { pos: last_seen }, 1, reward, origin, f64::MAX,
            if announce { &mut *out } else { &mut quiet });
        if !announce {
            self.pending_operation_reports.retain(|r| r.operation != id);
        }
        let o = self.operations.get_mut(&id).expect("just posted");
        o.briefing = Some(OperationBriefing { follow_up: FollowUpKind::FounderPrivateer, title,
            difficulty: if den.is_some() { "Three raiders".into() } else { "One raider".into() },
            suitable_fleets: "One Interceptor".into(), variant: 0,
            summary: den.map_or_else(|| format!("{who} raided your colony. Last seen lying in wait nearby."),
                |at| format!("{who} raided your colony and went home to his den at {at}.")) });
        Some(id)
    }

    /// This founder's contract on their privateer, if posted.
    pub(super) fn founding_privateer_contract(&self, owner: PlayerId) -> Option<OperationId> {
        use crate::operation::OperationScope;
        self.operations.values().find(|o| o.is_founder_privateer()
            && matches!(o.scope, OperationScope::Private { player } if player == owner)).map(|o| o.id)
    }

    /// §privateer-contract: the kill completes the contract, taken on or
    /// not: the colony that made it is the contractor. Its report leaves
    /// from the kill, on the same light as the kill's own report.
    pub(super) fn complete_founding_privateer_contract(&mut self, owner: PlayerId, pos: Vec2, out: &mut Vec<Event>) {
        use crate::operation::OperationState;
        let Some(id) = self.post_founding_privateer_contract(owner, false, out) else {
            return;
        };
        let now = self.time;
        let Some(o) = self.operations.get_mut(&id).filter(|o| !o.state.terminal()) else {
            return;
        };
        o.participants.insert(owner);
        o.contribution_mut(owner).combat += 100;
        if o.state == OperationState::Offered {
            o.state = OperationState::Active;
            o.starts_at = now;
        }
        self.complete_operation(id, Some(owner), pos, out);
    }

    /// The privateer's kill promotes the captain aboard the fleet that made it.
    pub(super) fn promote_privateer_hunter(&mut self, owner: PlayerId, fleet: EntityId) {
        let target = crate::captain::xp_for_level(crate::founding::PRIVATEER_PROMOTION_LEVEL);
        if let Some(captain) = self.players.get_mut(&owner).and_then(|corp| {
            corp.captains.values_mut().find(|c| c.assigned_fleet == Some(fleet) && !c.missing())
        }) {
            captain.grant_xp(target.saturating_sub(captain.xp));
        }
    }

    /// `note_founding_den_cleared`, and §vendetta the uncle's message among
    /// the den's stores, the moment its fall is noted.
    pub(super) fn note_and_tell_founding_den_cleared(&mut self, owner: PlayerId, events: &[Event], out: &mut Vec<Event>) {
        let unreported = self.players.get(&owner).is_some_and(|c| c.founding.den.is_some() && c.founding.den_report_at.is_none());
        if !unreported {
            return;
        }
        self.note_founding_den_cleared(owner, events);
        let f = &self.players[&owner].founding;
        if f.den_report_at.is_some() && !owner.is_bot()
            && let Some(pos) = f.den.and_then(|d| self.systems.iter().find(|s| s.id == d)).map(|s| s.pos)
        {
            out.push(Event::new(self.time, EventPayload::Vendetta {
                owner, beat: crate::founding::VendettaBeat::Den, pos, stolen: BTreeMap::new(), wreck: None }));
        }
    }

    /// When the report that the den fell reaches command: its light from the
    /// den, whoever cleared it. A den found cleared without its event (a
    /// reload between the two) counts from now.
    pub(super) fn note_founding_den_cleared(&mut self, owner: PlayerId, events: &[Event]) {
        let Some(f) = self.players.get(&owner).map(|c| &c.founding) else {
            return;
        };
        let (Some(den), None) = (f.den, f.den_report_at) else {
            return;
        };
        let cc = self.players[&owner].command_center;
        let fell = events.iter().find_map(|e| match &e.payload {
            EventPayload::PirateEnclaveCleared { system, pos, .. } if *system == den => Some((e.time, *pos)),
            _ => None,
        });
        let report = fell.map(|(at, pos)| at + crate::transit::delay(pos, cc, self.config.c))
            .or_else(|| (f.stage == crate::founding::FoundingStage::EliminateDen
                && self.enclaves.get(&den).is_none_or(|e| e.cleared)).then_some(self.time));
        if let Some((_, pos)) = fell {
            self.den_blast(owner, pos);
        }
        if let Some(at) = report {
            self.players.get_mut(&owner).unwrap().founding.den_report_at = Some(at);
        }
    }

    /// §stash (Peter, Nov 2 2026: "make sure it gets damaged around 75%"): the
    /// den's reactor blows as it falls. Every hull of the founder's at the den
    /// is left at most [`crate::founding::DEN_BLAST_HULL_FRAC`] of its hull,
    /// never destroyed, and each fleet it leaves damaged has to come home for
    /// the Shipyard's repairs before the next step.
    fn den_blast(&mut self, owner: PlayerId, pos: Vec2) {
        let frac = crate::founding::DEN_BLAST_HULL_FRAC;
        let near: Vec<EntityId> = self.fleets.iter()
            .filter(|(_, f)| f.owner == owner
                && (f.pos.distance(pos) <= crate::pirate::PIRATE_ASSAULT_RADIUS || self.same_system(f.pos, pos)))
            .map(|(id, _)| *id)
            .collect();
        for id in near {
            let Some(f) = self.fleets.get_mut(&id) else {
                continue;
            };
            for ship in f.ships.iter_mut().filter(|s| s.hp > 0.0) {
                ship.hp = ship.hp.min(ship.max_hp() * frac);
            }
            if f.ships.iter().any(|s| s.deficit() > 1e-6)
                && let Some(corp) = self.players.get_mut(&owner)
            {
                corp.founding.den_wounded.insert(id);
            }
        }
    }

    /// §stash: what the den's stash holds or will hold for this founder and
    /// is not home yet: what the raid took and §den-stores the den's stores,
    /// in the den until it falls, then what waits there and what a freighter
    /// is bringing home.
    pub fn founding_stash(&self, owner: PlayerId) -> BTreeMap<crate::cargo::Commodity, u32> {
        let mut goods = self.founding_stash_at_den(owner);
        if let Some(f) = self.players.get(&owner).map(|c| &c.founding) {
            for (&c, &n) in &f.stash_aboard {
                *goods.entry(c).or_default() += n;
            }
        }
        goods
    }

    /// §den-stores: what still waits at the den for a freighter.
    pub fn founding_stash_at_den(&self, owner: PlayerId) -> BTreeMap<crate::cargo::Commodity, u32> {
        let Some(f) = self.players.get(&owner).map(|c| &c.founding) else {
            return BTreeMap::new();
        };
        if !f.stash.is_empty() || f.den_report_at.is_some() {
            return f.stash.clone();
        }
        f.den.and_then(|d| self.enclaves.get(&d)).filter(|e| e.founder_den && !e.cleared)
            .map(|e| e.plunder.iter().filter(|(_, n)| **n > 0).map(|(c, n)| (*c, *n)).collect())
            .unwrap_or_default()
    }

    /// §stash: steps 13–16 (repair, freighter, speed, haul), each tick of any
    /// of them, so they overlap. A hull the den's blast hurt leaves the
    /// wounded list once it is whole (or gone). §den-stores (Peter, Nov 5
    /// 2026: "if they need to make two trips in their freighter so be it!"):
    /// an owned freighter at rest in the den system with room loads what it can
    /// ([`crate::founding::STASH_LOAD_ORDER`] first; §hold-room: each good by
    /// its own room, so the charter's loot is one Tiny load) and heads home, where
    /// `DeliverToSystem` unloads it as any haul home does. Once it berths
    /// there the next load waits for a hull; the stash is home with the last
    /// load, or with any carrier lost on the way.
    pub(super) fn advance_founding_stash(&mut self, owner: PlayerId, out: &mut Vec<Event>) {
        let now = self.time;
        let Some(corp) = self.players.get(&owner) else {
            return;
        };
        let f = &corp.founding;
        let healed: Vec<EntityId> = f.den_wounded.iter().copied()
            .filter(|id| self.fleets.get(id).is_none_or(|fl| fl.owner != owner || fl.ships.iter().all(|s| s.deficit() <= 1e-6)))
            .collect();
        let (den, home, stash, carrier, done) = (f.den, corp.home_system, f.stash.clone(), f.stash_fleet, f.stash_home_at.is_some());
        let corp = self.players.get_mut(&owner).unwrap();
        for id in healed {
            corp.founding.den_wounded.remove(&id);
        }
        if done {
            return;
        }
        let (Some(den), Some(home)) = (den, home) else {
            self.players.get_mut(&owner).unwrap().founding.stash_home_at = Some(now);
            return;
        };
        if let Some(id) = carrier {
            let lost = self.fleets.get(&id).is_none_or(|fl| fl.owner != owner);
            if lost || self.dock_of(id) == Some(DockSite::System(home)) {
                let f = &mut self.players.get_mut(&owner).unwrap().founding;
                f.stash_fleet = None;
                f.stash_aboard.clear();
                if lost || stash.is_empty() {
                    f.stash_home_at = Some(now);
                }
            }
            return;
        }
        if stash.is_empty() {
            self.players.get_mut(&owner).unwrap().founding.stash_home_at = Some(now);
            return;
        }
        let (Some(den_pos), Some(home_pos)) = (self.systems.iter().find(|s| s.id == den).map(|s| s.pos),
            self.systems.iter().find(|s| s.id == home).map(|s| s.pos)) else {
            return;
        };
        let engaged: std::collections::BTreeSet<EntityId> = self.engagements.values()
            .flat_map(|e| e.attackers.iter().chain(e.defenders.iter()).copied()).collect();
        let Some((id, room)) = self.fleets.iter()
            .filter(|(id, fl)| fl.owner == owner && !engaged.contains(id) && fl.vel.length() <= 0.5
                && fl.ships.iter().any(|s| s.kind.is_player_freighter())
                && fl.cargo_room().fits_any(stash.keys().copied()) && self.same_system(fl.pos, den_pos))
            .map(|(id, fl)| (*id, fl.cargo_room()))
            .min_by_key(|(id, _)| *id)
        else {
            return;
        };
        // What the charter needs first goes first.
        let first = crate::founding::STASH_LOAD_ORDER;
        let order: Vec<crate::cargo::Commodity> = first.into_iter().chain(stash.keys().copied().filter(|c| !first.contains(c))).collect();
        let (mut left, mut load) = (stash, BTreeMap::new());
        let mut free = room;
        for c in order {
            let Some(n) = left.get_mut(&c) else { continue };
            let take = free.take(c, *n);
            if take == 0 { continue; }
            *n -= take;
            load.insert(c, take);
        }
        left.retain(|_, n| *n > 0);
        let fl = self.fleets.get_mut(&id).unwrap();
        for (&c, &n) in &load {
            fl.add_cargo(c, n);
        }
        fl.mission = Some(TradeMission::DeliverToSystem { system: home });
        fl.order = FleetOrder::MoveTo { dest: home_pos };
        let pos = fl.pos;
        let f = &mut self.players.get_mut(&owner).unwrap().founding;
        f.stash = left;
        f.stash_aboard = load.clone();
        f.stash_fleet = Some(id);
        out.push(Event::new(now, EventPayload::FoundingStashLoaded { owner, system: den, pos, goods: load }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::founding::FoundingStage as S;

    fn founder() -> (World, PlayerId) {
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        let owner = PlayerId(77);
        w.step(&[Command::AddPlayer { id: owner, name: "Hunter".into() }]);
        let f = &mut w.players.get_mut(&owner).unwrap().founding;
        f.enabled = true;
        f.local_opening = true;
        (w, owner)
    }

    /// §den-retreat (Peter, Nov 12 2026: "let's have the first pirate raid
    /// retreat back to their den instead of the hunting area", then "One
    /// assault at the den"): home from his raid he joins the den's crew, so
    /// one assault takes him and his den together. A lone Interceptor (no
    /// captain, so the cautious case) wins it on every seed with hull to
    /// spare, and both steps, both contracts and the den's blast follow from
    /// the one battle.
    #[test]
    fn one_interceptor_takes_the_privateer_and_his_den_in_one_assault() {
        use crate::operation::OperationState;
        let mut worst = f64::INFINITY;
        let mut longest = 0.0_f64;
        for seed in [12648430_u64, 1, 2, 3, 4, 5, 6, 7] {
            let mut w = World::new(SimConfig::for_players(seed, 4));
            let owner = PlayerId(77);
            w.step(&[Command::AddPlayer { id: owner, name: "Hunter".into() }]);
            {
                let f = &mut w.players.get_mut(&owner).unwrap().founding;
                f.enabled = true;
                f.local_opening = true;
            }
            w.ensure_founding_den(owner);
            let den = w.players[&owner].founding.den.unwrap();
            let pack = w.enclaves[&den].pack.unwrap();
            let at = w.fleets[&pack].pos;
            // Home from his raid, as the retreat leaves him: his full hull in the crew.
            let his = w.alloc_entity_id();
            let ships = Fleet::single(his, PlayerId::PIRATE, ShipKind::Raider, at, FleetOrder::Idle, None).ships;
            w.fleets.get_mut(&pack).unwrap().absorb_ships(ships);
            w.players.get_mut(&owner).unwrap().founding.privateer = Some(pack);
            w.players.get_mut(&owner).unwrap().founding.set_stage(S::DefeatPrivateer, w.time);
            // The hunt begins: the den is known, and both contracts are posted.
            w.step(&[]);
            assert!(w.players[&owner].intel.contains_key(&den), "seed {seed}: the den is known as the hunt begins");
            let contracts = |w: &World| (w.operations.values().find(|o| o.briefing.as_ref()
                .is_some_and(|b| b.follow_up == crate::operation::FollowUpKind::FounderPrivateer)).map(|o| o.state),
                w.operations.values().find(|o| o.is_founder_den()).map(|o| o.state));
            assert!(matches!(contracts(&w), (Some(_), Some(_))), "seed {seed}: both contracts posted");
            // The Interceptor comes in from home.
            let hull = w.alloc_entity_id();
            w.fleets.insert(hull, Fleet::single(hull, owner, ShipKind::Raider, at, FleetOrder::Idle, None));
            let started = w.time;
            let mut before = 1.0;
            let mut cleared = None;
            for _ in 0..(600 * crate::config::TICK_HZ) {
                let hp = w.fleets.get(&hull).map_or(0.0, |f| f.ships[0].hp / f.ships[0].max_hp());
                let ev = w.step(&[]);
                if cleared.is_none() && ev.iter().any(|e| matches!(&e.payload,
                    EventPayload::PirateEnclaveCleared { system, .. } if *system == den)) {
                    cleared = Some(w.time - started);
                    before = hp;
                }
                if w.players[&owner].founding.stage == S::RepairInterceptor {
                    break;
                }
            }
            let f = &w.players[&owner].founding;
            assert!(cleared.is_some(), "seed {seed}: the den falls");
            assert!(w.fleets.contains_key(&hull), "seed {seed}: the Interceptor survives");
            assert!(f.reward_granted, "seed {seed}: his kill's report pays the bounty");
            assert_eq!(f.stage, S::RepairInterceptor, "seed {seed}: steps 12 and 13 done in the one battle");
            assert!(f.den_wounded.contains(&hull), "seed {seed}: the den's blast sends it home for repairs");
            assert!(matches!(contracts(&w), (Some(OperationState::Completed), Some(OperationState::Completed))),
                "seed {seed}: both contracts complete: {:?}", contracts(&w));
            worst = worst.min(before);
            longest = longest.max(cleared.unwrap());
        }
        eprintln!("den assault: worst {:.0}% hull left before the blast, longest {longest:.0} s", worst * 100.0);
        assert!(worst > 0.25, "with hull to spare: {:.0}%", worst * 100.0);
    }

    #[test]
    fn every_home_has_a_den_system_holding_only_a_rich_belt_within_survey_reach() {
        let w = World::new(SimConfig::for_players(12648430, 4));
        for slot in &w.home_slots {
            let den = slot.den_system.expect("a den system per home");
            let sys = w.systems.iter().find(|s| s.id == den).unwrap();
            assert_eq!(sys.bodies.len(), 1, "only the belt");
            assert!(sys.bodies[0].is_belt() && !sys.bodies[0].deposits.is_empty(), "a belt with ore");
            // §den-pocket (Peter, Oct 30 2026): and hides what the den guarded.
            assert_eq!(sys.bodies[0].pockets.iter().map(|p| &p.deposit).collect::<Vec<_>>(),
                [&crate::founding::den_pocket()], "the den's Titanium pocket");
            assert!(sys.pos.distance(slot.pos) <= 22_001.0, "on the doorstep, a few minutes out");
            assert!(w.exploration_systems.contains(&den));
        }
    }

    /// §den (Oct 29 2026): the den's fights stand in its asteroid field, with
    /// no world in the arena; a home's keep theirs.
    #[test]
    fn a_dens_battle_has_no_world_to_fight_beside() {
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        w.step(&[]);
        let den = w.players[&owner].founding.den.unwrap();
        let garrison = w.fleets[&w.enclaves[&den].pack.unwrap()].pos;
        let star = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        assert_eq!(w.battle_world_side(garrison, owner, PlayerId::PIRATE), None, "on its belt");
        assert_eq!(w.battle_world_side(star, owner, PlayerId::PIRATE), None, "at its star");
        let home = w.players[&owner].command_center;
        assert_eq!(w.battle_world_side(home, PlayerId::PIRATE, owner), Some(1), "a home fights beside its world");
    }

    #[test]
    fn the_den_rises_with_the_privateer_and_holds_without_raiding() {
        let (mut w, owner) = founder();
        let den = w.founding_den_system(owner).expect("a den system");
        assert!(!w.enclaves.contains_key(&den), "no den before the pirates come");
        w.ensure_founding_den(owner);
        w.ensure_founding_den(owner);
        assert_eq!(w.players[&owner].founding.den, Some(den));
        let e = &w.enclaves[&den];
        assert!(e.founder_den && e.permanent() && !e.cleared);
        let garrison = &w.fleets[&e.pack.unwrap()];
        assert!(garrison.founding_privateer);
        assert_eq!(garrison.ships.len(), crate::founding::DEN_GARRISON_RAIDERS as usize);
        assert_eq!(w.systems.iter().find(|s| s.id == den).unwrap().tier(crate::build::StructureKind::DefensePlatform), 0);
        // §real-coordinates: it takes its place on the den's belt, then holds.
        w.step(&[]);
        let garrison = &w.fleets[&w.enclaves[&den].pack.unwrap()];
        let sys = w.systems.iter().find(|s| s.id == den).unwrap();
        let ring = sys.bodies[0].belt.unwrap().radius * crate::body::SYSTEM_UNIT_SU;
        assert!((garrison.pos.distance(sys.pos) - ring).abs() < 1.0, "on the belt, not at the star");
        let pos = garrison.pos;
        for _ in 0..(120 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        let garrison = &w.fleets[&w.enclaves[&den].pack.unwrap()];
        assert!(garrison.pos.distance(pos) < 1.0, "the garrison holds its den");
        assert_eq!(w.enclaves[&den].tier, 1, "a den never grows");
    }

    #[test]
    fn a_lone_interceptor_clears_the_den_for_good_and_its_report_completes_the_step() {
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::EliminateDen, w.time);
        let pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Raider, pos, FleetOrder::Idle, None));
        let mut cleared_at = None;
        for _ in 0..(240 * crate::config::TICK_HZ) {
            let ev = w.step(&[]);
            if cleared_at.is_none() && ev.iter().any(|e| matches!(&e.payload,
                EventPayload::PirateEnclaveCleared { system, owner: o, .. } if *system == den && *o == owner)) {
                cleared_at = Some(w.time);
            }
            if w.players[&owner].founding.stage != S::EliminateDen {
                break;
            }
        }
        assert!(cleared_at.is_some(), "one Interceptor destroys the den's garrison");
        assert!(w.fleets.contains_key(&id), "and survives it");
        assert!(w.enclaves[&den].cleared, "the den stays cleared");
        assert_eq!(w.players[&owner].founding.stage, S::RepairInterceptor, "the report completes the step");
        // §stash: its reactor blew as it fell; the Interceptor limps home for repairs.
        let ship = &w.fleets[&id].ships[0];
        assert!(ship.hp > 0.0 && ship.hp <= ship.max_hp() * crate::founding::DEN_BLAST_HULL_FRAC + 1e-9,
            "the blast leaves it at {:.0}% hull", 100.0 * ship.hp / ship.max_hp());
        assert!(w.players[&owner].founding.den_wounded.contains(&id), "and the step waits on its repair");
        // §return-home (Peter, Nov 5 2026): it holds at the den until the
        // player sends it home.
        for _ in 0..(30 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert!(matches!(w.fleets[&id].order, FleetOrder::Idle), "no flight home on its own: {:?}", w.fleets[&id].order);
        assert!(w.fleets[&id].pos.distance(pos) < 2_000.0, "still at the den");
        w.time += crate::pirate::PIRATE_DORMANCY * 3.0;
        w.step(&[]);
        assert!(w.enclaves[&den].cleared && w.enclaves[&den].pack.is_none(), "no respawn");
    }

    /// §stash: a fallen den, a staffed and powered Shipyard on the landing
    /// world (as steps 8–9 leave it), the Interceptor at the den and the
    /// raid's loot and §den-stores the den's stores waiting there. Returns
    /// the world, the founder, home, the den's position, the Interceptor and
    /// the loot.
    fn stash_scene() -> (World, PlayerId, EntityId, Vec2, EntityId, BTreeMap<crate::cargo::Commodity, u32>) {
        use crate::build::StructureKind as K;
        use crate::cargo::Commodity as C;
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        let home = w.players[&owner].home_system.unwrap();
        let den_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        if let Some(pack) = w.enclaves.get_mut(&den).unwrap().pack.take() {
            w.fleets.remove(&pack);
        }
        w.enclaves.get_mut(&den).unwrap().cleared = true;
        {
            let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            let landing = s.bodies.iter_mut().find(|b| b.tier(K::CommandCenter) > 0).unwrap();
            landing.structures.insert(K::Shipyard, 1);
            landing.structures.insert(K::PowerPlant, 1);
            landing.assignments.insert(K::Shipyard, crate::production::Assignment::crew(1));
            s.stockpile.insert(C::Alloys, 100.0);
            s.stockpile.insert(C::Machinery, 100.0);
        }
        let hunter = stash_hull(&mut w, owner, ShipKind::Raider, den_pos);
        let mut loot: BTreeMap<C, u32> = crate::founding::DEN_STORES.into_iter().collect();
        // The raid's take joins the den's stores (`founding_raid`).
        *loot.entry(C::Electronics).or_default() += crate::founding::KIT_SEMICONDUCTORS as u32;
        *loot.entry(C::Armaments).or_default() += crate::founding::KIT_ARMAMENTS as u32;
        *loot.entry(C::FissionReactor).or_default() += 6;
        *loot.entry(C::ZephyrDrive).or_default() += 7;
        {
            let f = &mut w.players.get_mut(&owner).unwrap().founding;
            f.set_stage(S::RepairInterceptor, w.time);
            f.den_report_at = Some(w.time);
            f.stash = loot.clone();
        }
        w.den_blast(owner, den_pos);
        (w, owner, home, den_pos, hunter, loot)
    }

    fn stash_hull(w: &mut World, owner: PlayerId, kind: ShipKind, pos: Vec2) -> EntityId {
        let id = w.alloc_entity_id();
        let mut f = Fleet::single(id, owner, kind, pos, FleetOrder::Idle, None);
        f.fill_stores();
        w.fleets.insert(id, f);
        id
    }

    fn run_while(w: &mut World, owner: PlayerId, stage: S, minutes: f64) {
        for _ in 0..((minutes * 60.0) as usize * crate::config::TICK_HZ as usize) {
            w.step(&[]);
            if w.players[&owner].founding.stage != stage {
                break;
            }
        }
    }

    /// §stash (Peter, Nov 3 2026: "split it for this hauling exercise"):
    /// Repair the Interceptor, Build a Tiny Freighter, Haul back the den's
    /// loot, each done on its own fact.
    #[test]
    fn the_den_steps_repair_then_build_then_haul_each_on_its_own_fact() {
        use crate::cargo::Commodity as C;
        let (mut w, owner, home, den_pos, hunter, loot) = stash_scene();
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let ship = &w.fleets[&hunter].ships[0];
        assert!((ship.hp / ship.max_hp() - crate::founding::DEN_BLAST_HULL_FRAC).abs() < 1e-9, "a quarter of its hull left");
        assert_eq!(w.founding_stash(owner), loot, "the view reads the loot");
        // 13: home to the Shipyard, which mends it.
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::RepairInterceptor, "it is still at the den");
        w.fleets.get_mut(&hunter).unwrap().pos = home_pos;
        run_while(&mut w, owner, S::RepairInterceptor, 2.0);
        assert!(w.fleets[&hunter].ships.iter().all(|s| s.deficit() <= 1e-6), "the Shipyard mended it");
        // 14: waits for a freighter.
        assert_eq!(w.players[&owner].founding.stage, S::BuildTinyFreighter);
        for _ in 0..30 {
            w.step(&[]);
        }
        assert_eq!(w.players[&owner].founding.stage, S::BuildTinyFreighter, "no freighter yet");
        let freighter = stash_hull(&mut w, owner, ShipKind::TinyFreighter, home_pos);
        // 15 (§full-speed): a new freighter flies Slow; the step waits for Full.
        w.fleets.get_mut(&freighter).unwrap().transit = crate::ship::TransitMode::Slow;
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::FullSpeed);
        for _ in 0..30 {
            w.step(&[]);
        }
        assert_eq!(w.players[&owner].founding.stage, S::FullSpeed, "Slow, it waits");
        w.step(&[Command::SetFleetTransit { player_id: owner, fleet_id: freighter, mode: crate::ship::TransitMode::Full }]);
        for _ in 0..5 {
            w.step(&[]);
        }
        // 16: waits for the loot to come home.
        assert_eq!(w.players[&owner].founding.stage, S::HaulLoot);
        for _ in 0..30 {
            w.step(&[]);
        }
        assert_eq!((w.players[&owner].founding.stage, w.players[&owner].founding.stash_fleet), (S::HaulLoot, None),
            "nothing loads at home");
        let stock = |w: &World, c: C| w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&c).copied().unwrap_or(0.0);
        let before: BTreeMap<C, f64> = loot.keys().map(|&c| (c, stock(&w, c))).collect();
        // §hold-room (Peter, Nov 10 2026: "I don't like how the player has to
        // make 2 freighter trips to the den"): the 100 units fill 41.75 of the
        // Tiny Freighter's 50 room (parts 0.25, Machinery and mounts 0.5), one trip.
        assert_eq!(loot.values().sum::<u32>(), 100);
        assert_eq!(crate::cargo::manifest_quarters(&loot), 167);
        w.fleets.get_mut(&freighter).unwrap().pos = den_pos;
        let events = w.step(&[]);
        assert_eq!(w.players[&owner].founding.stash_fleet, Some(freighter), "it loads on arrival");
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::FoundingStashLoaded { goods, .. } if *goods == loot)),
            "the whole loot in one load");
        assert_eq!(w.founding_stash(owner), loot, "the view counts what is aboard");
        assert!(w.founding_stash_at_den(owner).is_empty(), "nothing waits at the den");
        for _ in 0..(15 * 60 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.players[&owner].founding.stash_fleet.is_none() {
                break;
            }
        }
        for _ in 0..(60 * crate::config::TICK_HZ) {
            if w.fleets[&freighter].cargo_units() == 0 { break; }
            w.step(&[]);
        }
        assert_eq!(w.fleets[&freighter].cargo_units(), 0, "unloaded");
        run_while(&mut w, owner, S::HaulLoot, 1.0);
        let f = &w.players[&owner].founding;
        assert_eq!(f.stage, S::PrepareExport, "the loot home: the export is next");
        for (&c, &n) in &loot {
            assert!((stock(&w, c) - before[&c] - n as f64).abs() < 1.0, "{c:?}: the loot is in the home stockpile");
        }
    }

    /// §stash: the three overlap. The loot can be home before the Interceptor
    /// is mended; the later steps then complete as they are reached.
    #[test]
    fn the_haul_can_finish_first_and_the_later_steps_complete_when_reached() {
        let (mut w, owner, home, den_pos, hunter, _) = stash_scene();
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let freighter = stash_hull(&mut w, owner, ShipKind::TinyFreighter, den_pos);
        w.step(&[]);
        assert_eq!(w.players[&owner].founding.stash_fleet, Some(freighter), "it loads during step 13");
        // §den-stores: as many trips as it takes (one, §hold-room).
        for _ in 0..3 {
            w.fleets.get_mut(&freighter).unwrap().pos = den_pos;
            for _ in 0..(15 * 60 * crate::config::TICK_HZ) {
                w.step(&[]);
                let f = &w.players[&owner].founding;
                if f.stash_home_at.is_some() || (f.stash_fleet.is_none() && w.fleets[&freighter].cargo_units() == 0) {
                    break;
                }
            }
        }
        assert!(w.players[&owner].founding.stash_home_at.is_some(), "the loot is home");
        assert_eq!(w.players[&owner].founding.stage, S::RepairInterceptor, "the Interceptor is still out");
        w.fleets.get_mut(&hunter).unwrap().pos = home_pos;
        run_while(&mut w, owner, S::RepairInterceptor, 2.0);
        for _ in 0..3 {
            w.step(&[]);
        }
        let f = &w.players[&owner].founding;
        assert_eq!(f.stage, S::PrepareExport, "steps 14 and 15 were done already");
        assert!([S::BuildTinyFreighter, S::HaulLoot].iter().all(|s| f.milestones.contains_key(s)), "each still stamped");
    }

    /// §den-contract (Peter, Nov 5 2026): the kill's report brings the den's
    /// coordinates and the Authority's contract on it, named for his family
    /// and taken on at once. It can't be dropped, and the den's fall pays it
    /// when that report reaches command, as the charter's reward moment.
    #[test]
    fn the_kill_brings_the_authoritys_contract_on_the_den_and_its_fall_pays_it() {
        use crate::operation::{OperationIssuer, OperationKind, OperationState};
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        let cc = w.players[&owner].command_center;
        let kill = cc + Vec2::new(20_000.0, 0.0);
        w.reveal_founding_den(owner, w.time, kill);
        let mut out = Vec::new();
        w.post_founding_den_contract(owner, kill, &mut out);
        w.post_founding_den_contract(owner, kill, &mut out);
        let posted: Vec<_> = w.operations.values().filter(|o| o.is_founder_den()).collect();
        assert_eq!(posted.len(), 1, "one contract per den");
        let (id, o) = (posted[0].id, posted[0]);
        assert!(matches!(o.kind, OperationKind::PirateBounty { system, .. } if system == den));
        assert_eq!((o.issuer, o.state), (OperationIssuer::Authority, OperationState::Active), "the Authority's, taken on");
        assert!(o.participants.contains(&owner));
        let family = w.pirate_family(owner).unwrap();
        let brief = o.briefing.as_ref().unwrap();
        assert_eq!(brief.title, format!("Eliminate the {} den", family.surname()), "by his name");
        assert!(brief.summary.starts_with(&format!("{}'s base at ", family.nephew_name())), "{}", brief.summary);
        assert_eq!(o.reward.credits, crate::founding::DEN_CONTRACT_CREDITS);
        // Its news rides the kill's light home: taken on, not offered.
        let report = w.pending_operation_reports.iter().find(|r| r.operation == id).unwrap();
        assert_eq!(report.snapshot.state, OperationState::Active);
        assert!((report.arrive_at - w.time - crate::transit::delay(kill, cc, w.config.c)).abs() < 1.0);
        w.step(&[Command::AbandonOperation { player_id: owner, operation_id: id }]);
        assert_eq!(w.operations[&id].state, OperationState::Active, "the charter's own: it can't be dropped");

        // A lone Interceptor clears the den, and the report pays.
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::EliminateDen, w.time);
        let pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        let hull = w.alloc_entity_id();
        w.fleets.insert(hull, Fleet::single(hull, owner, ShipKind::Raider, pos, FleetOrder::Idle, None));
        let before = w.players[&owner].credits;
        let mut moment = None;
        for _ in 0..(240 * crate::config::TICK_HZ) {
            for e in w.step(&[]) {
                if let EventPayload::CharterStipend { owner: o, title, credits, .. } = e.payload && o == owner {
                    moment = Some((title, credits));
                }
            }
            if w.players[&owner].founding.stage != S::EliminateDen {
                break;
            }
        }
        assert_eq!(w.players[&owner].founding.stage, S::RepairInterceptor, "the den's report completes the step");
        assert_eq!(w.operations[&id].known[&owner].state, OperationState::Completed, "and the contract, on the same light");
        assert_eq!(moment, Some((format!("{} den eliminated · contract", family.surname()), crate::founding::DEN_CONTRACT_CREDITS)),
            "a reward moment that opens the Expansion chapter");
        assert!(w.players[&owner].credits >= before + crate::founding::DEN_CONTRACT_CREDITS - 1e-6);
    }

    /// §privateer-contract: taken on in Operations like any contract, but
    /// the charter's own: it can't be dropped, and the kill completes it.
    #[test]
    fn the_privateers_contract_is_taken_on_and_never_dropped() {
        use crate::operation::{OperationIssuer, OperationState};
        let (mut w, owner) = founder();
        w.players.get_mut(&owner).unwrap().founding.privateer = Some(EntityId(9_999));
        let id = w.post_founding_privateer_contract(owner, true, &mut Vec::new()).expect("posted");
        assert_eq!(w.post_founding_privateer_contract(owner, true, &mut Vec::new()), Some(id), "one contract");
        assert_eq!((w.operations[&id].issuer, w.operations[&id].state), (OperationIssuer::Authority, OperationState::Offered));
        w.step(&[]);
        assert_eq!(w.operations[&id].known[&owner].state, OperationState::Offered, "its offer is news at the colony at once");
        w.step(&[Command::AcceptOperation { player_id: owner, operation_id: id }]);
        assert_eq!(w.operations[&id].state, OperationState::Active, "taken on");
        assert_eq!(w.operations[&id].expires_at, f64::MAX, "and still never lapses");
        w.step(&[Command::AbandonOperation { player_id: owner, operation_id: id }]);
        assert_eq!(w.operations[&id].state, OperationState::Active, "the charter's own: it can't be dropped");
        let kill = w.players[&owner].command_center + Vec2::new(6_600.0, 0.0);
        w.complete_founding_privateer_contract(owner, kill, &mut Vec::new());
        assert_eq!((w.operations[&id].state, w.operations[&id].winner), (OperationState::Completed, Some(owner)));
    }

    /// §privateer-contract: a kill before the hunt began (before its offer)
    /// still claims the contract, without an offer arriving after the fact.
    #[test]
    fn a_kill_before_the_offer_claims_the_contract_quietly() {
        use crate::operation::OperationState;
        let (mut w, owner) = founder();
        w.players.get_mut(&owner).unwrap().founding.privateer = Some(EntityId(9_999));
        let kill = w.players[&owner].command_center + Vec2::new(6_600.0, 0.0);
        w.complete_founding_privateer_contract(owner, kill, &mut Vec::new());
        let id = w.founding_privateer_contract(owner).expect("posted with the kill");
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        let reports: Vec<_> = w.pending_operation_reports.iter().filter(|r| r.operation == id).map(|r| r.snapshot.state).collect();
        assert_eq!(reports, vec![OperationState::Completed], "only its completion is news");
    }

    /// §den-contract: a den found cleared without its fall in hand still pays
    /// its contract.
    #[test]
    fn a_den_found_cleared_still_pays_its_contract() {
        use crate::operation::OperationState;
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        let pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        w.reveal_founding_den(owner, w.time, pos);
        w.post_founding_den_contract(owner, pos, &mut Vec::new());
        let id = w.operations.values().find(|o| o.is_founder_den()).unwrap().id;
        w.enclaves.get_mut(&den).unwrap().cleared = true;
        w.step(&[]);
        assert_eq!(w.operations[&id].state, OperationState::Completed);
        assert!(w.operations[&id].participants.contains(&owner));
    }

    /// §den-in-belt (Peter, Nov 9 2026: "why the battle at the pirate den
    /// occurred at the star instead of their pirate den in the asteroid
    /// belt"): the den holds a point of its belt facing the colony, and the
    /// fight for it is fought there.
    #[test]
    fn the_den_holds_its_belt_and_is_fought_there() {
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        let pack = w.enclaves[&den].pack.unwrap();
        let sys = w.systems.iter().find(|s| s.id == den).unwrap().clone();
        let belt = sys.first_belt().expect("a belt-only system");
        let home = w.systems.iter().find(|s| Some(s.id) == w.players[&owner].home_system).unwrap().pos;
        let spot = sys.belt_point(belt, home);
        assert_eq!(w.fleets[&pack].at_world, Some(crate::orbits::AtWorld { system: den, body: belt }));
        assert!(w.fleets[&pack].pos.distance(spot) < 1e-6, "on its belt, the side facing home");
        assert!(spot.distance(sys.pos) > 100.0, "not at the star");
        w.step(&[]);
        assert!(w.fleets[&pack].pos.distance(spot) < 1.0, "and it holds there");
        // An Interceptor come in from home stands at the same point: the
        // assault opens there.
        let hull = w.alloc_entity_id();
        w.fleets.insert(hull, Fleet::single(hull, owner, ShipKind::Raider, spot, FleetOrder::Idle, None));
        for _ in 0..(5 * crate::config::TICK_HZ) {
            w.step(&[]);
            if w.engagements.values().any(|e| e.attackers.contains(&hull) || e.defenders.contains(&hull)) {
                break;
            }
        }
        let fight = w.engagements.values().find(|e| e.attackers.contains(&hull) || e.defenders.contains(&hull))
            .expect("the den's assault");
        assert!(fight.pos.distance(spot) < 50.0, "fought at the den, in its belt: {:?} vs {:?}", fight.pos, spot);
        assert!(fight.pos.distance(sys.pos) > 100.0, "not at the star");
    }

    #[test]
    fn the_privateers_log_names_the_den_when_its_kill_report_arrives() {
        let (mut w, owner) = founder();
        w.ensure_founding_den(owner);
        let den = w.players[&owner].founding.den.unwrap();
        let cc = w.players[&owner].command_center;
        let kill = cc + Vec2::new(20_000.0, 0.0);
        w.reveal_founding_den(owner, w.time, kill);
        let snap = w.players[&owner].intel[&den];
        assert_eq!(snap.enclave_tier, 1);
        assert_eq!(snap.pos, kill, "observed where the privateer died: it reaches command with the kill report");
    }

    #[test]
    fn a_home_without_a_den_system_skips_the_den() {
        let (mut w, owner) = founder();
        for slot in &mut w.home_slots {
            slot.den_system = None;
        }
        w.ensure_founding_den(owner);
        assert_eq!(w.players[&owner].founding.den, None);
    }
}

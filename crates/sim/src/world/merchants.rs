//! §merchant: the founding merchant's visit (`crate::merchant`,
//! `docs/traveling-merchant.md`). §charter-v5: it is called when the report of
//! the privateer's raid reaches command (`World::advance_founding_raid`), docks
//! at the home, trades while docked and leaves when the charter is fulfilled
//! or its stay runs out. Bots are corporations and play the same charter, so
//! they get one too. §charter-v5 (Peter, Oct 16 2026): Teo leaves while the
//! founder eliminates the pirates: a last call once the privateer's kill is
//! reported (`merchant::LAST_CALL_S`), cut short when the den falls, so the
//! ore goes to the Market Hub from "Export your ore" on.
use super::*;
use crate::cargo::Commodity;
use crate::founding::FoundingStage;
use crate::merchant::{
    ARRIVAL_DISTANCE_SU, DEPARTURE_DISTANCE_SU, MerchantPhase, MerchantVisit,
};

impl World {
    /// Once a tick, after the charter's own check: call merchants for founders
    /// whose raid report just arrived, dock arrivals, send off finished visits
    /// and retire ships that have flown clear.
    pub(super) fn advance_merchants(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let due: Vec<(PlayerId, EntityId)> = self.players.iter()
            .filter(|(_, c)| c.founding.enabled && c.founding.local_opening && !c.founding.merchant_called
                && c.founding.stage != FoundingStage::Complete
                && c.founding.raid_report_at.is_some_and(|at| at <= now))
            .filter_map(|(&id, c)| c.home_system.map(|home| (id, home)))
            .collect();
        for (owner, home) in due {
            let Some(pos) = self.systems.iter().find(|s| s.id == home).map(|s| s.pos) else { continue };
            let fleet = self.alloc_entity_id();
            let from = pos + self.hubward(pos) * ARRIVAL_DISTANCE_SU;
            self.fleets.insert(fleet, Fleet::single(fleet, PlayerId::TCA, ShipKind::Freighter, from,
                FleetOrder::MoveTo { dest: pos }, None));
            let market = &self.market;
            let visit = MerchantVisit::founding(fleet, owner, home, self.time, |c| market.price(c));
            self.merchants.insert(fleet, visit);
            if let Some(corp) = self.players.get_mut(&owner) {
                corp.founding.merchant_called = true;
            }
        }

        let ships: Vec<EntityId> = self.merchants.keys().copied().collect();
        for ship in ships {
            let Some((pos, idle)) = self.fleets.get(&ship).map(|f| (f.pos, matches!(f.order, FleetOrder::Idle))) else {
                self.merchants.remove(&ship);
                continue;
            };
            // §charter-v5: the last call, from the privateer's kill report
            // until LAST_CALL_S later or the den's fall, whichever is first.
            if let Some(c) = self.players.get(&self.merchants[&ship].customer).filter(|c| c.founding.reward_granted) {
                let den_down = c.founding.den_report_at.is_some_and(|at| at <= self.time);
                let last = if den_down { self.time } else { self.time + crate::merchant::LAST_CALL_S };
                let v = self.merchants.get_mut(&ship).unwrap();
                v.leave_by = v.leave_by.min(last);
            }
            let visit = self.merchants[&ship].clone();
            let Some(host) = self.systems.iter().find(|s| s.id == visit.system).map(|s| s.pos) else {
                self.retire_merchant(ship);
                continue;
            };
            let finished = self.time >= visit.leave_by || self.players.get(&visit.customer).is_none_or(|c|
                c.founding.stage == FoundingStage::Complete || c.home_system != Some(visit.system));
            match visit.phase {
                MerchantPhase::Arriving | MerchantPhase::Docked if finished => {
                    let away = host + self.hubward(host) * (DEPARTURE_DISTANCE_SU + 2_000.0);
                    if let Some(f) = self.fleets.get_mut(&ship) {
                        f.order = FleetOrder::MoveTo { dest: away };
                    }
                    if let Some(v) = self.merchants.get_mut(&ship) {
                        v.phase = MerchantPhase::Leaving;
                    }
                    if visit.phase == MerchantPhase::Docked {
                        events.push(Event::new(self.time, EventPayload::MerchantDeparted {
                            owner: visit.customer, system: visit.system,
                        }));
                    }
                }
                MerchantPhase::Arriving => {
                    if idle && pos.distance(host) <= crate::transit::HYPERLIMIT {
                        if let Some(v) = self.merchants.get_mut(&ship) {
                            v.phase = MerchantPhase::Docked;
                            v.arrived_at = Some(self.time);
                        }
                        events.push(Event::new(self.time, EventPayload::MerchantArrived {
                            owner: visit.customer, system: visit.system,
                        }));
                    }
                }
                MerchantPhase::Docked => {}
                MerchantPhase::Leaving => {
                    if idle || pos.distance(host) >= DEPARTURE_DISTANCE_SU {
                        self.retire_merchant(ship);
                    }
                }
            }
        }
    }

    /// The unit vector from `pos` toward the Market Hub (any direction if
    /// they coincide): merchants come from, and return to, the Hub's side.
    fn hubward(&self, pos: Vec2) -> Vec2 {
        let to_hub = self.hub - pos;
        if to_hub.length() > 1.0 { to_hub.normalized() } else { Vec2::new(1.0, 0.0) }
    }

    /// §merchant: where the merchant shows as berthed while it trades: its
    /// host home, for the View alone. The sim's own [`World::dock_of`] stays
    /// ownership-based, so the merchant never refuels from the home's stockpile
    /// or counts as one of its fleets. Reported docked, it leaves the galaxy map
    /// like any berthed fleet (it no longer covers the home star) and orbits
    /// the home world in the System View.
    pub fn merchant_berth(&self, fleet: EntityId) -> Option<DockSite> {
        self.merchants.get(&fleet).filter(|visit| visit.phase == MerchantPhase::Docked)
            .map(|visit| DockSite::System(visit.system))
    }

    fn retire_merchant(&mut self, ship: EntityId) {
        self.merchants.remove(&ship);
        self.fleets.remove(&ship);
    }

    /// `MerchantTrade`: one atomic exchange with the merchant docked at the
    /// trader's home, or a refusal naming the first thing that doesn't fit.
    pub(super) fn merchant_trade(&mut self, owner: PlayerId, buys: &BTreeMap<Commodity, u32>,
        sells: &BTreeMap<Commodity, u32>, events: &mut Vec<Event>)
    {
        let Some((&ship, visit)) = self.merchants.iter().find(|(_, v)| v.customer == owner) else {
            if let Some(home) = self.players.get(&owner).and_then(|c| c.home_system) {
                events.push(self.merchant_refusal(owner, home, "No merchant in orbit"));
            }
            return;
        };
        let system = visit.system;
        let refusal = if visit.phase != MerchantPhase::Docked {
            Some("The merchant isn't docked")
        } else if self.charter_of(owner).exchange_closed() {
            Some("Your charter bars trade")
        } else {
            None
        };
        if let Some(reason) = refusal {
            events.push(self.merchant_refusal(owner, system, reason));
            return;
        }
        let quote = match visit.quote(buys, sells) {
            Ok(quote) => quote,
            Err(reason) => {
                events.push(self.merchant_refusal(owner, system, reason));
                return;
            }
        };
        let Some(home) = self.systems.iter().find(|s| s.id == system) else { return };
        let credits = self.players.get(&owner).map_or(0.0, |c| c.credits);
        let refusal = if sells.iter().any(|(&c, &n)| home.free_stock(c) + 1e-9 < n as f64) {
            Some("Not enough ore at home")
        } else if quote.net_units > 0 && home.storage_headroom() + 1e-9 < quote.net_units as f64 {
            Some("Not enough storage room at home")
        } else if credits + quote.revenue + 1e-9 < quote.cost {
            Some("Not enough credits")
        } else {
            None
        };
        if let Some(reason) = refusal {
            events.push(self.merchant_refusal(owner, system, reason));
            return;
        }

        if let Some(visit) = self.merchants.get_mut(&ship) {
            for (&c, &n) in buys {
                if let Some(left) = visit.stock.get_mut(&c) { *left -= n; }
            }
            visit.ore_bought += sells.get(&Commodity::MetallicOre).copied().unwrap_or(0);
        }
        if let Some(home) = self.systems.iter_mut().find(|s| s.id == system) {
            for (&c, &n) in buys { *home.stockpile.entry(c).or_insert(0.0) += n as f64; }
            for (&c, &n) in sells { *home.stockpile.entry(c).or_insert(0.0) -= n as f64; }
        }
        if let Some(corp) = self.players.get_mut(&owner) {
            corp.credits += quote.revenue - quote.cost;
            corp.founding.ore_sold += sells.get(&Commodity::MetallicOre).copied().unwrap_or(0);
        }
        let kept = |m: &BTreeMap<Commodity, u32>| m.iter().filter(|(_, n)| **n > 0).map(|(&c, &n)| (c, n)).collect();
        events.push(Event::new(self.time, EventPayload::MerchantTraded {
            owner, system, bought: kept(buys), sold: kept(sells), paid: quote.cost, received: quote.revenue,
        }));
    }

    fn merchant_refusal(&self, owner: PlayerId, system: EntityId, reason: &str) -> Event {
        Event::new(self.time, EventPayload::MerchantTradeRefused { owner, system, reason: reason.into() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TICK_HZ;
    use crate::merchant::{FOUNDING_MARKUP, MAX_STAY_S, ORE_CAP};
    use crate::settlement::FoundingProfile;

    fn founder() -> (World, PlayerId, EntityId) {
        let mut w = World::new(SimConfig::for_players(12648430, 2));
        w.enclaves.clear();
        let owner = PlayerId(971);
        w.step(&[Command::FoundColony { id: owner, name: "Merchant test".into(), profile: FoundingProfile::CoastalPlains }]);
        let home = w.players[&owner].home_system.unwrap();
        (w, owner, home)
    }

    fn run(w: &mut World, seconds: f64) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..(seconds * TICK_HZ as f64) as usize { events.extend(w.step(&[])); }
        events
    }

    /// §charter-v5: the report of the privateer's raid reaches command; the
    /// trader is called.
    fn raided(w: &mut World, owner: PlayerId) {
        let now = w.time;
        w.players.get_mut(&owner).unwrap().founding.raid_report_at = Some(now);
    }

    fn docked(w: &mut World, owner: PlayerId) -> EntityId {
        raided(w, owner);
        let events = run(w, 60.0);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::MerchantArrived { owner: o, .. } if o == owner)),
            "the merchant docks within a minute of the raid report");
        let (&ship, visit) = w.merchants.iter().find(|(_, v)| v.customer == owner).unwrap();
        assert_eq!(visit.phase, MerchantPhase::Docked);
        ship
    }

    fn stock(w: &World, home: EntityId, c: Commodity) -> f64 {
        w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&c).copied().unwrap_or(0.0)
    }

    #[test]
    fn a_merchant_comes_once_the_raid_is_reported_and_only_once() {
        let (mut w, owner, home) = founder();
        run(&mut w, 30.0);
        assert!(w.merchants.is_empty(), "no merchant before the raid");
        let later = w.time + 10.0;
        w.players.get_mut(&owner).unwrap().founding.raid_report_at = Some(later);
        run(&mut w, 5.0);
        assert!(w.merchants.is_empty(), "nor before its report arrives");
        let ship = docked(&mut w, owner);
        let visit = &w.merchants[&ship];
        assert_eq!((visit.system, visit.customer, visit.markup), (home, owner, FOUNDING_MARKUP));
        assert!(w.fleets[&ship].owner.is_tca(), "a free trader under Authority protection");
        assert!(w.players[&owner].founding.merchant_called);
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        assert!(w.fleets[&ship].pos.distance(home_pos) <= crate::transit::HYPERLIMIT);
        // The View shows it berthed at the home (off the galaxy map, orbiting
        // the home world); the sim's own docking never refuels it from there.
        assert_eq!(w.merchant_berth(ship), Some(DockSite::System(home)));
        assert_eq!(w.dock_of(ship), None);

        // Fulfilled: it leaves, flies clear and is gone, and never returns.
        let now = w.time;
        w.players.get_mut(&owner).unwrap().founding.set_stage(FoundingStage::Complete, now);
        let events = run(&mut w, 1.0);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::MerchantDeparted { owner: o, .. } if o == owner)));
        assert_eq!(w.merchants[&ship].phase, MerchantPhase::Leaving);
        assert_eq!(w.merchant_berth(ship), None, "leaving, it is back on the map");
        run(&mut w, 300.0);
        assert!(!w.merchants.contains_key(&ship) && !w.fleets.contains_key(&ship), "it flew clear and vanished");
        w.players.get_mut(&owner).unwrap().founding.stage = FoundingStage::PrepareExport;
        run(&mut w, 30.0);
        assert!(w.merchants.is_empty(), "one founding visit per corporation");
    }

    /// §charter-v5 (Peter, Oct 16 2026): it leaves while the founder
    /// eliminates the pirates: a last call once the privateer's kill is
    /// reported, cut short when the den falls.
    #[test]
    fn it_leaves_after_a_last_call_once_the_privateers_kill_is_reported() {
        let (mut w, owner, _) = founder();
        let ship = docked(&mut w, owner);
        w.players.get_mut(&owner).unwrap().founding.set_stage(FoundingStage::DefeatPrivateer, w.time);
        run(&mut w, 1.0);
        assert_eq!(w.merchants[&ship].phase, MerchantPhase::Docked, "it stays through the hunt");
        let paid = w.time;
        w.players.get_mut(&owner).unwrap().founding.reward_granted = true;
        run(&mut w, crate::merchant::LAST_CALL_S - 5.0);
        assert_eq!(w.merchants[&ship].phase, MerchantPhase::Docked, "a last call for the bounty's spending");
        assert!((w.merchants[&ship].leave_by - (paid + crate::merchant::LAST_CALL_S)).abs() < 1.0, "the client's clock");
        let events = run(&mut w, 10.0);
        assert_eq!(w.merchants[&ship].phase, MerchantPhase::Leaving, "then it leaves");
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::MerchantDeparted { owner: o, .. } if o == owner)));
    }

    #[test]
    fn the_dens_fall_cuts_the_last_call_short() {
        let (mut w, owner, _) = founder();
        let ship = docked(&mut w, owner);
        w.players.get_mut(&owner).unwrap().founding.reward_granted = true;
        run(&mut w, 30.0);
        assert_eq!(w.merchants[&ship].phase, MerchantPhase::Docked);
        let now = w.time;
        w.players.get_mut(&owner).unwrap().founding.den_report_at = Some(now);
        run(&mut w, 1.0);
        assert_eq!(w.merchants[&ship].phase, MerchantPhase::Leaving, "gone by the export step");
    }

    #[test]
    fn its_stay_runs_out_after_three_hours() {
        let (mut w, owner, _) = founder();
        let ship = docked(&mut w, owner);
        w.merchants.get_mut(&ship).unwrap().leave_by = w.time + 1.0;
        run(&mut w, 2.0);
        assert_eq!(w.merchants[&ship].phase, MerchantPhase::Leaving);
        assert!(MAX_STAY_S >= 3.0 * 3600.0);
    }

    #[test]
    fn trades_settle_whole_or_are_refused_with_a_reason() {
        let (mut w, owner, home) = founder();
        let ship = docked(&mut w, owner);
        let hub = crate::market::base_price;
        let ec_mid = w.merchants[&ship].mids[&Commodity::Electronics];
        let credits = w.players[&owner].credits;
        let before = stock(&w, home, Commodity::Electronics);
        let trade = |buys: Vec<(Commodity, u32)>, sells: Vec<(Commodity, u32)>| Command::MerchantTrade {
            player_id: owner, buys: buys.into_iter().collect(), sells: sells.into_iter().collect(),
        };

        let events = w.step(&[trade(vec![(Commodity::Electronics, 15)], vec![])]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::MerchantTraded { owner: o, paid, .. }
            if *o == owner && (*paid - 15.0 * ec_mid * 1.5).abs() < 1e-6)), "{events:?}");
        assert_eq!(stock(&w, home, Commodity::Electronics), before + 15.0);
        assert!((w.players[&owner].credits - (credits - 15.0 * ec_mid * 1.5)).abs() < 1e-6);
        assert_eq!(w.merchants[&ship].stock[&Commodity::Electronics], 85);
        assert!(ec_mid >= hub(Commodity::Electronics) * 0.5, "its prices come from the Hub");

        let refused = |w: &mut World, cmd: Command| w.step(&[cmd]).iter().find_map(|e| match &e.payload {
            EventPayload::MerchantTradeRefused { owner: o, reason, .. } if *o == owner => Some(reason.clone()),
            _ => None,
        });
        assert_eq!(refused(&mut w, trade(vec![(Commodity::Electronics, 86)], vec![])).as_deref(),
            Some("The merchant hasn't that much"));
        assert_eq!(refused(&mut w, trade(vec![], vec![(Commodity::MetallicOre, ORE_CAP + 1)])).as_deref(),
            Some("The merchant won't take that much ore"));
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(Commodity::MetallicOre, 10.0);
        assert_eq!(refused(&mut w, trade(vec![], vec![(Commodity::MetallicOre, 20)])).as_deref(),
            Some("Not enough ore at home"));
        let ore = w.step(&[trade(vec![], vec![(Commodity::MetallicOre, 10)])]);
        assert!(ore.iter().any(|e| matches!(e.payload, EventPayload::MerchantTraded { .. })), "selling ore pays");
        assert_eq!(w.merchants[&ship].ore_bought, 10);
        assert_eq!(w.players[&owner].founding.ore_sold, 10, "the charter hears of the sale");
        w.players.get_mut(&owner).unwrap().credits = 10.0;
        assert_eq!(refused(&mut w, trade(vec![(Commodity::Machinery, 5)], vec![])).as_deref(), Some("Not enough credits"));
        w.players.get_mut(&owner).unwrap().credits = 1_000_000.0;
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let room = sys.storage_headroom();
        *sys.stockpile.entry(Commodity::Provisions).or_insert(0.0) += room;
        assert_eq!(refused(&mut w, trade(vec![(Commodity::Alloys, 10)], vec![])).as_deref(),
            Some("Not enough storage room at home"));
        assert_eq!(w.merchants[&ship].stock[&Commodity::Alloys], 120, "a refused trade changes nothing");
    }

    #[test]
    fn only_its_customer_trades_and_only_while_docked() {
        let (mut w, owner, _) = founder();
        let stranger = PlayerId(972);
        w.step(&[Command::FoundColony { id: stranger, name: "Other".into(), profile: FoundingProfile::CoastalPlains }]);
        let ship = docked(&mut w, owner);
        let events = w.step(&[Command::MerchantTrade { player_id: stranger,
            buys: BTreeMap::from([(Commodity::Alloys, 1)]), sells: BTreeMap::new() }]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::MerchantTradeRefused { owner, reason, .. }
            if *owner == stranger && reason == "No merchant in orbit")));
        w.merchants.get_mut(&ship).unwrap().phase = MerchantPhase::Leaving;
        let events = w.step(&[Command::MerchantTrade { player_id: owner,
            buys: BTreeMap::from([(Commodity::Alloys, 1)]), sells: BTreeMap::new() }]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::MerchantTradeRefused { reason, .. }
            if reason == "The merchant isn't docked")));
    }
}

//! §claims (docs/scarcity-staples-plan.md step 1.7; Peter, Nov 10 2026): the
//! Authority's claims on new colonies. The rules and the price are in
//! `crate::claims`; this is the purchase at the Hub, the week's lapse and the
//! check where a Colony Ship founds its colony (`resolve_colony_arrivals`).
use super::*;
use crate::claims::{self, Claim, ClaimRefusal};

impl World {
    /// What the Authority asks `owner` for a claim on `system` now, or why it
    /// won't sell one. The one rule for the view and the purchase.
    pub fn claim_offer(&self, owner: PlayerId, system: EntityId) -> Result<f64, ClaimRefusal> {
        let Some(corp) = self.players.get(&owner) else { return Err(ClaimRefusal::NotForSale) };
        if self.is_rebel(owner) {
            return Err(ClaimRefusal::AtWar);
        }
        let Some(sys) = self.systems.iter().find(|s| s.id == system) else {
            return Err(ClaimRefusal::NotForSale);
        };
        if sys.owner.is_some() || self.home_slots.iter().any(|h| h.system == Some(system))
            || self.sealed_world(system)
        {
            return Err(ClaimRefusal::NotForSale);
        }
        if corp.claims.contains_key(&system) {
            return Err(ClaimRefusal::AlreadyHeld);
        }
        if self.players.iter().any(|(id, c)| *id != owner && c.claims.contains_key(&system)) {
            return Err(ClaimRefusal::HeldByAnother);
        }
        if self.charter_of(owner).exchange_closed() {
            return Err(ClaimRefusal::CharterClosed);
        }
        Ok(claims::price(sys.claim_cost, self.colonies_beyond_home(owner), corp.claims.len(), corp.tca_standing))
    }

    /// The systems `owner` holds past its home.
    pub fn colonies_beyond_home(&self, owner: PlayerId) -> usize {
        let home = self.players.get(&owner).and_then(|c| c.home_system);
        self.systems.iter().filter(|s| s.owner == Some(owner) && Some(s.id) != home).count()
    }

    /// May `owner` found a colony at `system`? Its claim, war with the
    /// Authority, or a found sealed world (landing there is war, claim or none).
    pub(super) fn may_colonize(&self, owner: PlayerId, system: EntityId) -> bool {
        self.is_rebel(owner) || self.sealed_world(system)
            || self.players.get(&owner).is_some_and(|c| c.claims.contains_key(&system))
    }

    /// A sealed world once found: the oldest clause of every charter forbids
    /// landing there, so the Authority sells no claim on it, and a landing is
    /// war (`precursor_landed`).
    fn sealed_world(&self, system: EntityId) -> bool {
        self.season.precursors.get(&system).is_some_and(|p| !p.found.is_empty())
    }

    /// Test setup: hand `owner` the claim on `system`, as if bought just now.
    #[cfg(test)]
    pub(crate) fn grant_claim(&mut self, owner: PlayerId, system: EntityId) {
        let now = self.time;
        self.players.get_mut(&owner).expect("a corporation").claims
            .insert(system, Claim { bought_at: now, lapses_at: now + claims::CLAIM_LAPSE_S });
    }

    /// `Command::BuyClaim` at the Hub. Paid orders see only the money they
    /// carried as the corporation's credits (`apply_paid`); what isn't spent
    /// travels home.
    pub(super) fn buy_claim(&mut self, owner: PlayerId, system: EntityId, events: &mut Vec<Event>) {
        let now = self.time;
        let cost = match self.claim_offer(owner, system) {
            Ok(cost) => cost,
            Err(reason) => {
                events.push(Event::new(now, EventPayload::ClaimRefused { owner, system, reason }));
                return;
            }
        };
        let Some(corp) = self.players.get_mut(&owner) else { return };
        if corp.credits + 1e-9 < cost {
            events.push(Event::new(now, EventPayload::ClaimRefused {
                owner, system, reason: ClaimRefusal::CantAfford { cost },
            }));
            return;
        }
        corp.credits -= cost; // burned: a sink, like a licence
        let lapses_at = now + claims::CLAIM_LAPSE_S;
        corp.claims.insert(system, Claim { bought_at: now, lapses_at });
        events.push(Event::new(now, EventPayload::ClaimBought { owner, system, cost, lapses_at }));
    }

    /// A claim held a week unused lapses; one on a system that has become
    /// anyone's colony is void.
    pub(super) fn tick_claims(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        if self.players.values().all(|c| c.claims.is_empty()) {
            return;
        }
        let taken: std::collections::BTreeSet<EntityId> =
            self.systems.iter().filter(|s| s.owner.is_some()).map(|s| s.id).collect();
        for (&owner, corp) in self.players.iter_mut() {
            corp.claims.retain(|system, _| !taken.contains(system));
            let lapsed: Vec<EntityId> = corp.claims.iter()
                .filter(|(_, c)| c.lapses_at <= now)
                .map(|(s, _)| *s)
                .collect();
            for system in lapsed {
                corp.claims.remove(&system);
                events.push(Event::new(now, EventPayload::ClaimLapsed { owner, system }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SimConfig;

    fn fixture() -> (World, PlayerId, PlayerId, EntityId) {
        let mut w = World::new(SimConfig::for_players(321, 4));
        let (a, b) = (PlayerId(7101), PlayerId(7102));
        w.step(&[Command::AddPlayer { id: a, name: "Claimant".into() },
            Command::AddPlayer { id: b, name: "Rival".into() }]);
        w.enclaves.clear();
        // §progression: a claim costs what its colony returns.
        for id in [a, b] {
            w.players.get_mut(&id).unwrap().credits = 1_000_000.0;
        }
        let home = w.players[&a].home_system.unwrap();
        let home_pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let target = w.systems.iter()
            .filter(|s| s.owner.is_none() && !w.home_slots.iter().any(|h| h.system == Some(s.id)))
            .min_by(|x, y| x.pos.distance(home_pos).total_cmp(&y.pos.distance(home_pos)))
            .unwrap().id;
        (w, a, b, target)
    }

    #[test]
    fn a_claim_is_sold_once_lapses_after_a_week_and_climbs_with_what_is_held() {
        let (mut w, a, b, target) = fixture();
        let first = w.claim_offer(a, target).unwrap();
        let cost = w.systems.iter().find(|s| s.id == target).unwrap().claim_cost;
        assert!((first - cost * crate::tca::tariff_mult(w.players[&a].tca_standing)).abs() < 1e-6);
        let mut events = Vec::new();
        w.buy_claim(a, target, &mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::ClaimBought { system, .. } if system == target)));
        assert!(w.may_colonize(a, target));
        assert_eq!(w.claim_offer(a, target), Err(ClaimRefusal::AlreadyHeld));
        assert_eq!(w.claim_offer(b, target), Err(ClaimRefusal::HeldByAnother), "one holder at a time");
        // The next claim costs 1.6× for the claim already held.
        let other = w.systems.iter()
            .find(|s| s.owner.is_none() && s.id != target && !w.home_slots.iter().any(|h| h.system == Some(s.id)))
            .unwrap().id;
        let other_cost = w.systems.iter().find(|s| s.id == other).unwrap().claim_cost;
        let next = w.claim_offer(a, other).unwrap();
        assert!((next / (other_cost * crate::tca::tariff_mult(w.players[&a].tca_standing)) - 1.6).abs() < 1e-9);
        // A week on, unused, it lapses and the system is for sale again.
        w.time += claims::CLAIM_LAPSE_S;
        let mut events = Vec::new();
        w.tick_claims(&mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::ClaimLapsed { system, .. } if system == target)));
        assert!(!w.may_colonize(a, target));
        assert!(w.claim_offer(b, target).is_ok());
    }

    #[test]
    fn standing_moves_the_price() {
        let (mut w, a, _, target) = fixture();
        let full = w.claim_offer(a, target).unwrap();
        w.players.get_mut(&a).unwrap().tca_standing = 150.0;
        assert!((w.claim_offer(a, target).unwrap() / full - 0.70).abs() < 1e-9, "Preferred: ×0.70, as licences");
        w.players.get_mut(&a).unwrap().tca_standing = 10.0;
        assert_eq!(w.claim_offer(a, target), Err(ClaimRefusal::CharterClosed), "Revoked: not sold");
    }

    #[test]
    fn at_war_or_on_a_found_sealed_world_no_claim_is_needed_or_sold() {
        let (mut w, a, b, target) = fixture();
        w.season.rebels.insert(a, Default::default());
        assert_eq!(w.claim_offer(a, target), Err(ClaimRefusal::AtWar));
        assert!(w.may_colonize(a, target), "at war: no claim needed");
        // A sealed world, once found: the charter forbids landing, so the
        // Authority sells no claim, and a landing (war) needs none.
        let sealed = w.systems.iter()
            .find(|s| s.owner.is_none() && s.id != target && !w.home_slots.iter().any(|h| h.system == Some(s.id)))
            .unwrap().id;
        w.season.precursors.insert(sealed, crate::season::Precursor { found: vec![(b, 0.0)], ..Default::default() });
        assert_eq!(w.claim_offer(b, sealed), Err(ClaimRefusal::NotForSale));
        assert!(w.may_colonize(b, sealed));
    }

    #[test]
    fn a_refused_claim_order_costs_nothing_and_its_payment_comes_home() {
        let (mut w, a, b, target) = fixture();
        w.grant_claim(a, target);
        let price = claims::price(w.systems.iter().find(|s| s.id == target).unwrap().claim_cost, 0, 0,
            w.players[&b].tca_standing);
        w.players.get_mut(&b).unwrap().credits = 10.0 * price;
        let start = w.players[&b].credits;
        let mut events = w.step(&[Command::BuyClaim { player_id: b, system_id: target }]);
        assert!(w.players[&b].credits < start, "the order leaves with its payment");
        for _ in 0..(40 * crate::config::TICK_HZ as usize * 60) {
            events.extend(w.step(&[]));
            if (w.players[&b].credits - start).abs() < 1.0 {
                break;
            }
        }
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::ClaimRefused {
            owner, reason: ClaimRefusal::HeldByAnother, .. } if owner == b)), "refused at the Hub");
        assert!((w.players[&b].credits - start).abs() < 1.0, "the payment came home");
        assert!(w.players[&b].claims.is_empty());
    }

    #[test]
    fn a_colony_ship_without_the_claim_founds_nothing_until_it_is_bought() {
        let (mut w, a, _, target) = fixture();
        let pos = w.systems.iter().find(|s| s.id == target).unwrap().pos;
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, a, ShipKind::Colony, pos, FleetOrder::Idle, None));
        let mut events = Vec::new();
        w.resolve_colony_arrivals(&mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::ClaimMissing { system, .. } if system == target)));
        assert_eq!(w.systems.iter().find(|s| s.id == target).unwrap().owner, None, "no claim, no colony");
        let mut again = Vec::new();
        w.resolve_colony_arrivals(&mut again);
        assert!(!again.iter().any(|e| matches!(e.payload, EventPayload::ClaimMissing { .. })), "told once");
        w.buy_claim(a, target, &mut Vec::new());
        w.resolve_colony_arrivals(&mut Vec::new());
        assert_eq!(w.systems.iter().find(|s| s.id == target).unwrap().owner, Some(a), "the claim held, it settles");
        assert!(w.players[&a].claims.is_empty(), "the claim is used up");
    }
}

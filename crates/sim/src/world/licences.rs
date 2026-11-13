//! §licences (docs/blueprint-licences.md, Peter Nov 8 2026): research is the
//! skill to build a hull, the Authority's licence the right to. Every Orbital
//! Shipyard and Capital Slipway hull needs one. Bought once at the Hub and
//! kept for good; suspended while the charter is Revoked or Proscribed; a
//! syndicate at war with the Authority needs none.
use super::*;
use crate::licence::{self, LicenceRefusal};

impl World {
    /// May `owner` lay a keel of `kind`, as far as licences go?
    pub(super) fn licensed(&self, owner: PlayerId, kind: ShipKind) -> bool {
        !licence::needs_licence(kind)
            || self.is_rebel(owner)
            || (self.players.get(&owner).is_some_and(|c| c.licences.contains_key(&kind))
                && !self.charter_of(owner).exchange_closed())
    }

    /// What the Authority asks `owner` for `kind`'s licence now, or why it
    /// won't sell one. The one rule for the view and the purchase.
    pub fn licence_offer(&self, owner: PlayerId, kind: ShipKind) -> Result<f64, LicenceRefusal> {
        let Some(corp) = self.players.get(&owner) else { return Err(LicenceRefusal::NotRequired) };
        licence::offer(kind, corp.tca_standing, self.hull_researched(owner, kind),
            corp.licences.contains_key(&kind), self.is_rebel(owner))
    }

    /// Is `kind`'s research done (or does it need none)? Research lives at
    /// headquarters, so the corporation always knows it.
    pub fn hull_researched(&self, owner: PlayerId, kind: ShipKind) -> bool {
        !crate::ship::requires_hull_unlock(kind) || self.research_hull(owner, kind)
    }

    /// `Command::BuyLicence` at the Hub. Paid orders see only the money they
    /// carried as the corporation's credits (`apply_paid`); what isn't spent
    /// travels home.
    pub(super) fn buy_licence(&mut self, owner: PlayerId, hull: ShipKind, events: &mut Vec<Event>) {
        let now = self.time;
        let cost = match self.licence_offer(owner, hull) {
            Ok(cost) => cost,
            Err(reason) => {
                events.push(Event::new(now, EventPayload::LicenceRefused { owner, hull, reason }));
                return;
            }
        };
        let Some(corp) = self.players.get_mut(&owner) else { return };
        if corp.credits + 1e-9 < cost {
            events.push(Event::new(now, EventPayload::LicenceRefused {
                owner, hull, reason: LicenceRefusal::CantAfford { cost },
            }));
            return;
        }
        corp.credits -= cost; // burned: a sink, like the reinstatement fee
        corp.licences.insert(hull, now);
        events.push(Event::new(now, EventPayload::LicenceBought { owner, hull, cost }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{BuildKind, StructureKind};
    use crate::config::SimConfig;
    use crate::module::Loadout;

    const DESTROYER_RESEARCH: &str = "hull_line_iv_destroyer";

    fn fixture() -> (World, PlayerId, EntityId) {
        let mut w = World::new(SimConfig::for_players(123, 4));
        let owner = PlayerId(6900);
        w.step(&[Command::AddPlayer { id: owner, name: "Licensee".into() }]);
        w.enclaves.clear();
        let home = w.players[&owner].home_system.unwrap();
        (w, owner, home)
    }

    /// An Orbital Shipyard at home, a Destroyer's parts in stock and its
    /// research done: everything but the licence.
    fn ready_for_a_destroyer(w: &mut World, owner: PlayerId, home: EntityId) {
        let what = BuildKind::Ship { ship: ShipKind::Destroyer };
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.set_tier(StructureKind::NavalDrydock, 1);
        for (good, _) in crate::build::recipe_for(what).costs {
            sys.stockpile.insert(*good, 10_000.0);
        }
        w.players.get_mut(&owner).unwrap().research.completed.insert(DESTROYER_RESEARCH.into());
    }

    fn rejected_for_licence(events: &[Event]) -> bool {
        events.iter().any(|e| matches!(e.payload, EventPayload::BuildRejected {
            reason: crate::event::BuildRejectReason::NeedsLicence, .. }))
    }

    #[test]
    fn a_destroyer_needs_the_authoritys_licence_as_well_as_its_research() {
        let (mut w, owner, home) = fixture();
        ready_for_a_destroyer(&mut w, owner, home);
        let stock = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
        let what = BuildKind::Ship { ship: ShipKind::Destroyer };
        let mut events = Vec::new();
        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut events);
        assert!(w.build_queue.is_empty(), "researched but unlicensed: no keel");
        assert!(rejected_for_licence(&events));
        assert_eq!(w.systems.iter().find(|s| s.id == home).unwrap().stockpile, stock, "a refusal spends nothing");

        w.players.get_mut(&owner).unwrap().credits = 50_000.0;
        let credits = w.players[&owner].credits;
        let price = w.licence_offer(owner, ShipKind::Destroyer).expect("for sale once researched");
        assert!((price - crate::licence::price(ShipKind::Destroyer, 100.0)).abs() < 1e-6, "full standing, full price");
        let mut events = Vec::new();
        w.buy_licence(owner, ShipKind::Destroyer, &mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::LicenceBought { hull: ShipKind::Destroyer, .. })));
        assert!((w.players[&owner].credits - (credits - price)).abs() < 1e-6, "the price is burned");
        assert_eq!(w.licence_offer(owner, ShipKind::Destroyer), Err(LicenceRefusal::AlreadyHeld), "bought once");

        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut Vec::new());
        assert_eq!(w.build_queue.len(), 1, "licensed: the keel is laid");
    }

    #[test]
    fn the_authority_refuses_a_licence_before_the_research_and_for_a_ground_hull() {
        let (mut w, owner, _) = fixture();
        assert_eq!(w.licence_offer(owner, ShipKind::Destroyer), Err(LicenceRefusal::NeedsResearch));
        assert_eq!(w.licence_offer(owner, ShipKind::Corvette), Err(LicenceRefusal::NotRequired));
        let credits = w.players[&owner].credits;
        let mut events = Vec::new();
        w.buy_licence(owner, ShipKind::Destroyer, &mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::LicenceRefused {
            reason: LicenceRefusal::NeedsResearch, .. })));
        assert_eq!(w.players[&owner].credits, credits, "a refusal is free");
        assert!(w.players[&owner].licences.is_empty());
    }

    #[test]
    fn standing_moves_the_price_and_capitals_wait_for_commissioned() {
        let (mut w, owner, _) = fixture();
        let research = &mut w.players.get_mut(&owner).unwrap().research.completed;
        for id in [DESTROYER_RESEARCH, "hull_line_vii_dreadnought"] {
            research.insert(id.into());
        }
        let full = w.licence_offer(owner, ShipKind::Destroyer).unwrap();
        w.players.get_mut(&owner).unwrap().tca_standing = 150.0;
        assert!((w.licence_offer(owner, ShipKind::Destroyer).unwrap() / full - 0.70).abs() < 1e-9, "Preferred: ×0.70");
        assert_eq!(w.licence_offer(owner, ShipKind::Dreadnought), Err(LicenceRefusal::NeedsCommissioned));
        w.players.get_mut(&owner).unwrap().tca_standing = 175.0;
        assert!((w.licence_offer(owner, ShipKind::Destroyer).unwrap() / full - 0.50).abs() < 1e-9, "Commissioned: half");
        assert!(w.licence_offer(owner, ShipKind::Dreadnought).is_ok(), "Commissioned opens the capitals");
        w.players.get_mut(&owner).unwrap().tca_standing = 60.0;
        assert!((w.licence_offer(owner, ShipKind::Destroyer).unwrap() / full - 2.0).abs() < 1e-9, "Suspended: double");
        w.players.get_mut(&owner).unwrap().tca_standing = 10.0;
        assert_eq!(w.licence_offer(owner, ShipKind::Destroyer), Err(LicenceRefusal::CharterClosed), "Revoked: not sold");
    }

    #[test]
    fn a_revoked_charter_suspends_licences_until_standing_returns() {
        let (mut w, owner, home) = fixture();
        ready_for_a_destroyer(&mut w, owner, home);
        w.players.get_mut(&owner).unwrap().licences.insert(ShipKind::Destroyer, 0.0);
        w.players.get_mut(&owner).unwrap().tca_standing = 10.0;
        let what = BuildKind::Ship { ship: ShipKind::Destroyer };
        let mut events = Vec::new();
        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut events);
        assert!(w.build_queue.is_empty() && rejected_for_licence(&events), "Revoked: the licence is suspended");
        w.players.get_mut(&owner).unwrap().tca_standing = 100.0;
        w.apply_build(owner, home, None, what, None, Loadout::default(), &mut Vec::new());
        assert_eq!(w.build_queue.len(), 1, "good standing restores it");
    }

    #[test]
    fn a_syndicate_at_war_with_the_authority_needs_no_licences() {
        let (mut w, owner, home) = fixture();
        ready_for_a_destroyer(&mut w, owner, home);
        w.season.rebels.insert(owner, Default::default());
        assert_eq!(w.licence_offer(owner, ShipKind::Destroyer), Err(LicenceRefusal::AtWar));
        w.apply_build(owner, home, None, BuildKind::Ship { ship: ShipKind::Destroyer }, None, Loadout::default(), &mut Vec::new());
        assert_eq!(w.build_queue.len(), 1, "at war: research is enough");
    }

    #[test]
    fn a_licence_order_carries_its_payment_to_the_hub_and_the_change_comes_home() {
        let (mut w, owner, _) = fixture();
        w.players.get_mut(&owner).unwrap().research.completed.insert(DESTROYER_RESEARCH.into());
        let price = w.licence_offer(owner, ShipKind::Destroyer).unwrap();
        let before = w.players[&owner].credits.max(0.0);
        w.players.get_mut(&owner).unwrap().credits = before + 2.0 * price;
        let start = w.players[&owner].credits;
        let mut events = w.step(&[Command::BuyLicence { player_id: owner, hull: ShipKind::Destroyer }]);
        let paid = start - w.players[&owner].credits;
        assert!((paid - price * (1.0 + super::super::treasury::PAYMENT_MARGIN)).abs() < 1.0,
            "the order leaves with the price and its margin: {paid}");
        // Light to the Hub, the licence, and the change's light home.
        for _ in 0..(40 * crate::config::TICK_HZ as usize * 60) {
            events.extend(w.step(&[]));
            if w.players[&owner].licences.contains_key(&ShipKind::Destroyer)
                && (start - w.players[&owner].credits - price).abs() < 1.0 {
                break;
            }
        }
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::LicenceBought { .. })), "bought at the Hub");
        assert!(w.players[&owner].licences.contains_key(&ShipKind::Destroyer));
        assert!((start - w.players[&owner].credits - price).abs() < 1.0, "only the price is spent; the margin came home");
    }
}

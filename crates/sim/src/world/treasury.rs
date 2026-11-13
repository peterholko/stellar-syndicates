//! §treasury (docs/treasury.md): the corporation's credits sit at its command
//! center, so it always knows them. An order to the Market Hub, or to a
//! colony, takes its payment from the treasury as it leaves and spends only
//! that at the receiver; the change travels home. Money made at the Hub (a
//! sale, a fill, a refund) travels home at lightspeed and lands with the
//! receipt that reports it. Peter, Oct 27 2026: with the credits kept at the
//! Hub, the top bar showed them a minute late and promised money already
//! spent. Automatic buys at the Hub (a route's shopping and fuel, depot
//! rules) have no click to take a payment from: they use money still on its
//! way home first (the Hub nets it out of what it sends, so a freighter's sale
//! pays for its return cargo), then draw on the treasury.
use super::*;
use crate::cargo::Commodity;

/// What an order carries above the going price for anything the market
/// prices: the client's ±10% price protection. The change comes back.
pub(crate) const PAYMENT_MARGIN: f64 = 0.10;

impl World {
    /// What `cmd` takes from the treasury as it leaves: the most it can cost
    /// at its receiver. `None`: it spends nothing.
    pub(super) fn payment_for(&self, cmd: &Command) -> Option<f64> {
        let margin = 1.0 + PAYMENT_MARGIN;
        let fee = |owner: PlayerId, system: EntityId, commodity: Commodity, units: u32| {
            self.systems.iter().find(|s| s.id == system).map_or(0.0, |sys| {
                crate::tca::freight_fee(units, self.market.price(commodity), self.hub.distance(sys.pos))
                    * crate::tca::tariff_mult(self.standing_of(owner)) * self.hub_fee_mult() * margin
            })
        };
        Some(match cmd {
            Command::MarketBuy { player_id, commodity, units, max_unit_price } => {
                let unit = max_unit_price.filter(|p| p.is_finite() && *p > 0.0)
                    .unwrap_or_else(|| self.market.quote_buy(*commodity, *units).unit_price * margin);
                let gross = *units as f64 * unit;
                gross + self.exchange_penalty(*player_id, gross)
            }
            Command::PlaceLimitOrder { side: Side::Buy, units, limit_price, .. } =>
                *units as f64 * limit_price.max(0.0),
            Command::HireSpecialist { .. } => crate::specialist::SPECIALIST_HIRE_COST,
            Command::PayReinstatement { player_id, points } => {
                let standing = self.players.get(player_id)?.tca_standing;
                let restorable = (crate::tca::TCA_STANDING_FULL - standing).max(0.0);
                points.max(0.0).min(restorable) * crate::tca::TCA_REINSTATE_FEE_PER_POINT
            }
            Command::RequestFuelRescue { fleet_id, .. } =>
                self.fuel_rescue_cost(*fleet_id).map_or(0.0, |cost| cost * margin),
            // §licences: the price at today's standing, with the margin for a
            // standing that moves on the way; the change comes back.
            Command::BuyLicence { player_id, hull } =>
                crate::licence::price(*hull, self.standing_of(*player_id)) * margin,
            // §claims: the price as reckoned at home (nothing about rivals'
            // claims), with the margin; the change comes back.
            Command::BuyClaim { player_id, system_id } => {
                let corp = self.players.get(player_id)?;
                let sys = self.systems.iter().find(|s| s.id == *system_id)?;
                crate::claims::price(sys.claim_cost, self.colonies_beyond_home(*player_id), corp.claims.len(),
                    self.standing_of(*player_id)) * margin
            }
            // §season: arrears and credit requisitions leave with their payment.
            Command::PayArrears { player_id, commodity } => self.season.squeeze.get(player_id)?
                .embargoes.get(commodity)?.arrears,
            Command::AnswerRequisition { player_id, id, pay: true } => match self.season.squeeze.get(player_id)?
                .requisitions.iter().find(|r| r.id == *id)?.demand {
                crate::season::Demand::Credits { amount } => amount,
                crate::season::Demand::Goods { .. } => return None,
            },
            Command::BookFreightOut { player_id, system, commodity, units }
            | Command::BookFreightIn { player_id, system, commodity, units, .. }
            | Command::StockSystem { player_id, system_id: system, commodity, units } =>
                fee(*player_id, *system, *commodity, *units),
            _ => return None,
        })
    }

    /// Take `payment` for `cmd` from the treasury, or refuse it at home, at
    /// once, with the reason the receiver would give. Returns whether it paid.
    pub(super) fn take_payment(&mut self, cmd: &Command, payment: f64, events: &mut Vec<Event>) -> bool {
        let owner = cmd.issuer();
        let Some(corp) = self.players.get_mut(&owner) else { return false };
        if corp.credits + 1e-9 >= payment {
            corp.credits -= payment;
            return true;
        }
        let (have, cc) = (corp.credits, corp.command_center);
        let first = events.len();
        match cmd {
            Command::MarketBuy { commodity, units, .. } | Command::PlaceLimitOrder { commodity, units, .. } =>
                self.reject_trade(events, owner, *commodity, *units, None, TradeRejectReason::CantAfford { cost: payment }),
            Command::PayReinstatement { .. } | Command::PayArrears { .. } | Command::AnswerRequisition { .. } =>
                self.reject_trade(events, owner, Commodity::Provisions, 0, None, TradeRejectReason::CantAfford { cost: payment }),
            Command::BookFreightOut { system, commodity, units, .. }
            | Command::BookFreightIn { system, commodity, units, .. }
            | Command::StockSystem { system_id: system, commodity, units, .. } =>
                self.reject_trade(events, owner, *commodity, *units, Some(*system), TradeRejectReason::CannotAffordFee { fee: payment }),
            Command::RequestFuelRescue { fleet_id, .. } => events.push(Event::new(self.time, EventPayload::FuelRescueRejected {
                owner, fleet: *fleet_id, reason: crate::tca::RescueRejectReason::CannotAfford { needed: payment, have },
            })),
            Command::BuyLicence { hull, .. } => events.push(Event::new(self.time, EventPayload::LicenceRefused {
                owner, hull: *hull, reason: crate::licence::LicenceRefusal::CantAfford { cost: payment },
            })),
            Command::BuyClaim { system_id, .. } => events.push(Event::new(self.time, EventPayload::ClaimRefused {
                owner, system: *system_id, reason: crate::claims::ClaimRefusal::CantAfford { cost: payment },
            })),
            _ => events.push(Event::new(self.time, EventPayload::PaymentRefused {
                owner, what: "a specialist".into(), cost: payment,
            })),
        }
        // Refused at home: the news is already here.
        for event in &mut events[first..] {
            event.origin = Some(cc);
        }
        false
    }

    /// `Command::Paid` at its receiver: the order sees only the money it
    /// carried, and whatever it doesn't spend travels home.
    pub(super) fn apply_paid(&mut self, payment: f64, order: &Command, events: &mut Vec<Event>) {
        let owner = order.issuer();
        let from = self.administration_receiver(order).map_or(self.hub, |(_, pos)| pos);
        let Some(treasury) = self.players.get_mut(&owner).map(|c| std::mem::replace(&mut c.credits, payment)) else {
            return;
        };
        self.apply_now(order, events);
        let change = self.players.get_mut(&owner).map_or(0.0, |c| std::mem::replace(&mut c.credits, treasury));
        self.wire_home(owner, change, from);
    }

    /// Send `amount` home from `from`: it lands when its light reaches the
    /// command center, with the receipt for it.
    pub(super) fn wire_home(&mut self, owner: PlayerId, amount: f64, from: Vec2) {
        if amount <= 1e-9 {
            return;
        }
        let (now, c) = (self.time, self.config.c);
        let Some(corp) = self.players.get_mut(&owner) else { return };
        let arrives_at = now + crate::transit::delay(from, corp.command_center, c);
        if arrives_at <= now + 1e-9 {
            corp.credits += amount;
        } else {
            corp.wires.push(Wire { amount, arrives_at });
        }
    }

    /// What an automatic buy at the Hub can reach: the treasury and the
    /// money still on its way home.
    pub(super) fn reachable_at_hub(&self, owner: PlayerId) -> f64 {
        self.players.get(&owner).map_or(0.0, |c| c.credits + c.incoming())
    }

    /// Run an automatic buy at the Hub (`buy` spends `credits` as before)
    /// against [`Self::reachable_at_hub`]: what it spends is netted out of
    /// the money on its way home first (the latest to land first), and only
    /// the rest comes out of the treasury.
    pub(super) fn at_hub<R>(&mut self, owner: PlayerId, buy: impl FnOnce(&mut World) -> R) -> R {
        let Some((treasury, incoming)) = self.players.get_mut(&owner).map(|c| {
            let before = (c.credits, c.incoming());
            c.credits += before.1;
            before
        }) else {
            return buy(self);
        };
        let result = buy(self);
        let Some(corp) = self.players.get_mut(&owner) else { return result };
        let spent = (treasury + incoming - corp.credits).max(0.0);
        let mut netted = spent.min(incoming);
        corp.credits -= incoming - netted;
        corp.wires.sort_by(|a, b| a.arrives_at.total_cmp(&b.arrives_at));
        for wire in corp.wires.iter_mut().rev() {
            let take = wire.amount.min(netted);
            wire.amount -= take;
            netted -= take;
        }
        corp.wires.retain(|w| w.amount > 1e-9);
        result
    }

    /// Land every wire whose light has reached its command center.
    pub(super) fn settle_wires(&mut self) {
        let now = self.time;
        for corp in self.players.values_mut().filter(|c| !c.wires.is_empty()) {
            let landed: f64 = corp.wires.iter().filter(|w| w.arrives_at <= now + 1e-9).map(|w| w.amount).sum();
            corp.wires.retain(|w| w.arrives_at > now + 1e-9);
            corp.credits += landed;
        }
    }

    /// The Authority's price to rescue `fleet_id` now, if it needs fuel.
    pub(super) fn fuel_rescue_cost(&self, fleet_id: EntityId) -> Option<f64> {
        let fleet = self.fleets.get(&fleet_id)?;
        let fuel = self.fuel_rescue_package(fleet_id)?;
        let commodity = fleet.thirstiest().map_or(crate::fuel::MOVEMENT_FUEL, |(c, _)| c);
        Some(crate::tca::TCA_RESCUE_SERVICE_FEE
            + fuel * self.market.price(commodity) * crate::tca::TCA_RESCUE_FUEL_PRICE_MULT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SimConfig;
    use crate::event::TradeEvent;
    use crate::settlement::FoundingProfile;

    fn founder() -> (World, PlayerId) {
        let mut w = World::new(SimConfig::for_players(12648430, 2));
        w.enclaves.clear();
        let owner = PlayerId(971);
        w.step(&[Command::FoundColony { id: owner, name: "Treasury test".into(), profile: FoundingProfile::CoastalPlains }]);
        (w, owner)
    }

    /// Jump the clock to `at` and run one tick there.
    fn reach(w: &mut World, at: f64) -> Vec<Event> {
        w.time = at;
        w.step(&[])
    }

    fn hub_delay(w: &World, owner: PlayerId) -> f64 {
        crate::transit::delay(w.hub, w.players[&owner].command_center, w.config.c)
    }

    #[test]
    fn a_hub_buy_pays_as_it_leaves_and_its_change_comes_home_with_the_receipt() {
        let (mut w, owner) = founder();
        let treasury = w.players[&owner].credits;
        let stocked = w.players[&owner].warehouse.get(&Commodity::Alloys).copied().unwrap_or(0);
        let cap = w.market.quote_buy(Commodity::Alloys, 10).unit_price * (1.0 + PAYMENT_MARGIN);
        let sent = w.time;
        w.step(&[Command::MarketBuy { player_id: owner, commodity: Commodity::Alloys, units: 10, max_unit_price: Some(cap) }]);
        let paid = treasury - w.players[&owner].credits;
        assert!((paid - (10.0 * cap + w.exchange_penalty(owner, 10.0 * cap))).abs() < 1e-6, "the payment leaves with the order");

        let delay = hub_delay(&w, owner);
        assert!(delay > 30.0, "the Hub is far from a home");
        let events = reach(&mut w, sent + delay);
        let cost = events.iter().find_map(|e| match e.payload {
            EventPayload::Trade(TradeEvent::Bought { units, unit_price, penalty, .. }) => Some(units as f64 * unit_price + penalty),
            _ => None,
        }).expect("bought at the Hub");
        assert_eq!(w.players[&owner].warehouse[&Commodity::Alloys], stocked + 10);
        assert_eq!(w.players[&owner].credits, treasury - paid, "the change is still on its way");
        reach(&mut w, sent + 2.0 * delay);
        assert!((w.players[&owner].credits - (treasury - cost)).abs() < 1e-6, "the change lands with the receipt");
    }

    #[test]
    fn a_buy_the_treasury_cannot_cover_is_refused_at_home_at_once() {
        let (mut w, owner) = founder();
        w.players.get_mut(&owner).unwrap().credits = 50.0;
        let events = w.step(&[Command::MarketBuy {
            player_id: owner, commodity: Commodity::Alloys, units: 10, max_unit_price: Some(100.0),
        }]);
        let cc = w.players[&owner].command_center;
        assert!(events.iter().any(|e| e.origin == Some(cc) && matches!(e.payload,
            EventPayload::Trade(TradeEvent::Rejected { reason: TradeRejectReason::CantAfford { .. }, .. }))),
            "refused at home, where the news is at once");
        assert_eq!(w.players[&owner].credits, 50.0);
        assert!(!w.pending_administration.iter().any(|(_, c)| matches!(c, Command::Paid { .. })), "nothing leaves");
    }

    #[test]
    fn at_the_hub_an_order_spends_only_what_it_carried() {
        let (mut w, owner) = founder();
        let treasury = w.players[&owner].credits;
        let order = Command::MarketBuy { player_id: owner, commodity: Commodity::Alloys, units: 10, max_unit_price: None };
        let mut events = Vec::new();
        w.apply_now(&Command::Paid { payment: 5.0, order: Box::new(order) }, &mut events);
        assert!(events.iter().any(|e| matches!(e.payload,
            EventPayload::Trade(TradeEvent::Rejected { reason: TradeRejectReason::CantAfford { .. }, .. }))));
        let corp = &w.players[&owner];
        assert_eq!(corp.credits, treasury, "the Hub can't reach the treasury at home");
        assert!((corp.incoming() - 5.0).abs() < 1e-9, "the payment travels back");
    }

    #[test]
    fn a_sale_at_the_hub_lands_with_its_receipt() {
        let (mut w, owner) = founder();
        w.players.get_mut(&owner).unwrap().warehouse.insert(Commodity::MetallicOre, 100);
        let treasury = w.players[&owner].credits;
        let sent = w.time;
        w.step(&[Command::MarketSell { player_id: owner, commodity: Commodity::MetallicOre, units: 40, min_unit_price: None }]);
        let delay = hub_delay(&w, owner);
        let events = reach(&mut w, sent + delay);
        let net = events.iter().find_map(|e| match e.payload {
            EventPayload::Trade(TradeEvent::Sold { units, unit_price, penalty, .. }) => Some(units as f64 * unit_price - penalty),
            _ => None,
        }).expect("sold at the Hub");
        assert_eq!(w.players[&owner].credits, treasury, "not before its receipt");
        reach(&mut w, sent + 2.0 * delay);
        assert!((w.players[&owner].credits - (treasury + net)).abs() < 1e-6, "it lands with the receipt");
    }

    #[test]
    fn automatic_buys_at_the_hub_net_out_money_on_its_way_home_first() {
        let (mut w, owner) = founder();
        let hub = w.hub;
        w.wire_home(owner, 300.0, hub);
        let treasury = w.players[&owner].credits;
        let spend = |w: &mut World, amount: f64| w.at_hub(owner, |w| w.players.get_mut(&owner).unwrap().credits -= amount);
        spend(&mut w, 100.0);
        assert_eq!(w.players[&owner].credits, treasury);
        assert!((w.players[&owner].incoming() - 200.0).abs() < 1e-9, "netted out of the money on its way home");
        spend(&mut w, 250.0);
        assert!(w.players[&owner].wires.is_empty());
        assert!((w.players[&owner].credits - (treasury - 50.0)).abs() < 1e-9, "the rest from the treasury");
    }
}

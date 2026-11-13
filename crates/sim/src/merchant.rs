//! §merchant: the TRAVELING MERCHANT (`docs/traveling-merchant.md`). A free
//! trader under Authority protection docks at a new corporation's home while
//! its founding charter runs, so the charter's builds need not wait on the
//! Market Hub's 26-minute round trip. §charter-v5: it drops in after the
//! privateer's raid, carrying what the raid took.
//!
//! It is a last resort, not a second Hub (Peter, Oct 13 2026: "the markup
//! should be like 50-100%, the idea the player should only rely on it when
//! needed"). It sells the founding goods well above the Hub and buys a few
//! Alloys well below it, so a round trip through it always loses money.
//! Prices are fixed for the visit: the Hub mids when it set out, which is what
//! light-delayed news from the Hub would show anyway.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};

/// The founding visit's markup over the Hub mid. Peter (Nov 6 2026: "I think
/// the trader should charge more like 100% because of the cost/time for the
/// player"): twice the Hub, the price of goods at the door with no Hub round
/// trip; the charter's stipends are sized to it. Tunable.
pub const FOUNDING_MARKUP: f64 = 1.00;
/// The one good the merchant buys. Peter (Nov 11 2026: "we should just change
/// the sell ore to sell alloy"): the charter's sale (step 9) is the Smelter's
/// Alloys, so no ore floor holds the Smelter back. The stipends fund the
/// trader's parts, so the sale is the lesson, not the money.
pub const BUYS: Commodity = Commodity::Alloys;
/// What the merchant pays for it, as a share of the Hub mid. Tunable.
pub const BUY_PRICE_SHARE: f64 = 0.50;
/// How many it takes in one visit (Peter, Nov 6 2026: "the free trader will
/// only take so much ... before not buying anymore"). A sample; the Landfall
/// crate carries them, so selling the most it takes leaves the Shipyard's
/// Alloys at the next step. Tunable.
pub const BUY_CAP: u32 = 10;
/// The longest a founding visit lasts; it leaves once the charter is
/// fulfilled. §den-contract (Peter, Nov 5 2026: "The free trader doesn't
/// leave after building your own freighter"): it sells the parts of every
/// charter build, so it stays for the whole charter, whose finale alone can
/// wait a day (`founding::VENDETTA_DEADLINE_S`). A week: a colony left
/// mid-charter doesn't keep it docked for good. Tunable.
pub const MAX_STAY_S: f64 = 7.0 * 24.0 * 3600.0;
/// Where it drops out of warp, and how far out it vanishes when it leaves.
/// §charter-v4: close enough to dock about 30 seconds after the reveal.
pub const ARRIVAL_DISTANCE_SU: f64 = 2_500.0;
pub const DEPARTURE_DISTANCE_SU: f64 = 6_000.0;

/// §traders (Peter, Oct 27 2026: "shouldn't we have like 3 different merchant
/// characters at least, that way not all the players see the same merchant"):
/// who flies a corporation's free trader. The names live here for the
/// server's own lines; the portraits and the talk live in the client
/// (`core/derive/trader.ts`), keyed by `slug`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeTrader {
    pub slug: &'static str,
    pub name: &'static str,
    pub first: &'static str,
    /// Its ship's name, without "the".
    pub ship: &'static str,
}

/// Peter (Oct 27 2026): "can we have 4th male free trader".
pub const FREE_TRADERS: [FreeTrader; 4] = [
    FreeTrader { slug: "teo", name: "Teo Brannick", first: "Teo", ship: "Long Odds" },
    FreeTrader { slug: "ines", name: "Ines Varga", first: "Ines", ship: "Clean Slate" },
    FreeTrader { slug: "mags", name: "Mags Halloran", first: "Mags", ship: "Second Wind" },
    FreeTrader { slug: "rafi", name: "Rafi Mensah", first: "Rafi", ship: "Small Fortune" },
];

impl FreeTrader {
    /// Dealt as the pirate family is: from the galaxy seed, the corporation
    /// and its join tick, so it never needs saving and a new galaxy deals
    /// afresh.
    pub fn of(seed: u64, owner: PlayerId, joined_tick: u64) -> &'static FreeTrader {
        let mut rng = crate::rng::Rng::keyed_id(seed, "free-trader-v1", owner.0 ^ joined_tick.rotate_left(32));
        &FREE_TRADERS[(rng.next_u64() % FREE_TRADERS.len() as u64) as usize]
    }
}

/// §charter-v5: the Semiconductors the last call's stock-up adds for research.
/// A running programme draws them from home stock (about 0.6 a minute at tier
/// I) while the charter's last builds go up, so without a reserve it eats the
/// Mining Barge's. Peter (Oct 26 2026): stock up before the trader leaves.
/// Tunable.
pub const RESEARCH_RESERVE: f64 = 20.0;

/// The founding visit's stock. §charter-v5: its semiconductors cover every
/// charter build (80: two Solar Arrays, the Shipyard, both Interceptors, the
/// Academy, the Survey Probe and the Mining Barge) and the research reserve,
/// its Armaments both Interceptors (the raid took the kit's; §vendetta: the
/// uncle's fight needs a second); the rest covers a kit spent on optional
/// builds. §den-stores (Nov 5 2026): the Tiny Freighter's parts, and its
/// fuel: a store of Uranium and a tank of Deuterium (52.5 each) for the
/// den's two loads at Full speed. §smelt-from-landing (Peter, Nov 6 2026): no
/// Alloys; the colony smelts its own from the Interceptor on, and the trader
/// sells "the more value items".
pub const FOUNDING_STOCK: [(Commodity, u32); 13] = [
    (Commodity::Machinery, 50),
    // §hull-ladder (Peter, Nov 7 2026: "the free trader has 1 warp drive and
    // 1 fusion reactor at a discount (debt with interest) if the player takes
    // out the den"): four Fission Reactors and three Zephyr Drives — the
    // first of each, the Interceptor's, FEATURED at half the Hub mid, a debt
    // the den's Authority contract repays; then the Tiny Freighter's pair of
    // each at the usual price, paid from the bounty, and the reactor the
    // barge's three still lack (the kit's six and seven cover the rest). The
    // raid takes both.
    (Commodity::FissionReactor, 4),
    (Commodity::ZephyrDrive, 3),
    // §hull-ladder: the Small Freighter's kit beyond its engines — frame,
    // bays, Nav Computer, plating and precision parts. A Hub import is a
    // light round trip and a freight run away (the charter harness measured
    // fifty minutes for this one hull), so the charter buys them here, at
    // the trader's twice; the uncle's Interceptors are the first Hub parts.
    (Commodity::HullSections, 15),
    (Commodity::CargoBay, 3),
    (Commodity::NavComputer, 1),
    (Commodity::Composites, 30),
    (Commodity::PrecisionComponents, 3),
    // §vendetta-campaign: the third Interceptor's 12 and 15 too.
    (Commodity::Electronics, 127),
    (Commodity::Armaments, 45),
    (Commodity::Polymers, 30),
    (Commodity::Provisions, 40),
    // §reactors-drives (Nov 7 2026): no Deuterium — nothing in the charter
    // burns it, and the trader never stocks a good research has not opened.
    (Commodity::Uranium, 60),
];

/// §shipbuilding-overhaul: the featured good sells its FIRST unit at this
/// share of the Hub mid (the Interceptor's Fission Reactor, ~72 Cr). Tunable.
pub const FEATURED_SHARE: f64 = 0.5;
/// The founding visit's featured goods: the Interceptor's reactor and drive.
pub const FOUNDING_FEATURED: [Commodity; 2] = [Commodity::FissionReactor, Commodity::ZephyrDrive];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MerchantPhase {
    /// Flying in from its drop point; not trading yet.
    Arriving,
    /// Docked at the host system: trading.
    Docked,
    /// Flying out; it vanishes past [`DEPARTURE_DISTANCE_SU`].
    Leaving,
}

/// One merchant visit to one corporation's home.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantVisit {
    /// The merchant's ship on the map (owned by the Authority sentinel).
    pub fleet: EntityId,
    /// The corporation it serves; only it may trade.
    pub customer: PlayerId,
    /// The host system: the corporation's home.
    pub system: EntityId,
    pub phase: MerchantPhase,
    pub arrived_at: Option<f64>,
    /// When it leaves even if the charter isn't fulfilled.
    pub leave_by: f64,
    /// The Hub mids when it set out, per good it trades.
    pub mids: BTreeMap<Commodity, f64>,
    pub markup: f64,
    /// What it still has to sell.
    pub stock: BTreeMap<Commodity, u32>,
    /// [`BUYS`] taken so far this visit.
    #[serde(default)]
    pub bought: u32,
    /// §shipbuilding-overhaul: the good whose next unit sells at
    /// [`FEATURED_SHARE`] of the mid — the trader's one discount, cleared by
    /// the first purchase of it.
    #[serde(default)]
    pub featured: BTreeSet<Commodity>,
}

/// A priced trade: what the player pays, receives and nets in storage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MerchantQuote {
    pub cost: f64,
    pub revenue: f64,
    /// Units into the home stockpile less units out of it.
    pub net_units: i64,
}

impl MerchantVisit {
    pub fn founding(fleet: EntityId, customer: PlayerId, system: EntityId, now: f64,
        mid: impl Fn(Commodity) -> f64) -> Self
    {
        let mut mids: BTreeMap<_, _> = FOUNDING_STOCK.iter().map(|&(c, _)| (c, mid(c))).collect();
        mids.insert(BUYS, mid(BUYS));
        Self {
            fleet,
            customer,
            system,
            phase: MerchantPhase::Arriving,
            arrived_at: None,
            leave_by: now + MAX_STAY_S,
            mids,
            markup: FOUNDING_MARKUP,
            stock: FOUNDING_STOCK.into_iter().collect(),
            bought: 0,
            featured: FOUNDING_FEATURED.into_iter().collect(),
        }
    }

    /// The featured price of `c`'s next unit, if it is the featured good.
    pub fn featured_price(&self, c: Commodity) -> Option<f64> {
        (self.featured.contains(&c) && self.stock.contains_key(&c))
            .then(|| self.mids.get(&c).copied().unwrap_or(0.0) * FEATURED_SHARE)
    }

    /// What `n` units of `c` cost: the featured unit first, the rest at the
    /// usual price.
    pub fn cost_of(&self, c: Commodity, n: u32) -> Option<f64> {
        let price = self.sell_price(c)?;
        Some(match self.featured_price(c) {
            Some(first) if n > 0 => first + price * (n - 1) as f64,
            _ => price * n as f64,
        })
    }

    /// What the merchant charges per unit, if it sells `c`.
    pub fn sell_price(&self, c: Commodity) -> Option<f64> {
        self.stock.contains_key(&c).then(|| self.mids.get(&c).copied().unwrap_or(0.0) * (1.0 + self.markup))
    }

    /// What the merchant pays per unit, if it buys `c` ([`BUYS`] only).
    pub fn buy_price(&self, c: Commodity) -> Option<f64> {
        (c == BUYS).then(|| self.mids.get(&c).copied().unwrap_or(0.0) * BUY_PRICE_SHARE)
    }

    /// [`BUYS`] it will still take this visit.
    pub fn room(&self) -> u32 {
        BUY_CAP.saturating_sub(self.bought)
    }

    /// Price a trade against the merchant's own limits: what it sells and has
    /// in stock, what it buys and still takes. The caller checks the player's
    /// credits, goods and storage.
    pub fn quote(&self, buys: &BTreeMap<Commodity, u32>, sells: &BTreeMap<Commodity, u32>)
        -> Result<MerchantQuote, &'static str>
    {
        if buys.values().chain(sells.values()).all(|n| *n == 0) {
            return Err("Nothing to trade");
        }
        let mut cost = 0.0;
        let mut net_units = 0_i64;
        for (&c, &n) in buys.iter().filter(|(_, n)| **n > 0) {
            let line = self.cost_of(c, n).ok_or("The merchant doesn't sell that")?;
            if n > self.stock.get(&c).copied().unwrap_or(0) {
                return Err("The merchant hasn't that much");
            }
            cost += line;
            net_units += n as i64;
        }
        let mut revenue = 0.0;
        for (&c, &n) in sells.iter().filter(|(_, n)| **n > 0) {
            let price = self.buy_price(c).ok_or("The merchant doesn't buy that")?;
            if c == BUYS && n > self.room() {
                return Err("The merchant won't take that many");
            }
            revenue += price * n as f64;
            net_units -= n as i64;
        }
        Ok(MerchantQuote { cost, revenue, net_units })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visit() -> MerchantVisit {
        MerchantVisit::founding(EntityId(7), PlayerId(1), EntityId(3), 100.0, crate::market::base_price)
    }

    #[test]
    fn founding_prices_sit_well_above_and_below_the_hub() {
        let v = visit();
        let hub = crate::market::base_price;
        // Peter (Nov 6 2026): twice the Hub.
        assert_eq!(v.sell_price(Commodity::Electronics), Some(hub(Commodity::Electronics) * 2.0));
        // Peter (Nov 11 2026): it buys the Smelter's Alloys, at half the Hub.
        assert_eq!(v.buy_price(Commodity::Alloys), Some(hub(Commodity::Alloys) * 0.5));
        assert_eq!(v.sell_price(Commodity::Alloys), None, "it sells no Alloys (Nov 6)");
        assert_eq!(v.buy_price(Commodity::MetallicOre), None, "nor buys ore");
        assert_eq!(v.buy_price(Commodity::Electronics), None, "it buys Alloys only");
        assert_eq!(v.leave_by, 100.0 + MAX_STAY_S);
        assert_eq!(v.phase, MerchantPhase::Arriving);
    }

    #[test]
    fn quotes_respect_stock_and_the_buying_cap() {
        let mut v = visit();
        let buys = BTreeMap::from([(Commodity::Electronics, 15), (Commodity::Machinery, 13)]);
        let sells = BTreeMap::from([(Commodity::Alloys, 8)]);
        let q = v.quote(&buys, &sells).unwrap();
        let hub = crate::market::base_price;
        let cost = (15.0 * hub(Commodity::Electronics) + 13.0 * hub(Commodity::Machinery)) * (1.0 + FOUNDING_MARKUP);
        assert!((q.cost - cost).abs() < 1e-9);
        assert!((q.revenue - 8.0 * hub(Commodity::Alloys) * 0.5).abs() < 1e-9);
        assert_eq!(q.net_units, 15 + 13 - 8);

        let stocked = FOUNDING_STOCK.iter().find(|(c, _)| *c == Commodity::Electronics).unwrap().1;
        assert!(v.quote(&BTreeMap::from([(Commodity::Electronics, stocked + 1)]), &BTreeMap::new()).is_err());
        assert!(v.quote(&BTreeMap::from([(Commodity::Titanium, 1)]), &BTreeMap::new()).is_err());
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::from([(Commodity::MetallicOre, 1)])).is_err(), "no ore");
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::new()).is_err());
        v.bought = BUY_CAP - 3;
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::from([(Commodity::Alloys, 4)])).is_err());
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::from([(Commodity::Alloys, 3)])).is_ok());
    }

    #[test]
    fn a_round_trip_through_the_merchant_always_loses() {
        let v = visit();
        for (c, _) in FOUNDING_STOCK {
            assert!(v.sell_price(c).unwrap() > crate::market::base_price(c), "{c:?}");
        }
        assert!(v.buy_price(BUYS).unwrap() < crate::market::base_price(BUYS));
    }

    /// §shipbuilding-overhaul: the first Fission Core is the trader's one
    /// discount — half the Hub mid; the next two cost the usual twice.
    #[test]
    fn the_first_fission_reactor_is_featured_at_half_the_hub() {
        let mut v = visit();
        let hub = crate::market::base_price(Commodity::FissionReactor);
        assert_eq!(v.featured_price(Commodity::FissionReactor), Some(hub * FEATURED_SHARE));
        assert_eq!(v.cost_of(Commodity::FissionReactor, 1), Some(hub * 0.5));
        assert_eq!(v.cost_of(Commodity::FissionReactor, 3), Some(hub * 0.5 + hub * 2.0 * 2.0));
        let q = v.quote(&BTreeMap::from([(Commodity::FissionReactor, 1)]), &BTreeMap::new()).unwrap();
        assert!((q.cost - hub * 0.5).abs() < 1e-9, "one Interceptor's engine for ~110 Cr");
        v.featured.clear();
        assert_eq!(v.cost_of(Commodity::FissionReactor, 2), Some(hub * 2.0 * 2.0), "the Tiny Freighter's pair at full price");
        assert_eq!(v.featured_price(Commodity::Machinery), None);
    }
}

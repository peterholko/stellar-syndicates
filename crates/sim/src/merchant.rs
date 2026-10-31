//! §merchant: the TRAVELING MERCHANT (`docs/traveling-merchant.md`). A free
//! trader under Authority protection docks at a new corporation's home while
//! its founding charter runs, so the charter's builds need not wait on the
//! Market Hub's 26-minute round trip. §charter-v5: it drops in after the
//! privateer's raid, carrying what the raid took.
//!
//! It is a last resort, not a second Hub (Peter, Oct 13 2026: "the markup
//! should be like 50-100%, the idea the player should only rely on it when
//! needed"). It sells the founding goods well above the Hub and buys a little
//! Ferrite Ore well below it, so a round trip through it always loses money.
//! Prices are fixed for the visit: the Hub mids when it set out, which is what
//! light-delayed news from the Hub would show anyway.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};

/// The founding visit's markup over the Hub mid. Pinned at the low end of the
/// 50–100% band so the whole charter stays affordable. Tunable.
pub const FOUNDING_MARKUP: f64 = 0.50;
/// What the merchant pays for Ferrite Ore, as a share of the Hub mid. Tunable.
pub const ORE_PRICE_SHARE: f64 = 0.50;
/// Ferrite Ore the merchant takes in one visit: enough that a founder who
/// spent the stipends elsewhere can still raise the raid's buy-back. Tunable.
pub const ORE_CAP: u32 = 300;
/// The longest a founding visit lasts; it leaves sooner once the charter is
/// fulfilled. Tunable.
pub const MAX_STAY_S: f64 = 3.0 * 3600.0;
/// §charter-v5 (Peter, Oct 16 2026): the trader leaves while the founder
/// eliminates the pirates. Once the privateer's kill is reported, with its
/// bounty, it stays this long for a last round of trade; the den's fall cuts
/// it short, so it is gone by "Export your ore".
pub const LAST_CALL_S: f64 = 180.0;
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
/// builds.
pub const FOUNDING_STOCK: [(Commodity, u32); 7] = [
    (Commodity::Alloys, 120),
    (Commodity::Machinery, 50),
    // §vendetta-campaign: the third Interceptor's 12 and 15 too.
    (Commodity::Electronics, 127),
    (Commodity::Armaments, 45),
    (Commodity::Polymers, 30),
    (Commodity::Provisions, 40),
    (Commodity::Deuterium, 30),
];

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
    /// Ferrite Ore taken so far this visit.
    pub ore_bought: u32,
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
        mids.insert(Commodity::MetallicOre, mid(Commodity::MetallicOre));
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
            ore_bought: 0,
        }
    }

    /// What the merchant charges per unit, if it sells `c`.
    pub fn sell_price(&self, c: Commodity) -> Option<f64> {
        self.stock.contains_key(&c).then(|| self.mids.get(&c).copied().unwrap_or(0.0) * (1.0 + self.markup))
    }

    /// What the merchant pays per unit, if it buys `c` (Ferrite Ore only).
    pub fn buy_price(&self, c: Commodity) -> Option<f64> {
        (c == Commodity::MetallicOre).then(|| self.mids.get(&c).copied().unwrap_or(0.0) * ORE_PRICE_SHARE)
    }

    /// Ferrite Ore it will still take this visit.
    pub fn ore_room(&self) -> u32 {
        ORE_CAP.saturating_sub(self.ore_bought)
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
            let price = self.sell_price(c).ok_or("The merchant doesn't sell that")?;
            if n > self.stock.get(&c).copied().unwrap_or(0) {
                return Err("The merchant hasn't that much");
            }
            cost += price * n as f64;
            net_units += n as i64;
        }
        let mut revenue = 0.0;
        for (&c, &n) in sells.iter().filter(|(_, n)| **n > 0) {
            let price = self.buy_price(c).ok_or("The merchant doesn't buy that")?;
            if c == Commodity::MetallicOre && n > self.ore_room() {
                return Err("The merchant won't take that much ore");
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
        assert_eq!(v.sell_price(Commodity::Electronics), Some(hub(Commodity::Electronics) * 1.5));
        assert_eq!(v.buy_price(Commodity::MetallicOre), Some(hub(Commodity::MetallicOre) * 0.5));
        assert_eq!(v.sell_price(Commodity::MetallicOre), None, "it doesn't sell ore");
        assert_eq!(v.buy_price(Commodity::Electronics), None, "it buys ore only");
        assert_eq!(v.leave_by, 100.0 + MAX_STAY_S);
        assert_eq!(v.phase, MerchantPhase::Arriving);
    }

    #[test]
    fn quotes_respect_stock_and_the_ore_cap() {
        let mut v = visit();
        let buys = BTreeMap::from([(Commodity::Electronics, 15), (Commodity::Machinery, 13)]);
        let sells = BTreeMap::from([(Commodity::MetallicOre, 40)]);
        let q = v.quote(&buys, &sells).unwrap();
        let hub = crate::market::base_price;
        let cost = (15.0 * hub(Commodity::Electronics) + 13.0 * hub(Commodity::Machinery)) * 1.5;
        assert!((q.cost - cost).abs() < 1e-9);
        assert!((q.revenue - 40.0 * hub(Commodity::MetallicOre) * 0.5).abs() < 1e-9);
        assert_eq!(q.net_units, 15 + 13 - 40);

        let stocked = FOUNDING_STOCK.iter().find(|(c, _)| *c == Commodity::Electronics).unwrap().1;
        assert!(v.quote(&BTreeMap::from([(Commodity::Electronics, stocked + 1)]), &BTreeMap::new()).is_err());
        assert!(v.quote(&BTreeMap::from([(Commodity::Titanium, 1)]), &BTreeMap::new()).is_err());
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::from([(Commodity::Alloys, 1)])).is_err());
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::new()).is_err());
        v.ore_bought = ORE_CAP - 10;
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::from([(Commodity::MetallicOre, 11)])).is_err());
        assert!(v.quote(&BTreeMap::new(), &BTreeMap::from([(Commodity::MetallicOre, 10)])).is_ok());
    }

    #[test]
    fn a_round_trip_through_the_merchant_always_loses() {
        let v = visit();
        for (c, _) in FOUNDING_STOCK {
            assert!(v.sell_price(c).unwrap() > crate::market::base_price(c), "{c:?}");
        }
        assert!(v.buy_price(Commodity::MetallicOre).unwrap() < crate::market::base_price(Commodity::MetallicOre));
    }
}

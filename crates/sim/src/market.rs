//! The GLOBAL MARKET (§9) — the market the Terran Charter Authority
//! operates at the wormhole hub. A single shared market with a reference mid per
//! commodity. **Execution is instant** (settlement is correlation, §3) — a
//! market order walks the integrated curve right now, settling against the
//! trader's MARKET WAREHOUSE (§TCA): buys deposit into it, sells and
//! sell-side limit escrow draw only from it, and no trade ever moves goods across
//! space. Prices *walk* with flow (buys lift, sells depress) along an integrated
//! exponential curve. The curve is path-consistent: one large order costs the
//! same as slices of the same total, and reversing the flow retraces the mid.
//! A small symmetric spread makes every round trip lose rather than mint money.
//!
//! Note: the *information* of the price is lightspeed-bound — that lag lives in
//! the server's view filter, not here. This struct is ground truth.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::PlayerId;
use crate::rng::Rng;

/// Half of the Exchange's total round-trip spread. A buy pays above the walked
/// mid and a sell receives below it; the two-percent round trip is the minimum
/// price of immediacy and the hard anti-churn invariant. Tunable.
const HALF_SPREAD: f64 = 0.01;
/// A limit auction print outside this band rests rather than repricing against a
/// wildly stale or manipulative order. Tunable; protection is symmetric around
/// the current global reference.
const LIMIT_PRICE_COLLAR_FRAC: f64 = 0.25;
/// Prices never fall below this.
const PRICE_FLOOR: f64 = 0.5;

/// A commodity's LIQUIDITY PROFILE (§9 thin books): how deep Sol's book is,
/// how much it absorbs at once, how fast it refills, and how hard the price is
/// pulled back to its reference. One profile per commodity, so the scarce goods
/// trade on a visibly thinner book than the bulk ores.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Liquidity {
    /// Units of flow that move the price by ~e× (the elasticity depth).
    pub depth: f64,
    /// How many units Sol's external market will buy or sell immediately in one
    /// commodity before that side must replenish. Player-to-player limit matches
    /// do not consume this pool. Tied to the curve depth so the last immediately
    /// available unit is already expensive.
    pub external_cap: f64,
    /// Units restored to each depleted external side on every one-second market
    /// update. Counter-flow restores liquidity immediately; this slow refill is
    /// the off-map economy recovering between shocks.
    pub replenish: f64,
    /// How strongly the price reverts toward its base each drift step.
    pub reversion: f64,
}

/// The bulk book: every ordinary good, the common ores included. Tunable.
pub const BULK_LIQUIDITY: Liquidity = Liquidity {
    depth: 1600.0,
    external_cap: 1600.0,
    replenish: 8.0,
    reversion: 0.02,
};

/// The THIN book for the scarce goods (§ore-ladder): one convoy load visibly
/// moves the price, Sol absorbs far less at once and refills slowly, and a
/// shock takes minutes rather than seconds to fade — so the light-delayed
/// ticker finally carries information worth racing for. Tunable.
pub const THIN_LIQUIDITY: Liquidity = Liquidity {
    depth: 600.0,
    external_cap: 600.0,
    replenish: 2.0,
    reversion: 0.005,
};

/// Which book a commodity trades on. Rare-metal ore and the rare elements it
/// refines into are the scarce goods; everything else is bulk. A DENSE ore's
/// book refills per unit of CONTENT, not per unit of ore (§ore-density): its
/// units are thirty times bigger, so Sol's appetite for them regrows thirty
/// times more slowly. A galaxy's whole rare-ore output can then actually
/// saturate Sol, and the players' own book takes over the price.
pub fn liquidity(c: Commodity) -> Liquidity {
    match c {
        Commodity::RareMetalOre => Liquidity {
            replenish: THIN_LIQUIDITY.replenish * crate::production::ore_bulk_ratio(c),
            ..THIN_LIQUIDITY
        },
        Commodity::RareElements => THIN_LIQUIDITY,
        _ => BULK_LIQUIDITY,
    }
}

/// The long-run base price of a commodity (what it reverts toward). Also the
/// canonical "how valuable is this good" scalar behind claim costs, survey value
/// bands and freight fees (§4). Deposit PLACEMENT follows
/// [`crate::galaxy::RAW_DEPOSIT_TABLE`], never this ladder.
///
/// Tunable reference prices: each recipe's combined primary + secondary yields
/// clear its fuel-inclusive basket at base prices (test-enforced). This is not
/// a guaranteed refining profit: live prices, finite demand, freight and staffing
/// still matter. Raw exports retain the same external-market buyers as materials.
///
/// §ore-ladder: the five ores span a TWO-HUNDREDFOLD per-unit ladder (6 / 8 /
/// 24 / 110 / 1,700) that tracks their real scarcity in the generator
/// (`galaxy::RAW_DEPOSIT_TABLE`), not their names — Crystalline and Ferrite are
/// the bulk ores, Cuprite the mid-ring ore, Titanium the outer-half ore,
/// Rare-metal the past-the-pirate-ring prize. The steep part is DENSITY
/// (`production::ore_bulk_ratio`): a unit of Rare-metal ore refines into
/// twenty Rare Elements and fourteen Conductive Metals and leaves the ground
/// at a thirtieth of the unit rate, so income per mining hour stays within a
/// handful of the bulk ores' while a single unit is worth two hundred Ferrite.
/// A raw ore sells for roughly 70% of its refined content (46% for Ferrite:
/// the starter smelter is where its value is), so refining always pays but a
/// raw haul is real income. The dear refined goods are needed in TENTHS of a
/// unit downstream, which is what keeps every end product at its old reference.
pub fn base_price(c: Commodity) -> f64 {
    match c {
        // Raw
        Commodity::Biomass => 5.0,
        Commodity::Silicates => 9.0,
        Commodity::MetallicOre => 8.0,
        Commodity::Volatiles => 9.0,
        Commodity::RareElements => 100.0,
        Commodity::CupriteOre => 24.0,
        Commodity::TitaniumOre => 110.0,
        Commodity::CrystallineOre => 6.0,
        Commodity::RareMetalOre => 1700.0,
        Commodity::ConductiveMetals => 30.0,
        Commodity::Titanium => 60.0,
        // Processed
        Commodity::Provisions => 9.0,
        Commodity::Fuel => 14.0,
        Commodity::Polymers => 16.0,
        Commodity::Alloys => 26.0,
        Commodity::Electronics => 34.0,
        // Advanced
        // (Machinery raised from the handoff's suggested 48: its input basket —
        // 1.2 Alloys + 0.6 Electronics + 0.4 Fuel = 57.2 — didn't clear. 62
        // clears with margin. The component chains now consume Machinery too.)
        Commodity::Machinery => 62.0,
        Commodity::Armaments => 56.0,
        // §industry-chains: ~20% value added over each unbonused input basket.
        Commodity::Composites => 50.0,
        Commodity::HullSections => 128.0,
        Commodity::PrecisionComponents => 92.0,
        Commodity::DriveAssemblies => 215.0,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    /// Current standing price per commodity.
    prices: BTreeMap<Commodity, f64>,
    /// Long-run base price each commodity reverts toward.
    base: BTreeMap<Commodity, f64>,
    /// Finite off-map units offered to corporations. Missing entries deserialize
    /// as a full pool so pre-liquidity snapshots migrate without a special pass.
    #[serde(default = "default_external_liquidity")]
    external_supply: BTreeMap<Commodity, f64>,
    /// Finite off-map demand available to absorb corporate sales.
    #[serde(default = "default_external_liquidity")]
    external_demand: BTreeMap<Commodity, f64>,
}

fn default_external_liquidity() -> BTreeMap<Commodity, f64> {
    Commodity::ALL
        .into_iter()
        .map(|c| (c, liquidity(c).external_cap))
        .collect()
}

/// A quantity-aware executable quote. `unit_price` is the AVERAGE across the
/// whole walked curve, never the stale pre-trade standing price.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketQuote {
    pub unit_price: f64,
    pub total: f64,
    pub next_price: f64,
}

impl Default for Market {
    fn default() -> Self {
        Market::new()
    }
}

impl Market {
    /// Saved markets keep their existing prices and depleted liquidity. Seed
    /// ONLY missing catalog entries when new goods land; never reprice trades
    /// or grant their cargo to warehouses/colonies during migration. The one
    /// exception is the REFERENCE LADDER itself (§ore-ladder): when a saved
    /// base differs from the catalog, the live price rides along at the same
    /// ratio so a stale snapshot never opens an arbitrage window against the
    /// new ladder, and each external pool is clamped to its good's book.
    /// Returns the commodities whose reference moved (empty on a current save).
    pub fn ensure_catalog(&mut self) -> Vec<Commodity> {
        let mut rebased = Vec::new();
        for c in Commodity::ALL {
            let catalog = base_price(c);
            let cap = liquidity(c).external_cap;
            match self.base.get(&c).copied() {
                Some(old) if (old - catalog).abs() > 1e-9 => {
                    let ratio = if old > 0.0 { catalog / old } else { 1.0 };
                    self.base.insert(c, catalog);
                    let live = self.prices.entry(c).or_insert(old);
                    *live = (*live * ratio).max(PRICE_FLOOR);
                    rebased.push(c);
                }
                Some(_) => {}
                None => {
                    self.base.insert(c, catalog);
                }
            }
            self.prices.entry(c).or_insert(catalog);
            let supply = self.external_supply.entry(c).or_insert(cap);
            *supply = supply.min(cap);
            let demand = self.external_demand.entry(c).or_insert(cap);
            *demand = demand.min(cap);
        }
        rebased
    }

    pub fn new() -> Self {
        let base: BTreeMap<Commodity, f64> = Commodity::ALL
            .into_iter()
            .map(|c| (c, base_price(c)))
            .collect();
        Market {
            prices: base.clone(),
            base,
            external_supply: default_external_liquidity(),
            external_demand: default_external_liquidity(),
        }
    }

    /// The current standing price of a commodity.
    pub fn price(&self, c: Commodity) -> f64 {
        *self.prices.get(&c).unwrap_or(&0.0)
    }

    /// All standing prices (for snapshots / the price ticker).
    pub fn prices(&self) -> &BTreeMap<Commodity, f64> {
        &self.prices
    }

    /// Units the external market can still sell immediately. This is market
    /// state, not a guarantee at a delayed command center: the server serves it
    /// through the same lagged ticker as price.
    pub fn available_to_buy(&self, c: Commodity) -> u32 {
        self.external_supply
            .get(&c)
            .copied()
            .unwrap_or(liquidity(c).external_cap)
            .floor()
            .max(0.0) as u32
    }

    /// Units of immediate external demand still available for corporate sales.
    pub fn available_to_sell(&self, c: Commodity) -> u32 {
        self.external_demand
            .get(&c)
            .copied()
            .unwrap_or(liquidity(c).external_cap)
            .floor()
            .max(0.0) as u32
    }

    /// Quote a buy without mutating the market. Exponential integration makes
    /// cost independent of order slicing: `q` at once costs exactly the same as
    /// any sequence whose quantities sum to `q` (up to floating-point residue).
    pub fn quote_buy(&self, c: Commodity, units: u32) -> MarketQuote {
        if units == 0 {
            return MarketQuote {
                unit_price: self.price(c),
                total: 0.0,
                next_price: self.price(c),
            };
        }
        let depth = liquidity(c).depth;
        let p = self.price(c);
        let x = units as f64 / depth;
        let next_price = p * x.exp();
        let unspread_total = p * depth * x.exp_m1();
        let total = unspread_total * (1.0 + HALF_SPREAD);
        MarketQuote {
            unit_price: total / units as f64,
            total,
            next_price,
        }
    }

    /// Quote a sell without mutation. Above the standing-price floor this is the
    /// exact inverse integral of [`Self::quote_buy`]. If an exceptional sale is
    /// large enough to reach the floor, its remainder clears at the floor rather
    /// than integrating through impossible negative/near-zero prices.
    pub fn quote_sell(&self, c: Commodity, units: u32) -> MarketQuote {
        if units == 0 {
            return MarketQuote {
                unit_price: self.price(c),
                total: 0.0,
                next_price: self.price(c),
            };
        }
        let depth = liquidity(c).depth;
        let p = self.price(c).max(PRICE_FLOOR);
        let units_f = units as f64;
        let to_floor = (depth * (p / PRICE_FLOOR).ln()).max(0.0);
        let (unspread_total, next_price) = if units_f <= to_floor + 1e-9 {
            let x = units_f / depth;
            (p * depth * (-x).exp_m1().abs(), p * (-x).exp())
        } else {
            let curved = depth * (p - PRICE_FLOOR);
            let flat = (units_f - to_floor) * PRICE_FLOOR;
            (curved + flat, PRICE_FLOOR)
        };
        let total = unspread_total * (1.0 - HALF_SPREAD);
        MarketQuote {
            unit_price: total / units_f,
            total,
            next_price: next_price.max(PRICE_FLOOR),
        }
    }

    /// Execute a quantity-aware buy and return its average-price quote.
    pub fn execute_buy(&mut self, c: Commodity, units: u32) -> f64 {
        debug_assert!(
            units <= self.available_to_buy(c),
            "external supply must be checked before execution"
        );
        let quote = self.quote_buy(c, units);
        self.prices.insert(c, quote.next_price);
        let cap = liquidity(c).external_cap;
        let supply = self.external_supply.entry(c).or_insert(cap);
        *supply = (*supply - units as f64).max(0.0);
        let demand = self.external_demand.entry(c).or_insert(cap);
        *demand = (*demand + units as f64).min(cap);
        quote.unit_price
    }

    /// Execute a quantity-aware sell and return its average-price quote.
    pub fn execute_sell(&mut self, c: Commodity, units: u32) -> f64 {
        debug_assert!(
            units <= self.available_to_sell(c),
            "external demand must be checked before execution"
        );
        let quote = self.quote_sell(c, units);
        self.prices.insert(c, quote.next_price);
        let cap = liquidity(c).external_cap;
        let demand = self.external_demand.entry(c).or_insert(cap);
        *demand = (*demand - units as f64).max(0.0);
        let supply = self.external_supply.entry(c).or_insert(cap);
        *supply = (*supply + units as f64).min(cap);
        quote.unit_price
    }

    /// Slow seeded drift: mean-revert toward base with a little noise. Called on
    /// a slow cadence so the market is alive and the price *lag* is visible.
    /// Each good reverts and refills at its own book's pace.
    pub fn drift(&mut self, rng: &mut Rng) {
        for (c, base) in &self.base {
            let book = liquidity(*c);
            let p = self.prices[c];
            let noise = p * rng.range(-0.015, 0.015);
            let np = (p + (base - p) * book.reversion + noise).max(PRICE_FLOOR);
            self.prices.insert(*c, np);
            let supply = self.external_supply.entry(*c).or_insert(book.external_cap);
            *supply = (*supply + book.replenish).min(book.external_cap);
            let demand = self.external_demand.entry(*c).or_insert(book.external_cap);
            *demand = (*demand + book.replenish).min(book.external_cap);
        }
    }

    /// Let matched PLAYER flow move the reference without allowing one auction
    /// print to re-anchor the whole external market. Equal player buy/sell volume
    /// has zero net flow, so a call-auction transfer does not move the Sol mid.
    pub fn apply_net_flow(&mut self, c: Commodity, bought: u32, sold: u32) {
        let signed = bought as i64 - sold as i64;
        if signed == 0 {
            return;
        }
        let p = self.price(c);
        let next = p * (signed as f64 / liquidity(c).depth).exp();
        self.prices.insert(c, next.max(PRICE_FLOOR));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

/// A resting limit order on the book. Buys are willing to pay UP TO
/// `limit_price`; sells want AT LEAST `limit_price`. They clear in a periodic
/// uniform-price call auction — the anti-sniping mechanism (§9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitOrder {
    pub id: u64,
    pub player: PlayerId,
    pub side: Side,
    pub commodity: Commodity,
    /// Units still resting (decremented as the order fills).
    pub units: u32,
    pub limit_price: f64,
}

/// The result of clearing one commodity's book at a single uniform price.
pub struct Clearing {
    pub price: f64,
    /// Per order: (order_id, units filled this clearing).
    pub fills: Vec<(u64, u32)>,
}

/// Match eligible orders at one candidate price, excluding trades where buyer
/// and seller are the same corporation. The book is complete except for those
/// self edges, so deterministic price/id priority gives a stable maximum-volume
/// result for the small per-commodity book.
fn match_at_price(
    buys: &[&LimitOrder],
    sells: &[&LimitOrder],
    price: f64,
) -> (u32, Vec<(u64, u32)>) {
    let mut sell_left: Vec<u32> = sells.iter().map(|o| o.units).collect();
    let mut by_order: BTreeMap<u64, u32> = BTreeMap::new();
    let mut volume = 0u32;

    for buy in buys
        .iter()
        .copied()
        .filter(|o| o.limit_price >= price - 1e-9)
    {
        let mut buy_left = buy.units;
        for (si, sell) in sells.iter().copied().enumerate() {
            if buy_left == 0 {
                break;
            }
            if sell.limit_price > price + 1e-9 || sell.player == buy.player {
                continue;
            }
            let fill = buy_left.min(sell_left[si]);
            if fill == 0 {
                continue;
            }
            buy_left -= fill;
            sell_left[si] -= fill;
            volume += fill;
            *by_order.entry(buy.id).or_insert(0) += fill;
            *by_order.entry(sell.id).or_insert(0) += fill;
        }
    }
    (volume, by_order.into_iter().collect())
}

/// Compute the uniform-price call auction for one commodity's orders (§9). All
/// trades clear at a single price, arrival order within the batch is irrelevant,
/// corporations cannot trade with themselves, and prices outside the reference
/// collar remain resting rather than becoming a manipulative global print.
pub fn clear_call_auction(orders: &[LimitOrder], reference: f64) -> Option<Clearing> {
    let mut buys: Vec<&LimitOrder> = orders.iter().filter(|o| o.side == Side::Buy).collect();
    let mut sells: Vec<&LimitOrder> = orders.iter().filter(|o| o.side == Side::Sell).collect();
    if buys.is_empty() || sells.is_empty() {
        return None;
    }
    // Best price first; deterministic tie-break by id.
    buys.sort_by(|a, b| {
        b.limit_price
            .partial_cmp(&a.limit_price)
            .unwrap()
            .then(a.id.cmp(&b.id))
    });
    sells.sort_by(|a, b| {
        a.limit_price
            .partial_cmp(&b.limit_price)
            .unwrap()
            .then(a.id.cmp(&b.id))
    });

    // Candidate clearing prices: every limit price. Pick the one maximising the
    // matched volume (ties → lowest imbalance, then lowest price).
    let lo = reference * (1.0 - LIMIT_PRICE_COLLAR_FRAC);
    let hi = reference * (1.0 + LIMIT_PRICE_COLLAR_FRAC);
    let mut candidates: Vec<f64> = orders
        .iter()
        .map(|o| o.limit_price)
        .filter(|p| *p >= lo - 1e-9 && *p <= hi + 1e-9)
        .collect();
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    candidates.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let demand = |p: f64| {
        buys.iter()
            .filter(|o| o.limit_price >= p - 1e-9)
            .map(|o| o.units)
            .sum::<u32>()
    };
    let supply = |p: f64| {
        sells
            .iter()
            .filter(|o| o.limit_price <= p + 1e-9)
            .map(|o| o.units)
            .sum::<u32>()
    };

    let mut best: Option<(f64, u32)> = None;
    for &p in &candidates {
        let (vol, _) = match_at_price(&buys, &sells, p);
        if vol == 0 {
            continue;
        }
        let imbalance = demand(p).abs_diff(supply(p));
        match best {
            Some((_, bv)) if vol < bv => {}
            Some((bp, bv)) if vol == bv => {
                // prefer lower imbalance, then lower price
                let bi = demand(bp).abs_diff(supply(bp));
                if imbalance < bi || (imbalance == bi && p < bp) {
                    best = Some((p, vol));
                }
            }
            _ => best = Some((p, vol)),
        }
    }
    let (price, _) = best?;
    let (_, fills) = match_at_price(&buys, &sells, price);
    Some(Clearing { price, fills })
}

#[cfg(test)]
mod economy_price_tests {
    use super::*;

    #[test]
    fn catalog_migration_adds_only_missing_goods_and_preserves_the_live_market() {
        let mut market = Market::new();
        market.execute_buy(Commodity::Alloys, 100);
        let old_price = market.price(Commodity::Alloys);
        let old_supply = market.available_to_buy(Commodity::Alloys);
        let goods = [Commodity::Composites, Commodity::HullSections,
            Commodity::PrecisionComponents, Commodity::DriveAssemblies];
        for good in goods {
            market.base.remove(&good);
            market.prices.remove(&good);
            market.external_supply.remove(&good);
            market.external_demand.remove(&good);
        }
        let mut loaded: Market = serde_json::from_str(&serde_json::to_string(&market).unwrap()).unwrap();
        loaded.ensure_catalog();
        assert_eq!(loaded.price(Commodity::Alloys), old_price);
        assert_eq!(loaded.available_to_buy(Commodity::Alloys), old_supply);
        for good in goods {
            assert_eq!(loaded.price(good), base_price(good));
            assert!(loaded.available_to_buy(good) > 0);
        }
        loaded.execute_buy(Commodity::DriveAssemblies, 10);
        let before = serde_json::to_string(&loaded).unwrap();
        loaded.ensure_catalog();
        assert_eq!(serde_json::to_string(&loaded).unwrap(), before, "migration is idempotent");
    }

    /// Combined recipe yields clear the base input basket; no claim about the
    /// current live market or profit after staffing and hauling overhead.
    /// Reads the LIVE converter table (`production::CONVERTERS`) — one source of
    /// truth, so recipes and prices can never drift apart.
    #[test]
    fn processed_prices_clear_their_input_baskets() {
        let mut covered = std::collections::BTreeSet::new();
        for conv in crate::production::CONVERTERS.iter().chain(crate::production::ORE_REFINING.iter()) {
            let input_cost: f64 = conv
                .inputs
                .iter()
                .map(|(c, per_unit)| base_price(*c) * per_unit)
                .sum();
            assert!(
                conv.outputs().map(|(c, units)| base_price(c) * units).sum::<f64>() > input_cost,
                "{:?} base {} must clear its input basket {input_cost:.2}",
                conv.output,
                base_price(conv.output)
            );
            covered.extend(conv.outputs().map(|(c, _)| c));
        }
        // Every non-raw commodity must be REACHABLE by some converter.
        for c in Commodity::ALL {
            if !Commodity::RAW.contains(&c) {
                assert!(covered.contains(&c), "{c:?} has no converter producing it");
            }
        }
    }

    #[test]
    fn immediate_round_trip_loses_the_spread_and_restores_the_mid() {
        let mut market = Market::new();
        let before = market.price(Commodity::Alloys);
        let bought_at = market.execute_buy(Commodity::Alloys, 400);
        let buy_cost = bought_at * 400.0;
        let sold_at = market.execute_sell(Commodity::Alloys, 400);
        let proceeds = sold_at * 400.0;

        assert!(
            proceeds < buy_cost,
            "an immediate round trip must lose money"
        );
        assert!(
            (market.price(Commodity::Alloys) - before).abs() < 1e-9,
            "reversing flow retraces the mid"
        );
        let expected_ratio = (1.0 - HALF_SPREAD) / (1.0 + HALF_SPREAD);
        assert!((proceeds / buy_cost - expected_ratio).abs() < 1e-12);
    }

    #[test]
    fn slicing_an_order_does_not_evade_price_impact() {
        let mut whole = Market::new();
        let whole_cost = whole.execute_buy(Commodity::Machinery, 400) * 400.0;

        let mut sliced = Market::new();
        let sliced_cost: f64 = (0..4)
            .map(|_| sliced.execute_buy(Commodity::Machinery, 100) * 100.0)
            .sum();

        assert!((whole_cost - sliced_cost).abs() < 1e-8);
        assert!(
            (whole.price(Commodity::Machinery) - sliced.price(Commodity::Machinery)).abs() < 1e-10
        );
    }

    #[test]
    fn external_liquidity_is_finite_and_counterflow_restores_it() {
        let mut market = Market::new();
        let cap = market.available_to_buy(Commodity::Fuel);
        assert_eq!(cap, BULK_LIQUIDITY.external_cap as u32);

        market.execute_buy(Commodity::Fuel, 400);
        assert_eq!(market.available_to_buy(Commodity::Fuel), cap - 400);
        assert_eq!(market.available_to_sell(Commodity::Fuel), cap);

        market.execute_sell(Commodity::Fuel, 150);
        assert_eq!(market.available_to_buy(Commodity::Fuel), cap - 250);
        assert_eq!(market.available_to_sell(Commodity::Fuel), cap - 150);
    }

    /// §ore-ladder: the scarce goods trade on a thinner book — the same convoy
    /// load moves their price further, Sol absorbs less of it at once, the
    /// external side refills more slowly, and the dent fades more slowly.
    #[test]
    fn rare_goods_trade_on_a_thinner_book_than_bulk_ores() {
        let mut market = Market::new();
        assert_eq!(market.available_to_sell(Commodity::RareMetalOre), THIN_LIQUIDITY.external_cap as u32);
        assert_eq!(market.available_to_sell(Commodity::MetallicOre), BULK_LIQUIDITY.external_cap as u32);
        let bulk0 = market.price(Commodity::MetallicOre);
        let rare0 = market.price(Commodity::RareMetalOre);
        market.execute_sell(Commodity::MetallicOre, 400);
        market.execute_sell(Commodity::RareMetalOre, 400);
        let bulk_drop = 1.0 - market.price(Commodity::MetallicOre) / bulk0;
        let rare_drop = 1.0 - market.price(Commodity::RareMetalOre) / rare0;
        assert!(
            rare_drop > bulk_drop * 2.0,
            "one convoy of rare ore must dent its price far harder: bulk {bulk_drop:.3} rare {rare_drop:.3}"
        );
        assert!((rare_drop - (1.0 - (-400.0f64 / THIN_LIQUIDITY.depth).exp())).abs() < 1e-9);

        let rare_before = market.available_to_sell(Commodity::RareMetalOre);
        let bulk_before = market.available_to_sell(Commodity::MetallicOre);
        let mut rng = Rng::new(5);
        market.drift(&mut rng);
        assert_eq!(
            market.available_to_sell(Commodity::MetallicOre) - bulk_before,
            BULK_LIQUIDITY.replenish as u32
        );
        // A minute later the bulk dent has mostly healed; the rare one lingers,
        // and Sol's appetite for the dense ore has regrown by only a few units.
        for _ in 0..59 {
            market.drift(&mut rng);
        }
        let rare_refill = liquidity(Commodity::RareMetalOre).replenish;
        assert!(rare_refill < THIN_LIQUIDITY.replenish, "dense ore refills per unit of content");
        let regrown = market.available_to_sell(Commodity::RareMetalOre) - rare_before;
        assert!(regrown.abs_diff((rare_refill * 60.0).round() as u32) <= 1, "regrown {regrown}");
        let bulk_left = (1.0 - market.price(Commodity::MetallicOre) / bulk0) / bulk_drop;
        let rare_left = (1.0 - market.price(Commodity::RareMetalOre) / rare0) / rare_drop;
        assert!(
            rare_left > bulk_left + 0.2,
            "the rare shock must outlast the bulk one: bulk {bulk_left:.2} rare {rare_left:.2} of the dent remain"
        );
    }

    /// §ore-ladder migration: a snapshot saved under an older reference ladder
    /// is rebased on load — the live price rides along at the same ratio, the
    /// external pools are clamped to the (possibly thinner) new book, and the
    /// pass is idempotent.
    #[test]
    fn catalog_migration_rebases_a_stale_reference_ladder() {
        let mut market = Market::new();
        let catalog = base_price(Commodity::RareMetalOre);
        market.base.insert(Commodity::RareMetalOre, 19.0);
        market.prices.insert(Commodity::RareMetalOre, 20.9);
        market.external_supply.insert(Commodity::RareMetalOre, 1600.0);
        let rebased = market.ensure_catalog();
        assert_eq!(rebased, vec![Commodity::RareMetalOre]);
        assert!((market.price(Commodity::RareMetalOre) - 20.9 * catalog / 19.0).abs() < 1e-9);
        assert_eq!(
            market.available_to_buy(Commodity::RareMetalOre),
            THIN_LIQUIDITY.external_cap as u32
        );
        let before = serde_json::to_string(&market).unwrap();
        assert!(market.ensure_catalog().is_empty());
        assert_eq!(serde_json::to_string(&market).unwrap(), before, "rebase is idempotent");
    }

    fn limit(id: u64, player: u64, side: Side, units: u32, price: f64) -> LimitOrder {
        LimitOrder {
            id,
            player: PlayerId(player),
            side,
            commodity: Commodity::Alloys,
            units,
            limit_price: price,
        }
    }

    #[test]
    fn call_auction_never_self_trades() {
        let only_self = [
            limit(1, 7, Side::Buy, 10, 27.0),
            limit(2, 7, Side::Sell, 10, 25.0),
        ];
        assert!(clear_call_auction(&only_self, 26.0).is_none());

        let with_counterparty = [
            limit(1, 7, Side::Buy, 10, 27.0),
            limit(2, 7, Side::Sell, 10, 25.0),
            limit(3, 8, Side::Sell, 6, 25.0),
        ];
        let clearing =
            clear_call_auction(&with_counterparty, 26.0).expect("the rival sale crosses");
        assert_eq!(clearing.fills, vec![(1, 6), (3, 6)]);
    }

    #[test]
    fn extreme_limit_prints_stay_outside_the_reference_collar() {
        let orders = [
            limit(1, 7, Side::Buy, 1, 10_000.0),
            limit(2, 8, Side::Sell, 1, 10_000.0),
        ];
        assert!(clear_call_auction(&orders, 26.0).is_none());
    }
}

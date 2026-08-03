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

/// Units of flow that move the price by ~100% (the elasticity depth / liquidity).
const DEPTH: f64 = 1600.0;
/// Half of the Exchange's total round-trip spread. A buy pays above the walked
/// mid and a sell receives below it; the two-percent round trip is the minimum
/// price of immediacy and the hard anti-churn invariant. Tunable.
const HALF_SPREAD: f64 = 0.01;
/// A limit auction print outside this band rests rather than repricing against a
/// wildly stale or manipulative order. Tunable; protection is symmetric around
/// the current global reference.
const LIMIT_PRICE_COLLAR_FRAC: f64 = 0.25;
/// How many units Sol's external market will buy or sell immediately in one
/// commodity before that side must replenish. Player-to-player limit matches do
/// not consume this pool. Tunable; deliberately tied to the curve depth so the
/// last immediately available unit is already expensive.
const EXTERNAL_LIQUIDITY_CAP: f64 = DEPTH;
/// Units restored to each depleted external side on every one-second market
/// update. Counter-flow restores liquidity immediately; this slow refill is the
/// off-map economy recovering between shocks. Tunable.
const EXTERNAL_LIQUIDITY_REPLENISH: f64 = 8.0;
/// Prices never fall below this.
const PRICE_FLOOR: f64 = 0.5;
/// How strongly prices revert toward their base each drift step.
const REVERSION: f64 = 0.02;

/// The long-run base price of a commodity (what it reverts toward). Also the
/// canonical "how valuable is this good" ranking used by galaxy generation to
/// place richer/more-valuable deposits toward the frontier (§4).
///
/// §economy: the 12-commodity ladder (all Tunable), chosen so every PROCESSED
/// good clears its input basket at base prices and every ADVANCED good clears
/// its own (test-enforced: `processed_prices_clear_their_input_baskets`) —
/// industry is worth doing without making raw-selling worthless. Note Volatiles
/// dropped 18 → 9: it is a common raw now, not a frontier prize; RARE ELEMENTS
/// takes that role at the rim.
pub fn base_price(c: Commodity) -> f64 {
    match c {
        // Raw
        Commodity::Biomass => 5.0,
        Commodity::Silicates => 6.0,
        Commodity::MetallicOre => 8.0,
        Commodity::Volatiles => 9.0,
        Commodity::RareElements => 22.0,
        // Processed
        Commodity::Provisions => 9.0,
        Commodity::Fuel => 14.0,
        Commodity::Polymers => 16.0,
        Commodity::Alloys => 26.0,
        Commodity::Electronics => 34.0,
        // Advanced
        // (Machinery raised from the handoff's suggested 48: its input basket —
        // 1.2 Alloys + 0.6 Electronics + 0.4 Fuel = 57.2 — didn't clear. 62
        // clears with margin; nothing consumes Machinery in a chain, no cascade.)
        Commodity::Machinery => 62.0,
        Commodity::Armaments => 56.0,
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
    Commodity::ALL.into_iter().map(|c| (c, EXTERNAL_LIQUIDITY_CAP)).collect()
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
    pub fn new() -> Self {
        let base: BTreeMap<Commodity, f64> =
            Commodity::ALL.into_iter().map(|c| (c, base_price(c))).collect();
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
        self.external_supply.get(&c).copied().unwrap_or(EXTERNAL_LIQUIDITY_CAP).floor().max(0.0) as u32
    }

    /// Units of immediate external demand still available for corporate sales.
    pub fn available_to_sell(&self, c: Commodity) -> u32 {
        self.external_demand.get(&c).copied().unwrap_or(EXTERNAL_LIQUIDITY_CAP).floor().max(0.0) as u32
    }

    /// Quote a buy without mutating the market. Exponential integration makes
    /// cost independent of order slicing: `q` at once costs exactly the same as
    /// any sequence whose quantities sum to `q` (up to floating-point residue).
    pub fn quote_buy(&self, c: Commodity, units: u32) -> MarketQuote {
        if units == 0 {
            return MarketQuote { unit_price: self.price(c), total: 0.0, next_price: self.price(c) };
        }
        let p = self.price(c);
        let x = units as f64 / DEPTH;
        let next_price = p * x.exp();
        let unspread_total = p * DEPTH * x.exp_m1();
        let total = unspread_total * (1.0 + HALF_SPREAD);
        MarketQuote { unit_price: total / units as f64, total, next_price }
    }

    /// Quote a sell without mutation. Above the standing-price floor this is the
    /// exact inverse integral of [`Self::quote_buy`]. If an exceptional sale is
    /// large enough to reach the floor, its remainder clears at the floor rather
    /// than integrating through impossible negative/near-zero prices.
    pub fn quote_sell(&self, c: Commodity, units: u32) -> MarketQuote {
        if units == 0 {
            return MarketQuote { unit_price: self.price(c), total: 0.0, next_price: self.price(c) };
        }
        let p = self.price(c).max(PRICE_FLOOR);
        let units_f = units as f64;
        let to_floor = (DEPTH * (p / PRICE_FLOOR).ln()).max(0.0);
        let (unspread_total, next_price) = if units_f <= to_floor + 1e-9 {
            let x = units_f / DEPTH;
            (p * DEPTH * (-x).exp_m1().abs(), p * (-x).exp())
        } else {
            let curved = DEPTH * (p - PRICE_FLOOR);
            let flat = (units_f - to_floor) * PRICE_FLOOR;
            (curved + flat, PRICE_FLOOR)
        };
        let total = unspread_total * (1.0 - HALF_SPREAD);
        MarketQuote { unit_price: total / units_f, total, next_price: next_price.max(PRICE_FLOOR) }
    }

    /// Execute a quantity-aware buy and return its average-price quote.
    pub fn execute_buy(&mut self, c: Commodity, units: u32) -> f64 {
        debug_assert!(units <= self.available_to_buy(c), "external supply must be checked before execution");
        let quote = self.quote_buy(c, units);
        self.prices.insert(c, quote.next_price);
        let supply = self.external_supply.entry(c).or_insert(EXTERNAL_LIQUIDITY_CAP);
        *supply = (*supply - units as f64).max(0.0);
        let demand = self.external_demand.entry(c).or_insert(EXTERNAL_LIQUIDITY_CAP);
        *demand = (*demand + units as f64).min(EXTERNAL_LIQUIDITY_CAP);
        quote.unit_price
    }

    /// Execute a quantity-aware sell and return its average-price quote.
    pub fn execute_sell(&mut self, c: Commodity, units: u32) -> f64 {
        debug_assert!(units <= self.available_to_sell(c), "external demand must be checked before execution");
        let quote = self.quote_sell(c, units);
        self.prices.insert(c, quote.next_price);
        let demand = self.external_demand.entry(c).or_insert(EXTERNAL_LIQUIDITY_CAP);
        *demand = (*demand - units as f64).max(0.0);
        let supply = self.external_supply.entry(c).or_insert(EXTERNAL_LIQUIDITY_CAP);
        *supply = (*supply + units as f64).min(EXTERNAL_LIQUIDITY_CAP);
        quote.unit_price
    }

    /// Slow seeded drift: mean-revert toward base with a little noise. Called on
    /// a slow cadence so the market is alive and the price *lag* is visible.
    pub fn drift(&mut self, rng: &mut Rng) {
        for (c, base) in &self.base {
            let p = self.prices[c];
            let noise = p * rng.range(-0.015, 0.015);
            let np = (p + (base - p) * REVERSION + noise).max(PRICE_FLOOR);
            self.prices.insert(*c, np);
            let supply = self.external_supply.entry(*c).or_insert(EXTERNAL_LIQUIDITY_CAP);
            *supply = (*supply + EXTERNAL_LIQUIDITY_REPLENISH).min(EXTERNAL_LIQUIDITY_CAP);
            let demand = self.external_demand.entry(*c).or_insert(EXTERNAL_LIQUIDITY_CAP);
            *demand = (*demand + EXTERNAL_LIQUIDITY_REPLENISH).min(EXTERNAL_LIQUIDITY_CAP);
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
        let next = p * (signed as f64 / DEPTH).exp();
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

    for buy in buys.iter().copied().filter(|o| o.limit_price >= price - 1e-9) {
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
    buys.sort_by(|a, b| b.limit_price.partial_cmp(&a.limit_price).unwrap().then(a.id.cmp(&b.id)));
    sells.sort_by(|a, b| a.limit_price.partial_cmp(&b.limit_price).unwrap().then(a.id.cmp(&b.id)));

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

    let demand = |p: f64| buys.iter().filter(|o| o.limit_price >= p - 1e-9).map(|o| o.units).sum::<u32>();
    let supply = |p: f64| sells.iter().filter(|o| o.limit_price <= p + 1e-9).map(|o| o.units).sum::<u32>();

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

    /// §economy BALANCE INVARIANT: every PROCESSED/ADVANCED good's base price
    /// clears its per-unit input basket at base prices — industry is worth doing
    /// (without making raw-selling worthless, which the raw ladder itself keeps).
    /// Reads the LIVE converter table (`production::CONVERTERS`) — one source of
    /// truth, so recipes and prices can never drift apart.
    #[test]
    fn processed_prices_clear_their_input_baskets() {
        let mut covered = std::collections::BTreeSet::new();
        for conv in &crate::production::CONVERTERS {
            let input_cost: f64 = conv.inputs.iter().map(|(c, per_unit)| base_price(*c) * per_unit).sum();
            assert!(
                base_price(conv.output) > input_cost,
                "{:?} base {} must clear its input basket {input_cost:.2}",
                conv.output,
                base_price(conv.output)
            );
            covered.insert(conv.output);
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

        assert!(proceeds < buy_cost, "an immediate round trip must lose money");
        assert!((market.price(Commodity::Alloys) - before).abs() < 1e-9, "reversing flow retraces the mid");
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
        assert!((whole.price(Commodity::Machinery) - sliced.price(Commodity::Machinery)).abs() < 1e-10);
    }

    #[test]
    fn external_liquidity_is_finite_and_counterflow_restores_it() {
        let mut market = Market::new();
        let cap = market.available_to_buy(Commodity::Fuel);
        assert_eq!(cap, EXTERNAL_LIQUIDITY_CAP as u32);

        market.execute_buy(Commodity::Fuel, 400);
        assert_eq!(market.available_to_buy(Commodity::Fuel), cap - 400);
        assert_eq!(market.available_to_sell(Commodity::Fuel), cap);

        market.execute_sell(Commodity::Fuel, 150);
        assert_eq!(market.available_to_buy(Commodity::Fuel), cap - 250);
        assert_eq!(market.available_to_sell(Commodity::Fuel), cap - 150);
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
        let clearing = clear_call_auction(&with_counterparty, 26.0).expect("the rival sale crosses");
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

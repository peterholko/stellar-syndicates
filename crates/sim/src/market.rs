//! The hub Exchange (§9). A single shared market with a standing price per
//! commodity. **Execution is instant** (settlement is correlation, §3) — a
//! market order fills against the standing price right now. Prices *walk* with
//! flow (buys lift, sells depress) along a simple elasticity curve, and drift on
//! a slow seeded random walk so there is always something to trade against.
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

    /// Execute a buy of `units`: fill at the current price, then walk the price
    /// up. Returns the per-unit fill price.
    pub fn execute_buy(&mut self, c: Commodity, units: u32) -> f64 {
        let p = self.price(c);
        let np = p * (1.0 + units as f64 / DEPTH);
        self.prices.insert(c, np);
        p
    }

    /// Execute a sell of `units`: fill at the current price, then walk the price
    /// down (floored). Returns the per-unit fill price.
    pub fn execute_sell(&mut self, c: Commodity, units: u32) -> f64 {
        let p = self.price(c);
        let np = (p * (1.0 - units as f64 / DEPTH)).max(PRICE_FLOOR);
        self.prices.insert(c, np);
        p
    }

    /// Slow seeded drift: mean-revert toward base with a little noise. Called on
    /// a slow cadence so the market is alive and the price *lag* is visible.
    pub fn drift(&mut self, rng: &mut Rng) {
        for (c, base) in &self.base {
            let p = self.prices[c];
            let noise = p * rng.range(-0.015, 0.015);
            let np = (p + (base - p) * REVERSION + noise).max(PRICE_FLOOR);
            self.prices.insert(*c, np);
        }
    }

    /// Re-anchor the standing price after a batch clearing (§9).
    pub fn set_price(&mut self, c: Commodity, p: f64) {
        self.prices.insert(c, p.max(PRICE_FLOOR));
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Compute the uniform-price call auction for one commodity's orders (§9). All
/// trades clear at a single price, so arrival order within the batch is
/// irrelevant. Returns `None` if nothing crosses. Deterministic: sorts by price
/// then by order id.
pub fn clear_call_auction(orders: &[LimitOrder]) -> Option<Clearing> {
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
    let mut candidates: Vec<f64> = orders.iter().map(|o| o.limit_price).collect();
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    candidates.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    let demand = |p: f64| buys.iter().filter(|o| o.limit_price >= p - 1e-9).map(|o| o.units).sum::<u32>();
    let supply = |p: f64| sells.iter().filter(|o| o.limit_price <= p + 1e-9).map(|o| o.units).sum::<u32>();

    let mut best: Option<(f64, u32)> = None;
    for &p in &candidates {
        let vol = demand(p).min(supply(p));
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
    let (price, mut volume) = best?;

    // Fill best-priced orders first, all at the uniform clearing price.
    let mut fills = Vec::new();
    for o in buys.iter().filter(|o| o.limit_price >= price - 1e-9) {
        if volume == 0 {
            break;
        }
        let f = o.units.min(volume);
        if f > 0 {
            fills.push((o.id, f));
            volume -= f;
        }
    }
    let mut volume_s = demand(price).min(supply(price));
    for o in sells.iter().filter(|o| o.limit_price <= price + 1e-9) {
        if volume_s == 0 {
            break;
        }
        let f = o.units.min(volume_s);
        if f > 0 {
            fills.push((o.id, f));
            volume_s -= f;
        }
    }
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
}

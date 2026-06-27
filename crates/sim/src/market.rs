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
use crate::rng::Rng;

/// Units of flow that move the price by ~100% (the elasticity depth / liquidity).
const DEPTH: f64 = 1600.0;
/// Prices never fall below this.
const PRICE_FLOOR: f64 = 0.5;
/// How strongly prices revert toward their base each drift step.
const REVERSION: f64 = 0.02;

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
        let base: BTreeMap<Commodity, f64> = [
            (Commodity::Fuel, 10.0),
            (Commodity::Ore, 8.0),
            (Commodity::Alloys, 26.0),
            (Commodity::Provisions, 6.0),
            (Commodity::Volatiles, 18.0),
        ]
        .into_iter()
        .collect();
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
}

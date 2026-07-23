//! RANKINGS (§rankings) — multi-category leaderboards on the ledger clock.
//!
//! Competitive identity fuel: a scoreboard for EVERY playstyle, not one solved
//! "valuation" ladder. Nine categories — merchant, raider, defender,
//! industrialist, spymaster, and the comeback kid — each a cumulative campaign
//! total the sim already produces at events that already fire (deliveries, battle
//! resolutions, builds, scout snapshots, captures). No per-tick cost: the counters
//! are incremented AT those events; the leaderboard is only assembled and PUBLISHED
//! on the periodic ledger close (the same slow cadence as the §9 valuation
//! recompute — see [`crate::world`]).
//!
//! FOG-RESPECTING BY CONSTRUCTION. The published table is a SNAPSHOT copy taken on
//! the ledger tick: between snapshots every corp sees the same last table, so a
//! live counter change never leaks mid-interval. The table is PUBLIC by design —
//! diegetically it is *the exchange's published quarterly ledger*, a matter of
//! record every corporation may read (rank + value per category). Only cumulative,
//! non-secret totals appear; no stockpiles, no fleet positions, no fog to break.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use crate::ship::ShipKind;

/// Minimum engagements a corp must have fought before its BATTLE EFFICIENCY earns
/// a rank/title — so one lucky skirmish can't top the ladder over a seasoned
/// fleet. Tunable.
pub const MIN_RANKED_ENGAGEMENTS: u32 = 3;

/// Hull floor for the efficiency ratio so a FLAWLESS victory (zero hull lost)
/// scores a large FINITE number, not infinity — one raider's hull. Tunable.
pub const EFFICIENCY_HULL_FLOOR: f64 = 20.0;

/// Sum the HULL of a per-kind ship-loss map (the combat report's currency),
/// converting ship counts to comparable hull via [`ShipKind::hull`].
pub fn hull_sum(losses: &BTreeMap<ShipKind, u32>) -> f64 {
    losses.iter().map(|(k, n)| *n as f64 * k.hull()).sum()
}

/// Per-corporation CUMULATIVE counters (campaign totals), incremented
/// deterministically at the events that already exist. Persisted with the corp
/// (`#[serde(default)]` on every field, so pre-feature snapshots load as zeroes).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RankingStats {
    /// TRADE THROUGHPUT — cargo units a corp's convoys successfully HAULED and
    /// delivered (home, an owned/ally system, or carried to the hub and sold).
    /// §TCA: an instant Charterhouse sale settles against the warehouse and moves
    /// nothing, so it earns NO throughput — only goods that actually crossed space
    /// count here.
    #[serde(default)]
    pub trade_units: u64,
    /// NET MARKET PROFIT (revenue side): proceeds from hub sales + filled sell
    /// limit orders.
    #[serde(default)]
    pub market_revenue: f64,
    /// NET MARKET PROFIT (cost side): spend on hub buys + filled buy limit orders.
    #[serde(default)]
    pub market_spend: f64,
    /// CARGO CAPTURED — units seized by raiding a convoy + plunder taken on
    /// capturing a system.
    #[serde(default)]
    pub cargo_captured: u64,
    /// CARGO PROTECTED — units delivered by a convoy that SURVIVED an engagement
    /// en route (it was in a battle and still reached its destination).
    #[serde(default)]
    pub cargo_protected: u64,
    /// BATTLE EFFICIENCY (numerator): enemy hull destroyed across all engagements.
    #[serde(default)]
    pub hull_destroyed: f64,
    /// BATTLE EFFICIENCY (denominator): own hull lost across all engagements.
    #[serde(default)]
    pub hull_lost: f64,
    /// Engagements fought — the min-engagements floor for a ranked efficiency.
    #[serde(default)]
    pub engagements: u32,
    /// SYSTEMS DEVELOPED — total system-upgrade tiers built.
    #[serde(default)]
    pub tiers_built: u32,
    /// INTEL GATHERED — scout snapshots captured (fresh sightings + refreshes).
    #[serde(default)]
    pub intel_snapshots: u32,
    /// RECOVERY (v1): the valuation TROUGH recorded at the corp's last major loss
    /// (a captured system), set at the next ledger recompute after the loss. The
    /// recovery score is how far valuation has climbed back ABOVE this floor.
    /// `None` until the corp has suffered a major loss.
    #[serde(default)]
    pub loss_floor: Option<f64>,
    /// A major loss occurred and awaits the next valuation recompute to stamp the
    /// trough (valuation is stale between the 60 s closes).
    #[serde(default)]
    pub loss_pending: bool,
}

impl RankingStats {
    /// Net market profit = lifetime sell proceeds − lifetime buy spend.
    pub fn market_profit(&self) -> f64 {
        self.market_revenue - self.market_spend
    }

    /// Enemy-hull-destroyed ÷ own-hull-lost, with the hull floor so a flawless
    /// campaign scores high-but-finite rather than infinite.
    pub fn battle_efficiency(&self) -> f64 {
        self.hull_destroyed / self.hull_lost.max(EFFICIENCY_HULL_FLOOR)
    }

    /// Whether this corp has fought enough to earn a RANK in battle efficiency.
    pub fn efficiency_ranked(&self) -> bool {
        self.engagements >= MIN_RANKED_ENGAGEMENTS
    }

    /// Valuation regained since the last major loss (0 if never lost, or still
    /// below the trough). `valuation` is the corp's current (last-close) equity.
    pub fn recovery(&self, valuation: f64) -> f64 {
        match self.loss_floor {
            Some(floor) => (valuation - floor).max(0.0),
            None => 0.0,
        }
    }
}

/// The nine ranking categories, in display order. Each is a scoreboard for one
/// playstyle; the leader of each earns a display TITLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RankingCategory {
    Valuation,
    TradeThroughput,
    MarketProfit,
    CargoCaptured,
    CargoProtected,
    BattleEfficiency,
    SystemsDeveloped,
    IntelGathered,
    Recovery,
}

impl RankingCategory {
    /// All categories, in display order.
    pub const ALL: [RankingCategory; 9] = [
        RankingCategory::Valuation,
        RankingCategory::TradeThroughput,
        RankingCategory::MarketProfit,
        RankingCategory::CargoCaptured,
        RankingCategory::CargoProtected,
        RankingCategory::BattleEfficiency,
        RankingCategory::SystemsDeveloped,
        RankingCategory::IntelGathered,
        RankingCategory::Recovery,
    ];

    /// Stable machine slug (client column key).
    pub fn slug(self) -> &'static str {
        match self {
            RankingCategory::Valuation => "valuation",
            RankingCategory::TradeThroughput => "trade_throughput",
            RankingCategory::MarketProfit => "market_profit",
            RankingCategory::CargoCaptured => "cargo_captured",
            RankingCategory::CargoProtected => "cargo_protected",
            RankingCategory::BattleEfficiency => "battle_efficiency",
            RankingCategory::SystemsDeveloped => "systems_developed",
            RankingCategory::IntelGathered => "intel_gathered",
            RankingCategory::Recovery => "recovery",
        }
    }

    /// The display TITLE the category leader wears — cheap competitive identity.
    pub fn title(self) -> &'static str {
        match self {
            RankingCategory::Valuation => "Magnate",
            RankingCategory::TradeThroughput => "Master Merchant",
            RankingCategory::MarketProfit => "Market Baron",
            RankingCategory::CargoCaptured => "Most Feared",
            RankingCategory::CargoProtected => "Iron Quartermaster",
            RankingCategory::BattleEfficiency => "Warlord",
            RankingCategory::SystemsDeveloped => "Master Builder",
            RankingCategory::IntelGathered => "All-Seeing",
            RankingCategory::Recovery => "Phoenix",
        }
    }
}

/// One corporation's row in a PUBLISHED rankings ledger — a snapshot copy taken on
/// the ledger tick. All values are cumulative campaign totals (public record).
/// Serialisable both ways: persisted with the world, and shipped verbatim to the
/// client (PlayerId serialises as a decimal string, so `player_id` matches the
/// client's own id for the "your row" highlight).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingRow {
    pub player_id: PlayerId,
    pub name: String,
    pub valuation: f64,
    pub trade_throughput: u64,
    pub market_profit: f64,
    pub cargo_captured: u64,
    pub cargo_protected: u64,
    pub battle_efficiency: f64,
    /// Engagements fought — shown so a provisional (unranked) efficiency reads
    /// honestly.
    pub battle_engagements: u32,
    /// Whether `battle_efficiency` met the min-engagements floor (else provisional
    /// — displayed but not ranked / title-eligible).
    pub battle_ranked: bool,
    pub systems_developed: u32,
    pub intel_gathered: u32,
    pub recovery: f64,
    /// The titles this corp holds as of this ledger (category leaders).
    pub titles: Vec<String>,
}

impl RankingRow {
    /// The comparable scalar for one category (higher = better rank). Battle
    /// efficiency below the engagements floor sorts to the bottom (provisional).
    pub fn category_value(&self, cat: RankingCategory) -> f64 {
        match cat {
            RankingCategory::Valuation => self.valuation,
            RankingCategory::TradeThroughput => self.trade_throughput as f64,
            RankingCategory::MarketProfit => self.market_profit,
            RankingCategory::CargoCaptured => self.cargo_captured as f64,
            RankingCategory::CargoProtected => self.cargo_protected as f64,
            RankingCategory::BattleEfficiency => {
                if self.battle_ranked {
                    self.battle_efficiency
                } else {
                    f64::NEG_INFINITY
                }
            }
            RankingCategory::SystemsDeveloped => self.systems_developed as f64,
            RankingCategory::IntelGathered => self.intel_gathered as f64,
            RankingCategory::Recovery => self.recovery,
        }
    }
}

/// Assemble a published ledger from the corp rows: a deterministic base order
/// (valuation desc, then id asc) and the category-leader TITLES stamped on. A
/// title is awarded only for a category value that is meaningfully positive (`> 0`
/// and finite), so a scoreboard where nobody has raided yields no "Most Feared".
/// Ties break to the lowest player id (deterministic). Pure function of the rows.
pub fn assemble(mut rows: Vec<RankingRow>) -> Vec<RankingRow> {
    rows.sort_by(|a, b| {
        b.valuation
            .partial_cmp(&a.valuation)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.player_id.0.cmp(&b.player_id.0))
    });
    for cat in RankingCategory::ALL {
        let mut best: Option<usize> = None;
        for (i, row) in rows.iter().enumerate() {
            let v = row.category_value(cat);
            if v <= 0.0 || !v.is_finite() {
                continue; // nobody leads an all-zero (or provisional) category
            }
            match best {
                None => best = Some(i),
                Some(bi) => {
                    let bv = rows[bi].category_value(cat);
                    if v > bv || (v == bv && row.player_id.0 < rows[bi].player_id.0) {
                        best = Some(i);
                    }
                }
            }
        }
        if let Some(bi) = best {
            rows[bi].titles.push(cat.title().to_string());
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn row(id: u64, valuation: f64) -> RankingRow {
        RankingRow {
            player_id: PlayerId(id),
            name: format!("C{id}"),
            valuation,
            trade_throughput: 0,
            market_profit: 0.0,
            cargo_captured: 0,
            cargo_protected: 0,
            battle_efficiency: 0.0,
            battle_engagements: 0,
            battle_ranked: false,
            systems_developed: 0,
            intel_gathered: 0,
            recovery: 0.0,
            titles: Vec::new(),
        }
    }

    fn titles_of(rows: &[RankingRow], id: u64) -> &[String] {
        &rows.iter().find(|r| r.player_id.0 == id).unwrap().titles
    }

    /// The LEADER of each populated category wears its title; a corp can hold
    /// several; base order is valuation desc then id asc.
    #[test]
    fn assemble_awards_titles_to_category_leaders() {
        let mut a = row(1, 100.0); // top valuation → Magnate
        a.trade_throughput = 50; // top throughput → Master Merchant
        let mut b = row(2, 80.0);
        b.cargo_captured = 30; // top captured → Most Feared
        b.trade_throughput = 10;
        let out = assemble(vec![b, a]);
        // Sorted valuation desc: corp 1 first.
        assert_eq!(out[0].player_id.0, 1);
        assert!(titles_of(&out, 1).contains(&"Magnate".to_string()));
        assert!(titles_of(&out, 1).contains(&"Master Merchant".to_string()));
        assert!(titles_of(&out, 2).contains(&"Most Feared".to_string()));
        assert!(!titles_of(&out, 2).contains(&"Master Merchant".to_string()));
    }

    /// An ALL-ZERO category awards NO title (nobody has raided → no "Most Feared").
    #[test]
    fn assemble_skips_empty_categories() {
        let out = assemble(vec![row(1, 100.0), row(2, 50.0)]);
        // Only Valuation is positive; nobody leads captured/intel/etc.
        assert_eq!(titles_of(&out, 1), &["Magnate".to_string()]);
        assert!(titles_of(&out, 2).is_empty());
    }

    /// A PROVISIONAL efficiency (below the engagements floor) never earns Warlord,
    /// even if numerically highest.
    #[test]
    fn assemble_excludes_provisional_efficiency() {
        let mut a = row(1, 100.0);
        a.battle_efficiency = 9.0;
        a.battle_ranked = false; // too few engagements
        let mut b = row(2, 50.0);
        b.battle_efficiency = 2.0;
        b.battle_ranked = true; // met the floor
        let out = assemble(vec![a, b]);
        assert!(titles_of(&out, 2).contains(&"Warlord".to_string()));
        assert!(!titles_of(&out, 1).contains(&"Warlord".to_string()));
    }

    /// Ties break to the lowest player id (deterministic).
    #[test]
    fn assemble_ties_break_to_lowest_id() {
        let mut a = row(7, 100.0);
        a.systems_developed = 5;
        let mut b = row(3, 100.0);
        b.systems_developed = 5;
        let out = assemble(vec![a, b]);
        assert!(titles_of(&out, 3).contains(&"Master Builder".to_string()));
        assert!(!titles_of(&out, 7).contains(&"Master Builder".to_string()));
    }

    #[test]
    fn stats_math_is_correct() {
        let mut s = RankingStats {
            market_revenue: 300.0,
            market_spend: 120.0,
            hull_destroyed: 100.0,
            ..Default::default()
        };
        assert_eq!(s.market_profit(), 180.0);
        // Efficiency uses the hull floor so a flawless run (0 hull lost) is finite.
        assert_eq!(s.battle_efficiency(), 100.0 / EFFICIENCY_HULL_FLOOR);
        s.hull_lost = 50.0;
        assert_eq!(s.battle_efficiency(), 2.0);
        // Efficiency is ranked only past the engagements floor.
        s.engagements = MIN_RANKED_ENGAGEMENTS - 1;
        assert!(!s.efficiency_ranked());
        s.engagements = MIN_RANKED_ENGAGEMENTS;
        assert!(s.efficiency_ranked());
        // Recovery: 0 until a loss floor exists, then valuation above the trough.
        assert_eq!(s.recovery(1000.0), 0.0);
        s.loss_floor = Some(400.0);
        assert_eq!(s.recovery(1000.0), 600.0);
        assert_eq!(s.recovery(300.0), 0.0); // still below the trough
    }

    #[test]
    fn hull_sum_converts_counts_to_hull() {
        let mut m: BTreeMap<ShipKind, u32> = BTreeMap::new();
        m.insert(ShipKind::Raider, 2);
        m.insert(ShipKind::Convoy, 1);
        // 2 raiders (20 each) + 1 convoy (10) = 50.
        assert_eq!(hull_sum(&m), 2.0 * ShipKind::Raider.hull() + ShipKind::Convoy.hull());
    }
}

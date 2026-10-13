//! §founding R10 · the CHARTER LEDGER. Tiered Authority goals that pick up
//! where the founding charter ends, three visible at a time. Each goal reads the corporation's own
//! world state and pays a small credit stipend once. Money only: the physical
//! economy stays the game, and no goal conjures goods or ships.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::founding::{FoundingProgram, FoundingStage};

/// What a ledger goal measures, always from the corporation's own state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerMetric {
    /// 1 while the corporation belongs to a syndicate.
    Syndicate,
    /// Limit orders placed on the Exchange since the ledger opened.
    LimitOrders,
    /// People living in owned systems.
    Population,
    /// The largest storage capacity among owned systems.
    Storage,
    /// Owned Smelters that are staffed and running.
    Refining,
    /// Hulls under command.
    Hulls,
    /// §charter v3: player Freighter hulls under command.
    Freighters,
    /// Contracts whose completion report has arrived.
    Contracts,
    /// Research programmes completed.
    Research,
    /// Star systems owned.
    Systems,
    /// Structures at Tier II or higher on owned worlds.
    TierTwo,
    /// §power: Power Plants standing on owned worlds.
    PowerPlants,
}

#[derive(Debug, Clone, Copy)]
pub struct LedgerGoal {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub metric: LedgerMetric,
    pub target: f64,
    pub reward: f64,
}

const fn goal(
    id: &'static str,
    title: &'static str,
    detail: &'static str,
    metric: LedgerMetric,
    target: f64,
    reward: f64,
) -> LedgerGoal {
    LedgerGoal { id, title, detail, metric, target, reward }
}

use LedgerMetric as M;

/// The ledger, in the order goals are shown. Early rungs teach the systems a
/// founder has not touched yet (syndicates, the Exchange, refining); later
/// rungs are the long tail of a growing charter.
pub const LEDGER: &[LedgerGoal] = &[
    goal("syndicate", "Join or found a syndicate",
        "Syndicates share defense, freight and research. Accept an invitation or found your own.",
        M::Syndicate, 1.0, 400.0),
    goal("limit_order", "Place an Exchange order",
        "Post a limit order at the Market Hub to buy or sell at your own price.",
        M::LimitOrders, 1.0, 250.0),
    goal("storage_700", "Store 700 goods at home",
        "Expand the camp storehouse or upgrade a Warehouse.",
        M::Storage, 700.0, 300.0),
    goal("population_4k", "House 4,000 people",
        "Build permanent quarters and keep food, jobs and migrants flowing.",
        M::Population, 4_000.0, 300.0),
    goal("refining", "Run a Smelter",
        "Research Enrichment, then build, staff and supply a Smelter.",
        M::Refining, 1.0, 500.0),
    goal("power_plant", "Power a second planet",
        "Your command's planet came with a Power Plant. Every other planet needs its own once it passes its 3 MW base.",
        M::PowerPlants, 2.0, 400.0),
    goal("freighters_2", "Run two Freighters",
        "Build a second Freighter at your Shipyard to carry more trade.",
        M::Freighters, 2.0, 400.0),
    goal("hulls_5", "Command five hulls",
        "Build Freighters and escorts at a staffed Shipyard.",
        M::Hulls, 5.0, 400.0),
    goal("contract_1", "Complete a contract",
        "Take an Authority, salvage or market contract from Operations and see it through.",
        M::Contracts, 1.0, 500.0),
    goal("research_3", "Complete three research programmes",
        "Keep your Academy staffed and a programme queued.",
        M::Research, 3.0, 500.0),
    goal("systems_2", "Hold a second system",
        "Claim a surveyed system with a colony ship or an outpost.",
        M::Systems, 2.0, 1_000.0),
    goal("tier_two", "Raise a structure to Tier II",
        "Upgrade a mine, farm or yard once its research allows.",
        M::TierTwo, 1.0, 600.0),
    goal("population_8k", "House 8,000 people",
        "Grow your worlds' Habitats and keep them fed.",
        M::Population, 8_000.0, 600.0),
    goal("storage_1500", "Store 1,500 goods",
        "Upgrade Warehouses or add orbital storage.",
        M::Storage, 1_500.0, 600.0),
    goal("research_6", "Complete six research programmes",
        "Deepen one tree or broaden across several.",
        M::Research, 6.0, 800.0),
    goal("hulls_8", "Command eight hulls",
        "A larger fleet carries more trade and guards more lanes.",
        M::Hulls, 8.0, 600.0),
    goal("contract_3", "Complete three contracts",
        "Contracts pay credits, standing and captain experience.",
        M::Contracts, 3.0, 800.0),
    goal("systems_3", "Hold three systems",
        "A third system spreads your risk and your supply.",
        M::Systems, 3.0, 1_500.0),
    goal("population_15k", "House 15,000 people",
        "Large populations staff advanced industry.",
        M::Population, 15_000.0, 1_000.0),
    goal("research_10", "Complete ten research programmes",
        "Unlock larger hulls, deeper extraction and better refining.",
        M::Research, 10.0, 1_200.0),
    goal("tier_two_5", "Run five Tier II structures",
        "Upgrade your core industry across your worlds.",
        M::TierTwo, 5.0, 900.0),
    goal("hulls_12", "Command twelve hulls",
        "A fleet that can escort, trade and defend at once.",
        M::Hulls, 12.0, 900.0),
    goal("systems_4", "Hold four systems",
        "A charter the Authority cannot ignore.",
        M::Systems, 4.0, 2_000.0),
    goal("population_30k", "House 30,000 people",
        "A major colony network.",
        M::Population, 30_000.0, 1_500.0),
];

/// Per-corporation ledger progress. Default: closed and empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CharterLedger {
    /// Whether the ledger has opened for this corporation.
    #[serde(default)]
    pub opened: bool,
    /// Goal id → sim time it was met. Grandfathered goals carry no payment.
    #[serde(default)]
    pub completed: BTreeMap<String, f64>,
    /// Goals met before the ledger existed for this save: done, never paid.
    #[serde(default)]
    pub grandfathered: std::collections::BTreeSet<String>,
    /// Exchange limit orders placed since the ledger opened.
    #[serde(default)]
    pub limit_orders: u32,
}

/// The facts a ledger evaluation reads, gathered by the world.
#[derive(Debug, Clone, Copy, Default)]
pub struct LedgerSnapshot {
    pub syndicate: bool,
    pub limit_orders: u32,
    pub population: f64,
    pub storage: f64,
    pub refining: u32,
    pub hulls: u32,
    pub freighters: u32,
    pub contracts: u32,
    pub research: u32,
    pub systems: u32,
    pub tier_two: u32,
    pub power_plants: u32,
}

impl LedgerSnapshot {
    pub fn value(&self, metric: LedgerMetric) -> f64 {
        match metric {
            M::Syndicate => f64::from(u8::from(self.syndicate)),
            M::LimitOrders => f64::from(self.limit_orders),
            M::Population => self.population,
            M::Storage => self.storage,
            M::Refining => f64::from(self.refining),
            M::Hulls => f64::from(self.hulls),
            M::Freighters => f64::from(self.freighters),
            M::Contracts => f64::from(self.contracts),
            M::Research => f64::from(self.research),
            M::Systems => f64::from(self.systems),
            M::TierTwo => f64::from(self.tier_two),
            M::PowerPlants => f64::from(self.power_plants),
        }
    }
}

impl CharterLedger {
    /// Unfinished goals this snapshot meets, in ledger order.
    pub fn newly_met(&self, snapshot: &LedgerSnapshot) -> Vec<&'static LedgerGoal> {
        LEDGER
            .iter()
            .filter(|g| !self.completed.contains_key(g.id) && snapshot.value(g.metric) >= g.target)
            .collect()
    }

    /// The next `n` unfinished goals, in ledger order.
    pub fn next(&self, n: usize) -> Vec<&'static LedgerGoal> {
        LEDGER
            .iter()
            .filter(|g| !self.completed.contains_key(g.id))
            .take(n)
            .collect()
    }
}

/// The ledger opens with the founder's first guarded export (the end of the
/// first session). Only local-opening founders get one; old saves keep theirs
/// closed unless they reach that milestone.
/// §charter v3: the ledger takes over when the founding charter is fulfilled
/// (older programmes opened it at their first-investment step).
pub fn ledger_open(founding: &FoundingProgram) -> bool {
    // §charter v3: the ledger is the charter's sequel, so it opens only once
    // the charter is fulfilled.
    founding.enabled && founding.local_opening && founding.stage == FoundingStage::Complete
}

/// How many goals the ledger shows at once.
pub const LEDGER_VISIBLE: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_ids_are_unique_and_rewards_are_modest() {
        let ids: std::collections::BTreeSet<_> = LEDGER.iter().map(|g| g.id).collect();
        assert_eq!(ids.len(), LEDGER.len());
        assert!((20..=40).contains(&LEDGER.len()));
        assert!(LEDGER.iter().all(|g| g.reward > 0.0 && g.reward <= 2_000.0 && g.target > 0.0));
        assert_eq!(LEDGER[0].metric, LedgerMetric::Syndicate, "the syndicate step leads the ledger");
    }

    #[test]
    fn goals_complete_in_any_order_and_the_next_three_skip_finished_rungs() {
        let mut ledger = CharterLedger::default();
        let snapshot = LedgerSnapshot { population: 4_500.0, storage: 700.0, ..Default::default() };
        let met: Vec<_> = ledger.newly_met(&snapshot).iter().map(|g| g.id).collect();
        assert_eq!(met, ["storage_700", "population_4k"]);
        for id in met {
            ledger.completed.insert(id.into(), 1.0);
        }
        let next: Vec<_> = ledger.next(LEDGER_VISIBLE).iter().map(|g| g.id).collect();
        assert_eq!(next, ["syndicate", "limit_order", "refining"]);
    }
}

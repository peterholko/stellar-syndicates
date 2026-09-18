//! Owner-only transaction history. Entries are appended by the existing receipt
//! delivery pass, NEVER by a query of current market truth. IDs are allocated
//! when light arrives, so neither pagination nor gaps disclose pending business.
//! The complete history lives in memory and in the same 15-minute checkpoint as
//! the galaxy: a crash rolls both back together, without phantom transactions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sim::{EntityId, Event, EventPayload, PlayerId, TradeEvent, World};

use crate::timeline::Timeline;

pub const PAGE_SIZE: usize = 25;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransactionDetails {
    Trade { trade: TradeEvent },
    Purchase {
        item: String, units: u32, unit_price: f64, fees: f64,
        system: Option<EntityId>, fleet: Option<EntityId>,
    },
    Sale { item: String, units: u32, unit_price: f64 },
    Service { name: String, cost: f64, fleet: EntityId },
    /// The old check-in journal retained prose, not exact prices/quantities.
    /// Preserve that evidence verbatim; do not reverse-engineer rounded text
    /// into fake structured financial records.
    EarlierReport { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEntry {
    pub id: u64,
    pub occurred_at: Option<f64>,
    pub reported_at: f64,
    pub details: TransactionDetails,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    pub owner: PlayerId,
    pub occurred_at: f64,
    pub details: TransactionDetails,
}

impl PendingTransaction {
    pub fn from_event(event: &Event) -> Option<Self> {
        use EventPayload::*;
        let (owner, details) = match &event.payload {
            Trade(trade) => (trade.player(), TransactionDetails::Trade { trade: *trade }),
            ModulesPurchased { owner, kind, n, dest, unit_price } => (*owner,
                TransactionDetails::Purchase { item: kind.slug().into(), units: *n,
                    unit_price: *unit_price, fees: 0.0, system: Some(*dest), fleet: None }),
            ModulesSold { owner, kind, n, unit_price } => (*owner,
                TransactionDetails::Sale { item: kind.slug().into(), units: *n, unit_price: *unit_price }),
            SpecialistHired { owner, kind, dest } => (*owner,
                TransactionDetails::Purchase { item: kind.slug().into(), units: 1,
                    unit_price: sim::specialist::SPECIALIST_HIRE_COST, fees: 0.0,
                    system: Some(*dest), fleet: None }),
            HubFuelPurchased { owner, fleet, units, unit_price, penalty } => (*owner,
                TransactionDetails::Purchase { item: "fuel".into(), units: *units,
                    unit_price: *unit_price, fees: *penalty, system: None, fleet: Some(*fleet) }),
            FuelRescueDispatched { owner, fleet, cost, .. } => (*owner,
                TransactionDetails::Service { name: "Authority fuel rescue".into(), cost: *cost, fleet: *fleet }),
            _ => return None,
        };
        Some(Self { owner, occurred_at: event.time, details })
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct TransactionHistory {
    since: Option<f64>,
    entries: BTreeMap<PlayerId, Vec<TransactionEntry>>,
}

impl TransactionHistory {
    /// One-time upgrade of old checkpoints. Only already-received transaction
    /// summaries can be recovered. Pending receipts stay on their original
    /// wavefront; missing older history is explicitly unknown, never fabricated.
    pub fn initialize(&mut self, world: &World, timeline: &Timeline) {
        if self.since.is_some() { return; }
        self.since = Some(world.time);
        for &owner in world.players.keys() {
            for report in timeline.digest(owner).0 {
                if ["Sold ", "Delivery arrived:", "Limit ", "Freight ", "Authority freight"]
                    .iter().any(|prefix| report.text.starts_with(prefix))
                {
                    self.record(owner, None, report.at_time,
                        TransactionDetails::EarlierReport { text: report.text });
                }
            }
        }
    }

    pub fn record(&mut self, owner: PlayerId, occurred_at: Option<f64>, reported_at: f64,
        details: TransactionDetails) -> TransactionEntry
    {
        let history = self.entries.entry(owner).or_default();
        let entry = TransactionEntry { id: history.len() as u64 + 1, occurred_at, reported_at, details };
        history.push(entry.clone());
        entry
    }

    /// O(page size), independent of total history. A per-owner cursor is not an
    /// entity ID or another corporation's transaction counter. The caller gets
    /// its owner exclusively from the authenticated session, never a client ID.
    pub fn page(&self, owner: PlayerId, before: Option<u64>) -> (Vec<TransactionEntry>, Option<u64>) {
        let history = self.entries.get(&owner).map_or(&[][..], Vec::as_slice);
        let end = before.map_or(history.len(), |id| id.saturating_sub(1).min(history.len() as u64) as usize);
        let start = end.saturating_sub(PAGE_SIZE);
        let entries = history[start..end].iter().rev().cloned().collect();
        let next = (start > 0).then_some(start as u64 + 1);
        (entries, next)
    }

    pub fn since(&self) -> f64 { self.since.unwrap_or(0.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_complete_owner_isolated_and_paged_without_duplicate_rows() {
        let mut history = TransactionHistory::default();
        let owner = PlayerId(1);
        for index in 0..87 {
            history.record(owner, Some(index as f64), index as f64 + 40.0,
                TransactionDetails::Trade { trade: TradeEvent::Sold { player: owner,
                    commodity: sim::Commodity::MetallicOre, units: 1, unit_price: 8.22, penalty: 0.1 } });
        }
        assert!(history.page(PlayerId(2), None).0.is_empty());
        let restored: TransactionHistory = crate::persistence::store::decode(
            &crate::persistence::store::encode(&history).unwrap()).unwrap();
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let (page, next) = restored.page(owner, cursor);
            assert!(page.len() <= PAGE_SIZE);
            ids.extend(page.iter().map(|entry| entry.id));
            if next.is_none() { break; }
            cursor = next;
        }
        assert_eq!(ids, (1..=87).rev().collect::<Vec<_>>());
        assert!(restored.page(owner, Some(0)).0.is_empty());
        assert_eq!(restored.page(owner, Some(u64::MAX)).0.len(), PAGE_SIZE);
    }
}

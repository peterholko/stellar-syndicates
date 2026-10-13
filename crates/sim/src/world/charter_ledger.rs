//! §founding R10: evaluate each founder's charter ledger from their own state.
//! Goals complete in any order; each pays its stipend once. Old saves that
//! passed the first export before the ledger existed are grandfathered: goals
//! they already meet are marked done without a payout.
use super::*;
use crate::charter::{self, LedgerSnapshot};

impl World {
    /// Count this tick's Exchange limit orders, open ledgers at the first
    /// guarded export, and pay newly met goals (evaluated once per second).
    pub(super) fn advance_charter_ledgers(&mut self, events: &[Event], out: &mut Vec<Event>) {
        for e in events {
            if let EventPayload::Trade(TradeEvent::LimitPlaced { player, .. }) = e.payload
                && let Some(corp) = self.players.get_mut(&player)
                && corp.ledger.opened
            {
                corp.ledger.limit_orders = corp.ledger.limit_orders.saturating_add(1);
            }
        }
        if self.tick % u64::from(crate::config::TICK_HZ) != 0 {
            return;
        }
        let now = self.time;
        let owners: Vec<PlayerId> = self
            .players
            .iter()
            .filter(|(_, c)| charter::ledger_open(&c.founding))
            .map(|(id, _)| *id)
            .collect();
        for owner in owners {
            let snapshot = self.ledger_snapshot(owner);
            let corp = self.players.get_mut(&owner).unwrap();
            if !corp.ledger.opened {
                corp.ledger.opened = true;
                // A ledger opening as the charter is fulfilled pays what is
                // already met.
                let fresh = corp.founding.milestones.get(&crate::founding::FoundingStage::Complete)
                    .is_some_and(|at| now - at < 60.0);
                if !fresh {
                    for goal in corp.ledger.newly_met(&snapshot) {
                        corp.ledger.completed.insert(goal.id.into(), now);
                        corp.ledger.grandfathered.insert(goal.id.into());
                    }
                    continue;
                }
            }
            for goal in corp.ledger.newly_met(&snapshot) {
                corp.ledger.completed.insert(goal.id.into(), now);
                corp.credits += goal.reward;
                out.push(Event::new(
                    now,
                    EventPayload::CharterStipend {
                        owner,
                        title: format!("Ledger · {}", goal.title),
                        credits: goal.reward,
                    },
                ));
            }
        }
    }

    /// The corporation's own facts the ledger measures.
    pub fn ledger_snapshot(&self, owner: PlayerId) -> LedgerSnapshot {
        let Some(corp) = self.players.get(&owner) else {
            return LedgerSnapshot::default();
        };
        let mut snapshot = LedgerSnapshot {
            syndicate: corp.syndicate.is_some(),
            limit_orders: corp.ledger.limit_orders,
            research: corp.research.completed.len() as u32,
            ..Default::default()
        };
        for sys in self.systems.iter().filter(|s| s.owner == Some(owner)) {
            snapshot.systems += 1;
            snapshot.storage = snapshot.storage.max(sys.storage_cap());
            for body in &sys.bodies {
                snapshot.population += body.population * 1_000_000.0;
                snapshot.tier_two += body.structures.values().filter(|tier| **tier >= 2).count() as u32;
                snapshot.power_plants += u32::from(body.tier(crate::build::StructureKind::PowerPlant) > 0);
                let smelting = body.tier(crate::build::StructureKind::Smelter) > 0
                    && body.assignments.get(&crate::build::StructureKind::Smelter).is_some_and(|a| {
                        a.suspended.is_none()
                            && (a.workers > 0 || a.specialists.values().any(|n| *n > 0))
                    });
                snapshot.refining += u32::from(smelting);
            }
        }
        snapshot.hulls = self
            .fleets
            .values()
            .filter(|f| f.owner == owner)
            .map(|f| f.ships.len() as u32)
            .sum();
        snapshot.freighters = self
            .fleets
            .values()
            .filter(|f| f.owner == owner)
            .map(Fleet::freighter_count)
            .sum();
        snapshot.contracts = self
            .operations
            .values()
            .filter(|o| {
                o.known
                    .get(&owner)
                    .is_some_and(|k| k.state == crate::operation::OperationState::Completed)
            })
            .count() as u32;
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn founder() -> (World, PlayerId) {
        let mut w = World::new(SimConfig::for_players(20_260_929, 2));
        w.enclaves.clear();
        let owner = PlayerId(4_401);
        w.step(&[Command::AddPlayer { id: owner, name: "Ledger".into() }]);
        (w, owner)
    }

    fn tick_once_per_second(w: &mut World, out: &mut Vec<Event>) {
        w.tick = w.tick.next_multiple_of(u64::from(crate::config::TICK_HZ));
        w.advance_charter_ledgers(&[], out);
    }

    #[test]
    fn the_ledger_opens_when_the_charter_is_fulfilled_and_pays_each_goal_once() {
        let (mut w, owner) = founder();
        let mut out = Vec::new();
        tick_once_per_second(&mut w, &mut out);
        assert!(!w.players[&owner].ledger.opened, "closed during the first session");
        let now = w.time;
        w.players.get_mut(&owner).unwrap().founding.set_stage(crate::founding::FoundingStage::BuildShipyard, now);
        tick_once_per_second(&mut w, &mut out);
        assert!(!w.players[&owner].ledger.opened, "closed while the charter still runs");
        w.players.get_mut(&owner).unwrap().founding.set_stage(crate::founding::FoundingStage::Complete, now);
        let before = w.players[&owner].credits;
        // Place a limit order and join a syndicate.
        let limit = Event::new(now, EventPayload::Trade(TradeEvent::LimitPlaced {
            player: owner, side: Side::Buy, commodity: crate::cargo::Commodity::Alloys, units: 1, limit_price: 1.0 }));
        tick_once_per_second(&mut w, &mut out);
        assert!(w.players[&owner].ledger.opened);
        w.advance_charter_ledgers(&[limit], &mut out);
        let syndicate = crate::ids::SyndicateId(7);
        w.players.get_mut(&owner).unwrap().syndicate = Some(syndicate);
        tick_once_per_second(&mut w, &mut out);
        let paid: Vec<_> = out.iter().filter_map(|e| match &e.payload {
            EventPayload::CharterStipend { owner: o, title, credits } if *o == owner => Some((title.clone(), *credits)),
            _ => None,
        }).collect();
        assert!(paid.iter().any(|(t, c)| t == "Ledger · Join or found a syndicate" && *c == 400.0));
        assert!(paid.iter().any(|(t, c)| t == "Ledger · Place an Exchange order" && *c == 250.0));
        let total: f64 = paid.iter().map(|(_, c)| c).sum();
        assert_eq!(w.players[&owner].credits, before + total);
        out.clear();
        tick_once_per_second(&mut w, &mut out);
        assert!(out.is_empty(), "each goal pays once");
    }

    #[test]
    fn a_charter_fulfilled_long_ago_is_grandfathered_without_payment() {
        let (mut w, owner) = founder();
        w.players.get_mut(&owner).unwrap().founding.set_stage(crate::founding::FoundingStage::Complete, 0.0);
        w.time += 3_600.0;
        let before = w.players[&owner].credits;
        let mut out = Vec::new();
        tick_once_per_second(&mut w, &mut out);
        assert!(out.is_empty());
        assert_eq!(w.players[&owner].credits, before);
        assert!(w.players[&owner].ledger.opened);
    }
}

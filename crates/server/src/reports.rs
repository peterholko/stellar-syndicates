//! Delayed report delivery — the per-player *event* scheduler (companion to the
//! per-player view filter, §14).
//!
//! Some events (a raid resolving, §8) are discrete news rather than continuous
//! state. They must reach each involved player only when the event's light
//! reaches that player's command center — so the attacker and the defender of
//! the same raid generally learn the outcome at DIFFERENT times. This scheduler
//! holds each report until its light has arrived for a given player.
//!
//! NOTE (M4): a report is marked delivered when handed to the outbound queue.
//! Reports are rare and the queue is almost never full, but a truly congested
//! client could miss one; M6 (robust sessions) will make delivery reliable
//! (re-deliver until acknowledged).

use std::collections::BTreeMap;

use sim::{Event, EventPayload, PlayerId, RaidOutcome, ShipKind, Vec2};

use crate::protocol::{RaidReport, Role};

/// Reports older than this are pruned even if undelivered (e.g. a recipient who
/// never reconnects) — keeps memory bounded.
const MAX_REPORT_AGE: f64 = 1800.0;

/// §battle-aftermath: concluded-battle reports RETAINED per player after
/// delivery — the aftermath map markers and the battle-results panel read
/// these back from every View, so they survive reconnects. Tunable.
pub const BATTLE_REPORTS_KEPT: usize = 20;

struct Recipient {
    player: PlayerId,
    delivered: bool,
}

struct PendingReport {
    id: u64,
    pos: Vec2,
    event_time: f64,
    attacker: PlayerId,
    defender: PlayerId,
    attacker_ship: sim::EntityId,
    target_ship: sim::EntityId,
    attacker_kind: sim::ShipKind,
    target_kind: sim::ShipKind,
    outcome: RaidOutcome,
    /// Per-kind ships each side lost over the engagement (§FLEETS Part 2).
    attacker_losses: BTreeMap<ShipKind, u32>,
    target_losses: BTreeMap<ShipKind, u32>,
    recipients: Vec<Recipient>,
}

/// §battle-aftermath: one player's RETAINED view of a concluded battle — the
/// full result as THAT side learned it, stamped with when they learned it.
/// Owner-only by construction: it lives keyed under that player and only their
/// View ever carries it.
#[derive(Clone)]
pub struct RetainedReport {
    pub id: u64,
    pub pos: Vec2,
    pub event_time: f64,
    /// Sim-time the report's light reached THIS player's command center.
    pub arrival_time: f64,
    pub you: Role,
    pub attacker_kind: sim::ShipKind,
    pub target_kind: sim::ShipKind,
    pub outcome: RaidOutcome,
    pub attacker_losses: Vec<crate::protocol::CompCount>,
    pub target_losses: Vec<crate::protocol::CompCount>,
}

#[derive(Default)]
pub struct ReportScheduler {
    pending: Vec<PendingReport>,
    next_id: u64,
    /// Delivered reports kept per participant (newest last, capped at
    /// [`BATTLE_REPORTS_KEPT`]).
    retained: BTreeMap<PlayerId, Vec<RetainedReport>>,
}

impl ReportScheduler {
    pub fn new() -> Self {
        ReportScheduler::default()
    }

    /// Ingest a tick's events, queuing delayed reports for raid resolutions.
    pub fn ingest(&mut self, events: &[Event]) {
        for e in events {
            if let EventPayload::RaidResolved {
                attacker,
                defender,
                attacker_ship,
                target_ship,
                attacker_kind,
                target_kind,
                outcome,
                pos,
                attacker_losses,
                target_losses,
            } = &e.payload
            {
                self.next_id += 1;
                self.pending.push(PendingReport {
                    id: self.next_id,
                    pos: *pos,
                    event_time: e.time,
                    attacker: *attacker,
                    defender: *defender,
                    attacker_ship: *attacker_ship,
                    target_ship: *target_ship,
                    attacker_kind: *attacker_kind,
                    target_kind: *target_kind,
                    outcome: *outcome,
                    attacker_losses: attacker_losses.clone(),
                    target_losses: target_losses.clone(),
                    recipients: vec![
                        Recipient { player: *attacker, delivered: false },
                        Recipient { player: *defender, delivered: false },
                    ],
                });
            }
        }
    }

    /// Reports now deliverable to `player` (their light has arrived), tailored
    /// to their side. Marks them delivered and prunes spent/stale reports.
    pub fn due_for(&mut self, player: PlayerId, cc: Vec2, c: f64, now: f64) -> Vec<RaidReport> {
        let mut out = Vec::new();
        for r in &mut self.pending {
            let arrival = r.event_time + r.pos.distance(cc) / c;
            if arrival > now {
                continue; // light hasn't reached this player yet
            }
            for rec in &mut r.recipients {
                if rec.player == player && !rec.delivered {
                    rec.delivered = true;
                    let you = if player == r.attacker { Role::Attacker } else { Role::Defender };
                    out.push(RaidReport {
                        report_id: r.id,
                        outcome: r.outcome,
                        attacker: r.attacker,
                        defender: r.defender,
                        attacker_ship: r.attacker_ship,
                        target_ship: r.target_ship,
                        attacker_kind: r.attacker_kind,
                        target_kind: r.target_kind,
                        pos: r.pos,
                        at_time: r.event_time,
                        age: now - r.event_time,
                        you,
                        attacker_losses: losses_view(&r.attacker_losses),
                        target_losses: losses_view(&r.target_losses),
                    });
                    // §battle-aftermath: RETAIN the delivered report for this
                    // participant (their aftermath marker + results panel),
                    // stamped with the exact light-arrival time. Capped FIFO.
                    let kept = self.retained.entry(player).or_default();
                    kept.push(RetainedReport {
                        id: r.id,
                        pos: r.pos,
                        event_time: r.event_time,
                        arrival_time: arrival,
                        you,
                        attacker_kind: r.attacker_kind,
                        target_kind: r.target_kind,
                        outcome: r.outcome,
                        attacker_losses: losses_view(&r.attacker_losses),
                        target_losses: losses_view(&r.target_losses),
                    });
                    if kept.len() > BATTLE_REPORTS_KEPT {
                        let excess = kept.len() - BATTLE_REPORTS_KEPT;
                        kept.drain(..excess);
                    }
                }
            }
        }
        self.pending.retain(|r| {
            r.recipients.iter().any(|rec| !rec.delivered) && (now - r.event_time) < MAX_REPORT_AGE
        });
        out
    }

    /// §battle-aftermath: the reports `player` has LEARNED of (delivered, so
    /// their light provably arrived), newest last. Strictly per-participant —
    /// a player who wasn't in the battle has no entry to read. Stable across
    /// calls, so a reconnecting client gets its markers back from the next View.
    pub fn retained_for(&self, player: PlayerId) -> &[RetainedReport] {
        self.retained.get(&player).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Flatten a per-kind loss map into the wire form (ordered by kind).
fn losses_view(losses: &BTreeMap<ShipKind, u32>) -> Vec<crate::protocol::CompCount> {
    losses
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(k, n)| crate::protocol::CompCount { kind: *k, count: *n })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::EntityId;

    fn raid_event(time: f64, attacker: PlayerId, defender: PlayerId, pos: Vec2) -> Event {
        Event::new(
            time,
            EventPayload::RaidResolved {
                attacker,
                defender,
                attacker_ship: EntityId(1),
                target_ship: EntityId(2),
                attacker_kind: sim::ShipKind::Raider,
                target_kind: sim::ShipKind::Convoy,
                outcome: RaidOutcome::TargetDestroyed,
                pos,
                attacker_losses: BTreeMap::new(),
                target_losses: BTreeMap::new(),
            },
        )
    }

    #[test]
    fn attacker_and_defender_learn_at_different_times() {
        let c = 300.0;
        let atk = PlayerId(1);
        let def = PlayerId(2);
        let pos = Vec2::new(0.0, 0.0); // raid happened at origin
        let atk_cc = Vec2::new(300.0, 0.0); // 1 s of light away
        let def_cc = Vec2::new(6000.0, 0.0); // 20 s of light away

        let mut sched = ReportScheduler::new();
        sched.ingest(&[raid_event(100.0, atk, def, pos)]);

        // At t=101.5: attacker's light (1 s) has arrived; defender's (20 s) not.
        assert_eq!(sched.due_for(atk, atk_cc, c, 101.5).len(), 1, "attacker should have learned");
        assert_eq!(sched.due_for(def, def_cc, c, 101.5).len(), 0, "defender should NOT know yet");

        // Attacker doesn't get it twice.
        assert_eq!(sched.due_for(atk, atk_cc, c, 130.0).len(), 0);

        // At t=121: defender's light has arrived.
        let d = sched.due_for(def, def_cc, c, 121.0);
        assert_eq!(d.len(), 1, "defender should now have learned");
        assert!((d[0].age - 21.0).abs() < 1e-6, "report age should be the light delay");
    }

    /// §battle-aftermath: retention is strictly per-participant and stamped
    /// with each side's OWN light-arrival time; a third player retains nothing.
    #[test]
    fn retained_reports_are_per_participant_and_light_stamped() {
        let c = 300.0;
        let atk = PlayerId(1);
        let def = PlayerId(2);
        let third = PlayerId(3);
        let pos = Vec2::new(0.0, 0.0);
        let atk_cc = Vec2::new(300.0, 0.0); // 1 s away
        let def_cc = Vec2::new(6000.0, 0.0); // 20 s away
        let third_cc = Vec2::new(600.0, 0.0); // near — but NOT a participant

        let mut sched = ReportScheduler::new();
        sched.ingest(&[raid_event(100.0, atk, def, pos)]);

        // Before anyone's light arrives: nothing retained anywhere.
        sched.due_for(atk, atk_cc, c, 100.5);
        assert!(sched.retained_for(atk).is_empty(), "not retained before the light arrives");

        // Attacker's light arrives → retained for the attacker, with arrival = event + 1 s.
        sched.due_for(atk, atk_cc, c, 101.5);
        let (a_id, a_arrival) = {
            let a = sched.retained_for(atk);
            assert_eq!(a.len(), 1);
            assert!(matches!(a[0].you, Role::Attacker));
            (a[0].id, a[0].arrival_time)
        };
        assert!((a_arrival - 101.0).abs() < 1e-9, "arrival stamp = event + THEIR light delay");
        // The defender (20 s away) has NOT learned yet — nothing retained.
        sched.due_for(def, def_cc, c, 101.5);
        assert!(sched.retained_for(def).is_empty(), "defender retains nothing before their light");

        // Defender's light arrives → their copy, stamped with THEIR arrival.
        sched.due_for(def, def_cc, c, 121.0);
        {
            let d = sched.retained_for(def);
            assert_eq!(d.len(), 1);
            assert!((d[0].arrival_time - 120.0).abs() < 1e-9);
            assert!(matches!(d[0].you, Role::Defender));
            assert_eq!(d[0].id, a_id, "both sides retain the SAME battle id");
        }

        // A non-participant NEVER retains it, no matter how close they were.
        sched.due_for(third, third_cc, c, 200.0);
        assert!(sched.retained_for(third).is_empty(), "leak: a non-participant must retain nothing");

        // Reconnect-stability: reading again returns the same list (the View
        // rebuilds markers from this on every broadcast).
        assert_eq!(sched.retained_for(atk).len(), 1);
        assert_eq!(sched.retained_for(atk)[0].id, a_id);
    }

    /// §battle-aftermath: the per-player journal keeps only the newest
    /// [`BATTLE_REPORTS_KEPT`] reports.
    #[test]
    fn retention_caps_at_kept_limit() {
        let c = 300.0;
        let atk = PlayerId(1);
        let def = PlayerId(2);
        let cc = Vec2::new(0.0, 0.0);
        let mut sched = ReportScheduler::new();
        for i in 0..(BATTLE_REPORTS_KEPT + 5) {
            sched.ingest(&[raid_event(100.0 + i as f64, atk, def, Vec2::new(i as f64, 0.0))]);
        }
        sched.due_for(atk, cc, c, 10_000.0);
        let kept = sched.retained_for(atk);
        assert_eq!(kept.len(), BATTLE_REPORTS_KEPT, "capped at the tunable");
        // Newest survive: the FIRST 5 (oldest) were dropped.
        assert!((kept[0].event_time - 105.0).abs() < 1e-9, "oldest kept = #6");
        assert!((kept.last().unwrap().event_time - (100.0 + (BATTLE_REPORTS_KEPT + 4) as f64)).abs() < 1e-9);
    }
}

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

use sim::{Event, EventPayload, PlayerId, RaidOutcome, Vec2};

use crate::protocol::{RaidReport, Role};

/// Reports older than this are pruned even if undelivered (e.g. a recipient who
/// never reconnects) — keeps memory bounded.
const MAX_REPORT_AGE: f64 = 1800.0;

struct Recipient {
    player: PlayerId,
    delivered: bool,
}

struct PendingReport {
    pos: Vec2,
    event_time: f64,
    attacker: PlayerId,
    defender: PlayerId,
    attacker_ship: sim::EntityId,
    target_ship: sim::EntityId,
    attacker_kind: sim::ShipKind,
    target_kind: sim::ShipKind,
    outcome: RaidOutcome,
    recipients: Vec<Recipient>,
}

#[derive(Default)]
pub struct ReportScheduler {
    pending: Vec<PendingReport>,
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
            } = e.payload
            {
                self.pending.push(PendingReport {
                    pos,
                    event_time: e.time,
                    attacker,
                    defender,
                    attacker_ship,
                    target_ship,
                    attacker_kind,
                    target_kind,
                    outcome,
                    recipients: vec![
                        Recipient { player: attacker, delivered: false },
                        Recipient { player: defender, delivered: false },
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
                    out.push(RaidReport {
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
                        you: if player == r.attacker {
                            Role::Attacker
                        } else {
                            Role::Defender
                        },
                    });
                }
            }
        }
        self.pending.retain(|r| {
            r.recipients.iter().any(|rec| !rec.delivered) && (now - r.event_time) < MAX_REPORT_AGE
        });
        out
    }
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
}

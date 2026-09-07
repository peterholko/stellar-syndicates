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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Recipient {
    player: PlayerId,
    delivered: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PendingReport {
    id: u64,
    #[serde(default)]
    battle_id: Option<sim::EntityId>,
    #[serde(default)]
    aftermath: Option<[sim::combat::aftermath::BattleAftermath; 2]>,
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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RetainedReport {
    pub id: u64,
    #[serde(default)]
    pub battle_id: Option<sim::EntityId>,
    #[serde(default)]
    pub aftermath: Option<sim::combat::aftermath::BattleAftermath>,
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

/// §contestable-territory Part 2: a queued CAPTURE report, delivered per
/// participant when the flip's light reaches them (same machinery as battles).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PendingCapture {
    id: u64,
    pos: Vec2,
    event_time: f64,
    new_owner: PlayerId,
    plunder: Vec<crate::protocol::StockSlot>,
    recipients: Vec<Recipient>,
}

/// A delivered CAPTURE as one participant learned it — powers the capture
/// aftermath marker + results panel. Owner-only by construction (keyed by the
/// two participants). `captor` = you took it; else you lost it.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RetainedCapture {
    pub id: u64,
    pub pos: Vec2,
    pub event_time: f64,
    pub arrival_time: f64,
    pub captor: bool,
    pub plunder: Vec<crate::protocol::StockSlot>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ReportScheduler {
    pending: Vec<PendingReport>,
    next_id: u64,
    /// Delivered reports kept per participant (newest last, capped at
    /// [`BATTLE_REPORTS_KEPT`]).
    retained: BTreeMap<PlayerId, Vec<RetainedReport>>,
    /// §Part 2: queued + delivered CAPTURE reports (same light-delayed, per-
    /// participant retention as battles).
    pending_captures: Vec<PendingCapture>,
    retained_captures: BTreeMap<PlayerId, Vec<RetainedCapture>>,
}

impl ReportScheduler {
    pub fn new() -> Self {
        ReportScheduler::default()
    }

    /// Ingest a tick's events, queuing delayed reports for raid resolutions.
    pub fn ingest(&mut self, events: &[Event]) {
        for e in events {
            if let EventPayload::RaidResolved {
                battle_id,
                aftermath,
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
                    battle_id: *battle_id,
                    aftermath: aftermath.clone(),
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
                        Recipient {
                            player: *attacker,
                            delivered: false,
                        },
                        Recipient {
                            player: *defender,
                            delivered: false,
                        },
                    ],
                });
            } else if let EventPayload::SystemCaptured {
                old_owner,
                new_owner,
                pos,
                plunder,
                ..
            } = &e.payload
            {
                // §Part 2: queue the flip for both participants, light-delayed.
                self.next_id += 1;
                self.pending_captures.push(PendingCapture {
                    id: self.next_id,
                    pos: *pos,
                    event_time: e.time,
                    new_owner: *new_owner,
                    plunder: plunder
                        .iter()
                        .map(|(commodity, units)| crate::protocol::StockSlot {
                            commodity: *commodity,
                            units: *units,
                        })
                        .collect(),
                    recipients: vec![
                        Recipient {
                            player: *old_owner,
                            delivered: false,
                        },
                        Recipient {
                            player: *new_owner,
                            delivered: false,
                        },
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
            let arrival = r.event_time + sim::transit::delay(r.pos, cc, c);
            if arrival > now {
                continue; // light hasn't reached this player yet
            }
            for rec in &mut r.recipients {
                if rec.player == player && !rec.delivered {
                    rec.delivered = true;
                    let you = if player == r.attacker {
                        Role::Attacker
                    } else {
                        Role::Defender
                    };
                    out.push(RaidReport {
                        report_id: r.id,
                        battle_id: r.battle_id,
                        aftermath: r.aftermath.as_ref().map(|sides| sides[usize::from(player != r.attacker)].clone()),
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
                        battle_id: r.battle_id,
                        aftermath: r.aftermath.as_ref().map(|sides| sides[usize::from(player != r.attacker)].clone()),
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
        // §Part 2: deliver + retain CAPTURE reports on the same light gate (no
        // transient toast — the timeline carries the notice; this feeds the
        // marker/panel). Strictly per-participant.
        for cap in &mut self.pending_captures {
            let arrival = cap.event_time + sim::transit::delay(cap.pos, cc, c);
            if arrival > now {
                continue;
            }
            for rec in &mut cap.recipients {
                if rec.player == player && !rec.delivered {
                    rec.delivered = true;
                    let kept = self.retained_captures.entry(player).or_default();
                    kept.push(RetainedCapture {
                        id: cap.id,
                        pos: cap.pos,
                        event_time: cap.event_time,
                        arrival_time: arrival,
                        captor: player == cap.new_owner,
                        plunder: cap.plunder.clone(),
                    });
                    if kept.len() > BATTLE_REPORTS_KEPT {
                        let excess = kept.len() - BATTLE_REPORTS_KEPT;
                        kept.drain(..excess);
                    }
                }
            }
        }
        self.pending_captures.retain(|cap| {
            cap.recipients.iter().any(|rec| !rec.delivered)
                && (now - cap.event_time) < MAX_REPORT_AGE
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

    /// §Part 2: the CAPTURE reports `player` has learned of, newest last —
    /// per-participant (a non-participant has none). Stable across calls.
    pub fn retained_captures_for(&self, player: PlayerId) -> &[RetainedCapture] {
        self.retained_captures
            .get(&player)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// Flatten a per-kind loss map into the wire form (ordered by kind).
fn losses_view(losses: &BTreeMap<ShipKind, u32>) -> Vec<crate::protocol::CompCount> {
    losses
        .iter()
        .filter(|(_, n)| **n > 0)
        .map(|(k, n)| crate::protocol::CompCount {
            kind: *k,
            count: *n,
        })
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
                battle_id: Some(EntityId(99)),
                aftermath: None,
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

    fn capture_event(time: f64, old_owner: PlayerId, new_owner: PlayerId, pos: Vec2) -> Event {
        let mut plunder = BTreeMap::new();
        plunder.insert(sim::Commodity::MetallicOre, 42);
        Event::new(
            time,
            EventPayload::SystemCaptured {
                old_owner,
                new_owner,
                system: EntityId(9),
                pos,
                plunder,
            },
        )
    }

    #[test]
    fn aftermath_is_frozen_owner_only_and_waits_for_conclusion_light() {
        use sim::combat::aftermath::{BattleAftermath, SurvivingFleet, BattleCaptainGain};
        let (a, d) = (PlayerId(1), PlayerId(2));
        let pos = Vec2::new(30_000.0, 0.0);
        let side = |owner: PlayerId, fleet: u64| {
            let captain = sim::Captain::founding(owner, EntityId(fleet));
            BattleAftermath {
                survivors: vec![SurvivingFleet { fleet_id: EntityId(fleet), kind: ShipKind::Raider,
                    composition: BTreeMap::from([(ShipKind::Raider, 2)]), hull: 0.72, withdrew: false,
                    guard_target: None, captain: Some(BattleCaptainGain { id: captain.id, name: captain.name.clone(),
                        portrait: captain.portrait, before: captain.sighting(), after: captain.sighting() }) }],
                bounty_credits: 4500.0,
            }
        };
        let mut event = raid_event(40.0, a, d, pos);
        if let EventPayload::RaidResolved { aftermath, .. } = &mut event.payload {
            *aftermath = Some([side(a, 101), side(d, 202)]);
        }
        let mut sched = ReportScheduler::new();
        sched.ingest(&[event]);
        let saved = serde_json::to_value(&sched).unwrap();
        let mut sched: ReportScheduler = serde_json::from_value(saved.clone()).unwrap();
        assert!(sched.due_for(a, Vec2::ZERO, 400.0, 54.999).is_empty());
        assert!(sched.retained_for(a).is_empty(), "survivors and promotions cannot bypass the 15s return leg");
        assert!(sched.due_for(PlayerId(3), pos, 400.0, 55.0).is_empty());
        let learned = sched.due_for(a, Vec2::ZERO, 400.0, 55.0);
        let own = learned[0].aftermath.as_ref().unwrap();
        assert_eq!(own.survivors.len(), 1);
        assert_eq!(own.survivors[0].fleet_id, EntityId(101));
        assert_eq!(own.bounty_credits, 4500.0);
        let packet = crate::wire::encode_server(&crate::protocol::ServerMsg::Report { report: learned[0].clone() }).unwrap();
        let decoded: serde_json::Value = rmp_serde::from_slice(&packet[4..]).unwrap();
        assert_eq!(decoded["report"]["aftermath"]["survivors"][0]["fleet_id"], "101");
        assert!(!decoded.to_string().contains("202"), "opponent officer/fleet details must not enter the DTO");
        let restored: ReportScheduler = serde_json::from_value(serde_json::to_value(&sched).unwrap()).unwrap();
        assert_eq!(restored.retained_for(a)[0].aftermath.as_ref().unwrap().survivors[0].hull, 0.72);
        assert!(restored.retained_for(d).is_empty());
        let defender = sched.due_for(d, Vec2::ZERO, 400.0, 55.0);
        assert_eq!(defender[0].aftermath.as_ref().unwrap().survivors[0].fleet_id, EntityId(202));
        let mut legacy = saved;
        legacy["pending"][0].as_object_mut().unwrap().remove("aftermath");
        let mut old: ReportScheduler = serde_json::from_value(legacy).unwrap();
        assert!(old.due_for(a, Vec2::ZERO, 400.0, 55.0)[0].aftermath.is_none());
    }

    #[test]
    fn battle_identity_survives_delayed_reports_and_checkpoint_roundtrips() {
        let (attacker, defender) = (PlayerId(1), PlayerId(2));
        let (cc, pos, c) = (Vec2::ZERO, Vec2::new(30_000.0, 0.0), 400.0);
        let first = raid_event(40.0, attacker, defender, pos);
        let mut second = first.clone();
        if let EventPayload::RaidResolved { battle_id, .. } = &mut second.payload {
            *battle_id = Some(EntityId(100));
        }
        let mut sched = ReportScheduler::new();
        // Same position AND conclusion instant: only the engagement id can
        // identify these separate fights. Their report counters are not ids 99/100.
        sched.ingest(&[first, second]);
        let frozen = serde_json::to_value(&sched).unwrap();
        let mut restored: ReportScheduler = serde_json::from_value(frozen.clone()).unwrap();
        assert!(restored.due_for(attacker, cc, c, 54.999).is_empty());
        assert!(restored.retained_for(attacker).is_empty());
        let reports = restored.due_for(attacker, cc, c, 55.0);
        assert_eq!(reports.iter().map(|r| r.report_id).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(reports.iter().map(|r| r.battle_id).collect::<Vec<_>>(), vec![Some(EntityId(99)), Some(EntityId(100))]);
        let packet = crate::wire::encode_server(&crate::protocol::ServerMsg::Report {
            report: reports[0].clone(),
        }).unwrap();
        let decoded: serde_json::Value = rmp_serde::from_slice(&packet[4..]).unwrap();
        assert_eq!(decoded["report"]["battle_id"], "99", "binary transport keeps engagement ids as strings");
        let reopened: ReportScheduler = serde_json::from_value(serde_json::to_value(&restored).unwrap()).unwrap();
        assert_eq!(reopened.retained_for(attacker).iter().map(|r| r.battle_id).collect::<Vec<_>>(),
            vec![Some(EntityId(99)), Some(EntityId(100))]);
        assert!(reopened.retained_for(defender).is_empty());

        let mut legacy = frozen;
        legacy["pending"][0].as_object_mut().unwrap().remove("battle_id");
        let mut old: ReportScheduler = serde_json::from_value(legacy).unwrap();
        let reports = old.due_for(attacker, cc, c, 55.0);
        assert_eq!(reports[0].battle_id, None, "legacy reports stay readable without guessing a link");
        assert_eq!(reports[1].battle_id, Some(EntityId(100)));
    }

    /// §Part 2: a CAPTURE is retained per-participant, each stamped with THEIR
    /// own light-arrival; a non-participant retains nothing (leak check). The
    /// captor's report is `captor=true`, the old owner's `captor=false`, both
    /// carrying the plundered stockpile (the defender's report itemizes it).
    #[test]
    fn capture_reports_are_per_participant_and_light_stamped() {
        let c = 300.0;
        let old_owner = PlayerId(1);
        let captor = PlayerId(2);
        let third = PlayerId(3);
        let pos = Vec2::new(0.0, 0.0);
        let old_cc = Vec2::new(300.0, 0.0); // 0.2 s of warp light away
        let cap_cc = Vec2::new(6000.0, 0.0); // 4 s of warp light away
        let third_cc = Vec2::new(600.0, 0.0); // near, but NOT a participant

        let mut sched = ReportScheduler::new();
        sched.ingest(&[capture_event(100.0, old_owner, captor, pos)]);

        // Old owner (0.2 s) learns first; the captor (4 s) not yet.
        sched.due_for(old_owner, old_cc, c, 101.5);
        sched.due_for(captor, cap_cc, c, 101.5);
        let lost_id = {
            let lost = sched.retained_captures_for(old_owner);
            assert_eq!(lost.len(), 1);
            assert!(!lost[0].captor, "the old owner's report reads as a LOSS");
            assert!((lost[0].arrival_time - 100.2).abs() < 1e-9);
            assert_eq!(
                lost[0].plunder.first().map(|s| s.units),
                Some(42),
                "the loss is itemized"
            );
            lost[0].id
        };
        assert!(
            sched.retained_captures_for(captor).is_empty(),
            "captor hasn't learned yet"
        );

        // The captor's light arrives → their own report, same battle id.
        sched.due_for(captor, cap_cc, c, 105.0);
        {
            let took = sched.retained_captures_for(captor);
            assert_eq!(took.len(), 1);
            assert!(took[0].captor, "the captor's report reads as a CAPTURE");
            assert_eq!(took[0].id, lost_id, "both sides share the capture id");
        }

        // A non-participant never retains it, however close.
        sched.due_for(third, third_cc, c, 300.0);
        assert!(
            sched.retained_captures_for(third).is_empty(),
            "leak: a non-participant sees no capture"
        );
    }

    #[test]
    fn attacker_and_defender_learn_at_different_times() {
        let c = 300.0;
        let atk = PlayerId(1);
        let def = PlayerId(2);
        let pos = Vec2::new(0.0, 0.0); // raid happened at origin
        let atk_cc = Vec2::new(300.0, 0.0); // 0.2 s of warp light away
        let def_cc = Vec2::new(6000.0, 0.0); // 4 s of warp light away

        let mut sched = ReportScheduler::new();
        sched.ingest(&[raid_event(100.0, atk, def, pos)]);

        // At t=101.5: attacker's light has arrived; defender's has not.
        assert_eq!(
            sched.due_for(atk, atk_cc, c, 101.5).len(),
            1,
            "attacker should have learned"
        );
        assert_eq!(
            sched.due_for(def, def_cc, c, 101.5).len(),
            0,
            "defender should NOT know yet"
        );

        // Attacker doesn't get it twice.
        assert_eq!(sched.due_for(atk, atk_cc, c, 130.0).len(), 0);

        // At t=105: defender's light has arrived.
        let d = sched.due_for(def, def_cc, c, 105.0);
        assert_eq!(d.len(), 1, "defender should now have learned");
        assert!(
            (d[0].age - 5.0).abs() < 1e-6,
            "report age should be the light delay"
        );
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
        let atk_cc = Vec2::new(300.0, 0.0); // 0.2 s of warp light away
        let def_cc = Vec2::new(6000.0, 0.0); // 4 s of warp light away
        let third_cc = Vec2::new(600.0, 0.0); // near — but NOT a participant

        let mut sched = ReportScheduler::new();
        sched.ingest(&[raid_event(100.0, atk, def, pos)]);

        // Before anyone's light arrives: nothing retained anywhere.
        sched.due_for(atk, atk_cc, c, 100.1);
        assert!(
            sched.retained_for(atk).is_empty(),
            "not retained before the light arrives"
        );

        // Attacker's light arrives → retained with its exact warp-light timestamp.
        sched.due_for(atk, atk_cc, c, 101.5);
        let (a_id, a_arrival) = {
            let a = sched.retained_for(atk);
            assert_eq!(a.len(), 1);
            assert!(matches!(a[0].you, Role::Attacker));
            (a[0].id, a[0].arrival_time)
        };
        assert!(
            (a_arrival - 100.2).abs() < 1e-9,
            "arrival stamp = event + THEIR light delay"
        );
        // The defender (4 s away) has NOT learned yet — nothing retained.
        sched.due_for(def, def_cc, c, 101.5);
        assert!(
            sched.retained_for(def).is_empty(),
            "defender retains nothing before their light"
        );

        // Defender's light arrives → their copy, stamped with THEIR arrival.
        sched.due_for(def, def_cc, c, 105.0);
        {
            let d = sched.retained_for(def);
            assert_eq!(d.len(), 1);
            assert!((d[0].arrival_time - 104.0).abs() < 1e-9);
            assert!(matches!(d[0].you, Role::Defender));
            assert_eq!(d[0].id, a_id, "both sides retain the SAME battle id");
        }

        // A non-participant NEVER retains it, no matter how close they were.
        sched.due_for(third, third_cc, c, 200.0);
        assert!(
            sched.retained_for(third).is_empty(),
            "leak: a non-participant must retain nothing"
        );

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
            sched.ingest(&[raid_event(
                100.0 + i as f64,
                atk,
                def,
                Vec2::new(i as f64, 0.0),
            )]);
        }
        sched.due_for(atk, cc, c, 10_000.0);
        let kept = sched.retained_for(atk);
        assert_eq!(kept.len(), BATTLE_REPORTS_KEPT, "capped at the tunable");
        // Newest survive: the FIRST 5 (oldest) were dropped.
        assert!(
            (kept[0].event_time - 105.0).abs() < 1e-9,
            "oldest kept = #6"
        );
        assert!(
            (kept.last().unwrap().event_time - (100.0 + (BATTLE_REPORTS_KEPT + 4) as f64)).abs()
                < 1e-9
        );
    }
}

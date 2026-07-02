//! The per-player check-in timeline (§16, async-automation Layer 3).
//!
//! Pillar 1 of the design: *presence gives awareness, not advantage*. A player
//! who logs in once a day must still be able to make good decisions — so when
//! they check in they need a clear digest of **what became observable while they
//! were away**, and a nudge toward the decisions waiting for them. (The decisions
//! themselves — "attention items" — are derived client-side from the player's own
//! View, since that already carries their owned systems, stockpiles, and standing
//! orders; this module owns only the historical *timeline*.)
//!
//! The timeline is a RETAINED, per-player journal that records discrete news at
//! the moment it BECAME OBSERVABLE to that player — own economy/automation on
//! their own clock (instant), distant battles and rival claims **light-delayed**
//! to their command center (the same retarded-time rule as [`crate::reports`] and
//! the view filter). Crucially it is populated for offline players too, so the
//! "since you were away" digest is real. It is ephemeral awareness state (not part
//! of the deterministic sim snapshot), like the report scheduler.

use std::collections::{BTreeMap, VecDeque};

use sim::{Commodity, Event, EventPayload, PlayerId, RaidOutcome, ShipKind, TradeEvent, World};

use crate::protocol::{TimelineEntry, TimelineSeverity};

/// Keep at most this many entries per player (a rolling check-in log).
const JOURNAL_CAP: usize = 40;
/// Drop a not-yet-observable entry whose light somehow never lands (bounded mem).
const MAX_PENDING_AGE: f64 = 1800.0;

struct Pending {
    player: PlayerId,
    /// Sim-time at which this becomes observable to `player` (light-arrival).
    observe_time: f64,
    severity: TimelineSeverity,
    text: String,
    promoted: bool,
}

#[derive(Default)]
pub struct Timeline {
    pending: Vec<Pending>,
    journal: BTreeMap<PlayerId, VecDeque<TimelineEntry>>,
    /// Last sim-time each player was online — the "while you were away" boundary.
    last_seen: BTreeMap<PlayerId, f64>,
}

impl Timeline {
    pub fn new() -> Self {
        Timeline::default()
    }

    /// Record a tick's events as future timeline entries, each at its
    /// player-specific observable time. Runs for ALL players (online or off).
    pub fn ingest(&mut self, events: &[Event], world: &World) {
        let c = world.config.c;
        for e in events {
            match &e.payload {
                // Own economy / automation — observable on your own clock now.
                EventPayload::Trade(te) => {
                    if let Some((sev, text)) = trade_entry(te, world) {
                        self.push(te.player(), e.time, sev, text);
                    }
                }
                // A battle — each side learns the SAME outcome when its light
                // reaches their command center (so generally at different times).
                EventPayload::RaidResolved {
                    attacker,
                    defender,
                    attacker_kind,
                    target_kind,
                    outcome,
                    pos,
                    ..
                } => {
                    for &p in &[*attacker, *defender] {
                        let Some(cc) = world.players.get(&p).map(|corp| corp.command_center) else {
                            continue;
                        };
                        let observe = e.time + pos.distance(cc) / c;
                        let (sev, text) = raid_entry(p, *attacker, *defender, *attacker_kind, *target_kind, *outcome);
                        self.push(p, observe, sev, text);
                    }
                }
                // The galaxy changed: your own claim is instant; a rival's claim is
                // awareness that arrives light-delayed (same gate as the map).
                EventPayload::SystemClaimed { system, owner, pos } => {
                    let name = system_name(world, *system);
                    for (&p, corp) in &world.players {
                        if p == *owner {
                            self.push(p, e.time, TimelineSeverity::Good, format!("You claimed {name}."));
                        } else {
                            let observe = e.time + pos.distance(corp.command_center) / c;
                            self.push(p, observe, TimelineSeverity::Warn, format!("A rival claimed {name}."));
                        }
                    }
                }
                // Construction is your own private administration (§step1) — owner-only,
                // observable instantly; the finished ship reveals as a light-gated ghost.
                EventPayload::BuildStarted { owner, system, what, .. } => {
                    let name = system_name(world, *system);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!("Construction started at {name}: {}.", build_label(*what)));
                }
                EventPayload::SystemUpgraded { owner, system, upgrade, tier } => {
                    let name = system_name(world, *system);
                    let what = match upgrade {
                        sim::SystemUpgrade::Extractor => format!("Extractor tier {tier} (more output)"),
                        sim::SystemUpgrade::Depot => format!("Depot tier {tier} (more storage)"),
                        sim::SystemUpgrade::Shipyard => format!("Shipyard tier {tier} (builds ships)"),
                        sim::SystemUpgrade::SensorArray => format!("Sensor Array tier {tier} (standing vision)"),
                    };
                    self.push(*owner, e.time, TimelineSeverity::Good, format!("{name} developed — {what}."));
                }
                // A soft-rejected build (§buildings step 1) — owner-only, instant
                // (your own administration): nothing was spent, the request just
                // couldn't be hosted. Tells the player WHY so the fix is obvious.
                EventPayload::BuildRejected { owner, system, what, reason } => {
                    let name = system_name(world, *system);
                    let text = match reason {
                        sim::BuildRejectReason::NoSlot => format!(
                            "Can't build {} at {name}: every development slot is used — systems must specialize.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::NeedsShipyard { required } => format!(
                            "Can't build {} at {name}: needs Shipyard tier {required} there.",
                            build_label(*what)
                        ),
                    };
                    self.push(*owner, e.time, TimelineSeverity::Warn, text);
                }
                EventPayload::FuelShortfall { owner, needed, kind } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn,
                        format!("A {} was held — out of fuel (needed ~{:.0}). Stockpile fuel near your fleet.", kind.label(), needed));
                }
                _ => {}
            }
        }
    }

    fn push(&mut self, player: PlayerId, observe_time: f64, severity: TimelineSeverity, text: String) {
        self.pending.push(Pending { player, observe_time, severity, text, promoted: false });
    }

    /// Promote every entry whose light has now arrived into its player's journal.
    /// Idempotent per entry; bounds each journal to the most recent [`JOURNAL_CAP`].
    pub fn promote(&mut self, now: f64) {
        for p in &mut self.pending {
            if !p.promoted && p.observe_time <= now {
                p.promoted = true;
                let j = self.journal.entry(p.player).or_default();
                j.push_back(TimelineEntry {
                    at_time: p.observe_time,
                    severity: p.severity,
                    text: p.text.clone(),
                });
                while j.len() > JOURNAL_CAP {
                    j.pop_front();
                }
            }
        }
        self.pending
            .retain(|p| !p.promoted && (now - p.observe_time) < MAX_PENDING_AGE);
    }

    /// Note that `player` is currently online at `now` — moves their
    /// "while you were away" boundary forward.
    pub fn mark_seen(&mut self, player: PlayerId, now: f64) {
        self.last_seen.insert(player, now);
    }

    /// The player's current journal plus the sim-time they were last online (the
    /// boundary the client uses to split "while you were away" from earlier).
    pub fn digest(&self, player: PlayerId) -> (Vec<TimelineEntry>, f64) {
        let entries = self
            .journal
            .get(&player)
            .map(|j| j.iter().cloned().collect())
            .unwrap_or_default();
        let away_since = self.last_seen.get(&player).copied().unwrap_or(0.0);
        (entries, away_since)
    }

    /// How many entries the player's journal holds (for cheap change-detection).
    pub fn journal_len(&self, player: PlayerId) -> usize {
        self.journal.get(&player).map(|j| j.len()).unwrap_or(0)
    }
}

fn commodity_name(c: Commodity) -> &'static str {
    match c {
        Commodity::Fuel => "fuel",
        Commodity::Ore => "ore",
        Commodity::Alloys => "alloys",
        Commodity::Provisions => "provisions",
        Commodity::Volatiles => "volatiles",
    }
}

fn system_name(world: &World, id: sim::EntityId) -> String {
    world
        .systems
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("{id}"))
}

/// Human label for a build job, for the check-in timeline (§step1).
fn build_label(what: sim::BuildKind) -> &'static str {
    match what {
        sim::BuildKind::Ship { ship: sim::ShipKind::Convoy } => "a Convoy",
        sim::BuildKind::Ship { ship: sim::ShipKind::Raider } => "a Raider",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Extractor } => "an Extractor",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Depot } => "a Depot",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Shipyard } => "a Shipyard",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::SensorArray } => "a Sensor Array",
    }
}

fn kind_word(k: ShipKind) -> &'static str {
    match k {
        ShipKind::Convoy => "convoy",
        ShipKind::Raider => "raider",
    }
}

/// The check-in line for an economy event the recipient cares about while away,
/// or `None` for the noisy/online-only ones we deliberately skip.
fn trade_entry(te: &TradeEvent, world: &World) -> Option<(TimelineSeverity, String)> {
    use TimelineSeverity::*;
    Some(match *te {
        TradeEvent::AutoDispatched { commodity, units, source, rule_id, .. } => (
            Good,
            format!(
                "Standing order #{rule_id} auto-shipped {units} {} from {} (raidable).",
                commodity_name(commodity),
                system_name(world, source)
            ),
        ),
        TradeEvent::Sold { commodity, units, unit_price, .. } => (
            Good,
            format!("Sold {units} {} at the hub for {unit_price:.2} ea.", commodity_name(commodity)),
        ),
        TradeEvent::Delivered { commodity, units, .. } => (
            Good,
            format!("Delivery arrived: +{units} {}.", commodity_name(commodity)),
        ),
        TradeEvent::LimitFilled { commodity, units, unit_price, side, .. } => {
            let s = format!("{side:?}").to_lowercase();
            (Good, format!("Limit {s} filled: {units} {} @ {unit_price:.2}.", commodity_name(commodity)))
        }
        TradeEvent::SupplyDiverted { commodity, units, system, action, .. } => {
            let name = system_name(world, system);
            let com = commodity_name(commodity);
            match action {
                sim::DivertAction::Lost => (
                    Bad,
                    format!("Supply to {name} lost — you no longer hold it: {units} {com} dropped."),
                ),
                sim::DivertAction::ReturnedHome => (
                    Warn,
                    format!("Supply to {name} re-routed home ({units} {com}) — system lost."),
                ),
                sim::DivertAction::SoldAtHub => (
                    Warn,
                    format!("Supply to {name} re-routed to sell at the hub ({units} {com}) — system lost."),
                ),
            }
        }
        // A full depot bounced part of a delivery onward to the hub (§buildings
        // step 2) — an attention item: the player should ship out or build a Depot.
        TradeEvent::StorageOverflow { commodity, units, system, .. } => {
            let name = system_name(world, system);
            (
                Warn,
                format!(
                    "Depot full at {name}: {units} {} couldn't be stored — re-routed to sell at the hub. Ship goods out or build a Depot.",
                    commodity_name(commodity)
                ),
            )
        }
        // Online-only / low-signal news (manual buys, dispatch-started, resting
        // placements) stays out of the away-digest.
        _ => return None,
    })
}

/// The check-in line for a battle, framed for `me`'s role and the outcome.
fn raid_entry(
    me: PlayerId,
    attacker: PlayerId,
    defender: PlayerId,
    attacker_kind: ShipKind,
    target_kind: ShipKind,
    outcome: RaidOutcome,
) -> (TimelineSeverity, String) {
    use TimelineSeverity::*;
    let i_attack = me == attacker;
    let _ = defender;
    match outcome {
        RaidOutcome::TargetDestroyed => {
            if i_attack {
                (Good, format!("Your raider destroyed an enemy {}.", kind_word(target_kind)))
            } else {
                (Bad, format!("You lost a {} to a hostile raider.", kind_word(target_kind)))
            }
        }
        RaidOutcome::AttackerDestroyed => {
            if i_attack {
                (Bad, format!("Your {} was destroyed in the attack.", kind_word(attacker_kind)))
            } else {
                (Good, "You destroyed an attacking raider.".to_string())
            }
        }
        RaidOutcome::BothDestroyed => (Bad, "Both ships were lost in a clash.".to_string()),
        RaidOutcome::BothSurvive => {
            if i_attack {
                (Info, "Your attack was driven off — no losses.".to_string())
            } else {
                (Good, "You drove off an attacking raider — no losses.".to_string())
            }
        }
        RaidOutcome::Escaped => {
            if i_attack {
                (Info, "Your quarry escaped to the hub.".to_string())
            } else {
                (Good, "Your convoy reached the hub safely.".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::{Command, EntityId, SimConfig, Vec2};

    fn world_with_two() -> (World, PlayerId, PlayerId) {
        let mut w = World::new(SimConfig::for_players(7, 4));
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        (w, a, b)
    }

    #[test]
    fn own_economy_news_is_observable_immediately() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        let ev = Event::new(
            w.time,
            EventPayload::Trade(TradeEvent::Sold {
                player: a,
                commodity: Commodity::Ore,
                units: 12,
                unit_price: 8.0,
            }),
        );
        tl.ingest(&[ev], &w);
        tl.promote(w.time);
        let (entries, _away) = tl.digest(a);
        assert_eq!(entries.len(), 1, "own sale should journal at once");
        assert!(entries[0].text.contains("Sold 12 ore"));
    }

    #[test]
    fn a_rival_claim_arrives_light_delayed() {
        let (w, a, b) = world_with_two();
        let mut tl = Timeline::new();
        // A claim far from b's command center: its light takes time to arrive.
        let cc = w.players[&b].command_center;
        let pos = cc + Vec2::new(3000.0, 0.0); // 10 s of light away at c=300
        let ev = Event::new(w.time, EventPayload::SystemClaimed { system: EntityId(999), owner: a, pos });
        tl.ingest(&[ev], &w);

        // Immediately: the owner knows; the rival does not yet.
        tl.promote(w.time);
        assert_eq!(tl.journal_len(a), 1, "owner learns instantly");
        assert_eq!(tl.journal_len(b), 0, "rival's light hasn't arrived");

        // After the light delay: the rival now sees it (light-respecting awareness).
        tl.promote(w.time + 11.0);
        assert_eq!(tl.journal_len(b), 1, "rival learns after the light arrives");
        let (entries, _) = tl.digest(b);
        assert!(entries[0].text.contains("rival claimed"));
    }

    #[test]
    fn offline_buffering_and_away_boundary() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        // Player A was last online at t = 5.
        tl.mark_seen(a, 5.0);
        // While "away", two automation events occur and become observable.
        for (t, units) in [(10.0_f64, 3u32), (20.0, 4)] {
            let ev = Event::new(
                t,
                EventPayload::Trade(TradeEvent::AutoDispatched {
                    player: a,
                    commodity: Commodity::Fuel,
                    units,
                    source: EntityId(1),
                    rule_id: 1,
                }),
            );
            tl.ingest(&[ev], &w);
        }
        tl.promote(25.0); // time has marched on while A was gone
        let (entries, away_since) = tl.digest(a);
        assert_eq!(entries.len(), 2, "events buffered while offline");
        assert_eq!(away_since, 5.0, "away boundary is the last-online time");
        assert!(entries.iter().all(|e| e.at_time > away_since), "all are 'while you were away'");
    }

    #[test]
    fn journal_is_bounded() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        for i in 0..(JOURNAL_CAP as u32 + 15) {
            let ev = Event::new(
                i as f64,
                EventPayload::Trade(TradeEvent::Delivered {
                    player: a,
                    commodity: Commodity::Ore,
                    units: i + 1,
                }),
            );
            tl.ingest(&[ev], &w);
        }
        tl.promote(1000.0);
        assert_eq!(tl.journal_len(a), JOURNAL_CAP, "journal keeps only the most recent cap");
    }
}

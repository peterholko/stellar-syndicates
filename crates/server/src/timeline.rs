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
//! the moment it BECAME OBSERVABLE to that player — economy/automation from its
//! physical Market Hub or system origin, distant battles and rival claims **light-delayed**
//! to their command center (the same retarded-time rule as [`crate::reports`] and
//! the view filter). Crucially it is populated for offline players too, so the
//! "since you were away" digest is real. It is ephemeral awareness state (not part
//! of the deterministic sim snapshot), like the report scheduler.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use sim::{Commodity, Event, EventPayload, PlayerId, RaidOutcome, ShipKind, TradeEvent, World};

use crate::protocol::{LogData, LogKind, LogPhase, LogRefs, TimelineEntry, TimelineSeverity};

/// Keep at most this many entries per player (a rolling check-in log). §log:
/// 40 forgot an active corporation's afternoon; past the cap, routine lines go
/// before warnings ([`trim`]).
const JOURNAL_CAP: usize = 300;
/// §log: and no more than a week of it.
const JOURNAL_MAX_AGE: f64 = 7.0 * 24.0 * 3600.0;
/// §log: a production stop that clears this fast was never news.
const SUSPEND_GRACE_S: f64 = 120.0;
/// Drop a not-yet-observable entry whose light somehow never lands (bounded mem).
const MAX_PENDING_AGE: f64 = 1800.0;
/// A held blockade strips about a unit and a half a second; each side gets one
/// line a minute per system for it, with the running haul, instead of one a unit.
const PLUNDER_LINE_SECS: f64 = 60.0;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Pending {
    player: PlayerId,
    /// Sim-time at which this becomes observable to `player` (light-arrival).
    observe_time: f64,
    severity: TimelineSeverity,
    text: String,
    promoted: bool,
    /// §welcome (R9): what the line is about, for the report on return.
    #[serde(default)]
    topic: Option<crate::welcome::Topic>,
    #[serde(default)]
    system: Option<sim::EntityId>,
    /// §notifications (R8): the alert this line raises for its player.
    #[serde(default)]
    alert: Option<crate::notify::AlertKind>,
    /// §log: kind, group, phase, refs and numbers for the client.
    #[serde(default)]
    meta: Meta,
}

/// §log (v71): what a line is about, beyond its text. Filled per event after
/// its arm runs ([`log_meta`]) unless the arm set it itself.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Meta {
    #[serde(default)]
    kind: Option<LogKind>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    phase: Option<LogPhase>,
    #[serde(default)]
    refs: LogRefs,
    #[serde(default)]
    data: Option<Box<LogData>>,
    /// Lines with one key that land in the same second merge into one.
    #[serde(default)]
    merge: Option<String>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Timeline {
    pending: Vec<Pending>,
    journal: BTreeMap<PlayerId, VecDeque<TimelineEntry>>,
    /// Last sim-time each player was online — the "while you were away" boundary.
    last_seen: BTreeMap<PlayerId, f64>,
    /// Length stops changing once the journal is full. Use a persisted revision
    /// so late-game raid warnings (and other fresh news) still reach live clients.
    #[serde(default)]
    revisions: BTreeMap<PlayerId, usize>,
    /// Plunder not yet put in a line, per (player, system): when that player's
    /// last plunder line for the system went out and the goods since.
    #[serde(skip)]
    plunder: BTreeMap<(PlayerId, sim::EntityId), (f64, BTreeMap<Commodity, u32>)>,
    /// §welcome (R9): each player's topic tallies since they were last
    /// online; cleared while they are on.
    #[serde(default)]
    away: BTreeMap<PlayerId, crate::welcome::AwayTally>,
    /// §notifications (R8): alerting lines promoted since the game loop last
    /// took them.
    #[serde(skip)]
    fresh_alerts: Vec<(PlayerId, crate::notify::AlertKind, String)>,
    /// §log: the last entry id handed out (ids are unique across players).
    #[serde(default)]
    next_id: u64,
    /// §log: production stops not yet reported, held for [`SUSPEND_GRACE_S`]:
    /// when it happened, its line and its light.
    #[serde(skip)]
    suspends: BTreeMap<(PlayerId, sim::EntityId, sim::StructureKind), (f64, f64, String)>,
    /// §log: production stops that were reported, so their restart is news.
    #[serde(skip)]
    reported_suspends: BTreeSet<(PlayerId, sim::EntityId, sim::StructureKind)>,
}

impl Timeline {
    pub fn new() -> Self {
        Timeline::default()
    }

    /// Record a tick's events as future timeline entries, each at its
    /// player-specific observable time. Runs for ALL players (online or off).
    pub fn ingest(&mut self, events: &[Event], world: &World) {
        // Every notice rides its own straight warp-light wavefront.
        for e in events {
            let first_notice = self.pending.len();
            match &e.payload {
                EventPayload::JumpFailed {
                    owner,
                    fleet,
                    pos,
                    reason,
                } => {
                    if let Some(cc) = world.players.get(owner).map(|corp| corp.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let why = match reason {
                            sim::JumpFailReason::NotAJumpFleet => {
                                "the formation no longer consists entirely of jump-capable hulls"
                            }
                            sim::JumpFailReason::OriginInGravityWell => {
                                "the fleet is inside a gravity well or an interdiction field"
                            }
                            sim::JumpFailReason::TargetInGravityWell => {
                                "the destination is inside a gravity well or an interdiction field"
                            }
                            sim::JumpFailReason::OutOfRange => {
                                "the destination is beyond jump range"
                            }
                        };
                        self.push(
                            *owner,
                            observe,
                            TimelineSeverity::Warn,
                            format!("Jump failed for fleet {fleet}: {why}."),
                        );
                    }
                }
                // A field this player raised, or one of theirs collapsing:
                // owner-only, on the light from the field.
                EventPayload::InterdictionRaised { owner, fleet, pos, radius, expires_at, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|corp| corp.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let minutes = ((expires_at - e.time) / 60.0).round();
                        self.push(*owner, observe, TimelineSeverity::Info, format!(
                            "Fleet {fleet} raised an interdiction field: fleets within {radius:.0} su drop out of warp \
                             and can't jump, for up to {minutes:.0} minutes while it holds station."));
                    }
                }
                EventPayload::InterdictionCollapsed { owner, fleet, pos, end, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|corp| corp.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let why = match end {
                            sim::interdiction::FieldEnd::Expired => "it ran its full time",
                            sim::interdiction::FieldEnd::Left => "the destroyer left the field",
                            sim::interdiction::FieldEnd::Lost => "the destroyer holding it up was lost",
                        };
                        self.push(*owner, observe, TimelineSeverity::Info,
                            format!("Fleet {fleet}'s interdiction field collapsed: {why}."));
                    }
                }
                // Own economy / automation uses the SAME physical origin and
                // wavefront as the live receipt. Otherwise an offline digest
                // could disclose a fill or delivery before the normal UI did.
                EventPayload::Trade(te) => {
                    if let Some((sev, text)) = trade_entry(te, world) {
                        let player = te.player();
                        let Some(cc) = world.players.get(&player).map(|corp| corp.command_center)
                        else {
                            continue;
                        };
                        let origin = e.origin.unwrap_or_else(|| crate::game_loop::trade_report_origin(world, te));
                        let observe = e.time + sim::transit::delay(origin, cc, world.config.c);
                        self.push(player, observe, sev, text);
                    }
                }
                // A battle — each side learns the SAME outcome when its light
                // reaches their command center (so generally at different times).
                EventPayload::RaidResolved {
                    attacker,
                    defender,
                    attacker_ship,
                    target_ship,
                    attacker_kind,
                    target_kind,
                    outcome,
                    pos,
                    ..
                } => {
                    for &p in &[*attacker, *defender] {
                        let Some(corp) = world.players.get(&p) else {
                            continue;
                        };
                        let cc = corp.command_center;
                        let enemy_ship = if p == *attacker { *target_ship } else { *attacker_ship };
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let privateer = (corp.founding.privateer == Some(enemy_ship)).then(|| {
                            world.pirate_family(p).map_or_else(|| "the Rogue Privateer".to_string(), |f| f.nephew_name())
                        });
                        let (sev, text) = raid_entry(
                            p,
                            *attacker,
                            *defender,
                            *attacker_kind,
                            *target_kind,
                            *outcome,
                            privateer,
                        );
                        self.push(p, observe, sev, text);
                    }
                }
                // §TCA Phase 2: a CITATION is a PUBLIC bulletin from the
                // Market Hub. Everyone learns it light-delayed from the hub — the
                // reputational hit rides the same wavefront as the legal one. The
                // culprit reads it as an indictment; everyone else reads it as
                // intelligence about who is worth avoiding (or hiring).
                // §season: the squeeze, owner-only, from the Hub.
                EventPayload::LevyRiseDemanded { owner, points, commodity, trade, due } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(world.hub, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Warn, format!(
                            "Levy rise demanded · +{:.0}% after your {trade:.0} Cr {} trade · answer by {}.",
                            points * 100.0, commodity_name(*commodity), fmt_clock(*due)));
                    }
                }
                EventPayload::LevyAnswered { owner, accepted, by_default, levy, commodity, arrears } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(world.hub, cc, world.config.c);
                        let (sev, text) = if *accepted {
                            (TimelineSeverity::Info, format!("Levy rise {} · levy now {:.0}%.",
                                if *by_default { "accepted by default" } else { "accepted" }, levy * 100.0))
                        } else {
                            (TimelineSeverity::Bad, format!("Levy refused · {} embargoed at the Hub · arrears {arrears:.0} Cr.",
                                commodity_name(*commodity)))
                        };
                        self.push(*owner, observe, sev, text);
                    }
                }
                EventPayload::EmbargoLifted { owner, commodity, arrears } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(world.hub, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Embargo lifted · {} · {arrears:.0} Cr arrears paid.", commodity_name(*commodity)));
                    }
                }
                EventPayload::RequisitionIssued { owner, demand, due, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(world.hub, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Warn, format!(
                            "Requisition · {} to the Hub by {}.", demand_text(demand), fmt_clock(*due)));
                    }
                }
                EventPayload::RequisitionSettled { owner, demand, paid, by_default, enforced, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(world.hub, cc, world.config.c);
                        let (sev, text) = if *paid {
                            (TimelineSeverity::Good, format!("Requisition paid · {}.", demand_text(demand)))
                        } else {
                            (TimelineSeverity::Bad, format!("Requisition {} · {} · standing −{:.0} · levy +1%{}.",
                                if *by_default { "lapsed" } else { "refused" }, demand_text(demand),
                                sim::season::REFUSAL_STANDING, if *enforced { " · enforcement ordered" } else { "" }))
                        };
                        self.push(*owner, observe, sev, text);
                    }
                }
                // §season: a decree reaches every corporation from the Hub.
                EventPayload::Decree { grip, emergency, .. } => {
                    let pos = e.origin.unwrap_or(world.hub);
                    for (&p, corp) in &world.players {
                        let observe = e.time + sim::transit::delay(pos, corp.command_center, world.config.c);
                        self.push(p, observe, TimelineSeverity::Warn,
                            format!("Authority decree · Grip {} · {}{}.", grip.numeral(), grip.title(),
                                if *emergency { " · emergency levy" } else { "" }));
                    }
                }
                // §season phase 3: Resolve, crackdowns and their evidence.
                EventPayload::ResolveMilestone { owner, resolve } => {
                    if world.players.contains_key(owner) {
                        self.push(*owner, e.time, TimelineSeverity::Good, format!(
                            "Resolve {:.0}% · {}.", resolve * 100.0,
                            if *resolve >= 1.0 { "your colonists stand as one" } else { "relics will answer you now" }));
                    }
                }
                EventPayload::CrackdownBegan { kind, system, target, until, evidence_until, .. } => {
                    let pos = e.origin.unwrap_or(world.hub);
                    let place = system.map(|id| format!(" at {}", system_name(world, id))).unwrap_or_default();
                    let named = target.and_then(|t| world.players.get(&t)).map(|c| format!(" · {} named", c.name)).unwrap_or_default();
                    let effect = match kind {
                        sim::season::CrackdownKind::Sabotage => " · Hub fees doubled",
                        sim::season::CrackdownKind::Massacre => " · bounty and enforcement",
                        sim::season::CrackdownKind::Quarantine => " · Authority freight suspended",
                        sim::season::CrackdownKind::RelicSeizure => " · hand over relics",
                        sim::season::CrackdownKind::Keystone => " · enforcement at its builders",
                    };
                    let evidence = if *evidence_until > 0.0 { " · evidence at the scene" } else { "" };
                    for (&p, corp) in &world.players {
                        let observe = e.time + sim::transit::delay(pos, corp.command_center, world.config.c);
                        let sev = if *target == Some(p) { TimelineSeverity::Bad } else { TimelineSeverity::Warn };
                        self.push(p, observe, sev, format!("Crackdown · {}{place}{named}{effect} until {}{evidence}.",
                            kind.title(), fmt_clock(*until)));
                    }
                }
                EventPayload::CrackdownExposed { kind, by, .. } => {
                    let pos = e.origin.unwrap_or(world.hub);
                    let who = world.players.get(by).map_or("A Scout", |c| c.name.as_str()).to_string();
                    for (&p, corp) in &world.players {
                        let observe = e.time + sim::transit::delay(pos, corp.command_center, world.config.c);
                        self.push(p, observe, TimelineSeverity::Good, format!(
                            "False flag exposed · {} · by {who} · crackdown lifted.", kind.title()));
                    }
                }
                // §season phase 4: relics, bearings, sealed worlds, the spy, war.
                EventPayload::RelicWoke { system, pos } => {
                    for (&p, corp) in &world.players {
                        let observe = e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let held = world.systems.iter().any(|s| s.id == *system && s.owner == Some(p));
                        self.push(p, observe, if held { TimelineSeverity::Good } else { TimelineSeverity::Info },
                            format!("Relic awake · {}{}.", system_name(world, *system), if held { " · yours to attune" } else { "" }));
                    }
                }
                EventPayload::BearingDrawn { owner, relic, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let pos = e.origin.unwrap_or(world.hub);
                        let observe = e.time + sim::transit::delay(pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Bearing drawn · {} · a sealed world lies along it.", system_name(world, *relic)));
                    }
                }
                EventPayload::PrecursorFound { owner, system, finder, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let pos = e.origin.unwrap_or(world.hub);
                        let observe = e.time + sim::transit::delay(pos, cc, world.config.c);
                        let by = if finder == owner { String::new() } else {
                            world.players.get(finder).map(|c| format!(" · by {}", c.name)).unwrap_or_default()
                        };
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Sealed world found · {}{by} · its Keystone can close every gate.", system_name(world, *system)));
                    }
                }
                EventPayload::SpyLeak { owner, war_at } => {
                    if world.players.contains_key(owner) {
                        self.push(*owner, e.time, TimelineSeverity::Bad, format!(
                            "An Authority spy · the Authority will know at {} · then war.", fmt_clock(*war_at)));
                    }
                }
                EventPayload::AuthorityWar { owner } => {
                    let pos = e.origin.unwrap_or(world.hub);
                    let who = world.players.get(owner).map_or("A corporation", |c| c.name.as_str()).to_string();
                    for (&p, corp) in &world.players {
                        let observe = e.time + sim::transit::delay(pos, corp.command_center, world.config.c);
                        if p == *owner {
                            self.push(p, observe, TimelineSeverity::Bad,
                                "War with the Authority · no Hub, no Authority freight · bounties on your hulls.".into());
                        } else {
                            self.push(p, observe, TimelineSeverity::Warn, format!("War · the Authority on {who} · bounties on its hulls."));
                        }
                    }
                }
                // §season phase 5: the Keystone, Gate Step and the season's end.
                EventPayload::KeystoneLevel { holder, system, level, fell } => {
                    let pos = e.origin.unwrap_or(world.hub);
                    let who = world.players.get(holder).map_or("A corporation", |c| c.name.as_str()).to_string();
                    for (&p, corp) in &world.players {
                        let observe = e.time + sim::transit::delay(pos, corp.command_center, world.config.c);
                        let sev = if *fell { if p == *holder { TimelineSeverity::Bad } else { TimelineSeverity::Good } }
                            else if p == *holder { TimelineSeverity::Good } else { TimelineSeverity::Warn };
                        self.push(p, observe, sev, format!("Keystone · {} · {} · level {level}/10{}.", system_name(world, *system), who,
                            if *fell { " · knocked down" } else if *level >= sim::season::KEYSTONE_LEVELS { " · the Seal charges, 48h" } else { "" }));
                    }
                }
                EventPayload::KeystoneFed { owner, system, commodity, units } => {
                    if world.players.contains_key(owner) {
                        self.push(*owner, e.time, TimelineSeverity::Info, format!(
                            "Keystone fed · {} · {units} {}.", system_name(world, *system), commodity_name(*commodity)));
                    }
                }
                EventPayload::GateStepped { owner, system, .. } => {
                    if world.players.contains_key(owner) {
                        self.push(*owner, e.time, TimelineSeverity::Info, format!("Gate Step · to {}.", system_name(world, *system)));
                    }
                }
                EventPayload::SeasonEnded { ending } => {
                    let pos = e.origin.unwrap_or(world.hub);
                    let text = match ending {
                        sim::season::Ending::Sovereign { holder, system, .. } => format!("The Seal fires · {} · {} is Sovereign.",
                            system_name(world, *system), world.players.get(holder).map_or("A corporation", |c| c.name.as_str())),
                        sim::season::Ending::CharterHolds { .. } => "The Charter holds · the season is over.".to_string(),
                    };
                    for (&p, corp) in &world.players {
                        let observe = e.time + sim::transit::delay(pos, corp.command_center, world.config.c);
                        self.push(p, observe, TimelineSeverity::Warn, text.clone());
                    }
                }
                EventPayload::EvidenceRead { owner, kind, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let pos = e.origin.unwrap_or(world.hub);
                        let observe = e.time + sim::transit::delay(pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Info, format!(
                            "Evidence read · {} · it was real.", kind.title()));
                    }
                }
                EventPayload::Citation {
                    culprit,
                    offense,
                    pos,
                    occurred_at,
                } => {
                    let who = world
                        .players
                        .get(culprit)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("{culprit}"));
                    let lag = (e.time - *occurred_at).max(0.0);
                    let when = if lag >= 1.0 {
                        format!(" (the offense was {} ago)", fmt_wait(lag))
                    } else {
                        String::new()
                    };
                    for (&p, corp) in &world.players {
                        let observe =
                            e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let (sev, text) = if p == *culprit {
                            (
                                TimelineSeverity::Bad,
                                format!(
                                    "The Terran Charter Authority has CITED your corporation for {}{when}.",
                                    offense.title()
                                ),
                            )
                        } else {
                            (
                                TimelineSeverity::Info,
                                format!(
                                    "Authority bulletin: {who} cited for {}{when}.",
                                    offense.title()
                                ),
                            )
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                // §TCA Phase 2: ENFORCEMENT bulletins, public from the Market Hub
                // on the same light-gating as a citation. The announcement's light
                // outruns the squadron — that IS the target's lead time.
                EventPayload::EnforcementDispatched {
                    target,
                    system,
                    pos,
                } => {
                    let who = world
                        .players
                        .get(target)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("{target}"));
                    let name = system_name(world, *system);
                    for (&p, corp) in &world.players {
                        let observe =
                            e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let (sev, text) = if p == *target {
                            (
                                TimelineSeverity::Bad,
                                format!(
                                    "AUTHORITY ENFORCEMENT DISPATCHED against your corporation — a squadron is \
                                     under way to blockade {name}. Pay reinstatement to call it off, fight it, or wait it out."
                                ),
                            )
                        } else {
                            (
                                TimelineSeverity::Info,
                                format!(
                                    "Authority bulletin: an enforcement squadron sails against {who} at {name}."
                                ),
                            )
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                EventPayload::EnforcementWithdrawn {
                    target,
                    recalled,
                    pos,
                } => {
                    let who = world
                        .players
                        .get(target)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("{target}"));
                    for (&p, corp) in &world.players {
                        let observe =
                            e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let (sev, text) = if p == *target {
                            if *recalled {
                                (TimelineSeverity::Good, "Authority enforcement RECALLED — your charter is back above the proscription line.".to_string())
                            } else {
                                (TimelineSeverity::Info, "The Authority's enforcement squadron has served its time and is standing down.".to_string())
                            }
                        } else {
                            (
                                TimelineSeverity::Info,
                                format!(
                                    "Authority bulletin: the enforcement squadron against {who} has stood down."
                                ),
                            )
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                // The galaxy changed: your own claim is instant; a rival's claim is
                // awareness that arrives light-delayed (same gate as the map).
                EventPayload::SystemClaimed { system, owner, pos } => {
                    let name = system_name(world, *system);
                    for (&p, corp) in &world.players {
                        if p == *owner {
                            self.push(
                                p,
                                e.time,
                                TimelineSeverity::Good,
                                format!("You claimed {name}."),
                            );
                        } else if !world.are_allied(p, *owner) {
                            // §log: rivals only; an ally's claim isn't a warning.
                            let observe = e.time
                                + sim::transit::delay(*pos, corp.command_center, world.config.c);
                            self.push(
                                p,
                                observe,
                                TimelineSeverity::Warn,
                                format!("A rival claimed {name}."),
                            );
                        }
                    }
                }
                // §ladder B4: a Titan dying is HEADLINE news — every corp hears
                // it (light-delayed from the wreck; the owner instantly).
                EventPayload::FlagshipDestroyed {
                    owner, name, pos, ..
                } => {
                    let title = match name {
                        Some(n) => format!("The *{n}* is destroyed."),
                        None => "A Titan is destroyed.".to_string(),
                    };
                    let wreck = *pos;
                    for (&p, corp) in &world.players {
                        if p == *owner {
                            self.push(p, e.time, TimelineSeverity::Bad, format!("{title} Your syndicate's flagship is gone — the yards may lay a new keel."));
                        } else {
                            let observe = e.time
                                + sim::transit::delay(wreck, corp.command_center, world.config.c);
                            self.push(p, observe, TimelineSeverity::Warn, title.clone());
                        }
                    }
                }
                // Queue/start notices are private site news. The common physical-
                // origin gate below applies the return delay, including activation.
                EventPayload::BuildStarted {
                    id,
                    owner,
                    system,
                    what,
                    complete_tick,
                } => {
                    let name = system_name(world, *system);
                    // A refit reuses the build notice; say what actually happens.
                    let refit = world.refit_queue.iter().find(|j| j.id == *id);
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        if let Some(job) = refit {
                            let fit = job.to.modules().iter().map(|m| m.label()).collect::<Vec<_>>().join(", ");
                            format!("Refit started at {name}: {} ← {}.", build_label(*what).trim_start_matches("a "), if fit.is_empty() { "standard fit".into() } else { fit })
                        } else if *complete_tick == u64::MAX && matches!(what, sim::BuildKind::Upgrade { .. } | sim::BuildKind::Founding { .. }) {
                            format!("Construction queued at {name}: {}.", build_label(*what))
                        } else if *complete_tick == u64::MAX {
                            format!("Construction queued at {name}: {} · awaiting workforce.", build_label(*what))
                        } else {
                            format!("Construction started at {name}: {}.", build_label(*what))
                        },
                    );
                }
                // §plunder: a held blockade stripped goods off a colony. BOTH
                // sides hear it on their own clock — the victim watching their
                // stores walk away is half the point of the mechanic.
                EventPayload::SystemPlundered {
                    by,
                    owner,
                    system,
                    commodity,
                    units,
                    pos,
                } => {
                    // A home raid has one summary on break-off, not sixty
                    // per-unit toasts. Inventory still updates on its own light.
                    if by.is_pirate() { continue; }
                    // A unit and a half a second would bury the log: each side
                    // gets one line a minute per system with the haul since.
                    let mut haul = |who: PlayerId| -> Option<BTreeMap<Commodity, u32>> {
                        let entry = self.plunder.entry((who, *system)).or_insert((f64::NEG_INFINITY, BTreeMap::new()));
                        *entry.1.entry(*commodity).or_default() += units;
                        if e.time - entry.0 < PLUNDER_LINE_SECS {
                            return None;
                        }
                        entry.0 = e.time;
                        Some(std::mem::take(&mut entry.1))
                    };
                    let victim_haul = haul(*owner);
                    let prize_haul = haul(*by);
                    self.plunder_lines(world, e.time, (*by, *owner, *system, *pos), victim_haul, prize_haul);
                }
                // §roster: a fleet finished REPAIRS — every hull back to full.
                // Exactly the kind of "ready when you are" news the check-in loop
                // exists for: you left a mauled wing in a yard, you come back and
                // it is whole. Fires once, on completion.
                EventPayload::FleetRepaired { owner, system, .. } => {
                    let name = system_name(world, *system);
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!("Repairs complete at {name} — the fleet is back to full hull."),
                    );
                }
                // §syndicate-projects (R6): members' news of the ladder.
                EventPayload::SyndicateRungFinished { syndicate, week, rung, points } => {
                    if let Some(s) = world.syndicates.get(syndicate) {
                        let text = format!("{}: week {week} rung {rung} done · +{points} points.", s.name);
                        for &member in &s.members {
                            self.push(member, e.time, TimelineSeverity::Good, text.clone());
                        }
                    }
                }
                EventPayload::SyndicateBonusRaised { syndicate, bonus, level } => {
                    if let Some(s) = world.syndicates.get(syndicate) {
                        let what = match bonus {
                            sim::syndicate::SyndicateBonus::Freight => "Freight",
                            sim::syndicate::SyndicateBonus::Research => "Research",
                            sim::syndicate::SyndicateBonus::Migration => "Migration",
                        };
                        let text = format!("{} raised {what} to {}.", s.name, roman(*level as u32));
                        for &member in &s.members {
                            self.push(member, e.time, TimelineSeverity::Good, text.clone());
                        }
                    }
                }
                EventPayload::SyndicateMedalWon { syndicate, week, place } => {
                    if let Some(s) = world.syndicates.get(syndicate) {
                        let medal = match place { 1 => "gold", 2 => "silver", _ => "bronze" };
                        let text = format!("{} took {medal} in week {week}'s projects.", s.name);
                        for &member in &s.members {
                            self.push(member, e.time, TimelineSeverity::Good, text.clone());
                        }
                    }
                }
                // §welcome (R9): a finished hull left the slip.
                EventPayload::ShipLaunched { owner, system, kind, .. } => {
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "Launched {} at {}.", build_label(sim::BuildKind::Ship { ship: *kind }), system_name(world, *system),
                    ));
                }
                EventPayload::SettlementDeveloped { owner, system, project, .. } => {
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "{}: {} complete.", system_name(world, *system),
                        build_label(sim::BuildKind::Founding { project: *project }),
                    ));
                }
                EventPayload::SystemUpgraded {
                    owner,
                    system,
                    upgrade,
                    tier,
                } => {
                    let name = system_name(world, *system);
                    // §economy: one title-driven line for all 16 structure kinds.
                    let what = format!("{} tier {tier}", upgrade.title());
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!("{name} developed — {what}."),
                    );
                }
                // A soft-rejected build (§buildings step 1) — owner-only, instant
                // (your own administration): nothing was spent, the request just
                // couldn't be hosted. Tells the player WHY so the fix is obvious.
                EventPayload::BuildRejected {
                    owner,
                    system,
                    what,
                    reason,
                } => {
                    let name = system_name(world, *system);
                    let text = match reason {
                        sim::BuildRejectReason::FoundingProgramme => format!(
                            "Can't build {} at {name}: complete the Founding Programme before launching a colony ship.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::NoSlot => format!(
                            "Can't build {} at {name}: every development slot is used — systems must specialize.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::NeedsYard { yard, required } => format!(
                            "Can't build {} at {name}: needs {} tier {required} there.",
                            build_label(*what),
                            yard.title()
                        ),
                        sim::BuildRejectReason::NoSlip { slips } => format!(
                            "Can't build {} at {name}: its yard's {slips} slipway{} {} full — raise the yard's tier or wait for a hull to finish.",
                            build_label(*what),
                            if *slips == 1 { "" } else { "s" },
                            if *slips == 1 { "is" } else { "are" }
                        ),
                        sim::BuildRejectReason::NotBuildable => format!(
                            "Can't build {} at {name}: it isn't a corporation-buildable hull.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::NeedsResearch => format!(
                            "Can't build {} at {name}: research required.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::TitanFielded => format!(
                            "Can't build {} at {name}: your syndicate already fields its Titan — one flagship per syndicate (rebuild only after it is lost).",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::OneStationPerSystem => format!(
                            "Can't build {} at {name}: the system's station stands on another world — raise that one instead.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::StarFortressLimit => format!(
                            "Can't raise a Star Fortress at {name}: your corporation already has one.",
                        ),
                    };
                    self.push(*owner, e.time, TimelineSeverity::Warn, text);
                }
                EventPayload::IndustryRejected { owner, system, reason } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn,
                        format!("{}: project refused — {reason}.", system_name(world,*system)));
                }
                // §leave (R4): going on leave, coming back, and refusals.
                EventPayload::LeaveChanged { owner, on, .. } => {
                    self.push(*owner, e.time, TimelineSeverity::Info, if *on {
                        "On leave. Your corporation can't be attacked and gives no orders; automation keeps running.".to_string()
                    } else {
                        "Back from leave. Orders are open again.".to_string()
                    });
                }
                EventPayload::LeaveRefused { owner, reason } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn, format!("Refused: {reason}."));
                }
                // §research-grant (R3).
                EventPayload::ResearchFunded { owner, programme, secs, credits } => {
                    let name = sim::research::programme(programme).map_or(programme.as_str(), |p| p.name);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "Research grant · {:.0} h of {name} · {credits:.0} Cr.", secs / 3_600.0));
                }
                EventPayload::ResearchGrantRefused { owner, reason } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn, format!("Research grant refused: {reason}."));
                }
                // §research-wiring: Salvage Rigs and Boarding Parties, owner-only
                // and light-delayed from the fight.
                EventPayload::WrecksSalvaged { owner, fleet, modules, left, pos } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let haul = modules.iter().map(|(m, n)| format!("{n} {}", m.label())).collect::<Vec<_>>().join(", ");
                        let rest = if *left > 0 { format!(" {left} more left in the wrecks: no crate berths.") } else { String::new() };
                        let text = if modules.is_empty() {
                            format!("Salvage: {left} modules left in the wrecks. Bring a freighter to carry crates.")
                        } else {
                            format!("Salvaged from the wrecks: {haul}, aboard {}.{rest}", fleet_label(world, *fleet))
                        };
                        self.push(*owner, observe, TimelineSeverity::Good, text);
                    }
                }
                EventPayload::PrizeTaken { owner, hulls, pos, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Boarders took {hulls} freighter{}: a prize fleet, battered and low on fuel.", if *hulls == 1 { "" } else { "s" }));
                    }
                }
                // §designations (R2).
                EventPayload::DesignationSet { owner, cap, target } => {
                    let what = match *target {
                        sim::research::DesignationTarget::Body { system, body } => body_name(world, system, body),
                        sim::research::DesignationTarget::SystemPair(a, b) =>
                            format!("{} ⇄ {}", system_name(world, a), system_name(world, b)),
                        sim::research::DesignationTarget::Fleet(id) => fleet_label(world, id),
                    };
                    self.push(*owner, e.time, TimelineSeverity::Good, format!("{} · {what}.", designation_title(*cap)));
                }
                EventPayload::DesignationRefused { owner, reason } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn, format!("Designation refused: {reason}."));
                }
                EventPayload::BuildCancelled { owner, system, what, refunded } => {
                    let goods = refunded.iter().map(|(c, units)| format!("{} {}", units.round(), c.display_name()))
                        .collect::<Vec<_>>().join(", ");
                    self.push(*owner, e.time, TimelineSeverity::Info,
                        format!("Construction cancelled at {}: {}. Returned {goods} to the stockpile.",
                            system_name(world, *system), build_label(*what)));
                }
                // A soft-rejected fleet ORDER — owner-only, whether preflight
                // refused it or receiver-side conditions changed before delivery.
                // The order never installed, the fleet kept its current one, and
                // nothing was spent. Without this line the refusal is invisible
                // and the click just seems to do nothing.
                EventPayload::OrderRejected { owner, fleet, reason, .. } => {
                    let text = match reason {
                        sim::OrderRejectReason::InsideSovereignZone =>
                            "Order refused: the target shelters inside the Authority's sovereign zone at the hub — \
                             no engagement may open there. Your fleet holds its current order."
                                .to_string(),
                        // §upkeep: the fleet is starving. Nothing is lost — it keeps
                        // its guns and its course — but it will not set out again
                        // until Provisions reach it.
                        sim::OrderRejectReason::Unsupplied =>
                            "Order refused: the fleet is out of Provisions and cannot set out. \
                             Ship food to a system near it — it keeps its guns and its current \
                             order, and moves again the moment it is fed."
                                .to_string(),
                        sim::OrderRejectReason::NotAJumpFleet =>
                            "Order refused: only all-Interceptor or all-Scout formations carry jump drives."
                                .to_string(),
                        sim::OrderRejectReason::TargetInGravityWell =>
                            "Order refused: jump destinations must be outside every system and Market Hub gravity well."
                                .to_string(),
                        sim::OrderRejectReason::FounderProtection =>
                            "Order refused: Founder Protection currently prevents combat between corporations. PvE targets remain legal."
                                .to_string(),
                        sim::OrderRejectReason::OnLeave =>
                            "Order refused: that corporation is on leave and can't be attacked.".to_string(),
                        sim::OrderRejectReason::DiplomaticProtection =>
                            "Order refused: a syndicate pact, non-aggression agreement, ceasefire, or separation window protects that target."
                                .to_string(),
                        sim::OrderRejectReason::FormalWarRequired =>
                            "Order refused: blockading territory requires an active war declaration or your corporation's live right of reprisal."
                                .to_string(),
                        sim::OrderRejectReason::DeliveryConditionsChanged =>
                            "Order refused: conditions changed before the signal reached the fleet. Its previous instructions remain in force."
                                .to_string(),
                        sim::OrderRejectReason::IndustryRefused =>
                            "Order refused: that fleet can't do this work where it is (check its hold, fuel and the site). Its previous instructions remain in force."
                                .to_string(),
                        sim::OrderRejectReason::ExplorationThreat =>
                            format!("Exploration aborted: fleet {fleet} detected a nearby threat and is retreating away from the site."),
                        sim::OrderRejectReason::NoInterdictor =>
                            format!("Interdiction refused: fleet {fleet} has no working destroyer carrying an Interdiction Field."),
                        sim::OrderRejectReason::InterdictorInGravityWell =>
                            format!("Interdiction refused: fleet {fleet} is inside a gravity well or another field. Raise fields in open space."),
                        sim::OrderRejectReason::InterdictorRecharging =>
                            format!("Interdiction refused: fleet {fleet}'s field generator is still recharging."),
                        sim::OrderRejectReason::InterdictorLowFuel =>
                            format!("Interdiction refused: fleet {fleet} can't spare the fuel a field burns."),
                        sim::OrderRejectReason::NeedsInterceptor =>
                            format!("Order refused: fleet {fleet} has no interceptor. A blockade needs at least one to \
                                run down what slips past it."),
                        sim::OrderRejectReason::NotRivalTerritory =>
                            "Order refused: only a system another corporation owns can be blockaded.".to_string(),
                        sim::OrderRejectReason::NeedsHold =>
                            format!("Raid refused: fleet {fleet} has no hold. Bring a freighter to carry the loot."),
                        sim::OrderRejectReason::StationAnchored =>
                            format!("Order refused: fleet {fleet} is an anchored station. Unanchor it first (60 s)."),
                        sim::OrderRejectReason::HostilitiesEnded =>
                            format!("Blockade called off: fleet {fleet}'s war with the system's owner is over (a treaty, \
                                a capture, or an expired right of reprisal). It is heading home."),
                    };
                    self.push(*owner, e.time, TimelineSeverity::Warn, text);
                }
                // A colony ship arrived at an already-claimed system (§ships
                // part 3) — you lost the race (or it flipped en route). OWNER-
                // ONLY, light-delayed from the hold position; the ship is intact.
                EventPayload::ColonyHeld { owner, system, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Warn, format!(
                            "Your colony ship arrived at {name} — already claimed. It is holding position, intact; redirect it to another system."
                        ));
                    }
                }
                // A rival fleet took station at a DEFENDED system of yours, or
                // your defense came back under a held blockade: the blockade is
                // CONTESTED — only a battle until the defense falls. The OWNER
                // learns it light-delayed; the BESIEGER instantly (it is there).
                EventPayload::BlockadeContested { by, owner, system, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Warn, if by.is_pirate() {
                            format!("Pirates attacking {name} — your defenses are fighting them.")
                        } else { format!(
                            "{name} is CONTESTED — a rival fleet is fighting your defenses there. Shipping runs while \
                             they hold; if they fall, the system is blockaded. Send warships to tip the fight."
                        ) });
                    }
                    if !by.is_sentinel() { self.push(*by, e.time, TimelineSeverity::Info, format!(
                        "Your fleet is fighting {name}'s defenses. Beat them to blockade the system; if your fleet \
                         retreats, the blockade is off."
                    )); }
                }
                // A rival BLOCKADE took HOLD at one of your systems
                // (§contestable-territory): its defense is beaten. The OWNER
                // learns it light-delayed (news travels home at c); the
                // BESIEGER learns instantly (their fleet is there).
                EventPayload::BlockadeEstablished {
                    by,
                    owner,
                    system,
                    pos,
                } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Bad, if by.is_pirate() {
                            // §log: what a home raid can cost (pirate::HOME_RAID_*).
                            format!("Pirates raiding {name} — they take up to 10% of each good, 60 units in all.")
                        } else { format!(
                            "{name} is under BLOCKADE — its defenses have fallen. Freighters in and out are cut off \
                             and its stores are being stripped. Send warships or build a Defense Platform tier to \
                             contest it."
                        ) });
                    }
                    if !by.is_sentinel() { self.push(*by, e.time, TimelineSeverity::Good, format!(
                        "Your blockade of {name} holds — its defenses are beaten. Its logistics are cut and its \
                         stores are yours while you hold station."
                    )); }
                }
                // A besieged system was CAPTURED (§contestable-territory Part 2).
                // Both participants learn it light-delayed from the flip site: the
                // OLD owner ("you lost X"), the CAPTOR ("you captured X"). Third
                // parties see the ownership change via the light-gated map.
                EventPayload::SystemCaptured {
                    old_owner,
                    new_owner,
                    system,
                    pos,
                    ..
                } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(old_owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*old_owner, observe, TimelineSeverity::Bad, format!(
                            "You LOST {name} — a besieger's marines took the ground. Its stockpile was plundered and its developments damaged; your fleets survive."
                        ));
                    }
                    if let Some(cc) = world.players.get(new_owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*new_owner, observe, TimelineSeverity::Good, format!(
                            "You CAPTURED {name} — your marines hold the ground. You inherit its (damaged) developments and plundered stockpile."
                        ));
                    }
                }
                // §ground G1: the landing force HELD OFF — it is short of the
                // break-even strength, so nothing went in and nothing was lost.
                // The old copy here said the transports were destroyed, which
                // was never true of a hold and is now the whole distinction
                // between this notice and `AssaultRepulsed`.
                EventPayload::AssaultHeld {
                    owner,
                    system,
                    marines,
                    needed,
                    pos,
                } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "A landing force is standing off {name} — {marines} marines against ground that would take about {needed}. Your garrison is why they haven't come down."
                        ));
                    }
                    // The would-be attacker is on station, so they see it at once.
                    if let Some(by) = world
                        .systems
                        .iter()
                        .find(|s| s.id == *system)
                        .and_then(|s| s.blockade.as_ref())
                        .map(|b| b.by)
                    {
                        self.push(by, e.time, TimelineSeverity::Warn, format!(
                            "Your commanders WILL NOT land at {name} — {marines} marines against a break-even of {needed}. Nothing was committed. Bring more transports, or keep bombarding to pin more of the garrison."
                        ));
                    }
                }
                // §ground G1: the drop went in. Both sides watch the same clock
                // from here, and the fight is live — the defender can still
                // change it by breaking the blockade.
                EventPayload::AssaultBegan {
                    attacker,
                    defender,
                    system,
                    marines,
                    defenders,
                    suppression,
                    pos,
                    ..
                } => {
                    let name = system_name(world, *system);
                    let pinned = (suppression * 100.0).round() as u32;
                    if let Some(cc) = world.players.get(defender).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*defender, observe, TimelineSeverity::Bad, format!(
                            "LANDING AT {name} — {marines} marines are on the ground against your {defenders} ({pinned}% of the garrison pinned by bombardment). BREAK THE BLOCKADE and the pinned troops rejoin the fight."
                        ));
                    }
                    self.push(*attacker, e.time, TimelineSeverity::Warn, format!(
                        "Your landing at {name} has gone in — {marines} marines against {defenders}, with {pinned}% of the garrison pinned. Keep the guns on station: if the blockade breaks, they come out of cover."
                    ));
                }
                // §ground G1: the landing was DESTROYED on the ground. This one
                // really does cost the transports.
                EventPayload::AssaultRepulsed {
                    owner,
                    system,
                    landed,
                    held,
                    pos,
                } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Your garrison at {name} DESTROYED a landing of {landed} marines — {held} of your troops still hold the ground. The transports went with them."
                        ));
                    }
                    if let Some(by) = world
                        .systems
                        .iter()
                        .find(|s| s.id == *system)
                        .and_then(|s| s.blockade.as_ref())
                        .map(|b| b.by)
                    {
                        self.push(by, e.time, TimelineSeverity::Bad, format!(
                            "Your landing at {name} was DESTROYED — all {landed} marines and the transports that carried them are lost, against {held} surviving defenders."
                        ));
                    }
                }
                // A blockade at one of your systems lifted (§contestable-territory):
                // the owner learns on the light from the system; the besieger, on
                // its own clock (it was there).
                EventPayload::BlockadeLifted { owner, system, pos, held, by } => {
                    let name = system_name(world, *system);
                    // §log: the last minute's plunder is reported, not dropped.
                    if let Some(by) = by.filter(|b| !b.is_pirate()) {
                        let flush = |plunder: &mut BTreeMap<(PlayerId, sim::EntityId), (f64, BTreeMap<Commodity, u32>)>, who: PlayerId| {
                            plunder.remove(&(who, *system)).map(|(_, goods)| goods).filter(|goods| !goods.is_empty())
                        };
                        let (victim, prize) = (flush(&mut self.plunder, *owner), flush(&mut self.plunder, by));
                        self.plunder_lines(world, e.time, (by, *owner, *system, *pos), victim, prize);
                    }
                    // §log: a pirate raid's end is its own line, with the take
                    // (PirateRaidWithdrawn); "LIFTED" after a robbery read as
                    // good news.
                    let pirates = by.is_some_and(|b| b.is_pirate());
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center).filter(|_| !pirates) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(
                            *owner,
                            observe,
                            TimelineSeverity::Good,
                            if *held {
                                format!("The blockade of {name} has LIFTED — logistics resume.")
                            } else {
                                format!("The attack on {name} was beaten off — the rival fleet has left.")
                            },
                        );
                    }
                    if let Some(by) = by.filter(|b| !b.is_sentinel()) {
                        self.push(by, e.time, TimelineSeverity::Info, format!("Your blockade of {name} is over."));
                    }
                    for who in [Some(*owner), *by].into_iter().flatten() {
                        self.plunder.remove(&(who, *system));
                    }
                }
                // §blockade-running: a freighter crossed a held blockade. Both
                // corporations learn on the light from where it crossed.
                EventPayload::BlockadeRun { owner, by, system, pos, caught, chance, .. } => {
                    let name = system_name(world, *system);
                    let pct = (chance * 100.0).round() as u32;
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        if *caught {
                            self.push(*owner, observe, TimelineSeverity::Bad, format!(
                                "Your freighter was caught running the blockade of {name} ({pct}% chance) — \
                                 the blockaders are raiding it."
                            ));
                        } else {
                            self.push(*owner, observe, TimelineSeverity::Good, format!(
                                "Your freighter slipped past the blockade of {name} ({pct}% chance of being caught)."
                            ));
                        }
                    }
                    self.push(*by, e.time, TimelineSeverity::Info, if *caught {
                        format!("Your blockade of {name} caught a freighter running it — your ships are raiding it.")
                    } else {
                        format!("A freighter slipped past your blockade of {name}.")
                    });
                }
                // §scouting: counter-scouting caught a probe. The system's owner
                // hears who it was; the prober hears only of a block. Both learn
                // it when the light from the scout reaches their command center.
                EventPayload::ScoutContest { by, owner, system, pos, blocked } => {
                    let name = system_name(world, *system);
                    let who = world.players.get(by).map(|c| c.name.clone()).unwrap_or_else(|| format!("{by}"));
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Warn, if *blocked {
                            format!("Scout probe blocked at {name}: {who}.")
                        } else {
                            format!("{name} scouted by {who}.")
                        });
                    }
                    if *blocked && let Some(cc) = world.players.get(by).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*by, observe, TimelineSeverity::Warn, format!(
                            "Probe blocked at {name}: its counter-scouting held. Send more Scouts."
                        ));
                    }
                }
                // A scout captured intel (§scout part 2) — OWNER-ONLY, delivered
                // when the capture's light (from the scout's position) reaches
                // the owner's command center: knowledge travels home at c.
                EventPayload::IntelGathered {
                    owner,
                    system,
                    defense_tier,
                    shipyard_tier,
                    pos,
                } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Info, format!(
                            "Scout report: {name} — Defense ×{defense_tier} · Shipyard ×{shipyard_tier}."
                        ));
                    }
                }
                // §economy Part 4: specialist news — hire/training/delivery on
                // the owner's own clock; a LOSS light-delayed from the wreck
                // (battle-news precedent). All owner-only.
                EventPayload::SpecialistHired { owner, kind, dest } => {
                    let name = system_name(world, *dest);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "Contract signed: a {} ships out from Sol for {name} (sub-light — protect the run).",
                        kind.title()
                    ));
                }
                EventPayload::SpecialistTrained {
                    owner,
                    system,
                    kind,
                } => {
                    let name = system_name(world, *system);
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!("The Academy at {name} graduated a {}.", kind.title()),
                    );
                }
                EventPayload::SpecialistsDelivered {
                    owner,
                    system,
                    manifest,
                } => {
                    let name = system_name(world, *system);
                    let who: Vec<String> = manifest
                        .iter()
                        .map(|(k, n)| format!("{}× {}", n, k.title()))
                        .collect();
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!("Personnel landed at {name}: {}.", who.join(", ")),
                    );
                }
                EventPayload::SpecialistsLost {
                    owner,
                    manifest,
                    pos,
                } => {
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let who: Vec<String> = manifest
                            .iter()
                            .map(|(k, n)| format!("{}× {}", n, k.title()))
                            .collect();
                        self.push(
                            *owner,
                            observe,
                            TimelineSeverity::Bad,
                            format!("Lost with the ship: {}.", who.join(", ")),
                        );
                    }
                }
                // §economy Part 3: a production line STOPPED or RECOVERED —
                // OWNER-ONLY, own clock, transitions only (latched in the sim, so
                // it can never spam). The named cause is the fix-first pointer.
                EventPayload::ProductionSuspended {
                    owner,
                    system,
                    structure,
                    reason,
                } => {
                    let name = system_name(world, *system);
                    use sim::SuspendReason as R;
                    let cause = match reason {
                        R::NoFood => "the colony is out of Provisions — ship food",
                        R::NoInputs => {
                            "its input basket ran dry — ship raws in or staff extraction"
                        }
                        R::StorageFull => {
                            "storage is FULL — ship goods out or build an Orbital Warehouse"
                        }
                        // The sim latches this policy pause quietly; kept for
                        // completeness should an event ever carry it.
                        R::FoodReserve => "the camp's food reserve is full",
                        R::NoSupplies => "it is out of Provisions or Machinery for its upkeep — ship some in",
                    };
                    // §log: held for [`SUSPEND_GRACE_S`]; a stop that clears
                    // sooner (a hauler drawing a full store down) is no news.
                    self.suspends.entry((*owner, *system, *structure)).or_insert((e.time, e.time, format!(
                        "{} at {name} SUSPENDED — {cause} (nothing is lost).",
                        structure.title()
                    )));
                }
                EventPayload::ProductionResumed {
                    owner,
                    system,
                    structure,
                } => {
                    let key = (*owner, *system, *structure);
                    if self.suspends.remove(&key).is_some() || !self.reported_suspends.remove(&key) {
                        continue;
                    }
                    let name = system_name(world, *system);
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!("{} at {name} is producing again.", structure.title()),
                    );
                }
                // §power: a planet's grid fell short, or recovered. Latched in
                // the sim; OWNER-ONLY, own clock.
                EventPayload::Brownout { owner, system, body, power, no_fuel } => {
                    let planet = body_name(world, *system, *body);
                    let fuel = world.systems.iter().find(|s| s.id == *system)
                        .and_then(|s| s.bodies.iter().find(|b| b.id == *body))
                        .and_then(|b| sim::power::plant_fuel(b.tier(sim::StructureKind::PowerPlant)))
                        .unwrap_or(sim::Commodity::Deuterium);
                    let cause = if *no_fuel {
                        format!(" · plant out of {}", if fuel == sim::Commodity::Helium3 { "Helium-3" } else { "Deuterium" })
                    } else { String::new() };
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Warn,
                        format!("Brownout · {planet} · {:.0}%{cause}", power * 100.0),
                    );
                }
                EventPayload::PowerRestored { owner, system, body } => {
                    let planet = body_name(world, *system, *body);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!("Power restored · {planet}"));
                }
                // §economy Part 2: a colony moved on the FOOD LADDER — OWNER-ONLY,
                // on the owner's own clock (own-economy precedent, like stockpiles
                // and FuelShortfall). Transitions only, so it never spams.
                EventPayload::FoodStateChanged {
                    owner,
                    system,
                    state,
                } => {
                    let name = system_name(world, *system);
                    use sim::FoodState as F;
                    let (sev, text) = match state {
                        F::WellSupplied => (
                            TimelineSeverity::Good,
                            format!(
                                "{name} is WELL SUPPLIED again — full workforce, growth resumed."
                            ),
                        ),
                        F::Rationing => (
                            TimelineSeverity::Warn,
                            format!(
                                "{name} is RATIONING — workforce slowed, growth paused. Ship Provisions there (nothing is lost)."
                            ),
                        ),
                        F::Critical => (
                            TimelineSeverity::Warn,
                            format!(
                                "Food CRITICAL at {name} — workforce at half strength. Ship Provisions there (nothing is lost)."
                            ),
                        ),
                        F::NoProvisions => (
                            TimelineSeverity::Bad,
                            format!(
                                "{name} is OUT OF PROVISIONS — industry stalled; the colony endures (nobody dies). Ship Provisions there."
                            ),
                        ),
                    };
                    self.push(*owner, e.time, sev, text);
                }
                // §founding R3: the Authority's one-time replacement starter hull.
                EventPayload::FounderFreighterReplaced { owner, system, .. } => {
                    let name = system_name(world, *system);
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!(
                            "The Terran Charter Authority chartered a replacement Tiny Freighter at {name}. \
                             The privateer withdrew; send the Freighter with your Interceptor as escort."
                        ),
                    );
                }
                // §founding R6/R10: a charter stipend, already stamped at the
                // milestone's arrival on the founder's own clock.
                EventPayload::CharterStipend { owner, title, credits, standing, dossier } => {
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!("Charter milestone · {title} · {}.", service_reward(*credits, *standing, dossier.as_deref())),
                    );
                }
                // §player-styles: the raider hears its raid turned home
                // because the victim has lost a day's limit.
                EventPayload::RaidCapped { by, owner, system, pos } => {
                    if let Some(corp) = world.players.get(by) {
                        let observe = e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let who = world.players.get(owner).map(|c| c.name.clone()).unwrap_or_else(|| format!("{owner}"));
                        self.push(*by, observe, TimelineSeverity::Info, format!(
                            "Raid on {}: {who} has lost a day's limit. Your fleet is coming home.", system_name(world, *system)));
                    }
                }
                // §charter-crates: a charter crate collected at home, on the
                // founder's own clock.
                EventPayload::CharterCrateCollected { owner, charter_crate } => {
                    let goods = charter_crate.goods().iter()
                        .map(|&(c, n)| format!("{} {}", n.round() as u32, c.display_name())).collect::<Vec<_>>().join(", ");
                    self.push(*owner, e.time, TimelineSeverity::Good, format!("Charter crate collected: {goods}."));
                }
                // §merchant: the traveling merchant's visit, owner-only. Its
                // docking is the first the founder hears of it: the client's
                // pop-up and the guide's trader steps start from this line. Its
                // markup stays its own business.
                EventPayload::MerchantArrived { owner, system } => {
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "A free trader docked at {}.", system_name(world, *system)));
                }
                // §contract-board: posted at the hub; the news comes with the offers.
                EventPayload::ContractBoardPosted { owner, count, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        self.push(*owner, e.time + sim::transit::delay(world.hub, cc, world.config.c),
                            TimelineSeverity::Info, format!("Contract board · {count} new contracts."));
                    }
                }
                EventPayload::ContractBoardBonus { owner, credits, standing, dossier, .. } => {
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "Contract board · {} done · bonus {}.", sim::operation::BOARD_BONUS_AFTER,
                        service_reward(*credits, *standing, dossier.as_deref())));
                }
                // §signals: picked up by the Command Center itself.
                EventPayload::SignalDetected { owner, kind, hazardous, .. } => {
                    self.push(*owner, e.time, TimelineSeverity::Info, format!(
                        "New signal · {}{}.", kind.name_for(world.free_trader(*owner)), if *hazardous { " · Hazardous" } else { "" }));
                }
                // §signals: already stamped on arrival at command.
                EventPayload::SignalReported { owner, number, kind, captain, xp, hurt, finds, .. } => {
                    let mut parts: Vec<String> = finds.iter().map(signal_find_text).collect();
                    parts.push(format!("+{xp} XP"));
                    if *hurt { parts.push(format!("{captain} hurt")); }
                    let head = if (*number as usize) <= sim::signals::FIRST_TEN {
                        format!("First find {number}/{}", sim::signals::FIRST_TEN)
                    } else { "Report in".into() };
                    self.push(*owner, e.time, TimelineSeverity::Good,
                        format!("{head} · {} · {}.", kind.name_for(world.free_trader(*owner)), parts.join(" · ")));
                }
                // §medals: awarded at the hub's ledger close; the news reaches
                // command with the published table.
                EventPayload::MedalWon { owner, week, category, place } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let medal = match place { 1 => "Gold", 2 => "Silver", _ => "Bronze" };
                        self.push(*owner, e.time + sim::transit::delay(world.hub, cc, world.config.c),
                            TimelineSeverity::Good, format!("Week {week} · {medal} medal · {}.", category.label()));
                    }
                }
                // §charter-v5: the privateer's raid, on its light home. The
                // client turns this line into the raid's story pop-up.
                EventPayload::FoundingRaided { owner, system, pos, stolen } => {
                    if let Some(corp) = world.players.get(owner) {
                        let observe = e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let goods: Vec<String> = stolen.iter().filter(|(_, n)| **n > 0)
                            .map(|(c, n)| format!("{n} {}", commodity_name(*c))).collect();
                        let took = match goods.split_last() {
                            None => "nothing".to_string(),
                            Some((last, [])) => last.clone(),
                            Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
                        };
                        self.push(*owner, observe, TimelineSeverity::Bad,
                            format!("Pirates raided {} and took {took}.", system_name(world, *system)));
                    }
                }
                // §vendetta: the pirate family's story, on its light home.
                // The client turns these lines into transmissions.
                EventPayload::Vendetta { owner, beat, pos, stolen, wreck } => {
                    if let Some(corp) = world.players.get(owner)
                        && let Some(family) = world.pirate_family(*owner)
                    {
                        let observe = e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let (nephew, uncle) = (family.nephew_name(), family.uncle_name());
                        let colony = corp.home_system.map_or_else(|| "your colony".to_string(), |h| system_name(world, h));
                        let goods: Vec<String> = stolen.iter().filter(|(_, n)| **n > 0)
                            .map(|(c, n)| format!("{n} {}", commodity_name(*c))).collect();
                        let took = match goods.split_last() {
                            None => "nothing".to_string(),
                            Some((last, [])) => last.clone(),
                            Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
                        };
                        use sim::founding::VendettaBeat as B;
                        let (severity, text) = match beat {
                            B::Kill => (TimelineSeverity::Good, format!("{nephew}'s last words named his uncle, {uncle}.")),
                            B::Den => (TimelineSeverity::Info, format!("Among the den's stores: a message from {uncle} to {nephew}.")),
                            B::Sworn => (TimelineSeverity::Bad, format!("{uncle} swore revenge for {nephew}.")),
                            B::Arrival => (TimelineSeverity::Bad, format!("{uncle} is over {colony}.")),
                            B::Won => (TimelineSeverity::Good, format!("{uncle} is dead.")),
                            // §gun-platform: repelled, he got away.
                            B::Lost if goods.is_empty() => (TimelineSeverity::Info, format!("{uncle} got away.")),
                            B::Lost => (TimelineSeverity::Bad, format!("{uncle} got away with {took}.")),
                            B::Fallen => (TimelineSeverity::Bad, format!("{uncle} sacked {colony}.")),
                            // §vendetta-campaign: his strikes on the expanded mining.
                            B::MineStrike => {
                                let world_name = corp.founding.vendetta.as_ref()
                                    .and_then(|v| v.strikes.iter().find(|s| s.kind == sim::founding::StrikeKind::Mine))
                                    .and_then(|s| world.systems.iter().find(|x| x.id == s.system)
                                        .and_then(|x| x.bodies.iter().find(|b| b.id == s.body)).map(|b| b.name.clone()))
                                    .unwrap_or_else(|| "your second mine".to_string());
                                (TimelineSeverity::Bad, format!("{uncle}'s raider is over {world_name}."))
                            }
                            B::BargeStrike => (TimelineSeverity::Bad, format!("{uncle}'s raiders are on your Mining Barge.")),
                            // §gun-platform: the first strike hunts a freighter, or wrecks the second world.
                            B::FreighterStrike => {
                                let hull = corp.founding.vendetta.as_ref()
                                    .and_then(|v| v.strikes.iter().find_map(|s| s.target))
                                    .map_or_else(|| "freighter".to_string(), |t| fleet_label(world, t));
                                (TimelineSeverity::Bad, format!("{uncle}'s raider is hunting your {hull}."))
                            }
                            B::Wrecked => {
                                let Some(w) = wreck else { continue };
                                (TimelineSeverity::Bad, format!("{uncle}'s raider wrecked the {} on {}.",
                                    w.structure.tier_title(1), body_name(world, w.system, w.body)))
                            }
                        };
                        self.push(*owner, observe, severity, format!("Vendetta · {text}"));
                    }
                }
                EventPayload::MerchantTraded { owner, bought, sold, paid, received, .. } => {
                    let goods = |m: &BTreeMap<Commodity, u32>| m.iter()
                        .map(|(c, n)| format!("{n} {}", c.display_name())).collect::<Vec<_>>().join(", ");
                    let mut parts = Vec::new();
                    if !bought.is_empty() { parts.push(format!("bought {} for {paid:.0} Cr", goods(bought))); }
                    if !sold.is_empty() { parts.push(format!("sold {} for {received:.0} Cr", goods(sold))); }
                    self.push(*owner, e.time, TimelineSeverity::Info,
                        format!("Free trader: {}.", parts.join("; ")));
                }
                EventPayload::MerchantTradeRefused { owner, reason, .. } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn,
                        format!("Free trader: trade refused — {reason}."));
                }
                // §treasury: refused at home as it was given.
                EventPayload::PaymentRefused { owner, what, cost } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn,
                        format!("Not enough credits for {what}: {cost:.0} Cr."));
                }
                // §traders: the corporation's own free trader (sim merchant.rs
                // names it; client trader.ts gives it a face and a voice).
                EventPayload::MerchantDeparted { owner, system } => {
                    let trader = world.free_trader(*owner);
                    self.push(*owner, e.time, TimelineSeverity::Info,
                        format!("{}'s {} left {}.", trader.name, trader.ship, system_name(world, *system)));
                }
                // §syndicates Part 3: an ally GARRISON's supply flipped at a HOST
                // system — OWNER-ONLY (the sender, whose fleet it is), light-delayed
                // from the distant host to their command center (NOT own-economy —
                // the garrison is far away). Transitions only, so no spam.
                EventPayload::GarrisonSupplyChanged { owner, host, fed } => {
                    if let (Some(cc), Some(hpos)) = (
                        world.players.get(owner).map(|c| c.command_center),
                        world.systems.iter().find(|s| s.id == *host).map(|s| s.pos),
                    ) {
                        let name = system_name(world, *host);
                        let observe = e.time + sim::transit::delay(hpos, cc, world.config.c);
                        let (sev, text) = if *fed {
                            (
                                TimelineSeverity::Good,
                                format!(
                                    "Your garrison at ally {name} is fed again — back on defense."
                                ),
                            )
                        } else {
                            (
                                TimelineSeverity::Warn,
                                format!(
                                    "Your garrison at ally {name} is UNFED — its defense is suspended until the host has Provisions (nothing is lost)."
                                ),
                            )
                        };
                        self.push(*owner, observe, sev, text);
                    }
                }
                EventPayload::PirateRaidInbound { owner, system, fleet, ships, pos } => {
                    if let Some(corp) = world.players.get(owner) {
                        let observe = e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        // §log: when it lands, as seen from the warning's arrival.
                        let eta = raid_eta(world, *fleet)
                            .map(|at| format!(" · ETA ~{} min", ((at - observe).max(60.0) / 60.0).round()))
                            .unwrap_or_default();
                        self.push(*owner, observe, TimelineSeverity::Bad, format!(
                            "Pirate raid inbound · {} · {ships} raider{}{eta}.",
                            system_name(world, *system), if *ships == 1 { "" } else { "s" }));
                    }
                }
                // §player-styles: a rival raid, once its victim's sensors see it.
                EventPayload::RaidSighted { owner, by, system, ships, pos, eta, .. } => {
                    if let Some(corp) = world.players.get(owner) {
                        let observe = e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let who = world.players.get(by).map(|c| c.name.clone()).unwrap_or_else(|| format!("{by}"));
                        let eta = eta.map(|at| format!(" · ETA ~{} min", ((at - observe).max(60.0) / 60.0).round()))
                            .unwrap_or_default();
                        self.push(*owner, observe, TimelineSeverity::Bad, format!(
                            "Raid inbound · {who} · {ships} ship{} → {}{eta}.", if *ships == 1 { "" } else { "s" },
                            system_name(world, *system)));
                    }
                }
                // §player-styles: a pirate crew set up near home.
                EventPayload::PirateHideoutSpotted { owner, system, .. } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn, format!(
                        "Pirate hideout · {}. Clear it like the den; its loot is yours to haul home.", system_name(world, *system)));
                }
                EventPayload::RaidRepeatStopped { owner, system, reason, .. } => {
                    let why = match reason {
                        sim::event::RepeatStop::Empty => "the last trip came home empty",
                        sim::event::RepeatStop::LostShips => "the last trip lost a ship",
                        sim::event::RepeatStop::Capped => "they've lost a day's limit",
                        sim::event::RepeatStop::Refused => "it can't be raided now",
                        sim::event::RepeatStop::NoFuel => "home has no fuel for the trip",
                        sim::event::RepeatStop::StoresFull => "home storage is full",
                    };
                    self.push(*owner, e.time, TimelineSeverity::Info, format!(
                        "Raids on {} stopped: {why}.", system_name(world, *system)));
                }
                EventPayload::PirateRaidWithdrawn { owner, system, pos, plunder, .. } => {
                    if let Some(corp) = world.players.get(owner) {
                        let observe = e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let goods = plunder.iter().map(|(c, n)| format!("{n} {}", commodity_name(*c)))
                            .collect::<Vec<_>>().join(", ");
                        let name = system_name(world, *system);
                        // §log: impact first; their haul is physical cargo.
                        if goods.is_empty() {
                            self.push(*owner, observe, TimelineSeverity::Good, format!("Pirates left {name} empty-handed."));
                        } else {
                            self.push(*owner, observe, TimelineSeverity::Bad, format!(
                                "Raided {name}: pirates took {goods}. Their haul can still be intercepted."));
                        }
                    }
                }
                // §pirates: a player DESTROYED a pirate enclave — OWNER-ONLY (the
                // victor), light-delayed from the base to their command center.
                EventPayload::PirateEnclaveCleared {
                    owner,
                    system,
                    pos,
                    plunder,
                    personal,
                } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let name = system_name(world, *system);
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let loot: u32 = plunder.values().sum();
                        let tail = if loot > 0 {
                            format!(" {loot} units await Freighter recovery on-site.")
                        } else {
                            String::new()
                        };
                        let den = world.enclaves.get(system).is_some_and(|e| e.founder_den);
                        let aftermath = if *personal {
                            // §player-styles: no crew comes back to it.
                            String::new()
                        } else if world.enclaves.get(system).is_some_and(|e| e.permanent()) {
                            "Site secured for settlement.".to_string()
                        } else {
                            format!("Raids from this source paused for {}m.", (sim::pirate::PIRATE_DORMANCY / 60.0).round())
                        };
                        // §charter-v4: the founder's den guarded the local belt;
                        // §charter-v5: the guide makes surveying it a long-term goal.
                        // §vendetta: the den is the pirate family's.
                        let den_name = world.pirate_family(*owner)
                            .map_or_else(|| "Privateer den".to_string(), |f| format!("The {} den", f.surname()));
                        self.push(*owner, observe, TimelineSeverity::Good, if den {
                            format!("{den_name} at {name} destroyed. Its asteroid belt looks rich in minerals.{tail}")
                        } else {
                            format!("Pirate hideout at {name} cleared.{}{tail}",
                                if aftermath.is_empty() { String::new() } else { format!(" {aftermath}") })
                        });
                    }
                }
                // §node: an EXOTIC system AWAKENED — announced GALAXY-WIDE,
                // light-delayed from the node to each observer's command center (the
                // awakening TIME is public, but news of the event still travels at
                // straight warp light). Same delivery as a rival claim.
                EventPayload::NodeAwakened { system, pos, bonus } => {
                    let name = system_name(world, *system);
                    for (&p, corp) in &world.players {
                        let observe =
                            e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        self.push(
                            p,
                            observe,
                            TimelineSeverity::Info,
                            format!(
                                "EXOTIC NODE AWAKENED at {name} — a {} node is now capturable.",
                                bonus.title()
                            ),
                        );
                    }
                }
                // §node EXPOSURE: a node's HOLDER changed — announced GALAXY-WIDE,
                // light-delayed, so every corp learns who now commands it. The holder
                // hears "you now command…"; everyone else hears who took it.
                EventPayload::NodeCaptured {
                    owner,
                    system,
                    pos,
                    bonus,
                } => {
                    let name = system_name(world, *system);
                    let holder = world
                        .players
                        .get(owner)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "A rival".into());
                    for (&p, corp) in &world.players {
                        let observe =
                            e.time + sim::transit::delay(*pos, corp.command_center, world.config.c);
                        let (sev, text) = if p == *owner {
                            (
                                TimelineSeverity::Good,
                                format!("You now command the {} node at {name}.", bonus.title()),
                            )
                        } else {
                            (
                                TimelineSeverity::Warn,
                                format!(
                                    "{holder} now commands the {} node at {name}.",
                                    bonus.title()
                                ),
                            )
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                // §explore Part 2: a SURVEY completed — OWNER-ONLY, light-delayed
                // from the fleet's position (knowledge travels home at c; the sim
                // inserts into `surveyed` on the same clock, so the notice and the
                // map's new geology land together).
                EventPayload::SurveyCompleted { owner, system, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let belts = belt_discovery(world, *system)
                            .map(|found| format!(" {found}"))
                            .unwrap_or_default();
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Survey of {name} complete — its worlds and geology are charted (permanent; allies receive a relayed copy).{belts}"
                        ));
                    }
                }
                // §pockets: a prospecting pass or a barge's strike — OWNER-ONLY,
                // light-delayed from the fleet, on the clock the find itself
                // reaches command (the pocket rows appear with the notice).
                EventPayload::ProspectCompleted { owner, system, body, found, pos } => {
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let belt = body_name(world, *system, *body);
                        let (sev, text) = if found.is_empty() {
                            (TimelineSeverity::Info, format!("Prospected {belt}: nothing new found."))
                        } else {
                            let ores: Vec<&str> = found.iter().map(|c| commodity_name(*c)).collect();
                            let plural = if found.len() > 1 { "pockets" } else { "pocket" };
                            (TimelineSeverity::Good, format!("Prospected {belt}: {} {plural} found.", ores.join(" and ")))
                        };
                        self.push(*owner, observe, sev, text);
                    }
                }
                // §belts: a station's anchoring and loading — owner-only,
                // light-delayed from the station.
                EventPayload::StationAnchored { owner, system, body, pos, .. }
                | EventPayload::StationUnanchored { owner, system, body, pos, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let belt = body_name(world, *system, *body);
                        let (sev, text) = if matches!(e.payload, EventPayload::StationAnchored { .. }) {
                            (TimelineSeverity::Good, format!("Station anchored at {belt}. Your barges there unload into it."))
                        } else {
                            (TimelineSeverity::Info, format!("Station unanchored at {belt}; free to move."))
                        };
                        self.push(*owner, observe, sev, text);
                    }
                }
                EventPayload::StationReleased { owner, system, units, pos, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let name = system_name(world, *system);
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "{name} is your colony: the station unloaded {units} units into its stockpile and is free to move."
                        ));
                    }
                }
                EventPayload::StationLoaded { owner, commodity, units, pos, .. } => {
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        self.push(*owner, observe, TimelineSeverity::Info, format!(
                            "Loaded {units} {} from your station.", commodity_name(*commodity)
                        ));
                    }
                }
                EventPayload::PocketStruck { owner, system, body, ore, pos } => {
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let belt = body_name(world, *system, *body);
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Barge struck a {} pocket in {belt}.", commodity_name(*ore)
                        ));
                    }
                }
                // §explore Part 3: the system's HIDDEN TRAIT revealed to its (new)
                // owner — the blind claimer's gamble resolving (or capture spoils).
                // OWNER-ONLY, light-delayed from the system.
                EventPayload::TraitRevealed {
                    owner,
                    system,
                    pos,
                    trait_,
                } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let what = match trait_ {
                            sim::explore::SystemTrait::BonusVein { commodity } => {
                                format!(
                                    "Bonus Vein — its {} deposit gains ×{} natural yield (within the ×{} site cap)",
                                    commodity.slug(),
                                    sim::explore::BONUS_VEIN_MULT,
                                    sim::explore::NATURAL_SPECIALTY_CAP,
                                )
                            }
                            sim::explore::SystemTrait::DeepDeposits => format!(
                                "Deep Deposits — natural yield gains ×{} (within the ×{} site cap), but the FIRST Extractor tier is wasted breaking through",
                                sim::explore::DEEP_DEPOSITS_BASE_MULT,
                                sim::explore::NATURAL_SPECIALTY_CAP,
                            ),
                            sim::explore::SystemTrait::UnstableGeology => format!(
                                "Unstable Geology — development costs ×{} here",
                                sim::explore::UNSTABLE_COST_MULT
                            ),
                            sim::explore::SystemTrait::VolatilePockets => format!(
                                "Volatile Pockets — Refinery output ×{} here",
                                sim::explore::VOLATILE_REFINERY_MULT
                            ),
                            sim::explore::SystemTrait::PrecursorCache => format!(
                                "Precursor Cache — a one-time {} Alloys deposited to the stockpile",
                                sim::explore::PRECURSOR_ALLOYS
                            ),
                        };
                        let sev = if matches!(trait_, sim::explore::SystemTrait::UnstableGeology) {
                            TimelineSeverity::Warn
                        } else {
                            TimelineSeverity::Good
                        };
                        self.push(
                            *owner,
                            observe,
                            sev,
                            format!("TRAIT REVEALED at {name}: {what}."),
                        );
                    }
                }
                // §node: a held node's upkeep flipped — OWNER-ONLY, on the owner's own
                // clock (own-economy precedent, like HabitatSupplyChanged). Transitions
                // only, so it never spams.
                EventPayload::NodeSupplyChanged { owner, system, fed } => {
                    let name = system_name(world, *system);
                    let (sev, text) = if *fed {
                        (
                            TimelineSeverity::Good,
                            format!("Node at {name} is fed again — its bonus is live."),
                        )
                    } else {
                        (
                            TimelineSeverity::Warn,
                            format!(
                                "Node at {name} is UNFED — its bonus is SUSPENDED. Ship its upkeep there (nothing is lost)."
                            ),
                        )
                    };
                    self.push(*owner, e.time, sev, text);
                }
                // A Defense Platform fought (§buildings step 2c) — OWNER-ONLY
                // detail (tiers lost / result), light-delayed from the battle
                // like any combat news. The attacker's side of the story arrives
                // separately via the ordinary RaidResolved report.
                // §orbital-defense: a hardpoint's new fit, reported from the
                // system (the light-delay floor below applies).
                EventPayload::StationRefitted { owner, system, mount, fit } => {
                    let name = system_name(world, *system);
                    let what = if fit.is_empty() {
                        "the stock beam battery".to_string()
                    } else {
                        fit.split('+').map(|m| m.replace('_', " ")).collect::<Vec<_>>().join(", ")
                    };
                    self.push(*owner, e.time, TimelineSeverity::Info,
                        format!("Hardpoint {} of the station at {name} now fields {what}.", mount + 1));
                }
                // §orbital-defense: the owner hears how its station fared, on
                // the light from the battle.
                EventPayload::StationEngaged { owner, system, pos, level, knocked_out, standing, enemy_destroyed, driven_off } => {
                    let name = system_name(world, *system);
                    let station = sim::station::level_name(*level);
                    let total = sim::station::level(*level).map_or(0, |l| l.hardpoints);
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let lost = if *knocked_out > 0 {
                            format!(" — {knocked_out} of {total} hardpoints knocked out; they repair while the system is free")
                        } else {
                            String::new()
                        };
                        let (sev, text) = if *standing == 0 {
                            (TimelineSeverity::Bad, format!(
                                "Your {station} at {name} was silenced: every hardpoint is out. A blockade can hold the system until they repair."))
                        } else if *enemy_destroyed {
                            (TimelineSeverity::Good, format!("Your {station} at {name} destroyed the attackers{lost}."))
                        } else if *driven_off {
                            (TimelineSeverity::Good, format!("Your {station} at {name} drove the attackers off{lost}."))
                        } else {
                            (TimelineSeverity::Warn, format!("Your {station} at {name} fought{lost}."))
                        };
                        self.push(*owner, observe, sev, text);
                    }
                }
                EventPayload::PlatformEngaged {
                    owner,
                    system,
                    pos,
                    raider_destroyed,
                    driven_off,
                    tiers_lost,
                } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + sim::transit::delay(*pos, cc, world.config.c);
                        let result = if *raider_destroyed {
                            "destroyed the raider"
                        } else if *driven_off {
                            "drove the raider off"
                        } else {
                            "was fought through"
                        };
                        let damage = if *tiers_lost > 0 {
                            format!(" — {tiers_lost} platform tier(s) lost")
                        } else {
                            String::new()
                        };
                        let sev = if *raider_destroyed || *driven_off {
                            TimelineSeverity::Good
                        } else {
                            TimelineSeverity::Bad
                        };
                        self.push(*owner, observe, sev, format!("Defense Platform at {name} engaged a hostile raider and {result}{damage}."));
                    }
                }
                EventPayload::FuelShortfall {
                    owner,
                    needed,
                    kind,
                    warning,
                } => {
                    let what = if *warning {
                        // §log: the order went ahead; it may still run dry.
                        format!("This {} may run out of fuel on the way ({needed:.0} needed); it went ahead.", kind.label())
                    } else if *kind == sim::fuel::ShortfallKind::Jump {
                        format!("A jump was cancelled — too little Antimatter (needed {needed:.1}). Refill the bottle at a berth, the Hub or from a tender, then jump again.")
                    } else {
                        format!("A {} was held — out of fuel. Refuel at a berth or call Authority Astral Assistance from the fleet panel.", kind.label())
                    };
                    self.push(*owner, e.time, TimelineSeverity::Warn, what);
                }
                // §fuel-overhaul: a fleet ran one store dry and switched to the other.
                EventPayload::FuelSwitched { owner, fleet, to } => {
                    let line = if *to == sim::ship::TransitMode::Slow {
                        format!("Fleet {} out of Deuterium · Slow on Uranium", fleet.0)
                    } else {
                        format!("Fleet {} out of Uranium · Full on Deuterium", fleet.0)
                    };
                    self.push(*owner, e.time, TimelineSeverity::Warn, line);
                }
                EventPayload::FuelRescueDispatched {
                    owner,
                    fuel,
                    cost,
                    ..
                } => {
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Info,
                        format!(
                            "Authority Astral Assistance dispatched a rescue tender with ~{fuel:.0} fuel — {cost:.0} credits charged."
                        ),
                    );
                }
                EventPayload::FuelRescueCompleted {
                    owner, fuel, pos, ..
                } => {
                    let observe = world
                        .players
                        .get(owner)
                        .map_or(e.time, |corp| e.time + sim::transit::delay(*pos, corp.command_center, world.config.c));
                    self.push(
                        *owner,
                        observe,
                        TimelineSeverity::Good,
                        format!(
                            "AAA rescue complete — ~{fuel:.0} fuel transferred; the fleet resumed its held order."
                        ),
                    );
                }
                EventPayload::FuelRescueFailed { owner, pos, .. } => {
                    let observe = world
                        .players
                        .get(owner)
                        .map_or(e.time, |corp| e.time + sim::transit::delay(*pos, corp.command_center, world.config.c));
                    self.push(
                        *owner,
                        observe,
                        TimelineSeverity::Bad,
                        "AAA rescue failed before service; the dispatch charge was not refunded.".to_string(),
                    );
                }
                EventPayload::FuelRescueRejected { owner, reason, .. } => {
                    let why = match reason {
                        sim::RescueRejectReason::FleetUnavailable => "that fleet is unavailable".to_string(),
                        sim::RescueRejectReason::NotStranded => "the latest Authority check says the fleet is not fuel-stalled".to_string(),
                        sim::RescueRejectReason::AlreadyDispatched => "a rescue tender is already outbound".to_string(),
                        sim::RescueRejectReason::CannotAfford { needed, have } => {
                            format!("the callout costs {needed:.0} credits; your treasury holds {have:.0}")
                        }
                    };
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Warn,
                        format!("AAA callout refused — {why}."),
                    );
                }
                // Execution is an internal event. Only served compliance light
                // can turn an order estimate into a confirmed notification.
                EventPayload::OrderDelivered { .. } => {}
                // §log: a confirmed order is the fleet panel's and a toast's
                // news; one journal line per order buried everything else.
                EventPayload::OrderConfirmed { .. } => {}
                // §research: corporation-owned institution news. It is private
                // administration, so only its owner receives it and no alliance
                // membership can disclose or duplicate it.
                EventPayload::ResearchCompleted { owner, programme } => {
                    let name = sim::research::programme(programme)
                        .map(|p| p.name)
                        .unwrap_or("a programme");
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        format!("Research complete: {name} — its effect is live corporation-wide."),
                    );
                }
                EventPayload::TierUnlocked {
                    owner,
                    field,
                    school,
                    tier,
                } => {
                    let where_ = match school {
                        Some(s) => format!("{} · {}", field.title(), s.title()),
                        None => field.title().to_string(),
                    };
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Info,
                        format!("Tier {tier} unlocked on {where_}."),
                    );
                }
                EventPayload::ResearchStalled { owner } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn, "Research stalled — no staffed Academy is contributing. Post crew to an Academy to resume.".to_string());
                }
                EventPayload::ResearchResumed { owner } => {
                    self.push(
                        *owner,
                        e.time,
                        TimelineSeverity::Good,
                        "Research resumed — a staffed Academy is contributing again."
                            .to_string(),
                    );
                }
                EventPayload::OperationUpdated {
                    operation,
                    recipient,
                    state,
                    progress,
                    arrive_at,
                    ..
                } => {
                    if *state == sim::OperationState::Active && *progress > 0 {
                        continue; // progress lives on the board; don't flood the check-in journal
                    }
                    // §contract-board: one line for the day's board, none per
                    // offer, and none as untaken offers lapse.
                    if matches!(state, sim::OperationState::Offered | sim::OperationState::Expired)
                        && world.operations.get(operation).is_some_and(|o| o.board_day.is_some())
                    {
                        continue;
                    }
                    // §log: "operation completed" is news for the corporation
                    // that won it, not for every one that was offered it.
                    let op = world.operations.get(operation);
                    if *state == sim::OperationState::Completed
                        && op.and_then(|o| o.winner).is_some_and(|winner| winner != *recipient)
                    {
                        continue;
                    }
                    let (phase, severity) = match state {
                        sim::OperationState::Offered => (LogPhase::Offered, TimelineSeverity::Info),
                        sim::OperationState::Active => (LogPhase::Active, TimelineSeverity::Info),
                        sim::OperationState::Completed => (LogPhase::Completed, TimelineSeverity::Good),
                        sim::OperationState::Failed => (LogPhase::Failed, TimelineSeverity::Warn),
                        sim::OperationState::Expired => (LogPhase::Expired, TimelineSeverity::Warn),
                        sim::OperationState::Abandoned => continue,
                    };
                    let refs = LogRefs { operation: Some(*operation), ..Default::default() };
                    // Immutable terms were frozen at this stage's event. A
                    // counter-raid keeps its own titled line: it names the next
                    // step. The rest merge with the same tick's like lines.
                    if let Some(title) = op.filter(|o| o.counter_raid.is_some())
                        .and_then(|o| o.briefing.as_ref()).map(|b| &b.title)
                    {
                        let what = match state {
                            sim::OperationState::Offered => "available",
                            sim::OperationState::Active => "active",
                            sim::OperationState::Completed => "completed",
                            sim::OperationState::Failed => "failed",
                            _ => "expired",
                        };
                        self.push_meta(*recipient, *arrive_at, severity, format!("{title} · {what}."), Meta {
                            kind: Some(LogKind::Operation), group: Some(format!("operation:{operation}")),
                            phase: Some(phase), refs, data: None, merge: None,
                        });
                    } else {
                        self.push_merged(*recipient, *arrive_at, severity, phase, operation.to_string(), refs);
                    }
                }
                EventPayload::DiplomacyUpdated {
                    recipient,
                    other,
                    state,
                    kind,
                    arrive_at,
                    ..
                } => {
                    let who = world
                        .players
                        .get(other)
                        .map(|corp| corp.name.clone())
                        .unwrap_or_else(|| format!("{other}"));
                    let (severity, text) = match kind {
                        sim::diplomacy::DiplomacyNoticeKind::Proposal => (
                            TimelineSeverity::Info,
                            format!("Diplomatic proposal received from {who}."),
                        ),
                        sim::diplomacy::DiplomacyNoticeKind::Accepted => (
                            TimelineSeverity::Good,
                            format!("{who} accepted the proposed {} agreement.", relation_label(*state)),
                        ),
                        sim::diplomacy::DiplomacyNoticeKind::Rejected => (
                            TimelineSeverity::Warn,
                            format!("{who} rejected the diplomatic proposal."),
                        ),
                        sim::diplomacy::DiplomacyNoticeKind::Declaration => (
                            TimelineSeverity::Bad,
                            format!(
                                "WAR DECLARATION received from {who}; the warning period has begun."
                            ),
                        ),
                        sim::diplomacy::DiplomacyNoticeKind::Activated => (
                            TimelineSeverity::Bad,
                            format!("War with {who} is now active."),
                        ),
                        sim::diplomacy::DiplomacyNoticeKind::Cancelled => (
                            TimelineSeverity::Warn,
                            format!("{who} has given notice to end the agreement."),
                        ),
                        sim::diplomacy::DiplomacyNoticeKind::Separation => (
                            TimelineSeverity::Info,
                            format!("Separation protection with {who} is active."),
                        ),
                        sim::diplomacy::DiplomacyNoticeKind::Reprisal => (
                            TimelineSeverity::Warn,
                            format!(
                                "Hostile action by {who} grants a limited right of reprisal."
                            ),
                        ),
                    };
                    self.push(*recipient, *arrive_at, severity, text);
                }
                _ => {}
            }
            let topic = crate::welcome::topic_of(&e.payload);
            for notice in &mut self.pending[first_notice..] {
                if let Some((topic, system)) = topic {
                    notice.topic = Some(topic);
                    notice.system = system;
                }
                notice.alert = crate::notify::alert_of(&e.payload, notice.player);
                if notice.meta.kind.is_none() {
                    let merge = notice.meta.merge.take();
                    notice.meta = Meta { merge, ..log_meta(world, e, notice.player) };
                }
            }
            // Owner-only controls privacy, not propagation. This final gate is
            // shared by every timeline branch, including attackers' own news
            // and receiver-side refusals. Pre-priced arrival notices keep their
            // existing clocks; a local desk has zero travel distance.
            if let Some(origin) = e.physical_origin(world) {
                for notice in &mut self.pending[first_notice..] {
                    if let Some(corp) = world.players.get(&notice.player) {
                        notice.observe_time = notice.observe_time.max(e.time
                            + sim::transit::delay(origin, corp.command_center, world.config.c));
                    }
                }
            }
        }
    }

    fn push(
        &mut self,
        player: PlayerId,
        observe_time: f64,
        severity: TimelineSeverity,
        text: String,
    ) {
        self.push_meta(player, observe_time, severity, text, Meta::default());
    }

    /// The lines a blockade's plunder writes: the victim's loss on its light
    /// from the system, the besieger's prize on its own clock.
    fn plunder_lines(&mut self, world: &World, time: f64,
        (by, owner, system, pos): (PlayerId, PlayerId, sim::EntityId, sim::Vec2),
        victim: Option<BTreeMap<Commodity, u32>>, prize: Option<BTreeMap<Commodity, u32>>)
    {
        let name = system_name(world, system);
        let list = |goods: &BTreeMap<Commodity, u32>| goods.iter()
            .map(|(c, n)| format!("{n} {}", commodity_name(*c))).collect::<Vec<_>>().join(", ");
        let meta = |goods: &BTreeMap<Commodity, u32>, sign: i64| Meta {
            kind: Some(LogKind::Blockade), group: Some(format!("blockade:{system}")),
            refs: LogRefs { system: Some(system), ..Default::default() },
            data: goods_data(goods, sign), ..Default::default()
        };
        // The VICTIM learns by light from the system, exactly as they learn the
        // blockade itself — owning ground grants no FTL knowledge of what is
        // happening on it (§6).
        if let (Some(goods), Some(cc)) = (victim, world.players.get(&owner).map(|c2| c2.command_center)) {
            let observe = time + sim::transit::delay(pos, cc, world.config.c);
            self.push_meta(owner, observe, TimelineSeverity::Bad, format!(
                "{name} is being STRIPPED — {} carried off by the blockade holding it. \
                 Break the blockade or keep losing stores.", list(&goods)
            ), meta(&goods, -1));
        }
        // The BESIEGER is there: it is their own fleet's prize, so it is
        // own-clock news like any of their economy.
        if let Some(goods) = prize {
            self.push_meta(by, time, TimelineSeverity::Good, format!(
                "Prize taken at {name}: {} aboard the blockade — it still has to get home.", list(&goods)
            ), meta(&goods, 1));
        }
    }

    /// A line whose kind, group or numbers the arm knows better than
    /// [`log_meta`] does.
    fn push_meta(&mut self, player: PlayerId, observe_time: f64, severity: TimelineSeverity, text: String, meta: Meta) {
        self.pending.push(Pending {
            player,
            observe_time,
            severity,
            text,
            promoted: false,
            topic: None,
            system: None,
            alert: None,
            meta,
        });
    }

    /// §log: operation notices that land in the same second merge into one
    /// line, "4 operations expired", with their ids in the data.
    fn push_merged(&mut self, player: PlayerId, observe_time: f64, severity: TimelineSeverity, phase: LogPhase,
        id: String, refs: LogRefs)
    {
        let key = format!("operation:{phase:?}");
        if let Some(p) = self.pending.iter_mut().rev().find(|p| !p.promoted && p.player == player
            && p.meta.merge.as_deref() == Some(key.as_str()) && (p.observe_time - observe_time).abs() < 1.0)
        {
            let data = p.meta.data.get_or_insert_with(Default::default);
            data.ids.push(id);
            let n = data.ids.len() as u32;
            data.count = Some(n);
            p.meta.refs = LogRefs::default();
            p.text = operation_text(phase, n);
            return;
        }
        self.push_meta(player, observe_time, severity, operation_text(phase, 1), Meta {
            kind: Some(LogKind::Operation), phase: Some(phase), refs,
            data: Some(Box::new(LogData { count: Some(1), ids: vec![id], ..Default::default() })),
            group: None, merge: Some(key),
        });
    }

    /// Promote every entry whose light has now arrived into its player's journal.
    /// Idempotent per entry; bounds each journal to the most recent [`JOURNAL_CAP`].
    pub fn promote(&mut self, now: f64) {
        // §log: a production stop still standing after its grace is news now.
        let due: Vec<_> = self.suspends.iter().filter(|(_, (at, _, _))| now - at >= SUSPEND_GRACE_S)
            .map(|(key, _)| *key).collect();
        for key in due {
            let (_, observe, text) = self.suspends.remove(&key).expect("listed above");
            self.reported_suspends.insert(key);
            self.push_meta(key.0, observe.max(now), TimelineSeverity::Warn, text, Meta {
                kind: Some(LogKind::Production), refs: LogRefs { system: Some(key.1), ..Default::default() },
                ..Default::default()
            });
        }
        for p in &mut self.pending {
            if !p.promoted && p.observe_time <= now {
                p.promoted = true;
                let j = self.journal.entry(p.player).or_default();
                let revision = self.revisions.entry(p.player).or_insert(j.len());
                *revision = revision.wrapping_add(1);
                self.next_id += 1;
                j.push_back(TimelineEntry {
                    at_time: p.observe_time,
                    severity: p.severity,
                    text: p.text.clone(),
                    id: self.next_id,
                    kind: p.meta.kind,
                    group: p.meta.group.clone(),
                    phase: p.meta.phase,
                    refs: p.meta.refs.clone(),
                    data: p.meta.data.clone(),
                });
                trim(j, now);
                if let Some(topic) = p.topic {
                    self.away.entry(p.player).or_default().entry(topic).or_default()
                        .add(p.severity, &p.text, p.system, p.meta.data.as_deref());
                }
                if let Some(kind) = p.alert {
                    self.fresh_alerts.push((p.player, kind, p.text.clone()));
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
        self.away.remove(&player);
    }

    /// §notifications (R8): the alerting lines promoted since the last take.
    pub fn take_alerts(&mut self) -> Vec<(PlayerId, crate::notify::AlertKind, String)> {
        std::mem::take(&mut self.fresh_alerts)
    }

    /// §welcome (R9): what reached `player` while they were away, by topic.
    pub fn away_tally(&self, player: PlayerId) -> Option<&crate::welcome::AwayTally> {
        self.away.get(&player)
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

    pub fn revision(&self, player: PlayerId) -> usize {
        self.revisions.get(&player).copied().unwrap_or_else(|| self.journal_len(player))
    }
}

/// I–V for a bonus level.
fn roman(n: u32) -> &'static str {
    ["0", "I", "II", "III", "IV", "V"].get(n as usize).copied().unwrap_or("V+")
}

pub(crate) fn commodity_name(c: Commodity) -> &'static str {
    c.name()
}

/// §belts: what a survey charted in a system's asteroid belts, e.g.
/// "Asteroid belt discovered: metallic (ferrite ore, bornite ore)."
fn belt_discovery(world: &World, id: sim::EntityId) -> Option<String> {
    let sys = world.systems.iter().find(|s| s.id == id)?;
    let belts: Vec<String> = sys
        .bodies
        .iter()
        .filter_map(|b| {
            let shape = b.belt?;
            let ores: Vec<&str> = b.deposits.iter().map(|d| commodity_name(d.resource)).collect();
            Some(if ores.is_empty() {
                format!("{} (bare rock)", shape.kind.slug())
            } else {
                format!("{} ({})", shape.kind.slug(), ores.join(", "))
            })
        })
        .collect();
    match belts.len() {
        0 => None,
        1 => Some(format!("Asteroid belt discovered: {}.", belts[0])),
        n => Some(format!("{n} asteroid belts discovered: {}.", belts.join("; "))),
    }
}

/// §pockets: a body's name ("Veles Belt"), or its system's when it is gone.
fn body_name(world: &World, system: sim::EntityId, body: u32) -> String {
    world
        .systems
        .iter()
        .find(|s| s.id == system)
        .and_then(|s| s.bodies.iter().find(|b| b.id == body))
        .map(|b| b.name.clone())
        .unwrap_or_else(|| system_name(world, system))
}

/// §designations: the capability's name in the timeline.
fn designation_title(cap: sim::research::Cap) -> &'static str {
    match cap {
        sim::research::Cap::CrownVein => "Crown Vein",
        sim::research::Cap::CrownProject => "Crown Project",
        sim::research::Cap::MassStreams => "Mass Stream",
        sim::research::Cap::SignatureMimicry => "Signature Mimicry",
        _ => "Designation",
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

/// A short label for a fleet in the timeline — "your <flagship> fleet".
fn fleet_label(world: &World, id: sim::EntityId) -> String {
    match world.fleets.get(&id) {
        Some(f) => {
            let k = match f.flagship_kind() {
                sim::ShipKind::Builder => "construction ship",
                sim::ShipKind::Convoy => "medium freighter",
                sim::ShipKind::TinyFreighter => "tiny freighter",
                sim::ShipKind::SmallFreighter => "small freighter",
                sim::ShipKind::LargeFreighter => "large freighter",
                sim::ShipKind::HeavyFreighter => "heavy freighter",
                sim::ShipKind::BulkFreighter => "bulk freighter",
                sim::ShipKind::Raider => "interceptor",
                sim::ShipKind::Corvette => "corvette",
                sim::ShipKind::Colony => "colony",
                sim::ShipKind::Transport => "transport",
                sim::ShipKind::Scout => "scout",
                sim::ShipKind::SurveyProbe => "survey probe",
                sim::ShipKind::MiningBarge => "mining barge",
                sim::ShipKind::MiningBeltStation => "mining belt station",
                sim::ShipKind::CombatFreighter => "combat freighter",
                sim::ShipKind::Freighter => "freighter",
                sim::ShipKind::Destroyer => "destroyer",
                sim::ShipKind::Cruiser => "cruiser",
                sim::ShipKind::Battleship => "battleship",
                sim::ShipKind::Dreadnought => "dreadnought",
                sim::ShipKind::Titan => "titan",
            };
            format!("your {k} fleet")
        }
        None => "your fleet".to_string(),
    }
}

/// An ABSOLUTE sim-time as a mission-clock stamp ("T+7:20") — used for the §TCA
/// freight timetable, whose departure and arrival instants are exact.
fn fmt_clock(t: f64) -> String {
    format!("T+{}", fmt_wait(t))
}

/// Format a wait in seconds as `M:SS` for the echo countdown label.
fn fmt_wait(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Human label for a build job, for the check-in timeline (§step1).
fn build_label(what: sim::BuildKind) -> &'static str {
    match what {
        sim::BuildKind::Ship { ship: sim::ShipKind::TinyFreighter } => "a Tiny Freighter",
        sim::BuildKind::Ship { ship: sim::ShipKind::SmallFreighter } => "a Small Freighter",
        sim::BuildKind::Ship { ship: sim::ShipKind::LargeFreighter } => "a Large Freighter",
        sim::BuildKind::Ship { ship: sim::ShipKind::HeavyFreighter } => "a Heavy Freighter",
        sim::BuildKind::Ship { ship: sim::ShipKind::BulkFreighter } => "a Bulk Freighter",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Builder,
        } => "a Construction Ship",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Convoy,
        } => "a Medium Freighter",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Raider,
        } => "an Interceptor",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Corvette,
        } => "a Corvette",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Colony,
        } => "a Colony Ship",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Transport,
        } => "a Troop Transport",
        // §TCA: never appears in a real build event (the Freighter is TCA-only),
        // but the match must be total — a defensive label.
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Freighter,
        } => "an Authority Freighter",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Scout,
        } => "a Scout",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::SurveyProbe,
        } => "a Survey Probe",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::MiningBarge,
        } => "a Mining Barge",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::MiningBeltStation,
        } => "a Mining Belt Station",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::CombatFreighter,
        } => "a Combat Freighter",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Destroyer,
        } => "a Destroyer",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Cruiser,
        } => "a Cruiser",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Battleship,
        } => "a Battleship",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Dreadnought,
        } => "a Dreadnought",
        sim::BuildKind::Ship {
            ship: sim::ShipKind::Titan,
        } => "a Titan",
        sim::BuildKind::Upgrade { upgrade } => upgrade.title(),
        sim::BuildKind::Founding { project } => match project {
            sim::settlement::SettlementProject::Prospect => "a settlement survey",
            sim::settlement::SettlementProject::SurfaceWorkings => "surface workings",
            sim::settlement::SettlementProject::Storehouse => "a camp storehouse expansion",
            sim::settlement::SettlementProject::Quarters => "permanent quarters",
        },
        // §economy Part 4: an Academy course — label by profession.
        sim::BuildKind::Train { specialist } => match specialist {
            sim::SpecialistKind::Geologist => "a Geologist (training)",
            sim::SpecialistKind::PetrochemicalEngineer => "a Petrochemical Engineer (training)",
            sim::SpecialistKind::Xenobiologist => "a Xenobiologist (training)",
            sim::SpecialistKind::IndustrialEngineer => "an Industrial Engineer (training)",
            sim::SpecialistKind::NavalArchitect => "a Naval Architect (training)",
            sim::SpecialistKind::ExpertGeologist => "an Expert Geologist (training)",
            sim::SpecialistKind::ExpertPetrochemicalEngineer => "an Expert Petrochemical Engineer (training)",
            sim::SpecialistKind::ExpertXenobiologist => "an Expert Xenobiologist (training)",
            sim::SpecialistKind::ExpertIndustrialEngineer => "an Expert Industrial Engineer (training)",
            sim::SpecialistKind::ExpertNavalArchitect => "an Expert Naval Architect (training)",
        },
        sim::BuildKind::RecruitCaptain { .. } => "an officer commission",
        // §modules Part B3: a module in manufacture.
        sim::BuildKind::Module { module } => match module {
            sim::ModuleKind::MassDriver => "a Mass Driver",
            sim::ModuleKind::TorpedoRack => "a Torpedo Rack",
            sim::ModuleKind::PointDefenseScreen => "a Point-Defense Screen",
            sim::ModuleKind::ReflectivePlating => "Reflective Plating",
            sim::ModuleKind::WhippleArmor => "Whipple Armor",
            sim::ModuleKind::ExtendedTanks => "Extended Tanks",
            sim::ModuleKind::ReconSuite => "Recon Suite",
            sim::ModuleKind::CargoPods => "Cargo Pods",
            sim::ModuleKind::EscortDatalink => "Escort Datalink",
            sim::ModuleKind::FuelTransferRig => "Fuel Transfer Rig",
            sim::ModuleKind::SurveyDrive => "Survey Drive",
            sim::ModuleKind::NebulaSpectrometer => "Nebula Spectrometer",
            sim::ModuleKind::PrismaticLance => "Prismatic Lance",
            sim::ModuleKind::StasisWeb => "a Stasis Web",
            sim::ModuleKind::WarpStabilizer => "a Warp Stabilizer",
            sim::ModuleKind::InterdictionField => "an Interdiction Field",
            sim::ModuleKind::JumpDrive => "a Jump Drive",
            sim::ModuleKind::LanceArray => "a Lance Array",
        },
    }
}

fn kind_word(k: ShipKind) -> &'static str {
    match k {
        ShipKind::Builder => "construction ship",
        ShipKind::Convoy => "medium freighter",
        ShipKind::TinyFreighter => "tiny freighter",
        ShipKind::SmallFreighter => "small freighter",
        ShipKind::LargeFreighter => "large freighter",
        ShipKind::HeavyFreighter => "heavy freighter",
        ShipKind::BulkFreighter => "bulk freighter",
        ShipKind::Raider => "interceptor",
        ShipKind::Corvette => "corvette",
        ShipKind::Colony => "colony ship",
        ShipKind::Transport => "troop transport",
        ShipKind::Scout => "scout",
        ShipKind::SurveyProbe => "survey probe",
        ShipKind::MiningBarge => "mining barge",
        ShipKind::MiningBeltStation => "mining belt station",
        ShipKind::CombatFreighter => "combat freighter",
        ShipKind::Freighter => "freighter",
        ShipKind::Destroyer => "destroyer",
        ShipKind::Cruiser => "cruiser",
        ShipKind::Battleship => "battleship",
        ShipKind::Dreadnought => "dreadnought",
        ShipKind::Titan => "titan",
    }
}

/// The check-in line for an economy event the recipient cares about while away,
/// or `None` for the noisy/online-only ones we deliberately skip.
/// §TCA Phase 2: the charter penalty burned on an Exchange settlement, attached
/// to the trade that incurred it — a fee the player can't see is a mystery
/// drain, not a consequence.
/// §season: a requisition's demand, e.g. "300 alloys" or "4,000 Cr".
fn demand_text(demand: &sim::season::Demand) -> String {
    match *demand {
        sim::season::Demand::Goods { commodity, units } => format!("{units} {}", commodity_name(commodity)),
        sim::season::Demand::Credits { amount } => format!("{amount:.0} Cr"),
    }
}

/// What a Hub trade paid on top: §season the Authority's levy, then any
/// charter penalty (the sim charges both as one `penalty`).
fn penalty_suffix(world: &World, player: PlayerId, gross: f64, penalty: f64) -> String {
    let levy = (gross * world.levy_frac(player)).min(penalty).max(0.0);
    let rest = penalty - levy;
    let mut out = String::new();
    if levy > 0.005 {
        out += &format!(" Levy {levy:.0} Cr.");
    }
    if rest > 0.005 {
        out += &format!(" Charter penalty burned: {rest:.0} Cr.");
    }
    out
}

fn trade_entry(te: &TradeEvent, world: &World) -> Option<(TimelineSeverity, String)> {
    use TimelineSeverity::*;
    Some(match *te {
        TradeEvent::AutoDispatched {
            commodity,
            units,
            source,
            rule_id,
            ..
        } => (
            Good,
            format!(
                "Standing order #{rule_id} auto-shipped {units} {} from {} (raidable).",
                commodity_name(commodity),
                system_name(world, source)
            ),
        ),
        TradeEvent::Sold {
            player,
            commodity,
            units,
            unit_price,
            penalty,
        } => (
            Good,
            format!(
                "Sold {units} {} at the hub for {unit_price:.2} ea.{}",
                commodity_name(commodity),
                penalty_suffix(world, player, units as f64 * unit_price, penalty)
            ),
        ),
        TradeEvent::Delivered {
            commodity,
            units,
            system,
            ..
        } => (
            Good,
            match system {
                Some(sid) => format!(
                    "Delivery arrived: +{units} {} — stocked at {}.",
                    commodity_name(commodity),
                    system_name(world, sid)
                ),
                None => format!(
                    "Delivery arrived: +{units} {} — into your hub warehouse.",
                    commodity_name(commodity)
                ),
            },
        ),
        TradeEvent::LimitFilled {
            player,
            commodity,
            units,
            unit_price,
            side,
            penalty,
        } => {
            let s = format!("{side:?}").to_lowercase();
            (
                Good,
                format!(
                    "Limit {s} filled: {units} {} @ {unit_price:.2}.{}",
                    commodity_name(commodity),
                    penalty_suffix(world, player, units as f64 * unit_price, penalty)
                ),
            )
        }
        TradeEvent::SupplyDiverted {
            commodity,
            units,
            system,
            action,
            ..
        } => {
            let name = system_name(world, system);
            let com = commodity_name(commodity);
            match action {
                sim::DivertAction::Lost => (
                    Bad,
                    format!(
                        "Supply to {name} lost — you no longer hold it: {units} {com} dropped."
                    ),
                ),
                sim::DivertAction::ReturnedHome => (
                    Warn,
                    format!("Supply to {name} re-routed home ({units} {com}) — system lost."),
                ),
                sim::DivertAction::SoldAtHub => (
                    Warn,
                    format!(
                        "Supply to {name} re-routed to sell at the hub ({units} {com}) — system lost."
                    ),
                ),
            }
        }
        // A full warehouse bounced part of a delivery onward to the hub (§buildings
        // step 2) — an attention item: the player should ship out or build an Orbital Warehouse.
        TradeEvent::StorageOverflow {
            commodity,
            units,
            system,
            ..
        } => {
            let name = system_name(world, system);
            (
                Warn,
                format!(
                    "Storage full at {name}: {units} {} couldn't be stored — re-routed to sell at the hub. Ship goods out or build an Orbital Warehouse.",
                    commodity_name(commodity)
                ),
            )
        }
        // A SOFT-REJECTED Exchange order or freight booking (§9, §TCA) — owner-only,
        // instant, and free: nothing was spent. Names the reason so the fix is obvious.
        TradeEvent::Rejected {
            commodity,
            units,
            system,
            reason,
            ..
        } => {
            let com = commodity_name(commodity);
            let where_ = system.map(|s| system_name(world, s));
            match reason {
                sim::TradeRejectReason::InsufficientWarehouseStock { have } => (
                    Warn,
                    match &where_ {
                        Some(name) => format!(
                            "Can't ship {units} {com} to {name}: your hub warehouse holds {have}."
                        ),
                        None => format!(
                            "Can't sell {units} {com}: your hub warehouse holds {have}. \
                             Ship goods to the hub first (Authority freight or a convoy)."
                        ),
                    },
                ),
                sim::TradeRejectReason::NotYourSystem => (
                    Warn,
                    format!(
                        "Authority freight refused {units} {com}: {} isn't yours. The Charter \
                         Authority serves your own colonies only.",
                        where_.unwrap_or_else(|| "that system".into())
                    ),
                ),
                sim::TradeRejectReason::InsufficientSystemStock { have } => (
                    Warn,
                    format!(
                        "Can't collect {units} {com} from {}: its stockpile holds {have}.",
                        where_.unwrap_or_else(|| "that system".into())
                    ),
                ),
                sim::TradeRejectReason::CannotAffordFee { fee } => (
                    Warn,
                    format!("Can't book {units} {com}: the Authority's freight fee is {fee:.0} credits."),
                ),
                sim::TradeRejectReason::FleetUnavailable => (
                    Warn,
                    "That fleet can't handle cargo right now — it must be YOURS, idle, and not in a fight.".to_string(),
                ),
                sim::TradeRejectReason::OutOfLogisticsRange => (
                    Warn,
                    "That fleet is too far from the dock to move cargo — bring it alongside first.".to_string(),
                ),
                sim::TradeRejectReason::NoCargoRoom { capacity } if capacity == 0 => (
                    Warn,
                    format!("That fleet has no cargo hold — only freighters haul goods (tried {units} {com})."),
                ),
                sim::TradeRejectReason::NoCargoRoom { capacity } => (
                    Warn,
                    format!("Not enough hold for {units} {com}: this fleet lifts {capacity} units."),
                ),
                // `units == 0` is the reinstatement shape (no commodity involved);
                // a real trade carries its units.
                sim::TradeRejectReason::CantAfford { cost } if units == 0 => (
                    Warn,
                    format!("Reinstatement costs {cost:.0} credits — more than your treasury holds."),
                ),
                sim::TradeRejectReason::CantAfford { cost } => (
                    Warn,
                    format!(
                        "Can't buy {units} {com}: it needs {cost:.0} credits (price + charter penalty) — \
                         more than your treasury holds."
                    ),
                ),
                sim::TradeRejectReason::PriceProtection { bound, actual } => (
                    Warn,
                    format!(
                        "Market order protected: observed bound {bound:.2}, true average price {actual:.2}. Nothing traded."
                    ),
                ),
                sim::TradeRejectReason::MarketLiquidity { available } => (
                    Warn,
                    format!(
                        "The Global Market can clear only {available} {com} immediately. Nothing traded — reduce the lot or place a limit order."
                    ),
                ),
                sim::TradeRejectReason::CharterSuspended => (
                    Bad,
                    format!(
                        "The Authority won't book {units} {com}: your charter is SUSPENDED. \
                         Freight already queued or aboard still completes — pay down your \
                         citations, or haul it yourself."
                    ),
                ),
                sim::TradeRejectReason::CharterRevoked => (
                    Bad,
                    format!(
                        "The Exchange is closed to you: your charter is REVOKED ({units} {com} not traded). \
                         Your warehouse is still yours to fetch from — pay reinstatement to trade again."
                    ),
                ),
                sim::TradeRejectReason::Embargoed => (
                    Warn,
                    format!("Embargoed · {com} · pay the arrears to trade or ship it through the Authority."),
                ),
                sim::TradeRejectReason::CargoMismatch => (
                    Warn,
                    format!("That fleet is already carrying something else — unload before loading {com}."),
                ),
                sim::TradeRejectReason::DestinationBlockaded => (
                    Warn,
                    format!(
                        "The Authority won't book {units} {com} to {} — it reports the system BLOCKADED. \
                         Break the blockade, or move the goods yourself.",
                        where_.unwrap_or_else(|| "that system".into())
                    ),
                ),
            }
        }
        // §TCA Phase 2: the reinstatement receipt — what it cost, and the band it
        // bought you back into.
        TradeEvent::CharterReinstated {
            points,
            cost,
            before,
            after,
            ..
        } => {
            let from = sim::charter_status(before);
            let to = sim::charter_status(after);
            let crossed = if from != to {
                format!(" — charter reinstated to {}", to.title())
            } else {
                String::new()
            };
            (
                Good,
                format!(
                    "Paid the Authority {cost:.0} credits for {points:.0} standing ({before:.0} → {after:.0}){crossed}."
                ),
            )
        }
        // §TCA: the booking receipt — what it cost and when the Authority sails.
        TradeEvent::FreightBooked {
            commodity,
            units,
            system,
            direction,
            fee,
            depart_at,
            eta,
            ..
        } => {
            let name = system_name(world, system);
            let (verb, dest) = match direction {
                sim::ShipmentDir::Outbound => ("Booked", format!("to {name}")),
                sim::ShipmentDir::Inbound => ("Booked pickup of", format!("from {name}")),
            };
            (
                Good,
                format!(
                    "{verb} {units} {} {dest} — fee {fee:.0}cr, departs {}, arrives {}.",
                    commodity_name(commodity),
                    fmt_clock(depart_at),
                    fmt_clock(eta)
                ),
            )
        }
        // §TCA: freight progress. Only the outcomes a player would want in an
        // away-digest; the routine "departed" tick stays out of it.
        TradeEvent::FreightMoved {
            commodity,
            units,
            remaining,
            system,
            stage,
            ..
        } => {
            let name = system_name(world, system);
            let com = commodity_name(commodity);
            match stage {
                sim::FreightStage::Departed => return None,
                sim::FreightStage::CollectedForPickup => return None,
                sim::FreightStage::DeliveredToSystem if remaining > 0 => (
                    Warn,
                    format!(
                        "Authority freight delivered {units} {com} to {name}; storage was full, so {remaining} remains aboard for return to your Market Warehouse."
                    ),
                ),
                sim::FreightStage::DeliveredToSystem => (
                    Good,
                    format!("Authority freight delivered {units} {com} to {name}."),
                ),
                sim::FreightStage::ArrivedAtWarehouse => (
                    Good,
                    format!(
                        "Authority freight landed {units} {com} from {name} in your hub warehouse."
                    ),
                ),
                sim::FreightStage::ReturnedUndeliverable => (
                    Warn,
                    format!(
                        "Authority freight couldn't unload {units} {com} at {name} — it's no longer yours, \
                         or its storage is full. The lot is back in your hub warehouse."
                    ),
                ),
                sim::FreightStage::ForfeitedOnCapture => (
                    Bad,
                    format!(
                        "Lost {units} {com} awaiting pickup at {name} — the system fell before the Authority collected it."
                    ),
                ),
                sim::FreightStage::LostWithFreighter => (
                    Bad,
                    format!(
                        "{units} {com} destroyed with the Authority freighter carrying it (to/from {name})."
                    ),
                ),
            }
        }
        // Online-only / low-signal news (manual buys, dispatch-started, resting
        // placements) stays out of the away-digest.
        _ => return None,
    })
}

/// The check-in line for a battle, framed for `me`'s role and the outcome.
/// `privateer` names the founder's scripted privateer (§vendetta: the pirate
/// family's nephew) instead of its internal hull class, so the tutorial fight
/// reads as one story.
fn raid_entry(
    me: PlayerId,
    attacker: PlayerId,
    defender: PlayerId,
    attacker_kind: ShipKind,
    target_kind: ShipKind,
    outcome: RaidOutcome,
    privateer: Option<String>,
) -> (TimelineSeverity, String) {
    use TimelineSeverity::*;
    let i_attack = me == attacker;
    let _ = defender;
    let enemy = |kind: ShipKind, article: &str| {
        privateer.clone().unwrap_or_else(|| format!("{article} {}", kind_word(kind)))
    };
    match outcome {
        RaidOutcome::TargetDestroyed => {
            if i_attack {
                (
                    Good,
                    format!(
                        "Your {} destroyed {}.",
                        kind_word(attacker_kind),
                        enemy(target_kind, "an enemy")
                    ),
                )
            } else {
                (
                    Bad,
                    format!(
                        "You lost your {} to {}.",
                        kind_word(target_kind),
                        enemy(attacker_kind, "a hostile")
                    ),
                )
            }
        }
        RaidOutcome::AttackerDestroyed => {
            if i_attack {
                (
                    Bad,
                    format!(
                        "Your {} was destroyed in the attack.",
                        kind_word(attacker_kind)
                    ),
                )
            } else {
                (Good, format!("You destroyed {}.", enemy(attacker_kind, "an attacking")))
            }
        }
        RaidOutcome::BothDestroyed => (Bad, "Both ships were lost in a clash.".to_string()),
        RaidOutcome::BothSurvive => {
            if i_attack {
                (Info, "Your attack was driven off — no losses.".to_string())
            } else {
                (
                    Good,
                    "You drove off an attacking raider — no losses.".to_string(),
                )
            }
        }
        RaidOutcome::Escaped => {
            if i_attack {
                (Info, "Your quarry escaped to the hub.".to_string())
            } else {
                (Good, "Your freighter reached the hub safely.".to_string())
            }
        }
    }
}


/// §signals: one find, as a report line reads it.
fn signal_find_text(find: &sim::signals::SignalFind) -> String {
    use sim::signals::SignalFind as F;
    let goods = |goods: &[(sim::Commodity, u32)]| goods.iter()
        .map(|(c, n)| format!("{n} {}", c.display_name())).collect::<Vec<_>>().join(", ");
    match find {
        F::Credits { credits } => format!("{credits:.0} Cr"),
        F::Standing { credits, standing } => format!("{credits:.0} Cr, +{standing:.0} standing"),
        F::CargoAboard { goods: g } => format!("{} aboard", goods(g)),
        F::Cache { goods: g, .. } => format!("{} left for a Freighter", goods(g)),
        F::ModuleAboard { module } => format!("{} crate aboard", module.label()),
        F::Dossier { programme } => format!("dossier · {}",
            sim::research::programme(programme).map_or(programme.as_str(), |p| p.name)),
        F::Blueprint { module } => format!("{} blueprint", module.label()),
        F::Captain { name } => format!("Captain {name} rescued"),
        F::Specialist { specialist } => format!("{} flying home", specialist.title()),
        F::Gear { gear } => format!("{} to the armory", gear.name()),
    }
}

/// §log: keep [`JOURNAL_CAP`] entries and no more than [`JOURNAL_MAX_AGE`] of
/// history. Past the cap, a routine line from the older half goes before any
/// warning or bad news, so operation and trade receipts can't push a raid out.
fn trim(j: &mut VecDeque<TimelineEntry>, now: f64) {
    while j.front().is_some_and(|e| now - e.at_time > JOURNAL_MAX_AGE) {
        j.pop_front();
    }
    while j.len() > JOURNAL_CAP {
        let routine = j.iter().take(j.len() / 2)
            .position(|e| matches!(e.severity, TimelineSeverity::Info | TimelineSeverity::Good));
        match routine {
            Some(i) => {
                j.remove(i);
            }
            None => {
                j.pop_front();
            }
        }
    }
}

/// §log: a relation as players read it ("NonAggression" read as code).
fn relation_label(state: sim::diplomacy::RelationState) -> &'static str {
    match state {
        sim::diplomacy::RelationState::Neutral => "neutral",
        sim::diplomacy::RelationState::NonAggression => "non-aggression",
        sim::diplomacy::RelationState::War => "war",
        sim::diplomacy::RelationState::Ceasefire => "ceasefire",
    }
}

/// §log: one line for `n` like operation notices.
fn operation_text(phase: LogPhase, n: u32) -> String {
    match (phase, n) {
        (LogPhase::Offered, 1) => "A new private operation is available.".into(),
        (LogPhase::Offered, n) => format!("{n} new private operations are available."),
        (LogPhase::Active, 1) => "A new public operation is active.".into(),
        (LogPhase::Active, n) => format!("{n} new public operations are active."),
        (LogPhase::Completed, 1) => "Operation completed — its reward has arrived.".into(),
        (LogPhase::Completed, n) => format!("{n} operations completed — their rewards have arrived."),
        (LogPhase::Failed, 1) => "An operation failed.".into(),
        (LogPhase::Failed, n) => format!("{n} operations failed."),
        (_, 1) => "An operation expired.".into(),
        (_, n) => format!("{n} operations expired."),
    }
}

/// §log: goods an event moved, signed for the player it concerns.
fn goods_data(goods: &BTreeMap<Commodity, u32>, sign: i64) -> Option<Box<LogData>> {
    (!goods.is_empty()).then(|| Box::new(LogData {
        goods: goods.iter().filter(|(_, n)| **n > 0).map(|(c, n)| (*c, sign * *n as i64)).collect(),
        ..Default::default()
    }))
}

/// §log: when a launched home raid should reach its target. It holds at its
/// base until `depart_at`, then cruises at warp (Interceptors, ~500 su/s; a
/// measured raid flew 227,797 su in 458 s).
fn raid_eta(world: &World, fleet: sim::EntityId) -> Option<f64> {
    let raid = world.pirate_home_raids.get(&fleet)?;
    let pack = world.fleets.get(&fleet)?;
    let home = world.systems.iter().find(|s| s.id == raid.system)?.pos;
    let speed = pack.transit_speed() * sim::transit::WARP_FACTOR;
    (speed > 0.0).then(|| raid.depart_at + pack.pos.distance(home) / speed)
}

/// §log (v71): the kind, group, phase, refs and numbers of the lines `e`
/// writes for `player`. Arms that know better set [`Meta`] themselves.
fn log_meta(world: &World, e: &Event, player: PlayerId) -> Meta {
    use EventPayload as E;
    let sys = |id: &sim::EntityId| LogRefs { system: Some(*id), ..Default::default() };
    let fleet = |id: &sim::EntityId| LogRefs { fleet: Some(*id), ..Default::default() };
    let both = |s: &sim::EntityId, f: &sim::EntityId| LogRefs { system: Some(*s), fleet: Some(*f), ..Default::default() };
    let plain = |kind: LogKind, refs: LogRefs| Meta { kind: Some(kind), refs, ..Default::default() };
    let episode = |kind: LogKind, group: String, phase: LogPhase, refs: LogRefs| Meta {
        kind: Some(kind), group: Some(group), phase: Some(phase), refs, ..Default::default()
    };
    let build = |system: &sim::EntityId, what: sim::BuildKind, phase: LogPhase| episode(LogKind::Build,
        format!("build:{system}:{}", crate::view::build_key(what)), phase, sys(system));
    match &e.payload {
        // A pirate raid on a home is one episode: inbound, attacking or
        // raiding, ended.
        E::PirateRaidInbound { system, fleet: f, ships, .. } => Meta {
            data: Some(Box::new(LogData { ships: Some(*ships), eta: raid_eta(world, *f), ..Default::default() })),
            ..episode(LogKind::Raid, format!("raid:{system}"), LogPhase::Inbound, both(system, f))
        },
        E::RaidSighted { system, fleet: f, ships, eta, .. } => Meta {
            data: Some(Box::new(LogData { ships: Some(*ships), eta: *eta, ..Default::default() })),
            ..episode(LogKind::Raid, format!("raid:{system}"), LogPhase::Inbound, both(system, f))
        },
        E::RaidRepeatStopped { system, fleet: f, .. } => plain(LogKind::Fleet, both(system, f)),
        E::PirateHideoutSpotted { system, .. } => plain(LogKind::Raid, sys(system)),
        E::BlockadeContested { by, system, .. } if by.is_pirate() =>
            episode(LogKind::Raid, format!("raid:{system}"), LogPhase::Attacking, sys(system)),
        E::BlockadeEstablished { by, system, .. } if by.is_pirate() =>
            episode(LogKind::Raid, format!("raid:{system}"), LogPhase::Raiding, sys(system)),
        E::BlockadeLifted { by: Some(by), system, .. } if by.is_pirate() =>
            episode(LogKind::Raid, format!("raid:{system}"), LogPhase::Ended, sys(system)),
        E::PirateRaidWithdrawn { system, fleet: f, plunder, .. } => Meta {
            data: Some(Box::new(LogData {
                duration: world.pirate_home_raids.get(f).and_then(|r| r.arrived_at).map(|at| e.time - at),
                ..*goods_data(plunder, -1).unwrap_or_default()
            })),
            ..episode(LogKind::Raid, format!("raid:{system}"), LogPhase::Ended, both(system, f))
        },
        E::FoundingRaided { system, stolen, .. } => Meta { data: goods_data(stolen, -1), ..plain(LogKind::Raid, sys(system)) },
        E::PirateEnclaveCleared { system, plunder, .. } => Meta { data: goods_data(plunder, 1), ..plain(LogKind::Raid, sys(system)) },
        E::Vendetta { stolen, .. } => Meta { data: goods_data(stolen, -1), ..plain(LogKind::Vendetta, LogRefs::default()) },
        // A rival blockade is an episode too: contested, held, lifted.
        E::BlockadeContested { system, .. } =>
            episode(LogKind::Blockade, format!("blockade:{system}"), LogPhase::Contested, sys(system)),
        E::BlockadeEstablished { system, .. } =>
            episode(LogKind::Blockade, format!("blockade:{system}"), LogPhase::Held, sys(system)),
        E::BlockadeLifted { system, .. } =>
            episode(LogKind::Blockade, format!("blockade:{system}"), LogPhase::Lifted, sys(system)),
        E::BlockadeRun { system, fleet: f, .. } => plain(LogKind::Blockade, both(system, f)),
        E::SystemPlundered { system, .. } => plain(LogKind::Blockade, sys(system)),
        E::SystemCaptured { system, new_owner, plunder, .. } => Meta {
            data: goods_data(plunder, if *new_owner == player { 1 } else { -1 }),
            ..plain(LogKind::Capture, sys(system))
        },
        E::AssaultHeld { system, .. } | E::AssaultBegan { system, .. } | E::AssaultRepulsed { system, .. } =>
            Meta { group: Some(format!("landing:{system}")), ..plain(LogKind::Capture, sys(system)) },
        E::SystemClaimed { system, .. } | E::NodeAwakened { system, .. } | E::NodeCaptured { system, .. } =>
            plain(LogKind::Territory, sys(system)),
        E::RaidResolved { battle_id, attacker, attacker_ship, target_ship, .. } => plain(LogKind::Battle, LogRefs {
            fleet: Some(if *attacker == player { *attacker_ship } else { *target_ship }),
            battle: *battle_id, ..Default::default()
        }),
        E::StationEngaged { system, .. } | E::PlatformEngaged { system, .. } => plain(LogKind::Battle, sys(system)),
        E::WrecksSalvaged { fleet: f, .. } | E::PrizeTaken { fleet: f, .. }
        | E::InterdictionRaised { fleet: f, .. } | E::InterdictionCollapsed { fleet: f, .. } => plain(LogKind::Battle, fleet(f)),
        E::FlagshipDestroyed { .. } | E::EmplacementDestroyed { .. } | E::ModulesLost { .. } | E::SpecialistsLost { .. } =>
            plain(LogKind::Battle, LogRefs::default()),
        // Construction: started and done are one episode per structure.
        E::BuildStarted { system, what, .. } => build(system, *what, LogPhase::Started),
        E::BuildCancelled { system, what, .. } => build(system, *what, LogPhase::Ended),
        E::SystemUpgraded { system, upgrade, .. } => build(system, sim::BuildKind::Upgrade { upgrade: *upgrade }, LogPhase::Done),
        E::ShipLaunched { system, fleet: f, kind, .. } => Meta {
            refs: both(system, f), ..build(system, sim::BuildKind::Ship { ship: *kind }, LogPhase::Done)
        },
        E::SettlementDeveloped { system, project, .. } => build(system, sim::BuildKind::Founding { project: *project }, LogPhase::Done),
        E::SpecialistTrained { system, .. } | E::StationRefitted { system, .. } | E::ModuleBuilt { system, .. }
        | E::ShipsRefitted { system, .. } => plain(LogKind::Build, sys(system)),
        E::BuildRejected { system, .. } | E::IndustryRejected { system, .. } => plain(LogKind::Refusal, sys(system)),
        E::ProductionSuspended { system, .. } | E::ProductionResumed { system, .. } | E::Brownout { system, .. }
        | E::PowerRestored { system, .. } | E::FoodStateChanged { system, .. } | E::NodeSupplyChanged { system, .. } =>
            plain(LogKind::Production, sys(system)),
        E::GarrisonSupplyStateChanged { system, .. } | E::ColonyHeld { system, .. } => plain(LogKind::Fleet, sys(system)),
        E::GarrisonSupplyChanged { host, .. } => plain(LogKind::Fleet, sys(host)),
        E::FleetRepaired { fleet: f, system, .. } | E::StationAnchored { fleet: f, system, .. }
        | E::StationUnanchored { fleet: f, system, .. } => plain(LogKind::Fleet, both(system, f)),
        E::FleetSupplyChanged { fleet: f, .. } | E::FuelSwitched { fleet: f, .. } | E::JumpFailed { fleet: f, .. }
        | E::FuelRescueDispatched { fleet: f, .. } | E::FuelRescueCompleted { fleet: f, .. }
        | E::FuelRescueFailed { fleet: f, .. } | E::FuelRescueRejected { fleet: f, .. } => plain(LogKind::Fleet, fleet(f)),
        E::FuelShortfall { .. } => plain(LogKind::Fleet, LogRefs::default()),
        E::OrderRejected { fleet: f, .. } => plain(LogKind::Refusal, fleet(f)),
        E::StationReleased { fleet: f, system, .. } => plain(LogKind::Freight, both(system, f)),
        E::StationLoaded { fleet: f, .. } => plain(LogKind::Freight, fleet(f)),
        E::SpecialistsDelivered { system, .. } | E::ModulesDelivered { system, .. } => plain(LogKind::Freight, sys(system)),
        E::IntelGathered { system, .. } | E::ScoutContest { system, .. } | E::SurveyCompleted { system, .. } | E::ProspectCompleted { system, .. }
        | E::PocketStruck { system, .. } | E::TraitRevealed { system, .. } => plain(LogKind::Survey, sys(system)),
        E::SignalDetected { .. } | E::SignalReported { .. } => plain(LogKind::Signal, LogRefs::default()),
        E::GearFound { system, .. } => plain(LogKind::Signal, sys(system)),
        E::ResearchCompleted { .. } | E::TierUnlocked { .. } | E::ResearchStalled { .. } | E::ResearchResumed { .. }
        | E::ResearchFunded { .. } | E::DesignationSet { .. } => plain(LogKind::Research, LogRefs::default()),
        E::ResearchGrantRefused { .. } | E::DesignationRefused { .. } | E::LeaveRefused { .. } | E::PaymentRefused { .. } =>
            plain(LogKind::Refusal, LogRefs::default()),
        E::LeaveChanged { .. } => plain(LogKind::Admin, LogRefs::default()),
        E::Trade(te) => plain(match te {
            TradeEvent::Rejected { .. } => LogKind::Refusal,
            TradeEvent::CharterReinstated { .. } => LogKind::Charter,
            TradeEvent::Sold { .. } | TradeEvent::Bought { .. } | TradeEvent::LimitFilled { .. } | TradeEvent::LimitPlaced { .. }
            | TradeEvent::LimitCancelled { .. } | TradeEvent::SellDispatched { .. } => LogKind::Trade,
            _ => LogKind::Freight,
        }, LogRefs::default()),
        E::SpecialistHired { .. } | E::ModulesPurchased { .. } | E::ModulesSold { .. } => plain(LogKind::Trade, LogRefs::default()),
        E::MerchantArrived { system, .. } | E::MerchantTraded { system, .. } | E::MerchantTradeRefused { system, .. }
        | E::MerchantDeparted { system, .. } => plain(LogKind::Trader, sys(system)),
        E::CharterStipend { credits, .. } => Meta {
            data: (*credits > 0.0).then(|| Box::new(LogData { credits: Some(*credits), ..Default::default() })),
            ..plain(LogKind::Charter, LogRefs::default())
        },
        E::CharterCrateCollected { .. } => plain(LogKind::Charter, LogRefs::default()),
        E::RaidCapped { system, .. } => plain(LogKind::Blockade, sys(system)),
        E::FounderFreighterReplaced { fleet: f, system, .. } => plain(LogKind::Charter, both(system, f)),
        E::EnforcementDispatched { system, .. } => plain(LogKind::Charter, sys(system)),
        E::Citation { .. } | E::EnforcementWithdrawn { .. } | E::Decree { .. } | E::LevyRiseDemanded { .. }
        | E::LevyAnswered { .. } | E::EmbargoLifted { .. } | E::RequisitionIssued { .. }
        | E::RequisitionSettled { .. } | E::ResolveMilestone { .. } | E::CrackdownExposed { .. }
        | E::EvidenceRead { .. } | E::SpyLeak { .. } | E::AuthorityWar { .. } => plain(LogKind::Charter, LogRefs::default()),
        E::RelicWoke { system, .. } | E::BearingDrawn { relic: system, .. } | E::PrecursorFound { system, .. }
        | E::KeystoneLevel { system, .. } | E::KeystoneFed { system, .. } => plain(LogKind::Territory, sys(system)),
        E::GateStepped { fleet: f, system, .. } => plain(LogKind::Fleet, both(system, f)),
        E::SeasonEnded { .. } => plain(LogKind::Charter, LogRefs::default()),
        E::CrackdownBegan { system, .. } => plain(LogKind::Charter, system.as_ref().map(sys).unwrap_or_default()),
        E::ContractBoardPosted { .. } | E::ContractBoardBonus { .. } => plain(LogKind::Operation, LogRefs::default()),
        E::OperationUpdated { operation, .. } => plain(LogKind::Operation, LogRefs { operation: Some(*operation), ..Default::default() }),
        E::MedalWon { .. } | E::SyndicateMedalWon { .. } => plain(LogKind::Medal, LogRefs::default()),
        E::SyndicateRungFinished { .. } | E::SyndicateBonusRaised { .. } => plain(LogKind::Syndicate, LogRefs::default()),
        E::DiplomacyUpdated { .. } => plain(LogKind::Diplomacy, LogRefs::default()),
        _ => plain(LogKind::Admin, LogRefs::default()),
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
            Command::AddPlayer {
                id: a,
                name: "A".into(),
            },
            Command::AddPlayer {
                id: b,
                name: "B".into(),
            },
        ]);
        (w, a, b)
    }

    #[test]
    fn settlement_completion_is_private_and_waits_for_its_site_report() {
        let (mut w, owner, rival) = world_with_two();
        let system = w.players[&owner].home_system.unwrap();
        let site = w.systems.iter().find(|s| s.id == system).unwrap().pos;
        let cc = site + Vec2::new(10.0 * sim::transit::signal_speed(w.config.c), 0.0);
        w.players.get_mut(&owner).unwrap().command_center = cc;
        let mut tl = Timeline::new();
        tl.ingest(&[Event::new(w.time, EventPayload::SettlementDeveloped {
            owner, system, body_id: 1, project: sim::settlement::SettlementProject::Storehouse,
        })], &w);
        tl.promote(w.time + 9.99);
        assert_eq!(tl.journal_len(owner), 0);
        tl.promote(w.time + 10.0);
        assert_eq!(tl.journal_len(owner), 1);
        assert!(tl.digest(owner).0[0].text.contains("complete"));
        assert_eq!(tl.journal_len(rival), 0);
    }

    /// A held blockade strips a unit and a half a second; each side reads one
    /// line a minute with the haul since the last, not one a unit.
    #[test]
    fn plunder_is_one_line_a_minute_with_the_haul() {
        let (w, owner, besieger) = world_with_two();
        let system = w.players[&owner].home_system.unwrap();
        let pos = w.systems.iter().find(|s| s.id == system).unwrap().pos;
        let mut tl = Timeline::new();
        for k in 0..150u32 {
            let t = w.time + f64::from(k) * (2.0 / 3.0);
            tl.ingest(&[Event::new(t, EventPayload::SystemPlundered {
                by: besieger, owner, system, commodity: Commodity::Alloys, units: 1, pos,
            })], &w);
        }
        tl.promote(w.time + 10_000.0);
        let (prize, _) = tl.digest(besieger);
        assert_eq!(prize.len(), 2, "100 seconds of plunder, two lines: {prize:?}");
        assert!(prize[0].text.contains(": 1 alloys"), "{}", prize[0].text);
        assert!(prize[1].text.contains(": 90 alloys"), "the minute's haul: {}", prize[1].text);
        assert_eq!(tl.digest(owner).0.len(), 2);
    }

    #[test]
    fn a_blockade_run_reaches_both_corporations_by_light() {
        let (w, owner, besieger) = world_with_two();
        let system = w.players[&owner].home_system.unwrap();
        let pos = w.players[&owner].command_center + Vec2::new(3000.0, 0.0);
        let mut tl = Timeline::new();
        tl.ingest(&[Event::new(w.time, EventPayload::BlockadeRun {
            owner, by: besieger, fleet: EntityId(77), system, pos, caught: true, chance: 0.657,
        })], &w);
        for (who, needle) in [(owner, "caught running the blockade"), (besieger, "caught a freighter")] {
            let lands = w.time + sim::transit::delay(pos, w.players[&who].command_center, w.config.c);
            tl.promote(lands - 1e-6);
            assert!(tl.digest(who).0.iter().all(|e| !e.text.contains(needle)), "not before the light");
            tl.promote(lands + 1e-6);
            let (lines, _) = tl.digest(who);
            assert!(lines.iter().any(|e| e.text.contains(needle)), "{lines:?}");
        }
        assert!(tl.digest(owner).0.iter().any(|e| e.text.contains("66%")));
    }

    #[test]
    fn own_market_news_arrives_from_the_market_hub() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        let ev = Event::new(
            w.time,
            EventPayload::Trade(TradeEvent::Sold {
                player: a,
                commodity: Commodity::MetallicOre,
                units: 12,
                unit_price: 8.0,
                penalty: 0.0,
            }),
        );
        tl.ingest(&[ev], &w);
        tl.promote(w.time);
        assert_eq!(
            tl.journal_len(a),
            0,
            "the sale cannot outrun its Market Hub receipt"
        );
        let arrives = w.time + sim::transit::delay(w.hub, w.players[&a].command_center, w.config.c);
        tl.promote(arrives + 1e-6);
        let (entries, _away) = tl.digest(a);
        assert_eq!(
            entries.len(),
            1,
            "own sale journals when its hub light arrives"
        );
        assert!(entries[0].text.contains("Sold 12 ferrite ore"));
    }

    #[test]
    fn a_rival_claim_arrives_light_delayed() {
        let (w, a, b) = world_with_two();
        let mut tl = Timeline::new();
        // A claim far from b's command center: its light takes time to arrive.
        let cc = w.players[&b].command_center;
        let pos = cc + Vec2::new(3000.0, 0.0); // 10 s of light away at c=300
        let ev = Event::new(
            w.time,
            EventPayload::SystemClaimed {
                system: EntityId(999),
                owner: a,
                pos,
            },
        );
        tl.ingest(&[ev], &w);

        // Ownership is permission, not a zero-delay communications channel.
        tl.promote(w.time);
        assert_eq!(tl.journal_len(a), 0, "the owner's claim report is also in flight");
        assert_eq!(tl.journal_len(b), 0, "rival's light hasn't arrived");

        // After the light delay: the rival now sees it (light-respecting awareness).
        tl.promote(w.time + 11.0);
        assert_eq!(tl.journal_len(b), 1, "rival learns after the light arrives");
        let (entries, _) = tl.digest(b);
        assert!(entries[0].text.contains("rival claimed"));
        tl.promote(w.time + sim::transit::delay(pos, w.players[&a].command_center, w.config.c) + 0.01);
        assert_eq!(tl.journal_len(a), 1, "the owner learns when its own report arrives");
    }

    #[test]
    fn offline_buffering_and_away_boundary() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        // Player A was last online at t = 5.
        tl.mark_seen(a, 5.0);
        // While "away", two automation events occur and become observable.
        let mut last_arrival: f64 = 0.0;
        for (t, units) in [(10.0_f64, 3u32), (20.0, 4)] {
            let trade = TradeEvent::AutoDispatched {
                player: a,
                commodity: Commodity::Deuterium,
                units,
                source: EntityId(1),
                rule_id: 1,
            };
            let origin = crate::game_loop::trade_report_origin(&w, &trade);
            last_arrival = last_arrival
                .max(t + sim::transit::delay(origin, w.players[&a].command_center, w.config.c));
            let ev = Event::new(t, EventPayload::Trade(trade));
            tl.ingest(&[ev], &w);
        }
        tl.promote(last_arrival + 1.0); // both report wavefronts have marched on
        let (entries, away_since) = tl.digest(a);
        assert_eq!(entries.len(), 2, "events buffered while offline");
        assert_eq!(away_since, 5.0, "away boundary is the last-online time");
        assert!(
            entries.iter().all(|e| e.at_time > away_since),
            "all are 'while you were away'"
        );
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
                    commodity: Commodity::MetallicOre,
                    units: i + 1,
                    system: None,
                }),
            );
            tl.ingest(&[ev], &w);
        }
        tl.promote(1000.0);
        assert_eq!(
            tl.journal_len(a),
            JOURNAL_CAP,
            "journal keeps only the most recent cap"
        );
    }

    #[test]
    fn pirate_raid_notices_share_normal_light_delay_and_survive_reconnect() {
        let (w, a, b) = world_with_two();
        let pos = w.players[&a].command_center + Vec2::new(30_000.0, 0.0);
        let system = w.players[&a].home_system.unwrap();
        let at = w.time;
        let arrived = at + sim::transit::delay(pos, w.players[&a].command_center, w.config.c);
        let mut tl = Timeline::new();
        tl.ingest(&[Event::new(at, EventPayload::PirateRaidInbound {
            owner: a, system, fleet: EntityId(9000), ships: 2, pos,
        })], &w);
        tl.promote(arrived - 1e-6);
        assert_eq!(tl.journal_len(a), 0, "warning is not an instant truth-side alert");
        let mut tl: Timeline = serde_json::from_str(&serde_json::to_string(&tl).unwrap()).unwrap();
        tl.promote(arrived);
        assert_eq!(tl.journal_len(a), 1);
        assert_eq!(tl.journal_len(b), 0, "a private threat is not broadcast to rivals");
        let entries = tl.digest(a).0;
        assert!(entries[0].text.starts_with("Pirate raid inbound · "));
        assert_eq!(entries[0].at_time, arrived);
        // §log: the warning is the start of one raid episode.
        assert_eq!((entries[0].kind, entries[0].phase), (Some(LogKind::Raid), Some(LogPhase::Inbound)));
        assert_eq!(entries[0].group.as_deref(), Some(format!("raid:{system}").as_str()));
        assert_eq!(entries[0].data.as_ref().and_then(|d| d.ships), Some(2));
        assert!(entries[0].id > 0);
        tl.ingest(&[Event::new(at + 60.0, EventPayload::PirateRaidWithdrawn {
            owner: a, system, fleet: EntityId(9000), pos,
            plunder: std::collections::BTreeMap::from([(Commodity::Electronics, 8)]),
        })], &w);
        tl.promote(arrived + 60.0 - 1e-6);
        assert_eq!(tl.journal_len(a), 1, "the outcome has its own wavefront too");
        tl.promote(arrived + 60.0);
        let ended = &tl.digest(a).0[1];
        assert!(ended.text.contains("8 semiconductors"));
        assert_eq!(ended.phase, Some(LogPhase::Ended));
        assert!(matches!(ended.severity, TimelineSeverity::Bad));
        assert_eq!(ended.data.as_ref().and_then(|d| d.goods.get(&Commodity::Electronics).copied()), Some(-8),
            "the take, signed as a loss");
        assert_eq!(tl.journal_len(b), 0);
    }

    #[test]
    fn exploration_retreat_notice_waits_for_the_onboard_reports_light() {
        let (w, owner, rival) = world_with_two();
        let pos = w.players[&owner].command_center + Vec2::new(40_000.0, 0.0);
        let arrival = w.time + sim::transit::delay(pos, w.players[&owner].command_center, w.config.c);
        let mut tl = Timeline::new();
        tl.ingest(&[Event::new(w.time, EventPayload::OrderRejected {
            owner, fleet: EntityId(9900), target: Some(EntityId(9901)),
            reason: sim::event::OrderRejectReason::ExplorationThreat,
        }).at_origin(pos)], &w);
        tl.promote(arrival - 1e-6);
        assert_eq!(tl.journal_len(owner), 0);
        tl.promote(arrival);
        assert_eq!(tl.journal_len(owner), 1);
        assert_eq!(tl.journal_len(rival), 0);
        assert!(tl.digest(owner).0[0].text.contains("retreating"));
    }

    #[test]
    fn cleared_hideout_news_promises_recovery_not_an_instant_stockpile_payout() {
        let (w, owner, rival) = world_with_two();
        let system = w.players[&owner].home_system.unwrap();
        let pos = w.players[&owner].command_center + Vec2::new(30_000.0, 0.0);
        let at = 100.0;
        let arrival = at + sim::transit::delay(pos, w.players[&owner].command_center, w.config.c);
        let mut tl = Timeline::new();
        tl.ingest(&[Event::new(at, EventPayload::PirateEnclaveCleared {
            owner, system, pos, plunder: std::collections::BTreeMap::from([(Commodity::Alloys, 16)]), personal: false,
        })], &w);
        tl.promote(arrival - 1e-6);
        assert_eq!(tl.journal_len(owner), 0);
        tl.promote(arrival);
        let notice = &tl.digest(owner).0[0].text;
        assert!(notice.contains("16 units await Freighter recovery on-site"));
        assert!(notice.contains("paused for 10m"));
        assert!(!notice.contains("seized"));
        assert_eq!(tl.journal_len(rival), 0);
    }

    #[test]
    fn pirate_raid_warning_changes_revision_even_when_journal_is_full() {
        let (w, a, _) = world_with_two();
        let mut tl = Timeline::new();
        for i in 0..JOURNAL_CAP {
            tl.push(a, i as f64, TimelineSeverity::Info, format!("Old notice {i}"));
        }
        tl.promote(1000.0);
        let before = tl.revision(a);
        let pos = w.players[&a].command_center;
        tl.ingest(&[Event::new(1001.0, EventPayload::PirateRaidInbound {
            owner: a, system: w.players[&a].home_system.unwrap(), fleet: EntityId(9000), ships: 1, pos,
        })], &w);
        tl.promote(1000.0);
        assert_eq!(tl.revision(a), before);
        tl.promote(1001.0);
        assert_eq!(tl.journal_len(a), JOURNAL_CAP);
        assert_ne!(tl.revision(a), before, "live clients must receive the new warning after the cap");
        assert!(tl.digest(a).0.last().unwrap().text.starts_with("Pirate raid inbound · "));
        let restored: Timeline = serde_json::from_str(&serde_json::to_string(&tl).unwrap()).unwrap();
        assert_eq!(restored.revision(a), tl.revision(a));
    }

    /// §log: operation notices that land together are one line with their ids;
    /// none carries a bracketed id in its text.
    #[test]
    fn operation_notices_that_land_together_merge() {
        let (w, a, _) = world_with_two();
        let mut tl = Timeline::new();
        let note = |id: u64, state| Event::new(10.0, EventPayload::OperationUpdated {
            operation: sim::OperationId(id), recipient: a, state, progress: 0, goal: 1,
            pos: Vec2::ZERO, arrive_at: 20.0,
        });
        tl.ingest(&(1..=4).map(|id| note(id, sim::OperationState::Offered)).collect::<Vec<_>>(), &w);
        tl.ingest(&[note(5, sim::OperationState::Expired)], &w);
        tl.promote(20.0);
        let entries = tl.digest(a).0;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "4 new private operations are available.");
        let data = entries[0].data.as_ref().unwrap();
        assert_eq!((data.count, data.ids.len()), (Some(4), 4));
        assert_eq!(entries[1].text, "An operation expired.");
        assert_eq!(entries[1].refs.operation, Some(sim::OperationId(5)));
        assert!(entries.iter().all(|e| !e.text.contains("[operation")));
    }

    /// §log: a week or 300 lines; past the cap, routine lines go first.
    #[test]
    fn the_journal_keeps_warnings_over_routine_lines() {
        let (_, a, _) = world_with_two();
        let mut tl = Timeline::new();
        tl.push(a, 0.0, TimelineSeverity::Bad, "Raided early.".into());
        for i in 0..JOURNAL_CAP {
            tl.push(a, 1.0 + i as f64, TimelineSeverity::Info, format!("Routine {i}"));
        }
        tl.promote(1000.0);
        let entries = tl.digest(a).0;
        assert_eq!(entries.len(), JOURNAL_CAP);
        assert_eq!(entries[0].text, "Raided early.", "the oldest warning outlives routine lines");
        assert!(!entries.iter().any(|e| e.text == "Routine 0"));
        tl.push(a, JOURNAL_MAX_AGE + 2.0, TimelineSeverity::Info, "A week on.".into());
        tl.promote(JOURNAL_MAX_AGE + 2.0);
        assert!(!tl.digest(a).0.iter().any(|e| e.text == "Raided early."), "a week is the limit");
    }

    /// §log: a production stop that clears within its grace is never news; a
    /// standing one is, and so is its restart.
    #[test]
    fn short_production_stops_are_not_news() {
        let (w, a, _) = world_with_two();
        let system = w.players[&a].home_system.unwrap();
        let mut tl = Timeline::new();
        let stop = |at| Event::new(at, EventPayload::ProductionSuspended {
            owner: a, system, structure: sim::StructureKind::MiningComplex,
            reason: sim::SuspendReason::StorageFull,
        });
        let start = |at| Event::new(at, EventPayload::ProductionResumed {
            owner: a, system, structure: sim::StructureKind::MiningComplex,
        });
        tl.ingest(&[stop(10.0)], &w);
        tl.promote(30.0);
        tl.ingest(&[start(40.0)], &w);
        tl.promote(500.0);
        assert_eq!(tl.journal_len(a), 0, "a hauler drew it down in time");
        tl.ingest(&[stop(600.0)], &w);
        tl.promote(600.0 + SUSPEND_GRACE_S);
        tl.ingest(&[start(900.0)], &w);
        tl.promote(900.0);
        let texts: Vec<_> = tl.digest(a).0.into_iter().map(|e| e.text).collect();
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert!(texts[0].contains("SUSPENDED") && texts[1].contains("producing again"));
    }

    /// §log: a pirate raid's end is the raid's own line with its take; the
    /// blockade lifting adds nothing. A rival blockade's last minute of
    /// plunder is reported before it lifts.
    #[test]
    fn a_raid_ends_on_its_take_and_a_blockade_reports_its_last_minute() {
        let (w, a, b) = world_with_two();
        let system = w.players[&a].home_system.unwrap();
        let pos = w.players[&a].command_center;
        let mut tl = Timeline::new();
        tl.ingest(&[Event::new(10.0, EventPayload::BlockadeLifted {
            owner: a, system, pos, held: true, by: Some(PlayerId::PIRATE),
        })], &w);
        tl.promote(10.0);
        assert_eq!(tl.journal_len(a), 0);
        let plundered = |at| Event::new(at, EventPayload::SystemPlundered {
            by: b, owner: a, system, commodity: Commodity::Alloys, units: 3, pos,
        });
        tl.ingest(&[plundered(100.0), plundered(110.0), plundered(120.0)], &w);
        tl.ingest(&[Event::new(130.0, EventPayload::BlockadeLifted { owner: a, system, pos, held: true, by: Some(b) })], &w);
        tl.promote(10_000.0);
        let lost: i64 = tl.digest(a).0.iter().filter_map(|e| e.data.as_ref())
            .filter_map(|d| d.goods.get(&Commodity::Alloys)).sum();
        assert_eq!(lost, -9, "every unit taken is reported");
        assert!(tl.digest(a).0.iter().any(|e| e.phase == Some(LogPhase::Lifted)));
    }

    /// §log: a fuel warning given with the order says the order went ahead.
    #[test]
    fn a_fuel_warning_is_not_a_held_fleet() {
        let (w, a, _) = world_with_two();
        let mut tl = Timeline::new();
        tl.ingest(&[Event::new(1.0, EventPayload::FuelShortfall {
            owner: a, needed: 40.0, kind: sim::fuel::ShortfallKind::Move, warning: true,
        })], &w);
        tl.promote(1.0);
        let text = &tl.digest(a).0[0].text;
        assert!(text.contains("went ahead") && !text.contains("held"), "{text}");
    }
}

/// §standing-ladder (R3): "+400 Cr · +3 standing · research data (Fusion
/// Freight)", leaving out what a reward didn't pay.
fn service_reward(credits: f64, standing: f64, dossier: Option<&str>) -> String {
    let mut parts = Vec::new();
    if credits > 0.0 {
        parts.push(format!("{credits:.0} Cr"));
    }
    if standing > 0.0 {
        parts.push(format!("+{} standing", (standing * 10.0).round() / 10.0));
    }
    if let Some(id) = dossier {
        parts.push(format!("research data ({})", sim::research::programme(id).map_or(id, |p| p.name)));
    }
    if parts.is_empty() {
        "nothing more today".into()
    } else {
        parts.join(" · ")
    }
}

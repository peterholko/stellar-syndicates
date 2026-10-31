//! §welcome (R9): the report on return, built on the server.
//!
//! Three parts. What became observable while the corporation was away,
//! tallied by topic as each timeline line's light arrived, so a long absence
//! is not cut to the journal's last forty lines; sales and purchases are summed
//! from the receipt history. What is waiting now: open Signals, the day's
//! board, idle Academies and yards, unread mail. And the next three actions.
//! Built from what the corporation already knows; never ahead of the light.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sim::{EntityId, EventPayload, FreightStage, PlayerId, TradeEvent, World};

use crate::protocol::TimelineSeverity;
use crate::transactions::{TransactionDetails, TransactionHistory};

/// Shorter absences are a tab switch, not a return (the client's rule too).
pub const WELCOME_MIN_AWAY_S: f64 = 600.0;
/// The report's "while you were away" list stays short.
const HAPPENED_MAX: usize = 6;
/// Items named in the sales line before "+N more".
const TRADE_ITEMS: usize = 3;

/// What a timeline line is about. Declaration order is the report's order
/// within a severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Siege,
    Raid,
    Battle,
    Research,
    Trade,
    Launch,
    Built,
    Arrival,
    Signal,
    Board,
    Syndicate,
    Medal,
    Charter,
}

impl Topic {
    /// The plural noun for a tally of more than one line.
    fn noun(self) -> &'static str {
        match self {
            Topic::Siege => "Siege reports",
            Topic::Raid => "Raid reports",
            Topic::Battle => "Battles",
            Topic::Research => "Research reports",
            Topic::Trade => "Trades",
            Topic::Launch => "Hulls launched",
            Topic::Built => "Construction finished",
            Topic::Arrival => "Arrivals",
            Topic::Signal => "Signal reports",
            Topic::Board => "Contract updates",
            Topic::Syndicate => "Syndicate projects",
            Topic::Medal => "Medals",
            Topic::Charter => "Charter payments",
        }
    }
}

/// The topic of the timeline lines an event writes, and the system they
/// concern when they name one. `None`: not part of the report on return.
pub fn topic_of(payload: &EventPayload) -> Option<(Topic, Option<EntityId>)> {
    use EventPayload as E;
    Some(match payload {
        E::BlockadeContested { system, .. } | E::BlockadeEstablished { system, .. }
        | E::BlockadeLifted { system, .. } | E::BlockadeRun { system, .. }
        | E::SystemCaptured { system, .. } | E::AssaultBegan { system, .. }
        | E::AssaultHeld { system, .. } | E::AssaultRepulsed { system, .. }
        | E::ColonyHeld { system, .. } | E::SystemPlundered { system, .. } => (Topic::Siege, Some(*system)),
        E::PirateRaidInbound { system, .. } | E::PirateRaidWithdrawn { system, .. } | E::RaidSighted { system, .. }
        | E::PirateHideoutSpotted { system, .. }
        | E::FoundingRaided { system, .. } | E::PirateEnclaveCleared { system, .. } => (Topic::Raid, Some(*system)),
        E::Vendetta { .. } => (Topic::Raid, None),
        E::StationEngaged { system, .. } | E::PlatformEngaged { system, .. } => (Topic::Battle, Some(*system)),
        E::RaidResolved { .. } | E::FlagshipDestroyed { .. } => (Topic::Battle, None),
        E::ResearchCompleted { .. } | E::TierUnlocked { .. } | E::ResearchStalled { .. } => (Topic::Research, None),
        E::ShipLaunched { system, .. } => (Topic::Launch, Some(*system)),
        E::SystemUpgraded { system, .. } | E::SettlementDeveloped { system, .. } => (Topic::Built, Some(*system)),
        E::SpecialistsDelivered { system, .. } | E::ModulesDelivered { system, .. }
        | E::MerchantArrived { system, .. } => (Topic::Arrival, Some(*system)),
        E::FuelRescueCompleted { .. } => (Topic::Arrival, None),
        E::Trade(TradeEvent::Delivered { system, .. }) => (Topic::Arrival, *system),
        E::Trade(TradeEvent::FreightMoved {
            system, stage: FreightStage::DeliveredToSystem | FreightStage::ArrivedAtWarehouse, ..
        }) => (Topic::Arrival, Some(*system)),
        E::Trade(TradeEvent::Sold { .. } | TradeEvent::Bought { .. } | TradeEvent::LimitFilled { .. }) => (Topic::Trade, None),
        E::SignalDetected { .. } | E::SignalReported { .. } => (Topic::Signal, None),
        E::ContractBoardPosted { .. } | E::ContractBoardBonus { .. } | E::OperationUpdated { .. } => (Topic::Board, None),
        E::MedalWon { .. } | E::SyndicateMedalWon { .. } => (Topic::Medal, None),
        E::SyndicateRungFinished { .. } | E::SyndicateBonusRaised { .. } => (Topic::Syndicate, None),
        E::CharterStipend { .. } => (Topic::Charter, None),
        _ => return None,
    })
}

/// Bad first, Info last.
fn rank(severity: TimelineSeverity) -> u8 {
    match severity {
        TimelineSeverity::Bad => 0,
        TimelineSeverity::Warn => 1,
        TimelineSeverity::Good => 2,
        TimelineSeverity::Info => 3,
    }
}

/// One topic's lines while away.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TopicTally {
    pub count: u32,
    /// The worst severity seen.
    pub worst: Option<TimelineSeverity>,
    /// The latest line, verbatim, and its severity: the state things are in now.
    pub latest: String,
    pub last: Option<TimelineSeverity>,
    /// The system the latest line that named one concerns.
    pub system: Option<EntityId>,
    /// §log: the goods these lines moved, summed (negative is lost).
    #[serde(default)]
    pub goods: BTreeMap<sim::Commodity, i64>,
    /// §log: the credits they reported, summed.
    #[serde(default)]
    pub credits: f64,
}

impl TopicTally {
    pub fn add(&mut self, severity: TimelineSeverity, text: &str, system: Option<EntityId>,
        data: Option<&crate::protocol::LogData>)
    {
        if let Some(data) = data {
            for (good, n) in &data.goods {
                *self.goods.entry(*good).or_default() += n;
            }
            self.credits += data.credits.unwrap_or(0.0);
        }
        if self.worst.is_none_or(|worst| rank(severity) < rank(worst)) {
            self.worst = Some(severity);
        }
        self.count += 1;
        self.latest = text.to_string();
        self.last = Some(severity);
        if system.is_some() {
            self.system = system;
        }
    }
}

/// A corporation's tallies since it was last online.
pub type AwayTally = BTreeMap<Topic, TopicTally>;

/// Where a line or action leads in the client.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WelcomeGo {
    System { id: EntityId },
    Shipyard { system: EntityId, body: u32 },
    Signals,
    Operations,
    Research,
    Comms,
    Market,
    Rankings,
    Syndicate,
    Log,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeLine {
    pub severity: TimelineSeverity,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub go: Option<WelcomeGo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeAction {
    pub label: String,
    pub go: WelcomeGo,
}

/// §welcome (R9): the report on return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeBackView {
    /// Sim seconds away.
    pub away: f64,
    /// What happened, worst first.
    pub happened: Vec<WelcomeLine>,
    /// What is waiting on the corporation now.
    pub waiting: Vec<WelcomeLine>,
    /// At most three, most urgent first.
    pub next: Vec<WelcomeAction>,
}

/// Everything the report reads, borrowed from the game loop.
pub struct WelcomeInputs<'a> {
    pub world: &'a World,
    pub player: PlayerId,
    /// When the corporation was last online (sim seconds).
    pub since: f64,
    pub tally: Option<&'a AwayTally>,
    pub history: &'a TransactionHistory,
    pub unread_mail: u32,
}

/// The report, or `None` for a short absence, a first visit, or a quiet one.
pub fn welcome_back(input: WelcomeInputs<'_>) -> Option<WelcomeBackView> {
    let world = input.world;
    let away = world.time - input.since;
    if input.since <= 0.0 || away < WELCOME_MIN_AWAY_S {
        return None;
    }
    let corp = world.players.get(&input.player)?;
    let name = |id: EntityId| world.systems.iter().find(|s| s.id == id).map_or_else(|| id.to_string(), |s| s.name.clone());

    // ── What happened ──
    let mut happened: Vec<(u8, Topic, WelcomeLine)> = Vec::new();
    if let Some(line) = trade_line(input.history, input.player, input.since) {
        happened.push((rank(TimelineSeverity::Good), Topic::Trade, line));
    }
    for (&topic, tally) in input.tally.into_iter().flatten() {
        if topic == Topic::Trade || tally.count == 0 {
            continue; // the receipts carry the exact sums
        }
        let severity = tally.worst.unwrap_or(TimelineSeverity::Info);
        // §log: a run of raids reports what they cost in all, not just the last.
        let lost: Vec<String> = tally.goods.iter().filter(|(_, n)| **n < 0)
            .map(|(good, n)| format!("{} {}", fmt_units(-n), crate::timeline::commodity_name(*good))).collect();
        let text = if tally.count == 1 {
            tally.latest.clone()
        } else if !lost.is_empty() {
            format!("{} ×{}: lost {} in all.", topic.noun(), tally.count, lost.join(", "))
        } else {
            format!("{} ×{}: {}", topic.noun(), tally.count, tally.latest)
        };
        let go = match topic {
            Topic::Research => Some(WelcomeGo::Research),
            Topic::Signal => Some(WelcomeGo::Signals),
            Topic::Board => Some(WelcomeGo::Operations),
            Topic::Medal => Some(WelcomeGo::Rankings),
            Topic::Syndicate => Some(WelcomeGo::Syndicate),
            Topic::Charter | Topic::Trade => None,
            _ => tally.system.map(|id| WelcomeGo::System { id }),
        };
        happened.push((rank(severity), topic, WelcomeLine { severity, text, go }));
    }
    happened.sort_by_key(|(rank, topic, _)| (*rank, *topic));
    let happened: Vec<WelcomeLine> = happened.into_iter().take(HAPPENED_MAX).map(|(_, _, line)| line).collect();

    // ── What is waiting ──
    let mut waiting = Vec::new();
    let mut next = Vec::new();
    // A siege or raid whose latest word is still bad comes first.
    for topic in [Topic::Siege, Topic::Raid] {
        if let Some(tally) = input.tally.and_then(|t| t.get(&topic))
            && tally.last.is_some_and(|s| rank(s) <= rank(TimelineSeverity::Warn))
            && let Some(id) = tally.system
            && next.iter().all(|a: &WelcomeAction| a.go != WelcomeGo::System { id })
        {
            next.push(WelcomeAction { label: format!("Defend {}", name(id)), go: WelcomeGo::System { id } });
        }
    }
    let answering: Vec<EntityId> = world.fleets.values()
        .filter(|f| f.owner == input.player)
        .filter_map(|f| match f.order {
            sim::FleetOrder::Expedition { site, task: sim::sites::ExpeditionTask::Signal, .. } => Some(site),
            _ => None,
        })
        .collect();
    let signals = corp.signals.signals.iter().filter(|s| s.open() && s.outcome.is_none() && !answering.contains(&s.id)).count();
    if signals > 0 {
        waiting.push(line(TimelineSeverity::Info, format!("{signals} Signal{} waiting", plural(signals)), WelcomeGo::Signals));
        next.push(WelcomeAction { label: "Answer a Signal".into(), go: WelcomeGo::Signals });
    }
    let academies: u32 = owned(world, input.player)
        .map(|s| s.bodies.iter().filter(|b| b.tier(sim::StructureKind::Academy) > 0).count() as u32)
        .sum();
    if academies > 0 && corp.research.active.is_none() {
        let what = if academies == 1 { "Academy idle".to_string() } else { format!("{academies} Academies idle") };
        waiting.push(line(TimelineSeverity::Warn, format!("{what}: no programme"), WelcomeGo::Research));
        next.push(WelcomeAction { label: "Pick a programme".into(), go: WelcomeGo::Research });
    }
    let board = world.operations.values()
        .filter(|o| o.board_day.is_some() && o.state == sim::operation::OperationState::Offered
            && o.is_visible_to(input.player)
            && matches!(o.scope, sim::operation::OperationScope::Private { player } if player == input.player))
        .count();
    if board > 0 {
        waiting.push(line(TimelineSeverity::Info, format!("{board} board contract{} open", plural(board)), WelcomeGo::Operations));
        next.push(WelcomeAction { label: "Take a contract".into(), go: WelcomeGo::Operations });
    }
    let idle_yards: Vec<(EntityId, u32)> = owned(world, input.player)
        .filter(|s| !world.build_queue.iter().any(|j| j.owner == input.player && j.system == s.id
            && matches!(j.what, sim::BuildKind::Ship { .. })))
        .filter_map(|s| s.bodies.iter()
            .find(|b| YARDS.iter().any(|&yard| b.tier(yard) > 0))
            .map(|b| (s.id, b.id)))
        .collect();
    if let Some(&(system, body)) = idle_yards.first() {
        let go = WelcomeGo::Shipyard { system, body };
        let text = if idle_yards.len() == 1 { format!("Yard idle at {}", name(system)) } else { format!("{} yards idle", idle_yards.len()) };
        waiting.push(line(TimelineSeverity::Info, text, go));
        next.push(WelcomeAction { label: format!("Queue a hull at {}", name(system)), go });
    }
    // §syndicate-projects (R6): points a manager could spend.
    if let Some(s) = corp.syndicate.and_then(|sid| world.syndicates.get(&sid))
        && s.may_manage_operations(input.player)
        && sim::syndicate::SyndicateBonus::ALL.iter().filter_map(|&b| s.projects.next_cost(b)).any(|cost| cost <= s.projects.points)
    {
        waiting.push(line(TimelineSeverity::Info, format!("{} project points to spend", s.projects.points), WelcomeGo::Syndicate));
        next.push(WelcomeAction { label: "Raise a syndicate bonus".into(), go: WelcomeGo::Syndicate });
    }
    if input.unread_mail > 0 {
        let n = input.unread_mail as usize;
        waiting.push(line(TimelineSeverity::Info, format!("{n} unread mail"), WelcomeGo::Comms));
        next.push(WelcomeAction { label: "Read mail".into(), go: WelcomeGo::Comms });
    }
    if happened.is_empty() && waiting.is_empty() && next.is_empty() {
        return None;
    }
    if next.is_empty() {
        next.push(WelcomeAction { label: "Open the Log".into(), go: WelcomeGo::Log });
    }
    next.truncate(3);
    Some(WelcomeBackView { away, happened, waiting, next })
}

/// The hull yards: ground Shipyard, Naval Drydock, Capital Slipway.
const YARDS: [sim::StructureKind; 3] =
    [sim::StructureKind::Shipyard, sim::StructureKind::NavalDrydock, sim::StructureKind::CapitalSlipway];

fn owned(world: &World, player: PlayerId) -> impl Iterator<Item = &sim::StarSystem> {
    world.systems.iter().filter(move |s| s.owner == Some(player))
}

fn line(severity: TimelineSeverity, text: String, go: WelcomeGo) -> WelcomeLine {
    WelcomeLine { severity, text, go: Some(go) }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// "Sold 1,240 ferrite ore, 300 alloys · +18,600 Cr" and its purchase twin,
/// from the receipts that reached command while away.
fn trade_line(history: &TransactionHistory, player: PlayerId, since: f64) -> Option<WelcomeLine> {
    let mut sold: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    let mut bought: BTreeMap<String, (u64, f64)> = BTreeMap::new();
    let add = |book: &mut BTreeMap<String, (u64, f64)>, item: String, units: u32, credits: f64| {
        let row = book.entry(item).or_default();
        row.0 += units as u64;
        row.1 += credits;
    };
    for entry in history.reported_after(player, since) {
        match &entry.details {
            TransactionDetails::Trade { trade } => match *trade {
                TradeEvent::Sold { commodity, units, unit_price, .. } =>
                    add(&mut sold, crate::timeline::commodity_name(commodity).into(), units, units as f64 * unit_price),
                TradeEvent::Bought { commodity, units, unit_price, .. } =>
                    add(&mut bought, crate::timeline::commodity_name(commodity).into(), units, units as f64 * unit_price),
                TradeEvent::LimitFilled { side, commodity, units, unit_price, .. } => {
                    let book = if side == sim::Side::Sell { &mut sold } else { &mut bought };
                    add(book, crate::timeline::commodity_name(commodity).into(), units, units as f64 * unit_price);
                }
                _ => {}
            },
            TransactionDetails::Sale { item, units, unit_price } => add(&mut sold, item.replace('_', " "), *units, *units as f64 * unit_price),
            TransactionDetails::Purchase { item, units, unit_price, fees, .. } =>
                add(&mut bought, item.replace('_', " "), *units, *units as f64 * unit_price + fees),
            _ => {}
        }
    }
    let part = |verb: &str, sign: &str, book: &BTreeMap<String, (u64, f64)>| -> Option<String> {
        if book.is_empty() {
            return None;
        }
        let mut rows: Vec<_> = book.iter().collect();
        rows.sort_by(|a, b| b.1.1.total_cmp(&a.1.1).then(a.0.cmp(b.0)));
        let total: f64 = rows.iter().map(|(_, (_, credits))| credits).sum();
        let mut items: Vec<String> = rows.iter().take(TRADE_ITEMS).map(|(item, (units, _))| format!("{} {item}", group(*units as f64))).collect();
        if rows.len() > TRADE_ITEMS {
            items.push(format!("+{} more", rows.len() - TRADE_ITEMS));
        }
        Some(format!("{verb} {} · {sign}{} Cr", items.join(", "), group(total)))
    };
    let parts: Vec<String> = [part("Sold", "+", &sold), part("Bought", "−", &bought)].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| WelcomeLine { severity: TimelineSeverity::Good, text: parts.join(" · "), go: Some(WelcomeGo::Market) })
}

/// 18600.4 → "18,600".
fn group(value: f64) -> String {
    let digits = (value.round() as i64).abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}


/// 25620 → "25,620".
fn fmt_units(n: i64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_puts_thousands_apart() {
        assert_eq!((group(0.0), group(999.4), group(18_600.0), group(1_234_567.0)), ("0".into(), "999".into(), "18,600".into(), "1,234,567".into()));
    }

    #[test]
    fn a_tally_keeps_the_worst_and_the_latest() {
        let mut tally = TopicTally::default();
        tally.add(TimelineSeverity::Bad, "Blockade at Vega.", Some(EntityId(4)), None);
        tally.add(TimelineSeverity::Good, "Blockade lifted at Vega.", None, None);
        assert_eq!((tally.count, tally.latest.as_str(), tally.system), (2, "Blockade lifted at Vega.", Some(EntityId(4))));
        assert!(matches!((tally.worst, tally.last), (Some(TimelineSeverity::Bad), Some(TimelineSeverity::Good))));
        // §log: the goods its lines moved add up.
        let lost = crate::protocol::LogData { goods: [(sim::Commodity::MetallicOre, -46)].into(), ..Default::default() };
        tally.add(TimelineSeverity::Bad, "Raided Vega.", None, Some(&lost));
        tally.add(TimelineSeverity::Bad, "Raided Vega.", None, Some(&lost));
        assert_eq!(tally.goods.get(&sim::Commodity::MetallicOre), Some(&-92));
    }
}

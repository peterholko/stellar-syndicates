//! The authoritative game loop — the heartbeat of the server (§14).
//!
//! A single Tokio task owns the [`World`] and the [`Sessions`] registry. Because
//! nothing else can touch them, there are no locks and no data races on game
//! state. The loop:
//!   1. ticks at a fixed [`TICK_HZ`] rate, advancing the world via the pure core;
//!   2. folds player intents / session events into sim commands at tick
//!      boundaries;
//!   3. pushes every connection its own per-player message (M1: the live tick;
//!      from M3: the delayed/fogged view);
//!   4. hands events and periodic snapshots to the off-hot-path persistence task.

use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, info};

use sim::{Command, PlayerId, World, DT, TICK_HZ};

use crate::persistence::{to_json, PersistJob, PersistenceHandle};
use crate::protocol::{
    BuildOptionView, ClientMsg, GalaxyInfo, InvSlot, MarketView, OrderView, PriceView,
    ServerMsg, StockSlot, SystemInfo, WalletView,
};
use crate::reports::ReportScheduler;
use crate::session::{ConnInfo, GameInput, ServerStatus, Sessions};
use crate::timeline::Timeline;
use crate::view::{self, PositionHistory, PriceHistory};

/// Push a per-player message every N sim ticks. At 30 Hz, N=3 → ~10 Hz network
/// updates: visibly live without flooding the socket.
const BROADCAST_EVERY: u64 = 3;

/// Default full-world snapshot cadence: every 10 s at the tick rate. Bounds how
/// much progress a restart can lose (the snapshot is the restart basis, §14).
pub const DEFAULT_SNAPSHOT_EVERY: u64 = 10 * TICK_HZ as u64;

/// A battle whose engagement has CONCLUDED in true space but whose conclusion
/// light hasn't yet reached every viewer (§battles-take-time). The sim removes
/// the engagement the instant it ends, so `active_battles()` drops it at the
/// TRUE end-time; but the aftermath report only lands `distance/c` later, when
/// the conclusion's light arrives. Without bridging that gap the "battle in
/// progress" icon vanishes FTL and, for the `distance/c` seconds until the
/// aftermath, the participant fleet ghosts (which the icon had been suppressing)
/// briefly re-appear at the site. We retain the concluded battle here so each
/// viewer keeps seeing the in-progress icon — and its participants suppressed —
/// until `ended_at + distance/c`, the exact instant the aftermath lands: a clean
/// in-progress → aftermath handoff with no re-appearing fleets.
struct ConcludedBattle {
    id: sim::EntityId,
    pos: sim::Vec2,
    started_at: f64,
    /// Sim-time the battle ended = the `RaidResolved` event time, so the icon's
    /// disappearance rides the SAME light wavefront as the aftermath report.
    ended_at: f64,
    a_owner: PlayerId,
    d_owner: PlayerId,
    participants: Vec<sim::EntityId>,
}

impl ConcludedBattle {
    /// Should a command center at `cc` still see this battle's IN-PROGRESS icon at
    /// wall-time `now`? True on the half-open window `[started_at + delay,
    /// ended_at + delay)` where `delay = |pos − cc| / c`:
    ///
    /// * the lower bound is the same light-gate the live icon used (never show a
    ///   battle whose start-light hasn't arrived), and
    /// * the upper bound is the conclusion's light-arrival — the exact instant the
    ///   aftermath report lands (`event_time + delay`), so the in-progress icon
    ///   flips to aftermath on one wavefront with neither gap nor overlap.
    fn shows_in_progress(&self, cc: sim::Vec2, c: f64, now: f64) -> bool {
        let delay = self.pos.distance(cc) / c;
        now >= self.started_at + delay && now < self.ended_at + delay
    }
}

struct GameLoop {
    world: World,
    sessions: Sessions,
    /// Per-player lightspeed view filter — keeps position history and builds
    /// each player's delayed/fogged view (§14).
    history: PositionHistory,
    /// Lagged hub-price ticker history (§9) — each player reads prices delayed
    /// by their light-distance from the hub.
    prices: PriceHistory,
    /// Delayed delivery of discrete reports (raid outcomes) — each player learns
    /// them on their own clock (§8).
    reports: ReportScheduler,
    /// Per-player retained check-in timeline (§16, Layer 3) — what became
    /// observable, buffered across disconnects, for the "welcome back" digest.
    timeline: Timeline,
    /// Last timeline length pushed to each player, so we only re-send when it grows.
    timeline_sent: HashMap<PlayerId, usize>,
    /// Battles that have concluded but whose conclusion light is still in flight
    /// to some viewer — kept so the in-progress icon lingers until the aftermath
    /// lands (see [`ConcludedBattle`]). Ephemeral awareness state, like `reports`.
    concluded_battles: Vec<ConcludedBattle>,
    /// Commands accumulated since the last tick, applied at the next boundary.
    pending: Vec<Command>,
    persistence: PersistenceHandle,
    /// Take a snapshot every this many ticks.
    snapshot_every: u64,
    /// Publishes server/ops status for the `/status` endpoint (meta channel).
    status_tx: watch::Sender<ServerStatus>,
}

impl GameLoop {
    fn new(
        world: World,
        persistence: PersistenceHandle,
        snapshot_every: u64,
        status_tx: watch::Sender<ServerStatus>,
    ) -> Self {
        let history = PositionHistory::for_world(&world);
        let prices = PriceHistory::for_world(&world);
        GameLoop {
            world,
            sessions: Sessions::new(),
            history,
            prices,
            reports: ReportScheduler::new(),
            timeline: Timeline::new(),
            timeline_sent: HashMap::new(),
            concluded_battles: Vec::new(),
            pending: Vec::new(),
            persistence,
            snapshot_every: snapshot_every.max(1),
            status_tx,
        }
    }

    /// Send the issuing player the outbound command-signal feedback for an order
    /// to one of THEIR ships. The comet's duration is the player's OBSERVED
    /// staleness of that ship (its ghost age), so it meets the ghost and reveals
    /// no true distance. Skipped if the player doesn't own the ship or it's
    /// currently dark to them.
    fn emit_command_signal(&self, player_id: PlayerId, ship_id: sim::EntityId) {
        let Some(corp) = self.world.players.get(&player_id) else {
            return;
        };
        let owns = self
            .world
            .fleets
            .get(&ship_id)
            .map(|s| s.owner == player_id)
            .unwrap_or(false);
        if !owns {
            return;
        }
        let cc = corp.command_center;
        let c = self.world.config.c;
        let now = self.world.time;
        // Observed one-way light delay to the ship (its ghost staleness). Falls
        // back to ~0 if just spawned at home. The order reaches the ship one delay
        // out — that's the whole outbound signal; the ship's reaction is then seen
        // directly on the map when its light arrives (no return signal needed).
        let age = self.history.observed_age(ship_id, cc, c, now).unwrap_or(0.0);
        self.sessions.send_to_player(
            player_id,
            ServerMsg::CommandSignal {
                ship_id,
                depart_time: now,
                arrive_time: now + age,
            },
        );
    }

    /// Publish current session/ops status (cheap; replaces the watched value).
    fn publish_status(&self) {
        let _ = self.status_tx.send(ServerStatus {
            online_players: self.sessions.online_player_count(),
            connections: self.sessions.connection_count(),
            tick: self.world.tick,
            sim_time: self.world.time,
        });
    }

    fn handle_input(&mut self, input: GameInput) {
        match input {
            GameInput::Connect {
                conn_id,
                player_id,
                name,
                outbound,
            } => {
                let newly_online = self.sessions.insert(
                    conn_id,
                    ConnInfo {
                        player_id,
                        name: name.clone(),
                        outbound,
                    },
                );
                // Greet this connection immediately with its identity, clock,
                // and the static galaxy geography.
                self.sessions.send_to_conn(
                    conn_id,
                    ServerMsg::Welcome {
                        player_id,
                        name: name.clone(),
                        protocol_version: crate::protocol::PROTOCOL_VERSION,
                        tick_hz: TICK_HZ,
                        tick: self.world.tick,
                        sim_time: self.world.time,
                        galaxy: GalaxyInfo {
                            hub: self.world.hub,
                            radius: self.world.config.galaxy_radius,
                            c: self.world.config.c,
                            sensor_range: self.world.config.sensor_range,
                            raider_speed: sim::ShipKind::Raider.max_speed(),
                            // Array-bubble tunables so the client renders its own
                            // arrays' coverage (§buildings step 2b).
                            // Scout bubble multiplier, for the client's coverage draw.
                            scout_sensor_mult: sim::ship::SCOUT_SENSOR_MULT,
                            sensor_array_base: sim::build::SENSOR_ARRAY_BASE,
                            sensor_array_per_tier: sim::build::SENSOR_ARRAY_PER_TIER,
                            // Platform protection radius, for the owner's own
                            // defended-system ring (§buildings step 2c).
                            defense_platform_radius: sim::build::DEFENSE_PLATFORM_RADIUS,
                            // §economy Part 2 colony tunables, for the owner-only
                            // population/food readout.
                            provisions_per_million_per_s: sim::colony::PROVISIONS_PER_MILLION_PER_S,
                            pop_cap_per_habitat_tier: sim::colony::POP_CAP_PER_HABITAT_TIER,
                            pop_growth_per_s: sim::colony::POP_GROWTH_PER_S,
                            specialist_hire_cost: sim::specialist::SPECIALIST_HIRE_COST,
                            // §economy Part 3: the refinery hint rate (full converter table on the wire in Part 6).
                            fuel_refinery_rate: sim::production::converter_for(sim::StructureKind::FuelRefinery).expect("refinery converts").rate,
                            // §contestable-territory Part 2: the siege duration.
                            siege_secs: self.world.siege_duration_secs(),
                            pirate_id: sim::PlayerId::PIRATE,
                            // §node: the awakening countdown + region radius so the
                            // client can telegraph and draw the holder's region ring.
                            node_awakening_time: self.world.config.node_awakening_time,
                            node_region_radius: sim::NODE_REGION_RADIUS,
                            // Static geography + geology (deposits, claim cost).
                            // Dynamic ownership/stockpile comes light-gated in View.
                            // §explore: PUBLIC geography only — the exact deposits
                            // are corp knowledge now (SystemStateView.deposits,
                            // surveyed-or-owner); the free spectral read is the BAND.
                            systems: system_infos(&self.world),
                            // What can be built + each recipe's cost/time (§step1).
                            build_options: build_options(),
                        },
                    },
                );
                // Welcome-back: the check-in digest of what became observable while
                // away (§16, Layer 3). `away_since` is their last-online time, so the
                // client can mark entries newer than it as "while you were away".
                let (entries, away_since) = self.timeline.digest(player_id);
                self.timeline_sent.insert(player_id, entries.len());
                self.sessions
                    .send_to_conn(conn_id, ServerMsg::Timeline { entries, away_since });

                // Ensure the corporation exists in the sim (idempotent).
                self.pending.push(Command::AddPlayer {
                    id: player_id,
                    name,
                });
                info!(
                    %player_id, conn_id, newly_online,
                    online_players = self.sessions.online_player_count(),
                    connections = self.sessions.connection_count(),
                    "player connected"
                );
            }
            GameInput::Disconnect { conn_id } => {
                if let Some((player_id, now_offline)) = self.sessions.remove(conn_id) {
                    info!(
                        %player_id, conn_id, now_offline,
                        online_players = self.sessions.online_player_count(),
                        "player disconnected"
                    );
                }
            }
            GameInput::Intent { conn_id, msg } => match msg {
                ClientMsg::Ping => {
                    debug!(conn_id, "ping");
                }
                ClientMsg::MoveShip { ship_id, dest } => {
                    // Attach the issuing player (the sim enforces ownership).
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, ship_id);
                        self.pending.push(Command::MoveShip {
                            player_id,
                            ship_id,
                            dest,
                        });
                    }
                }
                ClientMsg::CommitRaid { raider_id, target_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, raider_id);
                        self.pending.push(Command::CommitRaid {
                            player_id,
                            raider_id,
                            target_id,
                        });
                    }
                }
                ClientMsg::BlockadeSystem { fleet_id, system_id } => {
                    // §contestable-territory Part 1: light-delayed like a move.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, fleet_id);
                        self.pending.push(Command::BlockadeSystem { player_id, fleet_id, system_id });
                    }
                }
                ClientMsg::SurveySystem { fleet_id, system_id } => {
                    // §explore Part 2: light-delayed like a move.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, fleet_id);
                        self.pending.push(Command::SurveySystem { player_id, fleet_id, system_id });
                    }
                }
                ClientMsg::AttackFleet { fleet_id, target_id } => {
                    // §offensive-orders Part 1: light-delayed like a raid.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, fleet_id);
                        self.pending.push(Command::AttackFleet { player_id, fleet_id, target_id });
                    }
                }
                ClientMsg::SetFleetPosture { fleet_id, posture } => {
                    // §offensive-orders Part 2: instant per-fleet standing policy.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetPosture { player_id, fleet_id, posture });
                    }
                }
                // §syndicates Part 1: instant owner-only alliance admin.
                ClientMsg::CreateSyndicate { name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::CreateSyndicate { player_id, name });
                    }
                }
                ClientMsg::InviteToSyndicate { name } => {
                    // Invite BY NAME: the invitee's stable id IS the hash of their
                    // corp name (the same function `Join` uses), so the server can
                    // resolve it without exposing a corp directory. A non-joined
                    // name resolves to an id the sim soft-rejects.
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        let invitee = crate::protocol::player_id_from_name(&name);
                        self.pending.push(Command::InviteToSyndicate { player_id, invitee });
                    }
                }
                ClientMsg::AcceptSyndicateInvite { syndicate_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::AcceptSyndicateInvite { player_id, syndicate_id });
                    }
                }
                ClientMsg::LeaveSyndicate => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::LeaveSyndicate { player_id });
                    }
                }
                ClientMsg::DissolveSyndicate => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DissolveSyndicate { player_id });
                    }
                }
                ClientMsg::SetResearchQueue { queue } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetResearchQueue { player_id, queue });
                    }
                }
                ClientMsg::SaveFit { name, ship, loadout } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SaveFit { player_id, name, ship, loadout });
                    }
                }
                ClientMsg::DeleteFit { name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DeleteFit { player_id, name });
                    }
                }
                ClientMsg::NameFlagship { name } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::NameFlagship { player_id, name });
                    }
                }
                ClientMsg::RecallRaid { raider_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.emit_command_signal(player_id, raider_id);
                        self.pending.push(Command::RecallRaid { player_id, raider_id });
                    }
                }
                ClientMsg::MarketBuy { commodity, units, ship_to } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MarketBuy { player_id, commodity, units, ship_to });
                    }
                }
                ClientMsg::HubLoad { fleet_id, commodity, units } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HubLoad { player_id, fleet_id, commodity, units });
                    }
                }
                ClientMsg::HubUnload { fleet_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HubUnload { player_id, fleet_id });
                    }
                }
                ClientMsg::SystemLoad { fleet_id, system, commodity, units } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SystemLoad { player_id, fleet_id, system, commodity, units });
                    }
                }
                ClientMsg::SystemUnload { fleet_id, system } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SystemUnload { player_id, fleet_id, system });
                    }
                }
                ClientMsg::HaulToCharterhouse { fleet_id, sell_on_arrival } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HaulToCharterhouse { player_id, fleet_id, sell_on_arrival });
                    }
                }
                ClientMsg::PayReinstatement { points } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::PayReinstatement { player_id, points });
                    }
                }
                ClientMsg::SetEngageFreight { fleet_id, on } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetEngageFreight { player_id, fleet_id, on });
                    }
                }
                ClientMsg::BookFreightOut { system, commodity, units } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BookFreightOut { player_id, system, commodity, units });
                    }
                }
                ClientMsg::BookFreightIn { system, commodity, units, sell_on_arrival } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BookFreightIn { player_id, system, commodity, units, sell_on_arrival });
                    }
                }
                ClientMsg::MarketSell { commodity, units } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MarketSell { player_id, commodity, units });
                    }
                }
                ClientMsg::PlaceLimitOrder { side, commodity, units, limit_price } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::PlaceLimitOrder {
                            player_id,
                            side,
                            commodity,
                            units,
                            limit_price,
                        });
                    }
                }
                ClientMsg::ShipProduction { system_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ShipProduction { player_id, system_id });
                    }
                }
                ClientMsg::StockSystem { system_id, commodity, units } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::StockSystem { player_id, system_id, commodity, units });
                    }
                }
                ClientMsg::SetStandingOrder { order } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetStandingOrder { player_id, order });
                    }
                }
                ClientMsg::ClearStandingOrder { order_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::ClearStandingOrder { player_id, order_id });
                    }
                }
                ClientMsg::SetFleetDoctrine { doctrine } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetDoctrine { player_id, doctrine });
                    }
                }
                ClientMsg::BuildShip { system_id, ship_kind, join, loadout } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BuildShip { player_id, system_id, ship_kind, join, loadout });
                    }
                }
                ClientMsg::BuildModule { system_id, module } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BuildModule { player_id, system_id, module });
                    }
                }
                ClientMsg::RefitShips { fleet_id, ship, from, to, n } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::RefitShips { player_id, fleet_id, ship, from, to, n });
                    }
                }
                ClientMsg::TransferModules { from, to, manifest } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::TransferModules { player_id, from, to, manifest });
                    }
                }
                ClientMsg::BuyModule { module, n, dest_system } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::BuyModule { player_id, module, n, dest_system });
                    }
                }
                ClientMsg::SellModule { module, n, from_system } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SellModule { player_id, module, n, from_system });
                    }
                }
                ClientMsg::DevelopSystem { system_id, upgrade, body_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::DevelopSystem { player_id, system_id, upgrade, body_id });
                    }
                }
                ClientMsg::SetAssignment { system_id, structure, workers, specialists, body_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetAssignment { player_id, system_id, structure, workers, specialists, body_id });
                    }
                }
                ClientMsg::HireSpecialist { specialist, dest_system } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::HireSpecialist { player_id, specialist, dest_system });
                    }
                }
                ClientMsg::TrainSpecialist { system_id, specialist } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::TrainSpecialist { player_id, system_id, specialist });
                    }
                }
                ClientMsg::TransferSpecialists { from, to, manifest } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::TransferSpecialists { player_id, from, to, manifest });
                    }
                }
                ClientMsg::Withdraw { fleet_id } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::Withdraw { player_id, fleet_id });
                    }
                }
                ClientMsg::SetFleetTransit { fleet_id, mode } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SetFleetTransit { player_id, fleet_id, mode });
                    }
                }
                ClientMsg::MergeFleets { into, from } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::MergeFleets { player_id, into, from });
                    }
                }
                ClientMsg::SplitFleet { fleet_id, counts } => {
                    if let Some(player_id) = self.sessions.player_of(conn_id) {
                        self.pending.push(Command::SplitFleet { player_id, fleet_id, counts });
                    }
                }
                ClientMsg::EstimateEngagement { attacker, target } => {
                    // A read-only QUERY (§FLEETS Part 3): compute the projection
                    // from this player's OWN view and reply immediately. Touches
                    // no authoritative state.
                    if let Some(player_id) = self.sessions.player_of(conn_id)
                        && let Some(corp) = self.world.players.get(&player_id)
                    {
                        let cc = corp.command_center;
                        let c = self.world.config.c;
                        let now = self.world.time;
                        let arrays = self.world.array_sensor_sources(player_id);
                        if let Some(est) = crate::estimate::estimate_engagement(
                            &self.world, &self.history, player_id, cc, c, now, &arrays, attacker, target,
                        ) {
                            self.sessions.send_to_conn(conn_id, ServerMsg::EngagementEstimate(est));
                        }
                    }
                }
                // Join is handled at the WebSocket layer before the loop ever
                // sees intents on this connection; ignore a stray re-join.
                ClientMsg::Join { .. } => {
                    debug!(conn_id, "ignoring redundant join intent");
                }
            },
        }
    }

    /// Advance one tick: apply pending commands, integrate, persist, broadcast.
    fn tick(&mut self) {
        let commands = std::mem::take(&mut self.pending);
        // Snapshot the battles active BEFORE this step, keyed by id — any that are
        // gone AFTER the step concluded this tick, and we retain them so their
        // in-progress icon lingers until each viewer's conclusion light arrives
        // (§battles-take-time; see [`ConcludedBattle`]).
        let before: HashMap<sim::EntityId, sim::BattleInfo> = self
            .world
            .active_battles()
            .into_iter()
            .map(|b| (b.id, b))
            .collect();
        let systems_before = self.world.systems.len();
        let events = self.world.step(&commands);
        // §over-capacity homes: a join past the pre-generated slot pool MINTS a
        // new home system mid-run — public geography that every connected
        // client's Welcome snapshot predates. Re-broadcast the star chart so
        // the new star is drawable and selectable everywhere (not least by its
        // own new owner, whose first click otherwise falls through to the
        // command-center anchor).
        if self.world.systems.len() != systems_before {
            let update = ServerMsg::GalaxyUpdate { systems: system_infos(&self.world) };
            for (_conn_id, info) in self.sessions.iter_conns() {
                let _ = info.outbound.try_send(update.clone());
            }
        }
        // Every battle ends inside `resolve_raids`, which runs BEFORE the clock
        // advances in `step`; so a battle that concluded this tick ended at
        // `world.time - DT` — exactly the `RaidResolved` event time the aftermath
        // report is stamped with. Riding that same instant makes the icon's
        // disappearance and the aftermath's arrival one light wavefront.
        if !before.is_empty() {
            let ended_at = self.world.time - DT;
            let still_active: std::collections::BTreeSet<sim::EntityId> =
                self.world.active_battles().iter().map(|b| b.id).collect();
            for (id, b) in before {
                if !still_active.contains(&id) {
                    self.concluded_battles.push(ConcludedBattle {
                        id,
                        pos: b.pos,
                        started_at: b.started_at,
                        ended_at,
                        a_owner: b.a_owner,
                        d_owner: b.d_owner,
                        participants: b.participants,
                    });
                }
            }
        }
        // Drop concluded battles whose conclusion light has reached even the
        // farthest possible viewer (galaxy diameter / c) — their icon has flipped
        // to aftermath everywhere, so nothing more references them.
        if !self.concluded_battles.is_empty() {
            let max_delay = (2.0 * self.world.config.galaxy_radius) / self.world.config.c;
            let now = self.world.time;
            self.concluded_battles
                .retain(|cb| now - cb.ended_at <= max_delay + 1.0);
        }

        // Record true positions into the view filter's history every tick so
        // the retarded-time boundary resolves at full temporal resolution.
        self.history.record(&self.world);
        self.prices.record(&self.world);
        // Queue any discrete events (raid outcomes) for delayed per-player
        // delivery.
        self.reports.ingest(&events);
        // Record events into the per-player check-in timeline (§16, Layer 3) at
        // their observable time, then promote any whose light has now arrived —
        // for ALL players, online or off (offline buffering is the whole point).
        self.timeline.ingest(&events, &self.world);
        self.timeline.promote(self.world.time);
        for ev in &events {
            match &ev.payload {
                // Route economy news to the owning player immediately (their own
                // action / a delivery at their doorstep).
                sim::EventPayload::Trade(te) => {
                    self.sessions
                        .send_to_player(te.player(), ServerMsg::Trade { trade: *te });
                }
                // A ship was destroyed in true space: tell the view filter so it
                // keeps serving the ghost until each player's light arrives, then
                // vanishes it (delayed, per-viewer — never FTL).
                sim::EventPayload::ShipDestroyed { ship, pos, .. } => {
                    self.history.mark_destroyed(*ship, ev.time, *pos);
                }
                _ => {}
            }
        }

        // Off-hot-path: append events to the log.
        if !events.is_empty() {
            let payloads = events.iter().map(to_json).collect::<Vec<_>>();
            self.persistence.submit(PersistJob::Events {
                tick: self.world.tick,
                time: self.world.time,
                events: payloads,
            });
        }

        if self.world.tick.is_multiple_of(BROADCAST_EVERY) {
            self.broadcast();
            self.publish_status();
        }

        if self.world.tick.is_multiple_of(self.snapshot_every) {
            self.persistence.submit(PersistJob::Snapshot {
                tick: self.world.tick,
                time: self.world.time,
                world: to_json(&self.world),
            });
        }
    }

    /// Push every connection its own per-player delayed/fogged view, each
    /// computed from THAT player's command center (§6, §14). No player ever
    /// receives true positions or another player's view — the fairness
    /// guarantee, enforced by [`PositionHistory::view_for`].
    fn broadcast(&mut self) {
        let c = self.world.config.c;
        let now = self.world.time;
        let tick = self.world.tick;
        let hub = self.world.hub;

        // Build each online player's view ONCE (shared across their
        // connections), plus any delayed reports whose light has now reached
        // them. Everything is computed from THIS player's command center and
        // light-gated. A connection whose corporation isn't in the world yet
        // (AddPlayer not processed) simply gets nothing this tick.
        let mut views: HashMap<PlayerId, ServerMsg> = HashMap::new();
        let mut reports: HashMap<PlayerId, Vec<ServerMsg>> = HashMap::new();
        let mut timelines: HashMap<PlayerId, ServerMsg> = HashMap::new();
        for player_id in self.sessions.online_players() {
            let Some(corp) = self.world.players.get(&player_id) else {
                continue;
            };
            let cc = corp.command_center;
            // The viewer's standing SENSOR-ARRAY bubbles (§buildings step 2b) join
            // their coverage — same shared source of truth as the sim's pickets.
            let arrays = self.world.array_sensor_sources(player_id);
            // BATTLES (§battles-take-time), STRICTLY light-gated: a battle (and its
            // participants, revealed by weapons fire) appears only once the light
            // of its start has reached THIS player's command center.
            let mut battles: Vec<crate::protocol::BattleView> = Vec::new();
            let mut battle_reveal: std::collections::BTreeSet<sim::EntityId> = std::collections::BTreeSet::new();
            for b in self.world.active_battles() {
                let delay = b.pos.distance(cc) / c;
                if now >= b.started_at + delay {
                    battle_reveal.extend(b.participants.iter().copied());
                    battles.push(crate::protocol::BattleView {
                        id: b.id,
                        pos: b.pos,
                        age: delay,
                        started_at: b.started_at,
                        own: player_id == b.a_owner || player_id == b.d_owner,
                        // All participants are revealed to any observer of the
                        // battle (the weapons-fire site-reveal above), so their
                        // ids carry no more than the ghosts already sent.
                        participants: b.participants,
                    });
                }
            }
            // CONCLUDED battles whose conclusion light hasn't arrived yet: keep
            // showing the in-progress icon (and suppressing the participant ghosts
            // via `battle_reveal`) until `ended_at + delay` — the exact instant the
            // aftermath report lands. This bridges the FTL gap that used to let the
            // participant fleet icons re-appear between "battle ends" and "aftermath
            // arrives" (§battles-take-time). The `started_at + delay` lower bound
            // means a viewer whose START light never arrived (battle began and ended
            // faster than its light could reach them) still never sees a phantom icon.
            for cb in &self.concluded_battles {
                if cb.shows_in_progress(cc, c, now) {
                    battle_reveal.extend(cb.participants.iter().copied());
                    battles.push(crate::protocol::BattleView {
                        id: cb.id,
                        pos: cb.pos,
                        age: cb.pos.distance(cc) / c,
                        started_at: cb.started_at,
                        own: player_id == cb.a_owner || player_id == cb.d_owner,
                        participants: cb.participants.clone(),
                    });
                }
            }
            // §node: this viewer's regional dark-fleet effects (Veil quiets its
            // holders' dark fleets; Deep Scan resolves exact composition in-region).
            let veil_regions = self.world.active_veil_regions();
            let deep_scan_regions = self.world.deep_scan_regions(player_id);
            let mut ghosts = self.history.view_for_with_arrays(
                player_id, cc, c, now, &arrays, &battle_reveal,
                view::NodeEffects { veil: &veil_regions, deep_scan: &deep_scan_regions },
            );
            // §offensive-orders Part 2: attach each OWN fleet's engagement posture
            // (owner-only, fresh — a private standing policy like the corp doctrine;
            // rivals keep `None`, so it never leaks). The history-view can't see the
            // authoritative fleet, so fill it from the world here.
            for g in ghosts.iter_mut() {
                if g.own {
                    g.posture = self.world.fleets.get(&g.id).map(|f| f.posture);
                    // §TCA: owner-only blockade policy — does this fleet engage
                    // Authority freight arriving at the system it strangles?
                    // §TCA: the engage-freight choice is only MEANINGFUL for a
                    // fleet holding a Blockade (that's the only order the sim
                    // consults it under) — expose it only there, so the client
                    // never offers a toggle that does nothing.
                    g.engage_freight = self.world.fleets.get(&g.id).and_then(|f| {
                        matches!(f.order, sim::FleetOrder::Blockade { .. }).then_some(f.engage_freight)
                    });
                    // §syndicates Part 3: OWNER-ONLY garrison status — if this fleet
                    // is stationed as an ally garrison, its host + fed state.
                    if let Some(host) = self.world.garrison_host_of(g.id) {
                        g.garrison_host = Some(host);
                        g.garrison_fed = self.world.fleets.get(&g.id).is_some_and(|f| f.garrison_fed);
                    }
                    // §explore Part 2: OWNER-ONLY survey-dwell progress (0..1) for
                    // the progress ring — a rival never sees your order state.
                    g.survey_progress = self.world.fleets.get(&g.id).and_then(|f| match f.order {
                        sim::FleetOrder::Survey { dwell_since: Some(since), .. } => {
                            Some(((now - since) / sim::explore::SURVEY_SECS).clamp(0.0, 1.0))
                        }
                        _ => None,
                    });
                }
                // §syndicates Part 1: friendly ALLY tint — the owner (already on
                // the ghost) is a syndicate member as THIS viewer knows it
                // (light-delayed membership; `known_ally` returns false for own).
                g.ally = self.world.known_ally(player_id, g.owner, now);
                // §TCA: an Authority freighter's MANIFEST is two-tier PER ENTRY —
                // your own lots are always yours to see (they're your property),
                // everyone else's only from inside sensor range (`revealed`, the
                // same Tier-2 gate that governs a convoy's cargo). A distant rival
                // sees the hull go by and learns nothing about who ships what.
                if let Some(run) = self.world.freight_runs.get(&g.id) {
                    g.manifest = crate::view::visible_manifest(run, player_id, g.revealed);
                }
            }
            // §battle-aftermath: this player's RETAINED concluded-battle reports
            // (delivered = their light provably arrived). Strictly per-
            // participant — the scheduler holds them keyed by recipient.
            let battle_reports: Vec<crate::protocol::BattleReportView> = self
                .reports
                .retained_for(player_id)
                .iter()
                .map(|r| crate::protocol::BattleReportView {
                    id: r.id,
                    pos: r.pos,
                    at_time: r.event_time,
                    learned_at: r.arrival_time,
                    you: r.you,
                    attacker_kind: r.attacker_kind,
                    target_kind: r.target_kind,
                    outcome: r.outcome,
                    attacker_losses: r.attacker_losses.clone(),
                    target_losses: r.target_losses.clone(),
                })
                .collect();
            // §contestable-territory Part 2: retained CAPTURE reports (per-participant).
            let capture_reports: Vec<crate::protocol::CaptureReportView> = self
                .reports
                .retained_captures_for(player_id)
                .iter()
                .map(|r| crate::protocol::CaptureReportView {
                    id: r.id,
                    pos: r.pos,
                    at_time: r.event_time,
                    learned_at: r.arrival_time,
                    captor: r.captor,
                    plunder: r.plunder.clone(),
                })
                .collect();
            let anchors = view::filter_anchors(&self.world.home_slots, player_id, cc, c, now);
            // §syndicates Part 2: each syndicate ally's relayable scout intel (their
            // command center is the relay source). The View chain-light-delays each
            // ally's snapshots to this viewer, provenance preserved.
            let ally_intel: Vec<view::AllyIntel> = self
                .world
                .allies_of(player_id)
                .iter()
                .filter_map(|a| {
                    self.world
                        .players
                        .get(a)
                        .map(|ac| view::AllyIntel { id: *a, cc: ac.command_center, intel: &ac.intel })
                })
                .collect();
            let mut systems = view::filter_systems(
                &self.world.systems, player_id, cc, c, now, &self.world.build_queue, self.world.tick, DT,
                &corp.intel, &ally_intel, &corp.surveyed,
            );
            // §syndicates Part 1: friendly ALLY tint on systems whose (light-gated
            // known) owner is a syndicate member as THIS viewer knows it. Composes
            // both light-gates; grants no owner-only data (Part 1 is tint only).
            for sv in systems.iter_mut() {
                sv.ally = sv.owner.is_some_and(|o| self.world.known_ally(player_id, o, now));
                // §syndicates Part 3: OWNER-ONLY hosted-garrison indicator (the
                // coalition shield you're feeding). Only for your OWN systems.
                if sv.owner == Some(player_id)
                    && let Some((ships, fed)) = self.world.hosted_garrison(sv.id)
                {
                    sv.ally_garrison_ships = ships;
                    sv.ally_garrison_fed = fed;
                }
                // §node: attach the system's EXOTIC NODE, if any. Bonus + awakened
                // are PUBLIC (an awakened node is a galaxy-wide landmark; its awaken
                // time is public config, so the flag leaks nothing); `fed` and the
                // region ring are OWNER-ONLY.
                if let Some(n) = self.world.nodes.get(&sv.id) {
                    let own = sv.owner == Some(player_id);
                    sv.node = Some(crate::protocol::NodeStateView {
                        bonus: n.bonus.slug().to_string(),
                        title: n.bonus.title().to_string(),
                        awakened: n.awakened,
                        fed: own && n.fed,
                        region_radius: if own { sim::NODE_REGION_RADIUS } else { 0.0 },
                    });
                }
            }
            // §syndicates Part 1: the viewer's OWN roster + pending invites (fresh
            // private state, never a rival's private roster).
            let syndicate = corp
                .syndicate
                .and_then(|sid| self.world.syndicates.get(&sid))
                .map(|s| Box::new(crate::protocol::SyndicateView {
                    id: s.id,
                    name: s.name.clone(),
                    founder: s.founder,
                    is_founder: s.founder == player_id,
                    members: s
                        .members
                        .iter()
                        .map(|m| crate::protocol::SyndicateMember {
                            id: *m,
                            name: self.world.players.get(m).map(|c| c.name.clone()).unwrap_or_default(),
                        })
                        .collect(),
                    invited: s
                        .invites
                        .iter()
                        .filter_map(|i| self.world.players.get(i).map(|c| c.name.clone()))
                        .collect(),
                    // §fitting: the shared doctrine-fit library (owner-only).
                    fits: s
                        .fits
                        .iter()
                        .map(|f| crate::protocol::FitView {
                            name: f.name.clone(),
                            kind: f.kind,
                            modules: f.loadout.modules().to_vec(),
                        })
                        .collect(),
                    // §ladder B4: the christened Titan (owner-only here).
                    flagship_name: s.flagship_name.clone(),
                }));
            let syndicate_invites: Vec<crate::protocol::SyndicateInviteView> = self
                .world
                .syndicates
                .values()
                .filter(|s| s.invites.contains(&player_id))
                .map(|s| crate::protocol::SyndicateInviteView { id: s.id, name: s.name.clone() })
                .collect();
            // §research R6: the viewer's OWN research picture (owner-only), present
            // only while affiliated (research is a syndicate institution).
            let research = corp
                .syndicate
                .filter(|sid| self.world.syndicates.contains_key(sid))
                .map(|sid| Box::new(research_view(&self.world, sid)));

            // Lagged hub ticker: prices as of the light that has reached this
            // player's command center from the hub.
            let staleness = hub.distance(cc) / c;
            let lagged = self.prices.at(now - staleness);
            let prices = lagged
                .map(|m| {
                    m.iter()
                        .map(|(commodity, price)| PriceView { commodity: *commodity, price: *price })
                        .collect()
                })
                .unwrap_or_default();
            let market = MarketView { prices, staleness };

            // The player's own wallet — fresh (their local treasury + holdings +
            // resting limit orders).
            // §TCA Phase 2: the player's own charter standing. The BAND is always
            // derived (never stored), so this can't desync from the sim.
            let standing = corp.tca_standing;
            let charter = crate::protocol::CharterView {
                standing,
                max_standing: sim::tca::TCA_STANDING_MAX,
                status: sim::charter_status(standing),
                title: sim::charter_status(standing).title(),
                ladder: sim::tca::status_ladder().to_vec(),
                tariff_mult: sim::tca::tariff_mult(standing),
                market_penalty_frac: sim::tca::market_penalty_frac(standing),
                reinstate_cost_per_point: sim::tca::TCA_REINSTATE_FEE_PER_POINT,
            };

            let wallet = WalletView {
                credits: corp.credits,
                valuation: corp.valuation,
                inventory: corp
                    .inventory
                    .iter()
                    .map(|(commodity, units)| InvSlot { commodity: *commodity, units: *units })
                    .collect(),
                // §TCA: goods at the Charterhouse — what the Exchange trades against.
                warehouse: corp
                    .warehouse
                    .iter()
                    .map(|(commodity, units)| InvSlot { commodity: *commodity, units: *units })
                    .collect(),
                orders: self
                    .world
                    .book
                    .iter()
                    .filter(|o| o.player == player_id)
                    .map(|o| OrderView {
                        id: o.id,
                        side: o.side,
                        commodity: o.commodity,
                        units: o.units,
                        limit_price: o.limit_price,
                    })
                    .collect(),
                // The fleet's fuel reserve: sum Fuel across this player's systems
                // (owner-only — read off systems we own, so it never leaks).
                fuel_total: self
                    .world
                    .systems
                    .iter()
                    .filter(|s| s.owner == Some(player_id))
                    .map(|s| s.stockpile.get(&sim::Commodity::Fuel).copied().unwrap_or(0.0))
                    .sum(),
            };

            // §battle-records A2: the viewer's CURRENT sensor coverage (command
            // center + standing arrays + their own fleets' bubbles) gates a
            // third party's bucket access to a battle site.
            let mut coverage: Vec<(sim::Vec2, f64)> = vec![(cc, self.world.config.sensor_range)];
            coverage.extend_from_slice(&arrays);
            for f in self.world.fleets.values() {
                if f.owner == player_id {
                    coverage.push((f.pos, self.world.config.sensor_range * f.sensor_mult()));
                }
            }
            let battle_records =
                view::battle_record_views_named(&self.world.battle_records, player_id, cc, c, now, &coverage, &|corp| {
                    // §ladder B4: resolve a side's christened Titan name.
                    self.world
                        .players
                        .get(&corp)
                        .and_then(|p| p.syndicate)
                        .and_then(|sid| self.world.syndicates.get(&sid))
                        .and_then(|s| s.flagship_name.clone())
                });
            // §TCA: the Charterhouse freight desk. Terms for every system this
            // player owns (the only valid destinations), plus their OWN lots.
            let freight = crate::protocol::FreightView {
                next_departure: self.world.next_freight_departure(),
                period: self.world.freight_period_secs(),
                fee_frac: sim::tca::TCA_FREIGHT_FEE_FRAC,
                fee_per_unit_dist: sim::tca::TCA_FREIGHT_FEE_PER_UNIT_DIST,
                depot_fee_mult: sim::tca::TCA_DEPOT_FEE_MULT,
                terms: self
                    .world
                    .systems
                    .iter()
                    .filter(|s| s.owner == Some(player_id))
                    .map(|s| {
                        let distance = hub.distance(s.pos);
                        let depot = s.tier(sim::StructureKind::Depot) > 0;
                        let secs_out = sim::World::freight_flight_secs(distance);
                        crate::protocol::FreightTermsView {
                            system: s.id,
                            distance,
                            depot,
                            cap: sim::tca::shipment_cap(depot),
                            secs_out,
                            secs_round: secs_out * 2.0,
                        }
                    })
                    .collect(),
                shipments: self
                    .world
                    .shipments_of(player_id)
                    .into_iter()
                    .map(|(s, aboard)| crate::protocol::ShipmentView {
                        id: s.id.0,
                        system: s.system,
                        commodity: s.commodity,
                        units: s.units,
                        direction: s.direction,
                        sell_on_arrival: s.sell_on_arrival,
                        fee_paid: s.fee_paid,
                        booked_at: s.booked_at,
                        aboard,
                    })
                    .collect(),
            };

            views.insert(
                player_id,
                ServerMsg::View {
                    tick,
                    sim_time: now,
                    command_center: cc,
                    anchors,
                    systems,
                    ghosts,
                    market,
                    wallet,
                    charter,
                    freight,
                    // The player's own standing orders (fresh — private policy, not
                    // light-gated), so the client can list/edit them.
                    standing_orders: corp.standing_orders.clone(),
                    // The player's own fleet doctrine (fresh private policy).
                    doctrine: corp.doctrine,
                    // The player's own in-flight order lifecycles (§order-lifecycle)
                    // — owner-only private command data, like the wallet.
                    pending_orders: self
                        .world
                        .pending_commands(player_id)
                        .into_iter()
                        .map(|p| crate::protocol::PendingOrderView {
                            fleet_id: p.fleet,
                            delivered_at: p.delivered_at,
                            echo_at: p.echo_at,
                            kind: p.kind,
                        })
                        .collect(),
                    battles,
                    battle_reports,
                    capture_reports,
                    battle_records,
                    syndicate,
                    syndicate_invites,
                    research,
                    // §rankings: the published leaderboard — public, identical for
                    // every player, a verbatim copy of the sim's last ledger close.
                    rankings: self.world.rankings.clone(),
                },
            );
            let due = self.reports.due_for(player_id, cc, c, now);
            if !due.is_empty() {
                reports.insert(
                    player_id,
                    due.into_iter().map(|r| ServerMsg::Report { report: r }).collect(),
                );
            }

            // Mark the player online (advances their "away" boundary), and if their
            // check-in timeline gained entries since we last pushed (e.g. an
            // auto-dispatch or a battle whose light just arrived), re-send the digest.
            self.timeline.mark_seen(player_id, now);
            let jlen = self.timeline.journal_len(player_id);
            if self.timeline_sent.get(&player_id).copied().unwrap_or(0) != jlen {
                self.timeline_sent.insert(player_id, jlen);
                let (entries, away_since) = self.timeline.digest(player_id);
                timelines.insert(player_id, ServerMsg::Timeline { entries, away_since });
            }
        }

        for (_conn_id, info) in self.sessions.iter_conns() {
            if let Some(view) = views.get(&info.player_id) {
                // Non-blocking: never let one slow client stall the loop; a full
                // queue means the client is behind, so dropping this stale view
                // is correct — the next supersedes it.
                let _ = info.outbound.try_send(view.clone());
            }
            if let Some(reps) = reports.get(&info.player_id) {
                for r in reps {
                    let _ = info.outbound.try_send(r.clone());
                }
            }
            if let Some(tl) = timelines.get(&info.player_id) {
                let _ = info.outbound.try_send(tl.clone());
            }
        }
    }
}

/// The buildable options + their recipes (§step1), built from the sim's const
/// The public star chart as SystemInfo rows — the Welcome galaxy's `systems`
/// and every GalaxyUpdate re-broadcast share this one mapper, so the two can
/// never drift.
fn system_infos(world: &sim::World) -> Vec<SystemInfo> {
    world
        .systems
        .iter()
        .map(|s| SystemInfo {
            id: s.id,
            pos: s.pos,
            name: s.name.clone(),
            band: world.band_of(s).slug(),
            claim_cost: s.claim_cost,
        })
        .collect()
}

/// recipes and sent once in the Welcome galaxy. Whole-unit costs for the UI.
fn build_options() -> Vec<BuildOptionView> {
    use sim::{BuildKind, ShipKind, StructureKind};
    // §economy: the 5 ships + ALL 16 structures, data-driven (keys = slugs; a
    // legacy client sending an old slug still parses via the serde aliases).
    let ships = [
        ("convoy", "Convoy", BuildKind::Ship { ship: ShipKind::Convoy }),
        ("raider", "Raider", BuildKind::Ship { ship: ShipKind::Raider }),
        ("scout", "Scout", BuildKind::Ship { ship: ShipKind::Scout }),
        ("corvette", "Corvette", BuildKind::Ship { ship: ShipKind::Corvette }),
        ("colony", "Colony Ship", BuildKind::Ship { ship: ShipKind::Colony }),
        // §ladder: the warship ladder (research-gated hulls; the client shows
        // the gate copy, the sim enforces UnlockHull at BuildShip).
        ("destroyer", "Destroyer", BuildKind::Ship { ship: ShipKind::Destroyer }),
        ("cruiser", "Cruiser", BuildKind::Ship { ship: ShipKind::Cruiser }),
        ("battleship", "Battleship", BuildKind::Ship { ship: ShipKind::Battleship }),
        ("dreadnought", "Dreadnought", BuildKind::Ship { ship: ShipKind::Dreadnought }),
        ("titan", "Titan", BuildKind::Ship { ship: ShipKind::Titan }),
    ];
    // §modules Part B3: the 5 modules, keyed `module:<slug>` so the client routes
    // them to BuildModule (not BuildShip/DevelopSystem) while reusing the same
    // recipe-cost channel. They hold no slot and gate on an Armaments Complex.
    let modules = sim::module::MODULE_KINDS.map(|m| {
        (
            format!("module:{}", m.slug()),
            m.label().to_string(),
            BuildKind::Module { module: m },
        )
    });
    ships
    .into_iter()
    .map(|(k, l, w)| (k.to_string(), l.to_string(), w))
    .chain(StructureKind::ALL.into_iter().map(|k| (k.slug().to_string(), k.title().to_string(), BuildKind::Upgrade { upgrade: k })))
    .chain(modules)
    .map(|(key, label, what)| {
        let r = sim::build::recipe_for(what);
        BuildOptionView {
            key,
            label,
            costs: r.costs.iter().map(|(c, n)| StockSlot { commodity: *c, units: *n as u32 }).collect(),
            build_secs: r.build_ticks as f64 / TICK_HZ as f64,
        }
    })
    .collect()
}

/// §research R6: the gate progress bar for a SEALED node — the verb/metric the
/// tier waits on, current vs threshold. `None` when the tier carries no verb gate
/// (Tier I, or a IV/V node gated only by its ladder predecessor).
fn gate_progress(
    p: &sim::research::Programme,
    rs: &sim::research::ResearchState,
    metric: &dyn Fn(sim::research::Metric) -> f64,
    now: f64,
) -> Option<crate::protocol::GateProgressView> {
    use sim::research::Gate;
    match sim::research::tier_gate(p.field, p.school, p.tier) {
        Gate::None => None,
        Gate::Cumulative(v, t) => Some(crate::protocol::GateProgressView {
            label: v.label().to_string(),
            current: rs.verb(v),
            threshold: t,
        }),
        Gate::State(m, t) => Some(crate::protocol::GateProgressView {
            label: m.label().to_string(),
            current: metric(m),
            threshold: t,
        }),
        Gate::Sustained(m, _t, secs) => {
            // The endurance clock: days held continuously vs the required window.
            let held = rs
                .sustained_since
                .get(&m)
                .map(|since| (now - *since as f64).max(0.0) / 86_400.0)
                .unwrap_or(0.0);
            Some(crate::protocol::GateProgressView {
                label: format!("days holding {}", m.label()),
                current: held,
                threshold: secs as f64 / 86_400.0,
            })
        }
    }
}

/// §research R6: build the viewer's OWN syndicate research picture (owner-only).
fn research_view(world: &sim::World, sid: sim::SyndicateId) -> crate::protocol::ResearchView {
    use crate::protocol::{AcademyRow, ActiveResearchView, ProgrammeView, ResearchView};
    let syn = &world.syndicates[&sid];
    let rs = &syn.research;
    let now = world.time;
    let metric = |m| world.syndicate_metric(sid, m);

    // The per-Academy contribution table (the same factor chain the clock uses).
    let contribs = world.research_contributions(sid);
    let rate: f64 = contribs.iter().filter(|c| c.supplied).map(|c| c.rate).sum();
    let academies = contribs
        .iter()
        .map(|c| AcademyRow {
            system: c.system_name.clone(),
            body_id: c.body_id,
            tier: c.tier,
            throughput: c.throughput,
            staffing: c.staffing,
            skill: c.skill,
            food: c.food,
            rate: c.rate,
            supplied: c.supplied,
        })
        .collect();

    // The active programme banner (with a live ETA at the current rate).
    let active = rs.active.as_deref().and_then(|id| {
        sim::research::programme(id).map(|p| {
            let cost = sim::research::cost_of(id);
            let eta_secs = if rate > 1e-9 { Some((cost - rs.progress).max(0.0) / rate) } else { None };
            ActiveResearchView {
                id: id.to_string(),
                name: p.name.to_string(),
                progress: rs.progress,
                cost,
                eta_secs,
            }
        })
    });

    // The whole visible tree, each node tagged with the viewer's state + gate.
    let programmes = sim::research::visible_ids()
        .filter_map(|id| {
            let p = sim::research::programme(id)?;
            let state = if rs.has(id) {
                "completed"
            } else if rs.active.as_deref() == Some(id) {
                "active"
            } else if rs.queue.iter().any(|q| q == id) {
                "queued"
            } else if sim::research::is_available(id, rs, &metric, now) {
                "available"
            } else {
                "locked"
            };
            let gate = if state == "locked" { gate_progress(p, rs, &metric, now) } else { None };
            Some(ProgrammeView {
                id: id.to_string(),
                field: p.field.slug().to_string(),
                school: p.school.map(|s| s.slug().to_string()),
                tier: p.tier,
                name: p.name.to_string(),
                blurb: p.blurb.to_string(),
                state: state.to_string(),
                cost: sim::research::cost_of(id),
                gate,
            })
        })
        .collect();

    ResearchView { active, queue: rs.queue.clone(), rate, stalled: rs.stalled, academies, programmes }
}

/// Run the authoritative loop until all [`GameHandle`]s are dropped.
pub async fn run(
    world: World,
    persistence: PersistenceHandle,
    snapshot_every: u64,
    status_tx: watch::Sender<ServerStatus>,
    mut rx: mpsc::UnboundedReceiver<GameInput>,
) {
    let mut game = GameLoop::new(world, persistence, snapshot_every, status_tx);

    let mut ticker = interval(Duration::from_secs_f64(DT));
    // If we ever fall behind, skip missed ticks rather than bursting to catch
    // up (avoids a death spiral). Sim time tracks completed ticks regardless.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!(tick_hz = TICK_HZ, "authoritative game loop started");

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                game.tick();
            }
            maybe_input = rx.recv() => {
                match maybe_input {
                    Some(input) => game.handle_input(input),
                    // All senders dropped: nothing can ever drive the game again.
                    None => break,
                }
            }
        }
    }

    info!("authoritative game loop stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::Vec2;

    fn concluded(started_at: f64, ended_at: f64, pos: Vec2) -> ConcludedBattle {
        ConcludedBattle {
            id: sim::EntityId(1),
            pos,
            started_at,
            ended_at,
            a_owner: PlayerId(1),
            d_owner: PlayerId(2),
            participants: vec![sim::EntityId(10), sim::EntityId(11)],
        }
    }

    /// The in-progress icon of a concluded battle lingers until the CONCLUSION's
    /// light arrives — `ended_at + |pos − cc| / c` — which is exactly when the
    /// per-participant aftermath report lands (`ReportScheduler::due_for` gates on
    /// `event_time + dist/c`, and `event_time == ended_at`). So the icon flips to
    /// aftermath on ONE wavefront: no FTL early-vanish, no gap where the suppressed
    /// participant fleets re-appear. (The bug: the icon used to vanish at true
    /// `ended_at`, `delay` seconds before the aftermath, exposing the stale fleets.)
    #[test]
    fn concluded_icon_lingers_until_conclusion_light_matches_aftermath() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        // Battle 6000 su away → 20 s of light each way. It ran t=100..140.
        let pos = Vec2::new(6000.0, 0.0);
        let (started_at, ended_at) = (100.0, 140.0);
        let cb = concluded(started_at, ended_at, pos);
        let delay = pos.distance(cc) / c; // 20 s
        let aftermath_arrival = ended_at + delay; // when due_for delivers it

        // Just after the conclusion's light for the START has been seen but the
        // conclusion's light has NOT yet arrived: the in-progress icon still shows
        // (this is the window where the fleets used to wrongly re-appear).
        assert!(cb.shows_in_progress(cc, c, ended_at + 5.0), "icon must persist through the light-in-flight gap");
        assert!(cb.shows_in_progress(cc, c, aftermath_arrival - 0.001), "still in progress an instant before the aftermath");

        // At the aftermath's arrival the icon is gone (strict upper bound) — the
        // aftermath (delivered on `arrival <= now`) takes over on the same instant.
        assert!(!cb.shows_in_progress(cc, c, aftermath_arrival), "icon flips off exactly as the aftermath lands");
        assert!(!cb.shows_in_progress(cc, c, aftermath_arrival + 5.0), "and stays off after");
    }

    /// The linger is per-viewer and light-honest: a FAR command center keeps the
    /// in-progress icon longer than a NEAR one, because its conclusion light takes
    /// longer to arrive — never a global FTL flip.
    #[test]
    fn linger_is_per_viewer_light_delayed() {
        let c = 300.0;
        let pos = Vec2::new(0.0, 0.0);
        let cb = concluded(0.0, 40.0, pos);
        let near = Vec2::new(300.0, 0.0); // 1 s of light
        let far = Vec2::new(9000.0, 0.0); // 30 s of light

        // 41 s after start (1 s after true end): near viewer's conclusion light has
        // arrived (icon gone); the far viewer's has not (icon still shown).
        assert!(!cb.shows_in_progress(near, c, 41.0), "near viewer already saw the conclusion");
        assert!(cb.shows_in_progress(far, c, 41.0), "far viewer's conclusion light is still in flight");
    }

    /// A viewer whose START light never arrived before the battle ended (it began
    /// and ended faster than its light could reach them) must NEVER see a phantom
    /// in-progress icon — the lower bound guards against conjuring one late.
    #[test]
    fn no_phantom_icon_when_start_light_never_arrived() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        // 6000 su away (20 s of light) but the battle lasted only 1 s (t=0..1).
        let cb = concluded(0.0, 1.0, Vec2::new(6000.0, 0.0));
        // The visible window is [started_at + delay, ended_at + delay) = [20, 21):
        // one honest second, shifted whole by the 20 s light delay. Before 20 the
        // start-light hasn't landed (no phantom); from 21 the conclusion-light has.
        assert!(!cb.shows_in_progress(cc, c, 19.999), "no icon before the start light arrives");
        assert!(cb.shows_in_progress(cc, c, 20.5), "the honest 1 s sighting");
        assert!(!cb.shows_in_progress(cc, c, 21.0), "gone once the conclusion light arrives");
    }
}

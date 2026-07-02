//! The per-player lightspeed view filter — the code embodiment of the entire
//! information model (§6, §14), and the novel/risky core of the game.
//!
//! Between the simulation's ground truth and each player's socket sits this
//! filter. It keeps a recent history of every moving object's TRUE positions,
//! and for each player reconstructs what the light reaching THEIR command center
//! shows — every object as of the retarded time when the arriving light left it.
//!
//! ## The fairness guarantee
//!
//! A player must never receive information their light hasn't reached yet. We
//! make this exact. A position sample `(t, p)` of an object is *observable* at a
//! command center `cc` at wall-time `now` iff its light has arrived:
//!
//! ```text
//!   t + |p − cc| / c  ≤  now
//! ```
//!
//! Define `arrival(t) = t + |p(t) − cc| / c`. Its derivative is
//! `1 + d/dt|p−cc| / c ≥ 1 − |v|/c`, which is strictly positive whenever the
//! object moves slower than light (all ships do, by construction). So
//! `arrival` is **strictly increasing**: scanning samples newest→oldest, the
//! first one with `arrival ≤ now` is the unique latest observable state. We show
//! that one and nothing fresher — provably no leak.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::VecDeque;

use sim::{
    Cargo, Commodity, CountClass, EntityId, FleetOrder, HomeSlot, PlayerId, ShipKind, StarSystem,
    Vec2, World,
};

use crate::protocol::{
    AnchorView, BuildStateView, CargoView, CompCount, GhostView, IntelView, StockSlot,
    SystemStateView,
};

/// One recorded true state of a ship at a sim time.
#[derive(Clone, Copy)]
struct Sample {
    time: f64,
    pos: Vec2,
    vel: Vec2,
}

/// Position history + current metadata for one FLEET. Fleet-derived scalars
/// (flagship, broadcast, sensor bubble, cap speed, size bucket) are snapshotted
/// at record time so the view filter never needs the live sim `Fleet`.
struct Track {
    owner: PlayerId,
    /// Exact composition (kinds → counts) at the last record — revealed only in
    /// coverage / to the owner.
    composition: BTreeMap<ShipKind, u32>,
    /// The fleet's flagship kind (what it's drawn as).
    flagship: ShipKind,
    /// Whether the fleet broadcasts (any member broadcasts).
    broadcasts: bool,
    /// The best sensor bubble the fleet projects (max member `sensor_mult`).
    sensor_mult: f64,
    /// Formation cruise cap (min member `max_speed`) — drives uncertainty.
    max_speed: f64,
    /// The estimated-size bucket a fog observer sees.
    count_class: CountClass,
    /// Ordered oldest→newest.
    samples: VecDeque<Sample>,
    /// Last sim time this track was updated (for pruning dead ships).
    last_seen: f64,
    /// Current cargo (convoys). Static for the demo patrol convoys, so sending
    /// it alongside the (delayed) position is leak-free here; when convoys carry
    /// dynamic cargo (§9), cargo would move into the per-sample history so it is
    /// delayed exactly like position.
    cargo: Option<Cargo>,
    /// Current broadcast route (convoys' waypoints). Static for demo patrols
    /// (same caveat as cargo).
    route: Option<Vec<Vec2>>,
    /// If the ship was destroyed: (true time, true position) of the destruction.
    /// The ship is gone from true space, but each viewer keeps seeing its ghost
    /// until the light of this event reaches their command center (§6).
    destroyed: Option<(f64, Vec2)>,
}

/// The view filter's state: every moving object's recent true-position history.
pub struct PositionHistory {
    tracks: HashMap<EntityId, Track>,
    /// How many seconds of history to retain. Must exceed the largest possible
    /// light delay (galaxy diameter / c) so every long-lived object always has
    /// an observable sample.
    horizon: f64,
    /// Sensor detection radius each of a player's assets projects (config).
    sensor_range: f64,
}

impl PositionHistory {
    /// Create a history sized to a world's maximum possible light delay, with a
    /// safety margin.
    pub fn for_world(world: &World) -> Self {
        let max_delay = (2.0 * world.config.galaxy_radius) / world.config.c;
        PositionHistory {
            tracks: HashMap::new(),
            horizon: max_delay * 1.25 + 1.0,
            sensor_range: world.config.sensor_range,
        }
    }

    /// Record the current true positions of all ships. Called every tick so the
    /// retarded-time boundary is resolved at full temporal resolution.
    pub fn record(&mut self, world: &World) {
        let now = world.time;
        for (id, ship) in &world.fleets {
            let track = self.tracks.entry(*id).or_insert_with(|| Track {
                owner: ship.owner,
                composition: BTreeMap::new(),
                flagship: ship.flagship_kind(),
                broadcasts: ship.broadcasts(),
                sensor_mult: ship.sensor_mult(),
                max_speed: ship.max_speed(),
                count_class: ship.count_class(),
                samples: VecDeque::new(),
                last_seen: now,
                cargo: None,
                route: None,
                destroyed: None,
            });
            track.owner = ship.owner;
            track.composition = ship.composition.clone();
            track.flagship = ship.flagship_kind();
            track.broadcasts = ship.broadcasts();
            track.sensor_mult = ship.sensor_mult();
            track.max_speed = ship.max_speed();
            track.count_class = ship.count_class();
            track.last_seen = now;
            track.cargo = ship.cargo;
            track.route = route_of(&ship.order);
            track.samples.push_back(Sample {
                time: now,
                pos: ship.pos,
                vel: ship.vel,
            });
            // Drop samples older than the horizon.
            while let Some(front) = track.samples.front() {
                if now - front.time > self.horizon {
                    track.samples.pop_front();
                } else {
                    break;
                }
            }
        }
        // Forget tracks for ships that have been gone longer than the horizon
        // (their last light has passed) — including destroyed ships once every
        // viewer's light has reached the destruction. Keeps memory bounded.
        let horizon = self.horizon;
        self.tracks
            .retain(|_, t| now - t.last_seen <= horizon);
    }

    /// Mark a ship destroyed at true `time` and true `pos`. The ship is gone from
    /// the simulation, but its track is kept so each player keeps seeing its
    /// ghost until the light of the destruction reaches them (`view_for`).
    pub fn mark_destroyed(&mut self, ship_id: EntityId, time: f64, pos: Vec2) {
        if let Some(t) = self.tracks.get_mut(&ship_id) {
            t.destroyed = Some((time, pos));
            // Keep the track retained until the destruction's light has reached
            // every possible viewer (horizon > max delay).
            t.last_seen = time.max(t.last_seen);
        }
    }

    /// Build the delayed/fogged view of all ships for a player, applying the
    /// two-tier information model (§6) on top of the lightspeed delay:
    ///
    /// * **Tier 1 — broadcast:** convoys broadcast identity + position, so every
    ///   convoy is included galaxy-wide as a light-delayed ghost (with its
    ///   route). Raiders are dark — not broadcast.
    /// * **Tier 2 — sensor range:** a convoy's *cargo* is included only when the
    ///   convoy is within the player's sensor coverage; a *raider* is included
    ///   ONLY when within coverage (otherwise omitted entirely — no leak).
    ///
    /// Sensor coverage is the union of `sensor_range` circles around the
    /// player's assets — their command center and their own ships — taken at
    /// their **observed (delayed) positions**, the same ghosts the client draws.
    /// Detection therefore happens in the command center's delayed composite
    /// frame, using only light that has arrived: a raider is detected exactly
    /// when its delayed ghost falls inside a drawn coverage circle. This never
    /// reveals the true position of a dark ship (you still only ever see where
    /// it *was*), and it cannot disagree with the client's rendering.
    /// (Array-less convenience — production always goes through
    /// [`Self::view_for_with_arrays`]; the many fairness tests use this form.)
    #[cfg(test)]
    pub fn view_for(&self, viewer: PlayerId, cc: Vec2, c: f64, now: f64) -> Vec<GhostView> {
        self.view_for_with_arrays(viewer, cc, c, now, &[])
    }

    /// [`Self::view_for`] plus the viewer's SENSOR-ARRAY bubbles (§buildings
    /// step 2b): standing `(position, radius)` sources from the viewer's own
    /// array systems (`World::array_sensor_sources` — the shared coverage source
    /// of truth). They join the coverage union exactly like ship bubbles, so
    /// dark-raider detection and cargo reveal inherit them consistently. Systems
    /// are STATIC, so a fixed source position is as leak-free as the delayed
    /// ship ghosts; this only ever ADDS legitimate vision for the viewer — a
    /// rival's arrays are never passed in, and nothing about the array itself is
    /// sent (its tier stays owner-only in `filter_systems`).
    pub fn view_for_with_arrays(
        &self,
        viewer: PlayerId,
        cc: Vec2,
        c: f64,
        now: f64,
        arrays: &[(Vec2, f64)],
    ) -> Vec<GhostView> {
        // Pass 1: retarded ghost for every observable ship, and gather the
        // viewer's sensor coverage (command center + their own ships' ghosts).
        struct Pre<'a> {
            id: EntityId,
            owner: PlayerId,
            flagship: ShipKind,
            broadcasts: bool,
            max_speed: f64,
            count_class: CountClass,
            composition: &'a BTreeMap<ShipKind, u32>,
            sample: Sample,
            cargo: Option<Cargo>,
            route: &'a Option<Vec<Vec2>>,
            /// A destroyed raider that WAS legitimately within the viewer's sensor
            /// coverage at the retarded time of the ghost being shown. Latches its
            /// detection to that pre-destruction frame so a *post*-destruction
            /// coverage change (the winner breaking off home) can't un-detect — and
            /// thereby reveal — the kill before its light arrives (§6). Never set
            /// for a raider that was never detected, so it can't conjure existence.
            destroyed_detected: bool,
        }
        let mut pre = Vec::new();
        // Coverage as (center, radius) sources: the command center + own ship
        // ghosts at the global range, plus any standing array bubbles (each with
        // its OWN radius — a developed array outsees a ship).
        let mut coverage: Vec<(Vec2, f64)> = vec![(cc, self.sensor_range)];
        coverage.extend_from_slice(arrays);
        for (id, track) in &self.tracks {
            // Destroyed ships: the player keeps seeing the ghost (flying along on
            // old light) until the destruction's light reaches their command
            // center; only THEN does it vanish. Before that, serve it normally.
            if let Some((dt, dpos)) = track.destroyed
                && now >= dt + dpos.distance(cc) / c
            {
                continue; // the destruction has been observed — it's gone
            }
            let Some(sample) = latest_observable(&track.samples, cc, c, now) else {
                continue; // dark — no light from this object has arrived yet
            };
            if track.owner == viewer {
                // Each own fleet projects its best bubble — a scout aboard gives
                // an oversized one (`sensor_mult`: mobile vision).
                coverage.push((sample.pos, self.sensor_range * track.sensor_mult));
            }
            // For a destroyed DARK fleet (raiders/scouts only), decide visibility
            // in the ghost's OWN retarded frame (the world as the arriving light
            // shows it), not the `now` frame whose coverage already reflects the
            // post-kill break-off.
            let destroyed_detected = track.destroyed.is_some()
                && !track.broadcasts
                && self.detected_at_retarded_time(viewer, cc, sample.pos, sample.time, arrays);
            pre.push(Pre {
                id: *id,
                owner: track.owner,
                flagship: track.flagship,
                broadcasts: track.broadcasts,
                max_speed: track.max_speed,
                count_class: track.count_class,
                composition: &track.composition,
                sample,
                cargo: track.cargo,
                route: &track.route,
                destroyed_detected,
            });
        }

        // Pass 2: apply the two-tier visibility rules using the coverage.
        let mut ghosts = Vec::new();
        for p in pre {
            // A destroyed raider stays detected for as long as its retarded-frame
            // latch holds (until the Pass-1 destruction-light gate removes it); a
            // live raider/convoy uses the ordinary `now`-frame coverage.
            let detected = p.destroyed_detected || within_coverage(&coverage, p.sample.pos);

            // A DARK fleet (raiders/scouts only — nothing aboard broadcasts) is
            // present ONLY inside sensor coverage. (A player's own dark fleet sits
            // at the centre of its own sensor circle, so it is always present.)
            // Omitted entirely otherwise — never sent-and-hidden. Because a dark
            // fleet is only ever SEEN inside coverage, its composition is always
            // revealed when visible (consistent — no half-seen dark fleet).
            if !p.broadcasts && !detected {
                continue;
            }

            let age = now - p.sample.time;
            let own = p.owner == viewer;
            // ONE law governs ALL information — it travels at lightspeed with NO
            // exceptions, including the player's OWN ships (§6). There is no FTL
            // tether to your fleet: certainty is a function of PROXIMITY to the
            // command center, not ownership. So uncertainty is `age × max_speed`
            // for every object alike — an own ship far out is known exactly as
            // poorly as an enemy at the same distance; one close to the command
            // center is fresh and near-certain because its light barely lags.
            // `own` remains only a "this is mine" marker, never a certainty grant.
            let uncertainty = age * p.max_speed;
            let is_convoy = p.flagship == ShipKind::Convoy;
            // Convoy fleets broadcast their route; cargo only within sensor
            // coverage (Tier 2), exactly as before.
            let route = if is_convoy { p.route.clone() } else { None };
            let cargo = if detected {
                p.cargo.map(|cg| CargoView {
                    commodity: cg.commodity,
                    units: cg.units,
                })
            } else {
                None
            };
            // The INTEL LADDER (§13.1): the size bucket is ALWAYS available on a
            // visible fleet; the exact composition ONLY to the owner or inside
            // sensor coverage — never leaking the true count outside it.
            let composition = if own || detected {
                Some(
                    p.composition
                        .iter()
                        .map(|(k, n)| CompCount { kind: *k, count: *n })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };

            ghosts.push(GhostView {
                id: p.id,
                owner: p.owner,
                kind: p.flagship,
                pos: p.sample.pos,
                vel: p.sample.vel,
                age,
                uncertainty,
                own,
                route,
                cargo,
                count_class: p.count_class,
                composition,
            });
        }
        // Deterministic ordering by id.
        ghosts.sort_by_key(|g| g.id.0);
        ghosts
    }

    /// The staleness (light delay, seconds) of a ship's ghost as the command
    /// center at `cc` observes it — the age of the latest sample whose light has
    /// arrived. This is exactly how far behind the player's view of that ship
    /// is, and equals `|ghost_pos − cc| / c`. Used to pace the outbound command
    /// comet so it meets the ghost. `None` if the ship is currently dark.
    pub fn observed_age(&self, ship_id: EntityId, cc: Vec2, c: f64, now: f64) -> Option<f64> {
        let track = self.tracks.get(&ship_id)?;
        let sample = latest_observable(&track.samples, cc, c, now)?;
        Some(now - sample.time)
    }

    /// Was a (destroyed) raider's ghost — observed at retarded position `ghost_pos`,
    /// whose light left at sim-time `t_r` — inside the viewer's sensor coverage *at
    /// that same retarded time*? This reconstructs coverage from the viewer's own
    /// assets as they actually stood at `t_r`, i.e. in the frame whose light is
    /// arriving now, BEFORE any post-destruction break-off the viewer can't have
    /// seen yet. It only ever answers "yes" for a genuine past detection, so it can
    /// keep a dead raider visible until its destruction light arrives without ever
    /// revealing a raider the viewer never tracked (§6).
    fn detected_at_retarded_time(
        &self,
        viewer: PlayerId,
        cc: Vec2,
        ghost_pos: Vec2,
        t_r: f64,
        arrays: &[(Vec2, f64)],
    ) -> bool {
        // The command center is a fixed sensor asset.
        if ghost_pos.distance(cc) <= self.sensor_range {
            return true;
        }
        // Standing sensor arrays are fixed assets too (near-permanent
        // infrastructure — treated as present in any retarded frame; a
        // deliberate simplification vs. tracking per-array build times).
        if arrays.iter().any(|(p, r)| ghost_pos.distance(*p) <= *r) {
            return true;
        }
        for track in self.tracks.values() {
            if track.owner != viewer {
                continue; // coverage comes only from the viewer's own assets
            }
            // An asset can't have provided coverage after its own observed death.
            if let Some((dt, _)) = track.destroyed
                && t_r >= dt
            {
                continue;
            }
            if let Some(s) = sample_at(&track.samples, t_r)
                && s.pos.distance(ghost_pos) <= self.sensor_range * track.sensor_mult
            {
                return true;
            }
        }
        false
    }
}

/// Filter home anchors for a player: positions are static geography (always
/// shown), but each anchor's **owner** is revealed only once the light of the
/// claim event has reached this player's command center — or immediately if the
/// player owns it. Without this, a rival's presence would leak instantly (§6).
pub fn filter_anchors(
    slots: &[HomeSlot],
    viewer: PlayerId,
    cc: Vec2,
    c: f64,
    now: f64,
) -> Vec<AnchorView> {
    slots
        .iter()
        .map(|slot| {
            let owner = match (slot.owner, slot.claimed_at) {
                (Some(owner), _) if owner == viewer => Some(owner),
                (Some(owner), Some(claimed_at)) => {
                    let arrival = claimed_at + slot.pos.distance(cc) / c;
                    if arrival <= now {
                        Some(owner)
                    } else {
                        None // claim's light hasn't reached this player yet
                    }
                }
                _ => None,
            };
            AnchorView {
                pos: slot.pos,
                owner,
            }
        })
        .collect()
}

/// Filter the dynamic, per-tick state of star systems for a player (§4, §6). A
/// system's geography/geology (pos, name, deposits, claim cost) is public and
/// sent once at join; here we light-gate the DYNAMIC state:
///
/// * **owner** — the viewer sees their OWN claim instantly, but a rival's
///   ownership only once the claim's light has reached the viewer's command
///   center (`claimed_at + |pos − cc|/c ≤ now`). Exactly the home-anchor rule —
///   no faster-than-light presence/claim leak.
/// * **stockpile** — a system's accumulated production is private: shown only to
///   the owner (who can anyway predict it from the known deposit rates), never to
///   rivals. So no information about a rival's holdings ever leaks.
#[allow(clippy::too_many_arguments)]
pub fn filter_systems(
    systems: &[StarSystem],
    viewer: PlayerId,
    cc: Vec2,
    c: f64,
    now: f64,
    build_queue: &[sim::BuildJob],
    tick: u64,
    dt: f64,
    intel: &BTreeMap<EntityId, sim::IntelSnapshot>,
) -> Vec<SystemStateView> {
    systems
        .iter()
        .map(|sys| {
            let own = sys.owner == Some(viewer);
            let owner = match (sys.owner, sys.claimed_at) {
                (Some(owner), _) if owner == viewer => Some(owner),
                (Some(owner), Some(claimed_at)) => {
                    let arrival = claimed_at + sys.pos.distance(cc) / c;
                    if arrival <= now {
                        Some(owner)
                    } else {
                        None // the claim's light hasn't reached this player yet
                    }
                }
                _ => None,
            };
            // Only the owner sees the stockpile (whole units), and never rivals.
            let stockpile = own.then(|| {
                sys.stockpile
                    .iter()
                    .filter_map(|(commodity, amount)| {
                        let units = amount.floor() as u32;
                        (units >= 1).then_some(StockSlot { commodity: *commodity, units })
                    })
                    .collect()
            });
            // Owner-only: the soonest in-progress build here (the UI shows one job).
            let build = own
                .then(|| {
                    build_queue
                        .iter()
                        .filter(|j| j.system == sys.id && j.owner == viewer)
                        .min_by_key(|j| j.complete_tick)
                        .map(|j| BuildStateView {
                            key: build_key(j.what).to_string(),
                            complete_time: now + (j.complete_tick.saturating_sub(tick)) as f64 * dt,
                        })
                })
                .flatten();
            // Development slots (§buildings step 1): used = built tiers + this
            // viewer's in-progress upgrade jobs here, so the readout matches what
            // apply_build will actually accept next. Owner-only, like the tiers.
            let slots_used = sys.dev_slots_built()
                + build_queue
                    .iter()
                    .filter(|j| {
                        j.system == sys.id
                            && j.owner == viewer
                            && matches!(j.what, sim::BuildKind::Upgrade { .. })
                    })
                    .count() as u32;
            SystemStateView {
                id: sys.id,
                owner,
                stockpile,
                build,
                // Owner-only, like the stockpile: a system's development tier is
                // private intel. Gating it also avoids leaking an upgrade to a rival
                // FASTER THAN LIGHT (the field would otherwise update the instant it
                // lands, unlike the light-gated `owner`). Rivals see tier 0.
                extractor_tier: if own { sys.extractor_tier } else { 0 },
                depot_tier: if own { sys.depot_tier } else { 0 },
                shipyard_tier: if own { sys.shipyard_tier } else { 0 },
                sensor_tier: if own { sys.sensor_tier } else { 0 },
                // A rival NEVER sees a platform in the View — it reveals itself
                // only through engagement outcomes (delayed battle reports).
                defense_tier: if own { sys.defense_tier } else { 0 },
                habitat_tier: if own { sys.habitat_tier } else { 0 },
                // A rival must never learn whether your colonies are starving.
                habitat_fed: own && sys.habitat_fed,
                refinery_tier: if own { sys.refinery_tier } else { 0 },
                slots_used: if own { slots_used } else { 0 },
                slots_total: if own { sys.dev_slots() } else { 0 },
                // Storage (§buildings step 2) — owner-only like everything above.
                // `used` is floored to whole units to match the stockpile readout.
                storage_cap: if own { sys.storage_cap() as u32 } else { 0 },
                storage_used: if own { sys.storage_used().floor() as u32 } else { 0 },
                // The viewer's OWN scout intel about this rival system (§scout
                // part 2), delivered only once the capture's light — from where
                // the scout stood — has reached the viewer's command center. It
                // is the viewer's own gathered knowledge: no rival data flows
                // here beyond what the scout physically saw, and the scouted
                // side never learns anything.
                intel: if own {
                    None // your own systems need no spying
                } else {
                    intel.get(&sys.id).and_then(|snap| {
                        let arrival = snap.observed_at + snap.pos.distance(cc) / c;
                        (arrival <= now).then_some(IntelView {
                            defense_tier: snap.defense_tier,
                            shipyard_tier: snap.shipyard_tier,
                            observed_at: snap.observed_at,
                        })
                    })
                },
            }
        })
        .collect()
}

/// Stable key string for a buildable thing (matches the client's build commands).
pub fn build_key(what: sim::BuildKind) -> &'static str {
    match what {
        sim::BuildKind::Ship { ship: sim::ShipKind::Convoy } => "convoy",
        sim::BuildKind::Ship { ship: sim::ShipKind::Raider } => "raider",
        sim::BuildKind::Ship { ship: sim::ShipKind::Corvette } => "corvette",
        sim::BuildKind::Ship { ship: sim::ShipKind::Colony } => "colony",
        sim::BuildKind::Ship { ship: sim::ShipKind::Scout } => "scout",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Extractor } => "extractor",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Depot } => "depot",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Shipyard } => "shipyard",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::SensorArray } => "sensor_array",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::DefensePlatform } => "defense_platform",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Habitat } => "habitat",
        sim::BuildKind::Upgrade { upgrade: sim::SystemUpgrade::Refinery } => "refinery",
    }
}

/// History of the hub's standing prices, so each player can be shown the prices
/// **light-delayed** from the hub (§9). The Exchange ticker is a lightspeed
/// broadcast; far from the hub you read an old copy. Mirrors [`PositionHistory`]
/// but for the (single, shared) hub.
pub struct PriceHistory {
    samples: VecDeque<(f64, BTreeMap<Commodity, f64>)>,
    horizon: f64,
}

impl PriceHistory {
    pub fn for_world(world: &World) -> Self {
        let max_delay = (2.0 * world.config.galaxy_radius) / world.config.c;
        PriceHistory {
            samples: VecDeque::new(),
            horizon: max_delay * 1.25 + 1.0,
        }
    }

    pub fn record(&mut self, world: &World) {
        let now = world.time;
        self.samples.push_back((now, world.market.prices().clone()));
        while let Some((t, _)) = self.samples.front() {
            if now - t > self.horizon {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    /// The hub prices as of `target` sim-time (the latest sample whose time is
    /// `≤ target`). Falls back to the oldest sample if `target` predates history.
    pub fn at(&self, target: f64) -> Option<&BTreeMap<Commodity, f64>> {
        let mut best: Option<&BTreeMap<Commodity, f64>> = None;
        for (t, prices) in &self.samples {
            if *t <= target {
                best = Some(prices);
            } else {
                break;
            }
        }
        best.or_else(|| self.samples.front().map(|(_, p)| p))
    }
}

/// Is `p` within `range` of any sensor center?
/// Is `p` inside any coverage source `(center, radius)`? Sources carry their own
/// radii so ship bubbles (global range) and sensor-array bubbles (per-tier range)
/// share one union — the single coverage predicate.
fn within_coverage(sources: &[(Vec2, f64)], p: Vec2) -> bool {
    sources.iter().any(|(center, radius)| p.distance(*center) <= *radius)
}

/// The broadcast route (waypoints) implied by a ship's current order, if any.
fn route_of(order: &FleetOrder) -> Option<Vec<Vec2>> {
    match order {
        FleetOrder::Patrol { waypoints, .. } => Some(waypoints.clone()),
        FleetOrder::MoveTo { dest } => Some(vec![*dest]),
        _ => None,
    }
}

/// The latest sample whose light has reached `cc` by `now`. Relies on
/// `arrival(t)` being strictly increasing (object speed < c): the first sample
/// found scanning newest→oldest with `arrival ≤ now` is the answer.
fn latest_observable(samples: &VecDeque<Sample>, cc: Vec2, c: f64, now: f64) -> Option<Sample> {
    for s in samples.iter().rev() {
        let arrival = s.time + s.pos.distance(cc) / c;
        if arrival <= now {
            return Some(*s);
        }
    }
    None
}

/// The asset's recorded state contemporaneous with sim-time `t_r` — the newest
/// sample at or before `t_r` (samples are oldest→newest). Falls back to the
/// oldest retained sample if `t_r` predates history. Used to reconstruct sensor
/// coverage in a destroyed raider's retarded frame (`detected_at_retarded_time`).
fn sample_at(samples: &VecDeque<Sample>, t_r: f64) -> Option<Sample> {
    let mut best = None;
    for s in samples.iter() {
        if s.time <= t_r {
            best = Some(*s);
        } else {
            break;
        }
    }
    best.or_else(|| samples.front().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track_from(samples: Vec<Sample>, owner: PlayerId, kind: ShipKind) -> Track {
        let last = samples.last().map(|s| s.time).unwrap_or(0.0);
        let cargo = (kind == ShipKind::Convoy).then_some(sim::Cargo {
            commodity: sim::Commodity::Fuel,
            units: 100,
        });
        let mut composition = BTreeMap::new();
        composition.insert(kind, 1u32);
        Track {
            owner,
            composition,
            flagship: kind,
            broadcasts: kind.broadcasts(),
            sensor_mult: kind.sensor_mult(),
            max_speed: kind.max_speed(),
            count_class: CountClass::from_count(1),
            samples: samples.into(),
            last_seen: last,
            cargo,
            route: None,
            destroyed: None,
        }
    }

    fn at(id: u64, x: f64, y: f64, owner: PlayerId, kind: ShipKind) -> (EntityId, Track) {
        // A stationary ship sitting at (x,y) for 0..100 s, sampled at 10 Hz.
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 100.0 {
            samples.push(Sample { time: t, pos: Vec2::new(x, y), vel: Vec2::ZERO });
            t += 0.1;
        }
        (EntityId(id), track_from(samples, owner, kind))
    }

    /// History with one track (id 1) and an effectively-infinite sensor range,
    /// so the existing lightspeed-fairness tests are unaffected by Tier 2.
    fn history_with(track: Track) -> PositionHistory {
        let mut tracks = HashMap::new();
        tracks.insert(EntityId(1), track);
        PositionHistory { tracks, horizon: 1e9, sensor_range: 1e12 }
    }

    fn history_of(tracks: Vec<(EntityId, Track)>, sensor_range: f64) -> PositionHistory {
        PositionHistory { tracks: tracks.into_iter().collect(), horizon: 1e9, sensor_range }
    }

    /// A ship sitting at X, then jumping to Y at t=10. A far command center must
    /// not see the jump until its light arrives — the fairness guarantee.
    #[test]
    fn no_leak_before_light_arrives() {
        let c = 300.0;
        let x = Vec2::new(0.0, 0.0);
        let y = Vec2::new(0.0, 100.0);
        let cc = Vec2::new(6000.0, 0.0); // 20 s of light away from X
        let mut samples = Vec::new();
        // At X for t in [0,10), then at Y from t=10 onward, sampled at 10 Hz.
        let mut t = 0.0;
        while t < 30.0 {
            samples.push(Sample {
                time: t,
                pos: if t < 10.0 { x } else { y },
                vel: Vec2::ZERO,
            });
            t += 0.1;
        }
        let hist = history_with(track_from(samples, PlayerId(7), ShipKind::Raider));

        // Light delay from X to cc ≈ 6000/300 = 20 s; the jump at t=10 cannot be
        // seen before ~t=30. At now=25 the viewer must still see X.
        let g25 = &hist.view_for(PlayerId(99), cc, c, 25.0)[0];
        assert_eq!(g25.pos, x, "viewer saw the jump before its light arrived (LEAK)");

        // Sanity: the shown sample's light really has arrived.
        let arrival = (25.0 - g25.age) + g25.pos.distance(cc) / c;
        assert!(arrival <= 25.0 + 1e-9);

        // Much later (well after the jump's light could arrive), it sees Y.
        let g_late = &hist.view_for(PlayerId(99), cc, c, 40.0)[0];
        assert_eq!(g_late.pos, y, "viewer never saw the jump even long after");
    }

    /// The shown sample is exactly the boundary: it has arrived, and the next
    /// (newer) sample has not. This is the precise "only what light permits".
    #[test]
    fn shows_exactly_the_boundary_sample() {
        let c = 300.0;
        let cc = Vec2::new(3000.0, 0.0);
        // Ship drifting along +y from origin, sampled 10 Hz for 60 s.
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 60.0 {
            samples.push(Sample {
                time: t,
                pos: Vec2::new(0.0, t * 5.0),
                vel: Vec2::new(0.0, 5.0),
            });
            t += 0.1;
        }
        let hist = history_with(track_from(samples.clone(), PlayerId(7), ShipKind::Convoy));
        let now = 45.0;
        let g = &hist.view_for(PlayerId(99), cc, c, now)[0];

        // The shown sample arrived.
        let shown_t = now - g.age;
        let shown_arrival = shown_t + g.pos.distance(cc) / c;
        assert!(shown_arrival <= now + 1e-9, "shown sample hasn't arrived");

        // The next newer sample has NOT arrived (it would be a leak to show it).
        let next = samples
            .iter()
            .find(|s| s.time > shown_t + 1e-9)
            .expect("there is a newer sample");
        let next_arrival = next.time + next.pos.distance(cc) / c;
        assert!(next_arrival > now, "a newer sample had also arrived — not the boundary");
    }

    /// A nearer command center sees a fresher (smaller-age) picture than a far
    /// one — different players genuinely see different delayed worlds.
    #[test]
    fn nearer_sees_fresher_than_farther() {
        let c = 300.0;
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 60.0 {
            samples.push(Sample { time: t, pos: Vec2::new(t * 2.0, 0.0), vel: Vec2::new(2.0, 0.0) });
            t += 0.1;
        }
        let hist = history_with(track_from(samples, PlayerId(7), ShipKind::Raider));
        let now = 50.0;
        let near = Vec2::new(100.0, 200.0);
        let far = Vec2::new(0.0, 9000.0);
        let g_near = &hist.view_for(PlayerId(99), near, c, now)[0];
        let g_far = &hist.view_for(PlayerId(99), far, c, now)[0];
        assert!(g_near.age < g_far.age, "near {} should be fresher than far {}", g_near.age, g_far.age);
    }

    /// Anchor ownership is light-gated: you see your own claim instantly, but a
    /// rival's claim only after its light reaches you — no faster-than-light
    /// presence leak.
    #[test]
    fn anchor_ownership_is_light_gated() {
        let c = 300.0;
        let me = PlayerId(7);
        let rival = PlayerId(8);
        let cc = Vec2::new(0.0, 0.0);
        let slots = vec![
            HomeSlot { pos: Vec2::new(0.0, 0.0), owner: Some(me), claimed_at: Some(0.0), system: None },
            // Rival's anchor, 6000 units away → 20 s of light.
            HomeSlot { pos: Vec2::new(6000.0, 0.0), owner: Some(rival), claimed_at: Some(0.0), system: None },
            HomeSlot { pos: Vec2::new(0.0, 3000.0), owner: None, claimed_at: None, system: None },
        ];

        // At t=10s, the rival's claim light (20 s away) has NOT arrived.
        let v10 = filter_anchors(&slots, me, cc, c, 10.0);
        assert_eq!(v10[0].owner, Some(me), "own claim should be visible instantly");
        assert_eq!(v10[1].owner, None, "rival claim leaked before its light arrived");
        assert_eq!(v10[2].owner, None);

        // At t=25s, the rival's claim light has arrived.
        let v25 = filter_anchors(&slots, me, cc, c, 25.0);
        assert_eq!(v25[1].owner, Some(rival), "rival claim should be visible after light arrives");

        // Positions are always present (static geography).
        assert_eq!(v10[1].pos, Vec2::new(6000.0, 0.0));
    }

    /// System ownership is light-gated exactly like anchors, and a system's
    /// stockpile (accumulated production) is private to its owner — never leaked
    /// to a rival, even once they can see the ownership (§4, §6).
    #[test]
    fn system_ownership_is_light_gated_and_stockpile_is_owner_only() {
        use std::collections::BTreeMap;
        let c = 300.0;
        let me = PlayerId(7);
        let rival = PlayerId(8);
        let cc = Vec2::new(0.0, 0.0);
        let mk = |id, pos, name: &str, owner, claimed_at, stock: &[(Commodity, f64)]| StarSystem {
            id: EntityId(id),
            pos,
            name: name.into(),
            deposits: vec![],
            claim_cost: 1000.0,
            owner,
            claimed_at,
            stockpile: stock.iter().copied().collect::<BTreeMap<_, _>>(),
            extractor_tier: 0,
            depot_tier: 0,
            shipyard_tier: 0,
            sensor_tier: 0,
            defense_tier: 0, defense_pool: 0.0,
            habitat_tier: 0,
            habitat_fed: false,
            refinery_tier: 0,
        };
        let mut systems = vec![
            mk(1, Vec2::new(0.0, 0.0), "MINE", Some(me), Some(0.0), &[(Commodity::Alloys, 12.7)]),
            // Rival's claim 6000 su away → 20 s of light.
            mk(2, Vec2::new(6000.0, 0.0), "RIVAL", Some(rival), Some(0.0), &[(Commodity::Ore, 99.0)]),
            mk(3, Vec2::new(0.0, 3000.0), "FREE", None, None, &[]),
        ];
        systems[0].extractor_tier = 2; // mine — developed
        systems[0].shipyard_tier = 1; // mine — a shipyard (visible to me only)
        systems[0].sensor_tier = 1; // mine — an array (visible to me only)
        systems[0].defense_tier = 1; // mine — a platform (visible to me only)
        systems[1].extractor_tier = 3; // rival — must stay hidden
        systems[1].shipyard_tier = 2; // rival — their military industry must stay hidden
        systems[1].sensor_tier = 2; // rival — their intel infrastructure must stay hidden
        systems[1].defense_tier = 3; // rival — their fortification must stay hidden
        systems[0].habitat_tier = 1; // mine — a colony (visible to me only)
        systems[0].habitat_fed = true;
        systems[1].habitat_tier = 2; // rival — their colonies must stay hidden
        systems[1].habitat_fed = true; // …and whether they're starving, doubly so
        systems[0].refinery_tier = 1; // mine — a refinery (visible to me only)
        systems[1].refinery_tier = 2; // rival — their fuel industry must stay hidden

        // A build at MINE (owner) and one at RIVAL's system — only MINE's is visible.
        let builds = vec![
            sim::BuildJob { id: 1, owner: me, system: EntityId(1), what: sim::BuildKind::Ship { ship: sim::ShipKind::Convoy }, complete_tick: 300, join: None },
            sim::BuildJob { id: 2, owner: rival, system: EntityId(2), what: sim::BuildKind::Ship { ship: sim::ShipKind::Raider }, complete_tick: 300, join: None },
        ];

        // At t=10 s the rival's claim light (20 s) has NOT arrived.
        let v10 = filter_systems(&systems, me, cc, c, 10.0, &builds, 0, sim::DT, &BTreeMap::new());
        assert!(v10[0].build.is_some(), "owner sees their own in-progress build");
        assert!(v10[1].build.is_none(), "a rival's build state must never leak");
        assert_eq!(v10[0].owner, Some(me), "own claim is visible instantly");
        assert_eq!(v10[1].owner, None, "rival claim leaked before its light arrived");
        assert_eq!(v10[2].owner, None);
        // My stockpile is shown (whole units); the rival's is never shown.
        let mine = v10[0].stockpile.as_ref().expect("owner sees own stockpile");
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].commodity, Commodity::Alloys);
        assert_eq!(mine[0].units, 12, "stockpile reported in whole units");
        assert!(v10[1].stockpile.is_none(), "a rival's stockpile must never be shown");
        assert!(v10[2].stockpile.is_none());
        // Development tier is owner-only too — the owner sees their own…
        assert_eq!(v10[0].extractor_tier, 2, "owner sees their own development tier");
        assert_eq!(v10[1].extractor_tier, 0, "a rival's tier must never leak (not even faster-than-light)");
        // Development SLOTS follow the same owner-only rule (§buildings step 1):
        // used counts built tiers (2) — the queued job at MINE is a SHIP, which
        // holds no slot — and rivals see 0/0, never the budget or usage.
        assert_eq!(v10[0].slots_used, 7, "owner sees slots used (all built tiers; ships hold none)");
        assert_eq!(v10[0].slots_total, systems[0].dev_slots(), "owner sees the slot budget");
        assert_eq!((v10[1].slots_used, v10[1].slots_total), (0, 0), "a rival's slots never leak");
        assert_eq!((v10[2].slots_used, v10[2].slots_total), (0, 0));
        // Storage cap + fill (§buildings step 2) — owner-only on the same rule.
        assert_eq!(v10[0].storage_cap, systems[0].storage_cap() as u32, "owner sees their cap");
        assert_eq!(v10[0].storage_used, 12, "owner sees fill in whole units");
        assert_eq!(v10[0].depot_tier, 0);
        assert_eq!((v10[1].storage_cap, v10[1].storage_used, v10[1].depot_tier), (0, 0, 0), "a rival's storage never leaks");
        // Shipyard tier (§buildings step 3) — owner-only on the same rule.
        assert_eq!(v10[0].shipyard_tier, systems[0].shipyard_tier, "owner sees their shipyard tier");
        assert_eq!(v10[1].shipyard_tier, 0, "a rival's shipyard tier never leaks");
        // Sensor Array tier (§buildings step 2b) — owner-only on the same rule:
        // a rival must never learn where you can see.
        assert_eq!(v10[0].sensor_tier, systems[0].sensor_tier, "owner sees their sensor tier");
        assert_eq!(v10[1].sensor_tier, 0, "a rival's sensor tier never leaks");
        // Defense Platform tier (§buildings step 2c) — owner-only: a rival
        // weighing a raid learns fortification ONLY the hard way (via the
        // engagement outcome), never from the View.
        assert_eq!(v10[0].defense_tier, systems[0].defense_tier, "owner sees their platform tier");
        assert_eq!(v10[1].defense_tier, 0, "a rival's platform never leaks — deterrence is discovered by engagement");
        // Habitat tier + FED state (§buildings step 3a) — owner-only: a rival
        // must never learn you have colonies, let alone whether they're starving.
        assert_eq!((v10[0].habitat_tier, v10[0].habitat_fed), (1, true), "owner sees their habitat + supply state");
        assert_eq!((v10[1].habitat_tier, v10[1].habitat_fed), (0, false), "a rival's habitat/starvation never leaks");
        // Refinery tier (§buildings step 3b) — owner-only on the same rule.
        assert_eq!(v10[0].refinery_tier, systems[0].refinery_tier, "owner sees their refinery tier");
        assert_eq!(v10[1].refinery_tier, 0, "a rival's refinery never leaks");

        // At t=25 s the rival's claim light has arrived — ownership now visible…
        let v25 = filter_systems(&systems, me, cc, c, 25.0, &builds, 0, sim::DT, &BTreeMap::new());
        assert_eq!(v25[1].owner, Some(rival));
        // …but still NEVER their stockpile or development tier.
        assert!(v25[1].stockpile.is_none(), "ownership visible, holdings still private");
        assert_eq!(v25[1].extractor_tier, 0, "ownership visible, development tier still private");
        assert_eq!((v25[1].slots_used, v25[1].slots_total), (0, 0), "ownership visible, slots still private");
    }

    /// SCOUT INTEL delivery obeys light (§scout part 2): the snapshot is
    /// knowledge on the scout at the capture moment — the View withholds it
    /// until that light reaches the viewer's command center, then shows the
    /// stored (aging) values. And it is the VIEWER'S OWN intel: a viewer with
    /// no snapshot sees nothing, the viewer's own systems never carry intel,
    /// and the scouted rival's view is untouched (leak checks).
    #[test]
    fn scout_intel_is_light_delayed_and_owner_only() {
        use std::collections::BTreeMap;
        let c = 300.0;
        let me = PlayerId(7);
        let rival = PlayerId(8);
        let cc = Vec2::new(0.0, 0.0);
        // Rival system 6000 su out; my scout captured intel there at t=0
        // (capture pos = the system's neighborhood → 20 s of light to me).
        let systems = vec![StarSystem {
            id: EntityId(1),
            pos: Vec2::new(6000.0, 0.0),
            name: "S".into(),
            deposits: vec![],
            claim_cost: 1000.0,
            owner: Some(rival),
            claimed_at: Some(0.0),
            stockpile: BTreeMap::new(),
            extractor_tier: 0,
            depot_tier: 0,
            shipyard_tier: 0,
            sensor_tier: 0,
            defense_tier: 0, defense_pool: 0.0,
            habitat_tier: 0,
            habitat_fed: false,
            refinery_tier: 0,
        }];
        let mut intel = BTreeMap::new();
        intel.insert(
            EntityId(1),
            sim::IntelSnapshot { defense_tier: 2, shipyard_tier: 1, observed_at: 0.0, pos: Vec2::new(6000.0, 0.0) },
        );
        let builds: Vec<sim::BuildJob> = vec![];

        // t = 10 s: the report's light (20 s) hasn't arrived — nothing shown.
        let v10 = filter_systems(&systems, me, cc, c, 10.0, &builds, 0, sim::DT, &intel);
        assert!(v10[0].intel.is_none(), "intel must not appear before its light arrives");

        // t = 25 s: delivered — the stored snapshot, aging from observed_at = 0.
        let v25 = filter_systems(&systems, me, cc, c, 25.0, &builds, 0, sim::DT, &intel);
        let iv = v25[0].intel.expect("intel delivered once its light arrives");
        assert_eq!((iv.defense_tier, iv.shipyard_tier), (2, 1));
        assert!((iv.observed_at - 0.0).abs() < 1e-9, "a snapshot keeps its capture time — it ages");

        // Leak checks: a viewer WITHOUT snapshots sees nothing…
        let v_none = filter_systems(&systems, me, cc, c, 25.0, &builds, 0, sim::DT, &BTreeMap::new());
        assert!(v_none[0].intel.is_none(), "no snapshot, no intel");
        // …and the SCOUTED RIVAL's own view is untouched: their own system never
        // carries intel (own => None), even if a stale map were passed in.
        let v_rival = filter_systems(&systems, rival, Vec2::new(6000.0, 0.0), c, 25.0, &builds, 0, sim::DT, &intel);
        assert!(v_rival[0].intel.is_none(), "the scouted side learns nothing — not even that it was scouted");
    }

    // Build a stationary ship sampled 10 Hz over [0,60] at `pos`.
    fn still_track(pos: Vec2, owner: PlayerId, kind: ShipKind) -> Track {
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 60.0 {
            samples.push(Sample { time: t, pos, vel: Vec2::ZERO });
            t += 0.1;
        }
        track_from(samples, owner, kind)
    }

    /// Certainty tracks PROXIMITY to the command center, NOT ownership (§6). An
    /// own ship far from the command center is stale and uncertain — there is no
    /// FTL tether to your own fleet — exactly like an enemy at the same distance.
    #[test]
    fn own_ship_far_is_fogged_like_an_enemy() {
        let c = 300.0;
        let cc = Vec2::new(4000.0, 0.0); // ship sits at origin → 4000 su of light away
        let owner = PlayerId(7);
        let hist = history_with(still_track(Vec2::ZERO, owner, ShipKind::Raider));

        let g_own = &hist.view_for(owner, cc, c, 50.0)[0];
        // Own AND far ⇒ uncertain. No ownership exemption.
        assert!(g_own.own);
        assert!(g_own.uncertainty > 0.0, "a distant own ship must be uncertain, not certain");
        assert!((g_own.uncertainty - g_own.age * ShipKind::Raider.max_speed()).abs() < 1e-6,
            "own uncertainty must be age × max_speed, the same formula as any object");

        // An enemy raider on the SAME track is fogged identically — same age, same
        // uncertainty. Ownership changes only the `own` flag, nothing else.
        let g_enemy = &hist.view_for(PlayerId(99), cc, c, 50.0)[0];
        assert!(!g_enemy.own);
        assert!((g_own.uncertainty - g_enemy.uncertainty).abs() < 1e-9,
            "own and enemy at the same distance from the command center are equally uncertain");
        assert!((g_own.age - g_enemy.age).abs() < 1e-9, "same distance ⇒ same staleness");
    }

    /// An own ship CLOSE to the command center is crisp — small light delay ⇒
    /// small age ⇒ near-zero uncertainty. Certainty comes from proximity, and a
    /// nearby own ship is far fresher than the same ship seen from far away.
    #[test]
    fn own_ship_near_is_crisp() {
        let c = 300.0;
        let owner = PlayerId(7);
        let hist = history_with(still_track(Vec2::ZERO, owner, ShipKind::Raider));

        // Command center 30 su away: light delay 0.1 s ⇒ uncertainty ≈ 0.1·max_speed.
        let near = &hist.view_for(owner, Vec2::new(30.0, 0.0), c, 50.0)[0];
        // Same own ship viewed from 9000 su away: ~30 s stale, far more uncertain.
        let far = &hist.view_for(owner, Vec2::new(9000.0, 0.0), c, 50.0)[0];

        assert!(near.uncertainty < far.uncertainty,
            "near {} should be far crisper than far {}", near.uncertainty, far.uncertainty);
        assert!(near.uncertainty <= 0.2 * ShipKind::Raider.max_speed(),
            "a ship right by the command center is near-certain ({} su)", near.uncertainty);
    }

    // ---- Two-tier information model (broadcast + sensor range) ----

    const VIEWER: PlayerId = PlayerId(99);
    const RIVAL: PlayerId = PlayerId(7);

    /// A rival convoy far from all the viewer's assets is STILL visible
    /// (broadcast, galaxy-wide), but its cargo is hidden (out of sensor range).
    #[test]
    fn convoy_broadcasts_but_cargo_is_hidden_out_of_range() {
        let hist = history_of(vec![at(1, 5000.0, 0.0, RIVAL, ShipKind::Convoy)], 1000.0);
        let view = hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0);
        assert_eq!(view.len(), 1, "convoy should broadcast galaxy-wide");
        assert!(view[0].cargo.is_none(), "cargo must be hidden outside sensor range");
    }

    /// A rival convoy within the viewer's sensor coverage reveals its cargo.
    #[test]
    fn convoy_cargo_revealed_within_sensor_range() {
        let hist = history_of(vec![at(1, 5000.0, 0.0, RIVAL, ShipKind::Convoy)], 1000.0);
        // Command center 200 su from the convoy → inside the 1000 su sensor range.
        let view = hist.view_for(VIEWER, Vec2::new(4800.0, 0.0), 300.0, 60.0);
        assert_eq!(view.len(), 1);
        assert!(view[0].cargo.is_some(), "cargo must be revealed within sensor range");
    }

    /// Build a multi-kind fleet track sitting still at `pos`, deriving the same
    /// scalars `record()` snapshots from a real `sim::Fleet`.
    fn fleet_track(owner: PlayerId, pos: Vec2, comp: &[(ShipKind, u32)]) -> Track {
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 100.0 {
            samples.push(Sample { time: t, pos, vel: Vec2::ZERO });
            t += 0.1;
        }
        let mut f = sim::Fleet::single(EntityId(1), owner, comp[0].0, pos, FleetOrder::Idle, None);
        f.composition.clear();
        let mut composition = BTreeMap::new();
        for (k, n) in comp {
            f.composition.insert(*k, *n);
            composition.insert(*k, *n);
        }
        Track {
            owner,
            composition,
            flagship: f.flagship_kind(),
            broadcasts: f.broadcasts(),
            sensor_mult: f.sensor_mult(),
            max_speed: f.max_speed(),
            count_class: f.count_class(),
            samples: samples.into(),
            last_seen: 100.0,
            cargo: None,
            route: None,
            destroyed: None,
        }
    }

    /// LEAK CHECK (broadcasting fleet, outside coverage): the size BUCKET is
    /// present, but the exact composition is NEVER revealed — a far observer of a
    /// broadcasting hammer knows roughly how big it is, not what's in it.
    #[test]
    fn broadcasting_fleet_shows_bucket_but_hides_composition_outside_coverage() {
        // 3 convoys + 2 corvettes + 1 raider = 6 ships (broadcasts: has convoys).
        let comp = [(ShipKind::Convoy, 3), (ShipKind::Corvette, 2), (ShipKind::Raider, 1)];
        let hist = history_of(vec![(EntityId(1), fleet_track(RIVAL, Vec2::new(5000.0, 0.0), &comp))], 1000.0);
        let view = hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0);
        assert_eq!(view.len(), 1, "the broadcasting fleet is visible galaxy-wide");
        let g = &view[0];
        assert_eq!(g.count_class, CountClass::from_count(6), "size bucket always present");
        assert_eq!(g.count_class, CountClass::FourToSeven, "6 ships → the 4–7 bucket");
        assert!(g.composition.is_none(), "composition must NOT leak outside sensor coverage");
        assert_eq!(g.kind, ShipKind::Convoy, "drawn as its flagship");
    }

    /// LEAK CHECK (the other direction — inside coverage): the exact composition
    /// is revealed within sensor range, and it matches the true makeup.
    #[test]
    fn composition_revealed_inside_sensor_coverage() {
        let comp = [(ShipKind::Convoy, 3), (ShipKind::Corvette, 2), (ShipKind::Raider, 1)];
        let hist = history_of(vec![(EntityId(1), fleet_track(RIVAL, Vec2::new(5000.0, 0.0), &comp))], 1000.0);
        // Command center 200 su from the fleet → inside the 1000 su sensor range.
        let view = hist.view_for(VIEWER, Vec2::new(4800.0, 0.0), 300.0, 60.0);
        let g = &view[0];
        let revealed = g.composition.as_ref().expect("composition revealed inside coverage");
        let got: BTreeMap<ShipKind, u32> = revealed.iter().map(|c| (c.kind, c.count)).collect();
        assert_eq!(got[&ShipKind::Convoy], 3);
        assert_eq!(got[&ShipKind::Corvette], 2);
        assert_eq!(got[&ShipKind::Raider], 1);
    }

    /// Own fleets are always exact — the owner sees their full composition even
    /// far outside any sensor bubble (the light-delay still applies to position).
    #[test]
    fn own_fleet_always_shows_exact_composition() {
        let comp = [(ShipKind::Convoy, 2), (ShipKind::Colony, 1)];
        let hist = history_of(vec![(EntityId(1), fleet_track(VIEWER, Vec2::new(8000.0, 0.0), &comp))], 500.0);
        // Viewer's own fleet, far from the command center (out of the 500 su bubble).
        let view = hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0);
        let g = &view[0];
        assert!(g.own);
        assert!(g.composition.is_some(), "own fleet composition is always exact");
    }

    /// A DARK fleet (raiders/scouts only) is omitted entirely outside coverage;
    /// when it IS seen (inside coverage) its composition shows in full — there is
    /// no half-seen dark fleet, so the reveal is consistent.
    #[test]
    fn dark_fleet_hidden_outside_but_full_composition_when_seen() {
        let comp = [(ShipKind::Raider, 4), (ShipKind::Scout, 1)];
        let hist = history_of(vec![(EntityId(1), fleet_track(RIVAL, Vec2::new(5000.0, 0.0), &comp))], 1000.0);
        // Outside coverage: omitted entirely.
        let far = hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0);
        assert!(far.is_empty(), "a dark fleet out of coverage must not appear at all");
        // Inside coverage: seen, with full composition.
        let near = hist.view_for(VIEWER, Vec2::new(4800.0, 0.0), 300.0, 60.0);
        assert_eq!(near.len(), 1);
        assert!(near[0].composition.is_some(), "a seen dark fleet shows its full composition");
        assert_eq!(near[0].count_class, CountClass::from_count(5));
    }

    /// LEAK CHECK (size bucket, big fleet): the class must be WIDE enough to
    /// contain the true count — never a tell that pins the exact number.
    #[test]
    fn count_bucket_contains_true_size_without_revealing_it() {
        let comp = [(ShipKind::Convoy, 20)]; // 20 broadcasting convoys, far away
        let hist = history_of(vec![(EntityId(1), fleet_track(RIVAL, Vec2::new(6000.0, 0.0), &comp))], 800.0);
        let g = &hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0)[0];
        assert_eq!(g.count_class, CountClass::SixteenToThirty, "20 → the 16–30 bucket");
        assert!(g.composition.is_none(), "the exact 20 is never revealed outside coverage");
    }

    /// A dark rival raider outside the viewer's sensor coverage must be OMITTED
    /// entirely from the view (not sent-and-hidden) — the fairness guarantee.
    #[test]
    fn dark_raider_omitted_outside_sensor() {
        let hist = history_of(vec![at(1, 5000.0, 0.0, RIVAL, ShipKind::Raider)], 1000.0);
        let view = hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0);
        assert!(view.is_empty(), "a dark raider out of sensor range must not appear at all");
    }

    /// The moment a rival raider enters sensor coverage it becomes a detected
    /// contact (the player's only warning).
    #[test]
    fn raider_detected_within_sensor() {
        let hist = history_of(vec![at(1, 5000.0, 0.0, RIVAL, ShipKind::Raider)], 1000.0);
        let view = hist.view_for(VIEWER, Vec2::new(4800.0, 0.0), 300.0, 60.0);
        assert_eq!(view.len(), 1, "raider within sensor range is detected");
        assert!(!view[0].own);
    }

    /// A SENSOR ARRAY (§buildings step 2b) extends the owner's coverage: a dark
    /// raider that ship/CC coverage misses is detected once an owned array bubble
    /// covers it — and rival convoy cargo is revealed at array range. The same
    /// scene WITHOUT the array (or for a viewer without one) stays dark: the
    /// array only ever ADDS vision for its owner, it leaks nothing.
    #[test]
    fn sensor_array_extends_owner_coverage() {
        let cc = Vec2::new(0.0, 0.0);
        // Raider + convoy 5000 su out — far beyond the 1000 su ship/CC bubbles.
        let hist = history_of(
            vec![
                at(1, 5000.0, 0.0, RIVAL, ShipKind::Raider),
                at(2, 5100.0, 0.0, RIVAL, ShipKind::Convoy),
            ],
            1000.0,
        );
        // No array: the raider is omitted, the convoy's cargo hidden.
        let blind = hist.view_for(VIEWER, cc, 300.0, 60.0);
        assert_eq!(blind.len(), 1, "only the broadcast convoy, no raider");
        assert!(blind[0].cargo.is_none(), "cargo hidden without the array");
        // An owned array system near them (bubble 1200 su) covers both.
        let arrays = [(Vec2::new(4600.0, 0.0), 1200.0)];
        let seen = hist.view_for_with_arrays(VIEWER, cc, 300.0, 60.0, &arrays);
        assert_eq!(seen.len(), 2, "the array detects the dark raider");
        let convoy = seen.iter().find(|g| g.kind == ShipKind::Convoy).unwrap();
        assert!(convoy.cargo.is_some(), "cargo revealed at array range");
        assert!(seen.iter().any(|g| g.kind == ShipKind::Raider), "raider detected via the array");
    }

    /// A SCOUT (§scout) projects an OVERSIZED bubble (sensor_mult × range): a
    /// dark rival raider that an ordinary ship at the same spot would miss is
    /// detected by a scout there — mobile vision, the scout's whole point.
    /// (Also proves rival convoy cargo reveals at scout range.)
    #[test]
    fn scout_bubble_out_sees_an_ordinary_ship() {
        let cc = Vec2::new(0.0, 0.0);
        // Contacts 5000 su out; own ship at 3600 → 1400 su from them: beyond the
        // 1000 su ship bubble, inside the scout's 1.5× = 1500 su bubble.
        let with_raider = history_of(
            vec![
                at(1, 5000.0, 0.0, RIVAL, ShipKind::Raider),
                at(2, 5000.0, 0.0, RIVAL, ShipKind::Convoy),
                at(3, 3600.0, 0.0, VIEWER, ShipKind::Raider),
            ],
            1000.0,
        );
        let v = with_raider.view_for(VIEWER, cc, 300.0, 60.0);
        assert!(!v.iter().any(|g| g.kind == ShipKind::Raider && !g.own), "an ordinary ship at 1400 su misses the dark raider");

        let with_scout = history_of(
            vec![
                at(1, 5000.0, 0.0, RIVAL, ShipKind::Raider),
                at(2, 5000.0, 0.0, RIVAL, ShipKind::Convoy),
                at(3, 3600.0, 0.0, VIEWER, ShipKind::Scout),
            ],
            1000.0,
        );
        let v = with_scout.view_for(VIEWER, cc, 300.0, 60.0);
        assert!(v.iter().any(|g| g.kind == ShipKind::Raider && !g.own), "the scout's oversized bubble detects it");
        let convoy = v.iter().find(|g| g.kind == ShipKind::Convoy && !g.own).unwrap();
        assert!(convoy.cargo.is_some(), "…and reveals convoy cargo at scout range");
    }

    /// A rival SCOUT runs DARK exactly like a raider: omitted entirely outside
    /// the viewer's coverage, a detected contact inside it. A spy that broadcast
    /// would be useless — and a never-detected scout leaves no trace.
    #[test]
    fn rival_scout_is_dark_outside_coverage() {
        let hist = history_of(vec![at(1, 5000.0, 0.0, RIVAL, ShipKind::Scout)], 1000.0);
        let far = hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0);
        assert!(far.is_empty(), "a dark scout out of coverage must not appear at all");
        let near = hist.view_for(VIEWER, Vec2::new(4800.0, 0.0), 300.0, 60.0);
        assert_eq!(near.len(), 1, "inside coverage it's a detected contact like any dark ship");
        assert_eq!(near[0].kind, ShipKind::Scout);
    }

    /// A player's OWN raider sits at the centre of its own sensor circle, so it
    /// is always visible to them even far from the command center.
    #[test]
    fn own_raider_is_always_visible() {
        let hist = history_of(vec![at(1, 5000.0, 0.0, VIEWER, ShipKind::Raider)], 1000.0);
        let view = hist.view_for(VIEWER, Vec2::new(0.0, 0.0), 300.0, 60.0);
        assert_eq!(view.len(), 1, "own raider must always be visible");
        assert!(view[0].own);
    }

    /// A destroyed ship must NOT vanish from all views at once — each viewer
    /// keeps seeing its ghost until the light of the destruction reaches them, so
    /// a near and a far command center see it disappear at DIFFERENT times. There
    /// is ONE destruction event; both observe it, asymmetrically.
    #[test]
    fn destroyed_ship_vanishes_per_viewer_by_light() {
        let c = 300.0;
        let dpos = Vec2::new(0.0, 0.0); // destroyed at the origin at t=10
        let near = Vec2::new(300.0, 0.0); // 1 s of light from the destruction
        let far = Vec2::new(6000.0, 0.0); // 20 s of light from the destruction
        // The (convoy, so broadcast-visible) ship sat at the origin for t∈[0,10].
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 10.0 {
            samples.push(Sample { time: t, pos: dpos, vel: Vec2::ZERO });
            t += 0.1;
        }
        let mut hist = history_of(vec![(EntityId(1), track_from(samples, RIVAL, ShipKind::Convoy))], 1e12);
        hist.mark_destroyed(EntityId(1), 10.0, dpos);

        // The near CC observes the destruction at 10 + 1 = 11.
        assert_eq!(hist.view_for(VIEWER, near, c, 10.5).len(), 1, "near still sees it alive just before its light");
        assert_eq!(hist.view_for(VIEWER, near, c, 11.5).len(), 0, "near sees it destroyed after the light arrives");

        // The far CC observes it at 10 + 20 = 30 — so at t=25 it STILL sees the
        // ship alive (flying on old light) while the near CC already saw it die.
        assert_eq!(hist.view_for(VIEWER, far, c, 25.0).len(), 1, "far still sees the (already-dead) ship alive");
        assert_eq!(hist.view_for(VIEWER, near, c, 25.0).len(), 0, "...while near has long since seen it destroyed");
        assert_eq!(hist.view_for(VIEWER, far, c, 30.5).len(), 0, "far finally sees it destroyed when its light arrives");
    }

    /// A destroyed CONVOY (moving, broadcast-visible) keeps being served as a
    /// ghost — flying on old light — across the WHOLE interval [T, T + |P−cc|/c],
    /// and vanishes only when the destruction's light reaches the viewer (the
    /// moment the yellow result ring arrives). Near and far vanish at different
    /// times, each synced to its own light. Guards the convoy-raid disappearance
    /// bug: the server must NOT drop the ghost at the TRUE destruction time.
    #[test]
    fn convoy_ghost_persists_for_the_full_interval_then_vanishes_by_light() {
        let c = 300.0;
        // Convoy flies +x (vel 10) and is destroyed at t=20 at x=200.
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 20.0 {
            samples.push(Sample { time: t, pos: Vec2::new(t * 10.0, 0.0), vel: Vec2::new(10.0, 0.0) });
            t += 0.1;
        }
        let dpos = Vec2::new(200.0, 0.0);
        let mut hist = history_of(vec![(EntityId(1), track_from(samples, RIVAL, ShipKind::Convoy))], 1e12);
        hist.mark_destroyed(EntityId(1), 20.0, dpos);

        // FAR viewer: 4500 su from the kill → 15 s of light → observed-destruction
        // at t = 35. The convoy's spawn light arrives ~t=15, so it must be visible
        // across the entire [15, 35) interval and vanish only at 35.
        let far = Vec2::new(200.0, 4500.0); // |dpos-far| = 4500
        for now in [16.0, 25.0, 30.0, 34.5] {
            assert_eq!(hist.view_for(VIEWER, far, c, now).len(), 1,
                "far viewer must still see the dead convoy flying on old light at t={now} (light lands at 35)");
        }
        assert_eq!(hist.view_for(VIEWER, far, c, 35.5).len(), 0,
            "far viewer's convoy vanishes exactly when its destruction light arrives (t=35)");

        // NEAR viewer: 600 su → 2 s of light → vanishes at t=22, 13 s before the
        // far viewer. ONE destruction, observed asymmetrically.
        let near = Vec2::new(200.0, 600.0); // |dpos-near| = 600
        assert_eq!(hist.view_for(VIEWER, near, c, 21.5).len(), 1, "near still sees it just before its light");
        assert_eq!(hist.view_for(VIEWER, near, c, 22.5).len(), 0, "near vanishes at t=22 while far waits until 35");
    }

    /// A far rival raider is dark, but if the viewer has an OWN ship near it, the
    /// union coverage detects it (coverage is the union of all assets' radii).
    #[test]
    fn own_ship_extends_coverage_to_detect_raider() {
        let hist = history_of(
            vec![
                at(1, 5000.0, 0.0, RIVAL, ShipKind::Raider),
                at(2, 5300.0, 0.0, VIEWER, ShipKind::Convoy), // own scout 300 su away
            ],
            1000.0,
        );
        // Command center far away; detection comes from the own ship's radius.
        let view = hist.view_for(VIEWER, Vec2::new(0.0, 9000.0), 300.0, 60.0);
        let raider = view.iter().find(|g| g.id == EntityId(1));
        assert!(raider.is_some(), "own ship's sensor radius should detect the nearby raider");
    }

    // ---- Raider destruction observed through the lightspeed frame (§6, RVR) ----
    //
    // A raider is only visible inside the viewer's sensor coverage, and that
    // coverage is projected by the viewer's *live* assets. When a raider battle
    // resolves, the winner breaks off home (`send_ship_home`), carrying its sensor
    // bubble away from the kill. If the dead raider and the receding winner sit at
    // DIFFERENT distances from the command center, the winner's break-off becomes
    // observable BEFORE the dead raider's destruction light — so a naive `now`-frame
    // sensor test drops the dead ghost early, leaking the kill faster than light.
    // The fix latches a destroyed raider's detection to its own retarded frame.

    // A ship sitting still at `pos` for t ∈ [0, t_end], sampled at 10 Hz.
    fn still_samples(pos: Vec2, t_end: f64) -> Vec<Sample> {
        let mut s = Vec::new();
        let mut t = 0.0;
        while t <= t_end + 1e-9 {
            s.push(Sample { time: t, pos, vel: Vec2::ZERO });
            t += 0.1;
        }
        s
    }

    // A ship at `start` until `t_turn`, then moving toward `dest` at `speed`,
    // sampled at 10 Hz over [0, t_end] — models a winner breaking off for home.
    fn recede_samples(start: Vec2, dest: Vec2, speed: f64, t_turn: f64, t_end: f64) -> Vec<Sample> {
        let unit = (dest - start).normalized();
        let mut s = Vec::new();
        let mut t = 0.0;
        while t <= t_end + 1e-9 {
            let (pos, vel) = if t <= t_turn {
                (start, Vec2::ZERO)
            } else {
                (start + unit * (speed * (t - t_turn)), unit * speed)
            };
            s.push(Sample { time: t, pos, vel });
            t += 0.1;
        }
        s
    }

    fn dead_track(samples: Vec<Sample>, owner: PlayerId, t: f64, pos: Vec2) -> Track {
        let mut tr = track_from(samples, owner, ShipKind::Raider);
        tr.destroyed = Some((t, pos));
        tr
    }

    // Walk `now` forward; return the first `now` at which `ship` disappears from the
    // viewer's view after having been visible (its observed-destruction instant).
    fn vanish_time(hist: &PositionHistory, viewer: PlayerId, cc: Vec2, ship: EntityId, from: f64, to: f64) -> Option<f64> {
        let (mut now, mut seen) = (from, false);
        while now <= to {
            let present = hist.view_for(viewer, cc, 300.0, now).iter().any(|g| g.id == ship);
            if present {
                seen = true;
            } else if seen {
                return Some(now);
            }
            now += 0.1;
        }
        None
    }

    /// THE CORE REGRESSION. The viewer's own raider wins and breaks off home; the
    /// dead RIVAL raider must keep ghosting until the destruction's light reaches
    /// the viewer (T + |P−cc|/c), NOT vanish early when the winner recedes.
    #[test]
    fn destroyed_rival_raider_no_ftl_leak_when_winner_breaks_off() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        let p = Vec2::new(1500.0, 0.0); // dead rival, 5 s of light from cc
        let t = 10.0;
        let honest = t + p.distance(cc) / c; // = 15.0
        // Own attacker sat at (1300,0) (4.33 s of light) until T, then recedes home.
        let attacker = recede_samples(Vec2::new(1300.0, 0.0), cc, 250.0, t, 25.0);
        let hist = history_of(
            vec![
                (EntityId(1), dead_track(still_samples(p, t), RIVAL, t, p)),
                (EntityId(2), track_from(attacker, VIEWER, ShipKind::Raider)),
            ],
            250.0, // sensor range — tight, so the skew matters
        );
        // Sanity: before the destruction light, the dead rival IS visible.
        assert!(hist.view_for(VIEWER, cc, c, 13.0).iter().any(|g| g.id == EntityId(1)),
            "dead rival should still be a ghost well before its light arrives");
        let vanish = vanish_time(&hist, VIEWER, cc, EntityId(1), 10.0, 16.0)
            .expect("the dead rival must eventually be observed destroyed");
        assert!(vanish >= honest - 0.15,
            "FTL LEAK: dead rival raider vanished at {vanish:.2}s but its destruction light \
             only reaches the viewer at {honest:.2}s — the kill leaked {:.2}s faster than light",
            honest - vanish);
    }

    /// The viewer's OWN raider is the one destroyed (a rival won and recedes). The
    /// own dead raider seeds its own coverage, so it must vanish exactly on its own
    /// light — the new latch must not SHORTEN this.
    #[test]
    fn own_destroyed_raider_vanishes_on_its_own_light() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        let p = Vec2::new(1500.0, 0.0);
        let t = 10.0;
        let honest = t + p.distance(cc) / c; // 15.0
        let hist = history_of(
            vec![(EntityId(1), dead_track(still_samples(p, t), VIEWER, t, p))],
            250.0,
        );
        let vanish = vanish_time(&hist, VIEWER, cc, EntityId(1), 10.0, 16.0)
            .expect("own dead raider must eventually be observed destroyed");
        assert!((vanish - honest).abs() < 0.2,
            "own dead raider should vanish at its light {honest:.2}s, got {vanish:.2}s");
    }

    /// BOTH raiders destroyed at distinct distances. Each must vanish at ITS OWN
    /// honest light, never at the (earlier) battle-geometry time.
    #[test]
    fn both_destroyed_each_vanishes_on_its_own_light() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        let p_own = Vec2::new(1300.0, 0.0); // own dead raider, 4.33 s light
        let p_enemy = Vec2::new(1500.0, 0.0); // enemy dead raider, 5 s light (200 su apart)
        let t = 10.0;
        let honest_own = t + p_own.distance(cc) / c; // 14.33
        let honest_enemy = t + p_enemy.distance(cc) / c; // 15.0
        let hist = history_of(
            vec![
                (EntityId(1), dead_track(still_samples(p_own, t), VIEWER, t, p_own)),
                (EntityId(2), dead_track(still_samples(p_enemy, t), RIVAL, t, p_enemy)),
            ],
            250.0,
        );
        let v_own = vanish_time(&hist, VIEWER, cc, EntityId(1), 10.0, 16.0).expect("own should vanish");
        let v_enemy = vanish_time(&hist, VIEWER, cc, EntityId(2), 10.0, 16.0).expect("enemy should vanish");
        assert!((v_own - honest_own).abs() < 0.2, "own dead vanish {v_own:.2} != light {honest_own:.2}");
        assert!(v_enemy >= honest_enemy - 0.15,
            "FTL LEAK: enemy dead raider vanished at {v_enemy:.2}s, light arrives {honest_enemy:.2}s");
    }

    /// EXISTENCE GUARD. A dead rival raider the viewer never had sensors on must
    /// stay invisible at every instant — the latch may only confirm a real past
    /// detection, never conjure a never-tracked raider into view.
    #[test]
    fn destroyed_raider_never_detected_stays_invisible() {
        let c = 300.0;
        let cc = Vec2::new(0.0, 0.0);
        let p = Vec2::new(1500.0, 0.0); // far outside the 250 su cc range; no own assets
        let hist = history_of(
            vec![(EntityId(1), dead_track(still_samples(p, 10.0), RIVAL, 10.0, p))],
            250.0,
        );
        let mut now = 0.0;
        while now <= 20.0 {
            assert!(hist.view_for(VIEWER, cc, c, now).is_empty(),
                "a never-detected dead raider must never appear (existence leak at t={now:.1})");
            now += 0.25;
        }
    }
}

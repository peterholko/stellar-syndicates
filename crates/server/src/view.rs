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

use sim::{Cargo, Commodity, EntityId, HomeSlot, PlayerId, ShipKind, ShipOrder, StarSystem, Vec2, World};

use crate::protocol::{AnchorView, CargoView, GhostView, StockSlot, SystemStateView};

/// One recorded true state of a ship at a sim time.
#[derive(Clone, Copy)]
struct Sample {
    time: f64,
    pos: Vec2,
    vel: Vec2,
}

/// Position history + current metadata for one ship.
struct Track {
    owner: PlayerId,
    kind: ShipKind,
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
        for (id, ship) in &world.ships {
            let track = self.tracks.entry(*id).or_insert_with(|| Track {
                owner: ship.owner,
                kind: ship.kind,
                samples: VecDeque::new(),
                last_seen: now,
                cargo: None,
                route: None,
                destroyed: None,
            });
            track.owner = ship.owner;
            track.kind = ship.kind;
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
    pub fn view_for(&self, viewer: PlayerId, cc: Vec2, c: f64, now: f64) -> Vec<GhostView> {
        // Pass 1: retarded ghost for every observable ship, and gather the
        // viewer's sensor coverage (command center + their own ships' ghosts).
        struct Pre<'a> {
            id: EntityId,
            owner: PlayerId,
            kind: ShipKind,
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
        let mut coverage = vec![cc]; // the command center is itself an asset
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
                coverage.push(sample.pos);
            }
            // For a destroyed raider, decide visibility in the ghost's OWN retarded
            // frame (the world as the arriving light shows it), not the `now` frame
            // whose coverage already reflects the post-kill break-off.
            let destroyed_detected = track.destroyed.is_some()
                && track.kind == ShipKind::Raider
                && self.detected_at_retarded_time(viewer, cc, sample.pos, sample.time);
            pre.push(Pre {
                id: *id,
                owner: track.owner,
                kind: track.kind,
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
            let detected =
                p.destroyed_detected || within_sensor(&coverage, p.sample.pos, self.sensor_range);

            // A dark raider is present ONLY inside sensor coverage. (A player's
            // own raider sits at the centre of its own sensor circle, so it is
            // always present.) Omitted entirely otherwise — never sent-and-hidden.
            if p.kind == ShipKind::Raider && !detected {
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
            let uncertainty = age * p.kind.max_speed();
            let is_convoy = p.kind == ShipKind::Convoy;
            // Convoys broadcast their route; cargo only within sensor coverage.
            let route = if is_convoy { p.route.clone() } else { None };
            let cargo = if is_convoy && detected {
                p.cargo.map(|cg| CargoView {
                    commodity: cg.commodity,
                    units: cg.units,
                })
            } else {
                None
            };

            ghosts.push(GhostView {
                id: p.id,
                owner: p.owner,
                kind: p.kind,
                pos: p.sample.pos,
                vel: p.sample.vel,
                age,
                uncertainty,
                own,
                route,
                cargo,
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
    fn detected_at_retarded_time(&self, viewer: PlayerId, cc: Vec2, ghost_pos: Vec2, t_r: f64) -> bool {
        // The command center is a fixed sensor asset.
        if ghost_pos.distance(cc) <= self.sensor_range {
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
                && s.pos.distance(ghost_pos) <= self.sensor_range
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
pub fn filter_systems(
    systems: &[StarSystem],
    viewer: PlayerId,
    cc: Vec2,
    c: f64,
    now: f64,
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
            SystemStateView {
                id: sys.id,
                owner,
                stockpile,
            }
        })
        .collect()
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
fn within_sensor(centers: &[Vec2], p: Vec2, range: f64) -> bool {
    centers.iter().any(|center| p.distance(*center) <= range)
}

/// The broadcast route (waypoints) implied by a ship's current order, if any.
fn route_of(order: &ShipOrder) -> Option<Vec<Vec2>> {
    match order {
        ShipOrder::Patrol { waypoints, .. } => Some(waypoints.clone()),
        ShipOrder::MoveTo { dest } => Some(vec![*dest]),
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
        Track {
            owner,
            kind,
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
            HomeSlot { pos: Vec2::new(0.0, 0.0), owner: Some(me), claimed_at: Some(0.0) },
            // Rival's anchor, 6000 units away → 20 s of light.
            HomeSlot { pos: Vec2::new(6000.0, 0.0), owner: Some(rival), claimed_at: Some(0.0) },
            HomeSlot { pos: Vec2::new(0.0, 3000.0), owner: None, claimed_at: None },
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
        };
        let systems = vec![
            mk(1, Vec2::new(0.0, 0.0), "MINE", Some(me), Some(0.0), &[(Commodity::Alloys, 12.7)]),
            // Rival's claim 6000 su away → 20 s of light.
            mk(2, Vec2::new(6000.0, 0.0), "RIVAL", Some(rival), Some(0.0), &[(Commodity::Ore, 99.0)]),
            mk(3, Vec2::new(0.0, 3000.0), "FREE", None, None, &[]),
        ];

        // At t=10 s the rival's claim light (20 s) has NOT arrived.
        let v10 = filter_systems(&systems, me, cc, c, 10.0);
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

        // At t=25 s the rival's claim light has arrived — ownership now visible…
        let v25 = filter_systems(&systems, me, cc, c, 25.0);
        assert_eq!(v25[1].owner, Some(rival));
        // …but still NEVER their stockpile.
        assert!(v25[1].stockpile.is_none(), "ownership visible, holdings still private");
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

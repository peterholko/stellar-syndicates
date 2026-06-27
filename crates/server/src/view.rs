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

use sim::{Cargo, Commodity, EntityId, HomeSlot, PlayerId, ShipKind, ShipOrder, Vec2, World};

use crate::protocol::{AnchorView, CargoView, GhostView};

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
        // (their last light has passed). Keeps memory bounded as ships are
        // destroyed in later milestones.
        let horizon = self.horizon;
        self.tracks
            .retain(|_, t| now - t.last_seen <= horizon);
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
        }
        let mut pre = Vec::new();
        let mut coverage = vec![cc]; // the command center is itself an asset
        for (id, track) in &self.tracks {
            let Some(sample) = latest_observable(&track.samples, cc, c, now) else {
                continue; // dark — no light from this object has arrived yet
            };
            if track.owner == viewer {
                coverage.push(sample.pos);
            }
            pre.push(Pre {
                id: *id,
                owner: track.owner,
                kind: track.kind,
                sample,
                cargo: track.cargo,
                route: &track.route,
            });
        }

        // Pass 2: apply the two-tier visibility rules using the coverage.
        let mut ghosts = Vec::new();
        for p in pre {
            let detected = within_sensor(&coverage, p.sample.pos, self.sensor_range);

            // A dark raider is present ONLY inside sensor coverage. (A player's
            // own raider sits at the centre of its own sensor circle, so it is
            // always present.) Omitted entirely otherwise — never sent-and-hidden.
            if p.kind == ShipKind::Raider && !detected {
                continue;
            }

            let age = now - p.sample.time;
            let own = p.owner == viewer;
            let uncertainty = if own { 0.0 } else { age * p.kind.max_speed() };
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

    /// Own ships are coherent (zero uncertainty); others grow an uncertainty
    /// cone with staleness.
    #[test]
    fn own_is_certain_other_is_fogged() {
        let c = 300.0;
        let cc = Vec2::new(4000.0, 0.0);
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t <= 60.0 {
            samples.push(Sample { time: t, pos: Vec2::new(0.0, 0.0), vel: Vec2::ZERO });
            t += 0.1;
        }
        let owner = PlayerId(7);
        let hist = history_with(track_from(samples, owner, ShipKind::Raider));

        let g_own = &hist.view_for(owner, cc, c, 50.0)[0];
        assert_eq!(g_own.uncertainty, 0.0);
        assert!(g_own.own);

        let g_other = &hist.view_for(PlayerId(99), cc, c, 50.0)[0];
        assert!(g_other.uncertainty > 0.0);
        assert!(!g_other.own);
        // Uncertainty == age * max_speed.
        assert!((g_other.uncertainty - g_other.age * ShipKind::Raider.max_speed()).abs() < 1e-6);
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
}

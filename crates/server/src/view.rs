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

use std::collections::HashMap;
use std::collections::VecDeque;

use sim::{EntityId, HomeSlot, PlayerId, ShipKind, Vec2, World};

use crate::protocol::{AnchorView, GhostView};

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
}

/// The view filter's state: every moving object's recent true-position history.
pub struct PositionHistory {
    tracks: HashMap<EntityId, Track>,
    /// How many seconds of history to retain. Must exceed the largest possible
    /// light delay (galaxy diameter / c) so every long-lived object always has
    /// an observable sample.
    horizon: f64,
}

impl PositionHistory {
    /// Create a history sized to a world's maximum possible light delay, with a
    /// safety margin.
    pub fn for_world(world: &World) -> Self {
        let max_delay = (2.0 * world.config.galaxy_radius) / world.config.c;
        PositionHistory {
            tracks: HashMap::new(),
            horizon: max_delay * 1.25 + 1.0,
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
            });
            track.owner = ship.owner;
            track.kind = ship.kind;
            track.last_seen = now;
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

    /// Build the delayed/fogged view of all ships for a player whose command
    /// center is at `cc`, at wall-time `now`.
    pub fn view_for(&self, viewer: PlayerId, cc: Vec2, c: f64, now: f64) -> Vec<GhostView> {
        let mut ghosts = Vec::new();
        for (id, track) in &self.tracks {
            let Some(sample) = latest_observable(&track.samples, cc, c, now) else {
                // No light from this object has arrived yet (e.g. just spawned
                // far away) — it is simply dark to this player.
                continue;
            };
            let age = now - sample.time;
            let own = track.owner == viewer;
            // Own forces: a delayed-but-coherent feed — you know exactly where
            // they were, so no positional uncertainty. Others: the object could
            // have moved up to `max_speed · age` since the light left.
            let uncertainty = if own { 0.0 } else { age * track.kind.max_speed() };
            ghosts.push(GhostView {
                id: *id,
                owner: track.owner,
                kind: track.kind,
                pos: sample.pos,
                vel: sample.vel,
                age,
                uncertainty,
                own,
            });
        }
        // Deterministic ordering by id.
        ghosts.sort_by_key(|g| g.id.0);
        ghosts
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
        Track {
            owner,
            kind,
            samples: samples.into(),
            last_seen: last,
        }
    }

    fn history_with(track: Track) -> PositionHistory {
        let mut tracks = HashMap::new();
        tracks.insert(EntityId(1), track);
        PositionHistory { tracks, horizon: 1e9 }
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
}

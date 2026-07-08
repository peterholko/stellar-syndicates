//! SYNDICATES (§syndicates) — the social layer: corporations band into mutual
//! non-engagement pacts that also share intel (Part 2) and defend/supply each
//! other (Part 3).
//!
//! A [`Syndicate`] is an AFFILIATION, never an owner: fleets and systems still
//! belong to individual [`crate::ids::PlayerId`] corps, and battles/doctrine/intel
//! stay per-corp. The syndicate only changes friend-vs-foe (`World::are_allied`)
//! and what a viewer's light-delayed picture reveals.
//!
//! Membership is stored twice, kept in sync: the roster lives HERE (`members`,
//! for the panel + invites + cap), and a denormalized `syndicate` id on each
//! [`crate::world::Corporation`] gives O(1) `are_allied` and carries the
//! light-delay bookkeeping (a 2-state `prev`/`since` history) so distant players
//! learn of a join/leave only after the light from that corp's command center
//! arrives — membership propagates exactly like ownership.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ids::{PlayerId, SyndicateId};

/// SIZE CAP: a syndicate may hold at most this FRACTION of the active
/// corporations, so one coalition can't absorb the galaxy. Playtest placeholder.
pub const SYNDICATE_MAX_FRAC: f64 = 1.0 / 3.0;
/// A floor so a small galaxy can still form a 2-corp pact. Playtest placeholder.
pub const SYNDICATE_MIN_CAP: usize = 2;

/// The maximum member count for a syndicate given the number of ACTIVE
/// corporations (`World.players.len()`): `max(MIN_CAP, ⌊active·MAX_FRAC⌋)`.
pub fn syndicate_cap(active_corps: usize) -> usize {
    ((active_corps as f64 * SYNDICATE_MAX_FRAC).floor() as usize).max(SYNDICATE_MIN_CAP)
}

/// One alliance. Founder-managed in v1 (roles deferred): the founder invites,
/// dissolves, and — should they leave — hands the seat to the next member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Syndicate {
    pub id: SyndicateId,
    pub name: String,
    /// The managing corp (invites / dissolve). Reassigned if the founder leaves.
    pub founder: PlayerId,
    /// The roster, INCLUDING the founder. `BTreeSet` = deterministic iteration.
    pub members: BTreeSet<PlayerId>,
    /// Outstanding founder-issued invitations, consumed on accept (or dropped
    /// when the invitee joins elsewhere / the syndicate dissolves).
    #[serde(default)]
    pub invites: BTreeSet<PlayerId>,
    /// Sim-time the syndicate was founded (informational / roster display).
    pub created_at: f64,
}

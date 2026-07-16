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

/// §fitting: how many DOCTRINE FITS a syndicate may hold (soft-reject beyond —
/// replacing a same-name fit is always allowed). Tunable.
pub const SYNDICATE_MAX_FITS: usize = 24;
/// §fitting: fit names are capped at this many chars (trimmed; sim-side soft
/// sanitize, same discipline as syndicate names).
pub const FIT_NAME_MAX: usize = 24;

/// §fitting: one saved DOCTRINE FIT — a named (hull, loadout) the whole
/// syndicate can build from or refit to. Validated against the hull's slots +
/// fitting budget at SAVE time, so every stored fit is legal by construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctrineFit {
    pub name: String,
    pub kind: crate::ship::ShipKind,
    pub loadout: crate::module::Loadout,
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
    /// §research: the syndicate-wide Programme Boards state — active/queue,
    /// progress, completed set, verb counters, designations. serde-default so
    /// every old snapshot loads with empty research and ticks clean (no
    /// migration). Owner-only in the view.
    #[serde(default)]
    pub research: crate::research::ResearchState,
    /// §fitting: the syndicate's saved DOCTRINE FITS (any member saves /
    /// deletes; cap [`SYNDICATE_MAX_FITS`]). serde-default — old snapshots
    /// load with none. Owner-only in the view; fit NAMES never touch the sim
    /// state hash-relevant paths (they are labels on build inputs).
    #[serde(default)]
    pub fits: Vec<DoctrineFit>,
    /// §ladder B4: the syndicate's FLAGSHIP NAME — its one Titan, christened.
    /// Renders wherever the Titan stack appears for the OWNER side; rivals
    /// learn it only through participant battle records (never buckets).
    /// Cleared when the Titan dies (the headline). serde-default.
    #[serde(default)]
    pub flagship_name: Option<String>,
}

/// §ladder B4: flagship names cap at this many chars (same discipline as fits).
pub const FLAGSHIP_NAME_MAX: usize = 24;

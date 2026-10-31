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

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::ids::{PlayerId, SyndicateId};

/// SIZE CAP: a syndicate may hold at most this FRACTION of the active
/// corporations, so one coalition can't absorb the galaxy. Playtest placeholder.
pub const SYNDICATE_MAX_FRAC: f64 = 1.0 / 3.0;
/// A floor so a small galaxy can still form a 2-corp pact. Playtest placeholder.
pub const SYNDICATE_MIN_CAP: usize = 2;

/// §settle-near-friends (R7): a person's corporation counts toward seats while
/// it has given an order within this window (or is on leave).
pub const ACTIVE_WINDOW_S: f64 = 7.0 * 86_400.0;

/// The maximum member count for a syndicate given the number of ACTIVE HUMAN
/// corporations (see `World::active_human`): `max(MIN_CAP, ⌊active·MAX_FRAC⌋)`.
/// Bots and members idle past [`ACTIVE_WINDOW_S`] take no seat.
pub fn syndicate_cap(active_corps: usize) -> usize {
    ((active_corps as f64 * SYNDICATE_MAX_FRAC).floor() as usize).max(SYNDICATE_MIN_CAP)
}

/// §fitting: how many DOCTRINE FITS a syndicate may hold (soft-reject beyond —
/// replacing a same-name fit is always allowed). Tunable.
pub const SYNDICATE_MAX_FITS: usize = 24;
/// §fitting: fit names are capped at this many chars (trimmed; sim-side soft
/// sanitize, same discipline as syndicate names).
pub const FIT_NAME_MAX: usize = 24;

/// Syndicate permissions without turning the syndicate into an owner. Roles
/// authorize administration and shared operations; fleets, systems, goods and
/// rewards remain corporation property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyndicateRole {
    Member,
    Quartermaster,
    Officer,
    Founder,
}

/// §fitting: one saved DOCTRINE FIT — a named (hull, loadout) the whole
/// syndicate can build from or refit to. Validated against the hull's slots +
/// fitting budget at SAVE time, so every stored fit is legal by construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctrineFit {
    pub name: String,
    pub kind: crate::ship::ShipKind,
    pub loadout: crate::module::Loadout,
}

/// One alliance with explicit administrative roles. The founder retains final
/// dissolution authority and — should they leave — hands the seat to the next
/// member; officers and quartermasters receive only their published permissions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Syndicate {
    pub id: SyndicateId,
    pub name: String,
    /// The managing corp (invites / dissolve). Reassigned if the founder leaves.
    pub founder: PlayerId,
    /// The roster, INCLUDING the founder. `BTreeSet` = deterministic iteration.
    pub members: BTreeSet<PlayerId>,
    /// Per-member permissions. Missing legacy entries read as Member except for
    /// `founder`, which is always Founder by invariant.
    #[serde(default)]
    pub roles: BTreeMap<PlayerId, SyndicateRole>,
    /// Outstanding founder-issued invitations, consumed on accept (or dropped
    /// when the invitee joins elsewhere / the syndicate dissolves).
    #[serde(default)]
    pub invites: BTreeSet<PlayerId>,
    /// Sim-time the syndicate was founded (informational / roster display).
    pub created_at: f64,
    /// LEGACY §research snapshot carrier. Programme Boards are corporation-owned
    /// now; `World::fixup_after_load` copies this state to every current member
    /// once so existing campaigns lose no progress. New games leave it empty.
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
    /// §syndicate-projects (R6): the weekly ladder and the bonuses it bought.
    #[serde(default)]
    pub projects: SyndicateProjects,
}

// ─── §syndicate-projects (R6) ───────────────────────────────────────────────
// A weekly ladder of shared deliveries. Members give goods from their own
// worlds, by choice; each finished rung earns project points, and points buy
// lasting bonuses for every member. Nothing is levied, and leaving costs the
// bonuses, never goods. New members wait 24–96 h for the higher levels.

/// A lasting syndicate bonus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyndicateBonus {
    /// Player freighters' holds.
    Freight,
    /// Academy output.
    Research,
    /// Settlers arriving.
    Migration,
}

impl SyndicateBonus {
    pub const ALL: [SyndicateBonus; 3] = [Self::Freight, Self::Research, Self::Migration];

    /// The gain per level, as a fraction.
    pub fn per_level(self) -> f64 {
        match self {
            Self::Freight => 0.06,
            Self::Research => 0.04,
            Self::Migration => 0.08,
        }
    }
}

pub const BONUS_MAX_LEVEL: u8 = 5;
/// Project points to raise a bonus to levels I–V.
pub const BONUS_LEVEL_COST: [u32; 5] = [3, 6, 10, 15, 21];
/// How long a member must have belonged before each level reaches it.
pub const BONUS_LEVEL_WAIT_S: [f64; 5] = [0.0, 24.0 * 3_600.0, 48.0 * 3_600.0, 72.0 * 3_600.0, 96.0 * 3_600.0];
/// The week's rungs: goods to deliver, and the points each earns.
pub const LADDER_GOALS: [u32; 5] = [150, 400, 900, 1_800, 3_200];
pub const LADDER_POINTS: [u32; 5] = [1, 2, 3, 4, 5];
/// The goods the rungs ask for, turning with the week.
pub const LADDER_GOODS: [crate::cargo::Commodity; 5] = [
    crate::cargo::Commodity::Alloys,
    crate::cargo::Commodity::Electronics,
    crate::cargo::Commodity::Machinery,
    crate::cargo::Commodity::Polymers,
    crate::cargo::Commodity::Provisions,
];

/// The goods rung `rung` asks for in week `week`.
pub fn rung_goods(week: u32, rung: usize) -> crate::cargo::Commodity {
    LADDER_GOODS[(week as usize + rung) % LADDER_GOODS.len()]
}

/// The levels of a bonus at `level` that have reached a member who joined
/// `tenure` seconds ago.
pub fn level_reaching(level: u8, tenure: f64) -> u8 {
    (0..level as usize).take_while(|&i| tenure + 1e-9 >= BONUS_LEVEL_WAIT_S[i]).count() as u8
}

/// A syndicate's ladder this week and the bonuses it has bought.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyndicateProjects {
    /// The medal week (1-based) this ladder belongs to.
    #[serde(default)]
    pub week: u32,
    /// The rung in progress; `LADDER_GOALS.len()` once the week's ladder is done.
    #[serde(default)]
    pub rung: usize,
    #[serde(default)]
    pub progress: u32,
    /// Points earned and not yet spent.
    #[serde(default)]
    pub points: u32,
    /// Points earned this week: the weekly podium.
    #[serde(default)]
    pub week_points: u32,
    #[serde(default)]
    pub levels: BTreeMap<SyndicateBonus, u8>,
    /// Goods each member gave this week.
    #[serde(default)]
    pub given: BTreeMap<PlayerId, u32>,
    /// Weekly podium places won: (week, place).
    #[serde(default)]
    pub medals: Vec<(u32, u8)>,
}

impl SyndicateProjects {
    pub fn level(&self, bonus: SyndicateBonus) -> u8 {
        self.levels.get(&bonus).copied().unwrap_or(0)
    }

    /// Points to raise `bonus` one level; None at the top.
    pub fn next_cost(&self, bonus: SyndicateBonus) -> Option<u32> {
        let level = self.level(bonus);
        (level < BONUS_MAX_LEVEL).then(|| BONUS_LEVEL_COST[level as usize])
    }

    /// The rung in progress's goal; None once the week's ladder is done.
    pub fn goal(&self) -> Option<u32> {
        LADDER_GOALS.get(self.rung).copied()
    }

    /// A fresh ladder for `week`, keeping points, levels and medals.
    pub fn start_week(&mut self, week: u32) {
        self.week = week;
        self.rung = 0;
        self.progress = 0;
        self.week_points = 0;
        self.given.clear();
    }
}

impl Syndicate {
    pub fn role_of(&self, player: PlayerId) -> Option<SyndicateRole> {
        if !self.members.contains(&player) {
            None
        } else if player == self.founder {
            Some(SyndicateRole::Founder)
        } else {
            Some(
                self.roles
                    .get(&player)
                    .copied()
                    .unwrap_or(SyndicateRole::Member),
            )
        }
    }

    pub fn may_manage_members(&self, player: PlayerId) -> bool {
        matches!(
            self.role_of(player),
            Some(SyndicateRole::Founder | SyndicateRole::Officer)
        )
    }

    pub fn may_manage_operations(&self, player: PlayerId) -> bool {
        matches!(
            self.role_of(player),
            Some(
                SyndicateRole::Founder
                    | SyndicateRole::Officer
                    | SyndicateRole::Quartermaster
            )
        )
    }

}

/// §ladder B4: flagship names cap at this many chars (same discipline as fits).
pub const FLAGSHIP_NAME_MAX: usize = 24;

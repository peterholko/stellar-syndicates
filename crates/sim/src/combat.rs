//! Combat TYPES + battle RECORDS (§FLEETS Part 2, §battle-records, §tactical).
//!
//! §tactical SUPERSESSION: the pooled Lanchester attrition engine that lived
//! here (`attrition_tick` / `absorb` / `project_engagement`) is REPLACED by the
//! individual-ship tactical engine in [`crate::tactical`] — battles unpack into
//! positioned combatants, fight with seeded battle-isolated dice, and repack
//! into count-stacks. What remains here is engine-agnostic:
//!
//!   * the strategic COMBAT TYPES ([`Losses`], [`TypedDamage`], [`LoadoutMap`],
//!     [`StackPoolMap`], the [`Forces`] compositional container used for
//!     strength-only reads and the typical-fleet assumption);
//!   * the shared TUNABLES both layers read (raid caps, disengage exposure,
//!     hull accounting weights, platform tiers);
//!   * the BATTLE RECORDS machinery (recorder, retention, pruning) — a pure
//!     observer of whatever engine resolves the fight.
//!
//! Everything here stays pure and deterministic; the dice live in
//! [`crate::tactical`] behind a per-battle RNG that never touches world state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{DT, TICK_HZ};
use crate::doctrine::EngagementPolicy;
use crate::event::RaidOutcome;
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::module::Loadout;
use crate::ship::ShipKind;

/// A per-side per-fleet LOADOUT partition (§modules): `kind → loadout key →
/// count`, storing only NON-default (fitted) stacks. The wire/type form used by
/// the tactical unpack ([`crate::tactical::stacked`]) and the fleet partition
/// ([`crate::ship::Fleet`]).
pub type LoadoutMap = std::collections::BTreeMap<ShipKind, std::collections::BTreeMap<String, u32>>;

/// A per-STACK damage pool (§modules): `kind → loadout key → pool`. Persisted on
/// the engagement between ticks so each `(kind, loadout)` stack keeps its OWN
/// accumulated absorption (armored stacks that took less genuinely die less). A
/// nested map with STRING inner keys, so it round-trips through JSON (a tuple
/// `(kind, loadout)` map key would not).
pub type StackPoolMap = std::collections::BTreeMap<ShipKind, std::collections::BTreeMap<String, f64>>;

/// A TYPED damage 3-vector (§modules Part B) — the wire/report shape for
/// damage broken out by weapon family. The tactical engine deals per-hit typed
/// damage internally; this type survives as the strategic-layer vocabulary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct TypedDamage {
    pub beam: f64,
    pub driver: f64,
    pub torpedo: f64,
}

impl TypedDamage {
    pub fn total(&self) -> f64 {
        self.beam + self.driver + self.torpedo
    }
}

// --- TUNABLE COMBAT BLOCK (strategic-layer knobs) --------------------------
// §tactical supersession: the per-tick damage-rate ladder (`dmg_rate` /
// `DMG_RATE_CALIBRATION` / `RAID_RATE`) is DELETED — battle pacing is now set
// in [`crate::tactical`] by the step cadence ([`crate::tactical::tac_step_ticks`])
// and the to-hit/damage calibration ([`crate::tactical::HIT_DMG_CAL`],
// [`crate::tactical::RAID_DMG_MULT`]). What remains below are the knobs the
// STRATEGIC layer still owns: engagement windows, disengage exposure, and the
// hull/platform accounting weights.
/// A raid engagement ends after at most this fraction of `battle_target_secs`
/// (whichever comes first with cargo-seized / retreat) — raids stay quick
/// smash-and-grabs even as battles get slow. Tunable.
// §tactical: raised from 0.15 — raiders now physically CLOSE from standoff
// before the guns bear, so the smash-and-grab window absorbs the approach.
pub const RAID_CAP_FRAC: f64 = 0.35;
/// SAFETY VALVE: an engagement that has run this multiple of `battle_target_secs`
/// without either side hitting a retreat threshold forces a MUTUAL DISENGAGE —
/// no infinite grind between two no-retreat (doctrine Never) fleets. Tunable.
pub const MAX_BATTLE_MULT: f64 = 2.0;
/// The brief PARTING-SHOT exposure (seconds) a fleet that does NOT accept a
/// battle takes before its physical disengagement completes (§engagement
/// movement — the anti-lock rule). A raider jumped on AVOID doctrine suffers this
/// short scrape, then the SPEED TABLE decides whether it opens the gap or is
/// caught. Independent of the battle timescale (a scrape is quick). Tunable.
pub const DISENGAGE_EXPOSURE_SECS: f64 = 3.0;
/// Hull per point of defense weight (see [`ShipKind::hull`]).
pub const HULL_PER_DEFENSE: f64 = 10.0;
/// Minimum hull, so a zero-defense scout is still attritable (dies fast).
pub const HULL_MIN: f64 = 2.0;
/// A defense-platform tier as a combatant: its hull and its return fire. Keeps
/// the ram-attrition behaviour — a raider grinds through platform tiers, and the
/// platform shoots back.
pub const PLATFORM_TIER_HULL: f64 = 30.0;
pub const PLATFORM_TIER_ATTACK: f64 = 3.0;

/// Whole units lost in an attrition step. `per_kind` is the summed view (for
/// reports/records); `per_stack` keys by `(kind, loadout)` so the losing fleets
/// can shed the RIGHT loadout stacks (§modules — an armored ship that absorbed
/// less dies less). Both stay in step. Plus destroyed platform tiers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Losses {
    pub per_kind: BTreeMap<ShipKind, u32>,
    pub per_stack: BTreeMap<(ShipKind, Loadout), u32>,
    pub platform_tiers: u32,
}

impl Losses {
    pub fn is_empty(&self) -> bool {
        self.platform_tiers == 0 && self.per_kind.values().all(|n| *n == 0)
    }
    pub fn total_ships(&self) -> u32 {
        self.per_kind.values().copied().sum()
    }
    /// Record `n` ships of `(kind, loadout)` lost — updates BOTH views.
    pub fn add_stack(&mut self, kind: ShipKind, loadout: Loadout, n: u32) {
        if n > 0 {
            *self.per_kind.entry(kind).or_insert(0) += n;
            *self.per_stack.entry((kind, loadout)).or_insert(0) += n;
        }
    }
    /// Record `n` UNFITTED ships of `kind` lost (e.g. scouts stripped pre-tick).
    pub fn add_kind(&mut self, kind: ShipKind, n: u32) {
        self.add_stack(kind, Loadout::default(), n);
    }
}

/// A combat STACK key: same kind + same [`Loadout`] fight and die together.
pub type StackKey = (ShipKind, Loadout);

/// One side of an engagement as a COMPOSITIONAL container, partitioned into
/// `(kind, loadout)` STACKS (§modules), plus optional defense-platform tiers.
///
/// §tactical supersession: this no longer FIGHTS — `absorb`/`typed_attack`/
/// `attrition_tick` are deleted; battles resolve in [`crate::tactical`]. What
/// survives is the strength-only read the strategic layer still needs
/// (pirate/AI heuristics via [`Forces::strength`]) and the typical-fleet
/// assumption the Monte Carlo calculator builds on ([`typical_forces`] +
/// [`Forces::comp`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Forces {
    pub stacks: BTreeMap<StackKey, u32>,
    pub damage: BTreeMap<StackKey, f64>,
    pub platform_tiers: u32,
    pub platform_pool: f64,
}

impl Forces {
    /// A side of ALL-UNFITTED ships from a per-kind composition + damage pools.
    /// The projection calculator and every strength-only read use this (rivals
    /// are fog — their loadouts are never known, so they project as unfitted).
    pub fn from_fleet(comp: &BTreeMap<ShipKind, u32>, damage: &BTreeMap<ShipKind, f64>) -> Self {
        let stacks = comp
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(k, n)| ((*k, Loadout::default()), *n))
            .collect();
        let damage = damage
            .iter()
            .filter(|(_, d)| **d != 0.0)
            .map(|(k, d)| ((*k, Loadout::default()), *d))
            .collect();
        Forces { stacks, damage, platform_tiers: 0, platform_pool: 0.0 }
    }

    /// Fold defense-platform tiers into this side (defense of place).
    pub fn with_platform(mut self, tiers: u32, pool: f64) -> Self {
        self.platform_tiers = tiers;
        self.platform_pool = pool;
        self
    }

    /// The per-KIND composition (summed over loadout stacks) — for strength,
    /// reports, and the calculator's setup. Never leaks loadouts to a fog reader.
    pub fn comp(&self) -> BTreeMap<ShipKind, u32> {
        let mut c = BTreeMap::new();
        for ((k, _), n) in &self.stacks {
            *c.entry(*k).or_insert(0) += *n;
        }
        c
    }

    /// Total weighted STRENGTH (attack + defense presence) — the strategic
    /// heuristic (pirate AI, garrison sizing). Loadout-agnostic (a fitted ship
    /// is still one ship of its kind).
    pub fn strength(&self) -> f64 {
        self.stacks.iter().map(|((k, _), n)| k.combat_weight() * *n as f64).sum::<f64>()
            + self.platform_tiers as f64 * (PLATFORM_TIER_ATTACK + PLATFORM_TIER_HULL / HULL_PER_DEFENSE)
    }
}

/// A "typical warfleet" of the estimated bucket size (§Part 3 stale-intel
/// calculator, now the §tactical T4 Monte Carlo): the bucket MIDPOINT split
/// across the combatant kinds (an average-hull assumption). Used when the
/// target is OUT of sensor coverage — the projection then assumes typical
/// hulls, provably from the bucket midpoint, never the target's true
/// composition (the fog-leak invariant).
pub fn typical_forces(class: crate::ship::CountClass) -> Forces {
    let n = class.midpoint();
    let raiders = n.div_ceil(2);
    let corvettes = n - raiders;
    let mut comp = BTreeMap::new();
    if raiders > 0 {
        comp.insert(ShipKind::Raider, raiders);
    }
    if corvettes > 0 {
        comp.insert(ShipKind::Corvette, corvettes);
    }
    // A projected typical fleet is UNFITTED (a fog observer never knows a rival's
    // loadouts — the leak invariant), so it fights as plain beam brawlers.
    Forces::from_fleet(&comp, &BTreeMap::new())
}

// §tactical supersession: `attrition_tick` / `project_engagement` (the pooled
// Lanchester step + closed-form projection) are DELETED. The authoritative sim
// steps [`crate::tactical::TacticalState`]; the calculator samples the same
// engine via [`crate::tactical::simulate_engagement`] /
// [`crate::tactical::project_distribution`] (Monte Carlo — reality's exact
// function, sampled, on stale inputs).

// --- BATTLE RECORDS (§battle-records Part A) ---------------------------------
//
// Every engagement produces a recorded, watchable timeline. The recorder is a
// pure OBSERVER of the engagement lifecycle: it reads round-by-round state and
// never feeds back into resolution, so `same seed + commands → identical
// records` (the determinism law). Balance patches must never rewrite an old
// record — a `BattleRecord` is history captured at resolution time, replayed
// verbatim.
//
// Because nothing outruns light, the record IS the battle as far as any viewer
// is concerned: A2 unlocks round `i` per viewer exactly when its light arrives.
// This module owns the storage shape; `world.rs` owns the lifecycle hooks and
// `view.rs` the per-round fog gate.

/// How long a resolved record is retained before pruning (7 days of sim time),
/// UNLESS it is inside a participant corp's most-recent set. Tunable.
pub const RECORD_RETENTION_SECS: f64 = 7.0 * 24.0 * 60.0 * 60.0;
/// Per participating corp, this many most-recent records survive pruning
/// regardless of age (so a quiet corp keeps its last battles). Tunable.
pub const RECORD_PER_CORP_FLOOR: usize = 25;
/// Absolute cap on stored records; the oldest RESOLVED ones evict past it (a
/// runaway-battle backstop). Tunable.
pub const MAX_BATTLE_RECORDS: usize = 2000;
/// The target number of ROUNDS a full-length battle records — the timeline is
/// down-sampled to about this many flushes (plus one per event beat), so a
/// 45-minute battle and a 45-second one both read as a legible ~40-step replay.
/// Tunable.
pub const RECORD_TARGET_ROUNDS: u64 = 40;

/// One side of a recorded battle at the moment it OPENED. Part B adds the
/// per-loadout initial breakdown; for now the composition is per-kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideRecord {
    pub corp: PlayerId,
    /// Opening composition (per-kind survivors are tracked round-by-round).
    pub initial: BTreeMap<ShipKind, u32>,
    /// §modules B5: the LOADOUT partition this side started with (fitted stacks
    /// only; the unfitted remainder = `initial` − Σ these). The record's
    /// per-loadout intel — participant fidelity surfaces it, and the client
    /// labels the side and types its salvos by dominant weapon family. serde
    /// default = empty (all-unfitted / pre-feature records), zero migration.
    #[serde(default)]
    pub initial_loadouts: LoadoutMap,
    /// The corp's engagement doctrine at the open. OWNER-ONLY in the view (A2):
    /// a rival never learns your posture from watching the fight.
    pub posture: EngagementPolicy,
    /// Defense-platform tiers folded into this side at the open (0 = none).
    pub platform_tiers: u32,
}

/// A discrete beat worth flushing a round on and annotating in the replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundNote {
    /// Reinforcements joined `side` (0 = attackers, 1 = defenders) with `comp`.
    Joined { side: u8, comp: BTreeMap<ShipKind, u32> },
    /// `side` fell below its doctrine retreat threshold and is withdrawing.
    RetreatTripped { side: u8 },
    /// A player Withdraw order reached a fleet on `side` mid-battle.
    WithdrawOrdered { side: u8 },
    /// `side` began its parting-shot exposure (Avoid doctrine — not accepting).
    DisengageExposure { side: u8 },
    /// The defender's Defense Platform lost its last tier this round.
    PlatformDestroyed,
    /// The safety valve tripped — a no-retreat grind ends in mutual disengage.
    MutualDisengage,
    // Part B adds `SalvoDetail { side, family, kills }` (participant fidelity).
}

/// One recorded ROUND: the survivors after it, the damage each side dealt, the
/// ships each side LOST, and any beats. Indexing is by SIDE throughout: index 0
/// = attackers, 1 = defenders. `counts[s]`/`kills[s]` are ABOUT side `s` (its
/// survivors / its losses); `dealt[s]` is the damage OUTPUT by side `s`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoundRecord {
    pub tick: u64,
    pub counts: [BTreeMap<ShipKind, u32>; 2],
    pub dealt: [f64; 2],
    pub kills: [BTreeMap<ShipKind, u32>; 2],
    pub notes: Vec<RoundNote>,
    /// §tactical T3: the TRUTH KEYFRAME for this round — real combatant
    /// positions, live torpedo salvos, exact deaths. serde-default: old
    /// records have none (the client falls back to choreographed rendering).
    /// PARTICIPANT fidelity only on the wire.
    #[serde(default)]
    pub frame: Option<Keyframe>,
}

// --- §tactical T3: TRUTH KEYFRAMES --------------------------------------------------

/// Representative-combatant cap per keyframe (all capitals ALWAYS ride along;
/// the rest fill to this cap in stable cid order). Tunable.
pub const KEYFRAME_SHIP_CAP: usize = 60;
/// Exact death events kept per recorded round. Tunable.
pub const KEYFRAME_DEATH_CAP: usize = 40;

/// A recorded round's slice of battle TRUTH: where (a sample of) the ships
/// actually were, what torpedo salvos were in flight, and exactly where ships
/// died. The theater is an interpolating REPLAYER of these — reality provides
/// the choreography now.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Keyframe {
    pub ships: Vec<KfShip>,
    pub torpedoes: Vec<KfSalvo>,
    pub deaths: Vec<KfDeath>,
}

/// One sampled combatant (positions in battle-local arena coords; `hp` is the
/// remaining fraction so the client can dim the wounded).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KfShip {
    pub side: u8,
    pub kind: ShipKind,
    pub x: f32,
    pub y: f32,
    pub hp: f32,
    /// A Defense Platform tier (drawn as an emplacement, not a hull).
    #[serde(default)]
    pub plat: bool,
}

/// A live torpedo SALVO summary: `n` fish around a centroid, per firing side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KfSalvo {
    pub side: u8,
    pub x: f32,
    pub y: f32,
    pub n: u32,
}

/// An exact death event: which side lost what kind, where, at which tactical
/// step (capped per round — see [`KEYFRAME_DEATH_CAP`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KfDeath {
    pub step: u64,
    pub side: u8,
    pub kind: ShipKind,
    pub x: f32,
    pub y: f32,
}

/// The resolved outcome: who won (as a [`RaidOutcome`]) + each side's total
/// losses. The wreck marker is the existing `RaidResolved` event; the record id
/// ties the replay to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleOutcomeSummary {
    pub outcome: RaidOutcome,
    /// Total ships lost per side (attacker, defender) over the whole battle.
    pub total_losses: [BTreeMap<ShipKind, u32>; 2],
}

/// Recorder bookkeeping accumulated BETWEEN round flushes (not part of the
/// observable timeline, but persisted so a mid-battle snapshot resumes exactly).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct PendingRound {
    /// Damage dealt by [attackers, defenders] since the last flush.
    dealt: [f64; 2],
    /// Ships lost by [attackers, defenders] since the last flush.
    kills: [BTreeMap<ShipKind, u32>; 2],
    /// Beats since the last flush (each forces a flush next round tick).
    notes: Vec<RoundNote>,
    /// Sim tick of the last flush (round cadence is measured from here).
    last_flush_tick: u64,
    /// §tactical T3: the LATEST truth keyframe since the last flush (the
    /// flushed round carries it) + deaths accumulated across the window.
    frame: Option<Keyframe>,
    deaths: Vec<KfDeath>,
    /// Flush cadence in ticks (computed at open from the expected duration).
    round_every: u64,
}

/// One recorded battle: its sides' opening state, a per-round timeline captured
/// at resolution time, and (once resolved) an outcome summary. Keyed by the
/// engagement's own id, so the record, the map icon, and the news event share
/// one identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleRecord {
    pub id: EntityId,
    pub pos: Vec2,
    /// The defended system, if the battle stood at one (else open space).
    pub system: Option<EntityId>,
    pub started_tick: u64,
    /// `None` while the battle is still running.
    pub ended_tick: Option<u64>,
    /// A cargo raid (short, few rounds) vs a decisive battle.
    pub raid: bool,
    pub sides: [SideRecord; 2],
    pub rounds: Vec<RoundRecord>,
    pub outcome: Option<BattleOutcomeSummary>,
    /// Recorder accumulator between flushes (persisted; opaque to the view).
    #[serde(default)]
    pending: PendingRound,
}

/// Fold a side's `Losses` (scouts already merged in by the caller) into a
/// running per-kind loss map. Platform tiers are tracked via a note, not here.
fn add_losses_into(map: &mut BTreeMap<ShipKind, u32>, l: &Losses) {
    for (k, n) in &l.per_kind {
        if *n > 0 {
            *map.entry(*k).or_insert(0) += *n;
        }
    }
}

impl BattleRecord {
    /// The round-flush cadence (ticks) for a battle of this timescale: the
    /// expected duration down-sampled to ≈[`RECORD_TARGET_ROUNDS`] flushes, floor
    /// 1. Raids expect only a short slice, so they record a handful of rounds.
    pub fn round_every_for(battle_target_secs: f64, raid: bool) -> u64 {
        let frac = if raid { RAID_CAP_FRAC } else { 1.0 };
        let expected_ticks = (frac * battle_target_secs.max(1.0) * TICK_HZ as f64).max(1.0);
        ((expected_ticks / RECORD_TARGET_ROUNDS as f64).floor() as u64).max(1)
    }

    /// Open a record for a battle whose sides and geometry are known.
    pub fn open(
        id: EntityId,
        pos: Vec2,
        system: Option<EntityId>,
        raid: bool,
        started_tick: u64,
        battle_target_secs: f64,
        sides: [SideRecord; 2],
    ) -> Self {
        BattleRecord {
            id,
            pos,
            system,
            started_tick,
            ended_tick: None,
            raid,
            sides,
            rounds: Vec::new(),
            outcome: None,
            pending: PendingRound {
                round_every: Self::round_every_for(battle_target_secs, raid),
                last_flush_tick: started_tick,
                ..Default::default()
            },
        }
    }

    /// Accumulate one attrition tick: damage dealt by each side and the ships
    /// each side lost. `la`/`lb` are the attacker/defender losses this tick.
    pub fn accumulate(&mut self, dealt_by_a: f64, dealt_by_b: f64, la: &Losses, lb: &Losses) {
        self.pending.dealt[0] += dealt_by_a;
        self.pending.dealt[1] += dealt_by_b;
        add_losses_into(&mut self.pending.kills[0], la);
        add_losses_into(&mut self.pending.kills[1], lb);
    }

    /// Note a beat (forces a round flush on the next `flush_if_due`).
    /// §tactical T3: feed the latest truth keyframe (deaths ACCUMULATE across
    /// the round window, capped; the positional snapshot is last-wins).
    pub fn keyframe(&mut self, mut frame: Keyframe) {
        self.pending.deaths.append(&mut frame.deaths);
        self.pending.deaths.truncate(KEYFRAME_DEATH_CAP);
        self.pending.frame = Some(frame);
    }

    pub fn note(&mut self, note: RoundNote) {
        self.pending.notes.push(note);
    }

    /// Flush a round if the cadence elapsed OR a beat is pending, snapshotting
    /// the survivors `counts`. No-op otherwise.
    pub fn flush_if_due(&mut self, tick: u64, counts: [BTreeMap<ShipKind, u32>; 2]) {
        let due = tick.saturating_sub(self.pending.last_flush_tick) >= self.pending.round_every;
        if due || !self.pending.notes.is_empty() {
            self.flush_round(tick, counts);
        }
    }

    fn flush_round(&mut self, tick: u64, counts: [BTreeMap<ShipKind, u32>; 2]) {
        let dealt = self.pending.dealt;
        let kills = std::mem::take(&mut self.pending.kills);
        let notes = std::mem::take(&mut self.pending.notes);
        let frame = self.pending.frame.take().map(|mut f| {
            f.deaths = std::mem::take(&mut self.pending.deaths);
            f
        });
        self.pending.deaths.clear();
        self.pending.dealt = [0.0, 0.0];
        self.pending.last_flush_tick = tick;
        self.rounds.push(RoundRecord { tick, counts, dealt, kills, notes, frame });
    }

    fn pending_has_content(&self) -> bool {
        !self.pending.notes.is_empty()
            || self.pending.dealt != [0.0, 0.0]
            || self.pending.kills.iter().any(|m| m.values().any(|n| *n > 0))
    }

    /// Finalize: flush any tail round, then stamp the ending tick + outcome. A
    /// resolved record is frozen — never mutated again.
    pub fn finalize(
        &mut self,
        tick: u64,
        outcome: RaidOutcome,
        total_losses: [BTreeMap<ShipKind, u32>; 2],
        final_counts: [BTreeMap<ShipKind, u32>; 2],
    ) {
        if self.pending_has_content() {
            self.flush_round(tick, final_counts);
        }
        self.ended_tick = Some(tick);
        self.outcome = Some(BattleOutcomeSummary { outcome, total_losses });
    }

    /// Sim seconds since this record ended (`0` while still running).
    pub fn ended_secs_ago(&self, now: f64) -> f64 {
        match self.ended_tick {
            None => 0.0,
            Some(t) => (now - t as f64 * DT).max(0.0),
        }
    }
}

/// Prune resolved records: drop those older than [`RECORD_RETENTION_SECS`] that
/// are NOT in any participant corp's most-recent-[`RECORD_PER_CORP_FLOOR`] set,
/// then hard-cap the total at [`MAX_BATTLE_RECORDS`] by evicting the oldest.
/// Running battles (no `ended_tick`) are always kept. Deterministic (ties break
/// on id). Runs off the hot path — on record open, not per tick.
pub fn prune_records(records: &mut BTreeMap<EntityId, BattleRecord>, now: f64) {
    // Each corp's most-recent ids (by ended tick; running = newest) are protected.
    let mut by_corp: BTreeMap<PlayerId, Vec<(u64, EntityId)>> = BTreeMap::new();
    for (id, r) in records.iter() {
        let recency = r.ended_tick.unwrap_or(u64::MAX);
        for s in &r.sides {
            by_corp.entry(s.corp).or_default().push((recency, *id));
        }
    }
    let mut protected: std::collections::BTreeSet<EntityId> = Default::default();
    for (_corp, mut v) in by_corp {
        v.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1))); // newest first, id tiebreak
        for (_, id) in v.into_iter().take(RECORD_PER_CORP_FLOOR) {
            protected.insert(id);
        }
    }
    records.retain(|id, r| {
        if protected.contains(id) || r.ended_tick.is_none() {
            return true;
        }
        r.ended_secs_ago(now) < RECORD_RETENTION_SECS
    });
    // Hard cap: evict the oldest RESOLVED records past the ceiling (running
    // battles sort last via u64::MAX, so they are never evicted).
    if records.len() > MAX_BATTLE_RECORDS {
        let mut order: Vec<(u64, EntityId)> = records
            .iter()
            .map(|(id, r)| (r.ended_tick.unwrap_or(u64::MAX), *id))
            .collect();
        order.sort(); // oldest first
        let excess = records.len() - MAX_BATTLE_RECORDS;
        for (_, id) in order.into_iter().take(excess) {
            records.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- §battle-records: recorder unit behaviour --------------------------

    fn comp(pairs: &[(ShipKind, u32)]) -> BTreeMap<ShipKind, u32> {
        pairs.iter().copied().collect()
    }

    fn losses(pairs: &[(ShipKind, u32)]) -> Losses {
        let mut l = Losses::default();
        for (k, n) in pairs {
            l.add_kind(*k, *n);
        }
        l
    }

    /// A finished record for pruning tests: one corp per side, ended at `tick`.
    fn finished_record(id: EntityId, corp: PlayerId, tick: u64) -> BattleRecord {
        let sides = [
            SideRecord { corp, initial: BTreeMap::new(), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(9_999), initial: BTreeMap::new(), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
        ];
        let mut r = BattleRecord::open(id, Vec2::ZERO, None, false, tick, 45.0, sides);
        r.finalize(tick, RaidOutcome::BothSurvive, [BTreeMap::new(), BTreeMap::new()], [BTreeMap::new(), BTreeMap::new()]);
        r
    }

    #[test]
    fn round_cadence_targets_forty_flushes_under_both_presets() {
        // A full-length battle records ≈ RECORD_TARGET_ROUNDS flushes under BOTH
        // the playtest and production battle timescales — the timeline stays a
        // legible ~40-step replay whether the fight lasts 45 s or 45 min.
        for target in [45.0, 2700.0] {
            let re = BattleRecord::round_every_for(target, false);
            let full_ticks = target * TICK_HZ as f64;
            let flushes = full_ticks / re as f64;
            assert!(
                (flushes - RECORD_TARGET_ROUNDS as f64).abs() <= 2.0,
                "target {target}s → {flushes:.1} flushes (want ≈ {})",
                RECORD_TARGET_ROUNDS
            );
        }
        // A raid records only a handful of rounds (short cap slice).
        assert!(BattleRecord::round_every_for(45.0, true) >= 1);
    }

    #[test]
    fn accumulate_flushes_on_cadence_and_records_dealt_and_kills() {
        let sides = [
            SideRecord { corp: PlayerId(1), initial: comp(&[(ShipKind::Raider, 5)]), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: comp(&[(ShipKind::Raider, 5)]), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
        ];
        // round_every for a 20 s battle = floor(20*30/40) = 15 ticks.
        let mut r = BattleRecord::open(EntityId(1), Vec2::ZERO, None, false, 0, 20.0, sides);
        // Fourteen quiet ticks: no flush yet (under the cadence, no beat).
        for t in 1..=14 {
            r.accumulate(1.0, 0.5, &Losses::default(), &losses(&[(ShipKind::Raider, 0)]));
            r.flush_if_due(t, [comp(&[(ShipKind::Raider, 5)]), comp(&[(ShipKind::Raider, 5)])]);
        }
        assert!(r.rounds.is_empty(), "no flush before the cadence elapses");
        // Tick 15 hits the cadence and one enemy raider died: a round flushes.
        r.accumulate(1.0, 0.5, &Losses::default(), &losses(&[(ShipKind::Raider, 1)]));
        r.flush_if_due(15, [comp(&[(ShipKind::Raider, 5)]), comp(&[(ShipKind::Raider, 4)])]);
        assert_eq!(r.rounds.len(), 1, "the cadence tick flushes exactly one round");
        let round = &r.rounds[0];
        assert_eq!(round.tick, 15);
        assert!((round.dealt[0] - 15.0).abs() < 1e-9, "accumulated attacker damage");
        assert!((round.dealt[1] - 7.5).abs() < 1e-9, "accumulated defender damage");
        assert_eq!(round.kills[1].get(&ShipKind::Raider).copied(), Some(1), "defender's loss recorded");
        assert_eq!(round.counts[1].get(&ShipKind::Raider).copied(), Some(4), "survivors snapshotted");
    }

    #[test]
    fn a_beat_forces_a_flush_off_cadence() {
        let sides = [
            SideRecord { corp: PlayerId(1), initial: BTreeMap::new(), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: BTreeMap::new(), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
        ];
        let mut r = BattleRecord::open(EntityId(1), Vec2::ZERO, None, false, 0, 2700.0, sides);
        // A single early tick with a beat — nowhere near the (huge) cadence.
        r.note(RoundNote::Joined { side: 1, comp: comp(&[(ShipKind::Corvette, 3)]) });
        r.accumulate(2.0, 0.0, &Losses::default(), &Losses::default());
        r.flush_if_due(3, [BTreeMap::new(), comp(&[(ShipKind::Corvette, 3)])]);
        assert_eq!(r.rounds.len(), 1, "the join beat forced a flush off-cadence");
        assert!(matches!(r.rounds[0].notes[0], RoundNote::Joined { side: 1, .. }));
    }

    #[test]
    fn finalize_flushes_the_tail_and_stamps_the_outcome() {
        let sides = [
            SideRecord { corp: PlayerId(1), initial: comp(&[(ShipKind::Raider, 3)]), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: comp(&[(ShipKind::Raider, 2)]), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
        ];
        let mut r = BattleRecord::open(EntityId(1), Vec2::ZERO, None, false, 0, 2700.0, sides);
        r.accumulate(5.0, 1.0, &Losses::default(), &losses(&[(ShipKind::Raider, 2)]));
        assert!(r.rounds.is_empty(), "no cadence flush yet");
        r.finalize(
            7,
            RaidOutcome::TargetDestroyed,
            [comp(&[]), comp(&[(ShipKind::Raider, 2)])],
            [comp(&[(ShipKind::Raider, 3)]), comp(&[])],
        );
        assert_eq!(r.rounds.len(), 1, "the tail round flushed at finalize");
        assert_eq!(r.ended_tick, Some(7));
        let o = r.outcome.as_ref().expect("outcome stamped");
        assert_eq!(o.outcome, RaidOutcome::TargetDestroyed);
        assert_eq!(o.total_losses[1].get(&ShipKind::Raider).copied(), Some(2));
    }

    #[test]
    fn pruning_keeps_the_per_corp_floor_past_retention() {
        // 30 records for one corp, all ended LONG past retention. The per-corp
        // floor keeps the 25 most recent regardless of age; the 5 oldest prune.
        let corp = PlayerId(7);
        let now = 10_000_000.0; // well beyond 7 days of sim seconds
        let mut recs: BTreeMap<EntityId, BattleRecord> = BTreeMap::new();
        for i in 0..30u64 {
            let id = EntityId(i + 1);
            recs.insert(id, finished_record(id, corp, i * 10)); // all old vs `now`
        }
        prune_records(&mut recs, now);
        assert_eq!(recs.len(), RECORD_PER_CORP_FLOOR, "old records prune to the per-corp floor");
        // The survivors are the newest 25 (ended ticks 50..290), oldest 5 dropped.
        assert!(!recs.contains_key(&EntityId(1)), "the oldest record pruned");
        assert!(recs.contains_key(&EntityId(30)), "the newest record kept");
    }

    #[test]
    fn pruning_hard_caps_the_total_evicting_oldest() {
        // One corp, MAX+100 RECENT records: the floor protects 25, retention keeps
        // the rest (recent), so the hard cap trims to MAX by evicting the oldest.
        let corp = PlayerId(3);
        let mut recs: BTreeMap<EntityId, BattleRecord> = BTreeMap::new();
        let extra = 100u64;
        for i in 0..(MAX_BATTLE_RECORDS as u64 + extra) {
            let id = EntityId(i + 1);
            recs.insert(id, finished_record(id, corp, i)); // ended tick = i (recent vs now=0)
        }
        prune_records(&mut recs, 0.0);
        assert_eq!(recs.len(), MAX_BATTLE_RECORDS, "hard cap trims to the ceiling");
        assert!(!recs.contains_key(&EntityId(1)), "the oldest was evicted by the cap");
        assert!(recs.contains_key(&EntityId(MAX_BATTLE_RECORDS as u64 + extra)), "the newest survived");
    }

    #[test]
    fn running_battles_are_never_pruned() {
        let mut recs: BTreeMap<EntityId, BattleRecord> = BTreeMap::new();
        let sides = [
            SideRecord { corp: PlayerId(1), initial: BTreeMap::new(), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: BTreeMap::new(), initial_loadouts: Default::default(), posture: EngagementPolicy::default(), platform_tiers: 0 },
        ];
        // A still-running record (no ended_tick), plus 30 ancient finished ones
        // for a DIFFERENT corp so the floor can't protect the runner incidentally.
        recs.insert(EntityId(1), BattleRecord::open(EntityId(1), Vec2::ZERO, None, false, 0, 45.0, sides));
        for i in 0..30u64 {
            let id = EntityId(1000 + i);
            recs.insert(id, finished_record(id, PlayerId(42), i));
        }
        prune_records(&mut recs, 10_000_000.0);
        assert!(recs.contains_key(&EntityId(1)), "a running battle is always kept");
    }

    #[test]
    fn record_serde_round_trips_including_pending() {
        // A record mid-battle (pending accumulation live) round-trips exactly.
        let sides = [
            SideRecord { corp: PlayerId(1), initial: comp(&[(ShipKind::Raider, 4)]), initial_loadouts: Default::default(), posture: EngagementPolicy::EngageAny, platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: comp(&[(ShipKind::Corvette, 3)]), initial_loadouts: Default::default(), posture: EngagementPolicy::Avoid, platform_tiers: 2 },
        ];
        let mut r = BattleRecord::open(EntityId(0xE000_0000_0000_0001), Vec2::new(1.0, -2.0), Some(EntityId(5)), false, 3, 2700.0, sides);
        r.accumulate(3.25, 0.75, &losses(&[(ShipKind::Raider, 1)]), &Losses::default());
        r.note(RoundNote::PlatformDestroyed);
        r.flush_if_due(4, [comp(&[(ShipKind::Raider, 3)]), comp(&[(ShipKind::Corvette, 3)])]);
        r.accumulate(1.0, 0.0, &Losses::default(), &Losses::default()); // live pending
        let json = serde_json::to_string(&r).unwrap();
        let r2: BattleRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2, "the record round-trips byte-for-byte through JSON");
    }

    // --- §modules/§fitting: engine-agnostic module + affinity facts ---------
    // The counter matrix ITSELF (armor blunting, PD interception, torpedo
    // behaviour, capital emergence) is proven statistically ON the tactical
    // engine in `tactical.rs` — the pooled `absorb()` unit tests went with the
    // pooled engine. What stays here are the pure module/affinity tables the
    // engine consumes.

    #[test]
    fn loadout_offense_types_and_stacks_duplicates() {
        use crate::module::{DamageType, ModuleKind, TORP_MULT};
        // §fitting: 2× MassDriver fires at 2× DRIVER_MULT — duplicates stack
        // linearly; the fitting budget (2+2=4 ≤ Raider's 4) is the brake.
        let one = Loadout::new(vec![ModuleKind::MassDriver]);
        let two = Loadout::new(vec![ModuleKind::MassDriver, ModuleKind::MassDriver]);
        assert!(two.validate(ShipKind::Raider), "double-driver fits the Raider budget exactly");
        assert_eq!(one.offense().0, DamageType::Driver);
        assert!((two.offense().1 - 2.0 * one.offense().1).abs() < 1e-9, "the second copy doubles the output");
        // A double TORPEDO rack is budget-illegal on every subcapital (6 > 5)…
        let tt = Loadout::new(vec![ModuleKind::TorpedoRack, ModuleKind::TorpedoRack]);
        assert!(!tt.validate(ShipKind::Raider) && !tt.validate(ShipKind::Corvette));
        // …and a single rack types its offense as torpedo at TORP_MULT.
        let t = Loadout::new(vec![ModuleKind::TorpedoRack]);
        assert_eq!(t.offense(), (DamageType::Torpedo, TORP_MULT));
    }

    #[test]
    fn capital_fitting_combinations_live_on_the_big_budgets() {
        use crate::module::ModuleKind;
        // §ladder B6.6: Titan Torp+PD+both armors+2×Driver = 6 slots, 14 pts ≤ 45
        // — capitals are where combinations live. (The handoff's example set.)
        let big = Loadout::new(vec![
            ModuleKind::TorpedoRack,
            ModuleKind::PointDefenseScreen,
            ModuleKind::ReflectivePlating,
            ModuleKind::WhippleArmor,
            ModuleKind::MassDriver,
            ModuleKind::MassDriver,
        ]);
        assert_eq!(big.len(), 6);
        assert!(big.validate(ShipKind::Titan), "a full 6-module Titan fit is legal");
        assert!(!big.validate(ShipKind::Dreadnought), "5 slots — one fewer combination");
        // A 1-count Titan buckets sanely (no new CountClass needed).
        assert_eq!(crate::ship::CountClass::from_count(1), crate::ship::CountClass::One);
    }

    #[test]
    fn hull_affinity_table_scales_its_family_on_its_kind_only() {
        use crate::module::Family;
        use crate::ship::hull_affinity;
        // The affinity table: Raider→Torpedo and Corvette→Interception only;
        // everything else is 1.0 (incl. Beam everywhere on the original hulls
        // and Protection on every Stage-A hull).
        assert_eq!(hull_affinity(ShipKind::Raider, Family::Torpedo), 1.25);
        assert_eq!(hull_affinity(ShipKind::Corvette, Family::Interception), 1.25);
        assert_eq!(hull_affinity(ShipKind::Corvette, Family::Torpedo), 1.0, "right family, wrong kind");
        assert_eq!(hull_affinity(ShipKind::Raider, Family::Driver), 1.0, "right kind, wrong family");
        assert_eq!(hull_affinity(ShipKind::Raider, Family::Interception), 1.0);
        for k in [ShipKind::Convoy, ShipKind::Raider, ShipKind::Corvette, ShipKind::Colony, ShipKind::Scout] {
            assert_eq!(hull_affinity(k, Family::Beam), 1.0, "beam affinity 1.0 on the original hulls");
            assert_eq!(hull_affinity(k, Family::Protection), 1.0);
        }
        // §ladder: each capital's ONE affinity (the Titan's broad weapon spread).
        assert_eq!(hull_affinity(ShipKind::Destroyer, Family::Beam), 1.20);
        assert_eq!(hull_affinity(ShipKind::Cruiser, Family::Protection), 1.20);
        assert_eq!(hull_affinity(ShipKind::Battleship, Family::Driver), 1.20);
        assert_eq!(hull_affinity(ShipKind::Dreadnought, Family::Interception), 1.30);
        for f in [Family::Beam, Family::Driver, Family::Torpedo] {
            assert_eq!(hull_affinity(ShipKind::Titan, f), 1.10, "broadly good");
        }
        assert_eq!(hull_affinity(ShipKind::Titan, Family::Interception), 1.0, "best at nothing");
        assert_eq!(hull_affinity(ShipKind::Destroyer, Family::Torpedo), 1.0, "one affinity per capital");
    }
}

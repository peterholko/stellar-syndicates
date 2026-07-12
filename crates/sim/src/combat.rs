//! Combat: deterministic **Lanchester** attrition (§FLEETS Part 2, GDD §26.2).
//!
//! Battles are no longer a seeded whole-ship coin-flip. Two pooled sides deal
//! damage each tick in proportion to their weighted ATTACK power; the damage is
//! spread across the enemy's kinds by `count × hull` share and accumulates in
//! per-kind DAMAGE POOLS; when a kind's pool fills a hull, one ship of that kind
//! dies and the pool carries the remainder forward. You lose *counts*, not
//! coin-flips — partial victories and defeats fall out naturally.
//!
//! This is **the one source of truth** for combat: the authoritative sim
//! ([`crate::world`]) runs `attrition_tick` on real fleets, and the stale-intel
//! battle calculator (§Part 3) runs the SAME function forward on the observer's
//! view data — no reimplementation, no drift. Everything here is pure and
//! seed-free (deterministic): the outcome is a function of the inputs alone.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::{DT, TICK_HZ};
use crate::doctrine::EngagementPolicy;
use crate::event::RaidOutcome;
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::ship::ShipKind;

// --- TUNABLE COMBAT BLOCK (the Lanchester knobs) --------------------------
/// The per-tick damage fraction is DERIVED from `Config.battle_target_secs`, not
/// hardcoded — battle DURATION is a config-scaled strategic timescale (playtest
/// ≈45 s, production ≈45 min). See [`dmg_rate`].
///
/// CALIBRATION: for equal aimed-fire forces the strength decays ≈ exponentially,
/// so the time to a given loss fraction is `≈ C / rate` (independent of force
/// SIZE — both counts cancel). Measured empirically against the reference (equal
/// RAIDER forces grinding to the 50 % retreat threshold): `C ≈ 0.1435`
/// (rock-stable across rates — `duration × rate` is constant). Hence:
///
/// ```text
/// dmg_rate(target) = DMG_RATE_CALIBRATION / target
/// ```
///
/// so equal reference forces reach their retreat thresholds in
/// `≈ battle_target_secs` (the REQUIRED test proves it under both presets).
/// Lopsided fights end faster — Lanchester compounds the edge (concentration
/// test). A safety valve ([`MAX_BATTLE_MULT`]) forces mutual disengage if neither
/// threshold trips.
pub const DMG_RATE_CALIBRATION: f64 = 0.1435;

/// The per-tick damage fraction for a battle whose target duration is
/// `battle_target_secs`. `max(1.0)` guards a degenerate config.
pub fn dmg_rate(battle_target_secs: f64) -> f64 {
    DMG_RATE_CALIBRATION / battle_target_secs.max(1.0)
}

/// The per-tick damage rate for a cargo RAID — a FIXED quick rate, NOT the
/// config-scaled battle rate: slow battles must not slow raids (a raid is a
/// smash-and-grab, so a raider overpowers a defenceless convoy in ~1 s and seizes
/// its cargo whatever the battle timescale). Only DEFENDED targets (escort /
/// platform) turn a raid into a full-rate BATTLE. Tunable.
pub const RAID_RATE: f64 = 0.1;
/// Raids are survivable SKIRMISHES: a mild tunable expressing how much gentler a
/// raid is than a full battle (kept for the pure-function skirmish demonstration;
/// the authoritative raid rate is the fixed [`RAID_RATE`], and the low mutual
/// casualties come mainly from raid BREVITY — the short cap + early disengage).
pub const RAID_SKIRMISH_MULT: f64 = 0.3;
/// A raid engagement ends after at most this fraction of `battle_target_secs`
/// (whichever comes first with cargo-seized / retreat) — raids stay quick
/// smash-and-grabs even as battles get slow. Tunable.
pub const RAID_CAP_FRAC: f64 = 0.15;
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
/// The reference RETREAT fraction the duration calibration is anchored to (equal
/// forces withdraw when half their weighted strength is gone).
pub const REFERENCE_RETREAT_FRAC: f64 = 0.5;
/// Hull per point of defense weight (see [`ShipKind::hull`]).
pub const HULL_PER_DEFENSE: f64 = 10.0;
/// Minimum hull, so a zero-defense scout is still attritable (dies fast).
pub const HULL_MIN: f64 = 2.0;
/// A defense-platform tier as a combatant: its hull and its return fire. Keeps
/// the ram-attrition behaviour — a raider grinds through platform tiers, and the
/// platform shoots back.
pub const PLATFORM_TIER_HULL: f64 = 30.0;
pub const PLATFORM_TIER_ATTACK: f64 = 3.0;

/// Whole units of each kind lost in an attrition step (plus platform tiers).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Losses {
    pub per_kind: BTreeMap<ShipKind, u32>,
    pub platform_tiers: u32,
}

impl Losses {
    pub fn is_empty(&self) -> bool {
        self.platform_tiers == 0 && self.per_kind.values().all(|n| *n == 0)
    }
    pub fn total_ships(&self) -> u32 {
        self.per_kind.values().copied().sum()
    }
    fn add_kind(&mut self, kind: ShipKind, n: u32) {
        if n > 0 {
            *self.per_kind.entry(kind).or_insert(0) += n;
        }
    }
}

/// One side of an engagement as a pooled force: a per-kind ship count + its
/// damage pool, plus optional defense-platform tiers with their own pool. Both
/// the authoritative fleets and the projection calculator use this shape.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Forces {
    pub comp: BTreeMap<ShipKind, u32>,
    pub damage: BTreeMap<ShipKind, f64>,
    pub platform_tiers: u32,
    pub platform_pool: f64,
}

impl Forces {
    /// A side built from a fleet's composition + carried damage pools.
    pub fn from_fleet(comp: &BTreeMap<ShipKind, u32>, damage: &BTreeMap<ShipKind, f64>) -> Self {
        Forces {
            comp: comp.clone(),
            damage: damage.clone(),
            platform_tiers: 0,
            platform_pool: 0.0,
        }
    }

    /// Fold defense-platform tiers into this side (defense of place).
    pub fn with_platform(mut self, tiers: u32, pool: f64) -> Self {
        self.platform_tiers = tiers;
        self.platform_pool = pool;
        self
    }

    pub fn ship_count(&self) -> u32 {
        self.comp.values().copied().sum()
    }

    /// Remove all SCOUTS from this side, returning how many were lost. A scout
    /// "dies if engaged" (§scout — speed and darkness were its armor, not hull):
    /// the moment it is in a battle it is destroyed, whether attacking or caught.
    /// Applied once at engagement time before attrition.
    pub fn strip_scouts(&mut self) -> u32 {
        let n = self.comp.remove(&ShipKind::Scout).unwrap_or(0);
        self.damage.remove(&ShipKind::Scout);
        n
    }

    pub fn alive(&self) -> bool {
        self.ship_count() > 0 || self.platform_tiers > 0
    }

    /// Total weighted ATTACK power = Σ attack_weight×count + platform return fire.
    pub fn attack_power(&self) -> f64 {
        self.comp.iter().map(|(k, n)| k.attack_weight() * *n as f64).sum::<f64>()
            + self.platform_tiers as f64 * PLATFORM_TIER_ATTACK
    }

    /// Total weighted STRENGTH (attack + defense presence) — the retreat metric.
    pub fn strength(&self) -> f64 {
        self.comp.iter().map(|(k, n)| k.combat_weight() * *n as f64).sum::<f64>()
            + self.platform_tiers as f64 * (PLATFORM_TIER_ATTACK + PLATFORM_TIER_HULL / HULL_PER_DEFENSE)
    }

    /// Absorb `incoming` total damage, spread across kinds (and platform tiers)
    /// by `count × hull` share; convert filled pools into whole-ship deaths,
    /// carrying the remainder. Deterministic. Returns the units lost.
    pub fn absorb(&mut self, incoming: f64) -> Losses {
        let mut losses = Losses::default();
        if incoming <= 0.0 || !self.alive() {
            return losses;
        }
        // Weight each kind by count × hull; platform tiers as their own weight.
        let mut weights: Vec<(Option<ShipKind>, f64)> = Vec::new();
        let mut total_w = 0.0;
        for (k, n) in &self.comp {
            if *n > 0 {
                let w = *n as f64 * k.hull();
                weights.push((Some(*k), w));
                total_w += w;
            }
        }
        if self.platform_tiers > 0 {
            let w = self.platform_tiers as f64 * PLATFORM_TIER_HULL;
            weights.push((None, w));
            total_w += w;
        }
        if total_w <= 0.0 {
            return losses;
        }
        for (slot, w) in weights {
            let share = incoming * (w / total_w);
            match slot {
                Some(k) => {
                    let pool = self.damage.entry(k).or_insert(0.0);
                    *pool += share;
                    let hull = k.hull();
                    let have = self.comp.get(&k).copied().unwrap_or(0);
                    let mut killed = 0u32;
                    while killed < have && *pool + 1e-9 >= hull {
                        *pool -= hull;
                        killed += 1;
                    }
                    if killed > 0 {
                        let remaining = have - killed;
                        if remaining == 0 {
                            self.comp.remove(&k);
                            self.damage.remove(&k);
                        } else {
                            self.comp.insert(k, remaining);
                        }
                        losses.add_kind(k, killed);
                    }
                }
                None => {
                    self.platform_pool += share;
                    let mut killed = 0u32;
                    while killed < self.platform_tiers && self.platform_pool + 1e-9 >= PLATFORM_TIER_HULL {
                        self.platform_pool -= PLATFORM_TIER_HULL;
                        killed += 1;
                    }
                    if killed > 0 {
                        self.platform_tiers -= killed;
                        if self.platform_tiers == 0 {
                            self.platform_pool = 0.0;
                        }
                        losses.platform_tiers += killed;
                    }
                }
            }
        }
        losses
    }
}

/// A "typical warfleet" of the estimated bucket size (§Part 3 stale-intel
/// calculator): the bucket MIDPOINT split across the combatant kinds (an
/// average-hull assumption). Used when the target is OUT of sensor coverage — the
/// projection then assumes typical hulls, provably from the bucket midpoint,
/// never the target's true composition (the fog-leak invariant).
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
    Forces { comp, ..Default::default() }
}

/// One symmetric Lanchester attrition tick between two pooled sides. `rate` is
/// the already-scaled per-tick fraction (`dmg_rate(3.0)`, times the raid-skirmish
/// multiplier for a cargo raid). Each side deals `rate × attack_power` to the
/// other, spread by hull share; returns `(losses_a, losses_b)`.
///
/// Damage is computed from the pre-tick attack powers of BOTH sides (they fire
/// simultaneously), then applied — so neither side gets a free first strike.
pub fn attrition_tick(a: &mut Forces, b: &mut Forces, rate: f64) -> (Losses, Losses) {
    let dmg_to_b = rate * a.attack_power();
    let dmg_to_a = rate * b.attack_power();
    let lb = b.absorb(dmg_to_b);
    let la = a.absorb(dmg_to_a);
    (la, lb)
}

/// PROJECT an engagement forward to resolution (§Part 3 calculator + tests):
/// run `attrition_tick` until a side is destroyed, a side's strength falls below
/// its `*_retreat` fraction (survivors withdraw), or `max_ticks` elapses.
/// Returns the total per-side losses. Pure — clones are the caller's inputs, so
/// this never touches authoritative state.
pub fn project_engagement(
    a: &Forces,
    b: &Forces,
    rate: f64,
    a_retreat: Option<f64>,
    b_retreat: Option<f64>,
    max_ticks: u32,
) -> (Forces, Forces, Losses, Losses) {
    let mut a = a.clone();
    let mut b = b.clone();
    let (a0, b0) = (a.strength().max(1e-9), b.strength().max(1e-9));
    let mut la = Losses::default();
    let mut lb = Losses::default();
    for _ in 0..max_ticks {
        if !a.alive() || !b.alive() {
            break;
        }
        // Withdraw when a side has lost at least its retreat fraction of strength.
        if a_retreat.is_some_and(|f| 1.0 - a.strength() / a0 >= f) {
            break;
        }
        if b_retreat.is_some_and(|f| 1.0 - b.strength() / b0 >= f) {
            break;
        }
        let (ta, tb) = attrition_tick(&mut a, &mut b, rate);
        merge_losses(&mut la, &ta);
        merge_losses(&mut lb, &tb);
    }
    (a, b, la, lb)
}

fn merge_losses(into: &mut Losses, add: &Losses) {
    for (k, n) in &add.per_kind {
        into.add_kind(*k, *n);
    }
    into.platform_tiers += add.platform_tiers;
}

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
        self.pending.dealt = [0.0, 0.0];
        self.pending.last_flush_tick = tick;
        self.rounds.push(RoundRecord { tick, counts, dealt, kills, notes });
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

    fn forces(comp: &[(ShipKind, u32)]) -> Forces {
        let mut c = BTreeMap::new();
        for (k, n) in comp {
            c.insert(*k, *n);
        }
        Forces { comp: c, ..Default::default() }
    }

    #[test]
    fn proportional_two_sided_losses_both_sides_bleed() {
        // Two even 6-raider fleets clash — both take real, partial losses.
        let a = forces(&[(ShipKind::Raider, 6)]);
        let b = forces(&[(ShipKind::Raider, 6)]);
        let (fa, fb, la, lb) = project_engagement(&a, &b, dmg_rate(3.0), None, None, 100_000);
        // One side wins but the loser inflicted casualties on the way down.
        assert!(la.total_ships() > 0 && lb.total_ships() > 0, "both sides bled");
        assert!(fa.ship_count() == 0 || fb.ship_count() == 0, "a fight to the finish resolves");
    }

    #[test]
    fn concentration_law_one_big_fleet_beats_two_sequential_halves() {
        // A 20-raider fleet fights two 10-raider fleets ONE AFTER THE OTHER.
        let rate = dmg_rate(3.0);
        let mut big = forces(&[(ShipKind::Raider, 20)]);
        // First 10.
        let first = forces(&[(ShipKind::Raider, 10)]);
        let (b1, _e1, _lb1, _lf1) = project_engagement(&big, &first, rate, None, None, 1_000_000);
        big = b1;
        let after_first = big.ship_count();
        // Second 10 (the survivor fights fresh reinforcements).
        let second = forces(&[(ShipKind::Raider, 10)]);
        let (b2, _e2, _lb2, _lf2) = project_engagement(&big, &second, rate, None, None, 1_000_000);
        let survivors = b2.ship_count();
        // The concentrated fleet not only wins both, it keeps a healthy margin —
        // far more than the ~0 a divided 10+10 would (Lanchester's square law).
        assert!(after_first >= 16, "crushing the first 10 costs the 20 very little (had {after_first})");
        assert!(survivors >= 12, "the 20 beats two sequential 10s cheaply (survivors {survivors})");
    }

    #[test]
    fn concentration_beats_division_head_to_head() {
        // 20 vs 10 leaves far more survivors than the strength ratio alone (2:1)
        // would suggest — the square-law concentration advantage, numerically.
        let (big, _small, _lb, _ls) =
            project_engagement(&forces(&[(ShipKind::Raider, 20)]), &forces(&[(ShipKind::Raider, 10)]), dmg_rate(3.0), None, None, 1_000_000);
        // Linear expectation would be 10 survivors; Lanchester keeps ~√(20²−10²)≈17.
        assert!(big.ship_count() >= 15, "square-law survivors exceed the linear 10 (got {})", big.ship_count());
    }

    #[test]
    fn raid_skirmish_costs_less_than_a_full_battle() {
        // The SAME matchup at raid rate destroys far fewer ships over a fixed
        // window than at full battle rate — raids are survivable skirmishes.
        let a = forces(&[(ShipKind::Raider, 4)]);
        let b = forces(&[(ShipKind::Convoy, 4)]);
        // A window long enough to finish the full-rate fight but not the raid.
        let window = 40;
        let (_ra, _rb, _rla, rlb) = project_engagement(&a, &b, dmg_rate(3.0) * RAID_SKIRMISH_MULT, None, None, window);
        let (_ba, _bb, _bla, blb) = project_engagement(&a, &b, dmg_rate(3.0), None, None, window);
        assert!(rlb.total_ships() < blb.total_ships(), "raid rate spares more of the convoy over the same time");
    }

    #[test]
    fn corvettes_soak_damage_first_via_hull_share() {
        // An escorted convoy fleet: the high-hull corvettes absorb the lion's
        // share of incoming fire, so the convoys survive longer than unescorted.
        let escorted = forces(&[(ShipKind::Convoy, 3), (ShipKind::Corvette, 2)]);
        let bare = forces(&[(ShipKind::Convoy, 3)]);
        let attacker = || forces(&[(ShipKind::Raider, 4)]);
        let window = 60;
        let (esc_after, _e1, _l1, _l2) = project_engagement(&attacker(), &escorted, dmg_rate(3.0), None, None, window);
        let (bare_after, _b1, _b2, _b3) = project_engagement(&attacker(), &bare, dmg_rate(3.0), None, None, window);
        // `esc_after`/`bare_after` are the ATTACKER's survivors; compare convoy loss on the defender via a fresh run.
        let (_x, esc_def, _lx, _ly) = project_engagement(&attacker(), &escorted, dmg_rate(3.0), None, None, window);
        let (_p, bare_def, _lp, _lq) = project_engagement(&attacker(), &bare, dmg_rate(3.0), None, None, window);
        let esc_convoys = esc_def.comp.get(&ShipKind::Convoy).copied().unwrap_or(0);
        let bare_convoys = bare_def.comp.get(&ShipKind::Convoy).copied().unwrap_or(0);
        let _ = (esc_after, bare_after);
        assert!(esc_convoys >= bare_convoys, "escorted convoys outlast unescorted ({esc_convoys} vs {bare_convoys})");
    }

    #[test]
    fn platform_tiers_attrit_into_their_own_pool() {
        // A raider grinds a defended, empty system (platform only): tiers fall.
        let attacker = forces(&[(ShipKind::Raider, 3)]);
        let defender = Forces::default().with_platform(2, 0.0);
        let (_a, def, _la, lb) = project_engagement(&attacker, &defender, dmg_rate(3.0), None, None, 1_000_000);
        assert!(lb.platform_tiers >= 1, "the platform loses tiers to sustained attack");
        assert!(def.platform_tiers < 2 || !attacker.alive());
    }

    /// Ticks for equal reference forces (10 raiders each) to grind to the 50 %
    /// retreat threshold at a given target duration → seconds.
    fn reference_duration_secs(target: f64) -> f64 {
        let rate = dmg_rate(target);
        let mut a = forces(&[(ShipKind::Raider, 10)]);
        let mut b = forces(&[(ShipKind::Raider, 10)]);
        let (a0, b0) = (a.strength(), b.strength());
        let mut ticks = 0u64;
        loop {
            if !a.alive() || !b.alive() { break; }
            if 1.0 - a.strength() / a0 >= REFERENCE_RETREAT_FRAC { break; }
            if 1.0 - b.strength() / b0 >= REFERENCE_RETREAT_FRAC { break; }
            attrition_tick(&mut a, &mut b, rate);
            ticks += 1;
            if ticks > 20_000_000 { break; }
        }
        ticks as f64 / 30.0
    }

    #[test]
    fn equal_forces_duration_matches_target_under_both_presets() {
        // REQUIRED: equal reference forces reach their retreat thresholds in
        // ≈ battle_target_secs under BOTH the playtest and production presets.
        for target in [45.0, 2700.0] {
            let d = reference_duration_secs(target);
            let err = (d - target).abs() / target;
            assert!(err < 0.05, "target {target}s → measured {d:.1}s (err {:.1}%)", err * 100.0);
        }
    }

    #[test]
    fn lopsided_battle_ends_faster_than_an_equal_one() {
        // Lanchester compounds the edge: 14 vs 10 resolves well before equal 10v10.
        let target = 45.0;
        let rate = dmg_rate(target);
        let dur = |a0: u32, b0: u32| -> f64 {
            let mut a = forces(&[(ShipKind::Raider, a0)]);
            let mut b = forces(&[(ShipKind::Raider, b0)]);
            let (sa, sb) = (a.strength(), b.strength());
            let mut t = 0u64;
            loop {
                if !a.alive() || !b.alive() { break; }
                if 1.0 - a.strength() / sa >= 0.5 || 1.0 - b.strength() / sb >= 0.5 { break; }
                attrition_tick(&mut a, &mut b, rate);
                t += 1;
                if t > 20_000_000 { break; }
            }
            t as f64 / 30.0
        };
        assert!(dur(14, 10) < dur(10, 10) * 0.8, "the lopsided fight ends markedly faster");
    }

    #[test]
    fn mid_battle_relief_flips_the_outcome() {
        // Without relief: the attacker (8 raiders) beats the defender (5) outright.
        let atk = forces(&[(ShipKind::Raider, 8)]);
        let def = || forces(&[(ShipKind::Raider, 5)]);
        let (a_no, d_no, _, _) = project_engagement(&atk, &def(), dmg_rate(3.0), None, None, 1_000_000);
        assert!(a_no.ship_count() > 0 && d_no.ship_count() == 0, "attacker wins without relief");

        // With relief: fight partway, THEN reinforce the defender mid-battle — the
        // accumulated damage plus fresh hulls flips who wins. The drama Travian
        // can't do (relief that arrives during the fight changes the ratio).
        let mut a = atk.clone();
        let mut d = def();
        for _ in 0..25 {
            attrition_tick(&mut a, &mut d, dmg_rate(3.0));
        }
        *d.comp.entry(ShipKind::Raider).or_insert(0) += 7; // relief merges in
        let (a2, d2, _, _) = project_engagement(&a, &d, dmg_rate(3.0), None, None, 1_000_000);
        assert!(d2.ship_count() > 0 && a2.ship_count() == 0, "mid-battle relief flips the outcome");
    }

    #[test]
    fn retreat_at_fraction_preserves_survivors() {
        // A defender that withdraws at 50% strength lost keeps survivors that a
        // fight-to-the-death would have spent.
        let a = forces(&[(ShipKind::Raider, 6)]);
        let b = forces(&[(ShipKind::Raider, 6)]);
        let (_, b_retreat, _, _) = project_engagement(&a, &b, dmg_rate(3.0), None, Some(0.5), 1_000_000);
        let (_, b_death, _, _) = project_engagement(&a, &b, dmg_rate(3.0), None, None, 1_000_000);
        assert!(b_retreat.ship_count() > b_death.ship_count(), "withdrawing at 50% saves ships");
    }

    #[test]
    fn deterministic_no_seed() {
        let a = forces(&[(ShipKind::Raider, 5), (ShipKind::Corvette, 2)]);
        let b = forces(&[(ShipKind::Raider, 7)]);
        let r1 = project_engagement(&a, &b, dmg_rate(3.0), Some(0.5), None, 10_000);
        let r2 = project_engagement(&a, &b, dmg_rate(3.0), Some(0.5), None, 10_000);
        assert_eq!(r1.2, r2.2, "same inputs → same losses (seed-free)");
        assert_eq!(r1.3, r2.3);
    }

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
            SideRecord { corp, initial: BTreeMap::new(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(9_999), initial: BTreeMap::new(), posture: EngagementPolicy::default(), platform_tiers: 0 },
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
            SideRecord { corp: PlayerId(1), initial: comp(&[(ShipKind::Raider, 5)]), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: comp(&[(ShipKind::Raider, 5)]), posture: EngagementPolicy::default(), platform_tiers: 0 },
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
            SideRecord { corp: PlayerId(1), initial: BTreeMap::new(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: BTreeMap::new(), posture: EngagementPolicy::default(), platform_tiers: 0 },
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
            SideRecord { corp: PlayerId(1), initial: comp(&[(ShipKind::Raider, 3)]), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: comp(&[(ShipKind::Raider, 2)]), posture: EngagementPolicy::default(), platform_tiers: 0 },
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
            SideRecord { corp: PlayerId(1), initial: BTreeMap::new(), posture: EngagementPolicy::default(), platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: BTreeMap::new(), posture: EngagementPolicy::default(), platform_tiers: 0 },
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
            SideRecord { corp: PlayerId(1), initial: comp(&[(ShipKind::Raider, 4)]), posture: EngagementPolicy::EngageAny, platform_tiers: 0 },
            SideRecord { corp: PlayerId(2), initial: comp(&[(ShipKind::Corvette, 3)]), posture: EngagementPolicy::Avoid, platform_tiers: 2 },
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
}

//! §ground — THE GROUND ENGINE: a landing resolved over time.
//!
//! Replaces the single-tick threshold check that shipped with the ground arc
//! (`marines >= defenders` in one branch, capture or nothing). That version had
//! two problems worth naming, because they are what this module exists to fix:
//!
//! 1. **The defender had no agency.** The outcome was sealed the instant a
//!    marine fleet went Idle in range. Nothing the owner did afterwards — not
//!    relief, not breaking the blockade — could change it.
//! 2. **Suppression was a lie by omission.** The design says bombardment is "a
//!    window, not a wound", and closes the moment the guns stop. But
//!    suppression was sampled ONCE, at the landing. Breaking a blockade
//!    mid-invasion did nothing at all.
//!
//! A landing now takes real sim time and re-reads suppression EVERY tick from
//! live system state, so a besieger must hold the orbit for the whole landing
//! and the defender's counter-play is to relieve the system while boots are on
//! the ground. When the guns leave, the garrison comes out of its bunkers and
//! the tide can turn under the attacker.
//!
//! THE LAWS (mirroring `tactical.rs` — same discipline, different theatre)
//! 1. CONTAINMENT. An assault owns no strategic state. It reads suppression and
//!    writes exactly one thing back: who holds the ground at the end. Transports
//!    are consumed at the drop and the fleet is untouched thereafter.
//! 2. SEEDED, ISOLATED RANDOMNESS. Every assault derives its own stream from
//!    `(world_seed, assault_id)`. The assault stream NEVER touches the world's
//!    RNG (test-enforced), so adding or removing a landing shifts no unrelated
//!    draw. Dice live in per-round casualty variance only — bounded spice.
//! 3. NO INPUT CREEP. There are no ground tactics, no unit orders, no
//!    formations. You choose how many marines to land and whether to keep
//!    firing. That is the whole input surface, and it stays that way.

use serde::{Deserialize, Serialize};

use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::rng::Rng;

// --- TUNABLES ----------------------------------------------------------------

/// MARINES per point of standing garrison — the scale on which ground strength
/// is denominated. A tier-3 garrison fully fed and unsuppressed fields 75
/// troops. Tunable: this is the dial on how much an invasion costs relative to
/// the defense it faces.
pub const MARINES_PER_GARRISON_TIER: f64 = 25.0;

/// A landing runs about as long as a fleet action, as a multiple of
/// `battle_target_secs` — so the two theatres share a sense of scale and one
/// config dial moves both. Tunable.
pub const GROUND_ASSAULT_MULT: f64 = 1.0;

/// Per-round casualty variance, ±this fraction. Matches the tactical engine's
/// damage spice exactly: enough that a marginal landing is a genuine gamble,
/// bounded so a decisive one is never robbed. Tunable.
pub const GROUND_VARIANCE: f64 = 0.15;

/// Target number of recorded rounds in a landing's replay. The engine steps
/// every tick; the RECORD is downsampled to this, exactly as battle records are.
pub const GROUND_RECORD_ROUNDS: u64 = 32;

/// Derive an assault's own RNG stream from `(world_seed, assault_id)` — never
/// the world stream (law 2). Odd multiplier, so distinct ids never collide.
pub fn assault_rng(world_seed: u64, assault_id: u64) -> Rng {
    Rng::new(world_seed ^ assault_id.wrapping_mul(0xD1B5_4A32_D192_ED03))
}

/// The BREAK-EVEN landing against `tiers` of fed garrison under `suppression`:
/// the number of marines at which the fight is an even bet.
///
/// The square root is not a fudge — it falls out of resolving the fight over
/// time instead of comparing two numbers. Under Lanchester's square law two
/// forces annihilate when `M² = D²·(1−s)`, so the break-even landing is
/// `D·√(1−s)`. The practical consequence is deliberate: **bombardment discounts
/// a landing, it does not replace it.** Half-suppressing a garrison cuts the
/// requirement by 29%, not 50%, so troops stay the thing that takes ground.
///
/// This is a THRESHOLD OF ODDS, not of outcome. Land exactly this many and it
/// is a coin flip; margin above it buys confidence. The estimator (§8.6's
/// sibling) is what turns that into a number before you commit.
pub fn break_even_marines(tiers: f64, suppression: f64) -> f64 {
    let live = (1.0 - suppression.clamp(0.0, 1.0)).max(0.0);
    tiers.max(0.0) * MARINES_PER_GARRISON_TIER * live.sqrt()
}

// --- THE ENGINE ---------------------------------------------------------------

/// How a landing ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundOutcome {
    /// The marines hold the ground — the system flips.
    Taken,
    /// The landing was destroyed. The colony holds.
    Repulsed,
}

/// A landing in progress. Owns its own dice and nothing else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundAssault {
    pub id: EntityId,
    pub system: EntityId,
    pub pos: Vec2,
    pub attacker: PlayerId,
    pub defender: PlayerId,
    /// The formation that put the men down. Recorded rather than re-derived
    /// from position at resolution time: several of the attacker's fleets are
    /// typically on station (the blockade that ripened the siege, the escort),
    /// and guessing would charge the wrong one for the landing.
    pub lander: EntityId,
    pub started_tick: u64,
    /// Marines still fighting.
    pub marines: f64,
    /// Garrison troops still ALIVE. Distinct from how many are *fighting*:
    /// suppression pins troops in cover, it does not kill them, so the pool
    /// stays whole and the pinned fraction rejoins the moment the guns stop.
    pub defenders: f64,
    /// What landed, and what stood, at the drop — kept for the record.
    pub marines_landed: u32,
    pub defenders_initial: u32,
    pub garrison_tiers: u32,
    /// Lethality, normalized to the opening force so a landing takes about
    /// `GROUND_ASSAULT_MULT × battle_target_secs` whatever its SIZE. An even
    /// fight decays exponentially (`M(t) = M₀·e^{−Lt}`), so the time to
    /// annihilation is `ln(M₀)/L` — a fixed lethality would make a 500-marine
    /// invasion run half again as long as a 50-marine one for no reason a
    /// player could see. Dividing the log out makes duration scale-free, and
    /// lopsided fights still end early, as they should.
    pub lethality: f64,
    rng: Rng,
}

/// What one stepped tick did — the caller turns this into record rounds and
/// events. Casualties are reported so the record never has to re-derive them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GroundStep {
    pub marine_losses: f64,
    pub defender_losses: f64,
    /// Suppression actually in force this tick (the live read).
    pub suppression: f64,
}

impl GroundAssault {
    /// Open a landing. `tiers` is the garrison's FED tier sum; suppression is
    /// read per tick from then on, never frozen here.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        id: EntityId,
        world_seed: u64,
        system: EntityId,
        pos: Vec2,
        attacker: PlayerId,
        defender: PlayerId,
        lander: EntityId,
        marines: u32,
        tiers: u32,
        started_tick: u64,
        battle_target_secs: f64,
    ) -> Self {
        let defenders = tiers as f64 * MARINES_PER_GARRISON_TIER;
        let target = (GROUND_ASSAULT_MULT * battle_target_secs).max(1.0);
        // `e` floors the log at 1, so a two-man landing can't divide by ~zero.
        let scale = (marines as f64).max(defenders).max(std::f64::consts::E);
        GroundAssault {
            id,
            system,
            pos,
            attacker,
            defender,
            lander,
            started_tick,
            marines: marines as f64,
            defenders,
            marines_landed: marines,
            defenders_initial: defenders.round() as u32,
            garrison_tiers: tiers,
            lethality: scale.ln() / target,
            // The id already carries the world seed's uniqueness; mixing both
            // keeps two galaxies with the same assault id genuinely independent.
            rng: assault_rng(world_seed, id.0),
        }
    }

    /// Advance the fight by `dt` seconds under the CURRENT suppression.
    ///
    /// Both sides fire simultaneously — casualties are computed from the
    /// strengths at the top of the tick and applied after, so neither side gets
    /// a free first strike from iteration order.
    ///
    /// `suppression` is the live read. Raising it (guns on station) pins
    /// defenders out of the firing line; letting it fall (the blockade breaks)
    /// puts them straight back into it, mid-fight.
    pub fn step(&mut self, suppression: f64, dt: f64) -> GroundStep {
        let s = suppression.clamp(0.0, 1.0);
        // Pinned troops neither shoot nor are shot at: only the unsuppressed
        // fraction is in the fight this tick.
        let engaged = self.defenders * (1.0 - s);
        // ±GROUND_VARIANCE, drawn per side per tick. Two draws, always, in a
        // fixed order — so the stream advances identically regardless of which
        // side happens to be winning.
        let roll_a = 1.0 + self.rng.next_f64().mul_add(2.0, -1.0) * GROUND_VARIANCE;
        let roll_d = 1.0 + self.rng.next_f64().mul_add(2.0, -1.0) * GROUND_VARIANCE;
        let marine_losses = (engaged * self.lethality * dt * roll_d).max(0.0).min(self.marines);
        let defender_losses = (self.marines * self.lethality * dt * roll_a).max(0.0).min(self.defenders);
        self.marines -= marine_losses;
        self.defenders -= defender_losses;
        GroundStep { marine_losses, defender_losses, suppression: s }
    }

    /// Whether the fight is over, and how. Checked after every step.
    ///
    /// The defender wins ties: a mutual annihilation leaves nobody holding the
    /// ground, and ground not taken is ground held. That also makes the
    /// break-even landing an honest coin flip rather than a free win.
    pub fn resolved(&self) -> Option<GroundOutcome> {
        if self.marines < 1.0 {
            Some(GroundOutcome::Repulsed)
        } else if self.defenders < 1.0 {
            Some(GroundOutcome::Taken)
        } else {
            None
        }
    }

    /// Whole marines still fighting (the wire and the record deal in bodies).
    pub fn marines_u32(&self) -> u32 {
        self.marines.max(0.0).round() as u32
    }

    /// Whole defenders still alive.
    pub fn defenders_u32(&self) -> u32 {
        self.defenders.max(0.0).round() as u32
    }
}

// --- THE RECORD (§ground G2) --------------------------------------------------
// A landing is now a fight that takes time, so it leaves a replayable account of
// itself — the same discipline `BattleRecord` follows, because the two are shown
// through the same fog: the record is filtered per viewer and its rounds arrive
// by light, never faster.
//
// The record is a PURE OBSERVER. Nothing here ever feeds back into the fight.

/// A beat worth annotating in the replay. These are the moments a player wants
/// to point at afterwards and say "that is where it turned" — derived from the
/// round series rather than reported by the engine, so the engine stays ignorant
/// of the fact it is being watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundNote {
    /// The guns stopped — suppression is falling and pinned troops are coming
    /// back into the fight. The defender's counter-play, made visible.
    GunsLifted,
    /// Bombardment resumed and the garrison is being pinned again.
    GunsResumed,
    /// The garrison ran out of Provisions mid-fight and stopped counting
    /// (§5.1 — suspended, not destroyed, but the ground is open while it lasts).
    GarrisonStarved,
    /// The lead changed hands. The single most interesting beat in the replay.
    Tipped,
}

/// One recorded round: where both sides stood, what it cost them, and what the
/// guns were doing at the time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundRound {
    pub tick: u64,
    pub marines: u32,
    pub defenders: u32,
    /// Suppression in force over this round — the live read, which is what makes
    /// the replay show the tide turning when a blockade breaks.
    pub suppression: f64,
    pub marine_losses: u32,
    pub defender_losses: u32,
    pub notes: Vec<GroundNote>,
}

/// Accumulator between flushes. Casualties are summed per tick and emitted once
/// per recorded round, so the record's losses always add up to the real ones.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
struct PendingGroundRound {
    marine_losses: f64,
    defender_losses: f64,
    suppression: f64,
    ticks: u32,
}

/// The replayable account of one landing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundRecord {
    pub id: EntityId,
    pub system: EntityId,
    pub pos: Vec2,
    pub attacker: PlayerId,
    pub defender: PlayerId,
    pub started_tick: u64,
    /// `None` while the landing is still being fought.
    pub ended_tick: Option<u64>,
    pub marines_landed: u32,
    pub defenders_initial: u32,
    pub garrison_tiers: u32,
    /// Suppression at the moment of the drop — what the attacker committed on.
    pub suppression_at_drop: f64,
    pub rounds: Vec<GroundRound>,
    pub outcome: Option<GroundOutcome>,
    round_every: u64,
    #[serde(default)]
    pending: PendingGroundRound,
}

impl GroundRecord {
    /// Open a record for a landing whose sides are known.
    pub fn open(a: &GroundAssault, suppression_at_drop: f64, battle_target_secs: f64) -> Self {
        let expected = (GROUND_ASSAULT_MULT * battle_target_secs.max(1.0) * crate::config::TICK_HZ as f64).max(1.0);
        GroundRecord {
            id: a.id,
            system: a.system,
            pos: a.pos,
            attacker: a.attacker,
            defender: a.defender,
            started_tick: a.started_tick,
            ended_tick: None,
            marines_landed: a.marines_landed,
            defenders_initial: a.defenders_initial,
            garrison_tiers: a.garrison_tiers,
            suppression_at_drop,
            rounds: Vec::new(),
            outcome: None,
            round_every: ((expected / GROUND_RECORD_ROUNDS as f64).floor() as u64).max(1),
            pending: PendingGroundRound::default(),
        }
    }

    /// Feed one stepped tick in. Flushes a round on the record's cadence.
    pub fn accumulate(&mut self, tick: u64, step: &GroundStep, a: &GroundAssault) {
        self.pending.marine_losses += step.marine_losses;
        self.pending.defender_losses += step.defender_losses;
        self.pending.suppression += step.suppression;
        self.pending.ticks += 1;
        if tick.saturating_sub(self.started_tick).is_multiple_of(self.round_every) {
            self.flush(tick, a);
        }
    }

    /// Close the record out, flushing whatever is pending so the last moments of
    /// a landing are never lost to the cadence.
    pub fn close(&mut self, tick: u64, a: &GroundAssault, outcome: GroundOutcome) {
        if self.pending.ticks > 0 || self.rounds.is_empty() {
            self.flush(tick, a);
        }
        self.ended_tick = Some(tick);
        self.outcome = Some(outcome);
    }

    fn flush(&mut self, tick: u64, a: &GroundAssault) {
        let p = std::mem::take(&mut self.pending);
        let suppression = if p.ticks > 0 { p.suppression / p.ticks as f64 } else { self.suppression_at_drop };
        let (marines, defenders) = (a.marines_u32(), a.defenders_u32());
        // The beats, derived from the series rather than reported by the engine.
        let mut notes = Vec::new();
        if let Some(prev) = self.rounds.last() {
            // A tenth of the scale is enough to mean "the guns changed", and
            // small enough that ordinary decay noise doesn't trip it.
            if suppression + 0.10 < prev.suppression {
                notes.push(GroundNote::GunsLifted);
            } else if suppression > prev.suppression + 0.10 {
                notes.push(GroundNote::GunsResumed);
            }
            if suppression >= 1.0 && prev.suppression < 1.0 && a.garrison_tiers > 0 {
                notes.push(GroundNote::GarrisonStarved);
            }
            // Effective strength, not headcount: pinned troops are not in the
            // fight, so the lead a player cares about is the one being fought.
            let eff = |m: u32, d: u32, s: f64| (m as f64, d as f64 * (1.0 - s));
            let (pm, pd) = eff(prev.marines, prev.defenders, prev.suppression);
            let (cm, cd) = eff(marines, defenders, suppression);
            if (pm >= pd) != (cm >= cd) {
                notes.push(GroundNote::Tipped);
            }
        }
        self.rounds.push(GroundRound {
            tick,
            marines,
            defenders,
            suppression,
            marine_losses: p.marine_losses.round() as u32,
            defender_losses: p.defender_losses.round() as u32,
            notes,
        });
    }
}

/// Prune resolved landings on the same terms as battle records: recent ones
/// stay, each corp keeps a floor of its latest, and the total is hard-capped.
/// Running landings are never dropped.
pub fn prune_ground_records(records: &mut std::collections::BTreeMap<EntityId, GroundRecord>, now: f64) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut by_corp: BTreeMap<PlayerId, Vec<(u64, EntityId)>> = BTreeMap::new();
    for (id, r) in records.iter() {
        let recency = r.ended_tick.unwrap_or(u64::MAX);
        by_corp.entry(r.attacker).or_default().push((recency, *id));
        by_corp.entry(r.defender).or_default().push((recency, *id));
    }
    let mut protected: BTreeSet<EntityId> = Default::default();
    for (_corp, mut v) in by_corp {
        v.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        for (_, id) in v.into_iter().take(crate::combat::RECORD_PER_CORP_FLOOR) {
            protected.insert(id);
        }
    }
    records.retain(|id, r| {
        if protected.contains(id) || r.ended_tick.is_none() {
            return true;
        }
        let ended = r.ended_tick.unwrap_or(0) as f64 * crate::config::DT;
        now - ended <= crate::combat::RECORD_RETENTION_SECS
    });
    while records.len() > crate::combat::MAX_BATTLE_RECORDS {
        let Some(oldest) = records
            .iter()
            .filter(|(_, r)| r.ended_tick.is_some())
            .min_by_key(|(id, r)| (r.ended_tick.unwrap_or(0), **id))
            .map(|(id, _)| *id)
        else {
            break;
        };
        records.remove(&oldest);
    }
}

// --- THE PRE-COMMIT ESTIMATE (§ground G4) ------------------------------------
// §8.6's sibling, and it exists for the same reason: a landing is ROLLED now, so
// the difference between a loss reading as "my gamble" and "the game robbed me"
// is entirely whether the odds were on the table before you committed.
//
// Like the battle estimator, this runs THE REAL ENGINE headless — not a
// closed-form approximation of it. If the engine changes, the estimate changes
// with it, and the two can never drift apart.

/// Rollouts per projection. Enough to put the odds within a couple of points,
/// cheap enough to recompute on the wire every time the situation moves.
pub const LANDING_ROLLOUTS: u32 = 96;

/// What a landing is likely to cost, and what happens if the guns leave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandingOdds {
    /// 0..1 — how often this landing takes the ground, at the CURRENT suppression.
    pub win: f64,
    /// Mean marines lost across rollouts (the whole landing, when it fails).
    pub expected_marine_losses: f64,
    /// Mean seconds the fight runs — how long the guns must stay on station.
    pub expected_secs: f64,
    /// THE DECISIVE ONE: the same landing's odds if the bombardment stops the
    /// moment the men are down. A wide gap between this and `win` means the
    /// landing is not really yours — it is the blockade's, and it lasts exactly
    /// as long as you can hold the orbit.
    pub win_if_guns_leave: f64,
}

/// Project a landing by running the real engine `k` times on derived seeds.
/// Deterministic for the same `(inputs, base_seed)`, so two clients asking the
/// same question get the same answer.
pub fn project_landing(
    marines: u32,
    tiers: u32,
    suppression: f64,
    battle_target_secs: f64,
    base_seed: u64,
    k: u32,
) -> LandingOdds {
    if marines == 0 {
        return LandingOdds { win: 0.0, expected_marine_losses: 0.0, expected_secs: 0.0, win_if_guns_leave: 0.0 };
    }
    if tiers == 0 {
        // Undefended ground is taken by anyone who lands on it, guns or no guns.
        return LandingOdds { win: 1.0, expected_marine_losses: 0.0, expected_secs: 0.0, win_if_guns_leave: 1.0 };
    }
    let k = k.max(1);
    let mut wins = 0u32;
    let mut wins_bare = 0u32;
    let mut losses = 0.0;
    let mut secs = 0.0;
    for i in 0..k {
        let seed = base_seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        // The projection steps at 10 Hz rather than the sim's tick rate: the
        // fight is a smooth decay, so this is indistinguishable in outcome and
        // an order of magnitude cheaper to sample.
        let (won, lost, t) = rollout(marines, tiers, suppression, battle_target_secs, seed);
        if won {
            wins += 1;
        }
        losses += lost;
        secs += t;
        let (won_bare, _, _) = rollout(marines, tiers, 0.0, battle_target_secs, seed);
        if won_bare {
            wins_bare += 1;
        }
    }
    LandingOdds {
        win: wins as f64 / k as f64,
        expected_marine_losses: losses / k as f64,
        expected_secs: secs / k as f64,
        win_if_guns_leave: wins_bare as f64 / k as f64,
    }
}

/// One headless landing. Returns (took the ground, marines lost, seconds).
fn rollout(marines: u32, tiers: u32, suppression: f64, target: f64, seed: u64) -> (bool, f64, f64) {
    let mut a = GroundAssault::open(
        EntityId(seed), seed, EntityId(0), Vec2::new(0.0, 0.0),
        PlayerId(0), PlayerId(1), EntityId(0), marines, tiers, 0, target,
    );
    let dt = 0.1;
    let mut t = 0.0;
    // A hard step ceiling: the engine always terminates, but a projection must
    // never be the thing that hangs a tick.
    for _ in 0..20_000 {
        a.step(suppression, dt);
        t += dt;
        if let Some(o) = a.resolved() {
            return (o == GroundOutcome::Taken, marines as f64 - a.marines.max(0.0), t);
        }
    }
    (false, marines as f64, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assault(marines: u32, tiers: u32, seed: u64) -> GroundAssault {
        GroundAssault::open(
            EntityId(1),
            seed,
            EntityId(2),
            Vec2::new(0.0, 0.0),
            PlayerId(1),
            PlayerId(2),
            EntityId(3),
            marines,
            tiers,
            0,
            45.0,
        )
    }

    /// Run to resolution under a FIXED suppression, returning the outcome and
    /// how long it took. 10 Hz steps — fine enough to be smooth, coarse enough
    /// to be quick.
    fn run(a: &mut GroundAssault, suppression: f64) -> (GroundOutcome, f64) {
        let dt = 0.1;
        let mut t = 0.0;
        for _ in 0..100_000 {
            a.step(suppression, dt);
            t += dt;
            if let Some(o) = a.resolved() {
                return (o, t);
            }
        }
        panic!("a landing must always resolve");
    }

    /// LAW 2, the load-bearing half: same seed, same landing, byte-identical.
    /// Every viewer replaying a record must see the same fight.
    #[test]
    fn a_landing_is_reproducible_from_its_seed() {
        let mut a = assault(100, 3, 0xABCD);
        let mut b = assault(100, 3, 0xABCD);
        for _ in 0..200 {
            assert_eq!(a.step(0.2, 0.1), b.step(0.2, 0.1), "same seed, same tick, same casualties");
        }
        assert_eq!(a.marines, b.marines);
        assert_eq!(a.defenders, b.defenders);

        // And a DIFFERENT seed genuinely diverges (else the test above is vacuous).
        let mut c = assault(100, 3, 0x1234);
        let mut same = true;
        for _ in 0..200 {
            if c.step(0.2, 0.1) != a.step(0.2, 0.1) {
                same = false;
            }
        }
        assert!(!same, "a different seed must produce a different fight");
    }

    /// An overwhelming landing wins, and a hopeless one loses — the dice are
    /// spice, never a lottery. This is what keeps a priced decision meaningful.
    #[test]
    fn decisive_odds_are_not_a_lottery() {
        for seed in 0..40u64 {
            let mut strong = assault(300, 2, seed); // 300 v 50
            assert_eq!(run(&mut strong, 0.0).0, GroundOutcome::Taken, "seed {seed}: 6:1 must take the ground");
            let mut weak = assault(20, 4, seed); // 20 v 100
            assert_eq!(run(&mut weak, 0.0).0, GroundOutcome::Repulsed, "seed {seed}: 1:5 must be destroyed");
        }
    }

    /// The BREAK-EVEN point is an even bet — the property the whole UI leans on
    /// once `marines_needed` stops being a guarantee. Not "close to 50%" by
    /// luck: measured across an ensemble, with a band wide enough to be stable
    /// and tight enough to be meaningful.
    #[test]
    fn a_break_even_landing_is_a_coin_flip() {
        let tiers = 4u32;
        let need = break_even_marines(tiers as f64, 0.0).round() as u32;
        assert_eq!(need, 100, "4 unsuppressed tiers break even at 100 marines");
        let mut taken = 0;
        let n = 200;
        for seed in 0..n {
            let mut a = assault(need, tiers, seed);
            if run(&mut a, 0.0).0 == GroundOutcome::Taken {
                taken += 1;
            }
        }
        assert!(
            (60..=140).contains(&taken),
            "a break-even landing should win about half the time, took {taken}/{n}",
        );
    }

    /// SUPPRESSION IS THE DISCOUNT, and the square root is the price of it.
    /// Half a garrison pinned cuts the requirement by 29%, not 50% — troops stay
    /// the thing that takes ground.
    #[test]
    fn bombardment_discounts_a_landing_without_replacing_it() {
        let full = break_even_marines(4.0, 0.0);
        let half = break_even_marines(4.0, 0.5);
        assert_eq!(full, 100.0);
        assert!((half - 70.71).abs() < 0.01, "half-suppressed breaks even at ~71, got {half}");
        // And a landing sized for the SUPPRESSED garrison must actually beat it.
        let mut a = assault(half.ceil() as u32 + 25, 4, 7);
        assert_eq!(run(&mut a, 0.5).0, GroundOutcome::Taken);
        // The same landing against an UNSUPPRESSED garrison is destroyed.
        let mut b = assault(half.ceil() as u32 + 25, 4, 7);
        assert_eq!(run(&mut b, 0.0).0, GroundOutcome::Repulsed, "without the guns, the same force is not enough");
    }

    /// THE POINT OF THE WHOLE MODULE: a landing that is winning while the guns
    /// fire LOSES when the blockade breaks mid-fight. This is the defender's
    /// agency, and it is what makes "a window, not a wound" true in the code
    /// rather than only in the design document.
    #[test]
    fn breaking_the_blockade_mid_landing_turns_the_fight() {
        // Sized to beat a HALF-SUPPRESSED garrison, and not a whole one.
        let marines = break_even_marines(4.0, 0.5).ceil() as u32 + 20;

        // Fight one second under the guns, then FORK: same state, same seed,
        // same remaining dice — the only difference is whether the besieger
        // keeps firing. That makes this a clean counterfactual rather than two
        // loosely-comparable fights.
        let mut mid = assault(marines, 4, 99);
        for _ in 0..10 {
            mid.step(0.5, 0.1);
        }
        assert!(mid.resolved().is_none(), "the fight is still live when relief could arrive");

        let mut kept = mid.clone();
        let mut relieved = mid;
        assert_eq!(
            run(&mut kept, 0.5).0,
            GroundOutcome::Taken,
            "guns kept on station: the landing takes the ground",
        );
        assert_eq!(
            run(&mut relieved, 0.0).0,
            GroundOutcome::Repulsed,
            "guns leave mid-fight: the garrison comes out of cover and destroys the same landing",
        );
    }

    /// An even landing runs for roughly the target duration — the record needs
    /// something to downsample, and the theater needs a fight worth watching.
    #[test]
    fn an_even_landing_lasts_about_the_target_duration() {
        let need = break_even_marines(4.0, 0.0).round() as u32;
        let mut total = 0.0;
        let n = 20;
        for seed in 0..n {
            let mut a = assault(need, 4, seed);
            total += run(&mut a, 0.0).1;
        }
        let avg = total / n as f64;
        // Shorter than the nominal target on purpose: the square law amplifies
        // whatever lead the dice hand out, so a nominally even fight tips and
        // ends early. Duration is EMERGENT — the band is wide because the
        // constant is not tuned to hit a number.
        assert!((20.0..=70.0).contains(&avg), "an even landing should run on the order of the 45 s target, averaged {avg:.1}");
    }

    /// §ground G4: THE ESTIMATE MUST TRACK THE ENGINE. It runs the real thing,
    /// so a landing the estimator calls near-certain has to actually win, and
    /// one it calls hopeless has to actually lose. This is the whole basis on
    /// which a player is asked to commit men to a rolled outcome.
    #[test]
    fn the_estimate_matches_what_the_engine_actually_does() {
        let target = 45.0;
        let check = |marines: u32, tiers: u32, supp: f64| {
            let odds = project_landing(marines, tiers, supp, target, 0xBEEF, 64);
            let mut wins = 0;
            for seed in 0..64u64 {
                let mut a = assault(marines, tiers, 0xBEEF ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                if run(&mut a, supp).0 == GroundOutcome::Taken {
                    wins += 1;
                }
            }
            (odds.win, wins as f64 / 64.0)
        };
        // Decisive both ways: the estimate and the engine agree completely.
        let (e, r) = check(400, 2, 0.0);
        assert_eq!((e, r), (1.0, 1.0), "a 8:1 landing is certain, and the estimate says so");
        let (e, r) = check(15, 6, 0.0);
        assert_eq!((e, r), (0.0, 0.0), "a hopeless landing is hopeless, and the estimate says so");
        // Marginal: the estimate must land near the truth, not merely on the
        // right side of it.
        let (e, r) = check(100, 4, 0.0);
        assert!((e - r).abs() < 0.22, "a break-even landing: estimate {e:.2} vs engine {r:.2}");
        assert!((0.25..=0.75).contains(&e), "and it should read as genuinely uncertain, got {e:.2}");
    }

    /// The counterfactual is the point of the estimator, not a nicety: a landing
    /// that only works while the guns fire must SAY so, loudly, before the men
    /// are committed. Otherwise a besieger whose blockade breaks has no way to
    /// have known the risk they were taking.
    #[test]
    fn the_estimate_warns_when_a_landing_belongs_to_the_bombardment() {
        // Sized to win comfortably under heavy suppression, and to be hopeless
        // without it — exactly the situation a player must be warned about.
        let odds = project_landing(80, 6, 0.85, 45.0, 7, LANDING_ROLLOUTS);
        assert!(odds.win > 0.9, "with the guns on station this landing is a good bet ({:.2})", odds.win);
        assert!(
            odds.win_if_guns_leave < 0.1,
            "and a disaster without them ({:.2}) — the gap IS the warning",
            odds.win_if_guns_leave,
        );
        assert!(odds.expected_secs > 0.0, "and the guns must stay for a knowable length of time");

        // A landing that does NOT depend on the bombardment says that too.
        let solid = project_landing(400, 2, 0.5, 45.0, 7, LANDING_ROLLOUTS);
        assert!(solid.win > 0.95 && solid.win_if_guns_leave > 0.95, "an overwhelming landing needs no guns");
    }

    /// Same question, same answer — two clients must never be quoted different
    /// odds for the same situation.
    #[test]
    fn the_estimate_is_deterministic() {
        let a = project_landing(90, 4, 0.3, 45.0, 12345, LANDING_ROLLOUTS);
        let b = project_landing(90, 4, 0.3, 45.0, 12345, LANDING_ROLLOUTS);
        assert_eq!(a, b);
    }

    /// A mutual wipe leaves the ground with its owner. Ties go to the defender
    /// on purpose — ground not taken is ground held.
    #[test]
    fn the_defender_wins_a_mutual_annihilation() {
        let mut a = assault(50, 2, 3);
        a.marines = 0.4;
        a.defenders = 0.4;
        assert_eq!(a.resolved(), Some(GroundOutcome::Repulsed));
    }
}

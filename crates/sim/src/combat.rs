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

use crate::ship::ShipKind;

// --- TUNABLE COMBAT BLOCK (the Lanchester knobs) --------------------------
/// Fraction of a side's weighted attack power dealt as damage per tick. Tuned
/// so a full-rate engagement plays out over a watchable few seconds at 30 Hz
/// (a lone raider, attack 3, wears down a convoy hull of 10 in ~33 ticks ≈ 1 s;
/// grinds a corvette screen, hull 40, over ~130 ticks ≈ 4 s).
pub const DMG_RATE: f64 = 0.1;
/// Raids are survivable SKIRMISHES: a cargo-raid engagement runs at this
/// fraction of the full rate (both sides bleed slowly), so a raider can seize
/// and run without a decisive bloodbath. Blockade/siege/defense-of-place run at
/// full rate — decisive battles. Travian's two formulas.
pub const RAID_SKIRMISH_MULT: f64 = 0.3;
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
/// the already-scaled per-tick fraction (`DMG_RATE`, times the raid-skirmish
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
        let (fa, fb, la, lb) = project_engagement(&a, &b, DMG_RATE, None, None, 100_000);
        // One side wins but the loser inflicted casualties on the way down.
        assert!(la.total_ships() > 0 && lb.total_ships() > 0, "both sides bled");
        assert!(fa.ship_count() == 0 || fb.ship_count() == 0, "a fight to the finish resolves");
    }

    #[test]
    fn concentration_law_one_big_fleet_beats_two_sequential_halves() {
        // A 20-raider fleet fights two 10-raider fleets ONE AFTER THE OTHER.
        let rate = DMG_RATE;
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
            project_engagement(&forces(&[(ShipKind::Raider, 20)]), &forces(&[(ShipKind::Raider, 10)]), DMG_RATE, None, None, 1_000_000);
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
        let (_ra, _rb, _rla, rlb) = project_engagement(&a, &b, DMG_RATE * RAID_SKIRMISH_MULT, None, None, window);
        let (_ba, _bb, _bla, blb) = project_engagement(&a, &b, DMG_RATE, None, None, window);
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
        let (esc_after, _e1, _l1, _l2) = project_engagement(&attacker(), &escorted, DMG_RATE, None, None, window);
        let (bare_after, _b1, _b2, _b3) = project_engagement(&attacker(), &bare, DMG_RATE, None, None, window);
        // `esc_after`/`bare_after` are the ATTACKER's survivors; compare convoy loss on the defender via a fresh run.
        let (_x, esc_def, _lx, _ly) = project_engagement(&attacker(), &escorted, DMG_RATE, None, None, window);
        let (_p, bare_def, _lp, _lq) = project_engagement(&attacker(), &bare, DMG_RATE, None, None, window);
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
        let (_a, def, _la, lb) = project_engagement(&attacker, &defender, DMG_RATE, None, None, 1_000_000);
        assert!(lb.platform_tiers >= 1, "the platform loses tiers to sustained attack");
        assert!(def.platform_tiers < 2 || !attacker.alive());
    }

    #[test]
    fn mid_battle_relief_flips_the_outcome() {
        // Without relief: the attacker (8 raiders) beats the defender (5) outright.
        let atk = forces(&[(ShipKind::Raider, 8)]);
        let def = || forces(&[(ShipKind::Raider, 5)]);
        let (a_no, d_no, _, _) = project_engagement(&atk, &def(), DMG_RATE, None, None, 1_000_000);
        assert!(a_no.ship_count() > 0 && d_no.ship_count() == 0, "attacker wins without relief");

        // With relief: fight partway, THEN reinforce the defender mid-battle — the
        // accumulated damage plus fresh hulls flips who wins. The drama Travian
        // can't do (relief that arrives during the fight changes the ratio).
        let mut a = atk.clone();
        let mut d = def();
        for _ in 0..25 {
            attrition_tick(&mut a, &mut d, DMG_RATE);
        }
        *d.comp.entry(ShipKind::Raider).or_insert(0) += 7; // relief merges in
        let (a2, d2, _, _) = project_engagement(&a, &d, DMG_RATE, None, None, 1_000_000);
        assert!(d2.ship_count() > 0 && a2.ship_count() == 0, "mid-battle relief flips the outcome");
    }

    #[test]
    fn retreat_at_fraction_preserves_survivors() {
        // A defender that withdraws at 50% strength lost keeps survivors that a
        // fight-to-the-death would have spent.
        let a = forces(&[(ShipKind::Raider, 6)]);
        let b = forces(&[(ShipKind::Raider, 6)]);
        let (_, b_retreat, _, _) = project_engagement(&a, &b, DMG_RATE, None, Some(0.5), 1_000_000);
        let (_, b_death, _, _) = project_engagement(&a, &b, DMG_RATE, None, None, 1_000_000);
        assert!(b_retreat.ship_count() > b_death.ship_count(), "withdrawing at 50% saves ships");
    }

    #[test]
    fn deterministic_no_seed() {
        let a = forces(&[(ShipKind::Raider, 5), (ShipKind::Corvette, 2)]);
        let b = forces(&[(ShipKind::Raider, 7)]);
        let r1 = project_engagement(&a, &b, DMG_RATE, Some(0.5), None, 10_000);
        let r2 = project_engagement(&a, &b, DMG_RATE, Some(0.5), None, 10_000);
        assert_eq!(r1.2, r2.2, "same inputs → same losses (seed-free)");
        assert_eq!(r1.3, r2.3);
    }
}

//! §tactical — THE TACTICAL ENGINE: individual-ship combat, positional-lite.
//!
//! Replaces the pooled Lanchester engine (`combat::attrition_tick`/`absorb`).
//! Battles are simulated as INDIVIDUAL ships with positions, range bands,
//! live torpedo projectiles, and bounded seeded randomness. Player input is
//! unchanged forever: doctrine and fleet orders only — no tactical commands
//! exist or ever will (§law 3: the door stays welded shut).
//!
//! THE THREE LAWS
//! 1. CONTAINMENT — unpack, fight, repack. Fleets are count stacks everywhere
//!    outside an engagement; individuals exist only inside a battle. At open,
//!    stacks unpack into combatants; per step, survivors' HP deficits sync
//!    back into the existing per-stack damage pools and deaths flow through
//!    the existing `Losses` type — the strategic layer cannot tell the engine
//!    changed.
//! 2. SEEDED, ISOLATED RANDOMNESS. Every engagement derives its own RNG stream
//!    from `(world_seed, battle_id)` — same seed, same battle, byte-identical,
//!    for every viewer. The battle stream NEVER touches the world's RNG
//!    (test-enforced): adding or removing a battle shifts no unrelated draw.
//!    Dice live in targeting, to-hit, damage variance (±15%), and torpedo
//!    interception — bounded spice, never wild swings.
//! 3. NO INPUT CREEP. Role scripts are few, dumb, published constants — game
//!    rules, not AI. No per-player scripting, no formation editors.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::combat::{LoadoutMap, Losses, StackPoolMap};
use crate::math::Vec2;
use crate::module::{DamageType, Loadout};
use crate::rng::Rng;
use crate::ship::{hull_affinity, ShipKind};

// --- CADENCE & ARENA (Tunable) ---------------------------------------------------

/// Sim ticks per tactical STEP at the production timescale (30 ticks = 1 Hz).
/// Playtest battles (`battle_target_secs < 120`) run 2 Hz (15 ticks) so a 45 s
/// battle still gets ~90 steps of visible tactics — the cadence answer flagged
/// in the PR: we speed the clock rather than stretch the playtest preset.
pub const TAC_STEP_TICKS: u64 = 30;
pub const TAC_STEP_TICKS_FAST: u64 = 15;
pub fn tac_step_ticks(battle_target_secs: f64) -> u64 {
    if battle_target_secs < 120.0 { TAC_STEP_TICKS_FAST } else { TAC_STEP_TICKS }
}

/// Battle-local arena coordinates ("km-ish"). The defender anchors at the
/// origin; the attacker deploys from their real map-approach bearing at
/// standoff. Withdrawing ships burn for the arena edge and exit at
/// [`WITHDRAW_EXIT_RADIUS`] — pursuers get real shots while the withdrawer is
/// still in envelope (literal pursuit fire replaces the old abstract
/// disengage-exposure number).
pub const ARENA_RADIUS: f64 = 1_000.0;
pub const SPAWN_DIST: f64 = 900.0;
pub const WITHDRAW_EXIT_RADIUS: f64 = 1_400.0;
/// Map-speed → arena-speed scale (map cruise speeds are already a good arena
/// pace: a Raider crosses the arena in ~17 steps, a Titan in ~90).
pub const ARENA_SCALE: f64 = 1.0;
/// Per-step acceleration = `ACCEL_CAL / sqrt(hull_mass)` — capitals feel
/// ponderous without modelling facing/turn radii (out of scope, v1).
pub const ACCEL_CAL: f64 = 600.0;

/// §law: individuals are a per-battle cost, not a strategic one. Fleets beyond
/// the cap hold as reinforcement WAVES, committing (heavies first) as slots
/// open — huge fleets fight in echelons: a perf bound and good fiction.
pub const MAX_COMBATANTS_PER_SIDE: usize = 300;

// --- RANGE BANDS & WEAPONS (Tunable) -----------------------------------------------

/// Beam: LONG range, hitscan (arrives with its own light — nothing dodges it
/// at the firing solution level; its counter stays Reflective).
pub const BEAM_RANGE: f64 = 650.0;
/// Driver: MID range hitscan; beyond the band, falloff to [`BEAM_RANGE`].
pub const DRIVER_RANGE: f64 = 350.0;
pub const DRIVER_FALLOFF: f64 = 0.45;
/// Torpedo: launched from LONG standoff as a live entity — it travels across
/// steps, tracks its target, and can be killed in flight.
pub const TORP_LAUNCH_RANGE: f64 = 800.0;
pub const TORP_SPEED: f64 = 140.0;
pub const TORP_HIT_DIST: f64 = 30.0;

/// Weapon cooldowns in tactical steps.
pub const BEAM_COOLDOWN: u8 = 1;
pub const DRIVER_COOLDOWN: u8 = 2;
pub const TORP_COOLDOWN: u8 = 4;

/// TO-HIT — where the emergence lives:
/// `hit = clamp(BASE[family] × mass_factor(target) / (1 + speed_factor(target)))`.
/// Beams track well (near-flat); drivers are brutal against big slow hulls and
/// poor against darting Corvettes; torpedoes near-guarantee against capitals
/// and struggle against small fast ships. This SUPERSEDES the warship-ladder
/// handoff's bolted-on `TORP_CAPITAL_EDGE` (deleted): the capital-hunting
/// torpedo and the wolfpack answer are now EMERGENT from tracking.
pub const HIT_BASE: [f64; 3] = [0.85, 0.70, 0.90]; // beam, driver, torpedo
pub const HIT_MASS_EXP: [f64; 3] = [0.05, 0.35, 0.50];
/// Speed protects — hard. A darting skirmisher at flank speed shrugs off most
/// fire (small fights stay spicy and LONG); anything holding a line or
/// anchoring is a barn. Ordering preserved: beams track best, torpedoes worst
/// against the fast and best against the ponderous.
pub const HIT_SPEED_SENS: [f64; 3] = [2.60, 3.40, 4.20];
pub const HIT_CLAMP: (f64, f64) = (0.05, 0.95);

fn family_idx(ty: DamageType) -> usize {
    match ty {
        DamageType::Beam => 0,
        DamageType::Driver => 1,
        DamageType::Torpedo => 2,
    }
}

/// The chance one shot of `family` connects with a target of `mass` moving at
/// `speed` (its CURRENT arena velocity — a withdrawing burner is harder to hit
/// than a station-keeper, an anchored capital is a barn).
pub fn to_hit(family: DamageType, target_mass: f64, target_speed: f64) -> f64 {
    let i = family_idx(family);
    let m = (target_mass / 1_000.0).max(0.01).powf(HIT_MASS_EXP[i]);
    let v = (target_speed / 100.0) * HIT_SPEED_SENS[i];
    (HIT_BASE[i] * m / (1.0 + v)).clamp(HIT_CLAMP.0, HIT_CLAMP.1)
}

/// Damage per HIT: `offense_mult(loadout) × attack_weight(kind) × HIT_DMG_CAL
/// × affinity(kind, family) × U(0.85, 1.15)`, then per-target mitigation as
/// per-hit multipliers (Reflective vs beam, Whipple vs driver; torpedoes
/// ignore both). The counter matrix survives intact as EXPECTED-VALUE
/// relationships. `HIT_DMG_CAL` is the calibration dial: equal reference
/// forces resolve near `battle_target_secs` (statistical test).
pub const HIT_DMG_CAL: f64 = 22.0;
pub const DMG_VAR: (f64, f64) = (0.85, 1.15);
/// UNARMED hulls (attack weight 0 — convoys, colony ships) take amplified
/// damage: civilian bulk carries no armor belts and no damage control. This is
/// what keeps a raid a smash-and-grab and a caught colony ship a loss, even
/// though HP now scales with hull mass. Tunable, legible, published.
pub const CIVILIAN_SOFT: f64 = 10.0;
/// A raid is a smash-and-grab, not a line battle — its steps hit harder so it
/// resolves inside the short raid cap (mirrors the old RAID_RATE asymmetry),
/// with the approach run eating a real slice of the window.
pub const RAID_DMG_MULT: f64 = 14.0;

/// PD is LITERAL now: each PD-fitted ship rolls intercepts against torpedoes
/// entering its screen radius; a Dreadnought (or platform) projects a
/// platform-grade radius. Screening is positional truth: a PD Corvette
/// actually standing between the torpedo axis and your Battleship intercepts
/// more, because more torpedoes cross its bubble.
pub const PD_RADIUS: f64 = 180.0;
pub const PD_RADIUS_HEAVY: f64 = 400.0; // Dreadnought + platform projection
pub const PD_ROLL_BASE: f64 = 0.35;

/// Opening-window techs (First Strike, Grand Batteries) = a damage bonus
/// during the first [`OPENING_STEPS`] tactical steps.
pub const OPENING_STEPS: u64 = 5;
pub const OPENING_BONUS: f64 = 1.25;

/// Platform tiers fight as STATIONARY combatants at the defender's anchor:
/// big HP, a beam battery, and platform-grade PD. Tunable.
pub const PLATFORM_TIER_HP: f64 = 1_200.0;
pub const PLATFORM_TIER_AW: f64 = 3.0;

// --- ROLES (few, dumb, legible — published game rules, not AI) --------------------

/// The role SCRIPTS. Assigned at unpack from kind + loadout + doctrine; the
/// only mid-battle transition is doctrine-triggered `Withdraw`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Capitals + the anchored defender: hold station at the line's center.
    Anchor,
    /// Corvette/Destroyer/Cruiser default: advance to the preferred weapon
    /// band, hold, fire.
    Line,
    /// PD-fitted: interpose between own heavies and the dominant torpedo
    /// threat axis, recomputed per step.
    Screen,
    /// Raiders (and other fast light hulls): orbit the flanks at torpedo
    /// standoff.
    Skirmish,
    /// Doctrine-triggered: burn for the disengage edge; pursuers get real
    /// shots while the withdrawer is in envelope.
    Withdraw,
}

/// Per-weapon-family cooldown clocks (steps until ready).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct TypedCooldowns {
    pub beam: u8,
    pub driver: u8,
    pub torpedo: u8,
}

/// One SHIP inside a battle. Exists only here (§law 1) — outside, it is a
/// count in a stack. `stack` names the (kind, loadout-key) it repacks into.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Combatant {
    pub cid: u32,
    pub side: u8,
    pub kind: ShipKind,
    pub stack: String,
    /// Remaining hull. Unpack subtracts a pro-rata share of the stack's
    /// existing damage pool; repack returns `max_hp − hp` into it.
    pub hp: f64,
    pub max_hp: f64,
    pub pos: Vec2,
    pub vel: Vec2,
    pub cooldowns: TypedCooldowns,
    pub role: Role,
    /// A Defense Platform TIER (stationary; `kind`/`stack` unused for repack —
    /// tiers sync back to the system, not to a fleet).
    #[serde(default)]
    pub platform: bool,
}

impl Combatant {
    fn loadout(&self) -> Loadout {
        Loadout::from_key(&self.stack)
    }
    fn speed(&self) -> f64 {
        self.vel.length()
    }
    /// The PD screen radius this hull projects (0 = no PD fitted).
    fn pd_radius(&self) -> f64 {
        if self.platform {
            return PD_RADIUS_HEAVY;
        }
        if !self.loadout().has_pd() {
            return 0.0;
        }
        if self.kind == ShipKind::Dreadnought { PD_RADIUS_HEAVY } else { PD_RADIUS }
    }
}

/// A LIVE torpedo: launched at long standoff, travels across steps, tracks its
/// target, and can be killed in flight (PD rolls) — the projectile is real.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Torpedo {
    pub side: u8,
    pub pos: Vec2,
    pub target: u32, // cid — dies with its target (no retargeting, v1)
    pub dmg: f64,
}

/// One reinforcement WAVE entry: a stack held past the per-side cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveEntry {
    pub kind: ShipKind,
    pub stack: String,
    pub count: u32,
}

/// What one tactical step did — the world applies it through the SAME channels
/// the old engine used (`Losses`, per-stack pools, platform tiers).
#[derive(Debug, Default)]
pub struct StepOutcome {
    pub losses: [Losses; 2],
    /// Damage DEALT by each side this step (the record's scalar readout).
    pub dealt: [f64; 2],
    /// Platform tiers destroyed this step.
    pub platform_tiers_lost: u32,
}

/// Per-side research flags fed into a step (owner-level lookups happen in the
/// world — the engine stays pure).
#[derive(Debug, Clone, Copy, Default)]
pub struct SideMods {
    pub opening_bonus: bool, // First Strike / Grand Batteries
    pub flak_mult: f64,      // PD roll multiplier (1.0 = base)
}

impl SideMods {
    pub fn flak(&self) -> f64 {
        if self.flak_mult <= 0.0 { 1.0 } else { self.flak_mult }
    }
}

// --- THE PERSISTED BATTLE STATE ---------------------------------------------------

/// The tactical state carried by an `Engagement` (serde — a mid-battle
/// snapshot resumes exactly). Old snapshots load with `None` and MIGRATE
/// one-way at the next tick: the pooled counts + pools unpack into combatants
/// and the battle continues under this engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalState {
    pub rng: Rng,
    pub step: u64,
    pub next_cid: u32,
    pub combatants: Vec<Combatant>,
    #[serde(default)]
    pub torpedoes: Vec<Torpedo>,
    /// Over-cap holdbacks per side, committed heavies-first as slots open.
    #[serde(default)]
    pub waves: [Vec<WaveEntry>; 2],
    /// Starting ship HP per side (retreat baseline — set at open, like the old
    /// engine's start strength; reinforcements raise CURRENT only).
    pub start_hp: [f64; 2],
    /// The attacker's deployment bearing (unit vector from the defender).
    pub bearing: Vec2,
    /// Sides ordered to WITHDRAW (doctrine/raid-cap/safety) — sticky.
    #[serde(default)]
    pub withdrawing: [bool; 2],
}

/// Derive the battle's own RNG stream from `(world_seed, battle_id)` — never
/// from the world's live stream (§law 2).
pub fn battle_rng(world_seed: u64, battle_id: u64) -> Rng {
    Rng::new(world_seed ^ battle_id.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// The desired per-side composition, as (kind → loadout-key → count) with the
/// UNFITTED remainder under the `""` key — the sync/unpack input shape.
pub fn stacked(comp: &BTreeMap<ShipKind, u32>, loadouts: &LoadoutMap) -> LoadoutMap {
    let mut out: LoadoutMap = BTreeMap::new();
    for (kind, total) in comp {
        let mut fitted = 0u32;
        if let Some(m) = loadouts.get(kind) {
            for (key, n) in m {
                if key.is_empty() || *n == 0 {
                    continue;
                }
                let take = (*n).min(total.saturating_sub(fitted));
                if take > 0 {
                    *out.entry(*kind).or_default().entry(key.clone()).or_insert(0) += take;
                    fitted += take;
                }
            }
        }
        if *total > fitted {
            *out.entry(*kind).or_default().entry(String::new()).or_insert(0) += total - fitted;
        }
    }
    out
}

impl TacticalState {
    /// OPEN a battle: derive the stream, deploy the defender anchored at the
    /// origin and the attacker at standoff on their real approach bearing.
    /// Platforms unpack as stationary combatants. Scouts never unpack — they
    /// die at the boundary (the caller books them, exactly like the old
    /// `strip_scouts`).
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        world_seed: u64,
        battle_id: u64,
        a: &LoadoutMap,
        d: &LoadoutMap,
        a_pool: &StackPoolMap,
        d_pool: &StackPoolMap,
        platform_tiers: u32,
        platform_pool: f64,
        bearing: Vec2,
    ) -> Self {
        let b = if bearing.length() > 1e-9 { bearing.normalized() } else { Vec2::new(1.0, 0.0) };
        let mut st = TacticalState {
            rng: battle_rng(world_seed, battle_id),
            step: 0,
            next_cid: 0,
            combatants: Vec::new(),
            torpedoes: Vec::new(),
            waves: [Vec::new(), Vec::new()],
            start_hp: [0.0, 0.0],
            bearing: b,
            withdrawing: [false, false],
        };
        st.deploy_side(0, a, a_pool);
        st.deploy_side(1, d, d_pool);
        // Platform tiers: stationary anchors at the origin, damage pool spread
        // pro-rata like a ship stack's.
        if platform_tiers > 0 {
            let per = platform_pool / platform_tiers as f64;
            for i in 0..platform_tiers {
                let cid = st.alloc_cid();
                st.combatants.push(Combatant {
                    cid,
                    side: 1,
                    kind: ShipKind::Corvette, // unused for platforms (never repacked to a fleet)
                    stack: String::new(),
                    hp: (PLATFORM_TIER_HP - per).max(1.0),
                    max_hp: PLATFORM_TIER_HP,
                    pos: Vec2::new((i as f64 - platform_tiers as f64 / 2.0) * 20.0, 0.0),
                    vel: Vec2::new(0.0, 0.0),
                    cooldowns: TypedCooldowns::default(),
                    role: Role::Anchor,
                    platform: true,
                });
            }
        }
        st.start_hp = [st.side_hp(0, false), st.side_hp(1, false)];
        st
    }

    fn alloc_cid(&mut self) -> u32 {
        let c = self.next_cid;
        self.next_cid += 1;
        c
    }

    /// Deploy one side's stacks (heavies first, deterministic), respecting the
    /// per-side cap; the overflow holds as waves.
    fn deploy_side(&mut self, side: u8, stacks: &LoadoutMap, pools: &StackPoolMap) {
        // Heavies first: sort stacks by hull mass desc, then enum order, then key.
        let mut order: Vec<(&ShipKind, &String, &u32)> = Vec::new();
        for (k, m) in stacks {
            for (key, n) in m {
                if *n > 0 {
                    order.push((k, key, n));
                }
            }
        }
        order.sort_by(|a, b| {
            b.0.hull_mass()
                .partial_cmp(&a.0.hull_mass())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(b.0))
                .then(a.1.cmp(b.1))
        });
        for (kind, key, n) in order {
            if *kind == ShipKind::Scout {
                continue; // scouts die at the boundary — never unpacked
            }
            let pool = pools.get(kind).and_then(|m| m.get(key)).copied().unwrap_or(0.0);
            let per = pool / (*n).max(1) as f64;
            for _ in 0..*n {
                self.spawn(side, *kind, key.clone(), per);
            }
        }
    }

    /// Spawn one combatant (or hold it in a wave past the cap).
    fn spawn(&mut self, side: u8, kind: ShipKind, stack: String, pool_share: f64) {
        let alive = self.combatants.iter().filter(|c| c.side == side && !c.platform).count();
        if alive >= MAX_COMBATANTS_PER_SIDE {
            // Hold as a wave entry (merged per stack, deterministic order).
            let w = &mut self.waves[side as usize];
            if let Some(e) = w.iter_mut().find(|e| e.kind == kind && e.stack == stack) {
                e.count += 1;
            } else {
                w.push(WaveEntry { kind, stack, count: 1 });
            }
            return;
        }
        let hp = (kind.hull_mass() - pool_share).max(1.0);
        let idx = self.combatants.iter().filter(|c| c.side == side).count() as f64;
        // Deployment: defender anchored around the origin, attacker at standoff
        // along the approach bearing; a deterministic lateral fan spreads the line.
        let lateral = Vec2::new(-self.bearing.y, self.bearing.x);
        let spread = (idx % 21.0 - 10.0) * 35.0 + (idx / 21.0).floor() * 12.0;
        let pos = if side == 0 {
            self.bearing * SPAWN_DIST + lateral * spread
        } else {
            lateral * spread * 0.6 - self.bearing * (idx / 21.0).floor() * 30.0
        };
        let role = role_for(kind, &Loadout::from_key(&stack), side);
        let cid = self.alloc_cid();
        self.combatants.push(Combatant {
            cid,
            side,
            kind,
            stack,
            hp,
            max_hp: kind.hull_mass(),
            pos,
            vel: Vec2::new(0.0, 0.0),
            cooldowns: TypedCooldowns::default(),
            role,
        platform: false,
        });
    }

    /// SYNC the engine to the live strategic sides (joins unpack, withdrawals
    /// remove — the old engine got this for free by rebuilding every tick).
    /// Returns scout counts that must die at the boundary per side.
    pub fn sync(&mut self, desired: [&LoadoutMap; 2]) -> [u32; 2] {
        let mut scouts = [0u32; 2];
        for side in 0..2u8 {
            // Current per-stack counts (alive + held waves).
            let mut have: BTreeMap<(ShipKind, String), u32> = BTreeMap::new();
            for c in self.combatants.iter().filter(|c| c.side == side && !c.platform) {
                *have.entry((c.kind, c.stack.clone())).or_insert(0) += 1;
            }
            for w in &self.waves[side as usize] {
                *have.entry((w.kind, w.stack.clone())).or_insert(0) += w.count;
            }
            let mut want: BTreeMap<(ShipKind, String), u32> = BTreeMap::new();
            for (k, m) in desired[side as usize] {
                for (key, n) in m {
                    if *k == ShipKind::Scout {
                        scouts[side as usize] += *n;
                        continue;
                    }
                    if *n > 0 {
                        *want.entry((*k, key.clone())).or_insert(0) += *n;
                    }
                }
            }
            // Removals first (withdrawn fleets / strategic shrinkage): waves
            // first, then the LAST-spawned combatants of the stack (stable).
            let keys: Vec<(ShipKind, String)> = have.keys().cloned().collect();
            for key in keys {
                let h = have.get(&key).copied().unwrap_or(0);
                let w = want.get(&key).copied().unwrap_or(0);
                if h > w {
                    let mut excess = h - w;
                    let waves = &mut self.waves[side as usize];
                    if let Some(e) = waves.iter_mut().find(|e| e.kind == key.0 && e.stack == key.1) {
                        let take = e.count.min(excess);
                        e.count -= take;
                        excess -= take;
                    }
                    waves.retain(|e| e.count > 0);
                    while excess > 0 {
                        if let Some(i) = self
                            .combatants
                            .iter()
                            .rposition(|c| c.side == side && !c.platform && c.kind == key.0 && c.stack == key.1)
                        {
                            self.combatants.remove(i);
                            excess -= 1;
                        } else {
                            break;
                        }
                    }
                }
            }
            // Additions (relief joining the line): spawn at the side's edge.
            for (key, w) in &want {
                let h = have.get(key).copied().unwrap_or(0);
                for _ in h..*w {
                    self.spawn(side, key.0, key.1.clone(), 0.0);
                }
            }
        }
        scouts
    }

    /// Commit held waves into freed slots (heavies first — the same order the
    /// deploy used; deterministic echelons).
    fn commit_waves(&mut self) {
        for side in 0..2u8 {
            loop {
                let alive = self.combatants.iter().filter(|c| c.side == side && !c.platform).count();
                if alive >= MAX_COMBATANTS_PER_SIDE {
                    break;
                }
                // Heaviest held stack first.
                let Some(best) = self.waves[side as usize]
                    .iter()
                    .enumerate()
                    .max_by(|(ai, a), (bi, b)| {
                        a.kind
                            .hull_mass()
                            .partial_cmp(&b.kind.hull_mass())
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then(b.kind.cmp(&a.kind))
                            .then(b.stack.cmp(&a.stack))
                            .then(bi.cmp(ai))
                    })
                    .map(|(i, _)| i)
                else {
                    break;
                };
                let e = &mut self.waves[side as usize][best];
                let kind = e.kind;
                let stack = e.stack.clone();
                e.count -= 1;
                if e.count == 0 {
                    self.waves[side as usize].remove(best);
                }
                self.spawn(side, kind, stack, 0.0);
            }
        }
    }

    /// Σ remaining ship HP for a side (the retreat metric; platforms excluded
    /// unless `with_platform`).
    pub fn side_hp(&self, side: u8, with_platform: bool) -> f64 {
        self.combatants
            .iter()
            .filter(|c| c.side == side && (with_platform || !c.platform))
            .map(|c| c.hp)
            .sum()
    }

    pub fn alive(&self, side: u8) -> usize {
        self.combatants.iter().filter(|c| c.side == side && !c.platform).count()
            + self.waves[side as usize].iter().map(|w| w.count as usize).sum::<usize>()
    }

    pub fn platform_tiers(&self) -> u32 {
        self.combatants.iter().filter(|c| c.platform).count() as u32
    }

    /// All of `side`'s ships are dead or beyond the disengage edge.
    pub fn side_withdrawn(&self, side: u8) -> bool {
        self.withdrawing[side as usize]
            && self
                .combatants
                .iter()
                .filter(|c| c.side == side && !c.platform)
                .all(|c| c.pos.length() >= WITHDRAW_EXIT_RADIUS)
    }

    /// Order a side to withdraw (doctrine retreat / raid cap / safety valve).
    /// Sticky; every ship of the side flips to the Withdraw script.
    pub fn order_withdraw(&mut self, side: u8) {
        self.withdrawing[side as usize] = true;
        for c in self.combatants.iter_mut().filter(|c| c.side == side && !c.platform) {
            c.role = Role::Withdraw;
        }
        // Held waves never commit into a withdrawal.
        self.waves[side as usize].clear();
    }

    /// The per-stack damage pools implied by current HP deficits — persisted
    /// back onto the engagement each step so every existing consumer (serde,
    /// withdraw-mid-battle, repack) sees the exact old shapes (§law 1).
    pub fn pools(&self, side: u8) -> StackPoolMap {
        let mut out: StackPoolMap = BTreeMap::new();
        for c in self.combatants.iter().filter(|c| c.side == side && !c.platform) {
            let deficit = (c.max_hp - c.hp).max(0.0);
            if deficit > 1e-9 {
                *out.entry(c.kind).or_default().entry(c.stack.clone()).or_insert(0.0) += deficit;
            }
        }
        out
    }

    /// Platform damage pool (deficit of the LIVE tiers).
    pub fn platform_pool(&self) -> f64 {
        self.combatants.iter().filter(|c| c.platform).map(|c| (c.max_hp - c.hp).max(0.0)).sum()
    }

    // --- THE STEP -----------------------------------------------------------------

    /// One tactical step: movement → torpedo flight/PD → fire. Pure over its
    /// own state + the derived stream; the world's RNG is never touched.
    pub fn step(&mut self, raid: bool, mods: [SideMods; 2]) -> StepOutcome {
        self.step += 1;
        self.commit_waves();
        let mut out = StepOutcome::default();

        // 1. MOVEMENT: seek/arrive toward each role's desired point.
        let desired: Vec<Vec2> = (0..self.combatants.len()).map(|i| self.desired_point(i)).collect();
        for (i, want) in desired.iter().enumerate() {
            let c = &mut self.combatants[i];
            if c.platform {
                continue;
            }
            let max_speed = c.kind.max_speed() * ARENA_SCALE;
            let accel = ACCEL_CAL / c.max_hp.sqrt();
            let to = *want - c.pos;
            let dist = to.length();
            // Arrive: ease off inside two steps of the mark.
            let target_vel = if dist < 1e-6 {
                Vec2::new(0.0, 0.0)
            } else {
                to.normalized() * max_speed.min(dist / 1.5)
            };
            let dv = target_vel - c.vel;
            let dvl = dv.length();
            c.vel = if dvl <= accel { target_vel } else { c.vel + dv.normalized() * accel };
            c.pos = c.pos + c.vel;
        }

        // 2. TORPEDOES: fly, cross PD bubbles (intercept rolls), strike.
        let mut torps = std::mem::take(&mut self.torpedoes);
        let mut dead_torps: Vec<usize> = Vec::new();
        for (ti, t) in torps.iter_mut().enumerate() {
            let Some(target) = self.combatants.iter().find(|c| c.cid == t.target) else {
                dead_torps.push(ti); // target already gone — the fish runs dry
                continue;
            };
            let to = target.pos - t.pos;
            let dist = to.length();
            let travel = TORP_SPEED.min(dist);
            t.pos = t.pos + if dist > 1e-9 { to.normalized() * travel } else { Vec2::new(0.0, 0.0) };
            // PD: every enemy screen the torpedo is INSIDE this step rolls once.
            let mut intercepted = false;
            for pd in self.combatants.iter().filter(|c| c.side != t.side) {
                let r = pd.pd_radius();
                if r > 0.0 && (t.pos - pd.pos).length() <= r {
                    let aff = if pd.platform {
                        1.0
                    } else {
                        hull_affinity(pd.kind, crate::module::Family::Interception)
                    };
                    let chance = (PD_ROLL_BASE * aff * mods[(1 - t.side) as usize].flak()).min(0.95);
                    if self.rng.next_f64() < chance {
                        intercepted = true;
                        break;
                    }
                }
            }
            if intercepted {
                dead_torps.push(ti);
                continue;
            }
            // Terminal: to-hit vs the (possibly darting) target.
            if (t.pos - target.pos).length() <= TORP_HIT_DIST {
                let hit = to_hit(DamageType::Torpedo, target.max_hp, target.speed());
                if self.rng.next_f64() < hit {
                    let tid = target.cid;
                    let dmg = t.dmg; // armor ignores torpedoes — no mitigation
                    self.apply_damage(tid, dmg, &mut out, t.side);
                }
                dead_torps.push(ti);
            }
        }
        for i in dead_torps.into_iter().rev() {
            torps.remove(i);
        }
        self.torpedoes = torps;

        // 3. FIRE: index order (deterministic); dead targets drop out as they die.
        for i in 0..self.combatants.len() {
            let (side, pos, kind, stack, plat, cds, alive) = {
                let c = &self.combatants[i];
                (c.side, c.pos, c.kind, c.stack.clone(), c.platform, c.cooldowns, c.hp > 0.0)
            };
            if !alive {
                continue;
            }
            let lo = Loadout::from_key(&stack);
            let (family, mult) = if plat { (DamageType::Beam, 1.0) } else { lo.offense() };
            let cd = match family {
                DamageType::Beam => cds.beam,
                DamageType::Driver => cds.driver,
                DamageType::Torpedo => cds.torpedo,
            };
            if cd > 0 {
                let c = &mut self.combatants[i];
                c.cooldowns.beam = c.cooldowns.beam.saturating_sub(1);
                c.cooldowns.driver = c.cooldowns.driver.saturating_sub(1);
                c.cooldowns.torpedo = c.cooldowns.torpedo.saturating_sub(1);
                continue;
            }
            let aw = if plat { PLATFORM_TIER_AW } else { kind.attack_weight() };
            if aw <= 0.0 {
                continue; // civilians carry no guns
            }
            let band = match family {
                DamageType::Beam => BEAM_RANGE,
                DamageType::Driver => BEAM_RANGE, // falloff shots allowed out to long
                DamageType::Torpedo => TORP_LAUNCH_RANGE,
            };
            // TARGETING: seeded weighted roll over in-range enemies, weight ∝
            // threat mass (doctrine bias is a future hook, documented).
            let mut cands: Vec<(u32, f64, f64, f64, bool)> = Vec::new(); // cid, weight, dist, speed, plat
            for e in self.combatants.iter().filter(|e| e.side != side && e.hp > 0.0) {
                let d = (e.pos - pos).length();
                if d <= band {
                    cands.push((e.cid, e.max_hp.max(1.0), d, e.speed(), e.platform));
                }
            }
            if cands.is_empty() {
                continue;
            }
            let total: f64 = cands.iter().map(|c| c.1).sum();
            let mut roll = self.rng.next_f64() * total;
            let mut pick = cands.len() - 1;
            for (ci, c) in cands.iter().enumerate() {
                if roll < c.1 {
                    pick = ci;
                    break;
                }
                roll -= c.1;
            }
            let (tcid, _, tdist, tspeed, tplat) = cands[pick];
            let (tmax, tkind, tstack) = self
                .combatants
                .iter()
                .find(|c| c.cid == tcid)
                .map(|c| (c.max_hp, c.kind, c.stack.clone()))
                .unwrap_or((1.0, ShipKind::Corvette, String::new()));
            // Damage: offense mult × attack weight × calibration × affinity ×
            // variance; opening-window techs boost the first steps.
            let aff = if plat { 1.0 } else { hull_affinity(kind, crate::module::weapon_family(family)) };
            let opening = if self.step <= OPENING_STEPS && mods[side as usize].opening_bonus { OPENING_BONUS } else { 1.0 };
            let raid_mult = if raid { RAID_DMG_MULT } else { 1.0 };
            let base = mult * aw * HIT_DMG_CAL * aff * opening * raid_mult;
            match family {
                DamageType::Torpedo => {
                    let dmg = base * self.rng.range(DMG_VAR.0, DMG_VAR.1);
                    self.torpedoes.push(Torpedo { side, pos, target: tcid, dmg });
                    self.combatants[i].cooldowns.torpedo = TORP_COOLDOWN;
                }
                DamageType::Beam | DamageType::Driver => {
                    let mut hit = to_hit(family, tmax, tspeed);
                    if family == DamageType::Driver && tdist > DRIVER_RANGE {
                        hit *= DRIVER_FALLOFF; // beyond the band — falloff
                    }
                    let connects = self.rng.next_f64() < hit;
                    let mut dmg = base * self.rng.range(DMG_VAR.0, DMG_VAR.1);
                    // PER-HIT MITIGATION — the counter matrix, as multipliers:
                    // Reflective blunts beam, Whipple blunts driver, torpedoes
                    // ignore both (their branch never reaches here). Platforms
                    // carry no armor. Protection affinity scales the blunt,
                    // clamped below immunity (as in the pooled engine).
                    if !tplat {
                        let tlo = Loadout::from_key(&tstack);
                        let prot = hull_affinity(tkind, crate::module::Family::Protection);
                        if family == DamageType::Beam && tlo.reflects() {
                            dmg *= 1.0 - (crate::module::REFLECT_BLUNT * prot).min(0.95);
                        }
                        if family == DamageType::Driver && tlo.whipples() {
                            dmg *= 1.0 - (crate::module::WHIPPLE_BLUNT * prot).min(0.95);
                        }
                    }
                    if connects {
                        self.apply_damage(tcid, dmg, &mut out, side);
                    }
                    let c = &mut self.combatants[i];
                    match family {
                        DamageType::Beam => c.cooldowns.beam = BEAM_COOLDOWN,
                        _ => c.cooldowns.driver = DRIVER_COOLDOWN,
                    }
                }
            }
        }

        // 4. DEATHS: collect per-stack losses; remove the fallen.
        let mut dead: Vec<usize> = Vec::new();
        for (i, c) in self.combatants.iter().enumerate() {
            if c.hp <= 0.0 {
                dead.push(i);
            }
        }
        for i in dead.iter().rev() {
            let c = self.combatants.remove(*i);
            if c.platform {
                out.platform_tiers_lost += 1;
            } else {
                out.losses[c.side as usize].add_stack(c.kind, Loadout::from_key(&c.stack), 1);
            }
        }
        out
    }

    /// Damage into a combatant, with PER-HIT armor mitigation (Reflective vs
    /// beam, Whipple vs driver — torpedo callers skip this by passing the raw
    /// value; see call sites). Unarmed hulls take [`CIVILIAN_SOFT`] × the hit.
    /// Credits `dealt` to the firing side.
    fn apply_damage(&mut self, cid: u32, dmg: f64, out: &mut StepOutcome, by_side: u8) {
        if let Some(c) = self.combatants.iter_mut().find(|c| c.cid == cid) {
            let soft = if !c.platform && c.kind.attack_weight() <= 0.0 { CIVILIAN_SOFT } else { 1.0 };
            let dealt = dmg * soft;
            c.hp -= dealt;
            out.dealt[by_side as usize] += dealt;
        }
    }

    /// The role script's desired point for combatant `i` (published rules).
    fn desired_point(&self, i: usize) -> Vec2 {
        let c = &self.combatants[i];
        if c.platform {
            return c.pos;
        }
        let enemy_centroid = self.centroid(1 - c.side).unwrap_or(Vec2::new(0.0, 0.0));
        match c.role {
            Role::Withdraw => {
                // Burn for the disengage edge, directly away from the enemy.
                let away = c.pos - enemy_centroid;
                let dir = if away.length() > 1e-9 { away.normalized() } else { self.bearing * if c.side == 0 { 1.0 } else { -1.0 } };
                c.pos + dir * 400.0
            }
            Role::Anchor => {
                // Hold the line's center: defenders at the anchor, attackers at
                // their rally (bearing standoff shrunk to the beam band).
                let rally = if c.side == 0 { self.bearing * (BEAM_RANGE * 0.85) } else { Vec2::new(0.0, 0.0) };
                // Civilians tuck BEHIND the rally, away from the enemy.
                if c.kind.attack_weight() <= 0.0 {
                    let away = rally - enemy_centroid;
                    let dir = if away.length() > 1e-9 { away.normalized() } else { self.bearing };
                    return rally + dir * 250.0;
                }
                rally
            }
            Role::Line => {
                // Advance to the preferred weapon band off the nearest enemy —
                // then HOLD: a line ship inside its band stops and shoots
                // rather than chasing a darting skirmisher forever (holding
                // still trades evasion for gunnery; that's the line's job).
                let band = self.preferred_band(c);
                let near = self.nearest_enemy(i).unwrap_or(enemy_centroid);
                let from = c.pos - near;
                if from.length() <= band * 1.15 {
                    return c.pos; // in band — hold and fire
                }
                let dir = if from.length() > 1e-9 { from.normalized() } else { self.bearing };
                near + dir * band
            }
            Role::Screen => {
                // Interpose between our heaviest hull and the dominant torpedo
                // threat axis (the enemy torpedo-armed centroid), per step.
                let heavy = self
                    .combatants
                    .iter()
                    .filter(|h| h.side == c.side && !h.platform)
                    .max_by(|a, b| a.max_hp.partial_cmp(&b.max_hp).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|h| h.pos)
                    .unwrap_or(c.pos);
                let threat = self.torpedo_threat_centroid(1 - c.side).unwrap_or(enemy_centroid);
                let axis = threat - heavy;
                let dir = if axis.length() > 1e-9 { axis.normalized() } else { self.bearing };
                heavy + dir * (PD_RADIUS * 0.9)
            }
            Role::Skirmish => {
                // ORBIT the flanks at the skirmisher's OWN weapon standoff (a
                // torpedo boat rides the long fish band; a gun raider must
                // close to its band to bite). The desired point sits AHEAD on
                // the ring, so the skirmisher keeps circling at flank speed —
                // its speed IS its armor; a stalled skirmisher is a dead one.
                let band = self.preferred_band(c);
                let near = self.nearest_enemy(i).unwrap_or(enemy_centroid);
                let from = c.pos - near;
                let dir = if from.length() > 1e-9 { from.normalized() } else { self.bearing };
                // Rotate ~0.5 rad ahead around the target (cid parity picks the
                // orbit direction, so a squadron doesn't conga in one file).
                let (s, co) = if c.cid.is_multiple_of(2) { (0.48f64.sin(), 0.48f64.cos()) } else { (-(0.48f64.sin()), 0.48f64.cos()) };
                let ahead = Vec2::new(dir.x * co - dir.y * s, dir.x * s + dir.y * co);
                near + ahead * (band * 0.95)
            }
        }
    }

    fn preferred_band(&self, c: &Combatant) -> f64 {
        match Loadout::from_key(&c.stack).offense().0 {
            DamageType::Beam => BEAM_RANGE * 0.85,
            DamageType::Driver => DRIVER_RANGE * 0.85,
            DamageType::Torpedo => TORP_LAUNCH_RANGE * 0.9,
        }
    }

    fn nearest_enemy(&self, i: usize) -> Option<Vec2> {
        let c = &self.combatants[i];
        self.combatants
            .iter()
            .filter(|e| e.side != c.side && e.hp > 0.0)
            .min_by(|a, b| {
                (a.pos - c.pos)
                    .length()
                    .partial_cmp(&(b.pos - c.pos).length())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|e| e.pos)
    }

    fn centroid(&self, side: u8) -> Option<Vec2> {
        let pts: Vec<Vec2> = self.combatants.iter().filter(|c| c.side == side && c.hp > 0.0).map(|c| c.pos).collect();
        if pts.is_empty() {
            return None;
        }
        let sum = pts.iter().fold(Vec2::new(0.0, 0.0), |a, p| a + *p);
        Some(sum * (1.0 / pts.len() as f64))
    }

    fn torpedo_threat_centroid(&self, enemy_side: u8) -> Option<Vec2> {
        let pts: Vec<Vec2> = self
            .combatants
            .iter()
            .filter(|c| c.side == enemy_side && c.hp > 0.0 && Loadout::from_key(&c.stack).offense().0 == DamageType::Torpedo)
            .map(|c| c.pos)
            .collect();
        if pts.is_empty() {
            return None;
        }
        let sum = pts.iter().fold(Vec2::new(0.0, 0.0), |a, p| a + *p);
        Some(sum * (1.0 / pts.len() as f64))
    }
}

/// The role a hull takes at unpack — kind + loadout (+ side anchoring). A
/// published rule, not AI: capitals anchor, PD screens, light fast hulls
/// skirmish, the middle of the line holds the line.
pub fn role_for(kind: ShipKind, loadout: &Loadout, _side: u8) -> Role {
    if loadout.has_pd() {
        return Role::Screen;
    }
    // Capitals at/above the Battleship anchor the line.
    if kind.hull_mass() >= crate::ship::CAPITAL_MASS_THRESHOLD {
        return Role::Anchor;
    }
    match kind {
        ShipKind::Raider => Role::Skirmish,
        ShipKind::Corvette | ShipKind::Destroyer | ShipKind::Cruiser => Role::Line,
        // Civilians hold at the protected rear point (Anchor's civilian branch).
        _ => Role::Anchor,
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    #[test]
    fn probe_raid_1v1() {
        let mut a: LoadoutMap = BTreeMap::new();
        a.entry(ShipKind::Raider).or_default().insert(String::new(), 1);
        let mut d: LoadoutMap = BTreeMap::new();
        d.entry(ShipKind::Convoy).or_default().insert(String::new(), 1);
        let mut st = TacticalState::open(42, 7, &a, &d, &BTreeMap::new(), &BTreeMap::new(), 0, 0.0, Vec2::new(-1.0, 0.0));
        println!("open: {} combatants", st.combatants.len());
        for s in 0..40u64 {
            let out = st.step(true, [SideMods::default(), SideMods::default()]);
            let r = st.combatants.iter().find(|c| c.side == 0);
            let v = st.combatants.iter().find(|c| c.side == 1);
            println!(
                "step {s}: dealt=({:.0},{:.0}) raider={:?} convoy={:?}",
                out.dealt[0], out.dealt[1],
                r.map(|c| (c.pos.x.round(), c.pos.y.round(), c.hp.round())),
                v.map(|c| (c.pos.x.round(), c.pos.y.round(), c.hp.round())),
            );
            if st.combatants.iter().all(|c| c.side == 0) { println!("convoy dead at step {s}"); break; }
        }
    }
}

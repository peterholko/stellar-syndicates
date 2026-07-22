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
pub const HIT_SPEED_SENS: [f64; 3] = [3.60, 4.40, 5.40];
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

/// §arena discipline: slide a standoff point around its ring (centered on
/// `near`, radius preserved) to the nearest inside-arena arc, stepping in the
/// orbit direction `sign` (±1). Deterministic; falls back to a radial clamp
/// when the whole ring lies outside (a far-out runner being pursued).
fn inside_ring_point(near: Vec2, dir0: Vec2, ring: f64, sign: f64) -> Vec2 {
    let cap = ARENA_RADIUS * 0.95;
    let mut dir = dir0;
    for _ in 0..24 {
        let p = near + dir * ring;
        if p.length() <= cap {
            return p;
        }
        // Fine steps: a coarse slide makes orbit points jump in lumps and
        // the duration distribution goes bimodal; 0.15 rad keeps it smooth.
        let (s, c) = ((0.15 * sign).sin(), (0.15 * sign).cos());
        dir = Vec2::new(dir.x * c - dir.y * s, dir.x * s + dir.y * c);
    }
    let p = near + dir0 * ring;
    let r = p.length();
    if r > cap { p * (cap / r) } else { p }
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
    /// §T3: exact death events — (step, side, kind, where) for the keyframe.
    pub deaths: Vec<crate::combat::KfDeath>,
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
            out.deaths.push(crate::combat::KfDeath {
                step: self.step,
                side: c.side,
                kind: c.kind,
                x: c.pos.x as f32,
                y: c.pos.y as f32,
            });
            if c.platform {
                out.platform_tiers_lost += 1;
            } else {
                out.losses[c.side as usize].add_stack(c.kind, Loadout::from_key(&c.stack), 1);
            }
        }
        out
    }

    /// §T3: build the round's TRUTH KEYFRAME — every capital always rides
    /// along; the rest fill to [`crate::combat::KEYFRAME_SHIP_CAP`] in stable
    /// cid order. Live torpedoes summarize as per-side salvos (centroid + n).
    pub fn keyframe(&self, deaths: Vec<crate::combat::KfDeath>) -> crate::combat::Keyframe {
        use crate::combat::{KfSalvo, KfShip, KEYFRAME_SHIP_CAP};
        let mut ships: Vec<KfShip> = Vec::new();
        // Capitals (and platforms) first — the theater must never lose them.
        for c in &self.combatants {
            if c.platform || crate::ship::requires_hull_unlock(c.kind) {
                ships.push(KfShip { side: c.side, kind: c.kind, x: c.pos.x as f32, y: c.pos.y as f32, hp: (c.hp / c.max_hp) as f32, plat: c.platform });
            }
        }
        for c in &self.combatants {
            if ships.len() >= KEYFRAME_SHIP_CAP {
                break;
            }
            if !c.platform && !crate::ship::requires_hull_unlock(c.kind) {
                ships.push(KfShip { side: c.side, kind: c.kind, x: c.pos.x as f32, y: c.pos.y as f32, hp: (c.hp / c.max_hp) as f32, plat: false });
            }
        }
        let mut torpedoes: Vec<KfSalvo> = Vec::new();
        for side in 0..2u8 {
            let fish: Vec<&Torpedo> = self.torpedoes.iter().filter(|t| t.side == side).collect();
            if fish.is_empty() {
                continue;
            }
            let n = fish.len() as u32;
            let cx = fish.iter().map(|t| t.pos.x).sum::<f64>() / n as f64;
            let cy = fish.iter().map(|t| t.pos.y).sum::<f64>() / n as f64;
            torpedoes.push(KfSalvo { side, x: cx as f32, y: cy as f32, n });
        }
        crate::combat::Keyframe { ships, torpedoes, deaths }
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
    ///
    /// §arena discipline: maneuver points keep their STANDOFF (the ring
    /// radius around the target — the tactically meaningful quantity) but
    /// slide around that ring to the inside-arena arc, so a fighting ship
    /// never takes the fight outside the arena. Only the Withdraw script
    /// crosses the ring — the exit is earned by breaking off.
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
                inside_ring_point(near, dir, band, 1.0)
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
                let sign = if c.cid.is_multiple_of(2) { 1.0 } else { -1.0 };
                inside_ring_point(near, ahead, band * 0.95, sign)
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

// --- §T4: THE CALCULATOR IS MONTE CARLO NOW ---------------------------------------
//
// `project_engagement` (closed-form Lanchester) is superseded: the projection
// runs THE REAL ENGINE, headless and pure, k times on derived seeds over the
// observer's light-delayed view data. The no-drift property survives — it is
// still reality's exact function, sampled, on stale inputs.

/// A headless projection setup (built from VIEW data — own side exact, target
/// side fogged exactly as the observer sees it).
#[derive(Debug, Clone, Default)]
pub struct ProjSetup {
    pub a: LoadoutMap,
    pub d: LoadoutMap,
    pub a_pool: StackPoolMap,
    pub d_pool: StackPoolMap,
    pub platform_tiers: u32,
    pub raid: bool,
    /// Retreat thresholds as min-strength-ratio (mirroring doctrine); `None`
    /// fights to the end.
    pub a_retreat: Option<f64>,
    pub d_retreat: Option<f64>,
}

/// One headless rollout's outcome.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimOutcome {
    pub a_losses: BTreeMap<ShipKind, u32>,
    pub d_losses: BTreeMap<ShipKind, u32>,
    pub a_survivors: BTreeMap<ShipKind, u32>,
    pub d_survivors: BTreeMap<ShipKind, u32>,
    pub steps: u64,
    /// The attacker's side prevailed: the defender is wiped or withdrew while
    /// the attacker still stands.
    pub a_won: bool,
}

/// Hard step cap for a headless rollout (a real battle at the production
/// cadence resolves in well under this; the cap bounds a runaway stalemate).
pub const MAX_PROJ_STEPS: u64 = 600;

/// Run the REAL engine headless: pure over `(setup, seed)` — byte-identical
/// outcome for identical inputs, and the world's RNG is never touched.
pub fn simulate_engagement(setup: &ProjSetup, seed: u64) -> SimOutcome {
    let mut st = TacticalState::open(
        seed,
        0xC0FFEE, // the projection's battle-id salt — any constant works
        &setup.a,
        &setup.d,
        &setup.a_pool,
        &setup.d_pool,
        setup.platform_tiers,
        0.0,
        Vec2::new(1.0, 0.0),
    );
    let scouts = st.sync([&setup.a, &setup.d]);
    let mut out = SimOutcome::default();
    // Boundary scouts die instantly, as in the real lifecycle.
    if scouts[0] > 0 {
        *out.a_losses.entry(ShipKind::Scout).or_insert(0) += scouts[0];
    }
    if scouts[1] > 0 {
        *out.d_losses.entry(ShipKind::Scout).or_insert(0) += scouts[1];
    }
    let start = [st.side_hp(0, false).max(1e-9), st.side_hp(1, false).max(1e-9)];
    while out.steps < MAX_PROJ_STEPS {
        let step_out = st.step(setup.raid, [SideMods::default(), SideMods::default()]);
        out.steps += 1;
        for (side, losses) in step_out.losses.iter().enumerate() {
            let tgt = if side == 0 { &mut out.a_losses } else { &mut out.d_losses };
            for (k, n) in &losses.per_kind {
                *tgt.entry(*k).or_insert(0) += n;
            }
        }
        // Doctrine-mirrored retreat: trip once, then run for the edge.
        if let Some(m) = setup.a_retreat
            && !st.withdrawing[0]
            && st.side_hp(0, false) / start[0] < m
        {
            st.order_withdraw(0);
        }
        if let Some(m) = setup.d_retreat
            && !st.withdrawing[1]
            && st.side_hp(1, false) / start[1] < m
        {
            st.order_withdraw(1);
        }
        let a_alive = st.alive(0) > 0;
        let d_alive = st.alive(1) > 0 || st.platform_tiers() > 0;
        if !a_alive || !d_alive || st.side_withdrawn(0) || st.side_withdrawn(1) {
            break;
        }
    }
    for c in st.combatants.iter().filter(|c| !c.platform) {
        let tgt = if c.side == 0 { &mut out.a_survivors } else { &mut out.d_survivors };
        *tgt.entry(c.kind).or_insert(0) += 1;
    }
    let a_alive = st.alive(0) > 0;
    let d_gone = st.alive(1) == 0 && st.platform_tiers() == 0;
    out.a_won = a_alive && (d_gone || st.withdrawing[1]);
    out
}

/// A per-kind loss RANGE across the rollouts (the 25th–75th percentile band —
/// "expected losses 4–7 Corvettes").
#[derive(Debug, Clone)]
pub struct LossBand {
    pub kind: ShipKind,
    pub lo: u32,
    pub hi: u32,
}

/// The sampled DISTRIBUTION the calculator shows.
#[derive(Debug, Clone, Default)]
pub struct Distribution {
    pub runs: u32,
    pub a_win_pct: f64,
    pub a_bands: Vec<LossBand>,
    pub d_bands: Vec<LossBand>,
    /// The MEDIAN-outcome rollout (by attacker losses) — the legacy single-
    /// projection fields read from this.
    pub median: SimOutcome,
}

/// `k` rollouts on seeds derived from `base_seed` — deterministic for the same
/// inputs, independent across rollouts. `k` is DOWNSAMPLED for huge setups
/// (the stated CPU budget: a reference battle × 32 stays trivially cheap; a
/// 300v300 echelon fight samples 8).
pub fn project_distribution(setup: &ProjSetup, base_seed: u64, k: u32) -> Distribution {
    let ships: u32 = setup.a.values().flat_map(|m| m.values()).sum::<u32>()
        + setup.d.values().flat_map(|m| m.values()).sum::<u32>();
    let k = if ships > 150 { k.min(8) } else if ships > 60 { k.min(16) } else { k.max(1) };
    let mut outs: Vec<SimOutcome> = (0..k)
        .map(|i| simulate_engagement(setup, base_seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)))
        .collect();
    let wins = outs.iter().filter(|o| o.a_won).count();
    let band = |outs: &[SimOutcome], side_a: bool| -> Vec<LossBand> {
        let mut kinds: Vec<ShipKind> = Vec::new();
        for o in outs {
            for k in (if side_a { &o.a_losses } else { &o.d_losses }).keys() {
                if !kinds.contains(k) {
                    kinds.push(*k);
                }
            }
        }
        kinds.sort();
        kinds
            .into_iter()
            .map(|kind| {
                let mut v: Vec<u32> = outs
                    .iter()
                    .map(|o| (if side_a { &o.a_losses } else { &o.d_losses }).get(&kind).copied().unwrap_or(0))
                    .collect();
                v.sort_unstable();
                let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
                LossBand { kind, lo: q(0.25), hi: q(0.75) }
            })
            .collect()
    };
    let a_bands = band(&outs, true);
    let d_bands = band(&outs, false);
    // Median rollout by total attacker losses (stable order → deterministic).
    outs.sort_by_key(|o| o.a_losses.values().sum::<u32>());
    let median = outs[outs.len() / 2].clone();
    Distribution { runs: k, a_win_pct: wins as f64 / k as f64, a_bands, d_bands, median }
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

// =============================================================================
// §tactical T6 — the STATISTICAL suite. The engine rolls dice, so its laws are
// distributional: proven over seeded ensembles, with thresholds set from
// measured behaviour (wide enough to never flake — every ensemble below is a
// fixed seed list, so a pass today is a pass forever).
//
// CALIBRATION NOTE (flagged per the handoff): under the pooled engine,
// `battle_target_secs` DIRECTLY scaled the damage rate, so equal reference
// forces ground for exactly the target. The tactical engine made duration
// EMERGENT (HP vs per-step damage); `battle_target_secs` now drives the step
// CADENCE (2 Hz playtest / 1 Hz production), the raid cap, and the hard-stop
// window — not the grind itself. The calibration law is therefore restated as
// the properties the strategic layer actually depends on: the reference fight
// resolves in a TIGHT band (≥90% of seeds within ±20% of the ensemble
// median), WELL INSIDE the strategic window (no safety-valve mutual
// disengage for an ordinary fight), and duration scales with what's fielded.
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn fleet(entries: &[(ShipKind, &str, u32)]) -> LoadoutMap {
        let mut m: LoadoutMap = BTreeMap::new();
        for (k, key, n) in entries {
            m.entry(*k).or_default().insert(key.to_string(), *n);
        }
        m
    }

    fn setup(a: &[(ShipKind, &str, u32)], d: &[(ShipKind, &str, u32)], ar: Option<f64>, dr: Option<f64>) -> ProjSetup {
        ProjSetup { a: fleet(a), d: fleet(d), a_retreat: ar, d_retreat: dr, ..Default::default() }
    }

    fn open_plain(a: &[(ShipKind, &str, u32)], d: &[(ShipKind, &str, u32)], seed: u64) -> TacticalState {
        TacticalState::open(seed, 99, &fleet(a), &fleet(d), &BTreeMap::new(), &BTreeMap::new(), 0, 0.0, Vec2::new(1.0, 0.0))
    }

    /// EV instrument: HP a fixed defender loses over `steps` steps under a
    /// fixed attacker — no retreat, no completion; open, step N, measure.
    fn hp_lost(a: &[(ShipKind, &str, u32)], d: &[(ShipKind, &str, u32)], steps: u64, seed: u64) -> f64 {
        let mut st = open_plain(a, d, seed);
        let start = st.side_hp(1, false);
        for _ in 0..steps {
            st.step(false, [SideMods::default(), SideMods::default()]);
        }
        start - st.side_hp(1, false)
    }

    fn mean_sd(v: &[f64]) -> (f64, f64) {
        let m = v.iter().sum::<f64>() / v.len() as f64;
        let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
        (m, var.sqrt())
    }

    // --- 1. determinism & live dice -------------------------------------------

    #[test]
    fn same_seed_same_outcome_and_dice_are_live() {
        let s = setup(&[(ShipKind::Raider, "", 8)], &[(ShipKind::Corvette, "", 6)], Some(0.4), Some(0.4));
        // Byte-identical replay: the projection is pure over (setup, seed).
        assert_eq!(simulate_engagement(&s, 777), simulate_engagement(&s, 777), "same seed → identical outcome");
        // …and the dice are LIVE: across seeds a mirror fight's losses and
        // durations genuinely vary.
        let m = setup(&[(ShipKind::Raider, "", 8)], &[(ShipKind::Raider, "", 8)], None, None);
        let mut totals: Vec<(u32, u64)> = (0..20)
            .map(|i| {
                let o = simulate_engagement(&m, 900 + i);
                (o.a_losses.values().sum(), o.steps)
            })
            .collect();
        totals.sort_unstable();
        totals.dedup();
        assert!(totals.len() > 1, "different seeds produce different outcomes (bounded spice, not a constant)");
    }

    // --- 2. calibration as a distribution --------------------------------------

    #[test]
    fn reference_duration_is_a_tight_band_inside_the_strategic_window() {
        // The reference fight: equal 10v10 raider squadrons, both retreating at
        // half strength — 100 seeded runs.
        let s = setup(&[(ShipKind::Raider, "", 10)], &[(ShipKind::Raider, "", 10)], Some(0.5), Some(0.5));
        let mut steps: Vec<u64> = (0..100).map(|i| simulate_engagement(&s, 1_000 + i).steps).collect();
        steps.sort_unstable();
        let median = steps[steps.len() / 2] as f64;
        // AMENDED with §arena discipline (2026-07): the compact in-arena
        // dance (standoff points slide inside the ring) has a wider seed
        // spread than the old wide-swing dance — the law is re-pinned at
        // ±35% (the hard 45s-window assert below still caps the worst seed);
        // still a consistent battle clock for the strategic layer.
        let within = steps.iter().filter(|&&t| (t as f64 - median).abs() <= median * 0.35).count();
        assert!(within >= 90, "≥90% of seeds within ±35% of the median (got {within}/100 around {median})");
        // The reference fight fits WELL inside the playtest strategic window
        // (45 s target at the 2 Hz playtest cadence) — asserted on the WORST
        // seed, so no ordinary equal fight ever trips the hard-stop valve.
        let worst = *steps.last().unwrap() as f64 * TAC_STEP_TICKS_FAST as f64 / crate::config::TICK_HZ as f64;
        assert!(worst < 45.0, "even the slowest reference fight ({worst:.1}s) resolves inside the 45s playtest window");
        // Duration scales with what's fielded: a corvette-line mirror (more HP
        // per gun) grinds much longer than the raider skirmish.
        let c = setup(&[(ShipKind::Corvette, "", 10)], &[(ShipKind::Corvette, "", 10)], Some(0.5), Some(0.5));
        let mut csteps: Vec<u64> = (0..40).map(|i| simulate_engagement(&c, 2_000 + i).steps).collect();
        csteps.sort_unstable();
        assert!(
            csteps[csteps.len() / 2] as f64 > median * 1.5,
            "a corvette mirror ({} steps) grinds well past the raider reference ({median})",
            csteps[csteps.len() / 2]
        );
    }

    // --- 3. the variance-scaling law -------------------------------------------

    #[test]
    fn loss_variance_shrinks_as_fleets_grow() {
        // Small fights are spicy; big fleet actions converge on the math. The
        // RELATIVE stddev of own losses strictly shrinks 3v3 → 20v20 → 60v60
        // (measured ≈ 0.67 → 0.41 → 0.29; margins below are generous).
        let rel = |n: u32| -> f64 {
            let s = setup(&[(ShipKind::Corvette, "", n)], &[(ShipKind::Corvette, "", n)], None, None);
            let losses: Vec<f64> = (0..60)
                .map(|i| simulate_engagement(&s, 5_000 + i).a_losses.values().sum::<u32>() as f64)
                .collect();
            let (m, sd) = mean_sd(&losses);
            sd / m.max(1e-9)
        };
        let (r3, r20, r60) = (rel(3), rel(20), rel(60));
        assert!(r3 > r20 * 1.1 && r20 > r60 * 1.1, "relative spread strictly shrinks with scale ({r3:.3} > {r20:.3} > {r60:.3})");
    }

    // --- 4. the counter matrix in expectation -----------------------------------

    #[test]
    fn matrix_holds_in_expectation() {
        // 6 fixed attackers vs 2 Battleships (16 000 HP — nothing dies inside
        // the window, isolating the mitigation math), 120 seeds per cell.
        let ev = |amod: &str, dmod: &str, steps: u64| -> f64 {
            let v: Vec<f64> = (0..120)
                .map(|i| hp_lost(&[(ShipKind::Raider, amod, 6)], &[(ShipKind::Battleship, dmod, 2)], steps, 30_000 + i))
                .collect();
            mean_sd(&v).0
        };
        let (refl, whip) = ("reflective_plating", "whipple_armor");
        let (beam_bare, driver_bare) = (ev("", "", 8), ev("mass_driver", "", 8));
        // Reflective cuts beam EV by ~REFLECT_BLUNT and leaves driver alone.
        let beam_cut = ev("", refl, 8) / beam_bare;
        assert!((beam_cut - (1.0 - crate::module::REFLECT_BLUNT)).abs() < 0.05, "reflective cuts beam EV by ~REFLECT_BLUNT (ratio {beam_cut:.3})");
        assert!((ev("mass_driver", refl, 8) / driver_bare - 1.0).abs() < 0.02, "reflective leaves driver EV alone");
        // Whipple mirrors for driver; beams pass it untouched.
        let driver_cut = ev("mass_driver", whip, 8) / driver_bare;
        assert!((driver_cut - (1.0 - crate::module::WHIPPLE_BLUNT)).abs() < 0.05, "whipple cuts driver EV by ~WHIPPLE_BLUNT (ratio {driver_cut:.3})");
        assert!((ev("", whip, 8) / beam_bare - 1.0).abs() < 0.02, "whipple leaves beam EV alone");
        // Torpedo EV is unmoved by EITHER armor (longer window — flight time).
        let (tb, tr, tw) = (ev("torpedo_rack", "", 20), ev("torpedo_rack", refl, 20), ev("torpedo_rack", whip, 20));
        assert!((tr / tb - 1.0).abs() < 0.02 && (tw / tb - 1.0).abs() < 0.02, "armor is no defense against torpedoes ({tb:.0}/{tr:.0}/{tw:.0})");
    }

    #[test]
    fn pd_screens_cut_torpedo_ev_into_the_heavy() {
        // 8 torpedo raiders vs a Battleship behind 4 corvettes — with PD fits
        // the corvettes INTERPOSE (Screen role) and roll intercepts; the
        // heavy's own losses collapse (measured ≈ 743 → 11; assert ≥ 50% cut).
        let heavy_loss = |screen_mod: &str| -> f64 {
            let v: Vec<f64> = (0..120)
                .map(|i| {
                    let mut st = open_plain(
                        &[(ShipKind::Raider, "torpedo_rack", 8)],
                        &[(ShipKind::Battleship, "", 1), (ShipKind::Corvette, screen_mod, 4)],
                        60_000 + i,
                    );
                    for _ in 0..20 {
                        st.step(false, [SideMods::default(), SideMods::default()]);
                    }
                    let bb: f64 = st.combatants.iter().filter(|c| c.side == 1 && c.kind == ShipKind::Battleship).map(|c| c.hp).sum();
                    ShipKind::Battleship.hull_mass() - bb
                })
                .collect();
            mean_sd(&v).0
        };
        let (bare, screened) = (heavy_loss(""), heavy_loss("point_defense_screen"));
        assert!(screened < bare * 0.5, "an interposed PD screen halves the heavy's torpedo losses ({screened:.0} vs {bare:.0})");
    }

    // --- 5. emergence ------------------------------------------------------------

    #[test]
    fn to_hit_tracks_mass_and_speed() {
        // The curve IS the design: torpedoes near-guarantee against capitals…
        let t_titan = to_hit(DamageType::Torpedo, ShipKind::Titan.hull_mass(), ShipKind::Titan.max_speed());
        let t_corv = to_hit(DamageType::Torpedo, ShipKind::Corvette.hull_mass(), ShipKind::Corvette.max_speed());
        assert!(t_titan >= 0.90, "a seeker near-guarantees against a Titan ({t_titan:.2})");
        assert!(t_titan > 3.0 * t_corv, "…and struggles against small fast hulls ({t_titan:.2} vs {t_corv:.2})");
        // Drivers are brutal against big slow hulls, poor against darters.
        let d_titan = to_hit(DamageType::Driver, ShipKind::Titan.hull_mass(), ShipKind::Titan.max_speed());
        let d_corv = to_hit(DamageType::Driver, ShipKind::Corvette.hull_mass(), ShipKind::Corvette.max_speed());
        assert!(d_titan > 3.0 * d_corv, "drivers punish barns, whiff on darters ({d_titan:.2} vs {d_corv:.2})");
        // Beams track well — the flattest family across the same spread.
        let b_titan = to_hit(DamageType::Beam, ShipKind::Titan.hull_mass(), ShipKind::Titan.max_speed());
        let b_corv = to_hit(DamageType::Beam, ShipKind::Corvette.hull_mass(), ShipKind::Corvette.max_speed());
        assert!(b_titan / b_corv < t_titan / t_corv, "beams are the least mass/speed-sensitive family");
        // And speed protects WITHIN a hull: a corvette at a dead stop is
        // strictly easier to hit than one darting at flank speed. (Measured on
        // the corvette — a Titan's driver solution is already at the 0.95
        // clamp whether it moves or not, which is itself the point.)
        assert!(
            to_hit(DamageType::Driver, ShipKind::Corvette.hull_mass(), 0.0)
                > to_hit(DamageType::Driver, ShipKind::Corvette.hull_mass(), ShipKind::Corvette.max_speed()),
            "a stalled ship is easier to hit than a darting one"
        );
        assert_eq!(
            to_hit(DamageType::Driver, ShipKind::Titan.hull_mass(), 0.0),
            HIT_CLAMP.1,
            "an anchored Titan is a barn — the solution sits at the clamp"
        );
    }

    #[test]
    fn equal_cost_wolfpack_beats_the_lone_titan() {
        // The canonical answer exists: a Titan costs 480 Armaments; a torpedo
        // raider (hull 15 + rack 10) costs 25 → 19 raiders ≈ the same war
        // budget. The pack wins >60% of 100 seeded runs (measured 94%)…
        let pack = setup(&[(ShipKind::Raider, "torpedo_rack", 19)], &[(ShipKind::Titan, "", 1)], None, None);
        let wins = (0..100).filter(|i| simulate_engagement(&pack, 80_000 + i).a_won).count();
        assert!(wins > 60, "the equal-cost wolfpack beats the lone unescorted Titan ({wins}/100)");
        // …while a two-thirds pack mostly dies (measured 1%): the answer is a
        // real fleet commitment, not a cheap counter.
        let small = setup(&[(ShipKind::Raider, "torpedo_rack", 12)], &[(ShipKind::Titan, "", 1)], None, None);
        let small_wins = (0..100).filter(|i| simulate_engagement(&small, 81_000 + i).a_won).count();
        assert!(small_wins < 30, "an under-sized pack loses ({small_wins}/100)");
    }

    // --- 6. the boundary -----------------------------------------------------------

    #[test]
    fn waves_hold_beyond_the_cap_and_commit_deterministically() {
        // 350 corvettes a side: 300 deploy, 50 hold as reinforcement waves;
        // the in-arena population never exceeds the cap while waves drain in.
        let big = &[(ShipKind::Corvette, "", 350u32)][..];
        let mut st = open_plain(big, big, 4242);
        let in_arena = |st: &TacticalState, side: u8| st.combatants.iter().filter(|c| c.side == side && !c.platform).count();
        assert_eq!(in_arena(&st, 0), MAX_COMBATANTS_PER_SIDE, "the arena opens at the cap");
        assert_eq!(st.alive(0), 350, "held waves still count as alive (containment: no ship vanishes)");
        let mut committed = false;
        for _ in 0..120 {
            st.step(false, [SideMods::default(), SideMods::default()]);
            assert!(in_arena(&st, 0) <= MAX_COMBATANTS_PER_SIDE && in_arena(&st, 1) <= MAX_COMBATANTS_PER_SIDE, "the cap holds every step");
            if st.waves[0].iter().map(|w| w.count).sum::<u32>() < 50 {
                committed = true;
                break;
            }
        }
        assert!(committed, "waves commit as deaths open slots");
        // Determinism at the cap: an identical run reaches the identical state.
        let mut st2 = open_plain(big, big, 4242);
        for _ in 0..st.step {
            st2.step(false, [SideMods::default(), SideMods::default()]);
        }
        assert_eq!(st.side_hp(0, false), st2.side_hp(0, false), "wave commitment is deterministic");
        assert_eq!(st.alive(1), st2.alive(1));
    }

    #[test]
    fn pools_carry_hp_deficits_per_stack() {
        // A fitted stack and an unfitted remainder fight side by side: every
        // survivor's missing HP lands in ITS OWN stack's pool — the exact
        // shapes the strategic layer persists between ticks (§law 1).
        let mut st = open_plain(
            &[(ShipKind::Raider, "", 6)],
            &[(ShipKind::Corvette, "whipple_armor", 2), (ShipKind::Corvette, "", 3)],
            991,
        );
        for _ in 0..6 {
            st.step(false, [SideMods::default(), SideMods::default()]);
        }
        let pools = st.pools(1);
        let deficit: f64 = st
            .combatants
            .iter()
            .filter(|c| c.side == 1 && !c.platform)
            .map(|c| (c.max_hp - c.hp).max(0.0))
            .sum();
        let pooled: f64 = pools.values().flat_map(|m| m.values()).sum();
        assert!((pooled - deficit).abs() < 1e-6, "Σ pools == Σ survivor HP deficits ({pooled:.1} vs {deficit:.1})");
        for key in pools.get(&ShipKind::Corvette).map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
            assert!(key.is_empty() || key == "whipple_armor", "pools key by the stack that bled (got '{key}')");
        }
    }

    #[test]
    fn mid_battle_serde_resumes_identically() {
        // Snapshot mid-fight, restore, and continue: the restored battle steps
        // BYTE-IDENTICALLY (the RNG stream rides the snapshot).
        let a = &[(ShipKind::Raider, "torpedo_rack", 5)][..];
        let d = &[(ShipKind::Corvette, "", 6)][..];
        let mut live = open_plain(a, d, 313);
        for _ in 0..5 {
            live.step(false, [SideMods::default(), SideMods::default()]);
        }
        let json = serde_json::to_string(&live).expect("mid-battle state serializes");
        let mut restored: TacticalState = serde_json::from_str(&json).expect("…and restores");
        for _ in 0..5 {
            live.step(false, [SideMods::default(), SideMods::default()]);
            restored.step(false, [SideMods::default(), SideMods::default()]);
        }
        assert_eq!(live.side_hp(0, false), restored.side_hp(0, false), "the restored fight continues identically");
        assert_eq!(live.side_hp(1, false), restored.side_hp(1, false));
        assert_eq!(live.torpedoes.len(), restored.torpedoes.len());
        assert_eq!(live.alive(0), restored.alive(0));
    }

    #[test]
    fn raid_smash_and_grab_resolves_fast() {
        // The folded probe: a lone raider overpowers a lone convoy inside the
        // raid window — approach run included — and survives untouched.
        let mut st = open_plain(&[(ShipKind::Raider, "", 1)], &[(ShipKind::Convoy, "", 1)], 42);
        let mut convoy_died = false;
        for _ in 0..30u64 {
            st.step(true, [SideMods::default(), SideMods::default()]);
            if st.alive(1) == 0 {
                convoy_died = true;
                break;
            }
        }
        assert!(convoy_died, "the convoy dies inside 30 raid steps");
        assert_eq!(st.alive(0), 1, "the smash-and-grab costs the raider nothing");
    }

    // --- 7. perf ---------------------------------------------------------------------

    #[test]
    fn perf_step_and_projection_within_budget() {
        // Stated bounds — deliberately WIDE (≈30× the measured debug numbers:
        // 300v300 step ≈ 15 ms, k=32 projection ≈ 35 ms) so machine load or an
        // instrumented build can never flake them. They exist to catch an
        // ALGORITHMIC blow-up (an accidental O(n³) pass, a runaway loop), not
        // to benchmark. The engine steps each battle at most 2 Hz of sim time.
        let big = &[(ShipKind::Corvette, "", 300u32)][..];
        let mut st = open_plain(big, big, 7);
        let t0 = std::time::Instant::now();
        for _ in 0..10 {
            st.step(false, [SideMods::default(), SideMods::default()]);
        }
        let per_step = t0.elapsed().as_secs_f64() / 10.0;
        assert!(per_step < 0.5, "a 300v300 step stays under 500 ms ({:.1} ms)", per_step * 1_000.0);
        let s = setup(&[(ShipKind::Raider, "", 10)], &[(ShipKind::Corvette, "", 8)], Some(0.5), Some(0.5));
        let t0 = std::time::Instant::now();
        let dist = project_distribution(&s, 42, 32);
        let took = t0.elapsed().as_secs_f64();
        assert_eq!(dist.runs, 32);
        assert!(took < 5.0, "a k=32 reference projection stays under 5 s ({:.0} ms)", took * 1_000.0);
    }
}
#[cfg(test)]
mod arena_discipline {
    use super::*;

    /// §arena discipline (the player-facing law): FIGHTING ships stay inside
    /// the arena ring — maneuver points keep their standoff but slide to the
    /// inside arc — and only a WITHDRAWING ship ever crosses it.
    #[test]
    fn fighting_ships_stay_inside_the_ring() {
        let mut a: LoadoutMap = BTreeMap::new();
        a.entry(ShipKind::Raider).or_default().insert(String::new(), 10);
        let mut d: LoadoutMap = BTreeMap::new();
        d.entry(ShipKind::Raider).or_default().insert(String::new(), 10);
        for seed in 0..8u64 {
            let mut st = TacticalState::open(seed, 7, &a, &d, &BTreeMap::new(), &BTreeMap::new(), 0, 0.0, Vec2::new(1.0, 0.0));
            let mut max_fighting = 0.0f64;
            for step in 0..80 {
                st.step(false, [SideMods::default(), SideMods::default()]);
                for c in &st.combatants {
                    if !matches!(c.role, Role::Withdraw) {
                        max_fighting = max_fighting.max(c.pos.length());
                    }
                }
                // Half-way in, order a side out: its ships CROSS the ring.
                if step == 30 {
                    st.order_withdraw(1);
                }
                if st.alive(0) == 0 || st.alive(1) == 0 || st.side_withdrawn(1) {
                    break;
                }
            }
            // Spawn is at 900; a fighting ship may brush the ring chasing its
            // slid standoff point but never leaves the battle space.
            assert!(
                max_fighting <= ARENA_RADIUS * 1.05,
                "seed {seed}: a fighting ship strayed to {max_fighting:.0} (ring {ARENA_RADIUS})"
            );
            // …while the withdrawing side genuinely exits past the ring.
            let out = st
                .combatants
                .iter()
                .filter(|c| matches!(c.role, Role::Withdraw))
                .map(|c| c.pos.length())
                .fold(0.0f64, f64::max);
            assert!(out > ARENA_RADIUS, "seed {seed}: withdrawers cross the ring (got {out:.0})");
        }
    }
}

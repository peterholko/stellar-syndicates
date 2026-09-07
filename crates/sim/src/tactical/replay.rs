//! SERVER-PRIVATE deterministic archive. Store initial state + external inputs,
//! not shots. The viewer protocol must never serialize this module's types:
//! a seed/checkpoint would let a modified client step beyond its arrived light.
//!
//! Events are ordered at their actual world tick, including off-step roster
//! changes/withdrawals. `before_step` preserves their ordering relative to the
//! kernel. Research/captain/raid controls are recorded only when they change;
//! their source is never looked up in today's world during reconstruction.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::v1::{RosterDelta, SideMods, TacticalState};
use crate::combat::{KEYFRAME_DEATH_CAP, Keyframe};

pub const RULES_VERSION: u32 = 3;
/// A cold seek does at most this many extra tactical steps before its slice.
pub const CHECKPOINT_STEPS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Controls {
    raid: bool,
    mods: [SideMods; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Input {
    Roster(Vec<RosterDelta>),
    Controls(Controls),
    Withdraw(u8),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TimedInput {
    tick: u64,
    before_step: usize,
    input: Input,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Step {
    tick: u64,
    round: usize,
    checksum: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Checkpoint {
    through: usize,
    next_input: usize,
    controls: Option<Controls>,
    state: TacticalState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleReplay {
    version: u32,
    initial: TacticalState,
    initial_checksum: u64,
    inputs: Vec<TimedInput>,
    steps: Vec<Step>,
    checkpoints: Vec<Checkpoint>,
    controls: Option<Controls>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    UnsupportedVersion(u32),
    Diverged { step: usize },
    InvalidArchive,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported battle rules version {v}"),
            Self::Diverged { step } => write!(f, "battle replay diverged at step {step}"),
            Self::InvalidArchive => write!(f, "invalid battle replay input ordering"),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Frozen v1 fingerprint of the complete kernel state, including RNG, held
/// waves, cooldowns and roles. Hash numeric bits, not JSON text: serializer
/// upgrades must not invalidate history. This is not a security signature.
pub fn state_checksum(state: &TacticalState) -> u64 {
    struct Hash(u64);
    impl Hash {
        fn bytes(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 ^= u64::from(*byte);
                self.0 = self.0.wrapping_mul(0x100_0000_01b3);
            }
        }
        fn u(&mut self, value: u64) {
            self.bytes(&value.to_le_bytes());
        }
        fn f(&mut self, value: f64) {
            self.u(value.to_bits());
        }
        fn pos(&mut self, value: crate::Vec2) {
            self.f(value.x);
            self.f(value.y);
        }
        fn string(&mut self, value: &str) {
            self.u(value.len() as u64);
            self.bytes(value.as_bytes());
        }
        fn kind(&mut self, kind: crate::ShipKind) {
            use crate::ShipKind::*;
            // Explicit tags do not depend on future catalog enum insertion.
            self.u(match kind {
                Builder => 0,
                Raider => 1,
                Corvette => 2,
                Convoy => 3,
                Colony => 4,
                Scout => 5,
                Destroyer => 6,
                Cruiser => 7,
                Battleship => 8,
                Dreadnought => 9,
                Titan => 10,
                Transport => 11,
                Freighter => 12,
            });
        }
    }
    let mut h = Hash(0xcbf2_9ce4_8422_2325);
    // Preserve historical v1/v2 fingerprints. New archives also cover the
    // private maneuver seed: changing future steering must invalidate a checkpoint.
    if state.rules_version() > 1 { h.u(u64::from(state.rules_version())); }
    if let super::Rules::V3 { maneuver_seed } = state.rules { h.u(maneuver_seed); }
    h.u(state.rng.state_bits());
    h.u(state.step);
    h.u(u64::from(state.next_cid));
    h.u(state.combatants.len() as u64);
    for c in &state.combatants {
        h.u(u64::from(c.cid));
        h.u(u64::from(c.side));
        h.kind(c.kind);
        h.string(&c.stack);
        h.f(c.hp);
        h.f(c.max_hp);
        h.pos(c.pos);
        h.pos(c.vel);
        h.u(u64::from(c.cooldowns.beam));
        h.u(u64::from(c.cooldowns.driver));
        h.u(u64::from(c.cooldowns.torpedo));
        h.u(c.role as u64);
        h.u(u64::from(c.platform));
        h.u(u64::from(c.origin.is_some()));
        if let Some((fleet, ship)) = c.origin {
            h.u(fleet.0);
            h.u(u64::from(ship));
        }
    }
    h.u(state.torpedoes.len() as u64);
    for t in &state.torpedoes {
        h.u(u64::from(t.side));
        h.pos(t.pos);
        h.u(u64::from(t.target));
        h.f(t.dmg);
    }
    for waves in &state.waves {
        h.u(waves.len() as u64);
        for w in waves {
            h.u(w.fleet.0);
            h.u(u64::from(w.ship.id));
            h.kind(w.ship.kind);
            h.string(&w.ship.stack_key());
            h.f(w.ship.hp);
        }
    }
    for hp in state.start_hp {
        h.f(hp);
    }
    h.pos(state.bearing);
    for withdrawing in state.withdrawing {
        h.u(u64::from(withdrawing));
    }
    h.0
}

impl BattleReplay {
    pub(crate) fn new(initial: &TacticalState) -> Self {
        Self {
            version: initial.rules_version(),
            initial: initial.clone(),
            initial_checksum: state_checksum(initial),
            inputs: Vec::new(),
            steps: Vec::new(),
            checkpoints: Vec::new(),
            controls: None,
        }
    }

    fn input(&mut self, tick: u64, input: Input) {
        self.inputs.push(TimedInput {
            tick,
            before_step: self.steps.len(),
            input,
        });
    }

    pub(crate) fn roster(&mut self, tick: u64, changes: Vec<RosterDelta>) {
        if !changes.is_empty() {
            self.input(tick, Input::Roster(changes));
        }
    }

    pub(crate) fn controls(&mut self, tick: u64, raid: bool, mods: [SideMods; 2]) {
        let controls = Controls { raid, mods };
        if self.controls != Some(controls) {
            self.input(tick, Input::Controls(controls));
            self.controls = Some(controls);
        }
    }

    pub(crate) fn withdraw(&mut self, tick: u64, side: u8) {
        // Do not deduplicate by `withdrawing`: repeated orders also reset the
        // roles of newly joined hulls and discard newly held waves.
        self.input(tick, Input::Withdraw(side));
    }

    pub(crate) fn stepped(&mut self, tick: u64, round: usize, state: &TacticalState) {
        self.steps.push(Step {
            tick,
            round,
            checksum: state_checksum(state),
        });
        if self.steps.len().is_multiple_of(CHECKPOINT_STEPS) {
            self.checkpoints.push(Checkpoint {
                through: self.steps.len(),
                next_input: self.inputs.len(),
                controls: self.controls,
                state: state.clone(),
            });
        }
    }

    pub fn records_round(&self, round: usize) -> bool {
        self.steps
            .binary_search_by_key(&round, |step| step.round)
            .is_ok()
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    fn validate_version(&self) -> Result<(), ReplayError> {
        // Only explicit versions are supported. The state selects the same
        // frozen movement rules in live play and reconstruction, never today's
        // default. A v1 archive (including missing state tags) remains v1.
        if !(1..=RULES_VERSION).contains(&self.version) {
            return Err(ReplayError::UnsupportedVersion(self.version));
        }
        if self.version != self.initial.rules_version() { return Err(ReplayError::InvalidArchive); }
        Ok(())
    }

    fn apply(
        input: &Input,
        state: &mut TacticalState,
        controls: &mut Option<Controls>,
    ) -> Result<(), ReplayError> {
        match input {
            Input::Roster(changes) => {
                if changes.iter().any(|change| change.side > 1) {
                    return Err(ReplayError::InvalidArchive);
                }
                state.apply_roster_delta(changes);
            }
            Input::Controls(next) => *controls = Some(*next),
            Input::Withdraw(side) => {
                if *side > 1 {
                    return Err(ReplayError::InvalidArchive);
                }
                state.order_withdraw(*side);
            }
        }
        Ok(())
    }

    fn run(
        &self,
        checkpoint: Option<&Checkpoint>,
        end: usize,
        mut on_step: impl FnMut(usize, &TacticalState, super::v1::StepOutcome),
    ) -> Result<(TacticalState, usize, Option<Controls>), ReplayError> {
        self.validate_version()?;
        let (mut state, mut next_input, mut controls, start, expected) = match checkpoint {
            Some(cp) => {
                let expected = self
                    .steps
                    .get(
                        cp.through
                            .checked_sub(1)
                            .ok_or(ReplayError::InvalidArchive)?,
                    )
                    .ok_or(ReplayError::InvalidArchive)?
                    .checksum;
                (
                    cp.state.clone(),
                    cp.next_input,
                    cp.controls,
                    cp.through,
                    expected,
                )
            }
            None => (self.initial.clone(), 0, None, 0, self.initial_checksum),
        };
        if start > end || end > self.steps.len() || next_input > self.inputs.len() {
            return Err(ReplayError::InvalidArchive);
        }
        if state_checksum(&state) != expected {
            return Err(ReplayError::Diverged { step: start });
        }
        for index in start..end {
            let step = &self.steps[index];
            while let Some(event) = self.inputs.get(next_input) {
                if event.before_step > index {
                    break;
                }
                if event.before_step != index || event.tick > step.tick {
                    return Err(ReplayError::InvalidArchive);
                }
                Self::apply(&event.input, &mut state, &mut controls)?;
                next_input += 1;
            }
            let controls = controls.ok_or(ReplayError::InvalidArchive)?;
            let outcome = state.step(controls.raid, controls.mods);
            if state_checksum(&state) != step.checksum {
                return Err(ReplayError::Diverged { step: index + 1 });
            }
            on_step(index, &state, outcome);
        }
        Ok((state, next_input, controls))
    }

    /// Audit the whole recording, checking every tactical step. Also apply any
    /// off-step inputs after its last frame (e.g. a withdrawal before save).
    pub fn reconstruct(&self) -> Result<TacticalState, ReplayError> {
        let (mut state, mut next_input, mut controls) =
            self.run(None, self.steps.len(), |_, _, _| {})?;
        while let Some(event) = self.inputs.get(next_input) {
            if event.before_step != self.steps.len() {
                return Err(ReplayError::InvalidArchive);
            }
            Self::apply(&event.input, &mut state, &mut controls)?;
            next_input += 1;
        }
        Ok(state)
    }

    /// Reconstruct only the requested round slice. The server calls this AFTER
    /// its existing per-viewer light/fidelity gate. Even though this private
    /// archive contains future inputs, none beyond `to` are evaluated or sent.
    pub fn frames_range(
        &self,
        from: usize,
        to: usize,
    ) -> Result<Vec<(usize, Keyframe)>, ReplayError> {
        self.validate_version()?;
        let first = self.steps.partition_point(|step| step.round < from);
        let end = self.steps.partition_point(|step| step.round < to);
        if first >= end {
            return Ok(Vec::new());
        }
        let checkpoint = self.checkpoints.iter().rev().find(|cp| cp.through <= first);
        let mut frames = Vec::with_capacity(end - first);
        self.run(checkpoint, end, |index, state, mut outcome| {
            if index >= first {
                outcome.deaths.truncate(KEYFRAME_DEATH_CAP);
                frames.push((self.steps[index].round, state.step_keyframe(outcome)));
            }
        })?;
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::Ship;
    use crate::{BattleRecord, EntityId, Loadout, PlayerId, ShipKind, SideRecord, Vec2};

    fn fleet(id: u64, kind: ShipKind, fit: &str, n: u32) -> Vec<(EntityId, Ship)> {
        (0..n)
            .map(|index| {
                let mut ship = Ship::new(index, kind, Loadout::from_key(fit));
                ship.hp *= 0.8; // already-damaged hulls must not replay at full HP
                (EntityId(id), ship)
            })
            .collect()
    }

    fn sides() -> [SideRecord; 2] {
        [1, 2].map(|id| SideRecord {
            corp: PlayerId(id),
            initial: Default::default(),
            initial_loadouts: Default::default(),
            posture: crate::EngagementPolicy::EngageAny,
            platform_tiers: 0,
        })
    }

    fn writeback(state: &TacticalState, roster: &mut Vec<(EntityId, Ship)>) {
        let hp = state
            .hp_writeback()
            .into_iter()
            .map(|(f, s, hp)| ((f, s), hp))
            .collect::<std::collections::BTreeMap<_, _>>();
        roster.retain_mut(|(fleet, ship)| {
            if let Some(&health) = hp.get(&(*fleet, ship.id)) {
                ship.hp = health;
                true
            } else {
                false
            }
        });
    }

    fn scripted_battle(resume: bool) -> (BattleRecord, TacticalState) {
        let mut a = fleet(
            1,
            ShipKind::Battleship,
            "torpedo_rack+reflective_plating",
            12,
        );
        a.extend(fleet(3, ShipKind::Corvette, "point_defense_screen", 6));
        let mut d = fleet(2, ShipKind::Cruiser, "mass_driver+whipple_armor", 18);
        let mut live = TacticalState::open(912, 17, &a, &d, 2, 80.0, Vec2::new(0.7, 0.3));
        let mut direct = live.clone();
        let mut record = BattleRecord::open(EntityId(17), Vec2::ZERO, None, false, 0, sides());
        for i in 0..160u64 {
            let tick = i * 15 + 15;
            // Inputs land BETWEEN engine steps. Their ordering matters even
            // though those off-step ticks produce no animation frame.
            if i == 17 {
                d.extend(fleet(4, ShipKind::Destroyer, "reflective_plating", 9));
            }
            if i == 35 {
                a.retain(|(id, _)| *id != EntityId(3));
            }
            if i == 85 {
                d.extend(fleet(5, ShipKind::Titan, "point_defense_screen", 1));
            }
            assert_eq!(
                record.sync_tactical(tick - 3, &mut live, [&a, &d]),
                direct.sync([&a, &d])
            );
            let mods = [
                SideMods {
                    opening_bonus: true,
                    flak_mult: 1.3,
                    damage_mult: if i < 43 { 1.08 } else { 1.16 },
                },
                SideMods {
                    opening_bonus: false,
                    flak_mult: if i < 55 { 1.0 } else { 1.25 },
                    damage_mult: 1.04,
                },
            ];
            let outcome = record.step_tactical(tick, &mut live, i >= 120, mods);
            let expected = direct.step(i >= 120, mods);
            assert_eq!(live, direct, "recording is observational at step {i}");
            assert_eq!(outcome.deaths, expected.deaths);
            record.accumulate(
                outcome.dealt[0],
                outcome.dealt[1],
                &outcome.losses[0],
                &outcome.losses[1],
            );
            record.flush_step(tick, live.step_keyframe(outcome), Default::default());
            writeback(&live, &mut a);
            writeback(&live, &mut d);
            if i == 70 || i == 130 {
                record.withdraw_tactical(tick + 2, &mut live, 1);
                direct.order_withdraw(1);
            }
            assert_eq!(
                record.tactical_replay().unwrap().reconstruct().unwrap(),
                live,
                "all inputs, RNG and state reproduce through step {i}"
            );
            if resume && i == 51 {
                record = serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
                live = serde_json::from_str(&serde_json::to_string(&live).unwrap()).unwrap();
            }
        }
        (record, live)
    }

    #[test]
    fn deterministic_archive_records_interventions_not_shots_and_resumes_exactly() {
        let (record, live) = scripted_battle(false);
        let (resumed, resumed_live) = scripted_battle(true);
        assert_eq!(live, resumed_live);
        assert_eq!(record, resumed);
        let replay = record.tactical_replay().unwrap();
        assert_eq!(replay.reconstruct().unwrap(), live);
        assert_eq!(replay.steps.len(), 160);
        assert_eq!(replay.checkpoints.len(), 2);
        assert!(
            replay.input_count() < 20,
            "routine shots must not enter the input log"
        );
        let expected = record.frames_range(0, record.rounds.len()).unwrap();
        assert!(
            expected
                .iter()
                .flatten()
                .any(|frame| !frame.deaths.is_empty())
        );
        assert!(
            expected
                .iter()
                .flatten()
                .any(|frame| !frame.torpedoes.is_empty())
        );
        let json = serde_json::to_string(&record).unwrap();
        let restored: BattleRecord = serde_json::from_str(&json).unwrap();
        assert!(
            restored.rounds.iter().all(|round| round.frame.is_none()),
            "disk stores no duplicate frame movie"
        );
        // Out-of-order scrubs, crossing a checkpoint, full history and warmed cache.
        for (from, to) in [(139, 157), (62, 67), (1, 6), (0, 160), (139, 157)] {
            assert_eq!(restored.frames_range(from, to).unwrap(), expected[from..to]);
        }
        assert_eq!(
            serde_json::to_string(&restored).unwrap(),
            json,
            "viewing does not mutate persistent state"
        );
    }

    #[test]
    fn missing_reinforcement_or_modifier_is_detected_not_silently_replayed() {
        let (record, _) = scripted_battle(false);
        let replay = record.tactical_replay().unwrap();
        for roster in [true, false] {
            let mut broken = replay.clone();
            let index = broken
                .inputs
                .iter()
                .position(|event| {
                    if roster {
                        matches!(event.input, Input::Roster(_))
                    } else {
                        event.before_step > 0 && matches!(event.input, Input::Controls(_))
                    }
                })
                .unwrap();
            broken.inputs.remove(index);
            assert!(matches!(
                broken.reconstruct(),
                Err(ReplayError::Diverged { .. })
            ));
        }
        let mut unknown = replay.clone();
        unknown.version = RULES_VERSION + 1;
        assert_eq!(
            unknown.frames_range(0, 1),
            Err(ReplayError::UnsupportedVersion(RULES_VERSION + 1))
        );
        let mut damaged = replay.clone();
        damaged.checkpoints[0].state.combatants[0].hp += 1.0;
        assert!(matches!(
            damaged.frames_range(70, 71),
            Err(ReplayError::Diverged { .. })
        ));
    }

    #[test]
    fn replay_archive_is_smaller_than_the_equivalent_frame_movie() {
        let (record, _) = scripted_battle(false);
        let archive = serde_json::to_vec(&record).unwrap();
        let mut legacy = serde_json::to_value(&record).unwrap();
        legacy.as_object_mut().unwrap().remove("replay");
        for (stored, original) in legacy["rounds"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(&record.rounds)
        {
            stored["frame"] = serde_json::to_value(&original.frame).unwrap();
        }
        let movie = serde_json::to_vec(&legacy).unwrap();
        eprintln!(
            "battle archive: {} bytes; equivalent frame movie: {} bytes; {} sparse inputs / {} steps",
            archive.len(),
            movie.len(),
            record.tactical_replay().unwrap().input_count(),
            record.rounds.len()
        );
        assert!(
            archive.len() < movie.len(),
            "archive must actually remove per-round geometry storage"
        );
        let old: BattleRecord = serde_json::from_value(legacy).unwrap();
        assert!(old.tactical_replay().is_none());
        assert_eq!(
            old.frames_range(0, old.rounds.len()).unwrap(),
            record.frames_range(0, record.rounds.len()).unwrap(),
            "legacy frame movies remain readable"
        );
    }

    #[test]
    fn held_waves_and_repeated_withdrawals_are_reproduced() {
        let mut a = fleet(1, ShipKind::Raider, "", 307);
        let d = fleet(2, ShipKind::Corvette, "point_defense_screen", 2);
        let mut live = TacticalState::open(718, 9, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
        let mut record = BattleRecord::open(EntityId(9), Vec2::ZERO, None, false, 0, sides());
        record.sync_tactical(1, &mut live, [&a, &d]);
        assert_eq!(live.waves[0].len(), 7);
        record.withdraw_tactical(2, &mut live, 0);
        a.extend(fleet(3, ShipKind::Raider, "torpedo_rack", 3));
        record.sync_tactical(3, &mut live, [&a, &d]);
        record.withdraw_tactical(4, &mut live, 0);
        let out = record.step_tactical(15, &mut live, false, [SideMods::default(); 2]);
        record.flush_step(15, live.step_keyframe(out), Default::default());
        assert_eq!(
            record.tactical_replay().unwrap().reconstruct().unwrap(),
            live
        );
        assert!(live.waves[0].is_empty());
    }

    #[test]
    fn published_v1_kernel_fixture_stays_bit_identical() {
        let a = fleet(1, ShipKind::Raider, "torpedo_rack", 3);
        let d = fleet(2, ShipKind::Corvette, "point_defense_screen", 4);
        let mut state = TacticalState::open(98123, 27, &a, &d, 1, 30.0, Vec2::new(0.8, 0.6));
        for _ in 0..25 {
            state.step(false, [SideMods::default(); 2]);
        }
        // Published v1 is immutable. Introduce a new rules version for balance
        // changes; do not rebaseline this historical-state fingerprint.
        assert_eq!(state_checksum(&state), 17_995_168_257_162_801_699);
    }

    #[test]
    fn close_pass_replay_keeps_v2_through_reinforcements_and_restart() {
        maneuver_replay_roundtrip(false);
    }

    #[test]
    fn independent_maneuvers_replay_through_reinforcements_and_restart() {
        maneuver_replay_roundtrip(true);
    }

    fn maneuver_replay_roundtrip(current: bool) {
        let mut a = fleet(1, ShipKind::Raider, "", 12);
        let mut d = fleet(2, ShipKind::Raider, "mass_driver", 12);
        let mut live = if current {
            TacticalState::open_current(191, 77, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0))
        } else {
            let mut state = TacticalState::open(191, 77, &a, &d, 0, 0.0, Vec2::new(1.0, 0.0));
            state.rules = super::super::Rules::V2;
            state
        };
        let mut record = BattleRecord::open(EntityId(77), Vec2::ZERO, None, false, 0, sides());
        for i in 0..80 {
            let tick = (i + 1) * 15;
            if i == 17 { d.extend(fleet(3, ShipKind::Raider, "", 8)); }
            record.sync_tactical(tick - 1, &mut live, [&a, &d]);
            if i == 42 { record.withdraw_tactical(tick - 1, &mut live, 1); }
            let out = record.step_tactical(tick, &mut live, false, [SideMods::default(); 2]);
            record.accumulate(out.dealt[0], out.dealt[1], &out.losses[0], &out.losses[1]);
            record.flush_step(tick, live.step_keyframe(out), Default::default());
            writeback(&live, &mut a);
            writeback(&live, &mut d);
            assert_eq!(record.tactical_replay().unwrap().reconstruct().unwrap(), live);
            if i == 31 {
                record = serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
                live = serde_json::from_str(&serde_json::to_string(&live).unwrap()).unwrap();
            }
        }
        let replay = record.tactical_replay().unwrap();
        assert_eq!(replay.version, if current { 3 } else { 2 });
        assert_eq!(live.rules_version(), replay.version);
        assert!(!replay.checkpoints.is_empty());
        let restored: BattleRecord = serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(restored.frames_range(64, 80).unwrap(), record.frames_range(64, 80).unwrap());
        assert_eq!(restored.tactical_replay().unwrap().reconstruct().unwrap(), live);
        let mut mismatched = replay.clone();
        mismatched.version = 1;
        assert_eq!(mismatched.reconstruct(), Err(ReplayError::InvalidArchive));
        if let super::super::Rules::V3 { maneuver_seed } = replay.initial.rules {
            let mut tampered = replay.clone();
            tampered.initial.rules = super::super::Rules::V3 { maneuver_seed: maneuver_seed ^ 1 };
            assert_ne!(state_checksum(&tampered.initial), replay.initial_checksum);
            assert!(tampered.reconstruct().is_err(), "private steering seed is part of replay integrity");
        }
    }
}

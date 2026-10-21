//! §signals (Oct 2026, Peter): Travian's hero adventures. The Command
//! Center's sensors keep picking up faint, private contacts near home. Any
//! captained fleet can answer one: it flies out, sweeps the spot for
//! [`SIGNAL_SWEEP_S`] and turns for home. What it found is fixed when the
//! signal appears and reaches command with the report, at light speed. A
//! Hazardous signal pays double experience and may hurt the captain, who then
//! sits signals out for [`SIGNAL_HURT_S`]. The first ten are the same for every
//! corporation (they replaced the four first finds).
use crate::{EntityId, Vec2, cargo::Commodity, module::ModuleKind, specialist::SpecialistKind};
use serde::{Deserialize, Serialize};

/// The sweep on site, before the fleet turns for home.
pub const SIGNAL_SWEEP_S: f64 = 30.0;
/// Open signals at most: the next waits for a free slot.
pub const SIGNAL_OPEN_MAX: usize = 5;
/// Answered signals kept for their report cards until dismissed.
pub const SIGNAL_REPORTS_KEPT: usize = 5;
/// How long a hurt captain sits signals out.
pub const SIGNAL_HURT_S: f64 = 4.0 * 3_600.0;
/// Signals appear this far from home: the first ten on the doorstep.
pub const SIGNAL_DOORSTEP_SU: (f64, f64) = (8_000.0, 14_000.0);
pub const SIGNAL_NEAR_SU: (f64, f64) = (10_000.0, 25_000.0);
/// Captain experience for a sweep; Hazardous pays double.
pub const SIGNAL_XP: u32 = 45;
/// Signals numbered from here are ids outside the world's entity counter, so
/// spawning one never shifts another entity's id. Below 2^53 for the client.
pub const SIGNAL_ID_BASE: u64 = 1 << 52;
/// The scripted first ten.
pub const FIRST_TEN: usize = 10;
/// The first signals arrive together when signals open.
pub const SIGNAL_OPENING: u32 = 2;

/// Travian's x1 adventure curve, by days since a corporation's signals began:
/// three a day for three days, two a day to day 16, one and a half to day 63,
/// then one.
pub fn signal_interval_s(age_s: f64) -> f64 {
    let day = age_s / 86_400.0;
    86_400.0 / if day < 3.0 { 3.0 } else if day < 16.0 { 2.0 } else if day < 63.0 { 1.5 } else { 1.0 }
}

/// Hazardous injury odds: 45% less 15 points per Fieldcraft point, floor 5%.
pub fn hurt_chance(fieldcraft: u8) -> f64 {
    (0.45 - 0.15 * fieldcraft as f64).max(0.05)
}

/// Reward tiers rise with the world's age: day 7, then day 21.
pub fn signal_tier(world_age_s: f64) -> u8 {
    let day = world_age_s / 86_400.0;
    if day < 7.0 { 1 } else if day < 21.0 { 2 } else { 3 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    DriftingWreck,
    CargoPod,
    OreFragment,
    SilentProbe,
    DistressBeacon,
    SensorGhost,
    AuthorityCourier,
    AbandonedOutpost,
    PirateStash,
    GhostShip,
    PrecursorFragment,
    ExoticFlare,
    TeosLostCrate,
}

impl SignalKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::DriftingWreck => "Drifting wreck",
            Self::CargoPod => "Cargo pod",
            Self::OreFragment => "Ore fragment",
            Self::SilentProbe => "Silent probe",
            Self::DistressBeacon => "Distress beacon",
            Self::SensorGhost => "Sensor ghost",
            Self::AuthorityCourier => "Authority courier",
            Self::AbandonedOutpost => "Abandoned outpost",
            Self::PirateStash => "Pirate stash",
            Self::GhostShip => "Ghost ship",
            Self::PrecursorFragment => "Precursor fragment",
            Self::ExoticFlare => "Exotic flare",
            Self::TeosLostCrate => "Teo's lost crate",
        }
    }

    /// The world day a random signal of this kind can first appear, and its
    /// weight in the draw.
    fn draw(self) -> (f64, u32) {
        match self {
            Self::DriftingWreck | Self::CargoPod => (0.0, 14),
            Self::OreFragment => (0.0, 12),
            Self::SilentProbe | Self::DistressBeacon | Self::SensorGhost => (0.0, 10),
            Self::AuthorityCourier => (7.0, 6),
            Self::AbandonedOutpost | Self::PirateStash | Self::GhostShip => (7.0, 7),
            Self::PrecursorFragment | Self::ExoticFlare => (21.0, 5),
            Self::TeosLostCrate => (0.0, 2),
        }
    }

    pub const ALL: [SignalKind; 13] = [
        Self::DriftingWreck, Self::CargoPod, Self::OreFragment, Self::SilentProbe, Self::DistressBeacon,
        Self::SensorGhost, Self::AuthorityCourier, Self::AbandonedOutpost, Self::PirateStash, Self::GhostShip,
        Self::PrecursorFragment, Self::ExoticFlare, Self::TeosLostCrate,
    ];
}

/// One thing a signal holds, fixed when it appears.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignalReward {
    Credits { credits: f64 },
    /// An Authority courier returned: a fee and standing.
    Standing { credits: f64, standing: f64 },
    /// Goods aboard if the hold has room; the rest left as a cache for a
    /// Freighter.
    Cargo { goods: Vec<(Commodity, u32)> },
    /// A module crate: one rides in any hull.
    Module { module: ModuleKind },
    /// Data for the active programme, else the next one open.
    Dossier { fraction: f64 },
    /// The first of these not yet licensed; credits if all are.
    Blueprint { options: Vec<ModuleKind> },
    /// A veteran Fieldcraft captain joins the reserve at home.
    Captain,
    /// A specialist, home by liner.
    Specialist { specialist: SpecialistKind },
}

impl SignalReward {
    /// The first-ten strip's "next" line.
    pub fn teaser(&self) -> &'static str {
        match self {
            Self::Credits { .. } => "credits",
            Self::Standing { .. } => "Authority standing",
            Self::Cargo { .. } => "a cargo cache",
            Self::Module { .. } => "a module crate",
            Self::Dossier { .. } => "a research dossier",
            Self::Blueprint { .. } => "a blueprint",
            Self::Captain => "a captain",
            Self::Specialist { .. } => "a specialist",
        }
    }
}

/// What a sweep turned up, as its report reads at command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SignalFind {
    Credits { credits: f64 },
    Standing { credits: f64, standing: f64 },
    /// Loaded aboard the answering fleet, landing at home with it.
    CargoAboard { goods: Vec<(Commodity, u32)> },
    /// Left where the signal was, for a Freighter (an exploration site).
    Cache { site: EntityId, goods: Vec<(Commodity, u32)> },
    ModuleAboard { module: ModuleKind },
    Dossier { programme: String },
    Blueprint { module: ModuleKind },
    Captain { name: String },
    /// The survivors' own craft, flying home.
    Specialist { specialist: SpecialistKind },
}

/// The sweep's result, fixed on site; applied when its report reaches command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalOutcome {
    pub fleet: EntityId,
    pub captain: u32,
    pub captain_name: String,
    pub swept_at: f64,
    pub report_at: f64,
    pub xp: u32,
    pub hurt: bool,
    /// Goods and crates already aboard the fleet.
    pub aboard: Vec<SignalFind>,
    /// Goods with no room aboard, left for a Freighter.
    #[serde(default)]
    pub left: Vec<(Commodity, u32)>,
    /// Filled when the report arrives.
    #[serde(default)]
    pub finds: Vec<SignalFind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub id: EntityId,
    /// 1-based, in order of appearance: the first ten are scripted.
    pub number: u32,
    pub kind: SignalKind,
    pub hazardous: bool,
    pub pos: Vec2,
    pub detected_at: f64,
    pub xp: u32,
    /// Hidden from the client until the report arrives.
    pub rewards: Vec<SignalReward>,
    #[serde(default)]
    pub outcome: Option<SignalOutcome>,
    #[serde(default)]
    pub reported_at: Option<f64>,
}

impl Signal {
    pub fn open(&self) -> bool {
        self.reported_at.is_none()
    }
}

/// A corporation's signals: open ones, and answered ones until dismissed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SignalBook {
    /// When this corporation's signals began; None until they open.
    #[serde(default)]
    pub opened_at: Option<f64>,
    #[serde(default)]
    pub next_at: f64,
    /// Signals detected so far.
    #[serde(default)]
    pub detected: u32,
    #[serde(default)]
    pub signals: Vec<Signal>,
    /// Scripted first-ten numbers whose reports reached command.
    #[serde(default)]
    pub first_ten_reported: std::collections::BTreeSet<u32>,
}

impl SignalBook {
    pub fn open_count(&self) -> usize {
        self.signals.iter().filter(|s| s.open()).count()
    }

    /// The first ten's next find not yet reported, for the strip.
    pub fn first_ten_next(&self) -> Option<&'static str> {
        (1..=FIRST_TEN as u32).find(|n| !self.first_ten_reported.contains(n))
            .map(|n| first_ten(n).2.first().map_or("experience", SignalReward::teaser))
    }
}

/// §signals: the scripted first ten, the same for every corporation. Numbers
/// 1–4 are the old first finds.
pub fn first_ten(number: u32) -> (SignalKind, bool, Vec<SignalReward>) {
    use SignalKind as K;
    use SignalReward as R;
    match number {
        1 => (K::SilentProbe, false, vec![R::Credits { credits: 1_500.0 }]),
        2 => (K::DistressBeacon, false, vec![R::Captain]),
        3 => (K::DriftingWreck, false, vec![R::Blueprint { options: FIND_BLUEPRINTS.to_vec() }]),
        4 => (K::OreFragment, false, vec![R::Cargo { goods: FIND_CACHE.to_vec() }]),
        5 => (K::SensorGhost, false, vec![R::Dossier { fraction: 0.15 }]),
        6 => (K::DriftingWreck, false, vec![R::Module { module: ModuleKind::ExtendedTanks }]),
        7 => (K::AuthorityCourier, false, vec![R::Standing { credits: 600.0, standing: 2.0 }]),
        8 => (K::DistressBeacon, false, vec![R::Specialist { specialist: SpecialistKind::Geologist }]),
        9 => (K::CargoPod, false, vec![R::Cargo { goods: vec![(Commodity::Electronics, 15), (Commodity::Polymers, 20)] }]),
        // The first Hazardous signal holds nothing to lose: it teaches the risk.
        _ => (K::SensorGhost, true, vec![]),
    }
}

/// The reward captain's profile (`Captain::founding`), Fieldcraft all: one a
/// corporation already has is passed over for the next.
pub const FIND_CAPTAINS: [u64; 5] = [15, 7, 11, 19, 3];
/// The level the reward captain arrives at.
pub const FIND_CAPTAIN_LEVEL: u8 = 2;
pub const FIND_BLUEPRINTS: [ModuleKind; 3] = [ModuleKind::ReconSuite, ModuleKind::ExtendedTanks, ModuleKind::CargoPods];
/// The prospector's cache: fits a Tiny Freighter's hold in one trip.
pub const FIND_CACHE: [(Commodity, u32); 2] = [(Commodity::TitaniumOre, 30), (Commodity::RareMetalOre, 2)];

/// A random signal after the first ten: its kind by the world's age, then
/// what it holds.
pub fn draw_signal(rng: &mut crate::rng::Rng, world_age_s: f64) -> (SignalKind, bool, Vec<SignalReward>) {
    let day = world_age_s / 86_400.0;
    let pool: Vec<_> = SignalKind::ALL.into_iter().filter(|k| k.draw().0 <= day).collect();
    let total: u32 = pool.iter().map(|k| k.draw().1).sum();
    let mut pick = (rng.next_u64() % total as u64) as u32;
    let kind = pool.into_iter().find(|k| {
        let w = k.draw().1;
        if pick < w { true } else { pick -= w; false }
    }).unwrap_or(SignalKind::SensorGhost);
    let hazardous = match kind {
        SignalKind::AbandonedOutpost | SignalKind::PirateStash | SignalKind::GhostShip
        | SignalKind::PrecursorFragment | SignalKind::ExoticFlare => true,
        SignalKind::DistressBeacon => rng.next_f64() < 0.25,
        _ => false,
    };
    let rewards = draw_rewards(rng, kind, hazardous, signal_tier(world_age_s));
    (kind, hazardous, rewards)
}

fn draw_rewards(rng: &mut crate::rng::Rng, kind: SignalKind, hazardous: bool, tier: u8) -> Vec<SignalReward> {
    use Commodity as C;
    use ModuleKind as M;
    use SignalReward as R;
    let scale = [1.0, 1.6, 2.5][tier.clamp(1, 3) as usize - 1];
    let roll = rng.next_f64();
    let pick = rng.next_u64();
    let mut units = |lo: u32, hi: u32| ((lo as f64 + rng.next_f64() * (hi - lo) as f64) * scale).round() as u32;
    let one = |options: &[M]| options[(pick % options.len() as u64) as usize];
    let crate_tiers: [&[M]; 3] = [&[M::WhippleArmor, M::ExtendedTanks],
        &[M::WhippleArmor, M::ExtendedTanks, M::ReflectivePlating, M::ReconSuite],
        &[M::ReflectivePlating, M::ReconSuite, M::PointDefenseScreen, M::WarpStabilizer]];
    let crates = crate_tiers[tier.clamp(1, 3) as usize - 1];
    match kind {
        SignalKind::DriftingWreck if roll < 0.5 => vec![R::Cargo { goods: vec![(C::Alloys, units(20, 40)), (C::Machinery, units(4, 10))] }],
        SignalKind::DriftingWreck if roll < 0.85 => vec![R::Module { module: one(crates) }],
        SignalKind::DriftingWreck => vec![],
        SignalKind::CargoPod => vec![R::Cargo { goods: vec![match pick % 3 {
            0 => (C::Polymers, units(20, 35)),
            1 => (C::Provisions, units(25, 40)),
            _ => (C::Electronics, units(8, 15)),
        }] }],
        SignalKind::OreFragment => {
            let mut goods = vec![(if pick.is_multiple_of(2) { C::CupriteOre } else { C::TitaniumOre }, units(25, 40))];
            if tier >= 3 { goods.push((C::RareMetalOre, units(1, 2))); }
            vec![R::Cargo { goods }]
        }
        SignalKind::SilentProbe => vec![R::Credits { credits: units(500, 900) as f64 }],
        SignalKind::DistressBeacon if hazardous && roll < 0.1 => vec![R::Captain],
        SignalKind::DistressBeacon if roll < if hazardous { 0.8 } else { 0.55 } =>
            vec![R::Specialist { specialist: SpecialistKind::ALL[(pick % 5) as usize] }],
        SignalKind::DistressBeacon if roll < 0.8 || hazardous => vec![R::Credits { credits: units(300, 600) as f64 }],
        SignalKind::DistressBeacon => vec![],
        SignalKind::SensorGhost if roll < 0.4 => vec![R::Dossier { fraction: 0.15 }],
        SignalKind::SensorGhost => vec![],
        SignalKind::AuthorityCourier => vec![R::Standing { credits: units(400, 700) as f64, standing: 2.0 }],
        SignalKind::AbandonedOutpost => {
            let mut out = vec![R::Dossier { fraction: 0.2 },
                R::Cargo { goods: vec![(C::Electronics, units(10, 20)), (C::Machinery, units(8, 15))] }];
            if roll < 0.15 { out.push(R::Blueprint { options: FIND_BLUEPRINTS.to_vec() }); }
            out
        }
        SignalKind::PirateStash if roll < 0.4 => vec![R::Module { module: one(&[M::MassDriver, M::TorpedoRack]) }],
        SignalKind::PirateStash => vec![R::Credits { credits: units(800, 1_500) as f64 },
            R::Cargo { goods: vec![(C::Armaments, units(10, 20))] }],
        SignalKind::GhostShip => {
            let mut out = vec![R::Module { module: one(&[M::PointDefenseScreen, M::ReflectivePlating]) }];
            if roll < 0.1 { out.push(R::Blueprint { options: FIND_BLUEPRINTS.to_vec() }); }
            out
        }
        SignalKind::PrecursorFragment => {
            let mut out = vec![R::Dossier { fraction: 0.3 },
                R::Cargo { goods: vec![(C::PrecisionComponents, units(6, 12))] }];
            if roll < 0.1 {
                out.push(R::Blueprint { options: vec![M::SurveyDrive, M::NebulaSpectrometer, M::PrismaticLance] });
            }
            out
        }
        SignalKind::ExoticFlare => vec![R::Cargo { goods: vec![if pick.is_multiple_of(2) { (C::ExoticMatter, units(1, 3)) } else { (C::VoidCrystal, units(1, 2)) }] }],
        SignalKind::TeosLostCrate => vec![R::Cargo { goods: vec![if pick.is_multiple_of(2) { (C::Electronics, units(10, 15)) } else { (C::Deuterium, units(20, 30)) }] },
            R::Credits { credits: units(200, 400) as f64 }],
    }
}

/// Captain experience for a sweep: Hazardous double, an exotic flare four
/// times.
pub fn signal_xp(kind: SignalKind, hazardous: bool) -> u32 {
    SIGNAL_XP * if kind == SignalKind::ExoticFlare { 4 } else if hazardous { 2 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_slows_as_travians_does() {
        let day = 86_400.0;
        assert_eq!(signal_interval_s(0.0), 8.0 * 3_600.0);
        assert_eq!(signal_interval_s(3.0 * day), 12.0 * 3_600.0);
        assert_eq!(signal_interval_s(16.0 * day), 16.0 * 3_600.0);
        assert_eq!(signal_interval_s(63.0 * day), day);
    }

    #[test]
    fn fieldcraft_lowers_the_injury_odds_to_a_floor() {
        assert!((hurt_chance(1) - 0.30).abs() < 1e-9);
        assert!((hurt_chance(2) - 0.15).abs() < 1e-9);
        assert_eq!(hurt_chance(3), 0.05);
        assert_eq!(hurt_chance(5), 0.05);
    }

    #[test]
    fn random_signals_respect_the_worlds_age() {
        let mut rng = crate::rng::Rng::keyed(7, "signals-test");
        for _ in 0..500 {
            let (kind, hazardous, _) = draw_signal(&mut rng, 0.0);
            assert!(kind.draw().0 == 0.0, "{kind:?} appeared on day 0");
            assert!(!hazardous || kind == SignalKind::DistressBeacon);
        }
        let late: Vec<_> = (0..2_000).map(|_| draw_signal(&mut rng, 30.0 * 86_400.0).0).collect();
        for kind in SignalKind::ALL {
            assert!(late.contains(&kind), "{kind:?} never appears late");
        }
    }

    #[test]
    fn the_first_ten_end_on_a_hazard_with_nothing_to_lose() {
        let (kind, hazardous, rewards) = first_ten(10);
        assert_eq!((kind, hazardous), (SignalKind::SensorGhost, true));
        assert!(rewards.is_empty());
        assert!((1..10).all(|n| !first_ten(n).1), "the first nine are Routine");
    }
}

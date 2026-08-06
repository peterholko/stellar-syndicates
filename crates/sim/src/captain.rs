//! Named fleet officers. A Captain is a physical person assigned to one fleet;
//! their state therefore rides that fleet's ordinary delayed sighting instead of
//! appearing in a fresh corporation-wide roster.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::{EntityId, PlayerId, ShipKind, Vec2};

pub const CAPTAIN_ATTRIBUTE_MAX: u8 = 5;
pub const CAPTAIN_LEVEL_MAX: u8 = 10;
/// An officer whose formation is wiped out is unavailable while escape craft,
/// medical recovery, and the formal board of inquiry run. The report itself is
/// still light-delayed; recovery can never complete before command learns the
/// officer survived. Tunable.
pub const CAPTAIN_RECOVERY_S: f64 = 300.0;
pub const CAPTAIN_INJURY_RECOVERY_S: f64 = 600.0;
pub const CAPTAIN_CAPTURE_RECOVERY_S: f64 = 1_200.0;
/// Minimum useful experience awards. Kept here so the four career verbs read as
/// one published table rather than unrelated magic numbers in `world.rs`.
pub const CAPTAIN_XP_JUMP: u32 = 12;
pub const CAPTAIN_XP_SURVEY: u32 = 35;
pub const CAPTAIN_XP_LOGISTICS: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptainTitle {
    Lieutenant,
    LieutenantCommander,
    Commander,
    Captain,
    RearAdmiral,
    ViceAdmiral,
    Admiral,
    FleetAdmiral,
}

impl CaptainTitle {
    /// Ten experience levels feed eight service titles. The two upper flag
    /// grades deliberately take two levels each, while Fleet Admiral remains
    /// the maximum-level distinction.
    pub fn for_level(level: u8) -> Self {
        match level {
            0 | 1 => Self::Lieutenant,
            2 => Self::LieutenantCommander,
            3 => Self::Commander,
            4 => Self::Captain,
            5 => Self::RearAdmiral,
            6 | 7 => Self::ViceAdmiral,
            8 | 9 => Self::Admiral,
            _ => Self::FleetAdmiral,
        }
    }

    /// Weighted hull authority. Capacity, rather than raw headcount, makes two
    /// Interceptors a Lieutenant's formation while a Titan remains flag-rank
    /// work even when it is the only hull present.
    pub fn command_capacity(self) -> u32 {
        match self {
            Self::Lieutenant => 4,
            Self::LieutenantCommander => 8,
            Self::Commander => 16,
            Self::Captain => 32,
            Self::RearAdmiral => 64,
            Self::ViceAdmiral => 96,
            Self::Admiral => 160,
            Self::FleetAdmiral => 256,
        }
    }
}

/// Command POINTS per hull. This is the single authoritative type/number
/// relationship: support craft are cheap to coordinate; each rung of the line
/// of battle roughly doubles the burden, so senior ranks unlock both heavier
/// flagships and larger screens around them.
pub fn command_weight(kind: ShipKind) -> u32 {
    match kind {
        ShipKind::Scout | ShipKind::Convoy | ShipKind::Builder | ShipKind::Freighter => 1,
        ShipKind::Raider => 2,
        ShipKind::Corvette | ShipKind::Colony | ShipKind::Transport => 4,
        ShipKind::Destroyer => 8,
        ShipKind::Cruiser => 16,
        ShipKind::Battleship => 32,
        ShipKind::Dreadnought => 64,
        ShipKind::Titan => 128,
    }
}

pub fn command_load(composition: &BTreeMap<ShipKind, u32>) -> u32 {
    composition.iter().fold(0u32, |total, (kind, count)| {
        total.saturating_add(command_weight(*kind).saturating_mul(*count))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptainPortraitAge {
    Young,
    Middle,
    Senior,
}

impl CaptainPortraitAge {
    /// Portrait age follows the same delayed level sighting as the title: no
    /// portrait can disclose a distant promotion before its report arrives.
    pub fn for_level(level: u8) -> Self {
        match level {
            0..=3 => Self::Young,
            4..=7 => Self::Middle,
            _ => Self::Senior,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptainPortrait {
    MaraVenn,
    EliasRook,
    SeraOkafor,
    JunArclight,
    AstridNystrom,
    HenrikSondergaard,
    LucaFerraro,
    KarimBenYoussef,
    MateoQuispe,
    TaliaFaumuina,
    AnaLuisaNascimento,
    ElenaValdes,
    DaphneMarkou,
    HanMinJae,
    NadineIlunga,
    DaichiMori,
    ChenJianyu,
    NattayaChantarat,
    LinXiaoyu,
    ReinaKuroda,
    YoonSeoYeon,
}

impl CaptainPortrait {
    pub fn slug(self) -> &'static str {
        match self {
            Self::MaraVenn => "mara_venn",
            Self::EliasRook => "elias_rook",
            Self::SeraOkafor => "sera_okafor",
            Self::JunArclight => "jun_arclight",
            Self::AstridNystrom => "astrid_nystrom",
            Self::HenrikSondergaard => "henrik_sondergaard",
            Self::LucaFerraro => "luca_ferraro",
            Self::KarimBenYoussef => "karim_ben_youssef",
            Self::MateoQuispe => "mateo_quispe",
            Self::TaliaFaumuina => "talia_faumuina",
            Self::AnaLuisaNascimento => "ana_luisa_nascimento",
            Self::ElenaValdes => "elena_valdes",
            Self::DaphneMarkou => "daphne_markou",
            Self::HanMinJae => "han_min_jae",
            Self::NadineIlunga => "nadine_ilunga",
            Self::DaichiMori => "daichi_mori",
            Self::ChenJianyu => "chen_jianyu",
            Self::NattayaChantarat => "nattaya_chantarat",
            Self::LinXiaoyu => "lin_xiaoyu",
            Self::ReinaKuroda => "reina_kuroda",
            Self::YoonSeoYeon => "yoon_seo_yeon",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptainAttributes {
    pub command: u8,
    pub navigation: u8,
    pub fieldcraft: u8,
    pub logistics: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptainAttribute {
    Command,
    Navigation,
    Fieldcraft,
    Logistics,
}

/// The fate fixed when an officer's entire formation is lost. It is truth at
/// the wreck, but the owner does not receive it until the casualty report's
/// light arrives. Rescue, injury and capture are finite absences; death is a
/// permanent memorial entry and frees a roster berth only once it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptainLossFate {
    Rescued,
    Injured,
    Captured,
    Killed,
}

impl CaptainLossFate {
    /// Keyed rather than world-RNG-driven so adding an officer outcome cannot
    /// perturb combat, markets, or galaxy generation. The buckets are 45%
    /// rescue, 30% injury, 15% capture, and 10% death.
    pub fn for_loss(
        owner: PlayerId,
        captain_id: u32,
        fleet: EntityId,
        incident_key: u64,
    ) -> Self {
        let mut x = owner.0
            ^ fleet.0.rotate_left(17)
            ^ (captain_id as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ incident_key.rotate_left(37);
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
        match x % 20 {
            0..=8 => Self::Rescued,
            9..=14 => Self::Injured,
            15..=17 => Self::Captured,
            _ => Self::Killed,
        }
    }

    pub fn recovery_secs(self) -> Option<f64> {
        match self {
            Self::Rescued => Some(CAPTAIN_RECOVERY_S),
            Self::Injured => Some(CAPTAIN_INJURY_RECOVERY_S),
            Self::Captured => Some(CAPTAIN_CAPTURE_RECOVERY_S),
            Self::Killed => None,
        }
    }
}

/// The copy-safe portion recorded beside every fleet position sample. Identity
/// is immutable and stays on the track; progression belongs here because a
/// distant command center must not learn a level-up before its light arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptainSighting {
    pub level: u8,
    pub title: CaptainTitle,
    pub portrait_age: CaptainPortraitAge,
    pub command_capacity: u32,
    pub xp: u32,
    pub next_level_xp: u32,
    pub unspent: u8,
    pub attributes: CaptainAttributes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Captain {
    /// Corporation-local stable identity. Officer ids never collide within a
    /// roster and do not consume the world's physical-entity id space.
    #[serde(default)]
    pub id: u32,
    pub name: String,
    pub portrait: CaptainPortrait,
    /// The physical formation carrying this officer. `None` means reserve duty
    /// at `stationed_system`. A missing officer deliberately retains the lost
    /// fleet id until their delayed survival report arrives.
    pub assigned_fleet: Option<EntityId>,
    #[serde(default)]
    pub stationed_system: Option<EntityId>,
    /// Last true position while assigned, used only to price the casualty report.
    #[serde(default)]
    pub last_pos: Vec2,
    #[serde(default)]
    pub missing_since: Option<f64>,
    #[serde(default)]
    pub missing_report_at: Option<f64>,
    #[serde(default)]
    pub recover_at: Option<f64>,
    #[serde(default)]
    pub loss_fate: Option<CaptainLossFate>,
    pub level: u8,
    pub xp: u32,
    pub unspent: u8,
    pub attributes: CaptainAttributes,
}

impl Captain {
    /// Deterministic roster selection: different corporations normally open
    /// with different faces, while a replay/snapshot always chooses identically.
    pub fn founding(player: PlayerId, fleet: EntityId) -> Self {
        let (name, portrait, focus) = match player.0 % 21 {
            0 => (
                "Mara Venn",
                CaptainPortrait::MaraVenn,
                CaptainAttribute::Command,
            ),
            1 => (
                "Elias Rook",
                CaptainPortrait::EliasRook,
                CaptainAttribute::Logistics,
            ),
            2 => (
                "Sera Okafor",
                CaptainPortrait::SeraOkafor,
                CaptainAttribute::Navigation,
            ),
            3 => (
                "Jun Arclight",
                CaptainPortrait::JunArclight,
                CaptainAttribute::Fieldcraft,
            ),
            4 => (
                "Astrid Nyström",
                CaptainPortrait::AstridNystrom,
                CaptainAttribute::Navigation,
            ),
            5 => (
                "Henrik Søndergaard",
                CaptainPortrait::HenrikSondergaard,
                CaptainAttribute::Command,
            ),
            6 => (
                "Luca Ferraro",
                CaptainPortrait::LucaFerraro,
                CaptainAttribute::Logistics,
            ),
            7 => (
                "Karim Ben Youssef",
                CaptainPortrait::KarimBenYoussef,
                CaptainAttribute::Fieldcraft,
            ),
            8 => (
                "Mateo Quispe",
                CaptainPortrait::MateoQuispe,
                CaptainAttribute::Navigation,
            ),
            9 => (
                "Talia Faumuina",
                CaptainPortrait::TaliaFaumuina,
                CaptainAttribute::Command,
            ),
            10 => (
                "Ana Luísa Nascimento",
                CaptainPortrait::AnaLuisaNascimento,
                CaptainAttribute::Logistics,
            ),
            11 => (
                "Elena Valdés",
                CaptainPortrait::ElenaValdes,
                CaptainAttribute::Fieldcraft,
            ),
            12 => (
                "Daphne Markou",
                CaptainPortrait::DaphneMarkou,
                CaptainAttribute::Command,
            ),
            13 => (
                "Han Min-jae",
                CaptainPortrait::HanMinJae,
                CaptainAttribute::Logistics,
            ),
            14 => (
                "Nadine Ilunga",
                CaptainPortrait::NadineIlunga,
                CaptainAttribute::Navigation,
            ),
            15 => (
                "Daichi Mori",
                CaptainPortrait::DaichiMori,
                CaptainAttribute::Fieldcraft,
            ),
            16 => (
                "Chen Jianyu",
                CaptainPortrait::ChenJianyu,
                CaptainAttribute::Command,
            ),
            17 => (
                "Nattaya Chantarat",
                CaptainPortrait::NattayaChantarat,
                CaptainAttribute::Logistics,
            ),
            18 => (
                "Lin Xiaoyu",
                CaptainPortrait::LinXiaoyu,
                CaptainAttribute::Navigation,
            ),
            19 => (
                "Reina Kuroda",
                CaptainPortrait::ReinaKuroda,
                CaptainAttribute::Fieldcraft,
            ),
            _ => (
                "Yoon Seo-yeon",
                CaptainPortrait::YoonSeoYeon,
                CaptainAttribute::Logistics,
            ),
        };
        let mut out = Self {
            id: 0,
            name: name.to_string(),
            portrait,
            assigned_fleet: Some(fleet),
            stationed_system: None,
            last_pos: Vec2::ZERO,
            missing_since: None,
            missing_report_at: None,
            recover_at: None,
            loss_fate: None,
            level: 1,
            xp: 0,
            unspent: 0,
            attributes: CaptainAttributes {
                command: 1,
                navigation: 1,
                fieldcraft: 1,
                logistics: 1,
            },
        };
        out.raise(focus);
        out
    }

    /// Deterministic Academy graduate. The stride walks a corporation through
    /// different faces/foci without consuming the world RNG stream, so adding a
    /// recruitment never re-rolls combat, markets, or galaxy generation.
    pub fn recruit(player: PlayerId, serial: u32, system: EntityId, pos: Vec2) -> Self {
        let profile = PlayerId(player.0.wrapping_add(serial as u64 * 7));
        let mut out = Self::founding(profile, EntityId(0));
        out.id = serial;
        out.assigned_fleet = None;
        out.stationed_system = Some(system);
        out.last_pos = pos;
        out
    }

    pub fn available_in_reserve(&self) -> bool {
        self.assigned_fleet.is_none() && self.recover_at.is_none()
    }

    pub fn missing(&self) -> bool {
        self.missing_since.is_some()
    }

    pub fn sighting(&self) -> CaptainSighting {
        CaptainSighting {
            level: self.level,
            title: CaptainTitle::for_level(self.level),
            portrait_age: CaptainPortraitAge::for_level(self.level),
            command_capacity: self.command_capacity(),
            xp: self.xp,
            next_level_xp: xp_for_level(self.level.saturating_add(1)),
            unspent: self.unspent,
            attributes: self.attributes,
        }
    }

    pub fn grant_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
        while self.level < CAPTAIN_LEVEL_MAX
            && self.xp >= xp_for_level(self.level.saturating_add(1))
        {
            self.level += 1;
            self.unspent = self.unspent.saturating_add(1);
        }
    }

    pub fn train(&mut self, attribute: CaptainAttribute) -> bool {
        if self.unspent == 0 || self.attribute(attribute) >= CAPTAIN_ATTRIBUTE_MAX {
            return false;
        }
        self.unspent -= 1;
        self.raise(attribute);
        true
    }

    pub fn command_capacity(&self) -> u32 {
        CaptainTitle::for_level(self.level).command_capacity()
    }

    pub fn can_command(&self, composition: &BTreeMap<ShipKind, u32>) -> bool {
        command_load(composition) <= self.command_capacity()
    }

    pub fn command_damage_mult(&self) -> f64 {
        1.0 + (self.attributes.command.saturating_sub(1) as f64 * 0.02).min(0.10)
    }

    pub fn navigation_spool_mult(&self) -> f64 {
        1.0 - (self.attributes.navigation.saturating_sub(1) as f64 * 0.02).min(0.10)
    }

    pub fn fieldcraft_survey_mult(&self) -> f64 {
        1.0 - (self.attributes.fieldcraft.saturating_sub(1) as f64 * 0.03).min(0.15)
    }

    pub fn logistics_fuel_mult(&self) -> f64 {
        1.0 - (self.attributes.logistics.saturating_sub(1) as f64 * 0.02).min(0.10)
    }

    fn attribute(&self, attribute: CaptainAttribute) -> u8 {
        match attribute {
            CaptainAttribute::Command => self.attributes.command,
            CaptainAttribute::Navigation => self.attributes.navigation,
            CaptainAttribute::Fieldcraft => self.attributes.fieldcraft,
            CaptainAttribute::Logistics => self.attributes.logistics,
        }
    }

    fn raise(&mut self, attribute: CaptainAttribute) {
        let value = match attribute {
            CaptainAttribute::Command => &mut self.attributes.command,
            CaptainAttribute::Navigation => &mut self.attributes.navigation,
            CaptainAttribute::Fieldcraft => &mut self.attributes.fieldcraft,
            CaptainAttribute::Logistics => &mut self.attributes.logistics,
        };
        *value = value.saturating_add(1).min(CAPTAIN_ATTRIBUTE_MAX);
    }
}

/// Cumulative XP gate. Level 2 = 100, level 3 = 300, level 4 = 600 …
pub fn xp_for_level(level: u8) -> u32 {
    let n = level.saturating_sub(1) as u32;
    100 * n * (n + 1) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_ups_bank_explicit_bounded_attribute_points() {
        let mut c = Captain::founding(PlayerId(0), EntityId(9));
        c.grant_xp(300);
        assert_eq!((c.level, c.unspent), (3, 2));
        assert!(c.train(CaptainAttribute::Navigation));
        assert_eq!((c.attributes.navigation, c.unspent), (2, 1));

        c.attributes.navigation = CAPTAIN_ATTRIBUTE_MAX;
        assert!(!c.train(CaptainAttribute::Navigation));
        assert_eq!(c.unspent, 1, "a capped stat must not eat the point");
    }

    #[test]
    fn all_effects_stay_inside_the_published_caps() {
        let mut c = Captain::founding(PlayerId(0), EntityId(9));
        c.attributes = CaptainAttributes {
            command: 99,
            navigation: 99,
            fieldcraft: 99,
            logistics: 99,
        };
        assert_eq!(c.command_damage_mult(), 1.10);
        assert_eq!(c.navigation_spool_mult(), 0.90);
        assert_eq!(c.fieldcraft_survey_mult(), 0.85);
        assert_eq!(c.logistics_fuel_mult(), 0.90);
    }

    #[test]
    fn expanded_profiles_are_part_of_the_deterministic_roster() {
        let profiles = [
            (
                4,
                "Astrid Nyström",
                CaptainPortrait::AstridNystrom,
                CaptainAttribute::Navigation,
            ),
            (
                5,
                "Henrik Søndergaard",
                CaptainPortrait::HenrikSondergaard,
                CaptainAttribute::Command,
            ),
            (
                6,
                "Luca Ferraro",
                CaptainPortrait::LucaFerraro,
                CaptainAttribute::Logistics,
            ),
            (
                7,
                "Karim Ben Youssef",
                CaptainPortrait::KarimBenYoussef,
                CaptainAttribute::Fieldcraft,
            ),
            (
                8,
                "Mateo Quispe",
                CaptainPortrait::MateoQuispe,
                CaptainAttribute::Navigation,
            ),
            (
                9,
                "Talia Faumuina",
                CaptainPortrait::TaliaFaumuina,
                CaptainAttribute::Command,
            ),
            (
                10,
                "Ana Luísa Nascimento",
                CaptainPortrait::AnaLuisaNascimento,
                CaptainAttribute::Logistics,
            ),
            (
                11,
                "Elena Valdés",
                CaptainPortrait::ElenaValdes,
                CaptainAttribute::Fieldcraft,
            ),
            (
                12,
                "Daphne Markou",
                CaptainPortrait::DaphneMarkou,
                CaptainAttribute::Command,
            ),
            (
                13,
                "Han Min-jae",
                CaptainPortrait::HanMinJae,
                CaptainAttribute::Logistics,
            ),
            (
                14,
                "Nadine Ilunga",
                CaptainPortrait::NadineIlunga,
                CaptainAttribute::Navigation,
            ),
            (
                15,
                "Daichi Mori",
                CaptainPortrait::DaichiMori,
                CaptainAttribute::Fieldcraft,
            ),
            (
                16,
                "Chen Jianyu",
                CaptainPortrait::ChenJianyu,
                CaptainAttribute::Command,
            ),
            (
                17,
                "Nattaya Chantarat",
                CaptainPortrait::NattayaChantarat,
                CaptainAttribute::Logistics,
            ),
            (
                18,
                "Lin Xiaoyu",
                CaptainPortrait::LinXiaoyu,
                CaptainAttribute::Navigation,
            ),
            (
                19,
                "Reina Kuroda",
                CaptainPortrait::ReinaKuroda,
                CaptainAttribute::Fieldcraft,
            ),
            (
                20,
                "Yoon Seo-yeon",
                CaptainPortrait::YoonSeoYeon,
                CaptainAttribute::Logistics,
            ),
        ];
        for (player, name, portrait, focus) in profiles {
            let c = Captain::founding(PlayerId(player), EntityId(9));
            assert_eq!(c.name, name);
            assert_eq!(c.portrait, portrait);
            assert_eq!(c.attribute(focus), 2);
        }
    }

    #[test]
    fn loss_fates_are_deterministic_and_cover_every_outcome() {
        let owner = PlayerId(7);
        let fleet = EntityId(41);
        let mut seen = std::collections::BTreeSet::new();
        for incident in 0..200 {
            let a = CaptainLossFate::for_loss(owner, 3, fleet, incident);
            let b = CaptainLossFate::for_loss(owner, 3, fleet, incident);
            assert_eq!(a, b);
            seen.insert(a as u8);
        }
        assert_eq!(seen.len(), 4);
        assert_eq!(CaptainLossFate::Killed.recovery_secs(), None);
    }

    #[test]
    fn titles_and_portrait_ages_cover_the_ten_level_career() {
        let titles = [
            CaptainTitle::Lieutenant,
            CaptainTitle::LieutenantCommander,
            CaptainTitle::Commander,
            CaptainTitle::Captain,
            CaptainTitle::RearAdmiral,
            CaptainTitle::ViceAdmiral,
            CaptainTitle::ViceAdmiral,
            CaptainTitle::Admiral,
            CaptainTitle::Admiral,
            CaptainTitle::FleetAdmiral,
        ];
        let ages = [
            CaptainPortraitAge::Young,
            CaptainPortraitAge::Young,
            CaptainPortraitAge::Young,
            CaptainPortraitAge::Middle,
            CaptainPortraitAge::Middle,
            CaptainPortraitAge::Middle,
            CaptainPortraitAge::Middle,
            CaptainPortraitAge::Senior,
            CaptainPortraitAge::Senior,
            CaptainPortraitAge::Senior,
        ];
        for level in 1..=CAPTAIN_LEVEL_MAX {
            assert_eq!(CaptainTitle::for_level(level), titles[(level - 1) as usize]);
            assert_eq!(
                CaptainPortraitAge::for_level(level),
                ages[(level - 1) as usize]
            );
        }
    }

    #[test]
    fn command_authority_prices_hull_type_and_number() {
        let c = Captain::founding(PlayerId(0), EntityId(9));
        assert_eq!(CaptainTitle::for_level(c.level), CaptainTitle::Lieutenant);
        assert_eq!(c.command_capacity(), 4);

        let mut formation = BTreeMap::new();
        formation.insert(ShipKind::Raider, 2);
        assert!(
            c.can_command(&formation),
            "two Interceptors exactly fill 4 points"
        );
        formation.insert(ShipKind::Scout, 1);
        assert!(
            !c.can_command(&formation),
            "ship number can exceed authority"
        );

        formation.clear();
        formation.insert(ShipKind::Destroyer, 1);
        assert!(!c.can_command(&formation), "hull type can exceed authority");
        assert_eq!(command_weight(ShipKind::Titan), 128);
        assert!(
            CaptainTitle::Admiral.command_capacity() >= command_weight(ShipKind::Titan),
            "an Admiral can command a Titan"
        );
        assert!(
            CaptainTitle::ViceAdmiral.command_capacity() < command_weight(ShipKind::Titan),
            "a Titan remains Admiral work"
        );
    }
}

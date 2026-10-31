//! The new-corporation founding programme.
//!
//! This is authoritative simulation state, not a client checklist. Milestones
//! advance from things that actually happened in the world, so reconnecting or
//! playing from another client cannot skip the opening lessons.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{build::{BuildKind, StructureKind}, cargo::Commodity, ids::{EntityId, PlayerId}, math::Vec2, ship::ShipKind};

/// Earliest point at which a corporation may deliberately end founder safety.
pub const FOUNDER_PROTECTION_MIN_S: f64 = 30.0 * 60.0;
/// Founder safety always lasts at least this long.
pub const FOUNDER_PROTECTION_BASE_S: f64 = 24.0 * 60.0 * 60.0;
/// Once the programme completes, safety continues this long (never shorter
/// than the base window), so a founder who plays once a day is not exposed the
/// moment they finish.
pub const FOUNDER_PROTECTION_AFTER_COMPLETE_S: f64 = 24.0 * 60.0 * 60.0;
/// Founder safety is never permanent, even if the programme is abandoned.
pub const FOUNDER_PROTECTION_MAX_S: f64 = 48.0 * 60.0 * 60.0;

/// §charter-v5: the raid. The privateer sets out from beyond the colony's
/// sensors on its den's side, this many sensor ranges out (never past the den
/// itself), so it is seen within seconds of the mine coming online.
pub const RAID_START_SENSOR_RANGES: f64 = 1.25;
/// §raid-course (Oct 30 2026, Peter: "The first pirate raiders seem to fly
/// towards the player's star system instead of their planet"): a raider is
/// over the colony within this of the colony's world, not on crossing its
/// gravity well's edge.
pub const RAID_OVER_COLONY_SU: f64 = 150.0;
/// Over the colony it loads this many units a second: what it takes visibly
/// drains in a quarter-minute.
pub const RAID_LOOT_PER_SEC: f64 = 4.0;
/// What it takes: every unit of these in the home stockpile.
pub const RAID_TAKES: [Commodity; 2] = [Commodity::Electronics, Commodity::Armaments];
/// It never holds orbit longer than this, whatever stops it loading.
pub const RAID_MAX_HOLD_S: f64 = 90.0;
/// After unloading at its den, it lurks this many Command Center sensor
/// ranges out on its den's side (Peter, Oct 27 2026: "maybe the pirate should
/// move just outside of the player's home system sensor range"): just past
/// where the colony saw it go, quiet enough to be found only by a hunter.
pub const RAID_LURK_SENSOR_RANGES: f64 = 1.1;

/// §founding R6: the Authority's small charter stipend for first reaching a
/// charter milestone, named for the step just finished. Credits are money,
/// never conjured goods; the physical economy stays the game. The Rogue
/// Privateer pays its own bounty instead. §charter-v5: the stipends before the
/// bounty leave the founder about 800 Cr short of buying back what the raid
/// took (50 semiconductors and 15 Armaments at the trader's markup), so the
/// ore has to be sold at the trader's discount.
pub fn charter_stipend(stage: FoundingStage) -> Option<(&'static str, f64)> {
    Some(match stage {
        FoundingStage::BuildMine => ("Colony fed and housed", 300.0),
        FoundingStage::PirateRaid => ("Mine online", 300.0),
        FoundingStage::BuildInterceptor => ("Shipyard staffed", 400.0),
        FoundingStage::DefeatPrivateer => ("Interceptor launched", 300.0),
        FoundingStage::PrepareExport => ("Privateer den eliminated", 600.0),
        FoundingStage::BuildSmelter => ("Research under way", 300.0),
        FoundingStage::ExpandMining => ("Smelter running", 300.0),
        FoundingStage::SurveyCandidates => ("Mining expanded", 400.0),
        FoundingStage::BuildBarge => ("Nearby system charted", 500.0),
        FoundingStage::ProspectBelt => ("Mining Barge launched", 300.0),
        FoundingStage::MineBelt => ("Pocket found", 300.0),
        FoundingStage::HaulOre => ("Belt mining under way", 300.0),
        FoundingStage::Complete => ("Founding charter fulfilled", 800.0),
        _ => return None,
    })
}

/// §charter-v5: the kit's semiconductors, all of which the raid takes. Tunable.
pub const KIT_SEMICONDUCTORS: f64 = 40.0;
/// §charter-v5: the kit's Armaments: one Interceptor's worth, which the raid
/// takes too. Tunable.
pub const KIT_ARMAMENTS: f64 = 15.0;
/// §charter-v4: the kit's Provisions: about five and a half minutes of food
/// for the 6,000 settlers while the farms go up, plus the Academy's 20. Tunable.
pub const KIT_PROVISIONS: f64 = 100.0;

/// §charter-crates (Peter, Oct 31 2026: "the player starts with everything
/// they need to complete the tutorial ... their storage is basically full at
/// the very beginning"): the charter's supplies, sealed by the Authority and
/// released a chapter at a time. A sealed crate takes no room. Collected, it
/// lands in the home stockpile whole, past the cap if it must (production
/// then waits), so a crate never locks away the goods for the Warehouse that
/// would make room for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharterCrate {
    /// In the colony ship's holds: open at landing.
    Landfall,
    Raid,
    Pirates,
    Expansion,
    Reach,
    Vendetta,
}

impl CharterCrate {
    pub const ALL: [Self; 6] = [Self::Landfall, Self::Raid, Self::Pirates, Self::Expansion, Self::Reach, Self::Vendetta];

    /// The step that releases it: its chapter's first build.
    pub fn released_at(self) -> FoundingStage {
        match self {
            Self::Landfall => FoundingStage::BuildBioharvester,
            Self::Raid => FoundingStage::BuildGunPlatform,
            Self::Pirates => FoundingStage::BuildInterceptor,
            Self::Expansion => FoundingStage::PrepareExport,
            Self::Reach => FoundingStage::BuildBarge,
            Self::Vendetta => FoundingStage::SecureStores,
        }
    }

    /// The builds it carries goods for. Their Semiconductors and Armaments
    /// come from the trader and the Market Hub (the raid takes the Landfall
    /// crate's), so no crate carries those for a build.
    pub fn builds(self) -> &'static [BuildKind] {
        use {BuildKind::{Ship, Upgrade}, ShipKind as H, StructureKind as K};
        match self {
            Self::Landfall => &[Upgrade { upgrade: K::Bioharvester }, Upgrade { upgrade: K::Habitat },
                Upgrade { upgrade: K::Agroplex }, Upgrade { upgrade: K::MiningComplex }],
            Self::Raid => &[Upgrade { upgrade: K::OrbitalDefense }, Upgrade { upgrade: K::PowerPlant },
                Upgrade { upgrade: K::Shipyard }],
            Self::Pirates => &[Ship { ship: H::Raider }],
            // The export's freighter, the Academy, the Smelter, the second
            // mine and its Solar Array, the probe, and two Interceptors for
            // the uncle (his oath comes at step 17).
            Self::Expansion => &[Ship { ship: H::TinyFreighter }, Upgrade { upgrade: K::Academy },
                Upgrade { upgrade: K::Smelter }, Upgrade { upgrade: K::PowerPlant }, Upgrade { upgrade: K::MiningComplex },
                Ship { ship: H::SurveyProbe }, Ship { ship: H::Raider }, Ship { ship: H::Raider }],
            Self::Reach => &[Ship { ship: H::MiningBarge }, Upgrade { upgrade: K::UraniumMill }],
            Self::Vendetta => &[Upgrade { upgrade: K::Warehouse }, Upgrade { upgrade: K::Vault }],
        }
    }

    /// What it holds: its builds' goods (`each_crate_carries_its_builds`),
    /// plus food at landing, the ship parts the raid takes, the Interceptor's
    /// first repairs and the fuel the Expansion's hulls fill with. Tunable.
    pub fn goods(self) -> &'static [(Commodity, f64)] {
        use Commodity as C;
        match self {
            Self::Landfall => &[(C::Alloys, 110.0), (C::Machinery, 47.0), (C::Polymers, 20.0),
                (C::Provisions, KIT_PROVISIONS), (C::Electronics, KIT_SEMICONDUCTORS), (C::Armaments, KIT_ARMAMENTS)],
            Self::Raid => &[(C::Alloys, 95.0), (C::Machinery, 42.0)],
            Self::Pirates => &[(C::Alloys, 30.0), (C::Machinery, 2.0), (C::WarpCore, 1.0)],
            // Its Uranium fills the Tiny Freighter, the Survey Probe and most
            // of the barge until the Uranium Mill runs.
            Self::Expansion => &[(C::Alloys, 176.0), (C::Machinery, 47.0), (C::Polymers, 8.0), (C::Provisions, 20.0),
                (C::Uranium, 100.0), (C::WarpCore, 3.0)],
            Self::Reach => &[(C::Alloys, 60.0), (C::Machinery, 28.0), (C::Polymers, 12.0), (C::WarpCore, 1.0)],
            Self::Vendetta => &[(C::Alloys, 70.0), (C::Machinery, 18.0)],
        }
    }

    pub fn stock(self) -> BTreeMap<Commodity, f64> {
        self.goods().iter().copied().collect()
    }
}

/// §charter-crates: every crate together, the whole charter's supply.
pub fn charter_supplies() -> BTreeMap<Commodity, f64> {
    let mut goods = BTreeMap::new();
    for c in CharterCrate::ALL {
        for &(good, n) in c.goods() {
            *goods.entry(good).or_default() += n;
        }
    }
    goods
}

/// §charter-v4: the privateer den. Its garrison flies the privateer's weak
/// guns: three raiders at three-quarter hull are a real fight that one
/// Interceptor wins (measured over eleven seeds: 27–54 s, at worst 45% hull
/// left). It has no defense platforms; one platform tier kills a lone
/// Interceptor. Tunable.
pub const DEN_GARRISON_RAIDERS: u32 = 3;
pub const DEN_GARRISON_HULL_FRAC: f64 = 0.75;
/// Goods left at the den when it falls, for a Freighter to recover. §den-pocket:
/// among them a sample of what it guarded.
pub const DEN_PLUNDER: [(crate::cargo::Commodity, u32); 3] = [(crate::cargo::Commodity::Alloys, 40),
    (crate::cargo::Commodity::Electronics, 15), (DEN_POCKET_ORE, 10)];
/// The den's belt is rich: its rolled deposits times this. Tunable.
pub const DEN_BELT_RICHNESS_MULT: f64 = 1.8;
/// §den-pocket (Peter, Oct 30 2026: the barge has to be worth it): what the
/// den guarded, a hidden Titanium Ore pocket in its belt, 110 Cr a unit at the
/// Hub against Ferrite's 8. The charter's barge finds it on its first
/// prospecting pass. Finite like every pocket: 600 units, about 66,000 Cr at
/// base price, near the home belt's Cuprite pocket's worth. Tunable.
pub const DEN_POCKET_ORE: crate::cargo::Commodity = crate::cargo::Commodity::TitaniumOre;
pub const DEN_POCKET_RESERVES: f64 = 600.0;
/// The den pocket, the same for every den, like [`crate::belts::home_pocket`].
pub fn den_pocket() -> crate::galaxy::Deposit {
    crate::galaxy::Deposit { resource: DEN_POCKET_ORE, richness: crate::belts::HOME_POCKET_RICHNESS,
        reserves: Some(DEN_POCKET_RESERVES), accessibility: 0.1 }
}
/// Destroying the privateer promotes the captain aboard the killing fleet: its
/// experience is topped up to this level's (the kill alone earns 85 of 100).
pub const PRIVATEER_PROMOTION_LEVEL: u8 = 2;

/// The Academy does real work for the first programme, but the founding grant
/// leaves only this much Tier-I throughput to fund. §refining (Oct 30 2026):
/// the Smelter step waits on Enrichment, so three minutes at 1× pacing (it was
/// twelve); later programmes use the ordinary season-scale costs.
pub const FIRST_RESEARCH_REMAINING_S: f64 = 3.0 * 60.0;

/// The Rogue Privateer pays money, never a conjured construction manifest. This
/// is the rounded base-market value of the retired mixed-goods bounty plus its
/// old cash award: enough purchasing power for the founding imports, while the
/// player still has to choose, buy, freight, and receive every manufactured good.
/// §vendetta (Oct 29 2026): it also pays for the trader's last call, which now
/// buys the second Interceptor's semiconductors and Armaments (1,872 Cr more,
/// so 4,500 → 6,400 keeps the old margin). §vendetta-campaign (Oct 30 2026):
/// and the third's, another 1,872, so 8,300.
pub const PRIVATEER_CREDIT_BOUNTY: f64 = 8_300.0;

/// The tutorial threat starts well downrange. Its lateral offset and slow
/// convergence make the ordinary 20,000-su Freighter contact a useful warning
/// rather than a one-second ambush at 4× pacing.
pub const PRIVATEER_LEAD_SU: f64 = 45_000.0;
/// Lateral displacement from the Freighter's actual outbound route. The threat
/// must first turn and converge instead of spawning directly in its path.
pub const PRIVATEER_ROUTE_OFFSET_SU: f64 = 25_000.0;
/// A limping tutorial Raider: barely faster than a Convoy, less than half the
/// starting Interceptor's speed. Ordinary enclave pirates remain untouched.
pub const PRIVATEER_SPEED_MULT: f64 = 0.45;
/// Start undamaged so players see combat take the privateer from full hull
/// to destruction. Tutorial safety comes from weak weapons, not missing HP.
/// Ordinary enclave pirates are unaffected.
pub const PRIVATEER_HULL_FRAC: f64 = 1.0;
/// Its improvised weapons are deliberately weak. A caught Freighter therefore
/// takes damage over time instead of being erased by the ordinary raid burst,
/// leaving a real relief window for the player's Interceptor. Full starting
/// hull gives it staying power without increasing its damage per hit.
pub const PRIVATEER_DAMAGE_MULT: f64 = 0.05;

/// §refining (Peter, Oct 30 2026): the charter's research step, Enrichment,
/// because it unlocks the Smelter the next step builds.
pub const CHARTER_RESEARCH: &str = "mat_enrichment";

/// Three legible opening choices surfaced by the guide. They remain ordinary
/// catalogue programmes: the player may ignore them and choose any open Tier I.
pub const RECOMMENDED_FIRST_RESEARCH: [&str; 3] =
    ["comp_shadow_iv_recon_suite", "prop_expedition_iii_extended_tanks", "mat_deep_bores"];

/// The founding charter (v5, `docs/founding-charter.md`), twenty-one steps in
/// six chapters ([`FoundingStage::STEPS`]):
/// - Landfall: BuildBioharvester → BuildHabitat → BuildAgroplex → BuildMine.
/// - Raid: PirateRaid (the privateer takes the kit's semiconductors and
///   Armaments) → SellOre (to the free trader, which docks after the raid) →
///   PowerShipyard → BuildShipyard.
/// - Pirates: BuildInterceptor → DefeatPrivateer → EliminateDen (its base at
///   the local belt, holding what the raid took).
/// - Expansion: PrepareExport (the first ore to the Market Hub, now the lane
///   is safe) → FirstResearch (Enrichment) → BuildSmelter (it eats more ore
///   than one mine makes) → ExpandMining (a powered mine on another home
///   world) → SurveyCandidates.
/// - Reach: BuildBarge → ProspectBelt (the den belt's Titanium pocket) →
///   MineBelt → HaulOre.
/// - Vendetta: the privateer's uncle comes for the colony
///   (`docs/pirate-vendetta.md`).
///
/// Keep enum order stable for saved identities: later stages are appended,
/// not inserted in play order. Retired stages (the Scout, the beacon, the
/// investment choice, colonies, the counterfeit, the Freighter export) advance
/// to their successors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingStage {
    BuildShipyard,
    BuildMine,
    BuildConvoy,
    /// Prepare the opening export. The first meaningful departure by ANY owned
    /// Freighter starts the privateer lesson regardless of its manifest; the
    /// Ferrite Ore sale receipt remains a separate later milestone.
    #[serde(alias = "leave_home_well")]
    ExportProduction,
    DefeatPrivateer,
    /// Wait for a Ferrite Ore sale receipt to reach the command center.
    #[serde(alias = "first_sale")]
    CompleteExport,
    BuildAcademy,
    FirstResearch,
    BuildScout,
    #[serde(alias = "survey_system")]
    SurveyCandidates,
    BuildColony,
    EstablishColony,
    Complete,
    BuildSecondFreighter,
    /// Two suggested investments, not a permanent class choice. Either a second
    /// owned freight hull or an operating researched Smelter completes the chapter.
    GrowBusiness,
    /// Version-two opening: a short, local recovery before the Hub voyage.
    LocalRecovery,
    /// §charter-v5: "Export your ore", after the den: done when the report of
    /// the first Ferrite Ore leaving for the Market Hub reaches command.
    PrepareExport,
    /// §belts: build a Mining Barge, once the surveys have charted a belt.
    BuildBarge,
    /// §belts: send it to the charted belt; done when command hears that it
    /// is cutting ore there.
    MineBelt,
    /// §charter v3: bring the barge's ore home and unload it, or sell it at
    /// the Hub; done when command hears it was delivered.
    HaulOre,
    /// §charter-v4: build and staff the first Bioharvester.
    BuildBioharvester,
    /// §charter-v4: build the first Habitat.
    BuildHabitat,
    /// §charter-v4: build and staff the first Agroplex.
    BuildAgroplex,
    /// §charter-v4: the Shipyard needs more than the Command Center's reactor:
    /// build a Solar Array.
    PowerShipyard,
    /// Retired (§charter-v5): the counterfeit's "buy real semiconductors".
    BuySemiconductors,
    /// Retired (§charter-v5): the first Freighter.
    BuildFreighter,
    /// §charter-v4: build an Interceptor (Shipyard I builds it).
    BuildInterceptor,
    /// §charter-v4: a powered, staffed mine on another world of the home system.
    ExpandMining,
    /// §charter-v4: the dead privateer's log named its den, a pirate base at
    /// the home's local belt; done when the report of its clearance reaches
    /// command.
    EliminateDen,
    /// §charter-v5: the privateer raids the colony as the mine comes online;
    /// done when the report of what it took reaches command.
    PirateRaid,
    /// §charter-v5: sell Ferrite Ore to the free trader (or it has gone).
    SellOre,
    /// §vendetta: the dead privateer's uncle comes for the colony. Done when
    /// the report of how it ended reaches command, win or lose.
    Vendetta,
    /// §refining (Peter, Oct 30 2026: why expand mining): build and staff a
    /// Smelter once Enrichment is in. It eats more ore than one mine makes,
    /// which is what the next step's second mine answers.
    BuildSmelter,
    /// §den-pocket (Peter, Oct 30 2026: the barge has to be worth it): the
    /// barge prospects the den's belt; done when the find report reaches
    /// command. The den guarded a Titanium pocket.
    ProspectBelt,
    /// §gun-platform (Peter, Oct 30 2026): straight after the first raid, a
    /// Gun Platform over the home (the station ladder's first level), from
    /// kit Alloys and Machinery. Pirates leave its worlds alone after it; the
    /// uncle's finale is fought under its gun.
    BuildGunPlatform,
    /// §charter-crates (Peter, Oct 31 2026: "a step to increase storage at
    /// the end and at the same time build the player's Vault"): after the
    /// barge's haul, before the uncle; done once a Warehouse and a Vault
    /// stand at home.
    SecureStores,
}

impl FoundingStage {
    /// §charter-v5: the charter's steps in play order (Complete follows).
    pub const STEPS: [FoundingStage; 23] = [
        Self::BuildBioharvester,
        Self::BuildHabitat,
        Self::BuildAgroplex,
        Self::BuildMine,
        Self::PirateRaid,
        Self::BuildGunPlatform,
        Self::SellOre,
        Self::PowerShipyard,
        Self::BuildShipyard,
        Self::BuildInterceptor,
        Self::DefeatPrivateer,
        Self::EliminateDen,
        Self::PrepareExport,
        Self::FirstResearch,
        Self::BuildSmelter,
        Self::ExpandMining,
        Self::SurveyCandidates,
        Self::BuildBarge,
        Self::ProspectBelt,
        Self::MineBelt,
        Self::HaulOre,
        Self::SecureStores,
        Self::Vendetta,
    ];

    /// The 1-based step, 24 for Complete; retired stages count as the step
    /// they hand on to.
    pub fn step(self) -> usize {
        let live = match self {
            Self::Complete | Self::BuildColony | Self::EstablishColony => return Self::STEPS.len() + 1,
            Self::BuySemiconductors => Self::PowerShipyard,
            Self::BuildFreighter | Self::BuildConvoy => Self::BuildInterceptor,
            Self::LocalRecovery | Self::ExportProduction => Self::PrepareExport,
            Self::CompleteExport | Self::GrowBusiness | Self::BuildSecondFreighter => Self::BuildShipyard,
            Self::BuildAcademy => Self::FirstResearch,
            Self::BuildScout => Self::SurveyCandidates,
            other => other,
        };
        Self::STEPS.iter().position(|s| *s == live).map_or(Self::STEPS.len() + 1, |i| i + 1)
    }
}

impl Default for FoundingStage {
    fn default() -> Self {
        Self::Complete
    }
}

/// §first-session (Oct 15 2026): the charter's opening buildings rise in
/// [`OPENING_BUILD_SECS`] while the charter runs: level I of each on the
/// landing world (the home's Command Center world), whatever its construction
/// speed, so the first session isn't spent waiting on them. Tunable.
pub const OPENING_BUILD_SECS: f64 = 10.0;
pub const OPENING_BUILDS: [StructureKind; 5] = [StructureKind::MiningComplex, StructureKind::Bioharvester,
    StructureKind::Habitat, StructureKind::Agroplex, StructureKind::PowerPlant];

/// §charter-gates (Oct 26 2026, Peter): the charter's first mine (step 4)
/// rises in this, the privateer's flight to the colony: it sets out as the
/// mine starts and arrives as it comes online. Mirrored by the client's
/// `market.ts`. Tunable.
pub const CHARTER_MINE_SECS: f64 = 30.0;

/// §charter-gates (Oct 26 2026, Peter: "have the tutorial gate the
/// construction of the structures"): the charter step that opens a structure
/// it builds. Until then a founder can't build it, so no Solar Array or
/// Shipyard goes up ahead of the raid. Structures the charter doesn't build
/// aren't gated here (most wait on research anyway).
pub fn charter_step(kind: StructureKind) -> Option<usize> {
    use StructureKind as K;
    let stage = match kind {
        K::Bioharvester => FoundingStage::BuildBioharvester,
        K::Habitat => FoundingStage::BuildHabitat,
        K::Agroplex => FoundingStage::BuildAgroplex,
        K::MiningComplex => FoundingStage::BuildMine,
        K::PowerPlant => FoundingStage::PowerShipyard,
        K::Shipyard => FoundingStage::BuildShipyard,
        K::Academy => FoundingStage::FirstResearch,
        K::Smelter => FoundingStage::BuildSmelter,
        K::AsteroidQuarry => FoundingStage::ExpandMining,
        // §gun-platform: the station opens after the first raid.
        K::OrbitalDefense => FoundingStage::BuildGunPlatform,
        // §charter-crates: the Vault opens with "Secure your stores".
        K::Vault => FoundingStage::SecureStores,
        K::AssemblyHall => FoundingStage::Complete,
        _ => return None,
    };
    Some(stage.step())
}

/// §charter-v5: where the raiding privateer is in its raid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaidPhase {
    /// Flying in on the colony.
    Inbound,
    /// Over the colony, loading what it takes.
    Looting,
    /// Flying home to its den with the loot.
    Returning,
    /// Unloaded; lurking between its den and the colony.
    Lurking,
}

/// §charter-v5: the privateer's one raid on a founder's colony. Persisted, so
/// a reload neither repeats it nor refills what it may take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoundingRaid {
    pub phase: RaidPhase,
    /// Where it heads home to: its den, or its starting point without one.
    pub base: Vec2,
    /// Where it lurks once home.
    pub lurk: Vec2,
    pub arrived_at: Option<f64>,
    /// What it may still take, frozen when it arrives over the colony.
    #[serde(default)]
    pub allowance: BTreeMap<Commodity, u32>,
    #[serde(default)]
    pub stolen: BTreeMap<Commodity, u32>,
    #[serde(default)]
    pub loading_credit: f64,
}

/// §vendetta: the five pirate families a founder can cross, by personality
/// (`docs/pirate-vendetta.md`; portraits in `client/art-src/pirate-portraits-v1`).
pub const PIRATE_FAMILIES: [&str; 5] = ["showboat", "hothead", "collector", "scrapper", "fatalist"];
pub const PIRATE_SURNAMES: [&str; 20] = ["Vance", "Kessler", "Marrow", "Okafor", "Drago", "Castellan",
    "Reyes", "Holt", "Varga", "Sato", "Brandt", "Mercer", "Calloway", "Novak", "Quill", "Ashby",
    "Rourke", "Szabo", "Moreau", "Dunmore"];
/// First names for the nephew (the raider) and his brother.
pub const PIRATE_NEPHEWS: [&str; 20] = ["Jace", "Rook", "Dez", "Kip", "Wren", "Corin", "Milo", "Jory",
    "Nico", "Dax", "Bram", "Ezra", "Kai", "Lazlo", "Rafe", "Tomas", "Ike", "Jonah", "Vic", "Arlo"];
pub const PIRATE_UNCLES: [&str; 20] = ["Augustin", "Silas", "Bartholomew", "Cornelius", "Mordecai",
    "Ignatius", "Lucan", "Horace", "Vasily", "Ambrose", "Ezekiel", "Thaddeus", "Casimir", "Severin",
    "Barnaby", "Gideon", "Leopold", "Ulric", "Magnus", "Ansel"];

/// §vendetta: the pirate family a corporation crosses: the raiding nephew,
/// his uncle and his brother. Dealt from the galaxy seed, the corporation and
/// its join tick, so it never needs saving and a new galaxy deals afresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PirateFamily {
    pub family: usize,
    pub surname: usize,
    pub nephew: usize,
    pub uncle: usize,
    pub brother: usize,
}

impl PirateFamily {
    pub fn of(seed: u64, owner: PlayerId, joined_tick: u64) -> Self {
        let mut rng = crate::rng::Rng::keyed_id(seed, "pirate-family-v1", owner.0 ^ joined_tick.rotate_left(32));
        let mut pick = |n: usize| (rng.next_u64() % n as u64) as usize;
        let family = pick(PIRATE_FAMILIES.len());
        let surname = pick(PIRATE_SURNAMES.len());
        let nephew = pick(PIRATE_NEPHEWS.len());
        let uncle = pick(PIRATE_UNCLES.len());
        let brother = (nephew + 1 + pick(PIRATE_NEPHEWS.len() - 1)) % PIRATE_NEPHEWS.len();
        Self { family, surname, nephew, uncle, brother }
    }
    pub fn slug(&self) -> &'static str {
        PIRATE_FAMILIES[self.family]
    }
    pub fn surname(&self) -> &'static str {
        PIRATE_SURNAMES[self.surname]
    }
    pub fn nephew_name(&self) -> String {
        format!("{} {}", PIRATE_NEPHEWS[self.nephew], self.surname())
    }
    pub fn uncle_name(&self) -> String {
        format!("{} {}", PIRATE_UNCLES[self.uncle], self.surname())
    }
    pub fn brother_name(&self) -> String {
        format!("{} {}", PIRATE_NEPHEWS[self.brother], self.surname())
    }
}

/// §vendetta: once the step begins, the uncle's pack waits this long beyond
/// the colony's sensors before it sets out: about ten minutes from the ore
/// coming home to the fight.
pub const VENDETTA_LEAD_S: f64 = 8.0 * 60.0;
/// §vendetta-campaign (Peter, Oct 30 2026: "conclude in some big battle"):
/// his flagship and four escorts, plus whichever strike raiders got away,
/// sized so three Interceptors beat them (5 galaxies of 6) and two lose (4 of
/// 6; `founding_vendetta` tests, no captains aboard). Tunable.
pub const VENDETTA_FLAGSHIP: ShipKind = ShipKind::Corvette;
pub const VENDETTA_ESCORTS: u32 = 4;
pub const VENDETTA_DAMAGE_MULT: f64 = 0.28;
/// §vendetta-campaign: the Interceptors the guide asks for before he comes.
pub const VENDETTA_DEFENDERS: usize = 3;
/// §vendetta-campaign (Peter: "I would hope the player could be online for
/// the final large battle", but never "strictly for online players"): his
/// countdown runs only while the founder is online, yet he sets out by this
/// long after the step begins regardless.
pub const VENDETTA_DEADLINE_S: f64 = 24.0 * 3_600.0;
/// §vendetta-campaign (Peter: "the uncle should harass the player's expanded
/// mining ops"): two strikes before the finale. The first comes this long
/// after his oath: at a freighter of the founder's near home, else at the
/// second mine's world.
pub const STRIKE_MINE_DELAY_S: f64 = 3.0 * 60.0;
/// The second jumps the barge this long after it starts cutting the den's
/// belt.
pub const STRIKE_BARGE_DELAY_S: f64 = 45.0;
/// A strike's countdown, like his, runs only while the founder is online, and
/// strikes by this long after it was scheduled regardless.
pub const STRIKE_DEADLINE_S: f64 = 3_600.0;
/// Raiders per strike.
pub const STRIKE_MINE_RAIDERS: u32 = 1;
pub const STRIKE_BARGE_RAIDERS: u32 = 2;
/// §gun-platform (Peter, Oct 30 2026: stolen ore "doesn't seem like an
/// equivalent revenge action to losing a nephew"): the first strike hunts a
/// freighter of the founder's in open space this near home (outside every
/// gravity well, not docked). It breaks off if the freighter docks or reaches
/// a well. With no such freighter it wrecks the second mine's world instead.
pub const STRIKE_FREIGHTER_SU: f64 = 30_000.0;
/// Over the second mine's world and unopposed, it wrecks one structure this
/// often, in this order (the mine, a belt's quarry, then the Solar Array),
/// and leaves when none is left. Any fight ends it. Tunable.
pub const STRIKE_WRECK_S: f64 = 30.0;
pub const STRIKE_WRECKS: [StructureKind; 3] =
    [StructureKind::MiningComplex, StructureKind::AsteroidQuarry, StructureKind::PowerPlant];
/// A strike's raiders all destroyed: this bounty, on the report.
pub const STRIKE_BOUNTY: f64 = 500.0;
/// How far from the belt the barge strike sets out.
pub const STRIKE_BARGE_START_SU: f64 = 5_000.0;
/// Over the colony he loads like a pirate home raid (at most a tenth of each
/// good and 60 units in all) for at most this long. §gun-platform: if nothing
/// still fights for the colony when he is done, it falls (game over).
pub const VENDETTA_HOLD_S: f64 = 120.0;
/// His death pays a bounty and lands his flagship's salvage at home.
pub const VENDETTA_BOUNTY: f64 = 6_000.0;
pub const VENDETTA_SALVAGE: [(Commodity, u32); 3] =
    [(Commodity::Alloys, 60), (Commodity::Machinery, 20), (Commodity::Armaments, 15)];

/// §vendetta: the uncle's revenge, the charter's last step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoundingVendetta {
    /// When his oath reached command (the founder reached step 16).
    pub sworn_at: f64,
    /// His pack, once raised beyond the colony's sensors, and when it sets out.
    #[serde(default)]
    pub fleet: Option<EntityId>,
    #[serde(default)]
    pub depart_at: Option<f64>,
    /// Its raid over the colony: the founding raid's phases and loot ledger.
    #[serde(default)]
    pub raid: Option<FoundingRaid>,
    /// Where the pack was last, for the light of how it ended.
    #[serde(default)]
    pub last_pos: Option<Vec2>,
    /// The pack is (or was last tick) in a fight: when it ends with his
    /// flagship alive and a defender still near home, he breaks off.
    #[serde(default)]
    pub fought: bool,
    /// True when his flagship died, false when he got away (repelled, or
    /// the colony fell).
    #[serde(default)]
    pub won: Option<bool>,
    /// §gun-platform (Peter, Oct 30 2026: "if the player hasn't built enough
    /// defenses, game over and they can restart"): he sacked the colony with
    /// nothing left fighting for it. The charter stops here until the founder
    /// restarts.
    #[serde(default)]
    pub fallen: bool,
    /// He has fought the colony's station once over it (it joins a fight
    /// with its warships on its own).
    #[serde(default)]
    pub assaulted: bool,
    /// When the report of how it ended reaches command: the step completes.
    #[serde(default)]
    pub report_at: Option<f64>,
    /// A win's bounty and salvage are paid, once.
    #[serde(default)]
    pub paid: bool,
    /// §vendetta-campaign: the latest he sets out, online or not.
    #[serde(default)]
    pub deadline_at: Option<f64>,
    /// §vendetta-campaign: his strikes on the founder's expanded mining.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strikes: Vec<VendettaStrike>,
    /// Strike raiders that got away: they ride with him at the end.
    #[serde(default)]
    pub strike_survivors: u32,
}

/// §vendetta-campaign: which of the founder's expanded mining a strike hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrikeKind {
    /// The second mine, on another world of the home system.
    Mine,
    /// The Mining Barge, cutting the den's belt.
    Barge,
    /// §gun-platform: a freighter of the founder's in open space near home,
    /// hunted instead of the second mine when one is there at launch.
    Freighter,
}

/// §gun-platform: a structure a strike raider wrecked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wreck {
    pub system: EntityId,
    pub body: u32,
    pub structure: StructureKind,
}

/// §vendetta-campaign: one strike. Its raiders set out at `at` (which slides
/// while the founder is offline, never past `deadline`), hit their target and
/// get away or die. All dead pays [`STRIKE_BOUNTY`] on the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VendettaStrike {
    pub kind: StrikeKind,
    /// The target's system and body: the mine's world, or the barge's belt.
    pub system: EntityId,
    pub body: u32,
    pub at: f64,
    pub deadline: f64,
    #[serde(default)]
    pub fleet: Option<EntityId>,
    /// Where they set out from, and go back to.
    #[serde(default)]
    pub base: Option<Vec2>,
    /// Over the mine's world since (a mine strike).
    #[serde(default)]
    pub over_since: Option<f64>,
    /// §gun-platform: the freighter a freighter strike hunts.
    #[serde(default)]
    pub target: Option<EntityId>,
    /// §gun-platform: what a mine strike wrecked, in order, and when it
    /// wrecks the next.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wrecked: Vec<StructureKind>,
    #[serde(default)]
    pub next_wreck_at: Option<f64>,
    /// Breaking off for home.
    #[serde(default)]
    pub leaving: bool,
    /// In (or just out of) a fight: once it ends with them alive, they go.
    #[serde(default)]
    pub fought: bool,
    /// Their last position, for the light of how it ended.
    #[serde(default)]
    pub last_pos: Option<Vec2>,
    /// When the report that they all died reaches command.
    #[serde(default)]
    pub killed_report_at: Option<f64>,
    /// Over: gone home, called off, or killed and paid.
    #[serde(default)]
    pub done: bool,
}

impl VendettaStrike {
    pub fn new(kind: StrikeKind, system: EntityId, body: u32, at: f64) -> Self {
        Self { kind, system, body, at, deadline: at + STRIKE_DEADLINE_S, fleet: None, base: None, over_since: None,
            target: None, wrecked: Vec::new(), next_wreck_at: None, leaving: false, fought: false, last_pos: None,
            killed_report_at: None, done: false }
    }
}

/// §vendetta: the story's turns, each a transmission on its light home.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VendettaBeat {
    /// The nephew's last words name his uncle.
    Kill,
    /// The uncle's message to his nephew, among the den's stores.
    Den,
    /// The uncle swears to come (the founder reached step 16).
    Sworn,
    /// He is over the colony.
    Arrival,
    /// His flagship died.
    Won,
    /// He got away: repelled after a fight the colony's defense survived.
    Lost,
    /// §vendetta-campaign: his raider is over the second mine.
    MineStrike,
    /// §vendetta-campaign: his raiders jump the barge.
    BargeStrike,
    /// §gun-platform: his raider hunts a freighter.
    FreighterStrike,
    /// §gun-platform: a strike raider wrecked a structure (`Vendetta::wreck`).
    Wrecked,
    /// §gun-platform: he sacked the colony with nothing left fighting for
    /// it: game over.
    Fallen,
}

/// Persistent per-corporation progress. `Default` is deliberately complete and
/// disabled: old snapshots must not be pushed backwards into onboarding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundingProgram {
    /// Old saves retain their existing arc. Only first joins opt into v2.
    #[serde(default)]
    pub local_opening: bool,
    #[serde(default)]
    pub recovery_site: Option<EntityId>,
    #[serde(default)]
    pub scout: Option<EntityId>,
    /// Arrival at command, not the time the cache was physically recovered.
    #[serde(default)]
    pub recovery_report_at: Option<f64>,
    #[serde(default)]
    pub export_dispatched_at: Option<f64>,
    #[serde(default)]
    pub local_ready_report_at: Option<f64>,
    #[serde(default)]
    pub stage: FoundingStage,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub started_at: f64,
    #[serde(default)]
    pub interceptor: Option<EntityId>,
    /// Initially the granted Tiny Freighter; rebound to whichever owned
    /// Freighter actually departs first and becomes the privateer's target.
    #[serde(default)]
    pub convoy: Option<EntityId>,
    #[serde(default)]
    pub privateer: Option<EntityId>,
    /// Legacy standalone-flight report, retained for snapshot compatibility.
    #[serde(default)]
    pub departure_report_at: Option<f64>,
    #[serde(default)]
    pub privateer_report_at: Option<f64>,
    #[serde(default)]
    pub reward_granted: bool,
    /// When the first Ferrite Ore sale receipt reaches the command center.
    /// Retains its save-field name so old snapshots migrate without a custom pass.
    #[serde(default)]
    pub sale_report_at: Option<f64>,
    /// Sale-report arrivals. Old saves may also contain Provisions; only Metallic
    /// Ore is required now. Progress follows receipt light, never Market-Hub truth.
    #[serde(default)]
    pub opening_export_reports: BTreeMap<Commodity, f64>,
    /// Legacy delivery ledger, retained for snapshot compatibility. Completing
    /// the export now requires only an executed Ore sale, not a separate delivery.
    #[serde(default)]
    pub opening_export_deliveries: BTreeSet<Commodity>,
    /// Legacy v1 physical-haul bit. Retained only for snapshot compatibility;
    /// new corporations use the Ferrite Ore sale receipt above.
    #[serde(default)]
    pub market_haul_completed: bool,
    #[serde(default)]
    pub initial_surveyed: usize,
    /// The two nearby, contrasting systems assigned to this founding chapter:
    /// population-led first, mineral-led second. Their exact geology remains
    /// survey-gated; the ids merely make the tutorial objective deterministic.
    #[serde(default)]
    pub survey_candidates: Vec<EntityId>,
    #[serde(default)]
    pub initial_owned_systems: usize,
    #[serde(default)]
    pub initial_research_completed: usize,
    /// The first programme gets one transparent founding grant; never repeated
    /// by changing queues, reconnecting, or joining/leaving a syndicate. The
    /// grant covers the first programme's basket as well as most of its clock.
    #[serde(default)]
    pub research_grant_applied: bool,
    /// Legacy colony-kit ledger. New completions grant no kit; already-awarded
    /// goods and this flag survive saves unchanged.
    #[serde(default)]
    pub colony_kit_granted: bool,
    /// Sim timestamps at which each authoritative milestone first became true.
    /// This is the live-play and headless balance instrument: no UI estimates.
    #[serde(default)]
    pub milestones: BTreeMap<FoundingStage, f64>,
    #[serde(default)]
    pub protection_broken: bool,
    /// §founding R3: the Authority's one replacement starter Freighter.
    #[serde(default)]
    pub freighter_replaced: bool,
    /// §founding R6: charter stipends already paid, by milestone.
    #[serde(default)]
    pub stipends: BTreeMap<FoundingStage, f64>,
    /// §founding R7: the latest hull that turned back from the recovery site
    /// because of a threat, and when that report reaches command. Cleared
    /// when a hull is sent to the site again.
    #[serde(default)]
    pub turned_back: Option<(EntityId, f64)>,
    /// §belts: the charted belt (system, belt body) the barge lesson points
    /// at, chosen when the surveys finish: a surveyed prospect's belt first.
    #[serde(default)]
    pub belt_target: Option<(EntityId, u32)>,
    /// §belts: when the report that a barge is cutting ore at a belt away
    /// from home reaches the command center.
    #[serde(default)]
    pub belt_report_at: Option<f64>,
    /// §charter v3: when the report that a barge delivered its ore (to a
    /// stockpile or the Market warehouse) reaches the command center.
    #[serde(default)]
    pub haul_report_at: Option<f64>,
    /// §merchant: the founding merchant has been called (once per charter).
    #[serde(default)]
    pub merchant_called: bool,
    /// §charter-v5: the privateer's raid on the colony, once launched.
    #[serde(default)]
    pub raid: Option<FoundingRaid>,
    /// §charter-v5: when command first sees the raiding privateer; from then
    /// on the colony tracks it.
    #[serde(default)]
    pub raid_seen_at: Option<f64>,
    /// §charter-v5: when the report of what the raid took reaches command.
    #[serde(default)]
    pub raid_report_at: Option<f64>,
    /// §charter-v5: Ferrite Ore sold to the free trader.
    #[serde(default)]
    pub ore_sold: u32,
    /// §charter-v4: the system of this corporation's privateer den, once the
    /// privateer has appeared (its den with it).
    #[serde(default)]
    pub den: Option<EntityId>,
    /// §charter-v4: when the report that the den fell reaches command.
    #[serde(default)]
    pub den_report_at: Option<f64>,
    /// §vendetta: the uncle's revenge, from his oath (step 16) to its end.
    #[serde(default)]
    pub vendetta: Option<FoundingVendetta>,
    /// §charter-crates: the charter crates already collected.
    #[serde(default)]
    pub crates_collected: BTreeSet<CharterCrate>,
}

impl Default for FoundingProgram {
    fn default() -> Self {
        Self {
            local_opening: false,
            recovery_site: None,
            scout: None,
            recovery_report_at: None,
            export_dispatched_at: None,
            local_ready_report_at: None,
            stage: FoundingStage::Complete,
            enabled: false,
            started_at: 0.0,
            interceptor: None,
            convoy: None,
            privateer: None,
            departure_report_at: None,
            privateer_report_at: None,
            reward_granted: false,
            sale_report_at: None,
            opening_export_reports: BTreeMap::new(),
            opening_export_deliveries: BTreeSet::new(),
            market_haul_completed: false,
            initial_surveyed: 0,
            survey_candidates: Vec::new(),
            initial_owned_systems: 0,
            initial_research_completed: 0,
            research_grant_applied: false,
            colony_kit_granted: false,
            milestones: BTreeMap::new(),
            protection_broken: false,
            freighter_replaced: false,
            stipends: BTreeMap::new(),
            turned_back: None,
            belt_target: None,
            belt_report_at: None,
            haul_report_at: None,
            merchant_called: false,
            raid: None,
            raid_seen_at: None,
            raid_report_at: None,
            ore_sold: 0,
            den: None,
            den_report_at: None,
            vendetta: None,
            crates_collected: BTreeSet::new(),
        }
    }
}

impl FoundingProgram {
    pub fn new(
        now: f64,
        initial_surveyed: usize,
        initial_owned_systems: usize,
        survey_candidates: Vec<EntityId>,
    ) -> Self {
        let first = FoundingStage::STEPS[0];
        let mut programme = Self {
            stage: first,
            enabled: true,
            started_at: now,
            initial_surveyed,
            initial_owned_systems,
            survey_candidates,
            ..Self::default()
        };
        programme.milestones.insert(first, now);
        programme
    }

    /// §charter-gates: whether the local charter lets this corporation build
    /// `kind` yet ([`charter_step`]).
    pub fn allows(&self, kind: StructureKind) -> bool {
        !self.enabled || !self.local_opening || charter_step(kind).is_none_or(|step| self.stage.step() >= step)
    }

    /// §charter-v4: the charter's land grant covers only the landing world
    /// until "Expand mining operations" opens the rest of the home system.
    pub fn land_grant_open(&self) -> bool {
        !self.enabled || !self.local_opening || self.stage.step() >= FoundingStage::ExpandMining.step()
    }

    /// The stipend this programme earns on first reaching `stage`
    /// ([`charter_stipend`]); a home without a privateer den earns none for one.
    pub fn stipend(&self, stage: FoundingStage) -> Option<(&'static str, f64)> {
        if stage == FoundingStage::PrepareExport && self.den.is_none() {
            return None;
        }
        charter_stipend(stage)
    }

    /// Advance once and stamp the fact. Keeping this mutation in one helper is
    /// what makes live telemetry and the headless harness read the same clock.
    pub fn set_stage(&mut self, next: FoundingStage, now: f64) {
        if self.stage == next {
            return;
        }
        self.stage = next;
        self.milestones.entry(next).or_insert(now);
    }

    pub fn expansion_unlocked(&self) -> bool {
        !self.enabled
            || matches!(
                self.stage,
                FoundingStage::BuildColony
                    | FoundingStage::EstablishColony
                    | FoundingStage::BuildBarge
                    | FoundingStage::ProspectBelt
                    | FoundingStage::MineBelt
                    | FoundingStage::HaulOre
                    | FoundingStage::SecureStores
                    | FoundingStage::Vendetta
                    | FoundingStage::Complete
            )
    }

    /// §charter-crates: released by this step and not yet collected.
    pub fn crate_waiting(&self, c: CharterCrate) -> bool {
        self.enabled && self.local_opening && !self.crates_collected.contains(&c)
            && self.stage.step() >= c.released_at().step()
    }

    /// §charter-crates: what the Authority still holds sealed, released or
    /// not: supplies the builds ahead can count on.
    pub fn sealed_goods(&self) -> BTreeMap<Commodity, f64> {
        let mut goods = BTreeMap::new();
        if !self.enabled || !self.local_opening {
            return goods;
        }
        for c in CharterCrate::ALL.into_iter().filter(|c| !self.crates_collected.contains(c)) {
            for &(good, n) in c.goods() {
                *goods.entry(good).or_default() += n;
            }
        }
        goods
    }

    /// Founder safety has a minimum clock and also cannot be voluntarily ended
    /// before the privateer outcome's light reaches the command center. It then
    /// lasts until the player chooses aggression or [`Self::protection_until`].
    /// §gun-platform: the colony fell to the uncle's finale and the news is
    /// home: game over until the founder restarts.
    pub fn fallen(&self, now: f64) -> bool {
        self.vendetta.as_ref().is_some_and(|v| v.fallen && v.report_at.is_some_and(|at| at <= now))
    }

    pub fn protected(&self, now: f64) -> bool {
        self.enabled && !self.protection_broken && now < self.protection_until()
    }

    /// When founder safety ends on its own: 24 hours after completing the
    /// programme (never before the 24-hour base), and always by the 48-hour
    /// ceiling. An unfinished programme keeps the full ceiling.
    pub fn protection_until(&self) -> f64 {
        let ceiling = self.started_at + FOUNDER_PROTECTION_MAX_S;
        match self.milestones.get(&FoundingStage::Complete) {
            Some(done) => (done + FOUNDER_PROTECTION_AFTER_COMPLETE_S)
                .max(self.started_at + FOUNDER_PROTECTION_BASE_S)
                .min(ceiling),
            None => ceiling,
        }
    }

    /// §founding R8: the Landfall chapter, from settling until the raid's
    /// report (§charter-v5; before it, the first export) reaches command.
    /// Contract offers stay few and patient here.
    pub fn landfall_focus(&self) -> bool {
        self.enabled
            && self.local_opening
            && self.raid_report_at.is_none()
            && self.export_dispatched_at.is_none()
            && self.stage != FoundingStage::Complete
    }

    pub fn may_break_protection(&self, now: f64) -> bool {
        self.enabled
            && now >= self.started_at + FOUNDER_PROTECTION_MIN_S
            && self.privateer_report_at.is_some_and(|at| now >= at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn founder_safety_ends_only_by_eligible_aggression_or_the_hard_ceiling() {
        let mut programme = FoundingProgram::new(10.0, 0, 1, Vec::new());
        let after_min = programme.started_at + FOUNDER_PROTECTION_MIN_S + 1.0;

        assert!(programme.protected(after_min));
        assert!(!programme.may_break_protection(after_min));

        programme.privateer_report_at = Some(after_min - 1.0);
        assert!(programme.protected(after_min));
        assert!(programme.may_break_protection(after_min));

        programme.protection_broken = true;
        assert!(!programme.protected(after_min));

        programme.protection_broken = false;
        assert!(!programme.protected(programme.started_at + FOUNDER_PROTECTION_MAX_S));
    }

    #[test]
    fn founder_safety_outlasts_the_programme_by_a_day_within_the_ceiling() {
        let hour = 3600.0;
        let mut programme = FoundingProgram::new(0.0, 0, 1, Vec::new());
        assert_eq!(programme.protection_until(), 48.0 * hour, "unfinished: full ceiling");
        programme.set_stage(FoundingStage::Complete, 2.0 * hour);
        assert_eq!(programme.protection_until(), 26.0 * hour, "done early: a day after completion");
        assert!(programme.protected(25.0 * hour) && !programme.protected(26.0 * hour));
        let mut late = FoundingProgram::new(0.0, 0, 1, Vec::new());
        late.set_stage(FoundingStage::Complete, 40.0 * hour);
        assert_eq!(late.protection_until(), 48.0 * hour, "never past the ceiling");
    }
}

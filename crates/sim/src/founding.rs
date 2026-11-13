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
/// Founder safety lasts this long from founding, charter finished or not
/// (Peter, Nov 10 2026: no incentive "to slow play and drag out their tutorial
/// completion"). It used to run until a day after completion, up to 48 hours,
/// which paid an unfinished charter the most.
pub const FOUNDER_PROTECTION_S: f64 = 24.0 * 60.0 * 60.0;

/// §charter-v5: the raid. The privateer sets out from beyond the colony's
/// sensors on its den's side, this many sensor ranges out (never past the den
/// itself). §smelter-raid (Nov 9 2026): it sets out as the Smelter's order
/// goes in, so it is seen about as the Smelter stands (a 20 s build).
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
/// §shipbuilding-overhaul (Peter, Nov 6 2026: "the first pirate raid takes the
/// player's stash of warp engines"): the Fission Cores first, then the
/// Semiconductors and Weapon Mounts as before. It leaves the Uranium.
pub const RAID_TAKES: [Commodity; 4] = [Commodity::FissionReactor, Commodity::ZephyrDrive, Commodity::Electronics, Commodity::Armaments];
/// It never holds orbit longer than this, whatever stops it loading.
pub const RAID_MAX_HOLD_S: f64 = 90.0;
/// A home without a den (an overflow home): after its raid the privateer
/// lurks this many Command Center sensor ranges out on the side it came from
/// (Peter, Oct 27 2026: "maybe the pirate should move just outside of the
/// player's home system sensor range"), quiet enough to be found only by a
/// hunter. §den-retreat (Peter, Nov 12 2026: "let's have the first pirate
/// raid retreat back to their den instead of the hunting area"): with a den,
/// it goes home and joins the garrison there.
pub const RAID_LURK_SENSOR_RANGES: f64 = 1.1;

/// §founding R6: the Authority's small charter stipend for first reaching a
/// charter milestone, named for the step just finished. Credits are money,
/// never conjured goods; the physical economy stays the game. The Rogue
/// Privateer pays its own bounty instead. §charter-v5: the start and the
/// stipends before the bounty leave the founder a few dozen Cr short of the
/// trader's parts for the Solar Array, the Shipyard and the Interceptor (the
/// raid took the kit's semiconductors and Armaments), so the ore has to be
/// sold at the trader's discount. Peter (Nov 5 2026): no Pirates crate;
/// "Shipyard staffed" pays 800 more for the first Interceptor's 20 Alloys
/// from the trader (≈780 at its markup).
///
/// §three-stipends (Peter, Nov 6 2026: "There are too many milestones, let's
/// have it when the Mine is staffed and online, so make that like 2000cr and
/// the Smelter Running and the 2nd Mine Running. Remove the other ones."):
/// three, 2,000 each. With the trader at twice the Hub
/// ([`crate::merchant::FOUNDING_MARKUP`]) and §smelt-from-landing the Alloys
/// smelted at home, the start and the mine's and the Smelter's stipends buy
/// the trader's parts for the Solar Array, the Shipyard and the Interceptor
/// with room to spare; the second mine's, with the privateer's bounty and the
/// den's contract, buy the parts after the den. The den pays through the
/// Authority's contract on it ([`DEN_CONTRACT_CREDITS`]), the privateer
/// through its bounty.
pub fn charter_stipend(stage: FoundingStage) -> Option<(&'static str, f64)> {
    Some(match stage {
        // §charter-v6: the mine's step hands on to the Smelter, the Smelter's
        // to the raid.
        FoundingStage::BuildSmelter => ("Mine online", 2_000.0),
        FoundingStage::PirateRaid => ("Smelter running", 2_000.0),
        FoundingStage::SurveyCandidates => ("Second mine running", 2_000.0),
        _ => return None,
    })
}

/// §charter-v5: the kit's semiconductors, all of which the raid takes.
/// §stash (Peter, Nov 2 2026): 35, so the raid's take with the kit's 15
/// Armaments was exactly one Tiny Freighter's hold. Peter (Nov 5 2026: "Let's
/// have the Pirate raid only take 25 units"): 20, with 5 Armaments. Tunable.
pub const KIT_SEMICONDUCTORS: f64 = 20.0;
/// §charter-v6 (Nov 7 2026): the Solar Array is built at step 4, before the
/// raid, so the kit carries its Semiconductors on top of the raid's
/// (`POWER_PLANT_RECIPE`; `the_kit_carries_the_solar_arrays_semiconductors`).
pub const PLANT_SEMICONDUCTORS: f64 = 8.0;
/// §charter-v5: the kit's Armaments, which the raid takes too. Tunable.
pub const KIT_ARMAMENTS: f64 = 5.0;
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
///
/// Peter (Nov 4 2026): no crate after the raid ("it doesn't make sense"):
/// its Laser Battery, Solar Array and Shipyard goods land with the colony.
/// Peter (Nov 5 2026: "just give the player another warp core to start"): no
/// Pirates crate either. The first Interceptor's Warp Core lands with the
/// colony; its Alloys come from the trader with its Semiconductors and
/// Armaments. Peter (Nov 5 2026: "the pirate den to hold what the player
/// needs for the next steps"): no Expansion crate. The den holds them
/// ([`DEN_STORES`]); the Tiny Freighter that hauls them comes with its Warp
/// Core at landing and its parts and fuel from the trader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharterCrate {
    /// In the colony ship's holds: open at landing.
    Landfall,
    Reach,
    Vendetta,
    /// §progression (Nov 11 2026): the Expansion chapter's own builds, which
    /// home production paid for before output fell twentyfold.
    Expansion,
}

impl CharterCrate {
    pub const ALL: [Self; 4] = [Self::Landfall, Self::Reach, Self::Vendetta, Self::Expansion];

    /// The step that releases it: its chapter's first build.
    pub fn released_at(self) -> FoundingStage {
        match self {
            // §habitat-first: the charter's first step.
            Self::Landfall => FoundingStage::BuildHabitat,
            Self::Reach => FoundingStage::BuildBarge,
            Self::Vendetta => FoundingStage::SecureStores,
            // The Academy: the first Expansion build no earlier crate covers.
            Self::Expansion => FoundingStage::FirstResearch,
        }
    }

    /// The builds it carries goods for. Their Semiconductors and Armaments
    /// come from the trader and the Market Hub (the raid takes the Landfall
    /// crate's), so no crate carries those for a build. The first Interceptor
    /// and the Tiny Freighter are no crate's: only their Warp Cores land with
    /// the colony. Nothing lands spare for a later step: the guide's prefill
    /// and the bots would spend it on the next build (§den-stores, Nov 5
    /// 2026: the Academy's Alloys went to the Interceptor and the Smelter
    /// waited on Alloys nothing nearby sold).
    pub fn builds(self) -> &'static [BuildKind] {
        use {BuildKind::{Ship, Upgrade}, ShipKind as H, StructureKind as K};
        match self {
            // §smelt-from-landing (Nov 6 2026): and the Smelter.
            Self::Landfall => &[Upgrade { upgrade: K::Bioharvester }, Upgrade { upgrade: K::Habitat },
                Upgrade { upgrade: K::Agroplex }, Upgrade { upgrade: K::MiningComplex },
                Upgrade { upgrade: K::OrbitalDefense }, Upgrade { upgrade: K::PowerPlant }, Upgrade { upgrade: K::Shipyard },
                Upgrade { upgrade: K::Smelter }],
            Self::Reach => &[Ship { ship: H::MiningBarge }, Upgrade { upgrade: K::UraniumMill }],
            Self::Vendetta => &[Upgrade { upgrade: K::Warehouse }, Upgrade { upgrade: K::Vault }],
            // The second world's mine may be a Quarry on a belt: its goods
            // cover a Mining Complex too.
            Self::Expansion => &[Upgrade { upgrade: K::Academy }, Upgrade { upgrade: K::PowerPlant },
                Upgrade { upgrade: K::AsteroidQuarry }, Ship { ship: H::SurveyProbe }],
        }
    }

    /// What it holds: its builds' goods (`each_crate_carries_its_builds`),
    /// plus food at landing, the ship parts the raid takes, the Interceptor's
    /// first repairs and the barge's fuel. Tunable.
    pub fn goods(self) -> &'static [(Commodity, f64)] {
        use Commodity as C;
        match self {
            // §hull-ladder (Peter, Nov 7 2026: the raid takes "the players
            // drives and reactors because those are obviously the most
            // valuable items"): six Fission Reactors — the Interceptor's one,
            // the Tiny Freighter's two and the barge's three — and seven
            // Zephyr Drives — every charter hull's less the trader's three —
            // which the raid takes and the den gives back; and the
            // Interceptor's and the Tiny Freighter's kit parts (a Nav Computer
            // and Hull Sections each, the Interceptor's six Pulse Lasers, the
            // Tiny's Cargo Bay). §smelt-from-landing: the Smelter's 30 Alloys
            // and 15 Machinery too. §sell-alloys (Nov 11 2026): and the most
            // the trader takes at step 9, so the sale never costs the Shipyard.
            Self::Landfall => &[(C::Alloys, 235.0 + crate::merchant::BUY_CAP as f64), (C::Machinery, 104.0), (C::Polymers, 20.0),
                (C::Provisions, KIT_PROVISIONS), (C::Electronics, KIT_SEMICONDUCTORS + PLANT_SEMICONDUCTORS), (C::Armaments, KIT_ARMAMENTS),
                (C::FissionReactor, 6.0), (C::ZephyrDrive, 7.0), (C::NavComputer, 2.0), (C::HullSections, 3.0),
                (C::PulseLaser, 6.0), (C::CargoBay, 1.0),
                // §hull-ladder (Nov 7 2026): the Tiny Freighter's plating.
                (C::Composites, 4.0)],
            // The barge's parts and the Uranium Mill's goods; its three
            // reactors and drives are the den's recovered six and the kit's
            // seven. Its Uranium fills most of the barge's tank until the
            // Uranium Mill runs (the Expansion crate's did, Nov 5 2026).
            // §hull-ladder (Nov 7 2026): the barge's frame, plating and
            // precision parts from its regenerated kit.
            Self::Reach => &[(C::Alloys, 40.0), (C::Machinery, 18.0), (C::NavComputer, 1.0),
                (C::HullSections, 9.0), (C::CargoBay, 1.0), (C::Composites, 18.0), (C::PrecisionComponents, 2.0),
                (C::Uranium, 50.0)],
            Self::Vendetta => &[(C::Alloys, 70.0), (C::Machinery, 18.0)],
            // The Academy, the second world's Solar Array and Quarry, the
            // Survey Probe; their Semiconductors are the trader's.
            Self::Expansion => &[(C::Alloys, 106.0), (C::Machinery, 30.0), (C::Polymers, 10.0), (C::Provisions, 20.0)],
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

/// §den-stores (Peter, Nov 5 2026: "the pirate den to hold what the player
/// needs for the next steps and if they need to make two trips in their
/// freighter so be it!"): what the den holds beside the raid's take, from the
/// day it rises. The Machinery of the second mine's Solar Array and Mining
/// Complex, the Small Freighter and a unit for repairs; the Warp Cores of the
/// Small Freighter and the uncle's two Interceptors and, with the raid's 5,
/// the Interceptors' 30 Armaments. Peter (Nov 6 2026: "it should really have
/// high value items only"; "max 100 units of loot so 2 trips"): no Alloys,
/// the den's one cheap bulk good (§smelt-from-landing: the colony smelts its
/// own). With the raid's 25, 94 units: two Tiny Freighter loads (50, 44).
/// Tunable.
/// §shipbuilding-overhaul (Nov 6 2026): the cores are the raid's six, so the
/// den stores none of its own. With the raid's 31, 97 units: two Tiny
/// Freighter loads (50, 47).
/// §hull-ladder (Nov 7 2026): the raid's take grew to 38 (6 reactors, 7
/// drives, 20 Semiconductors, 5 Weapon Mounts), so the den's own Machinery
/// drops to 37: 100 units in all, two Tiny Freighter loads (50, 50).
/// §hold-room (Peter, Nov 10 2026: "I don't like how the player has to make 2
/// freighter trips to the den"): by room the 100 units fill 41.75 of a Tiny
/// Freighter's 50, one trip.
pub const DEN_STORES: [(Commodity, u32); 2] = [
    (Commodity::Machinery, 37),
    (Commodity::Armaments, 25),
];
/// §den-stores: what a hold takes first from the den, in the order the
/// charter needs it: the Academy's semiconductors and the Machinery, then the
/// uncle's fight. Goods not listed follow in commodity order.
pub const STASH_LOAD_ORDER: [Commodity; 5] =
    [Commodity::Electronics, Commodity::Machinery, Commodity::FissionReactor, Commodity::ZephyrDrive, Commodity::Armaments];

/// §charter-v4: the privateer den. Its garrison flies the privateer's weak
/// guns: three raiders at three-quarter hull were a real fight that one
/// Interceptor wins (measured over eleven seeds: 27–54 s, at worst 45% hull
/// left). It has no defense platforms; one platform tier kills a lone
/// Interceptor. §den-retreat (Nov 12 2026): two, since the privateer comes
/// home after its raid and joins them, so the den holds three again. Tunable.
pub const DEN_GARRISON_RAIDERS: u32 = 2;
pub const DEN_GARRISON_HULL_FRAC: f64 = 0.75;
/// §stash (Peter, Nov 2 2026: "why not just reduce the pirate den's stash to
/// 50 units exactly"): the den holds only what the raid took, one Tiny
/// Freighter's hold. When it falls its reactor blows: every hull of the
/// founder's in the den system is left at most this much of its hull, so the
/// Interceptor comes home for the Shipyard's repairs (Peter: "damaged around
/// 75%"). Tunable.
pub const DEN_BLAST_HULL_FRAC: f64 = 0.25;
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
/// and the third's, another 1,872, so 8,300. §stash (Nov 2 2026): the den's
/// stash brings back 35 semiconductors and 15 Armaments the last call no
/// longer buys (3,045 Cr at the trader's price), so 5,300 keeps the margin.
/// §den-contract (Nov 5 2026): the trader stays for the whole charter, so it
/// buys those goods as each build comes up instead.
/// §hull-ladder (Nov 7 2026): with a light engine at 1,000 the Tiny Freighter's
/// pair of reactors and drives from the trader is 4,000; the bounty covers it
/// and its parts.
/// §privateer-contract (Nov 9 2026): the Authority's contract on him pays it
/// (`World::post_founding_privateer_contract`), on his kill's report.
pub const PRIVATEER_CREDIT_BOUNTY: f64 = 8_000.0;

/// §den-contract (Peter, Nov 5 2026: "destroying the privateer provides
/// coordinates to the pirate den and it is noticed that there is an Authority
/// contract to eliminate the pirate's den by his name"): what the Authority's
/// contract on the den pays, once its fall reaches command. It replaced the
/// den's charter stipend, at the same sum: with the bounty it pays for the
/// Tiny Freighter's parts and fuel and the trader's goods for the builds
/// after the den. Tunable.
/// §hull-ladder: raised with the engine price, then again (5,000 → 8,000)
/// when the Small Freighter's kit moved to the trader (`merchant::FOUNDING_STOCK`).
pub const DEN_CONTRACT_CREDITS: f64 = 8_000.0;
/// §den-contract: a tier-1 bounty's standing (`World::refresh_operations`).
pub const DEN_CONTRACT_STANDING: f64 = 2.0;

/// The tutorial threat starts well downrange. Its lateral offset and slow
/// convergence make the ordinary 20,000-su Freighter contact a useful warning
/// rather than a one-second ambush at 4× pacing.
pub const PRIVATEER_LEAD_SU: f64 = 45_000.0;
/// Lateral displacement from the Freighter's actual outbound route. The threat
/// must first turn and converge instead of spawning directly in its path.
pub const PRIVATEER_ROUTE_OFFSET_SU: f64 = 25_000.0;
/// A limping tutorial Raider: barely faster than a Convoy, less than half the
/// starting Interceptor's speed. Ordinary enclave pirates remain untouched.
/// §reactors-drives: the privateer flies a Maelstrom Drive (×1.6 rated), so
/// the multiplier is 0.45 / 1.6 — the same 0.45 of a Zephyr Interceptor's
/// speed the chase was measured at.
pub const PRIVATEER_SPEED_MULT: f64 = 0.45 / 1.6;
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
/// because it unlocked the Smelter. §freight-frames (Peter, Nov 6 2026: "give
/// me ideas where to introduce the player to the research mechanics to unlock
/// something", then option 1): the Smelter needs no research now, so the
/// step researches Freight Frames, whose Small Freighter (three Tiny holds)
/// the step after next builds for the Hub runs.
pub const CHARTER_RESEARCH: &str = "prop_freight_frames";

/// Three legible opening choices surfaced by the guide. They remain ordinary
/// catalogue programmes: the player may ignore them and choose any open Tier I.
pub const RECOMMENDED_FIRST_RESEARCH: [&str; 3] =
    ["comp_shadow_iv_recon_suite", "prop_expedition_iii_extended_tanks", "mat_deep_bores"];

/// The founding charter (v6, `docs/founding-charter.md`), twenty-nine steps in
/// six chapters ([`FoundingStage::STEPS`]):
/// - Landfall: BuildHabitat (unpowered) → PowerShipyard (the Solar Array:
///   everything powered waits for it) → BuildBioharvester → BuildAgroplex →
///   BuildMine → BuildSmelter (§smelt-from-landing, Nov 6 2026: the colony
///   makes its own Alloys).
/// - Raid: PirateRaid (the privateer takes the freighter's engines) →
///   BuildGunPlatform → SellOre (Alloys to the free trader, which docks after
///   the raid; Nov 11 2026, no longer ore) → BuildShipyard.
/// - Pirates: BuildInterceptor → DefeatPrivateer → EliminateDen (its base at
///   the local belt, holding what the raid took).
/// - Expansion: RepairInterceptor → BuildTinyFreighter → FullSpeed → HaulLoot
///   (the den's loot home on the first freighter) → PrepareExport (the first
///   ore to the Market Hub, now the lane is safe) → FirstResearch (Freight
///   Frames) → AnswerSignal (while it runs) → BuildSmallFreighter (the hull
///   it unlocks) → ExpandMining (a powered mine on another home world) →
///   SurveyCandidates.
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
    /// §charter-v5: the first export, after the den: done when the report of
    /// the first cargo leaving for the Market Hub reaches command.
    /// §charter-v6 (Nov 7 2026): "Export your Alloys" — the Smelter's, the
    /// charter's promise.
    PrepareExport,
    /// §belts: build a Mining Barge, once the surveys have charted a belt.
    BuildBarge,
    /// §belts: send it to the charted belt; done when command hears that it
    /// is cutting ore there.
    MineBelt,
    /// §charter v3: bring the barge's ore home and unload it, or sell it at
    /// the Hub; done when command hears it was delivered.
    HaulOre,
    /// §charter-v4: build and staff the first Bioharvester. §habitat-first
    /// (Nov 8 2026): step 3, on the Solar Array's power (1 MW).
    BuildBioharvester,
    /// §charter-v4: build the first Habitat. §habitat-first (Nov 8 2026):
    /// step 1; it draws no power.
    BuildHabitat,
    /// §charter-v4: build and staff the first Agroplex. §habitat-first: step
    /// 4 (1 MW).
    BuildAgroplex,
    /// §charter-v4: build a Solar Array. §charter-v6 (Peter, Nov 7 2026):
    /// before the mine — the Command Center's reactor runs the colony, not the
    /// mine or the Smelter ("Build a Solar Array"). §habitat-first (Nov 8
    /// 2026): step 2, before the farms, which draw power too.
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
    /// §charter-v5: the privateer raids the colony; done when the report of
    /// what it took reaches command. §habitat-first (Peter, Nov 8 2026: "the
    /// first pirate raid should arrive after the smelter is built"): it sets
    /// out when this step opens, the Smelter built and staffed; the mine is an
    /// ordinary opening build (it rose in the privateer's flight from Oct 26).
    PirateRaid,
    /// §charter-v5: sell to the free trader (or it has gone). §sell-alloys
    /// (Nov 11 2026): Alloys, not ore; the id keeps its name for saves.
    SellOre,
    /// §vendetta: the dead privateer's uncle comes for the colony. Done when
    /// the report of how it ended reaches command, win or lose.
    Vendetta,
    /// §refining (Peter, Oct 30 2026: why expand mining): build and staff a
    /// Smelter. It eats more ore than one mine makes, which is what the
    /// second mine answers. §smelt-from-landing (Peter, Nov 6 2026): no
    /// research, so the colony smelts its own Alloys from the Interceptor on;
    /// the colony ship carries its goods. Peter (Nov 6 2026: "maybe we should
    /// do it before the shipyard", then "before the solar array too"): right
    /// after the ore sale. Its 2 MW share the Command Center's 1 MW with the
    /// mine, everything slowed to a third, until the next step's Solar Array.
    BuildSmelter,
    /// §den-pocket (Peter, Oct 30 2026: the barge has to be worth it): the
    /// barge prospects the den's belt; done when the find report reaches
    /// command. The den guarded a Titanium pocket.
    ProspectBelt,
    /// §gun-platform (Peter, Oct 30 2026): straight after the first raid, a
    /// Laser Battery at the home (the station ladder's first level, on the ground), from
    /// kit Alloys and Machinery. Pirates leave its worlds alone after it; the
    /// uncle's finale is fought under its gun.
    BuildGunPlatform,
    /// §charter-crates (Peter, Oct 31 2026: "a step to increase storage at
    /// the end and at the same time build the player's Vault"): after the
    /// barge's haul, before the uncle; done once a Warehouse and a Vault
    /// stand at home.
    SecureStores,
    /// §stash (Peter, Nov 3 2026: "split it for this hauling exercise"):
    /// after the den, the Interceptor its dying blast mauled comes home and
    /// the Shipyard repairs it. Done when every hull the blast hurt is whole.
    RepairInterceptor,
    /// §stash: the corporation's first freighter. Done when it owns one.
    BuildTinyFreighter,
    /// §stash: the freighter hauls back the den's loot (what the raid took,
    /// one Tiny Freighter's hold). Done when it is home, or lost on the way.
    /// The three overlap: a later one already done completes when reached.
    HaulLoot,
    /// §full-speed (Peter, Nov 4 2026): before the haul, a freighter's
    /// Transit to Full speed (a new one flies Slow on Uranium). Done when one
    /// flies Full, or the loot is already aboard. It overlaps like the stash's.
    FullSpeed,
    /// §signals (Peter, Nov 4 2026): while the charter's research runs, a
    /// captained fleet answers a signal near home. Done once one is on its
    /// way, or with no signal or captain to send; bots skip it.
    AnswerSignal,
    /// §freight-frames (Peter, Nov 6 2026): the hull Freight Frames unlocks.
    /// Done when the corporation owns a Small Freighter.
    BuildSmallFreighter,
}

impl FoundingStage {
    /// §charter-v5: the charter's steps in play order (Complete follows).
    /// §charter-v6 (Peter, Nov 7 2026): the goal is spoken first — mine ore,
    /// smelt Alloys, ship them to the Market Hub — and the Command Center's
    /// reactor runs the colony, not the mine or the Smelter, so the Solar
    /// Array comes before them; the raid that takes the freighter's engines
    /// breaks the plan at step 7. §habitat-first (Nov 8 2026): the settlers'
    /// home first (unpowered: the colony ship's reactor keeps its people
    /// alive), then the Solar Array, then the food chain, which draws power
    /// like the mine (`power::powered`).
    pub const STEPS: [FoundingStage; 29] = [
        Self::BuildHabitat,
        Self::PowerShipyard,
        Self::BuildBioharvester,
        Self::BuildAgroplex,
        Self::BuildMine,
        Self::BuildSmelter,
        Self::PirateRaid,
        Self::BuildGunPlatform,
        Self::SellOre,
        Self::BuildShipyard,
        Self::BuildInterceptor,
        Self::DefeatPrivateer,
        Self::EliminateDen,
        Self::RepairInterceptor,
        Self::BuildTinyFreighter,
        Self::FullSpeed,
        Self::HaulLoot,
        Self::PrepareExport,
        Self::FirstResearch,
        Self::AnswerSignal,
        Self::BuildSmallFreighter,
        Self::ExpandMining,
        Self::SurveyCandidates,
        Self::BuildBarge,
        Self::ProspectBelt,
        Self::MineBelt,
        Self::HaulOre,
        Self::SecureStores,
        Self::Vendetta,
    ];

    /// The 1-based step, 30 for Complete; retired stages count as the step
    /// they hand on to.
    pub fn step(self) -> usize {
        let live = match self {
            Self::Complete | Self::BuildColony | Self::EstablishColony => return Self::STEPS.len() + 1,
            Self::BuySemiconductors => Self::BuildShipyard,
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

/// §export-follow (Peter, Nov 2 2026: "the player has to follow the tiny
/// freighter for 90 seconds max"): the first shipment is followed out of the
/// well, from its dock to the hyperlimit (under 45 s at a Tiny Freighter's
/// 40 su/s from any orbit inside the 900 su well), where it warps and the
/// step completes on its light home. This caps the follow: the step completes
/// this long after departure even if the ship has not cleared the well.
/// Mirrored by the client's `founding.ts`. Tunable.
pub const EXPORT_FOLLOW_MAX_S: f64 = 90.0;

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
    /// Home: §den-retreat, with its den's garrison (its own fleet now), or
    /// without a den, lurking near the colony.
    Lurking,
}

/// §charter-v5: the privateer's one raid on a founder's colony. Persisted, so
/// a reload neither repeats it nor refills what it may take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoundingRaid {
    pub phase: RaidPhase,
    /// Where it heads home to: its den's garrison, or its starting point
    /// without one.
    pub base: Vec2,
    /// Where it waits once home: §den-retreat, its den (the same point as
    /// `base`); without a den, just outside the colony's sensors.
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
    /// §export-follow (Peter, Nov 2 2026): when the loaded freighter first
    /// left its dock for the Hub; the follow's clock. [`EXPORT_FOLLOW_MAX_S`]
    /// after it the step completes whatever the ship has done.
    #[serde(default)]
    pub export_departed_at: Option<f64>,
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
    /// §trader-call (Peter, Nov 5 2026: "Have the trader start traveling to
    /// the player's system once the pirate has disappeared from the galaxy
    /// map into the 'hunt area'"): when the colony lost sight of the
    /// privateer after the raid's report, as its last light reached command.
    /// The trader sets out then.
    #[serde(default)]
    pub raid_lost_at: Option<f64>,
    /// Alloys sold to the free trader (step 9; Nov 11 2026, no longer ore).
    #[serde(default)]
    pub alloys_sold: u32,
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
    /// §stash: what the den held when it fell (what the raid took and
    /// §den-stores its stores), for the first Tiny Freighter to bring home.
    /// What a load takes leaves it: the rest waits at the den.
    #[serde(default)]
    pub stash: BTreeMap<Commodity, u32>,
    /// §stash: the freighter carrying the stash home, once it has loaded.
    /// §den-stores: the current load's, cleared when it berths at home.
    #[serde(default)]
    pub stash_fleet: Option<EntityId>,
    /// §den-stores: what the current load carries home; the rest of
    /// [`Self::stash`] still waits at the den.
    #[serde(default)]
    pub stash_aboard: BTreeMap<Commodity, u32>,
    /// §stash: when the stash reached home, or its freighter was lost.
    #[serde(default)]
    pub stash_home_at: Option<f64>,
    /// §stash: the fleets the den's blast left damaged, until each is whole.
    #[serde(default)]
    pub den_wounded: BTreeSet<EntityId>,
}

impl Default for FoundingProgram {
    fn default() -> Self {
        Self {
            local_opening: false,
            recovery_site: None,
            scout: None,
            recovery_report_at: None,
            export_dispatched_at: None,
            export_departed_at: None,
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
            raid_lost_at: None,
            alloys_sold: 0,
            den: None,
            den_report_at: None,
            vendetta: None,
            crates_collected: BTreeSet::new(),
            stash: BTreeMap::new(),
            stash_fleet: None,
            stash_aboard: BTreeMap::new(),
            stash_home_at: None,
            den_wounded: BTreeSet::new(),
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
    /// ([`charter_stipend`]). §den-contract: the den pays through its
    /// contract, so a home without one loses nothing here.
    pub fn stipend(&self, stage: FoundingStage) -> Option<(&'static str, f64)> {
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

    /// When founder safety ends on its own: [`FOUNDER_PROTECTION_S`] after
    /// founding, however far the charter has got. Nothing a player wants may
    /// depend on staying in the tutorial.
    pub fn protection_until(&self) -> f64 {
        self.started_at + FOUNDER_PROTECTION_S
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
        assert!(!programme.protected(programme.started_at + FOUNDER_PROTECTION_S));
    }

    /// Peter (Nov 10 2026): no reward for dragging out the tutorial. Safety is
    /// a flat day from founding, finished, unfinished or finished late.
    #[test]
    fn founder_safety_is_a_flat_day_however_far_the_charter_got() {
        let hour = 3600.0;
        let unfinished = FoundingProgram::new(0.0, 0, 1, Vec::new());
        assert_eq!(unfinished.protection_until(), 24.0 * hour, "unfinished: no longer");
        assert!(unfinished.protected(23.9 * hour) && !unfinished.protected(24.0 * hour));
        let mut early = FoundingProgram::new(0.0, 0, 1, Vec::new());
        early.set_stage(FoundingStage::Complete, 2.0 * hour);
        assert_eq!(early.protection_until(), 24.0 * hour, "done early: no shorter");
        let mut late = FoundingProgram::new(0.0, 0, 1, Vec::new());
        late.set_stage(FoundingStage::Complete, 40.0 * hour);
        assert_eq!(late.protection_until(), 24.0 * hour, "done late: no longer");
        assert!(!late.protected(40.0 * hour));
    }
}

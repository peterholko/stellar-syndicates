//! Ships: the mobile entities of the galaxy.
//!
//! Civilian and military hulls differ in formation speed, mass, cargo, sensors,
//! and combat role. Those ordinary properties create the convoy/escort/raider
//! tradeoffs; there is no special chase rule.
//!
//! Every fleet acts on a standing **order** and the world advances it once per
//! tick. Movement is constant-speed within the active drive regime: thrusters
//! steer, while warp locks the course and must drop before a meaningful turn.
//! There is no real-time piloting — the async-native, lightspeed-bound design
//! demands standing orders, not micro.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::{Cargo, Commodity};
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::movement::advance_toward;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShipKind {
    /// Slow, heavy, player-owned hauler. Carries trade.
    Convoy,
    /// Fast, light interceptor. Cuts chords across open space to run convoys
    /// down.
    Raider,
    /// The dedicated DEFENDER (§ships part 2): moderate mass (slower than a
    /// raider, faster than a convoy), no cargo, DEFENSE-heavy in the weighted
    /// battle model. It CANNOT raid (raiding is the raider's verb — crisp
    /// roles); it protects by SCREENING: any friendly corvette near a raid
    /// contact on a civilian ship duels the raider first (escort when shadowing
    /// a convoy, garrison when parked at an owned system — standing, offline).
    /// BROADCASTS under the Convention: a declared escort DETERS (a dark
    /// defender would just be a raider with extra steps).
    Corvette,
    /// The SETTLEMENT ship (§ships part 3): a heavy civilian hull — slow and
    /// expensive to fuel — with no cargo bay (it IS the cargo: colonists +
    /// infrastructure). Claiming is now PHYSICAL: send it to an unclaimed
    /// system; on arrival ownership transfers and the ship is CONSUMED (it
    /// becomes the colony). BROADCASTS under the Convention (a declared
    /// civilian settlement vessel — and your expansion is telegraphed,
    /// raidable, escortable). Destroyed in transit = colonists lost.
    Colony,
    /// The ACTIVE-INTEL ship (§scout): the lightest hull in the game — fastest
    /// base cruise speed, cheapest to fuel — with NO cargo capacity and negligible
    /// combat strength (in any engagement it is simply destroyed; its defense is
    /// speed and darkness, not armor). Runs DARK like a raider, amplifies a
    /// Raider-bearing fleet's sensor bubble (`sensor_mult`), and near rival
    /// systems captures timestamped intel snapshots of their fortifications.
    Scout,
    // --- §ladder: the WARSHIP LADDER (research-gated; appended after Scout so
    // every BTreeMap iteration order + snapshot stays stable). Capitals buy
    // PRESENCE and ROLE, never efficiency — combat weight per Armaments spent
    // peaks at Destroyer/Cruiser and declines up the ladder (pinned by test).
    /// The heavy combatant (Line IV): the first true ship of the line — real
    /// beam broadsides (Beam affinity), 3 slots / 8 fitting points.
    Destroyer,
    /// The season's prestige warship (Line V): the armored core of a fleet
    /// (Protection affinity), 4 slots / 12 points, the efficiency PEAK of the
    /// ladder — everything above buys presence, not value.
    Cruiser,
    /// The siege anchor (Line VI): driver broadsides (Driver affinity) and the
    /// hull that holds a blockade line — its presence accelerates a siege
    /// clock (`SIEGE_ANCHOR_MULT`). 4 slots / 18 points.
    Battleship,
    /// The fleet screen made flesh (Line VII): Interception affinity ×1.30 — a
    /// PD-fitted Dreadnought screens its whole side at platform grade. 5 slots
    /// / 28 points.
    Dreadnought,
    /// The flagship (Line VIII): a syndicate fields AT MOST ONE, and may name
    /// it. Broadly good (×1.10 to every weapon family), best at nothing — its
    /// answer is the wolfpack torpedo Raider, unless its 45 points went to PD
    /// or Dreadnought screens (the puzzle the budgets exist to pose).
    Titan,
    /// §ground M7: THE TROOP TRANSPORT — the hull an invasion is actually built
    /// around. Slow, unarmed, and it BROADCASTS: an assault force is telegraphed,
    /// raidable, and escortable, exactly like the colony ship whose job it takes
    /// over. It carries no cargo and no guns; what it carries is marines, and
    /// losing it in transit loses them (expansion by force has stakes, the same
    /// way settlement does). Built at a GARRISON, not a shipyard — troops come
    /// from barracks, not slipways.
    Transport,
    /// §emplacements: the CONSTRUCTION SHIP — the working hull that puts
    /// deep space sensors where they stand. Emplacements
    /// are not conjured at a yard and teleported out; a builder is ordered to
    /// the site, holds there for the assembly, and is FREED when it finishes —
    /// it is the crane, not the kit, so one hull serves a whole programme and
    /// losing it mid-job costs you the crane.
    ///
    /// Named `Builder`, deliberately not `Constructor`: serde's snake_case of
    /// that word is "constructor", which collides with
    /// `Object.prototype.constructor` in every plain-object lookup in the
    /// client — an insidious wrong-value bug, not an error.
    Builder,
    /// THE AUTHORITY FREIGHTER (§TCA): the scheduled common-carrier hull the
    /// TERRAN CHARTER AUTHORITY runs between the Market Hub and the colonies.
    /// Owned ONLY by the [`crate::ids::PlayerId::TCA`] sentinel — NOT buildable by
    /// any corporation ([`Self::is_buildable`] is false), so it sits outside the
    /// warship ladder entirely. Broadcasts like a convoy (a declared common
    /// carrier), carries no attack, is slower and chunkier than a convoy, and
    /// hauls a MULTI-owner manifest that lives on the [`crate::tca::FreightRun`],
    /// not in `Fleet.cargo`. In a hostile world it is just a fat civilian hull:
    /// raidable and destroyable like any convoy (Phase 1 makes a freighter kill
    /// consequence-free; the law arrives in Phase 2).
    Freighter,
    // Append-only: existing saves/replays keep Convoy as the Medium Freighter,
    // and Freighter remains the Authority's unrelated common carrier.
    #[serde(rename = "tiny_freighter")]
    TinyFreighter,
    #[serde(rename = "small_freighter")]
    SmallFreighter,
    #[serde(rename = "large_freighter")]
    LargeFreighter,
    #[serde(rename = "heavy_freighter")]
    HeavyFreighter,
    #[serde(rename = "bulk_freighter")]
    BulkFreighter,
    /// §charting: THE SURVEY PROBE — a cheap, crewless, one-way craft. It flies
    /// to a system, surveys it like a Scout, sends the report home at light
    /// speed and is SPENT doing it. Lets a corporation chart many systems at
    /// once without risking Scouts; it has no guns and no armour, so anything
    /// that catches it destroys it.
    #[serde(rename = "survey_probe")]
    SurveyProbe,
    /// §belts: THE MINING BARGE — the belt line's working miner (EVE's
    /// Retriever). Ordered to a charted asteroid belt, it works within
    /// [`crate::industry::SKIM_RANGE_SU`] of the star and fills its own small
    /// hold, or the stockpile when the system is yours. No guns, a poor
    /// hauler: a freighter, or a trip home, takes the ore to market.
    #[serde(rename = "mining_barge")]
    MiningBarge,
    /// §belts: THE MINING BELT STATION — a slow ore depot that ANCHORS at a
    /// charted belt (EVE's Rorqual with its core deployed). Barges at that belt
    /// unload into its 3,000-unit hold; freighters collect by hand or on a
    /// freight route. No guns and a tough hull: a big, still target that
    /// warships guard. Anchored, it cannot move, merge or split.
    #[serde(rename = "mining_belt_station")]
    MiningBeltStation,
    /// §holds: THE COMBAT FREIGHTER — the raiding freighter (Peter, Oct 27
    /// 2026: "force the player to bring along a freighter to loot"). Goods only
    /// ride in holds, so a blockade or a convoy raid keeps only what freighters
    /// riding with it can carry. Its hold is LOOT-ONLY ([`Self::is_loot_only`]):
    /// plunder, seized cargo and finds, never a dock's, a market's or a route's
    /// goods, so it never replaces a trade freighter. Faster than every
    /// freighter and the Corvette, slower than an Interceptor; unarmed,
    /// armoured, and dark. Raided like any freighter.
    #[serde(rename = "combat_freighter")]
    CombatFreighter,
}

/// Mass added per unit of cargo carried, for the HEAVIEST class (raw ore).
/// Under the constant-speed movement model cargo does NOT slow a fleet; the
/// extra mass raises fuel burn over a given distance, so richer shipments are
/// more expensive to move. §cargo-mass (Nov 7 2026): goods weigh by class
/// (`Commodity::mass_per_unit`); this is the ore figure, and what a planned
/// load of unknown goods is priced at.
pub const CARGO_MASS_PER_UNIT: f64 = crate::cargo::MASS_RAW;

/// Whole cargo units in the legacy Convoy / Medium Freighter. Other sizes use
/// `ShipKind::cargo_units`; a mixed fleet sums its actual holds (including pods).
/// Standing logistics uses that same aggregate capacity. Tunable.
pub const CARGO_UNITS_PER_CONVOY: u32 = 400;

/// §shipbuilding-overhaul: firepower of one Pulse Laser (the light gun)…
pub const PULSE_LASER_FIREPOWER: f64 = 0.5;
/// …and of one Beam Battery (the line gun). Tunable.
pub const BEAM_BATTERY_FIREPOWER: f64 = 2.0;

/// The player freight ladder (Tunable). The legacy Convoy is the medium hull:
/// upgrading its hold preserves every existing manifest instead of shrinking it.
pub const PLAYER_FREIGHTERS: [ShipKind; 6] = [ShipKind::TinyFreighter,
    ShipKind::SmallFreighter, ShipKind::Convoy, ShipKind::LargeFreighter,
    ShipKind::HeavyFreighter, ShipKind::BulkFreighter];

impl ShipKind {
    pub fn is_player_freighter(self) -> bool {
        PLAYER_FREIGHTERS.contains(&self)
    }

    /// §holds: a hold that takes only plunder, seized cargo and finds — the
    /// Combat Freighter's. It counts toward [`Fleet::cargo_capacity`] but not
    /// [`Fleet::trade_capacity`].
    pub fn is_loot_only(self) -> bool {
        self == ShipKind::CombatFreighter
    }

    /// A freighter hull: the trade ladder or the Combat Freighter. Raided for
    /// its hold, and its berth takes a utility, never a hardpoint.
    pub fn is_freighter_hull(self) -> bool {
        self.is_player_freighter() || self.is_loot_only()
    }

    pub fn cargo_units(self) -> u32 {
        match self {
            Self::TinyFreighter => 50,
            Self::SmallFreighter => 150,
            Self::Convoy => CARGO_UNITS_PER_CONVOY,
            Self::LargeFreighter => 1_000,
            Self::HeavyFreighter => 2_500,
            Self::BulkFreighter => 6_000,
            Self::MiningBarge => 150, // §belts: an ore hopper, not a cargo bay
            Self::MiningBeltStation => 3_000, // §belts: six ore silos
            Self::CombatFreighter => 80, // §holds: loot only
            _ => 0,
        }
    }
    /// Hull (empty) MASS, m₀. Mass sets hull durability/tactical size and feeds
    /// the fuel-capacity and fuel-cost calculations. Cruise speed is a separate
    /// per-kind value in [`Self::max_speed`]; the constant-speed model has no
    /// acceleration relationship to infer from mass.
    pub fn hull_mass(self) -> f64 {
        match self {
            ShipKind::Convoy => 4500.0,
            ShipKind::TinyFreighter => 1500.0,
            ShipKind::SmallFreighter => 2500.0,
            ShipKind::LargeFreighter => 8000.0,
            ShipKind::HeavyFreighter => 14000.0,
            ShipKind::BulkFreighter => 24000.0,
            ShipKind::Builder => 2500.0, // a crane and its shops — working mass, no bulk hold
            ShipKind::Raider => 200.0,
            ShipKind::Corvette => 800.0,
            ShipKind::Colony => 6000.0, // heavy civilian hull — fuel-∝-mass bites
            ShipKind::Scout => 80.0,
            ShipKind::SurveyProbe => 40.0, // §charting: a crewless drone, the lightest hull
            ShipKind::MiningBarge => 3_500.0, // §belts: cutters, hoppers and a small hold
            ShipKind::MiningBeltStation => 12_000.0, // §belts: silos, cradles and a gantry
            ShipKind::CombatFreighter => 2_000.0, // §holds: an armoured hold
            // §ladder: anchored to the Corvette (800) — 2.5× / 5× / 10× / 20× /
            // 40×. Mass drives fuel cost, so the capital fuel burn emerges here.
            ShipKind::Destroyer => 2_000.0,
            ShipKind::Cruiser => 4_000.0,
            ShipKind::Battleship => 8_000.0,
            ShipKind::Dreadnought => 16_000.0,
            ShipKind::Titan => 32_000.0,
            // §ground: a trooper is a big soft hull — heavy, but not a warship.
            ShipKind::Transport => 7_000.0,
            ShipKind::Freighter => 6000.0, // chunkier than a convoy — a bulk carrier
        }
    }

    /// Base CONSTANT cruise speed (sim units / s), GDD §14.1 — there is no
    /// acceleration; a fleet uses its slowest member's value
    /// ([`Fleet::max_speed`]), then the active drive regime applies its factor.
    /// The light-speed relationship is enforced centrally by
    /// [`crate::config::SimConfig`], so no duplicated `c` value belongs here.
    /// Ordering preserves the old relative feel (scout > raider > corvette >
    /// convoy > colony).
    ///
    /// These values are playtest tunables; their relative ordering is the design.
    /// Warp multiplication belongs to [`crate::transit`], and the config tests
    /// guard the light-to-hull speed ratio whenever this table changes.
    pub fn max_speed(self) -> f64 {
        match self {
            ShipKind::Convoy => 40.0,
            ShipKind::TinyFreighter | ShipKind::SmallFreighter => 40.0,
            ShipKind::LargeFreighter => 38.0,
            ShipKind::HeavyFreighter => 34.0,
            ShipKind::BulkFreighter => 30.0,
            ShipKind::Builder => 35.0, // ponderous — it is a worksite, not a courier
            ShipKind::Raider => 100.0,
            ShipKind::Corvette => 65.0, // keeps station with convoys, can't chase raiders
            ShipKind::Colony => 33.0,   // slowest civilian — the long, visible voyage
            ShipKind::Scout => 115.0,   // the fastest crewed hull — still < c/2
            ShipKind::SurveyProbe => 120.0, // §charting: a drone edges past it, still < c/2
            ShipKind::MiningBarge => 30.0, // §belts: as slow as a Bulk Freighter
            ShipKind::MiningBeltStation => 15.0, // §belts: station-keeping thrusters, not a drive
            // §holds: outruns every freighter and the Corvette; an Interceptor
            // catches it.
            ShipKind::CombatFreighter => 70.0,
            // §ladder: each rung slower than the last (0.85 / 0.70 / 0.55 /
            // 0.45 / 0.35 × the Corvette's 65) — a capital ARRIVES, it never
            // chases. All far below c = 300.
            ShipKind::Destroyer => 55.0,
            ShipKind::Cruiser => 45.0,
            ShipKind::Battleship => 36.0,
            ShipKind::Dreadnought => 29.0,
            ShipKind::Titan => 23.0, // the slowest hull flying — presence, not pursuit
            ShipKind::Transport => 30.0, // an invasion is slow and everyone sees it coming
            ShipKind::Freighter => 32.0, // slower than a convoy — a laden common carrier
        }
    }

    /// Whether this kind BROADCASTS under the Convention (visible galaxy-wide,
    /// light-delayed). Corporate Convoys may run silent; the Authority's
    /// Freighter always transmits. Pirates sit outside the Convention, so a
    /// pirate fleet never broadcasts whatever it flies ([`Fleet::broadcasts`]).
    pub fn broadcasts(self) -> bool {
        // Corvettes (a DECLARED escort deters) and colony ships (a declared
        // civilian settlement vessel — expansion is telegraphed) broadcast;
        // corporate Convoys, Raiders and Scouts run dark. §ladder: every
        // CAPITAL broadcasts — a ship of the line is a declared presence
        // (deterrence is its job; sneaking is the Raider's). The Authority's
        // common-carrier Freighter is deliberately public everywhere.
        matches!(
            self,
            ShipKind::Corvette
                | ShipKind::Colony
                | ShipKind::Destroyer
                | ShipKind::Cruiser
                | ShipKind::Battleship
                | ShipKind::Dreadnought
                | ShipKind::Titan
                // §ground: a troop convoy is a declared civilian-escort formation
                // — an invasion cannot be a surprise.
                | ShipKind::Transport
                | ShipKind::Builder
                | ShipKind::Freighter
        )
    }

    /// Whether this hull projects a mobile sensor bubble. Raiders carry the
    /// standard picket suite; Convoys carry a short-range traffic/threat set.
    pub fn projects_sensor(self) -> bool {
        self == ShipKind::Raider || self.is_player_freighter()
    }

    /// §shipbuilding-overhaul: a station-keeping hull (barge, crane, depot)
    /// carries half the engine density of a ship that goes places.
    pub fn is_working_hull(self) -> bool {
        matches!(self, ShipKind::MiningBarge | ShipKind::Builder | ShipKind::MiningBeltStation)
    }

    /// §shipbuilding-overhaul: the warp CORES a hull of this kind is built with
    /// (of whatever grade): one per 650 hull mass, half again on a warship,
    /// half on a working hull, at least one. The Survey Probe's burn-out drive
    /// needs none. Peter (Nov 6 2026): "the tiny freighter requires 2 warp
    /// engines but the interceptor requires just one because it is smaller
    /// mass." Tunable.
    pub fn engines(self) -> u32 {
        // §hull-ladder (docs/hull-ladder-plan.md, Nov 7 2026): engines are half
        // a hull's price, so the counts climb the ladder (×5 a warship rung at
        // a light engine of 1,000) and the big hulls take heavy and capital
        // engines (`engine_size`). Peter: the Tiny keeps its two, the
        // Interceptor its one.
        match self {
            ShipKind::SurveyProbe => 0,
            ShipKind::Raider | ShipKind::Scout => 1,
            ShipKind::Corvette => 5,
            ShipKind::Destroyer => 25,
            ShipKind::Cruiser => 5,
            ShipKind::Battleship => 25,
            ShipKind::Dreadnought => 10,
            ShipKind::Titan => 100,
            ShipKind::TinyFreighter => 2,
            ShipKind::SmallFreighter => 4,
            ShipKind::Convoy | ShipKind::Freighter => 13,
            ShipKind::LargeFreighter => 40,
            ShipKind::HeavyFreighter => 5,
            ShipKind::BulkFreighter => 15,
            ShipKind::CombatFreighter => 3,
            ShipKind::Colony => 10,
            ShipKind::Transport => 15,
            ShipKind::Builder => 4,
            ShipKind::MiningBarge => 3,
            ShipKind::MiningBeltStation => 30,
        }
    }

    /// §hull-ladder: the size of this hull's engines and frame. Light up to
    /// the Destroyer and the Large Freighter, heavy for the Cruiser, the
    /// Battleship and the two big haulers, capital for the Dreadnought and
    /// the Titan.
    pub fn engine_size(self) -> crate::fuel::EngineSize {
        use crate::fuel::EngineSize;
        match self {
            ShipKind::Cruiser | ShipKind::Battleship | ShipKind::HeavyFreighter | ShipKind::BulkFreighter => EngineSize::Heavy,
            ShipKind::Dreadnought | ShipKind::Titan => EngineSize::Capital,
            _ => EngineSize::Light,
        }
    }

    /// The reactor good this hull is built with at `grade`.
    pub fn reactor_good(self, grade: crate::fuel::ReactorGrade) -> Commodity { self.engine_size().reactor(grade) }
    /// The drive good this hull is built with for `drive`.
    pub fn drive_good(self, drive: crate::fuel::Drive) -> Commodity { self.engine_size().drive(drive) }

    /// The engine a hull takes when nothing chose one: scripted spawns
    /// (pirates, the Authority, test fixtures). The two dark hulls keep their
    /// jump on an Antimatter Reactor and a Maelstrom Drive; everything else
    /// flies a Fusion Reactor on a Zephyr Drive, at rated speed. A yard build
    /// picks its engine explicitly (`Command::BuildShip::reactor` / `drive`).
    pub fn default_engine(self) -> crate::fuel::Engine {
        use crate::fuel::{Drive, ReactorGrade};
        match self {
            ShipKind::Raider | ShipKind::Scout => (ReactorGrade::Antimatter, Drive::Maelstrom),
            _ => (ReactorGrade::Fusion, Drive::Zephyr),
        }
    }

    /// Can a hull of this kind be built with (or refitted to) this engine? The
    /// reactor must power the drive, and ships of the line never take a
    /// Maelstrom Drive: a capital arrives, it never chases.
    pub fn can_mount(self, reactor: crate::fuel::ReactorGrade, drive: crate::fuel::Drive) -> bool {
        self.engines() > 0 && reactor.powers(drive) && !(drive.jumps() && self.is_siege_anchor())
    }

    /// §fuel-overhaul: a Fold hull's antimatter bottle: two full-range jumps of
    /// the bare hull, at least [`crate::fuel::ANTIMATTER_BOTTLE_MIN`].
    pub fn antimatter_bottle(self) -> f64 {
        (self.hull_mass() * crate::fuel::ANTIMATTER_PER_HULL_MASS).max(crate::fuel::ANTIMATTER_BOTTLE_MIN)
    }

    /// Battleship-or-heavier ship of the line ([`is_siege_anchor`]).
    pub fn is_siege_anchor(self) -> bool {
        is_siege_anchor(self)
    }

    /// Multiplier on `config.sensor_range` when this hull accompanies a mobile
    /// sensor source. A Scout makes a Raider picket out-see an ordinary Raider;
    /// `projects_sensor` still decides whether the fleet emits a bubble at all.
    pub fn sensor_mult(self) -> f64 {
        match self {
            ShipKind::Scout => SCOUT_SENSOR_MULT,
            k if k.is_player_freighter() => CONVOY_SENSOR_MULT,
            _ => 1.0,
        }
    }

    // --- COMBAT STRENGTHS (§ships part 1, GDD §26.2 spirit) -------------------
    // Battles are weighted-strength contests, not unit counts: each side's
    // strength is a SUM of these per-kind weights, and the seeded outcome table
    // (`world::outcome_probs`) is a function of the attack/defense RATIO. The
    // weights re-express today's exact outcomes in the new units (see the
    // anchor-point notes on `outcome_probs`), so pre-existing raid results are
    // numerically unchanged. All tunable.

    /// §shipbuilding-overhaul: the GUNS a hull of this kind is built with —
    /// `(Pulse Lasers, Beam Batteries)`. They are kit parts, and their sum is
    /// the hull's firepower (`attack_weight`): a Pulse Laser is 0.5, a Beam
    /// Battery 2.0. Counts reproduce the pre-overhaul weights, so the balance
    /// matrix stands; capitals carrying a few Pulse Lasers read as secondary
    /// batteries. Only the Destroyer moved (2.4 → 2.5). Civilians, working
    /// hulls, the Scout and the Combat Freighter carry none. Tunable.
    pub fn guns(self) -> (u32, u32) {
        match self {
            ShipKind::Raider => (6, 0),      // the hunter: a gunboat
            ShipKind::Corvette => (2, 0),    // the screen; barely bites back
            ShipKind::Destroyer => (1, 1),
            ShipKind::Cruiser => (1, 2),
            ShipKind::Battleship => (0, 4),
            ShipKind::Dreadnought => (2, 7), // battle rules v12: 15
            ShipKind::Titan => (0, 12),
            _ => (0, 0),
        }
    }

    /// Offensive weight when this kind is the AGGRESSOR in an engagement: the
    /// sum of its guns (§shipbuilding-overhaul). Raw broadside climbs the
    /// ladder — but per Weapon Mount spent it DECLINES past the Cruiser (the
    /// efficiency invariant).
    pub fn attack_weight(self) -> f64 {
        let (pulse, batteries) = self.guns();
        pulse as f64 * PULSE_LASER_FIREPOWER + batteries as f64 * BEAM_BATTERY_FIREPOWER
    }

    /// Defensive weight when this kind is ATTACKED (or screening a defender).
    pub fn defense_weight(self) -> f64 {
        match self {
            ShipKind::Builder => 1.0, // a soft worksite that needs escorting
            ShipKind::Raider => 2.0,
            ShipKind::Corvette => 4.0, // the armored screen — built to be attacked
            ShipKind::Convoy => 1.0,
            ShipKind::TinyFreighter | ShipKind::SmallFreighter | ShipKind::LargeFreighter
                | ShipKind::HeavyFreighter | ShipKind::BulkFreighter => 1.0,
            ShipKind::Colony => 1.0, // a fat civilian hull — escort it
            ShipKind::Scout => 0.0,  // no armor at all
            ShipKind::SurveyProbe => 0.0, // nor has a probe
            ShipKind::MiningBarge => 1.0, // a working civilian hull, like a freighter
            // §belts: a tough hull, a cruiser's worth, so a lone raider can't
            // gut it before its guard arrives.
            ShipKind::MiningBeltStation => 6.0,
            ShipKind::CombatFreighter => 2.0, // §holds: armoured like an Interceptor
            // §ladder: capitals are defense-heavier than they are gun-heavy —
            // presence means being HARD TO REMOVE. Hull derives from this.
            ShipKind::Destroyer => 2.6,
            ShipKind::Cruiser => 5.5,
            ShipKind::Battleship => 12.0,
            ShipKind::Dreadnought => 26.0,
            ShipKind::Titan => 44.0,
            ShipKind::Transport => 1.0, // a fat hull that needs escorting
            ShipKind::Freighter => 1.0, // a fat civilian hull, like a convoy
        }
    }

    /// Whether this kind counts toward doctrine's local FORCE-RATIO assessments
    /// (weighted strength, not head-count). Non-combatants (convoys, scouts) are
    /// excluded exactly as the old raider-count was — so raider-only worlds see
    /// identical ratios.
    pub fn is_combatant(self) -> bool {
        matches!(
            self,
            ShipKind::Raider
                | ShipKind::Corvette
                | ShipKind::Destroyer
                | ShipKind::Cruiser
                | ShipKind::Battleship
                | ShipKind::Dreadnought
                | ShipKind::Titan
        )
    }

    /// A combatant's weight in force-ratio comparisons (attack + defense — its
    /// total fighting presence). Equal-kind fleets produce the same ratios as
    /// the old head-count.
    pub fn combat_weight(self) -> f64 {
        self.attack_weight() + self.defense_weight()
    }

    /// HULL: the strategic ACCOUNTING weight of one ship of this kind — used by
    /// rankings and corp stats (hull destroyed/absorbed). Derived from its
    /// defense weight (`defense × [`crate::combat::HULL_PER_DEFENSE`]`) with a
    /// small floor. §tactical: battle HP is [`ShipKind::hull_mass`], not this —
    /// this stays the cross-kind score currency. Tunable via the combat block.
    /// Whether a CORPORATION can build this kind (§TCA). Everything on the ladder
    /// is buildable EXCEPT the Authority [`Self::Freighter`], which only the TCA
    /// sentinel ever mints — it is excluded from every BUILDABLE menu and the
    /// `BuildShip` handler soft-rejects it (`recipe_for`/`required_shipyard_tier`
    /// never see it).
    pub fn is_buildable(self) -> bool {
        !matches!(self, ShipKind::Freighter)
    }

    pub fn hull(self) -> f64 {
        (self.defense_weight() * crate::combat::HULL_PER_DEFENSE).max(crate::combat::HULL_MIN)
    }

    /// §modules Part B: how many MODULE slots this hull carries. Warships fit
    /// modules; a player Freighter has one berth, for a utility or a Warp
    /// Stabilizer (the blockade runner). §ladder: capitals
    /// are where COMBINATIONS live — slots and points both climb. Tunable.
    pub fn module_slots(self) -> u32 {
        match self {
            ShipKind::Builder => 0, // no hardpoints — its fittings ARE the crane
            ShipKind::SurveyProbe => 0, // a sealed, single-use drone
            ShipKind::MiningBarge => 1, // one utility berth, like a freighter
            ShipKind::MiningBeltStation => 0, // §belts: its fittings ARE the silos
            ShipKind::CombatFreighter => 1, // §holds: one utility berth, like a freighter
            ShipKind::Corvette => 2,
            ShipKind::Raider => 2,
            ShipKind::Scout => 1,
            ShipKind::Convoy => 1,
            ShipKind::TinyFreighter | ShipKind::SmallFreighter | ShipKind::LargeFreighter
                | ShipKind::HeavyFreighter | ShipKind::BulkFreighter => 1,
            // §TCA: the Authority's carrier fits no modules — it is not a warship.
            ShipKind::Colony | ShipKind::Transport | ShipKind::Freighter => 0,
            ShipKind::Destroyer => 3,
            ShipKind::Cruiser => 4,
            ShipKind::Battleship => 4,
            ShipKind::Dreadnought => 5,
            ShipKind::Titan => 6,
        }
    }
}

/// §fitting: the hull's FITTING-POINT budget — the capacity it spends on module
/// fitting costs (`ModuleKind::fitting_cost`). The SECOND constraint besides
/// slots; both bind at build/refit/fit-save (`Loadout::validate`). Tunable, but
/// preserve the relationships the Stage-A tests pin:
/// - a torpedo Corvette can't take heavy armor (Torp 3 + Whipple 3 = 6 > 5);
///   Driver 2 + Whipple 3 = 5 fits exactly — the classic brawler;
/// - a torpedo Raider is a glass cannon (Torp 3 leaves 1 point — nothing else
///   fits today; intended).
pub fn fitting_points(kind: ShipKind) -> u32 {
    match kind {
        ShipKind::Builder => 0,
        ShipKind::SurveyProbe => 0,
        ShipKind::MiningBarge => 2,
        ShipKind::MiningBeltStation => 0,
        ShipKind::CombatFreighter => 2,
        ShipKind::Scout => 2,
        ShipKind::Convoy => 2,
        ShipKind::TinyFreighter | ShipKind::SmallFreighter | ShipKind::LargeFreighter
            | ShipKind::HeavyFreighter | ShipKind::BulkFreighter => 2,
        ShipKind::Colony => 2,
        ShipKind::Transport => 2,
        ShipKind::Raider => 4,
        ShipKind::Corvette => 5,
        // §ladder: the big budgets — capitals combine freely (a Titan carries
        // Torp+PD+both armors and change); subcaps must CHOOSE.
        ShipKind::Destroyer => 8,
        ShipKind::Cruiser => 12,
        ShipKind::Battleship => 18,
        ShipKind::Dreadnought => 28,
        ShipKind::Titan => 45,
        // §TCA: not a warship — no slots, so no fitting budget either.
        ShipKind::Freighter => 0,
    }
}

/// §fitting: hull AFFINITY — the % bonus a hull grants to a module FAMILY's
/// effect. Depth without marks: a Raider is a torpedo boat, a Corvette is the
/// screen. Scales MAGNITUDES only — no counter relationship ever changes (the
/// matrix shape assertions stay untouched). Rendered as a named factor line
/// wherever the family's effect shows (build detail, battle tooltips). Tunable.
/// (A Scout utility affinity is parked until utility modules exist.)
pub fn hull_affinity(kind: ShipKind, family: crate::module::Family) -> f64 {
    use crate::module::Family;
    match (kind, family) {
        (ShipKind::Raider, Family::Torpedo) => 1.25, // the torpedo boat
        (ShipKind::Corvette, Family::Interception) => 1.25, // the screen
        // §ladder: each capital is good at a KIND of fighting. The Titan is
        // broadly good (×1.10 to every weapon family), best at nothing — the
        // one Beam affinity above 1.0, deliberately (it never breaks the
        // unfitted-calibration anchor because Titans never existed pre-fitting).
        (ShipKind::Destroyer, Family::Beam) => 1.20,
        (ShipKind::Cruiser, Family::Protection) => 1.20,
        (ShipKind::Battleship, Family::Driver) => 1.20,
        (ShipKind::Dreadnought, Family::Interception) => 1.30, // platform-grade screen
        (ShipKind::Titan, Family::Beam | Family::Driver | Family::Torpedo) => 1.10,
        _ => 1.0,
    }
}

/// The mass line above which a hull counts as a CAPITAL — it anchors instead
/// of holding the line ([`crate::tactical::Role::Anchor`]) and platform-grade
/// PD bubbles key off it. The threshold sits between the Cruiser (4000) and
/// the Battleship (8000), clear of the Colony's 6000. Tunable.
///
/// §tactical supersession of ladder B3: the flat `TORP_CAPITAL_EDGE` ×1.25 is
/// DELETED — the capital-hunting torpedo is now EMERGENT from tracking (to-hit
/// rises steeply with target mass and falls with target speed, so a seeker
/// near-guarantees against a Titan and struggles against a darting Corvette).
/// The wolfpack answer exists because the physics says so, not because a
/// multiplier was bolted on.
pub const CAPITAL_MASS_THRESHOLD: f64 = 7_000.0;

/// §ladder: does this hull require its research programme (UnlockHull) before
/// it can be built? Warship ladder hulls and freight above Tiny are researched.
/// §research-locked (Nov 11 2026): the Corvette too (Escort Datalink).
pub fn requires_hull_unlock(kind: ShipKind) -> bool {
    matches!(
        kind,
        ShipKind::Corvette
            | ShipKind::Destroyer
            | ShipKind::Convoy
            | ShipKind::SmallFreighter
            | ShipKind::LargeFreighter
            | ShipKind::HeavyFreighter
            | ShipKind::BulkFreighter
            | ShipKind::Cruiser
            | ShipKind::Battleship
            | ShipKind::Dreadnought
            | ShipKind::Titan
            | ShipKind::MiningBeltStation
    )
}

/// §ladder: a BATTLESHIP anchoring a blockade accelerates the siege clock —
/// the named "siege anchor" line. Applied to capture-clock progress while any
/// blockading fleet carries a Battleship or heavier combatant. Tunable.
pub const SIEGE_ANCHOR_MULT: f64 = 1.25;
/// Is this hull a siege anchor (Battleship-or-heavier ship of the line)?
pub fn is_siege_anchor(kind: ShipKind) -> bool {
    kind.is_combatant() && kind.hull_mass() >= CAPITAL_MASS_THRESHOLD
}

/// The FLAGSHIP precedence (GDD §13.1): a fleet is DRAWN and named for its
/// most-significant member — colony first (the point of the whole voyage),
/// then convoy (trade), corvette (escort), raider (teeth), scout (eyes). A
/// fleet-of-one resolves to that ship's own kind, so nothing changes for the
/// N=1 world. Highest precedence first.
pub const FLAGSHIP_PRECEDENCE: [ShipKind; 22] = [
    // §ladder: a capital OUTRANKS everything — a fleet with a Titan IS the
    // Titan (its name, its sprite, its label), down the ladder from there.
    ShipKind::Titan,
    ShipKind::Dreadnought,
    ShipKind::Battleship,
    ShipKind::Cruiser,
    ShipKind::Destroyer,
    // §ground: a troop convoy reads as the invasion it is — outranking the
    // civilian hulls it travels with, so a fleet carrying one is drawn as one.
    ShipKind::Transport,
    ShipKind::Colony,
    // §belts: a station is the biggest civilian thing in any fleet it joins.
    ShipKind::MiningBeltStation,
    ShipKind::BulkFreighter,
    ShipKind::HeavyFreighter,
    ShipKind::LargeFreighter,
    ShipKind::Convoy,
    ShipKind::SmallFreighter,
    ShipKind::TinyFreighter,
    // §belts: a mining fleet is drawn as its barge unless it hauls with a
    // freighter, but ahead of the escort riding with it.
    ShipKind::MiningBarge,
    // A freighter fleet is pure freighters (never mixed with player ships), so its
    // rank here only names how a lone Authority hull is drawn — a hauler.
    ShipKind::Freighter,
    ShipKind::Corvette,
    ShipKind::Raider,
    // §holds: a raid fleet is drawn as its Interceptors; a lone Combat
    // Freighter is the freighter it is (and is raided like one).
    ShipKind::CombatFreighter,
    // The working hull: shown only when nothing above it rides along.
    ShipKind::Builder,
    ShipKind::Scout,
    ShipKind::SurveyProbe,
];

/// All ship kinds, in a fixed deterministic order (composition iteration,
/// damage-pool distribution, report ordering). Kept in sync with [`ShipKind`].
pub const ALL_SHIP_KINDS: [ShipKind; 22] = [
    ShipKind::Convoy,
    ShipKind::Raider,
    ShipKind::Corvette,
    ShipKind::Colony,
    ShipKind::Scout,
    ShipKind::Destroyer,
    ShipKind::Cruiser,
    ShipKind::Battleship,
    ShipKind::Dreadnought,
    ShipKind::Titan,
    ShipKind::Transport,
    ShipKind::Builder,
    ShipKind::Freighter,
    ShipKind::TinyFreighter,
    ShipKind::SmallFreighter,
    ShipKind::LargeFreighter,
    ShipKind::HeavyFreighter,
    ShipKind::BulkFreighter,
    ShipKind::SurveyProbe,
    ShipKind::MiningBarge,
    ShipKind::MiningBeltStation,
    ShipKind::CombatFreighter,
];

/// The fastest flying speed across every ship kind — the single number the
/// light-game invariant ([`crate::config::SimConfig::light_ratio`]) is measured
/// against: `c` must comfortably outrun even the quickest hull so information
/// and orders can, in principle, overtake any raider. Recomputed from the speed
/// table so a future speed edit can't silently outrun light unnoticed.
pub fn fastest_ship_speed() -> f64 {
    // §reactors-drives: on the fastest drive.
    fastest_rated_speed() * crate::fuel::Drive::Maelstrom.speed_mult()
}

/// The speed table's top entry, before any drive: what the "comfortable"
/// light margin is measured against (the drive ladder spends the headroom
/// above it down to the floor).
pub fn fastest_rated_speed() -> f64 {
    ALL_SHIP_KINDS
        .iter()
        .map(|k| k.max_speed())
        .fold(0.0_f64, f64::max)
}

/// An ESTIMATED-SIZE BUCKET for a fleet seen through the fog (GDD §13.1 intel
/// ladder). A far observer of a broadcasting hammer knows roughly HOW BIG it is
/// — never the exact count, and never what's IN it (that needs sensor coverage).
///
/// Buckets, not ± ranges, on purpose: an exact N can't be inverted out of a
/// bucket the way it could from "±2". Thresholds are tunable but must only ever
/// WIDEN the estimate (the fog-leak tests assert the class is never narrower
/// than the true count warrants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountClass {
    One,
    TwoToThree,
    FourToSeven,
    EightToFifteen,
    SixteenToThirty,
    ThirtyOnePlus,
}

impl CountClass {
    /// The deterministic bucket for an exact total count. Tunable thresholds:
    /// `1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+`.
    pub fn from_count(n: u32) -> Self {
        match n {
            0..=1 => CountClass::One,
            2..=3 => CountClass::TwoToThree,
            4..=7 => CountClass::FourToSeven,
            8..=15 => CountClass::EightToFifteen,
            16..=30 => CountClass::SixteenToThirty,
            _ => CountClass::ThirtyOnePlus,
        }
    }

    /// The human-facing label ("est. 4–7 ships").
    pub fn label(self) -> &'static str {
        match self {
            CountClass::One => "1",
            CountClass::TwoToThree => "2–3",
            CountClass::FourToSeven => "4–7",
            CountClass::EightToFifteen => "8–15",
            CountClass::SixteenToThirty => "16–30",
            CountClass::ThirtyOnePlus => "31+",
        }
    }

    /// A representative count for the STALE-INTEL calculator (Part 3): when an
    /// observer has only the bucket (target out of sensor coverage), the
    /// projected battle assumes this many ships of "typical" composition. It is
    /// deliberately the bucket MIDPOINT, never the true count (leak-checked).
    pub fn midpoint(self) -> u32 {
        match self {
            CountClass::One => 1,
            CountClass::TwoToThree => 2,
            CountClass::FourToSeven => 5,
            CountClass::EightToFifteen => 11,
            CountClass::SixteenToThirty => 23,
            CountClass::ThirtyOnePlus => 40,
        }
    }
}

// --- CREW AND STANDING UPKEEP (§upkeep) ----------------------------------------

/// Operating crew in PEOPLE, not workforce cohorts or cargo mass. Tunable hull
/// complements: freight holds are largely automated, while warships need more
/// personnel. Every existing hull is fully crewed; recruitment and crew
/// casualties are not simulated. Deriving this from the roster keeps old saves
/// compatible and makes merging, splitting and losing hulls conserve headcount.
/// Marines, colony settlers and specialist passengers are counted separately.
pub fn crew_size(kind: ShipKind) -> u32 {
    match kind {
        ShipKind::TinyFreighter => 5,
        ShipKind::SmallFreighter => 10,
        ShipKind::Convoy => 20,
        ShipKind::LargeFreighter => 30,
        ShipKind::HeavyFreighter => 50,
        ShipKind::BulkFreighter => 80,
        ShipKind::Builder => 30,
        ShipKind::Scout => 4,
        ShipKind::SurveyProbe => 0, // crewless: it eats nothing
        ShipKind::MiningBarge => 15,
        ShipKind::MiningBeltStation => 60,
        ShipKind::CombatFreighter => 10, // a prize crew
        ShipKind::Colony => 50,
        ShipKind::Transport => 40,
        ShipKind::Raider => 12,
        ShipKind::Corvette => 40,
        ShipKind::Destroyer => 120,
        ShipKind::Cruiser => 350,
        ShipKind::Battleship => 800,
        ShipKind::Dreadnought => 1_800,
        ShipKind::Titan => 4_000,
        // The crew exists, but sentinel-owned ships are fed outside the player
        // economy. The exemption belongs to the owner, not to the hull's food.
        ShipKind::Freighter => 24,
    }
}

fn colonists_aboard(kind: ShipKind) -> u32 {
    if kind == ShipKind::Colony {
        (crate::colony::COLONY_FOUNDING_POP * 1_000_000.0) as u32
    } else {
        0
    }
}

/// Baseline food for one fully crewed hull with its initial embarked complement.
/// A live fleet uses its actual remaining marines and specialist passengers in
/// [`Fleet::upkeep_per_sec`]. Every person eats the same baseline colony ration.
pub fn upkeep_per_sec(kind: ShipKind) -> f64 {
    f64::from(crew_size(kind) + marine_capacity(kind) + colonists_aboard(kind))
        * crate::colony::PROVISIONS_PER_PERSON_PER_S
}

// --- §ground M6/M7: MARINES ----------------------------------------------------
// Taking ground needs troops on it. Orbital supremacy suppresses a garrison
// (§ground M6) but can never take a colony — the prize must survive, and
// population never decreases — so somebody has to land.
//
// Capitals carry a marine complement as a matter of course, which finally gives
// the top of the ladder a job besides presence; the dedicated Troop Transport
// carries far more per ton and is the hull an actual invasion is built around.

/// MARINE COMPLEMENT carried by one hull of this kind. Warships carry a boarding
/// party; the Transport is a purpose-built trooper. Everything civilian carries
/// none — colonists are not soldiers. Tunable.
pub fn marine_capacity(kind: ShipKind) -> u32 {
    match kind {
        // The dedicated invasion hull.
        ShipKind::Transport => 40,
        // Capitals carry real boarding parties — presence with a purpose.
        ShipKind::Titan => 30,
        ShipKind::Dreadnought => 18,
        ShipKind::Battleship => 12,
        ShipKind::Cruiser => 7,
        ShipKind::Destroyer => 4,
        // Everything else: none. A corvette screens, a raider steals, and a
        // colony ship settles empty ground — none of them storm a planet.
        _ => 0,
    }
}

/// §dock: WHERE a fleet is berthed. The Market Hub is not a system (it has no
/// id — it is a fixed point in the galaxy), so a plain `Option<EntityId>` can't
/// name it without conflating "berthed at the hub" with "not berthed at all".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockSite {
    /// The Terran Market Hub at the wormhole hub — the galaxy's busiest dock,
    /// and the one place every corporation's traffic converges. Hub berths are
    /// deliberately unmetered: qualifying fleets do not reserve or exhaust
    /// slots there.
    Hub,
    /// A star system this fleet's owner (or an ally) holds.
    System(EntityId),
}

/// §dock: how close a fleet must be to a dock to BE DOCKED — one radius for
/// every purpose, replacing the three that grew up independently (logistics at
/// 260, repair and marine re-embark at 80). A hull 150 su out could be loaded
/// with cargo but not repaired, and nothing anywhere said why.
///
/// Sized as a docking APPROACH rather than a contact: it is generous enough that
/// a player who parked "at" a system or the Market Hub is docked. Nothing under
/// way can trip it — the predicate requires an Idle fleet — so a ship merely
/// passing near a facility is never captured by it. Tunable.
pub const DOCK_RADIUS: f64 = 260.0;

/// Radius (sim units) within which an arriving COLONY SHIP settles an
/// unclaimed system (§ships part 3) — matches the raid contact radius, so
/// "arrival" means the same thing everywhere. Tunable.
pub const COLONY_CLAIM_RADIUS: f64 = 80.0;

/// Radius (sim units) within which a friendly CORVETTE screens a raid contact
/// on a civilian ship (§ships part 2): shadowing a convoy = escort; parked at
/// an owned system = garrison (same reach as the Defense Platform, so a
/// garrisoned corvette covers the whole protected zone). One rule, both roles.
/// Tunable.
pub const CORVETTE_PROTECT_RADIUS: f64 = 1300.0;

/// The scout's sensor-bubble multiplier over the standard ship bubble (its
/// entire reason to exist: 1.5 × 2200 = 3300 su — out-seeing a tier-1 Sensor
/// Array). Tunable.
pub const SCOUT_SENSOR_MULT: f64 = 1.5;

/// A Convoy sees one quarter as far as a command center or Raider picket:
/// 0.25 × the current 80,000-su base = 20,000 su. Tunable.
pub const CONVOY_SENSOR_MULT: f64 = 0.25;

/// Range (sim units) at which a SCOUT passing a RIVAL-owned system captures an
/// intel snapshot of its fortifications (§scout part 2). ≈ the Defense
/// Platform's protection radius — close enough to be engageable, so scouting a
/// defended system is a risk. Tunable.
pub const SCOUT_INTEL_RANGE: f64 = 1300.0;

/// A scout that stays parked in range keeps its snapshot fresh SILENTLY; the
/// owner-only "Scout report" notice re-fires only when a snapshot had gone
/// stale by this much (the scout left and returned) or the observed tiers
/// changed. Anti-spam. Tunable.
pub const SCOUT_INTEL_RENOTIFY_S: f64 = 60.0;

/// Seconds a patrolling ship waits at each waypoint before moving on.
const PATROL_DWELL: f64 = 2.5;

/// A fleet's TRANSIT THROTTLE (§Part 4, §fuel-overhaul): which drive regime
/// and fuel it flies on.
/// - `Full` runs at formation speed on the fusion tank (Deuterium, Helium-3)
///   and lights up the fleet's signature at flank.
/// - `Stealth` creeps at `STEALTH_FRACTION` on the same tank to stay quiet;
///   half the burn per second over twice the time, so the same trip energy.
/// - `Slow` runs at [`crate::fuel::SLOW_SPEED_FRACTION`] on the fission store
///   (Uranium), burning [`crate::fuel::SLOW_FUEL_MULT`] of Full's energy per
///   distance. Every formation without a ship of the line can fly it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitMode {
    #[default]
    Full,
    Stealth,
    Slow,
}

impl TransitMode {
    /// The fraction of formation speed this mode travels at.
    pub fn fraction(self) -> f64 {
        match self {
            TransitMode::Full => 1.0,
            TransitMode::Stealth => crate::detection::STEALTH_FRACTION,
            TransitMode::Slow => crate::fuel::SLOW_SPEED_FRACTION,
        }
    }
}

/// Tunables: a system guard pursues only inside this system-centered circle,
/// never an ever-moving radius around the quarry. No free travel or remote guns.
pub const SYSTEM_DEFENSE_RADIUS: f64 = 10_000.0;
pub const SYSTEM_DEFENSE_MIN_RADIUS: f64 = 5_000.0;
pub const SYSTEM_DEFENSE_MAX_RADIUS: f64 = 20_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SystemDefense {
    pub system: EntityId,
    pub station: Vec2,
    pub radius: f64,
}

impl SystemDefense {
    pub fn order(self) -> FleetOrder { FleetOrder::DefendSystem { assignment: self } }
}

/// A fleet's standing order — what it does without further input. Orders are
/// FLEET-LEVEL (GDD §13.1): the whole formation moves, intercepts, and holds as
/// one entity. A fleet-of-one behaves exactly as the old single ship did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FuelTransferPhase {
    Rendezvous,
    Waiting,
    Transferring,
    Complete,
    Empty,
    Unsafe,
    Unavailable,
}

/// Onboard transfer ledger. The server stamps this alongside cargo/fuel facts;
/// neither the requested amount nor progress is read from truth by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FuelTransfer {
    pub target: EntityId,
    pub requested: u32,
    pub spent: u32,
    pub delivered: f64,
    pub phase: FuelTransferPhase,
}

/// §player-styles (Peter, Oct 31 2026): a raid set to repeat. After each trip
/// home it sets out again once refuelled and repaired, until a trip comes home
/// empty or short a hull, the victim's daily limit is reached, the target can't
/// be raided, or the fleet is given another order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RaidRepeat {
    pub system: EntityId,
    pub station: Vec2,
    /// Trips flown, this one included.
    pub trip: u32,
    /// Hulls aboard when this trip set out.
    pub ships: u32,
    /// On the way home or waiting there.
    pub homeward: bool,
    /// Units this trip carried home.
    pub haul: u32,
    /// The victim's daily limit ended this trip.
    pub capped: bool,
    /// When it docked home after this trip, waiting on its repairs.
    #[serde(default)]
    pub docked_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FleetOrder {
    /// Mobile service: pursue using ordinary drives; world handles the safe
    /// stationary rendezvous. A terminal report holds here until reassignment.
    Refuel {
        transfer: FuelTransfer,
        next_pump: f64,
    },
    /// Investigate or work a known interstellar contact; travel and hold use
    /// the ordinary drives. Completion and reports are managed by the world.
    Expedition {
        site: EntityId,
        station: Vec2,
        task: crate::sites::ExpeditionTask,
        #[serde(default)]
        dwell_since: Option<f64>,
    },
    /// At rest, no goal.
    Idle,
    /// Travel to a fixed point under the drive state machine, then go
    /// [`FleetOrder::Idle`].
    MoveTo {
        dest: Vec2,
    },
    /// Hold position while the world validates and manages the 10-second spool.
    Jump {
        dest: Vec2,
        #[serde(default)]
        spool_started: Option<f64>,
    },
    /// Cycle forever through a list of waypoints, dwelling briefly at each.
    /// (M2 demo behaviour so the shared world is visibly alive; real
    /// player-issued orders arrive in M4/M5.)
    Patrol {
        waypoints: Vec<Vec2>,
        index: usize,
        /// Sim time until which the ship holds at the current waypoint.
        dwell_until: f64,
    },
    /// Autonomously pursue a target ship to intercept (§8). Resolved by the
    /// world in true space (contact → convoy lost; target reaches safety →
    /// raid fails). Pursuit steering lives in [`crate::movement::pursue_step`]
    /// (proportional steer-and-correct) and is driven by the world (it needs the
    /// target's state).
    Intercept {
        target: EntityId,
    },
    /// Shadow a named friendly fleet and defend it using the Interceptor's own
    /// local sensor picture. The world owns the moving formation point and any
    /// defensive sortie; unlike Intercept, reaching the target opens no combat.
    Guard {
        target: EntityId,
    },
    /// Return to this owned system, defend locally sensed hostile warships,
    /// then return to the same station. The assignment survives every sortie.
    DefendSystem {
        assignment: SystemDefense,
    },
    /// BLOCKADE a rival system (§contestable-territory Part 1): fly to the
    /// system and take STATION on it, strangling its logistics. `station` is the
    /// target system's position (static, captured at issue time) so the
    /// self-contained advance can steer to it without a world lookup; `system`
    /// names the target for the world's blockade resolution. On arrival the
    /// fleet HOLDS on station (keeps this order — it does not go Idle), and the
    /// world's standing-defense engages it as any hostile contact.
    /// §emplacements: fly to `site` and BUILD there. The fleet holds at the
    /// site while the work runs; the world's construction resolver owns the
    /// clock and frees the fleet when the emplacement stands. `started` is the
    /// sim time work began, `None` while still in transit.
    Construct {
        site: Vec2,
        emplacement: crate::emplace::EmplacementKind,
        #[serde(default)]
        started: Option<f64>,
    },
    /// §emplacements: fly to a RIVAL structure and TEAR IT DOWN. Mirrors
    /// [`FleetOrder::Construct`] in shape and in flight — hold on station while
    /// the world's demolition resolver runs the clock — but needs the target's
    /// id as well as its position, because the thing being worked on already
    /// exists and has to be found again to remove it. `site` is captured at
    /// issue so the self-contained advance can steer without a world lookup;
    /// structures never move, so it cannot go stale.
    Demolish {
        target: EntityId,
        site: Vec2,
        #[serde(default)]
        started: Option<f64>,
    },
    Blockade {
        system: EntityId,
        station: Vec2,
        /// §raid-order (Peter, Oct 28 2026): a RAID on the system — blockade
        /// rules, but the fleet loads until its holds are full, then carries
        /// the loot home and unloads it (`World::raid_turnarounds`). It also
        /// turns home when the blockade is contested with loot aboard, or when
        /// nothing is left in reach.
        #[serde(default)]
        raid: bool,
        /// §player-styles: a raid that sets out again after each trip home
        /// (`Fleet::raid_repeat`).
        #[serde(default)]
        repeat: bool,
    },
    /// ATTACK a rival fleet to DESTROY it (§offensive-orders Part 1): the targeted
    /// destroy verb. Pursues exactly like [`FleetOrder::Intercept`], but on contact
    /// it opens a FULL-DURATION engagement (`raid = false`) regardless of the
    /// target's kind — so a convoy is destroyed (its cargo lost with it), not
    /// raided. RAID (`Intercept` on a convoy) steals; ATTACK destroys.
    Attack {
        target: EntityId,
    },
    /// SURVEY a system's geology (§explore Part 2 — the scout's second job): fly
    /// to within `SURVEY_RANGE` of the star and DWELL `SURVEY_SECS`, all-or-nothing
    /// (leaving range or entering an engagement aborts with no partial credit —
    /// re-issuable). `station` is the star's position (captured at issue, like
    /// Blockade) so the self-contained advance steers without a world lookup;
    /// `dwell_since` is the world-managed dwell clock (None = still approaching /
    /// reset). While dwelling the fleet runs LOUD (active sensing). On completion
    /// the exact geology travels home at c and the order goes Idle.
    Survey {
        system: EntityId,
        station: Vec2,
        #[serde(default)]
        dwell_since: Option<f64>,
    },
}

/// What a physical cargo fleet does when it reaches its destination (§9).
/// Ordinary market settlement itself moves no hull. Player loading, legacy
/// one-way deliveries, and standing logistics can attach one of these missions;
/// a standing order always assigns a real player fleet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeMission {
    DeliverHome,
    SellAtHub,
    /// Deposit the cargo into the destination system's stockpile on arrival (a
    /// system→system supply convoy; §15). Cargo is lost if the destination is no
    /// longer owned by the convoy's owner when it arrives (no gifting rivals).
    DeliverToSystem {
        system: EntityId,
    },
    /// §TCA Part 5: HAUL TO THE MARKET HUB — a PLAYER-owned convoy carrying its
    /// own cargo to the hub, where it deposits into the owner's warehouse and
    /// (optionally) sells the lot at that tick's quantity-aware execution price. Unlike the legacy
    /// `SellAtHub`, the fleet SURVIVES: it is the player's hull, so it goes Idle at
    /// the Market Hub ready for its next job. This is the player-owned half of
    /// the two logistics channels; TCA freight is the other.
    DeliverToWarehouse {
        sell_on_arrival: bool,
    },
}

/// A patrolling raider's AUTONOMOUS defensive sortie (§5.1, Pillar 1): it has
/// broken off its patrol on its own to intercept a hostile raider threatening a
/// friendly convoy, and will resume the saved `patrol` route once the threat is
/// gone. Its presence also marks this Intercept as defensive (not a player's
/// manual raid), so the world's standing-doctrine logic owns its lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefenseEngagement {
    pub target: EntityId,
    pub patrol: Vec<Vec2>,
    /// A direct Guard order to resume after this defensive sortie. `None`
    /// preserves the older patrol-picket lifecycle and old snapshots.
    #[serde(default)]
    pub guard: Option<EntityId>,
    /// A fixed-system post to resume; absent for legacy patrol/fleet escorts.
    #[serde(default)]
    pub system: Option<SystemDefense>,
}

/// §roster — ONE INDIVIDUAL HULL. Fleets are ROSTERS now, not histograms: every
/// ship in the game is a record with its own identity, fit, and remaining hull.
/// This is what makes battle damage PERSIST exactly — a survivor carries its own
/// deficit out of the arena and into the next fight, with no pooling and no
/// pro-rata share anywhere.
///
/// `id` is FLEET-LOCAL (allocated from [`Fleet::next_ship_id`]): a ship is only
/// ever addressed as `(fleet, ship)`, and a hull moving between fleets by
/// merge/split is re-issued an id by its new owner. Fleet-local keeps ids small
/// in snapshots and needs no world-wide counter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ship {
    pub id: u32,
    pub kind: ShipKind,
    /// The fit this HULL carries. Empty = the stock beam brawler.
    #[serde(default)]
    pub loadout: crate::module::Loadout,
    /// Remaining hull. Full = [`ShipKind::hull_mass`] (the tactical engine's
    /// battle HP). Damage persists here between engagements until repaired.
    pub hp: f64,
    /// §reactors-drives: the REACTOR this hull's engines run on, built or
    /// refitted. Names the fuel it burns. serde default = Fission.
    #[serde(default)]
    pub reactor: crate::fuel::ReactorGrade,
    /// The WARP DRIVE its engines turn: speed, Stealth, the jump. serde
    /// default = Zephyr.
    #[serde(default)]
    pub drive: crate::fuel::Drive,
}

impl Ship {
    /// A fresh, undamaged hull on its kind's default engine.
    pub fn new(id: u32, kind: ShipKind, loadout: crate::module::Loadout) -> Self {
        Self::with_engine(id, kind, loadout, kind.default_engine())
    }

    /// A fresh, undamaged hull built with `engine` (reactor, drive).
    pub fn with_engine(id: u32, kind: ShipKind, loadout: crate::module::Loadout, engine: crate::fuel::Engine) -> Self {
        Ship {
            id,
            kind,
            loadout,
            hp: kind.hull_mass(),
            reactor: engine.0,
            drive: engine.1,
        }
    }

    /// This hull's engine.
    pub fn engine(&self) -> crate::fuel::Engine { (self.reactor, self.drive) }

    /// This hull's tank for its reactor's fuel, in units: the rung's energy
    /// aboard per unit of hull mass (an Antimatter hull's at least its jump
    /// bottle), times the fit's tank multiplier.
    pub fn tank(&self) -> f64 {
        if self.kind.engines() == 0 {
            return 0.0;
        }
        let base = self.kind.hull_mass() * self.reactor.tank_per_hull_mass();
        let base = if self.reactor == crate::fuel::ReactorGrade::Antimatter { base.max(crate::fuel::ANTIMATTER_BOTTLE_MIN) } else { base };
        base * self.loadout.fuel_capacity_mult()
    }
    /// Full hull — the tactical engine's `max_hp` for this kind.
    pub fn max_hp(&self) -> f64 {
        self.kind.hull_mass()
    }
    /// §shipbuilding-overhaul Phase 5: this hull's beam guns (one crystal
    /// each), Torpedo Racks and Mass Drivers.
    pub fn beam_guns(&self) -> f64 { crate::munitions::beam_guns(self.kind, &self.loadout) }
    pub fn racks(&self) -> f64 { crate::munitions::racks(&self.loadout) }
    pub fn drivers(&self) -> f64 { crate::munitions::drivers(&self.loadout) }
    /// Missing hull (never negative).
    pub fn deficit(&self) -> f64 {
        (self.max_hp() - self.hp).max(0.0)
    }
    /// The stack key this hull belongs to — `(kind, loadout key)`.
    pub fn stack_key(&self) -> String {
        self.loadout.key()
    }
}

/// §course-change: the drive spin-up / shut-down state machine.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveState {
    /// On thrusters. The ONLY state in which a fleet may change heading.
    #[default]
    Thrusters,
    /// Spinning a drive up. Still on thruster speed until it catches.
    Spooling {
        /// What it is spinning up INTO.
        to: crate::transit::Regime,
        /// Seconds left.
        left: f64,
    },
    /// Under way at `Regime` speed, locked on its current course.
    Cruising(crate::transit::Regime),
    /// Shutting down to thrusters. Still moving, still unable to steer.
    /// `from` preserves the exact transition duration; serde defaults it to Warp
    /// so pre-`from` snapshots load harmlessly.
    Dropping {
        #[serde(default = "crate::transit::Regime::warp")]
        from: crate::transit::Regime,
        left: f64,
    },
}

impl DriveState {
    /// Can the fleet change heading right now? Only on thrusters — which is the
    /// whole mechanic.
    pub fn can_steer(self) -> bool {
        matches!(self, DriveState::Thrusters)
    }

    /// The regime this state moves at. Transitions run at thruster speed: a
    /// drive that has not caught yet is not carrying you, and one that is
    /// shutting down has already let go.
    pub fn regime(self) -> crate::transit::Regime {
        match self {
            DriveState::Cruising(r) => r,
            _ => crate::transit::Regime::Thrusters,
        }
    }
}

/// The committed leg of an Intercept/Attack pursuit. Targets may turn, but a
/// warp drive cannot: the world holds this analytic lead point steady until the
/// bounded replan time, then pays the ordinary drop/turn/spool cost if needed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PursuitPlan {
    pub target: EntityId,
    /// Last discontinuous relocation seen when this leg was priced. A changed
    /// stamp invalidates the commitment immediately.
    #[serde(default)]
    pub target_jump: Option<f64>,
    pub aim: Vec2,
    pub replan_at: f64,
}

/// Derived fleet fuel effects. World refreshes these at tick boundaries and
/// after loading; completed corporate research remains the only authority.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FuelResearch {
    pub capacity: f64,
    pub consumption: f64,
    /// §fuel-overhaul: Fusion Freight's multiplier on a player freighter
    /// hull's fusion burn, and its cargo's (1 until researched).
    pub freighter_fusion: f64,
    /// Torch Regime's multiplier on a warship hull's cruise burn (never on
    /// freighters, and never on a jump).
    pub warship_burn: f64,
    /// Drive Tuning: every hull's speed.
    pub speed: f64,
    /// Military Drives and Torch Regime: a warship hull's speed.
    pub warship_speed: f64,
    /// §research-wiring Express Charters: a player freighter hull's speed.
    pub freighter_speed: f64,
    /// §research-wiring Long Patrol: a warship hull's cruise burn.
    pub warship_fuel: f64,
    /// §research-wiring Mass Streams: this fleet rides its owner's stream
    /// (set per fleet each tick; 1 off the stream).
    pub stream: f64,
    /// §syndicate-projects (R6): the Freight bonus on player freighter holds.
    pub cargo: f64,
    /// §holds: Prize Holds' multiplier on a Combat Freighter's hold.
    pub prize_hold: f64,
    /// §shipbuilding-overhaul Phase 5: Hardened Magazines' multiplier on the
    /// torpedo and slug magazines (never on crystals: one per gun).
    pub magazine: f64,
}

impl Default for FuelResearch {
    fn default() -> Self {
        Self { capacity: 1.0, consumption: 1.0, freighter_fusion: 1.0,
            warship_burn: 1.0, speed: 1.0, warship_speed: 1.0, freighter_speed: 1.0, warship_fuel: 1.0, stream: 1.0, cargo: 1.0,
            prize_hold: 1.0, magazine: 1.0 }
    }
}

impl FuelResearch {
    pub fn from_state(state: &crate::research::ResearchState) -> Self {
        let mods = crate::research::mods(state);
        Self {
            capacity: mods.get(&crate::research::ModKey::FuelCapacity).copied().unwrap_or(1.0),
            consumption: mods.get(&crate::research::ModKey::FuelConsumption).copied().unwrap_or(1.0),
            freighter_fusion: mods.get(&crate::research::ModKey::FreighterFusionBurn).copied().unwrap_or(1.0),
            warship_burn: mods.get(&crate::research::ModKey::WarshipBurn).copied().unwrap_or(1.0),
            speed: mods.get(&crate::research::ModKey::SpeedAll).copied().unwrap_or(1.0),
            warship_speed: mods.get(&crate::research::ModKey::SpeedWarship).copied().unwrap_or(1.0),
            freighter_speed: mods.get(&crate::research::ModKey::ConvoySpeed).copied().unwrap_or(1.0),
            warship_fuel: mods.get(&crate::research::ModKey::FuelOffensive).copied().unwrap_or(1.0),
            stream: 1.0,
            cargo: 1.0,
            prize_hold: mods.get(&crate::research::ModKey::RaidSteal).copied().unwrap_or(1.0),
            magazine: mods.get(&crate::research::ModKey::MagazineCapacity).copied().unwrap_or(1.0),
        }
    }
}

/// §hold-room (Nov 10 2026): the room left in a hold, in quarters of a room
/// unit (`crate::cargo::ROOM_SCALE`), so a quarter-room part and a whole-room
/// unit of ore draw on one integer. Capacities stay in room units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldRoom(u64);

impl HoldRoom {
    /// No limit: a pirate's first fleet stows a whole seizure.
    pub const UNLIMITED: Self = Self(u64::MAX);

    /// `capacity` room units, less `used` quarters.
    pub fn new(capacity: u32, used: u64) -> Self {
        Self((capacity as u64 * crate::cargo::ROOM_SCALE as u64).saturating_sub(used))
    }

    /// Units of `c` that fit.
    pub fn fits(self, c: Commodity) -> u32 {
        (self.0 / c.room_quarters() as u64).min(u32::MAX as u64) as u32
    }

    /// Stow up to `want` units of `c`: the units that fit, their room taken.
    pub fn take(&mut self, c: Commodity, want: u32) -> u32 {
        let n = want.min(self.fits(c));
        self.0 = self.0.saturating_sub(n as u64 * c.room_quarters() as u64);
        n
    }

    /// No room left at all.
    pub fn is_full(self) -> bool {
        self.0 == 0
    }

    /// Does a unit of any of `goods` fit? A hold with half a room unit left is
    /// full for ore but not for Alloys.
    pub fn fits_any(self, goods: impl IntoIterator<Item = Commodity>) -> bool {
        goods.into_iter().any(|c| self.fits(c) > 0)
    }

    /// The room left, in quarters.
    pub fn quarters(self) -> u64 {
        self.0
    }
}

/// A FLEET: the map/sim unit (GDD §13.1). One or more ships of mixed kinds
/// moving, fighting, and being observed as a SINGLE entity. A fleet-of-one is
/// the N=1 case and behaves exactly as the old single ship-per-unit world.
///
/// §roster: [`Fleet::ships`] is the SOURCE OF TRUTH — `composition` and
/// `loadouts` are a denormalized CACHE rebuilt from it after every mutation
/// ([`Fleet::rebuild_cache`]). Storing it twice is deliberate (the same pattern
/// syndicate membership uses): every hot read — detection signature, mass, fuel,
/// combat weight, the fog bucket — stays an O(1) map lookup instead of walking a
/// 300-hull roster at 30 Hz, and the wire projection is unchanged. The two are
/// pinned together by `roster_and_cache_always_agree`.
///
/// `composition` is a deterministic `BTreeMap` (id-sorted kind iteration) of how
/// many of each kind ride in the formation. It is never empty for a live fleet —
/// an emptied fleet is removed from the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fleet {
    pub id: EntityId,
    pub owner: PlayerId,
    #[serde(default)]
    pub pirate_faction: Option<crate::pirate::PirateFaction>,
    /// §pirate-dens: the den (an enclave's system) this pirate belongs to; it
    /// retreats, refuels and mends there. Kept current by `assign_pirate_dens`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pirate_den: Option<EntityId>,
    /// §roster: THE INDIVIDUAL HULLS — the source of truth. Kept sorted by `id`
    /// (deterministic iteration; a JSON array is far leaner than a keyed map at
    /// 300 hulls). `#[serde(default)]` + `fixup_after_load` synthesize a roster
    /// for every pre-roster snapshot, so old saves load pristine.
    #[serde(default)]
    pub ships: Vec<Ship>,
    /// Monotonic allocator for this fleet's ship ids.
    #[serde(default)]
    pub next_ship_id: u32,
    /// CACHE (derived from `ships`): how many of each kind ride in this
    /// formation. Never written directly — see [`Fleet::rebuild_cache`].
    pub composition: BTreeMap<ShipKind, u32>,
    pub pos: Vec2,
    pub vel: Vec2,
    pub order: FleetOrder,
    /// Stable warp leg for a moving-target order. `None` for every other order;
    /// serde default preserves pre-planner snapshots.
    #[serde(default)]
    pub pursuit_plan: Option<PursuitPlan>,
    /// True-time stamp of the last discontinuous relocation. The view samples
    /// this in commit 3; no player-facing surface reads it directly.
    #[serde(default)]
    pub last_jump: Option<f64>,
    /// Cargo carried (convoys only; raiders carry none). Broadcast withholds
    /// this — it is revealed by sensor range, not by the Convention. Capacity
    /// scales with the number of convoys aboard; existing single-convoy rules
    /// are the N=1 case, unchanged.
    pub cargo: Option<Cargo>,
    /// Additional commodity stacks in this fleet's hold. `cargo` remains the
    /// primary stack so pre-manifest snapshots deserialize unchanged; all live
    /// cargo logic goes through the manifest helpers below and treats the two
    /// fields as one hold. New stacks use this map once the primary commodity
    /// differs, allowing a Convoy to carry a mixed founding/supply package.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cargo_extra: BTreeMap<crate::cargo::Commodity, u32>,
    /// If set, this is a trade convoy fleet that resolves on arrival (§9).
    pub mission: Option<TradeMission>,
    /// If set, this fleet is on an AUTONOMOUS defensive intercept (it broke off
    /// patrol to engage a threat) — server-driven standing doctrine, runs whether
    /// or not the owner is connected.
    #[serde(default)]
    pub defense: Option<DefenseEngagement>,
    /// COLONY fleets only (§ships part 3): the "arrived at an already-claimed
    /// system" notice has been sent for the current hold, so it isn't re-sent
    /// every tick. Cleared whenever the fleet moves again. serde default = false.
    #[serde(default)]
    pub notified_held: bool,
    /// §ground G1: marines this formation has already put on the ground. Hulls
    /// survive a landing; the men in them do not, so without this a capital
    /// group would land the same boarding parties every tick forever. Cleared
    /// by re-embarking at an owned Garrison. serde default = 0 (pre-feature
    /// fleets are fully manned, which is what they were).
    #[serde(default)]
    pub marines_spent: u32,
    /// Is the WARP DRIVE lit? Warp works anywhere outside a gravity well and is
    /// a state a fleet is in rather than a separate map layer.
    ///
    /// Warp off is slow and quiet — since
    /// signature already scales with speed (§6.3), going dark and going slow are
    /// the same act. serde default = false, so pre-feature fleets load on
    /// thrusters.
    #[serde(default)]
    pub warp: bool,
    /// FUEL IN THE TANKS. Carried, not drawn.
    ///
    /// This used to live in system stockpiles, and a moving fleet reached back
    /// to whichever of its owner's systems could pay — from any distance, every
    /// tick. That is what made running dry need an invented failure mode, and
    /// the one that shipped did not work: movement happened first and only
    /// zeroed velocity afterwards, which `advance` recomputes, so a fleet with
    /// no fuel flew on regardless. It is also why automation had to PREDICT
    /// affordability from straight-line estimates, and why hauling fuel needed
    /// an exemption to avoid deadlocking its own resupply.
    ///
    /// Carrying it deletes all of that. A fleet cannot move without fuel because
    /// moving IS decrementing a number it holds, and range stops being a
    /// bookkeeping question and becomes a property of the formation.
    ///
    /// §fuel-overhaul: this is the FUSION TANK, counted in Deuterium of energy.
    /// It includes any Helium-3 aboard (each unit [`crate::fuel::HELIUM3_FUEL_EQUIV`]
    /// of energy in one unit of space), so `fuel` can exceed the tank's space.
    #[serde(default)]
    pub fuel: f64,
    /// §fuel-overhaul: Helium-3 units inside the fusion tank (their energy is
    /// already counted in `fuel`). Space used = Deuterium + Helium-3 units.
    #[serde(default)]
    pub helium3: f64,
    /// §fuel-overhaul: the FISSION STORE (Uranium), burned by Slow warp.
    #[serde(default)]
    pub uranium: f64,
    /// §fuel-overhaul: the ANTIMATTER BOTTLE on jump-capable hulls.
    #[serde(default)]
    pub antimatter: f64,
    /// §shipbuilding-overhaul Phase 5: the MAGAZINES (`crate::munitions`).
    /// Torpedoes for the racks and Slugs for the drivers, filled at a dock
    /// like a tank and drawn down round by round in battle (rules v17); Beam
    /// Crystals in the beam guns, one per gun, fresh with the hull and worn by
    /// firing. Fleets no player pays for (pirates, the Authority) carry none
    /// and are exempt.
    #[serde(default)]
    pub torpedoes: f64,
    #[serde(default)]
    pub slugs: f64,
    #[serde(default)]
    pub crystals: f64,
    /// Runtime cache, not a saved upgrade or a fuel grant. Rebuilt from the
    /// owner's research; old snapshots need no new authoritative fields.
    #[serde(skip)]
    pub(crate) fuel_research: FuelResearch,
    /// §course-change: what the drives are DOING — cruising, spinning up, or
    /// shutting down.
    ///
    /// A fleet cannot steer above thrusters. Changing course means dropping all
    /// the way out, turning, and re-entering, and each of those transitions
    /// takes time. Holding it as a state rather than deriving it per tick is
    /// what makes the cost real: a reversal is a sequence the fleet has to live
    /// through, not an instantaneous re-aim.
    #[serde(default)]
    pub drive_state: DriveState,
    /// Which cruise regime this fleet was moving through, as of the last
    /// tick it moved. Recorded rather than re-derived so the badge the map shows
    /// is the regime the fleet was actually flown at.
    #[serde(default)]
    pub regime: crate::transit::Regime,
    /// This fleet has run dry and is holding. A latch, so the
    /// owner-only notice fires once per stall rather than thirty times a second.
    /// Cleared the tick it is refuelled.
    #[serde(default)]
    pub stalled: bool,
    /// Per-kind DAMAGE POOLS accumulated in an ongoing engagement (Part 2,
    /// Lanchester attrition). Empty when not/never engaged; serde default keeps
    /// old snapshots loading. A kind's ships die whole once its pool ≥ its hull,
    /// carrying the remainder forward.
    #[serde(default)]
    pub damage: BTreeMap<ShipKind, f64>,
    /// TRANSIT THROTTLE: Full (default), Stealth or Slow (Uranium). Governs move speed
    /// and, via the retarded velocity, the fleet's detection signature.
    #[serde(default)]
    pub transit: TransitMode,
    /// The one scripted founding privateer is a visibly limping Raider. Keeping
    /// the marker on the fleet confines the speed handicap to that encounter;
    /// ordinary pirates and every player Raider retain the normal hull table.
    #[serde(default)]
    pub founding_privateer: bool,
    /// Weak, full-hull opposition confined to optional post-privateer contracts.
    #[serde(default)]
    pub operation_privateer: bool,
    /// §vendetta: the privateer's uncle and his escorts, whose guns the
    /// charter's finale scales (`founding::VENDETTA_DAMAGE_MULT`).
    #[serde(default)]
    pub vendetta_pirate: bool,
    /// §player-styles: a personal pirate hideout's guards, whose guns are
    /// scaled (`pirate::HIDEOUT_DAMAGE_MULT`).
    #[serde(default)]
    pub hideout_crew: bool,
    /// §player-styles: the system whose owner has been warned of this fleet's
    /// raid or blockade, once a trip.
    #[serde(default)]
    pub raid_sighted: Option<EntityId>,
    /// §player-styles: a raid set to repeat, from its first trip until it
    /// stops or is recalled.
    #[serde(default)]
    pub raid_repeat: Option<RaidRepeat>,
    /// §economy Part 4: SPECIALIST PASSENGERS aboard (kind → headcount) —
    /// people, not cargo, but they ride the SAME two-tier fog rule: the
    /// broadcast never includes them, a sensor-revealed manifest does. Berths
    /// come from the logistics hulls (`specialist::passenger_capacity`). Lost
    /// with the fleet (the one specialist loss rule); a merging fleet folds
    /// them in; a consumed colony ship DISEMBARKS them into the new colony.
    /// serde default keeps every old snapshot loading (nobody aboard).
    #[serde(default)]
    pub passengers: BTreeMap<crate::specialist::SpecialistKind, u32>,
    /// ENGAGEMENT POSTURE (§offensive-orders Part 2): standing per-fleet aggression
    /// — Passive (default), Defensive, or WeaponsFree. serde default = Passive so
    /// every old snapshot loads with today's behaviour (byte-preserving).
    #[serde(default)]
    pub posture: crate::doctrine::EngagementPosture,
    #[serde(default)]
    pub mission_profile: crate::doctrine::MissionProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub industry: Option<crate::industry::FleetIndustry>,
    /// §belts: a Mining Belt Station's anchor at a belt (none for any other hull).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<crate::industry::Anchor>,
    /// §real-coordinates: the world this fleet came to rest at, and rides as
    /// it orbits. Set on arrival at a system, cleared when it sets out again
    /// or is caught in a fight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_world: Option<crate::orbits::AtWorld>,
    /// §syndicates Part 3: when this fleet is an ALLY GARRISON (stationed at an
    /// ally's system), whether the HOST is currently feeding it its Provisions
    /// upkeep. Recomputed every tick (like `habitat_fed`); UNFED = its defense
    /// contribution suspends (never destroyed). Meaningless (always `true`) for a
    /// fleet that isn't a garrison. serde default `true` so old snaps load fed.
    #[serde(default = "default_true")]
    pub garrison_fed: bool,
    /// §upkeep: is this fleet's standing Provisions upkeep currently being met?
    /// Recomputed every tick from the owner's stockpiles. An UNSUPPLIED fleet is
    /// IMMOBILIZED — it accepts no new movement or offensive order — but it is
    /// never destroyed, never disarmed, and never stopped mid-flight: it still
    /// defends itself, still finishes the leg it is on, and recovers the instant
    /// food reaches it. Shortages suspend; they do not destroy (§5.1). serde
    /// default `true` so every pre-upkeep snapshot loads supplied.
    #[serde(default = "default_true")]
    pub supplied: bool,
    /// §plunder: the sub-unit remainder of goods stripped from a blockaded
    /// system. Plunder accrues at a per-second rate but loads in WHOLE units, so
    /// the fraction rides here between ticks rather than rounding away (which
    /// would silently change the effective rate). serde default 0.0.
    #[serde(default)]
    pub plunder_frac: f64,
    /// §raid-order: this fleet has taken loot on its current raid, so a
    /// contested blockade sends it home rather than waiting.
    #[serde(default)]
    pub raid_looted: bool,
    /// §rankings: has this fleet EVER been a participant in an engagement? Latches
    /// true when the fleet joins any battle, so a convoy that fought and STILL
    /// delivered can be credited "cargo protected" on arrival. serde default false
    /// keeps old snapshots loading (never-fought).
    #[serde(default)]
    pub fought: bool,
    /// §modules Part B: the LOADOUT partition — `kind → loadout key → count`,
    /// storing ONLY non-default (fitted) stacks. The invariant `Σ loadouts[kind]
    /// ≤ composition[kind]` holds; the remainder is implicitly UNFITTED (the
    /// stock beam brawler). `#[serde(default)]` → every old snapshot loads with
    /// no field → all-unfitted → zero migration.
    #[serde(default)]
    pub loadouts: crate::combat::LoadoutMap,
    /// §TCA: while this fleet BLOCKADES a system, also engage Terran Charter
    /// Authority FREIGHTERS that arrive there. Off by default — a blockade
    /// strangles a rival's own logistics without picking a fight with the
    /// chartering power, and in Phase 1 that choice is free (the law arrives in
    /// Phase 2). When on, an arriving freighter becomes an ordinary hostile
    /// contact and the existing raid-vs-battle logic decides the rest. serde
    /// default `false` so every old snapshot loads with today's behaviour.
    #[serde(default)]
    pub engage_freight: bool,
    /// Standing policy for a freighter fleet: when a held blockade cuts off the
    /// system it is delivering to, RUN the blockade instead of holding outside
    /// it until it lifts (§blockade-running). serde default `false`, so
    /// deliveries wait a blockade out as they always have.
    #[serde(default)]
    pub run_blockades: bool,
    /// The blockaded system this passage has already rolled against: one roll
    /// per crossing of its run zone. Cleared when the fleet leaves the zone or
    /// stops in it, so going in and coming back out are separate runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blockade_run: Option<EntityId>,
    /// This delivery is WAITING OUTSIDE the held blockade of this system: it
    /// holds on the standoff ring and hasn't arrived, so its mission stays
    /// open until the blockade lifts (or it is set to run it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_outside: Option<EntityId>,
    /// §TCA Part 5: this hull was AUTO-SPAWNED for one special one-way delivery and
    /// is CONSUMED when it arrives. Standing orders and ordinary commodity
    /// shipments no longer set this flag: they use a real player hull or Authority
    /// freight. A hull the
    /// player actually owns and loaded is not disposable: it survives its delivery
    /// and goes Idle, ready for the next job. Without this, repointing the hub
    /// endpoint at the surviving `DeliverToWarehouse` mission would quietly turn
    /// standing orders into a free-convoy factory. serde default `false`.
    #[serde(default)]
    pub disposable: bool,
    /// §contract-props: transponder off whatever it flies (a rescue
    /// contract's stranded liner). Only sensors find it, so it never shows
    /// on anyone's map from beyond them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub silent: bool,
}

/// serde default for `Fleet::garrison_fed` (old snapshots load fed).
fn default_true() -> bool {
    true
}

impl Fleet {
    /// Report the standing post even while its fleet is on a defensive sortie.
    /// Manual reassignment clears the sortie at delivery, not when clicked.
    pub fn system_defense(&self) -> Option<SystemDefense> {
        match self.order {
            FleetOrder::DefendSystem { assignment } => Some(assignment),
            FleetOrder::Intercept { target } => self.defense.as_ref()
                .filter(|d| d.target == target).and_then(|d| d.system),
            _ => None,
        }
    }
    /// Build a FLEET-OF-ONE — the migration/spawn primitive. Every place the old
    /// world made a `Ship::new(...)` makes a `Fleet::single(...)`, so the N=1
    /// world is byte-for-byte the same behaviour.
    pub fn single(
        id: EntityId,
        owner: PlayerId,
        kind: ShipKind,
        pos: Vec2,
        order: FleetOrder,
        cargo: Option<Cargo>,
    ) -> Self {
        let mut composition = BTreeMap::new();
        composition.insert(kind, 1);
        // A fleet-of-one starts with a full tank: the founding fleet,
        // replacements and scripted hulls. §fuel-overhaul: a hull built at a yard
        // is emptied at launch and fills at the yard's dock (`launch_empty`).
        let mut fleet = Fleet {
            id,
            owner,
            // §roster: the fleet-of-one primitive is one HULL at full health.
            ships: vec![Ship::new(0, kind, crate::module::Loadout::default())],
            next_ship_id: 1,
            composition,
            pos,
            vel: Vec2::ZERO,
            order,
            pursuit_plan: None,
            last_jump: None,
            cargo,
            cargo_extra: BTreeMap::new(),
            mission: None,
            defense: None,
            notified_held: false,
            marines_spent: 0,
            warp: false,
            fuel: 0.0,
            helium3: 0.0,
            uranium: 0.0,
            antimatter: 0.0,
            torpedoes: 0.0,
            slugs: 0.0,
            crystals: 0.0,
            fuel_research: FuelResearch::default(),
            regime: crate::transit::Regime::Thrusters,
            drive_state: DriveState::default(),
            stalled: false,
            damage: BTreeMap::new(),
            transit: TransitMode::Full,
            founding_privateer: false,
            operation_privateer: false,
            vendetta_pirate: false,
            hideout_crew: false,
            raid_sighted: None,
            raid_repeat: None,
            passengers: BTreeMap::new(),
            posture: crate::doctrine::EngagementPosture::Passive,
            pirate_faction: None,
            pirate_den: None,
            mission_profile: crate::doctrine::MissionProfile::default(),
            industry: None,
            anchor: None,
            at_world: None,
            garrison_fed: true,
            supplied: true,
            plunder_frac: 0.0,
            raid_looted: false,
            fought: false,
            loadouts: BTreeMap::new(),
            engage_freight: false,
            run_blockades: false,
            blockade_run: None,
            held_outside: None,
            disposable: false,
            silent: false,
        };
        fleet.fill_stores();
        fleet
    }

    /// A fleet-of-one on `engine` (reactor, drive), tank full.
    pub fn single_on(id: EntityId, owner: PlayerId, kind: ShipKind, engine: crate::fuel::Engine, pos: Vec2,
        order: FleetOrder, cargo: Option<Cargo>) -> Self
    {
        let mut fleet = Self::single(id, owner, kind, pos, order, cargo);
        for s in &mut fleet.ships { s.reactor = engine.0; s.drive = engine.1; }
        fleet.launch_empty();
        fleet.fill_stores();
        fleet
    }

    /// §fuel-overhaul: a fleet-of-one flying Slow (the founding freighter's
    /// default throttle).
    pub fn single_slow(id: EntityId, owner: PlayerId, kind: ShipKind, pos: Vec2,
        order: FleetOrder, cargo: Option<Cargo>) -> Self
    {
        let mut fleet = Self::single(id, owner, kind, pos, order, cargo);
        fleet.transit = TransitMode::Slow;
        fleet
    }

    // --- §roster: the ROSTER is truth; composition/loadouts are its cache ------

    /// Rebuild the denormalized `composition` + `loadouts` cache from `ships`.
    /// Called after EVERY roster mutation — the one place the two can be brought
    /// back into agreement, so they can never silently drift.
    pub fn rebuild_cache(&mut self) {
        self.ships.sort_by_key(|s| s.id); // deterministic iteration + JSON order
        self.composition.clear();
        self.loadouts.clear();
        for s in &self.ships {
            *self.composition.entry(s.kind).or_insert(0) += 1;
            if !s.loadout.is_empty() {
                *self
                    .loadouts
                    .entry(s.kind)
                    .or_default()
                    .entry(s.stack_key())
                    .or_insert(0) += 1;
            }
        }
    }

    /// Allocate the next fleet-local ship id.
    fn alloc_ship_id(&mut self) -> u32 {
        let id = self.next_ship_id;
        self.next_ship_id += 1;
        id
    }

    /// Push `n` fresh hulls of `(kind, loadout)` at FULL health and refresh the
    /// cache. The single entry point for new ships joining a fleet.
    pub fn add_fitted(&mut self, kind: ShipKind, loadout: &crate::module::Loadout, n: u32) {
        for _ in 0..n {
            let id = self.alloc_ship_id();
            self.ships.push(Ship::new(id, kind, loadout.clone()));
        }
        if n > 0 {
            self.rebuild_cache();
        }
    }

    /// Replace the ENTIRE roster with `n` fresh unfitted hulls of `kind` — the
    /// spawn primitive for scripted formations (pirate packs, Authority
    /// enforcement squadrons) and for test setup that wants a known fleet.
    pub fn reset_to(&mut self, kind: ShipKind, n: u32) {
        self.ships.clear();
        self.next_ship_id = 0;
        self.add_fitted(kind, &crate::module::Loadout::default(), n);
        self.launch_empty();
        self.fill_stores();
    }

    /// Every tank and magazine full for the hulls aboard now. A spawned
    /// formation is sized after `Fleet::single`, whose tank fits one hull.
    pub fn fill_stores(&mut self) {
        for c in crate::fuel::ReactorGrade::ALL.map(|g| g.fuel()).into_iter().chain(crate::munitions::MUNITIONS) {
            let cap = self.tank_capacity(c);
            let store = self.store_mut(c);
            *store = store.max(cap);
        }
    }

    /// Push `n` fresh hulls of `(kind, loadout)` on `engine` (reactor, drive)
    /// at FULL health and refresh the cache.
    pub fn add_fitted_on(&mut self, kind: ShipKind, loadout: &crate::module::Loadout, engine: crate::fuel::Engine, n: u32) {
        for _ in 0..n {
            let id = self.alloc_ship_id();
            let ship = Ship::with_engine(id, kind, loadout.clone(), engine);
            // §shipbuilding-overhaul Phase 5: a new gun comes with its crystal;
            // racks and drivers launch empty and load at the dock.
            self.crystals += ship.beam_guns();
            self.ships.push(ship);
        }
        if n > 0 {
            self.rebuild_cache();
        }
    }

    /// §reactors-drives: refit `n` hulls of `kind` to `reactor` and/or
    /// `drive` (None keeps that part), those not yet on the target first by
    /// id. Returns the engines that came out, one per hull changed, for the
    /// yard's stockpile. Tanks of a fuel no hull burns any more are clamped by
    /// the caller.
    pub fn set_engines(&mut self, kind: ShipKind, reactor: Option<crate::fuel::ReactorGrade>,
        drive: Option<crate::fuel::Drive>, n: u32) -> Vec<crate::fuel::Engine>
    {
        let target = |s: &Ship| (reactor.unwrap_or(s.reactor), drive.unwrap_or(s.drive));
        let mut idx: Vec<usize> = (0..self.ships.len())
            .filter(|i| self.ships[*i].kind == kind && self.ships[*i].engine() != target(&self.ships[*i]))
            .collect();
        idx.sort_by_key(|i| self.ships[*i].id);
        idx.truncate(n as usize);
        let mut out = Vec::with_capacity(idx.len());
        for i in idx {
            let to = target(&self.ships[i]);
            out.push(self.ships[i].engine());
            self.ships[i].reactor = to.0;
            self.ships[i].drive = to.1;
        }
        out
    }

    /// ABSORB hulls from another fleet (merge / relief / a completed refit),
    /// re-issuing ids from this fleet's allocator. Damage travels WITH the hull.
    pub fn absorb_ships(&mut self, incoming: Vec<Ship>) {
        if incoming.is_empty() {
            return;
        }
        for mut s in incoming {
            s.id = self.alloc_ship_id();
            self.ships.push(s);
        }
        self.rebuild_cache();
    }

    /// DETACH `n` hulls of `kind` for a split, FITTED FIRST (so a detached
    /// escort keeps its fits — the pre-roster `detach_loadouts` behaviour) and
    /// then by id. Returns the actual hulls, damage and all.
    pub fn detach_ships(&mut self, kind: ShipKind, n: u32) -> Vec<Ship> {
        let mut idx: Vec<usize> = (0..self.ships.len())
            .filter(|i| self.ships[*i].kind == kind)
            .collect();
        // Fitted first, then by id — deterministic.
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            x.loadout
                .is_empty()
                .cmp(&y.loadout.is_empty())
                .then(x.id.cmp(&y.id))
        });
        idx.truncate(n as usize);
        idx.sort_unstable();
        let mut out = Vec::with_capacity(idx.len());
        for i in idx.into_iter().rev() {
            out.push(self.ships.remove(i));
        }
        out.reverse();
        if !out.is_empty() {
            self.rebuild_cache();
        }
        out
    }

    /// RE-FIT `n` existing hulls of `kind` to `loadout` in place, taking the
    /// currently-unfitted ones first (by id — deterministic). This is what a
    /// completed refit does to hulls that never left, and the natural way to
    /// express "this wing flies fitted" in setup. Returns how many were changed.
    pub fn set_fitted(&mut self, kind: ShipKind, loadout: &crate::module::Loadout, n: u32) -> u32 {
        let mut idx: Vec<usize> = (0..self.ships.len())
            .filter(|i| self.ships[*i].kind == kind)
            .collect();
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            y.loadout
                .is_empty()
                .cmp(&x.loadout.is_empty())
                .then(x.id.cmp(&y.id))
        });
        idx.truncate(n as usize);
        let changed = idx.len() as u32;
        for i in idx {
            self.ships[i].loadout = loadout.clone();
        }
        if changed > 0 {
            self.rebuild_cache();
        }
        changed
    }

    /// The roster entry for `(kind, ship id)`, if it is still aboard.
    pub fn ship_mut(&mut self, kind: ShipKind, id: u32) -> Option<&mut Ship> {
        self.ships.iter_mut().find(|s| s.id == id && s.kind == kind)
    }

    /// §ground: the MARINES this formation can put on the ground — summed over
    /// its hulls. What an assault is measured in.
    pub fn marines(&self) -> u32 {
        let carried: u32 = self.ships.iter().map(|s| marine_capacity(s.kind)).sum();
        carried.saturating_sub(self.marines_spent)
    }

    /// §veil: the marines aboard TROOP TRANSPORTS only — the hulls hardened and
    /// slow enough to get down through a Veil. A capital's boarding party stays
    /// aboard while a Veil is up. Spent complements come off this first, since
    /// a landing commits the whole formation.
    pub fn transport_marines(&self) -> u32 {
        let carried: u32 = self.ships.iter().filter(|s| s.kind == ShipKind::Transport)
            .map(|s| marine_capacity(s.kind)).sum();
        carried.saturating_sub(self.marines_spent)
    }

    /// §ground G1: mark this formation's whole complement as COMMITTED. Troops
    /// that have gone down cannot go down twice — without this a capital group
    /// would re-land its boarding parties every tick, since the hulls survive a
    /// landing even when the men do not. Cleared by re-embarking (§ground).
    pub fn commit_marines(&mut self) {
        self.marines_spent = self.ships.iter().map(|s| marine_capacity(s.kind)).sum();
    }

    /// Take on fresh troops — only at an owned colony with a Garrison to draw
    /// them from. This is why a capital group that has spent its parties has to
    /// go home before it can threaten ground again.
    pub fn reembark_marines(&mut self) {
        self.marines_spent = 0;
    }

    /// Operating personnel on the actual hull roster, including their officers.
    /// No extra captain charge: that is a role within the complement.
    pub fn crew_count(&self) -> u32 {
        self.ships.iter().map(|s| crew_size(s.kind)).sum()
    }

    /// Everyone currently aboard eats, including settlers and specialists.
    /// Deployed marines no longer draw shipboard rations; cargo never adds crew.
    pub fn provisions_headcount(&self) -> u32 {
        self.crew_count()
            + self.marines()
            + self.ships.iter().map(|s| colonists_aboard(s.kind)).sum::<u32>()
            + self.passengers.values().sum::<u32>()
    }

    /// §upkeep: standing food demand, whether moving or docked. Use the colony's
    /// per-person baseline, never a second per-hull food-price table.
    pub fn upkeep_per_sec(&self) -> f64 {
        f64::from(self.provisions_headcount()) * crate::colony::PROVISIONS_PER_PERSON_PER_S
    }

    /// Total missing hull across the whole formation, and its full-health total —
    /// the aggregate the View reports (never per-hull; that would leak counts).
    pub fn damage_fraction(&self) -> f64 {
        let max: f64 = self.ships.iter().map(|s| s.max_hp()).sum();
        if max <= 0.0 {
            return 0.0;
        }
        (self.ships.iter().map(|s| s.deficit()).sum::<f64>() / max).clamp(0.0, 1.0)
    }

    // --- §modules Part B: the LOADOUT partition (invariant: Σ ≤ composition) ---

    /// Fitted ships of `kind` (Σ its loadout stacks); the rest are unfitted.
    pub fn fitted_count(&self, kind: ShipKind) -> u32 {
        self.loadouts
            .get(&kind)
            .map(|m| m.values().sum())
            .unwrap_or(0)
    }

    // §roster: `fold_loadouts` / `detach_loadouts` are RETIRED. They moved fits
    // as counts, separately from the hulls, which the roster makes both
    // unnecessary and wrong — a hull's fit AND its accumulated damage now travel
    // together in the `Ship` record. Use `absorb_ships` / `detach_ships`.

    /// Remove `n` ships from the `(kind, loadout)` stack — decrements
    /// `composition[kind]` and, for a fitted loadout, its stack count. Returns
    /// how many were removed (clamped to what's in that stack).
    /// TAKE up to `n` hulls out of the `(kind, loadout)` stack and RETURN them,
    /// most-damaged first. Combat kills the ships that were already hurt (which
    /// is both intuitive and keeps a surviving roster healthy rather than
    /// uniformly chipped); a yard likewise takes the neediest hulls first.
    ///
    /// Returning the hulls — rather than a count — is what lets a caller that
    /// intends to give them BACK (the refit queue) preserve their health. A
    /// caller that is destroying them just drops the Vec.
    pub fn take_stack(
        &mut self,
        kind: ShipKind,
        loadout: &crate::module::Loadout,
        n: u32,
    ) -> Vec<Ship> {
        let key = loadout.key();
        let mut idx: Vec<usize> = (0..self.ships.len())
            .filter(|i| self.ships[*i].kind == kind && self.ships[*i].stack_key() == key)
            .collect();
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            y.deficit()
                .partial_cmp(&x.deficit())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(x.id.cmp(&y.id))
        });
        idx.truncate(n as usize);
        if idx.is_empty() {
            return Vec::new();
        }
        idx.sort_unstable();
        let mut out = Vec::with_capacity(idx.len());
        for i in idx.into_iter().rev() {
            out.push(self.ships.remove(i));
        }
        out.reverse();
        self.rebuild_cache();
        out
    }

    /// Remove `n` ships from the `(kind, loadout)` stack, discarding them.
    /// Returns how many went (the loss-accounting path).
    pub fn remove_stack(
        &mut self,
        kind: ShipKind,
        loadout: &crate::module::Loadout,
        n: u32,
    ) -> u32 {
        self.take_stack(kind, loadout, n).len() as u32
    }

    /// §roster: SUPERSEDED — the loadout partition is a projection of the roster
    /// now, so it cannot drift out of the invariant in the first place. Kept as a
    /// named entry point (and for any hand-edited cache) that simply rebuilds it
    /// from the hulls, which is the strongest form of "normalize" available.
    pub fn normalize_loadouts(&mut self) {
        self.rebuild_cache();
    }

    /// How many ships of `kind` ride in this fleet (0 if none).
    pub fn count(&self, kind: ShipKind) -> u32 {
        self.composition.get(&kind).copied().unwrap_or(0)
    }

    /// Does the fleet contain at least one ship of `kind`?
    pub fn contains(&self, kind: ShipKind) -> bool {
        self.count(kind) > 0
    }

    /// Total ship count across all kinds.
    pub fn total_count(&self) -> u32 {
        self.composition.values().copied().sum()
    }

    /// The estimated-size bucket a fog observer sees (never the exact count).
    pub fn count_class(&self) -> CountClass {
        CountClass::from_count(self.total_count())
    }

    /// Add `n` UNFITTED ships of `kind` at full health (§roster), tanks full:
    /// the scripted-spawn primitive (packs growing, relief squadrons, test
    /// setup). A yard build joins through `add_fitted_on` and fills at the dock.
    pub fn add(&mut self, kind: ShipKind, n: u32) {
        self.add_fitted(kind, &crate::module::Loadout::default(), n);
        self.fill_stores();
    }

    /// Remove up to `n` ships of `kind`. Sheds UNFITTED hulls first (the
    /// pre-roster behaviour, where `normalize_loadouts` trimmed the fitted
    /// partition last), then by id — deterministic. Returns how many went.
    pub fn remove(&mut self, kind: ShipKind, n: u32) -> u32 {
        let mut idx: Vec<usize> = (0..self.ships.len())
            .filter(|i| self.ships[*i].kind == kind)
            .collect();
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            y.loadout
                .is_empty()
                .cmp(&x.loadout.is_empty())
                .then(x.id.cmp(&y.id))
        });
        idx.truncate(n as usize);
        let take = idx.len() as u32;
        if take == 0 {
            return 0;
        }
        idx.sort_unstable();
        for i in idx.into_iter().rev() {
            self.ships.remove(i);
        }
        self.rebuild_cache();
        take
    }

    /// Remove exactly one ship of `kind` (e.g. a colony consumed on claim).
    /// Returns true if one was present and removed.
    pub fn remove_one(&mut self, kind: ShipKind) -> bool {
        self.remove(kind, 1) == 1
    }

    /// True once the fleet has no ships left — it should be removed from the world.
    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }

    /// The kind this fleet is DRAWN and named for (flagship precedence). For a
    /// fleet-of-one this is simply that ship's kind.
    pub fn flagship_kind(&self) -> ShipKind {
        for k in FLAGSHIP_PRECEDENCE {
            if self.contains(k) {
                return k;
            }
        }
        // A live fleet is never empty; fall back defensively.
        ShipKind::Scout
    }

    /// A fleet BROADCASTS (Convention, visible galaxy-wide) if ANY member kind
    /// broadcasts. A corporate Convoy remains dark; the Authority's Freighter
    /// remains public even in a mixed formation. Pirates are outlaws, not
    /// signatories: their Corvettes and capitals run dark like their Raiders and
    /// are found only by detection, never shown galaxy-wide. The View, pickets
    /// and defenses all read this one rule.
    pub fn broadcasts(&self) -> bool {
        !self.silent && !self.owner.is_pirate() && self.composition.keys().any(|k| k.broadcasts())
    }

    /// A mobile sensor source exists for an Interceptor, Freighter, or a Scout
    /// fitted with Recon. Their ranges remain distinct through `sensor_mult`.
    pub fn projects_sensor(&self) -> bool {
        self.composition.keys().any(|k| k.projects_sensor()) || self.has_recon()
    }

    /// A mixed formation jumps only when every hull carries a drive.
    /// The fewest Warp Stabilizers any live hull carries: a fleet slips a
    /// tackle only when every hull can.
    pub fn min_stabilizers(&self) -> usize {
        self.ships.iter().filter(|s| s.hp > 0.0)
            .map(|s| s.loadout.modules().iter().filter(|m| **m == crate::module::ModuleKind::WarpStabilizer).count())
            .min()
            .unwrap_or(0)
    }

    /// §shipbuilding-overhaul: a fleet jumps only when every hull with engines
    /// rides a Fold Core (and at least one does).
    pub fn can_jump(&self) -> bool {
        self.ships.iter().any(|s| s.kind.engines() > 0 && s.drive.jumps())
            && self.ships.iter().all(|s| s.kind.engines() == 0 || s.drive.jumps())
    }

    /// §explore: is this fleet ACTIVELY SURVEYING (in the dwell window)? Drives
    /// the loudness multiplier through the one shared signature path — true only
    /// while the dwell clock runs, never during the approach.
    /// §belts: survey dwells and belt mining are LOUD work; both take the
    /// ×1.5 signature factor (`explore::SURVEY_SIGNATURE_FACTOR`).
    pub fn loud_work(&self) -> bool {
        self.surveying()
            || matches!(&self.industry,
                Some(crate::industry::FleetIndustry::Mine { status, .. }) if status == "mining")
            || matches!(&self.industry,
                Some(crate::industry::FleetIndustry::Prospect { status, .. }) if status == "prospecting")
    }

    pub fn surveying(&self) -> bool {
        matches!(
            self.order,
            FleetOrder::Survey {
                dwell_since: Some(_),
                ..
            } | FleetOrder::Expedition { dwell_since: Some(_), .. }
        )
    }

    /// The range multiplier of a mobile sensor fleet. A Raider carries the full
    /// bubble and can be boosted by a Scout; a Convoy alone uses its traffic
    /// suite. A stock Scout is passive; a Recon-equipped one carries sensors.
    pub fn sensor_mult(&self) -> f64 {
        let stock = if self.contains(ShipKind::Raider) {
            self.composition
                .keys()
                .map(|k| k.sensor_mult())
                .fold(1.0_f64, f64::max)
        } else if self.has_freighter() {
            CONVOY_SENSOR_MULT
        } else {
            0.0
        };
        // Scout: a new 40k bubble; Interceptor: 80k → 120k. These are local
        // instruments, not faster light. Mixed formations take the best source,
        // never multiply every hull's bonus together. Config's base is 80k.
        let recon = if self.has_recon_on(ShipKind::Raider) { 1.5 }
            else if self.has_recon_on(ShipKind::Scout) { 0.5 } else { 0.0 };
        stock.max(recon)
    }

    pub fn has_recon_on(&self, kind: ShipKind) -> bool {
        self.loadouts.get(&kind).is_some_and(|fits| fits.iter().any(|(key, n)|
            *n > 0 && crate::module::Loadout::from_key(key).has_recon()))
    }

    pub fn has_recon(&self) -> bool {
        self.has_recon_on(ShipKind::Scout) || self.has_recon_on(ShipKind::Raider)
    }

    pub fn exploration_range_mult(&self) -> f64 { if self.has_recon() { 2.0 } else { 1.0 } }

    /// A specialist instrument finds stationary sites only; it does not turn a
    /// civilian Scout into a combat sensor or shorten any information delay.
    pub fn has_spectrometer(&self) -> bool {
        self.loadouts.get(&ShipKind::Scout).is_some_and(|fits| fits.iter().any(|(key, n)|
            *n > 0 && crate::module::Loadout::from_key(key).has_spectrometer()))
    }

    /// Total EMPTY-HULL mass = Σ hull_mass(kind) × count.
    pub fn hull_mass(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.hull_mass() * *n as f64)
            .sum()
    }

    /// Commodity hold space belongs to each Freighter hull. Cargo Pods double
    /// only fitted hulls; the rest of a mixed fleet keeps its original capacity.
    /// §syndicate-projects (R6): the Freight bonus widens player freighter
    /// holds, rounded down per hull kind. §holds: includes loot-only holds;
    /// anything loaded from a dock, a market or a route uses
    /// [`Self::trade_capacity`].
    pub fn cargo_capacity(&self) -> u32 {
        self.hold_capacity(|_| true)
    }

    /// §holds: the hold space a dock, a market or a route may fill — every
    /// hold but a Combat Freighter's. Cargo is pooled, so loot aboard takes
    /// trade room too.
    pub fn trade_capacity(&self) -> u32 {
        self.hold_capacity(|kind| !kind.is_loot_only())
    }

    fn hold_capacity(&self, counts: impl Fn(ShipKind) -> bool) -> u32 {
        self.composition.iter().filter(|(kind, _)| counts(**kind)).map(|(kind, count)| {
            let extra: u32 = self.loadouts.get(kind).into_iter()
                .flat_map(|fits| fits.iter())
                .map(|(key, n)| n * (crate::module::Loadout::from_key(key).cargo_capacity_mult() - 1))
                .sum();
            self.freight_bonus(*kind, (count + extra) * kind.cargo_units())
        }).sum()
    }

    /// `units` of `kind`'s hold space with the Freight bonus applied.
    fn freight_bonus(&self, kind: ShipKind, units: u32) -> u32 {
        let mult = if kind.is_player_freighter() { self.fuel_research.cargo }
            else if kind.is_loot_only() { self.fuel_research.prize_hold } else { 1.0 };
        if mult > 1.0 {
            (units as f64 * mult + 1e-9).floor() as u32
        } else {
            units
        }
    }

    pub fn has_freighter(&self) -> bool {
        self.composition.iter().any(|(kind, n)| *n > 0 && kind.is_player_freighter())
    }

    /// A trade freighter or a Combat Freighter aboard (§holds).
    pub fn has_freighter_hull(&self) -> bool {
        self.composition.iter().any(|(kind, n)| *n > 0 && kind.is_freighter_hull())
    }

    pub fn freighter_count(&self) -> u32 {
        self.composition.iter().filter(|(kind, _)| kind.is_player_freighter()).map(|(_, n)| n).sum()
    }

    /// Admission check before reserving refit crates. Cargo is pooled, so a
    /// partial refit may remove only as much capacity as the whole manifest frees.
    pub fn cargo_capacity_after_refit(&self, kind: ShipKind, from: &crate::module::Loadout, to: &crate::module::Loadout, n: u32) -> u32 {
        let base = kind.cargo_units() * n;
        self.cargo_capacity().saturating_sub(self.freight_bonus(kind, base * from.cargo_capacity_mult()))
            + self.freight_bonus(kind, base * to.cargo_capacity_mult())
    }

    /// Every non-empty commodity stack aboard, in deterministic commodity
    /// order. The legacy primary stack is folded into the map so old saves and
    /// new mixed manifests have one read surface.
    pub fn cargo_stacks(&self) -> Vec<Cargo> {
        let mut manifest = self.cargo_extra.clone();
        if let Some(cargo) = self.cargo
            && cargo.units > 0
        {
            *manifest.entry(cargo.commodity).or_insert(0) += cargo.units;
        }
        manifest
            .into_iter()
            .filter(|(_, units)| *units > 0)
            .map(|(commodity, units)| Cargo { commodity, units })
            .collect()
    }

    /// Does `stacks` equal [`Self::cargo_stacks`] as it would be built now?
    /// Walks the manifest in the same order without allocating it, so a
    /// recorder can compare against its last report before copying anything.
    pub fn cargo_stacks_matches(&self, stacks: &[Cargo]) -> bool {
        let mut extra = self.cargo.filter(|cargo| cargo.units > 0);
        let mut expected = stacks.iter();
        for (&commodity, &units) in &self.cargo_extra {
            if let Some(cargo) = extra
                && cargo.commodity < commodity
            {
                if expected.next() != Some(&cargo) {
                    return false;
                }
                extra = None;
            }
            let mut total = units;
            if let Some(cargo) = extra
                && cargo.commodity == commodity
            {
                total += cargo.units;
                extra = None;
            }
            if total > 0 && expected.next() != Some(&Cargo { commodity, units: total }) {
                return false;
            }
        }
        if let Some(cargo) = extra
            && expected.next() != Some(&cargo)
        {
            return false;
        }
        expected.next().is_none()
    }

    /// Units aboard across every commodity. §hold-room: a COUNT of goods, not
    /// the room they fill (`Self::hold_used`); capacity checks use room.
    pub fn cargo_units(&self) -> u32 {
        self.cargo.map_or(0, |cargo| cargo.units) + self.cargo_extra.values().copied().sum::<u32>()
    }

    /// §hold-room: the room the goods aboard fill, in quarters.
    pub fn hold_used(&self) -> u64 {
        self.cargo.map_or(0, |c| c.units as u64 * c.commodity.room_quarters() as u64)
            + crate::cargo::manifest_quarters(&self.cargo_extra)
    }

    /// The room left under `capacity` room units: a plunder rig, or a hold
    /// after a refit.
    pub fn room_under(&self, capacity: u32) -> HoldRoom {
        HoldRoom::new(capacity, self.hold_used())
    }

    /// The room left in every hold, loot-only included.
    pub fn cargo_room(&self) -> HoldRoom {
        self.room_under(self.cargo_capacity())
    }

    /// §holds: the room a dock, a market or a route may still fill.
    pub fn trade_room(&self) -> HoldRoom {
        self.room_under(self.trade_capacity())
    }

    /// Do the goods aboard fit in `capacity` room units?
    pub fn cargo_fits(&self, capacity: u32) -> bool {
        self.hold_used() <= capacity as u64 * crate::cargo::ROOM_SCALE as u64
    }

    pub fn cargo_amount(&self, commodity: crate::cargo::Commodity) -> u32 {
        self.cargo
            .filter(|cargo| cargo.commodity == commodity)
            .map_or(0, |cargo| cargo.units)
            + self.cargo_extra.get(&commodity).copied().unwrap_or(0)
    }

    pub fn cargo_is_empty(&self) -> bool {
        self.cargo_units() == 0
    }

    /// Add a whole stack to the shared hold. The caller owns capacity checks;
    /// this function owns representation and merges like commodities.
    pub fn add_cargo(&mut self, commodity: crate::cargo::Commodity, units: u32) {
        if units == 0 {
            return;
        }
        match self.cargo.as_mut() {
            Some(primary) if primary.commodity == commodity => primary.units += units,
            Some(_) => *self.cargo_extra.entry(commodity).or_insert(0) += units,
            None => {
                let extra = self.cargo_extra.remove(&commodity).unwrap_or(0);
                self.cargo = Some(Cargo {
                    commodity,
                    units: units + extra,
                });
            }
        }
    }

    /// Remove up to `units` of one commodity and return the amount removed.
    pub fn remove_cargo(&mut self, commodity: crate::cargo::Commodity, units: u32) -> u32 {
        if units == 0 {
            return 0;
        }
        let mut removed = 0;
        if self.cargo.is_some_and(|cargo| cargo.commodity == commodity) {
            let primary = self.cargo.as_mut().unwrap();
            removed = units.min(primary.units);
            primary.units -= removed;
            if primary.units == 0 {
                self.cargo = None;
            }
        } else if let Some(extra) = self.cargo_extra.get_mut(&commodity) {
            removed = units.min(*extra);
            *extra -= removed;
            if *extra == 0 {
                self.cargo_extra.remove(&commodity);
            }
        }
        self.promote_cargo_primary();
        removed
    }

    /// Empty the whole hold and return its prior manifest.
    pub fn take_cargo(&mut self) -> Vec<Cargo> {
        let manifest = self.cargo_stacks();
        self.cargo = None;
        self.cargo_extra.clear();
        manifest
    }

    /// Replace the whole hold with these stacks (zero entries ignored).
    pub fn set_cargo_stacks(&mut self, stacks: impl IntoIterator<Item = Cargo>) {
        self.cargo = None;
        self.cargo_extra.clear();
        for stack in stacks {
            self.add_cargo(stack.commodity, stack.units);
        }
    }

    fn promote_cargo_primary(&mut self) {
        self.cargo_extra.retain(|_, units| *units > 0);
        if self.cargo.is_none()
            && let Some((&commodity, &units)) = self.cargo_extra.first_key_value()
        {
            self.cargo_extra.remove(&commodity);
            self.cargo = Some(Cargo { commodity, units });
        }
    }

    /// Cargo mass carried by the fleet (§7): each good at its own weight
    /// (`Commodity::mass_per_unit`), so a hold of ore burns twice what a hold
    /// of Alloys does and fuel for a colony rides nearly free.
    pub fn cargo_mass(&self) -> f64 {
        self.cargo.as_ref().map_or(0.0, |c| c.units as f64 * c.commodity.mass_per_unit())
            + self.cargo_extra.iter().map(|(c, n)| *n as f64 * c.mass_per_unit()).sum::<f64>()
    }

    /// Total mass = Σ hull + cargo (§7). Drives fuel-∝-distance×mass exactly as
    /// before; a fleet-of-one convoy with cargo reduces to the old `Ship::mass`.
    pub fn mass(&self) -> f64 {
        self.hull_mass() + self.cargo_mass()
    }

    // --- §reactors-drives: engines, tanks and burn --------------------------

    /// Hulls with engines aboard, by reactor.
    pub fn reactors(&self) -> BTreeMap<crate::fuel::ReactorGrade, u32> {
        let mut out = BTreeMap::new();
        for s in self.ships.iter().filter(|s| s.kind.engines() > 0) {
            *out.entry(s.reactor).or_insert(0) += 1;
        }
        out
    }

    /// Hulls with engines aboard, by drive.
    pub fn drives(&self) -> BTreeMap<crate::fuel::Drive, u32> {
        let mut out = BTreeMap::new();
        for s in self.ships.iter().filter(|s| s.kind.engines() > 0) {
            *out.entry(s.drive).or_insert(0) += 1;
        }
        out
    }

    /// Hulls with engines aboard, by (reactor, drive).
    pub fn engines_aboard(&self) -> BTreeMap<crate::fuel::Engine, u32> {
        let mut out = BTreeMap::new();
        for s in self.ships.iter().filter(|s| s.kind.engines() > 0) {
            *out.entry(s.engine()).or_insert(0) += 1;
        }
        out
    }

    /// The slowest drive aboard: the formation flies on it. None with no
    /// engines aboard.
    pub fn slowest_drive(&self) -> Option<crate::fuel::Drive> {
        self.ships.iter().filter(|s| s.kind.engines() > 0).map(|s| s.drive).min()
    }

    /// §reactors-drives: the slowest drive's multiplier on rated speed (1 with
    /// no engines aboard).
    pub fn drive_speed_mult(&self) -> f64 {
        self.slowest_drive().map_or(1.0, |d| d.speed_mult())
    }

    /// §reactors-drives: the slowest drive's multiplier on ENERGY a distance —
    /// every hull flies at its speed, so every hull pays its square.
    pub fn drive_energy_mult(&self) -> f64 {
        self.slowest_drive().map_or(1.0, |d| d.energy_mult())
    }

    /// The fuels the hulls aboard burn, in commodity order.
    pub fn fuels_aboard(&self) -> Vec<Commodity> {
        let mut v: Vec<Commodity> = self.ships.iter().filter(|s| s.kind.engines() > 0).map(|s| s.reactor.fuel()).collect();
        v.sort();
        v.dedup();
        v
    }

    /// §shipbuilding-overhaul Phase 5: every store a dock fills — the fuels
    /// the hulls burn and the munitions the guns aboard can hold.
    pub fn supplies_aboard(&self) -> Vec<Commodity> {
        let mut v = self.fuels_aboard();
        v.extend(crate::munitions::MUNITIONS.into_iter().filter(|c| self.magazine_capacity(*c) > 1e-9));
        v
    }

    /// Beam guns, Torpedo Racks and Mass Drivers aboard.
    pub fn beam_guns(&self) -> f64 { self.ships.iter().map(Ship::beam_guns).sum() }
    pub fn racks(&self) -> f64 { self.ships.iter().map(Ship::racks).sum() }
    pub fn drivers(&self) -> f64 { self.ships.iter().map(Ship::drivers).sum() }

    /// The MAGAZINE for a munition: rounds per rack or driver (Hardened
    /// Magazines widens them), one crystal per beam gun.
    pub fn magazine_capacity(&self, c: Commodity) -> f64 {
        use crate::munitions::{CRYSTALS_PER_GUN, SLUGS_PER_DRIVER, TORPEDOES_PER_RACK};
        match c {
            Commodity::Torpedoes => self.racks() * TORPEDOES_PER_RACK * self.fuel_research.magazine,
            Commodity::Slugs => self.drivers() * SLUGS_PER_DRIVER * self.fuel_research.magazine,
            Commodity::BeamCrystals => self.beam_guns() * CRYSTALS_PER_GUN,
            _ => 0.0,
        }
    }

    /// Anything aboard that takes a munition.
    pub fn has_magazines(&self) -> bool {
        crate::munitions::MUNITIONS.iter().any(|c| self.magazine_capacity(*c) > 1e-9)
    }

    /// The magazines as the battle engine carries them.
    pub fn magazine(&self) -> crate::munitions::Magazine {
        crate::munitions::Magazine {
            torpedoes: self.torpedoes,
            slugs: self.slugs,
            crystals: self.crystals,
            crystal_capacity: self.magazine_capacity(Commodity::BeamCrystals),
        }
    }

    /// Take the engine's magazines back after a round. A rack that died took
    /// its rounds with it (clamped to what is left); crystals keep their wear
    /// across the guns that remain.
    pub fn apply_magazine(&mut self, m: &crate::munitions::Magazine) {
        self.torpedoes = m.torpedoes.min(self.magazine_capacity(Commodity::Torpedoes));
        self.slugs = m.slugs.min(self.magazine_capacity(Commodity::Slugs));
        let cap = self.magazine_capacity(Commodity::BeamCrystals);
        // Same guns as when the engine took them: the exact value back (so a
        // quiet round records no new input); fewer guns: the same wear across
        // the ones that remain.
        self.crystals = if (cap - m.crystal_capacity).abs() < 1e-9 || m.crystal_capacity <= 1e-9 { m.crystals.min(cap) }
            else { (m.crystals / m.crystal_capacity).clamp(0.0, 1.0) * cap };
    }

    /// The fuel of the heaviest hull mass aboard: the one a one-fuel verb
    /// (a rescue, a raid's bunker) reaches for.
    pub fn primary_fuel(&self) -> Option<Commodity> {
        let mut by_fuel: BTreeMap<Commodity, f64> = BTreeMap::new();
        for s in self.ships.iter().filter(|s| s.kind.engines() > 0) {
            *by_fuel.entry(s.reactor.fuel()).or_insert(0.0) += s.kind.hull_mass();
        }
        by_fuel.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|(f, _)| f)
    }

    /// The TANK for `fuel`: what the hulls burning it can hold, in units.
    /// Proportional to hull mass, because the per-tick burn is too — which makes
    /// RANGE roughly independent of fleet size and, since every grade holds the
    /// same ENERGY per unit of mass, of the core grade. Cargo is excluded: a
    /// hold full of ore is not a fuel bunker. Bunkerage multiplies every tank.
    pub fn tank_capacity(&self, fuel: Commodity) -> f64 {
        if fuel.is_munition() {
            return self.magazine_capacity(fuel);
        }
        self.ships.iter().filter(|s| s.reactor.fuel() == fuel).map(Ship::tank).sum::<f64>()
            * self.fuel_research.capacity
    }

    /// Units of `fuel` aboard (a munition: its magazine).
    pub fn store(&self, fuel: Commodity) -> f64 {
        match fuel {
            Commodity::Uranium => self.uranium,
            Commodity::Deuterium => self.fuel,
            Commodity::Helium3 => self.helium3,
            Commodity::Antimatter => self.antimatter,
            Commodity::Torpedoes => self.torpedoes,
            Commodity::Slugs => self.slugs,
            Commodity::BeamCrystals => self.crystals,
            _ => 0.0,
        }
    }

    pub fn store_mut(&mut self, fuel: Commodity) -> &mut f64 {
        match fuel {
            Commodity::Uranium => &mut self.uranium,
            Commodity::Deuterium => &mut self.fuel,
            Commodity::Helium3 => &mut self.helium3,
            Commodity::Antimatter => &mut self.antimatter,
            Commodity::Torpedoes => &mut self.torpedoes,
            Commodity::Slugs => &mut self.slugs,
            Commodity::BeamCrystals => &mut self.crystals,
            other => panic!("{other:?} is not a fuel or a munition"),
        }
    }

    /// Room left in the tank for `fuel`.
    pub fn room(&self, fuel: Commodity) -> f64 {
        (self.tank_capacity(fuel) - self.store(fuel)).max(0.0)
    }

    /// Fill the tank (or magazine) for `fuel` from a supply, returning what
    /// was taken.
    pub fn take_fuel(&mut self, fuel: Commodity, available: f64) -> f64 {
        if !fuel.is_supply() {
            return 0.0;
        }
        let taken = self.room(fuel).min(available.max(0.0));
        *self.store_mut(fuel) += taken;
        taken
    }

    /// The Deuterium tank (the Fusion Cores' fuel). Kept under its old name:
    /// the wire and the client call the Deuterium tank `fuel`.
    pub fn fuel_capacity(&self) -> f64 { self.tank_capacity(Commodity::Deuterium) }
    /// The Uranium tank (the Fission Cores' fuel).
    pub fn uranium_capacity(&self) -> f64 { self.tank_capacity(Commodity::Uranium) }
    /// The Helium-3 tank (the Helium Cores' fuel).
    pub fn helium3_capacity(&self) -> f64 { self.tank_capacity(Commodity::Helium3) }
    /// The Antimatter tank (the Fold Cores' fuel and jump bottle).
    pub fn antimatter_capacity(&self) -> f64 { self.tank_capacity(Commodity::Antimatter) }

    /// §fuel-overhaul: empty every tank and magazine (a hull fresh off the
    /// slip; its crystals come with its guns through `add_fitted_on`).
    pub fn launch_empty(&mut self) {
        self.fuel = 0.0;
        self.helium3 = 0.0;
        self.uranium = 0.0;
        self.antimatter = 0.0;
        self.torpedoes = 0.0;
        self.slugs = 0.0;
        self.crystals = 0.0;
    }

    /// Free room in the Deuterium tank.
    pub fn fusion_room(&self) -> f64 { self.room(Commodity::Deuterium) }

    /// Fill the Deuterium tank, returning what was taken.
    pub fn refuel(&mut self, available: f64) -> f64 { self.take_fuel(Commodity::Deuterium, available) }
    /// Fill the Helium-3 tank, returning the units taken.
    pub fn refuel_helium3(&mut self, available: f64) -> f64 { self.take_fuel(Commodity::Helium3, available) }
    /// Fill the Uranium tank, returning what was taken.
    pub fn refuel_uranium(&mut self, available: f64) -> f64 { self.take_fuel(Commodity::Uranium, available) }
    /// Fill the Antimatter tank, returning what was taken.
    pub fn refuel_antimatter(&mut self, available: f64) -> f64 { self.take_fuel(Commodity::Antimatter, available) }

    /// Deuterium units aboard.
    pub fn deuterium(&self) -> f64 { self.fuel }

    /// ENERGY aboard (in Uranium) across every tank, and the tanks' total.
    pub fn energy_aboard(&self) -> f64 {
        crate::fuel::ReactorGrade::ALL.iter().map(|g| self.store(g.fuel()) * g.energy_per_unit()).sum()
    }

    pub fn energy_capacity(&self) -> f64 {
        crate::fuel::ReactorGrade::ALL.iter().map(|g| self.tank_capacity(g.fuel()) * g.energy_per_unit()).sum()
    }

    /// Flying Slow this tick (half speed, a quarter of the energy per distance).
    pub fn flies_slow(&self) -> bool {
        self.transit == TransitMode::Slow && self.can_slow()
    }

    /// Any formation can throttle back to Slow.
    pub fn can_slow(&self) -> bool {
        self.ships.iter().any(|s| s.kind.engines() > 0)
    }

    /// Stealth needs every engine-bearing hull on a Gale Drive or better.
    pub fn can_stealth(&self) -> bool {
        self.can_slow() && self.ships.iter().all(|s| s.kind.engines() == 0 || s.drive.can_stealth())
    }

    /// The throttle the fleet actually flies: Stealth falls back to Full on a
    /// fission hull.
    pub fn effective_transit(&self) -> TransitMode {
        match self.transit {
            TransitMode::Stealth if !self.can_stealth() => TransitMode::Full,
            TransitMode::Slow if !self.can_slow() => TransitMode::Full,
            mode => mode,
        }
    }

    /// The throttle's multiplier on ENERGY per distance.
    pub fn cruise_fuel_mult(&self) -> f64 {
        self.cruise_mult_for(self.flies_slow())
    }

    /// The multiplier on energy per distance at a throttle, weighed hull by hull
    /// by mass, since burn is proportional to mass: on Slow a quarter for every
    /// hull; on Full, Fusion Freight's cut for each player freighter and the
    /// cargo it carries, so an escort riding along costs its own share and no
    /// more. Torch Regime makes warship hulls thirstier at either throttle.
    pub fn cruise_mult_for(&self, slow: bool) -> f64 {
        let total = self.mass();
        let drive = self.drive_energy_mult();
        if total <= 1e-9 {
            return drive * if slow { crate::fuel::SLOW_FUEL_MULT } else { 1.0 };
        }
        let hulls: f64 = self.composition.iter().filter(|(_, n)| **n > 0)
            .map(|(kind, n)| kind.hull_mass() * *n as f64 * self.hull_regime(*kind, slow)).sum();
        drive * (hulls + self.cargo_mass() * self.cargo_regime(slow)) / total
    }

    /// One hull's energy multiplier at a throttle (see `cruise_mult_for`).
    fn hull_regime(&self, kind: ShipKind, slow: bool) -> f64 {
        let torch = if kind.is_combatant() { self.fuel_research.warship_burn * self.fuel_research.warship_fuel } else { 1.0 };
        let regime = if slow { crate::fuel::SLOW_FUEL_MULT }
            else if kind.is_player_freighter() { self.fuel_research.freighter_fusion } else { 1.0 };
        regime * torch
    }

    /// The cargo's energy multiplier: Fusion Freight's cut rides a freighter's hold.
    fn cargo_regime(&self, slow: bool) -> f64 {
        if slow { crate::fuel::SLOW_FUEL_MULT }
        else if self.has_freighter() { self.fuel_research.freighter_fusion } else { 1.0 }
    }

    /// Research and the selected throttle compose once, both in the physical
    /// burn and in range/route estimates. Captain bonuses are separate.
    pub fn propulsion_fuel_mult(&self) -> f64 {
        self.cruise_fuel_mult() * self.fuel_research.consumption
    }

    /// §fuel-overhaul: what an escort burns while flying formation on a charge:
    /// the charge's throttle. Returns (slow, energy-per-distance multiplier).
    pub fn formation_burn(&self, charge_slow: bool) -> (bool, f64) {
        let slow = charge_slow && self.can_slow();
        (slow, self.cruise_mult_for(slow) * self.fuel_research.consumption)
    }

    /// UNITS of each fuel aboard that `base` energy (a `fuel_tick` of this
    /// fleet's mass and speed, before throttle and research) costs at a
    /// throttle. Each hull pays its mass share of the energy — regime, Torch
    /// Regime and research consumption included — from its own core's fuel,
    /// and the cargo's share rides every hull by mass.
    pub fn burn_units(&self, base: f64, slow: bool) -> Vec<(Commodity, f64)> {
        let total = self.mass();
        if total <= 1e-9 || base <= 0.0 {
            return Vec::new();
        }
        let hull_total = self.hull_mass().max(1e-9);
        let cargo = self.cargo_mass() * self.cargo_regime(slow);
        // §reactors-drives: the slowest drive sets the pace and its square the
        // energy, for every hull.
        let drive = self.drive_energy_mult();
        let mut units: BTreeMap<Commodity, f64> = BTreeMap::new();
        for s in self.ships.iter().filter(|s| s.kind.engines() > 0) {
            let mass = s.kind.hull_mass();
            let weight = mass * self.hull_regime(s.kind, slow) + cargo * mass / hull_total;
            let energy = base * self.fuel_research.consumption * drive * weight / total;
            *units.entry(s.reactor.fuel()).or_insert(0.0) += energy / s.reactor.energy_per_unit();
        }
        units.into_iter().collect()
    }

    /// Can every tank cover its share of `units`?
    pub fn can_pay(&self, units: &[(Commodity, f64)]) -> bool {
        units.iter().all(|(fuel, u)| self.store(*fuel) + 1e-9 >= *u)
    }

    /// Draw `units` from the tanks (never below zero).
    pub fn pay(&mut self, units: &[(Commodity, f64)]) {
        for (fuel, u) in units {
            let store = self.store_mut(*fuel);
            *store = (*store - u).max(0.0);
        }
    }

    /// Burn `energy` (in Uranium) spread over the hulls by mass, each from its
    /// own fuel: the interdiction field's charge, a test's drain.
    pub fn burn_energy(&mut self, energy: f64) {
        let hull_total = self.hull_mass();
        if hull_total <= 1e-9 || energy <= 0.0 {
            return;
        }
        let shares: Vec<(Commodity, f64)> = self.ships.iter().filter(|s| s.kind.engines() > 0)
            .map(|s| (s.reactor.fuel(), energy * s.kind.hull_mass() / hull_total / s.reactor.energy_per_unit()))
            .collect();
        self.pay(&shares);
    }

    /// The fuel the fleet needs most: the tank with the least range at the
    /// current throttle, and its room. None when nothing aboard burns fuel.
    pub fn thirstiest(&self) -> Option<(Commodity, f64)> {
        let base = crate::fuel::fuel_tick(self.mass(), self.transit_speed(), 1.0);
        self.burn_units(base.max(1e-9), self.flies_slow()).into_iter()
            .map(|(fuel, per_s)| (fuel, if per_s > 1e-12 { self.store(fuel) / per_s } else { f64::INFINITY }))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(fuel, _)| (fuel, self.room(fuel)))
    }

    /// How far this fleet can still go at `factor` (1 on thrusters,
    /// `WARP_FACTOR` in warp): the shortest tank's range at the current
    /// throttle. Distance, not time — what a range ring draws.
    pub fn range_at(&self, factor: f64) -> f64 {
        let speed = self.transit_speed();
        let base = crate::fuel::fuel_tick(self.mass(), speed, 1.0);
        let units = self.burn_units(base, self.flies_slow());
        if units.is_empty() {
            return f64::INFINITY;
        }
        let seconds = units.iter()
            .map(|(fuel, per_s)| if *per_s <= 1e-12 { f64::INFINITY } else { self.store(*fuel) / per_s })
            .fold(f64::INFINITY, f64::min);
        seconds * speed * factor
    }

    /// §fuel-overhaul: Antimatter a jump of `distance` burns from the Fold tank.
    pub fn jump_antimatter_cost(&self, distance: f64) -> f64 {
        crate::fuel::fuel_cost(distance, self.mass()) / crate::transit::JUMP_FUEL_FACTOR
            * self.fuel_research.consumption
    }

    /// Formation speed × throttle. Slow's fuel saving is separate: simply
    /// halving speed and burn/second would leave total trip fuel unchanged.
    pub fn transit_speed(&self) -> f64 {
        self.max_speed() * self.effective_transit().fraction()
    }

    /// §sensors: what the core is doing right now. Under way is read off the
    /// motion itself (at its transit mode's throttle), as the View reads it; in
    /// place, machinery runs while mining, prospecting, surveying, building or
    /// tearing down, and always at an anchored belt depot; otherwise it holds.
    pub fn core_state(&self) -> crate::detection::CoreState {
        use crate::detection::CoreState;
        if self.vel.length() > crate::detection::UNDER_WAY_SU_S {
            return CoreState::UnderWay(self.effective_transit());
        }
        let building = matches!(self.order,
            FleetOrder::Construct { started: Some(_), .. } | FleetOrder::Demolish { started: Some(_), .. });
        if self.loud_work() || building || self.contains(ShipKind::MiningBeltStation) {
            CoreState::Working
        } else {
            CoreState::Holding
        }
    }

    /// §sensors: the core's output — mass × rated speed × state — in Raiders
    /// at full throttle. Recorded with every served sample, so the View judges
    /// it in the retarded frame exactly as the sim's own sensors do.
    pub fn core_output(&self) -> f64 {
        // §reactors-drives: the drive's speed bonus is not louder — sensor
        // ranges stay tuned in Raiders-at-rated-speed, whatever the drive.
        crate::detection::core_output(self.mass(), self.max_speed() / self.drive_speed_mult(), self.core_state())
    }

    /// The fleet's detection SIGNATURE (§sensors) at its CURRENT state: √ of
    /// its core output. Dark fleets are found by it; a broadcaster is also
    /// heard by its transponder.
    pub fn signature(&self) -> f64 {
        crate::detection::signature(self.core_output())
    }

    /// The signature every sensor reads: an active survey sweep adds its ×1.5
    /// (`explore::SURVEY_SIGNATURE_FACTOR`) on top of the work it costs the core.
    pub fn detection_signature(&self) -> f64 {
        self.signature() * if self.surveying() { crate::explore::SURVEY_SIGNATURE_FACTOR } else { 1.0 }
    }

    /// FORMATION speed (GDD §14.2): the SLOWEST member sets the pace — the
    /// minimum constant `speed(kind)` among present kinds. For a fleet-of-one this
    /// is that ship's own speed; a hammer carrying a colony ship crawls at the
    /// colony's pace, telegraphing itself by physics. Cargo does NOT slow a fleet
    /// (constant-speed model, §14.1) — it costs FUEL (mass), not time.
    pub fn max_speed(&self) -> f64 {
        let hull_speed = self
            .composition
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(kind, count)| {
                // Drive Tuning speeds every hull; Military Drives and Torch
                // Regime speed warships; Express Charters freighters; a Mass
                // Stream the whole formation.
                let base = kind.max_speed() * self.fuel_research.speed * self.fuel_research.stream
                    * if kind.is_combatant() { self.fuel_research.warship_speed } else { 1.0 }
                    * if kind.is_player_freighter() { self.fuel_research.freighter_speed } else { 1.0 };
                let fits = self.loadouts.get(kind);
                let fitted = fits.map(|f| f.values().sum::<u32>()).unwrap_or(0);
                let unfitted = if fitted < *count { base } else { f64::INFINITY };
                fits.into_iter().flat_map(|f| f.iter()).filter(|(_, n)| **n > 0)
                    .map(|(key, _)| base * crate::module::Loadout::from_key(key).speed_mult())
                    .fold(unfitted, f64::min)
            })
            .fold(f64::INFINITY, f64::min)
            // §reactors-drives: the slowest drive aboard sets the pace.
            * self.drive_speed_mult();
        if self.founding_privateer {
            hull_speed * crate::founding::PRIVATEER_SPEED_MULT
        } else if self.operation_privateer {
            hull_speed * crate::operation::FOLLOW_UP_PIRATE_SPEED_MULT
        } else {
            hull_speed
        }
    }

    /// Total offensive weight = Σ attack_weight(kind) × count.
    pub fn attack_power(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.attack_weight() * *n as f64)
            .sum()
    }

    /// Total defensive weight = Σ defense_weight(kind) × count.
    pub fn defense_power(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.defense_weight() * *n as f64)
            .sum()
    }

    /// Total combat weight (force-ratio presence) = Σ combat_weight(kind) × count.
    /// Non-combatant kinds contribute their (small) defense weight only, exactly
    /// as head-count comparisons did for the N=1 world.
    pub fn combat_weight(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.combat_weight() * *n as f64)
            .sum()
    }

    /// Whether the fleet carries any teeth (a combatant kind) for doctrine
    /// force-ratio assessments.
    pub fn is_combatant(&self) -> bool {
        self.composition.keys().any(|k| k.is_combatant())
    }

    /// Advance this fleet one timestep at simulation time `time`. Moves at the
    /// FORMATION constant speed (slowest member sets the pace), scaled by the
    /// transit throttle (Full/Stealth/Slow).
    pub fn advance(&mut self, time: f64, dt: f64, env: &crate::transit::TransitEnv<'_>) {
        let heading = if self.vel.length_sq() > 1e-9 {
            self.vel
        } else {
            Vec2::ZERO
        };
        let under_way = !matches!(self.order, FleetOrder::Idle | FleetOrder::Jump { .. });
        let drive = self.warp || (under_way && !env.in_well(self.pos));
        // §course-change: DRIVE THE STATE MACHINE, then read speed off it.
        //
        // A fleet cannot steer above thrusters. To change course it drops all
        // the way out, turns, and re-enters — and each transition takes time.
        // That is what makes a reversal cost something, and it replaces the old
        // bounded-turn-rate model, which decided the same question by geometry
        // and lost: threshold crossings changed both speed and turn geometry.
        let want = if drive && !env.in_well(self.pos) {
            crate::transit::Regime::Warp
        } else {
            crate::transit::Regime::Thrusters
        };
        // How far the course it WANTS is from the course it is ON.
        let aim = match self.order {
            FleetOrder::MoveTo { dest } => dest - self.pos,
            FleetOrder::DefendSystem { assignment } => assignment.station - self.pos,
            FleetOrder::Jump { .. } => Vec2::ZERO,
            FleetOrder::Construct { site, .. } | FleetOrder::Demolish { site, .. } => {
                site - self.pos
            }
            FleetOrder::Blockade { station, .. } | FleetOrder::Survey { station, .. }
            | FleetOrder::Expedition { station, .. } => {
                station - self.pos
            }
            FleetOrder::Patrol {
                ref waypoints,
                index,
                ..
            } if !waypoints.is_empty() => waypoints[index % waypoints.len()] - self.pos,
            _ => Vec2::ZERO,
        };
        let off_course = if heading.length_sq() > 1e-9 && aim.length_sq() > 1e-9 {
            heading
                .normalized()
                .dot(aim.normalized())
                .clamp(-1.0, 1.0)
                .acos()
        } else {
            0.0
        };
        self.drive_state = match self.drive_state {
            // At rest on thrusters: start spinning up if there is anything to
            // spin up into.
            DriveState::Thrusters if want != crate::transit::Regime::Thrusters => {
                DriveState::Spooling {
                    to: want,
                    left: crate::transit::spool_seconds(want),
                }
            }
            DriveState::Thrusters => DriveState::Thrusters,
            // Mid spin-up: finish, or abandon it if the fleet no longer wants it.
            // Changed its mind mid spin-up: start the new one from scratch, or
            // fall back to thrusters if it no longer wants a drive at all.
            DriveState::Spooling { to, .. } if to != want => {
                if want == crate::transit::Regime::Thrusters {
                    DriveState::Thrusters
                } else {
                    DriveState::Spooling {
                        to: want,
                        left: crate::transit::spool_seconds(want),
                    }
                }
            }
            DriveState::Spooling { to, left } => {
                let left = left - dt;
                if left <= 0.0 {
                    DriveState::Cruising(to)
                } else {
                    DriveState::Spooling { to, left }
                }
            }
            // Cruising: any change of regime means SHUTTING DOWN first.
            DriveState::Cruising(r) if r != want => DriveState::Dropping {
                from: r,
                left: crate::transit::drop_seconds(r),
            },
            // ...and so does any change of COURSE. In warp nothing is steering,
            // so wanting to go somewhere other than where you are pointed is not
            // something you can act on — you drop out, come about on thrusters,
            // and light it again.
            DriveState::Cruising(crate::transit::Regime::Warp)
                if off_course > crate::transit::COURSE_LOCK_RAD =>
            {
                DriveState::Dropping {
                    from: crate::transit::Regime::Warp,
                    left: crate::transit::drop_seconds(crate::transit::Regime::Warp),
                }
            }
            DriveState::Cruising(r) => DriveState::Cruising(r),
            DriveState::Dropping { from, left } => {
                let left = left - dt;
                if left <= 0.0 {
                    DriveState::Thrusters
                } else {
                    DriveState::Dropping { from, left }
                }
            }
        };
        self.regime = self.drive_state.regime();
        let factor = match self.regime {
            crate::transit::Regime::Warp => crate::transit::WARP_FACTOR,
            crate::transit::Regime::Thrusters => 1.0,
        };
        let speed = self.transit_speed() * factor;
        // WHO IS STEERING. On thrusters, the fleet. In warp the course is locked,
        // so the fleet flies the line it committed to when it lit the drive.
        let steered = self.drive_state.can_steer();
        let radius = if steered { 0.0 } else { f64::INFINITY };

        match &mut self.order {
            FleetOrder::Idle => {
                // Holds station. (Already at rest.)
                self.vel = Vec2::ZERO;
            }
            FleetOrder::Jump { .. } => {
                self.vel = Vec2::ZERO;
            }
            FleetOrder::MoveTo { dest } => {
                let step =
                    crate::movement::advance_turning(self.pos, self.vel, *dest, speed, dt, radius);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    self.order = FleetOrder::Idle;
                }
            }
            FleetOrder::Construct { site, .. } | FleetOrder::Demolish { site, .. } => {
                // Fly to the worksite and HOLD there — the construction /
                // demolition resolver runs the clock; going Idle would abandon
                // the job.
                let site = *site;
                let step =
                    crate::movement::advance_turning(self.pos, self.vel, site, speed, dt, radius);
                self.pos = step.pos;
                self.vel = step.vel;
            }
            FleetOrder::Blockade { station, .. }
            | FleetOrder::DefendSystem { assignment: SystemDefense { station, .. } } => {
                // Fly to station, then HOLD there (keep the Blockade order — the
                // world reads on-station presence as an active blockade; going
                // Idle would drop it). Once arrived, advance_toward returns the
                // station point at zero velocity, so it simply holds each tick.
                let step = crate::movement::advance_turning(
                    self.pos, self.vel, *station, speed, dt, radius,
                );
                self.pos = step.pos;
                self.vel = step.vel;
            }
            FleetOrder::Survey { station, .. } | FleetOrder::Expedition { station, .. } => {
                // §explore: fly to the star and HOLD (the world's survey resolver
                // runs the dwell clock + completion; going Idle would drop it).
                let step = crate::movement::advance_turning(
                    self.pos, self.vel, *station, speed, dt, radius,
                );
                self.pos = step.pos;
                self.vel = step.vel;
            }
            FleetOrder::Patrol {
                waypoints,
                index,
                dwell_until,
            } => {
                if waypoints.is_empty() {
                    self.vel = Vec2::ZERO;
                    return;
                }
                if time < *dwell_until {
                    self.vel = Vec2::ZERO;
                    return;
                }
                let dest = waypoints[*index % waypoints.len()];
                let step = advance_toward(self.pos, dest, speed, dt);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    *dwell_until = time + PATROL_DWELL;
                    *index = (*index + 1) % waypoints.len();
                }
            }
            // §pursuit: the WORLD sets the chase leg (it needs the target's
            // state to aim), but the FLIGHT is ours — through the same drive
            // machinery as any other order, so a chase lights warp exactly like
            // its prey. Contact is the world's call (resolve_raids), so arrival
            // never ends the order: retire the leg and hold for the next aim.
            FleetOrder::Intercept { .. } | FleetOrder::Attack { .. } | FleetOrder::Guard { .. } | FleetOrder::Refuel { .. } => {
                // The world owns true target state and performs this step using
                // the same drive machinery with a temporary fixed aim.
                self.vel = Vec2::ZERO;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{Cargo, Commodity};
    use crate::ids::{EntityId, PlayerId};
    use crate::math::Vec2;

    fn fleet(comp: &[(ShipKind, u32)], cargo: Option<Cargo>) -> Fleet {
        let mut f = Fleet::single(
            EntityId(1),
            PlayerId(1),
            ShipKind::Scout,
            Vec2::ZERO,
            FleetOrder::Idle,
            cargo,
        );
        // §roster: drop the placeholder hull `single` seeded, then crew for real.
        f.ships.clear();
        f.next_ship_id = 0;
        f.rebuild_cache();
        for (k, n) in comp {
            f.add(*k, *n);
        }
        f
    }

    #[test]
    fn every_hull_feeds_its_people_at_the_colony_ration_rate() {
        for kind in ALL_SHIP_KINDS {
            let f = fleet(&[(kind, 1)], None);
            // §charting: the Survey Probe is the one crewless hull — it eats nothing.
            assert!(crew_size(kind) > 0 || kind == ShipKind::SurveyProbe, "{kind:?} has an operating crew");
            assert_eq!(f.crew_count(), crew_size(kind));
            let expected = f.provisions_headcount() as f64 / 1_000_000.0
                * crate::colony::PROVISIONS_PER_MILLION_PER_S;
            assert!((f.upkeep_per_sec() - expected).abs() < 1e-12, "{kind:?}");
            assert!((upkeep_per_sec(kind) - expected).abs() < 1e-12, "{kind:?} quote matches draw");
        }
        for (kind, crew) in PLAYER_FREIGHTERS.into_iter().zip([5, 10, 20, 30, 50, 80]) {
            assert_eq!(crew_size(kind), crew);
        }
        assert_eq!(crew_size(ShipKind::Raider), 12);
    }

    #[test]
    fn starter_fleet_food_depends_on_crew_not_cargo_speed_or_hull_damage() {
        let mut f = fleet(&[(ShipKind::TinyFreighter, 1), (ShipKind::Raider, 1)], None);
        assert_eq!(f.crew_count(), 17);
        let rate = f.upkeep_per_sec();
        // §progression: a person eats at the output scale.
        assert!((rate * 25.0 * 60.0 - 1.02 * crate::production::OUTPUT_SCALE).abs() < 1e-12,
            "a 25-minute escort circuit feeds 17 people, not 1,750");
        f.cargo = Some(Cargo { commodity: Commodity::MetallicOre, units: 50 });
        f.transit = TransitMode::Stealth;
        f.ships[0].hp *= 0.5;
        assert_eq!(f.upkeep_per_sec(), rate,
            "cargo, throttle and hull damage do not change crew headcount");
        let mut freight = fleet(&[(ShipKind::TinyFreighter, 1)], None);
        let normal = freight.upkeep_per_sec();
        freight.transit = TransitMode::Slow;
        assert_eq!(freight.upkeep_per_sec(), normal);
    }

    #[test]
    fn crew_follows_roster_splits_merges_losses_and_saves() {
        let mut f = fleet(&[(ShipKind::TinyFreighter, 2), (ShipKind::Raider, 3)], None);
        let people = f.crew_count();
        let food = f.upkeep_per_sec();
        let detached = f.detach_ships(ShipKind::Raider, 2);
        let mut wing = fleet(&[], None);
        wing.absorb_ships(detached);
        assert_eq!(f.crew_count() + wing.crew_count(), people);
        assert!((f.upkeep_per_sec() + wing.upkeep_per_sec() - food).abs() < 1e-12);
        f.absorb_ships(wing.ships);
        assert_eq!(f.crew_count(), people);
        let restored: Fleet = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(restored.crew_count(), people, "no new save fields are required");
        assert_eq!(restored.upkeep_per_sec(), food);
        f.remove_one(ShipKind::Raider);
        assert_eq!(f.crew_count(), people - crew_size(ShipKind::Raider));
    }

    #[test]
    fn embarked_people_eat_and_disembarked_people_stop_drawing_ship_rations() {
        let mut f = fleet(&[(ShipKind::Colony, 1), (ShipKind::Transport, 1)], None);
        f.passengers.insert(crate::specialist::SpecialistKind::NavalArchitect, 2);
        assert_eq!(f.crew_count(), 90);
        assert_eq!(f.provisions_headcount(), 90 + 1_000 + 40 + 2);
        f.commit_marines();
        assert_eq!(f.provisions_headcount(), 90 + 1_000 + 2);
        f.passengers.clear();
        assert_eq!(f.provisions_headcount(), 90 + 1_000);
        f.remove_one(ShipKind::Colony);
        assert_eq!(f.provisions_headcount(), 40);
        f.reembark_marines();
        assert_eq!(f.provisions_headcount(), 40 + 40);
    }

    #[test]
    fn fleet_of_one_matches_the_old_single_ship_exactly() {
        // A convoy fleet-of-one moves at the convoy's constant speed (§14.1) —
        // cargo affects fuel (mass), not speed.
        let cargo = Some(Cargo {
            commodity: Commodity::MetallicOre,
            units: 100,
        });
        let f = fleet(&[(ShipKind::Convoy, 1)], cargo);
        assert_eq!(
            f.max_speed(),
            ShipKind::Convoy.max_speed(),
            "fleet-of-one speed == its kind's speed"
        );
        assert_eq!(f.flagship_kind(), ShipKind::Convoy);
        assert_eq!(f.total_count(), 1);
    }

    #[test]
    fn formation_speed_is_set_by_the_slowest_member() {
        // A hammer (raider) carrying a colony ship lumbers at the COLONY's pace.
        let f = fleet(&[(ShipKind::Raider, 3), (ShipKind::Colony, 1)], None);
        assert_eq!(
            f.max_speed(),
            ShipKind::Colony.max_speed(),
            "slowest member sets the formation speed"
        );
        // Raider alone is far faster — proving the formation penalty.
        let raider = fleet(&[(ShipKind::Raider, 1)], None);
        assert!(
            raider.max_speed() > f.max_speed(),
            "an unencumbered raider is faster"
        );
    }

    #[test]
    fn mass_and_fuel_sum_over_the_whole_convoy_count() {
        let cargo = Some(Cargo {
            commodity: Commodity::MetallicOre,
            units: 50,
        });
        let f = fleet(&[(ShipKind::Convoy, 3)], cargo);
        let expected = 3.0 * ShipKind::Convoy.hull_mass() + 50.0 * CARGO_MASS_PER_UNIT;
        assert!(
            (f.mass() - expected).abs() < 1e-9,
            "mass = Σ hull×count + cargo"
        );
        // Fuel ∝ distance × total mass, so a 3-convoy fleet burns 3× a 1-convoy
        // fleet's hull share over the same leg (cargo held equal).
        let one = fleet(&[(ShipKind::Convoy, 1)], None);
        let three = fleet(&[(ShipKind::Convoy, 3)], None);
        let d = 1000.0;
        assert!(
            (crate::fuel::fuel_cost(d, three.mass()) - 3.0 * crate::fuel::fuel_cost(d, one.mass()))
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn a_hold_of_ore_outweighs_a_hold_of_alloys_or_fuel() {
        let hold = |commodity| Some(Cargo { commodity, units: 50 });
        let ore = fleet(&[(ShipKind::Convoy, 1)], hold(Commodity::MetallicOre));
        let alloys = fleet(&[(ShipKind::Convoy, 1)], hold(Commodity::Alloys));
        let mut mixed = fleet(&[(ShipKind::Convoy, 1)], hold(Commodity::Uranium));
        mixed.cargo_extra.insert(Commodity::Alloys, 50);
        assert!((ore.cargo_mass() - 50.0 * crate::cargo::MASS_RAW).abs() < 1e-9);
        assert!((alloys.cargo_mass() - 50.0 * crate::cargo::MASS_BULK).abs() < 1e-9);
        assert!((mixed.cargo_mass() - 50.0 * (crate::cargo::MASS_SUPPLY + crate::cargo::MASS_BULK)).abs() < 1e-9);
        assert!((mixed.mass() - ShipKind::Convoy.hull_mass() - mixed.cargo_mass()).abs() < 1e-9);
        let d = 10_000.0;
        assert!(crate::fuel::fuel_cost(d, ore.mass()) > crate::fuel::fuel_cost(d, alloys.mass()),
            "the same hold of ore burns more than of Alloys");
    }

    #[test]
    fn broadcasts_if_any_member_broadcasts() {
        // Corporate logistics may run silent; the Authority carrier may not.
        assert!(!fleet(&[(ShipKind::Raider, 2), (ShipKind::Convoy, 1)], None).broadcasts());
        assert!(fleet(&[(ShipKind::Raider, 2), (ShipKind::Freighter, 1)], None).broadcasts());
        assert!(fleet(&[(ShipKind::Corvette, 1)], None).broadcasts());
        // Raiders and/or scouts only → dark.
        assert!(!fleet(&[(ShipKind::Raider, 3)], None).broadcasts());
        assert!(!fleet(&[(ShipKind::Raider, 2), (ShipKind::Scout, 1)], None).broadcasts());
    }

    #[test]
    fn pirates_never_broadcast_but_the_authority_always_declares() {
        let warships = [(ShipKind::Corvette, 2), (ShipKind::Destroyer, 1), (ShipKind::Cruiser, 1)];
        let mut pirates = fleet(&warships, None);
        pirates.owner = crate::ids::PlayerId::PIRATE;
        assert!(!pirates.broadcasts(), "an outlaw pack runs dark whatever it flies");
        let mut authority = fleet(&[(ShipKind::Corvette, 1)], None);
        authority.owner = crate::ids::PlayerId::TCA;
        assert!(authority.broadcasts(), "Authority warships keep declaring themselves");
        assert!(fleet(&warships, None).broadcasts(), "a corporation's warships still broadcast");
    }

    #[test]
    fn corporate_convoys_are_dark_without_becoming_jump_ships() {
        use crate::fuel::{Drive, ReactorGrade};
        assert!(!ShipKind::Convoy.broadcasts());
        assert!(!ShipKind::Convoy.default_engine().1.jumps());
        assert!(ShipKind::Freighter.broadcasts());
        assert!(!ShipKind::Freighter.default_engine().1.jumps());
        assert!(ShipKind::Raider.default_engine() == (ReactorGrade::Antimatter, Drive::Maelstrom) && !ShipKind::Raider.broadcasts());
        assert!(ShipKind::Scout.default_engine() == (ReactorGrade::Antimatter, Drive::Maelstrom) && !ShipKind::Scout.broadcasts());
        // The power rule, and no Maelstrom on a ship of the line.
        assert!(!ShipKind::Battleship.can_mount(ReactorGrade::Antimatter, Drive::Maelstrom));
        assert!(ShipKind::Battleship.can_mount(ReactorGrade::Antimatter, Drive::Tempest));
        assert!(ShipKind::Battleship.can_mount(ReactorGrade::Helium, Drive::Tempest));
        assert!(!ShipKind::Raider.can_mount(ReactorGrade::Fission, Drive::Gale));
        assert!(ShipKind::Raider.can_mount(ReactorGrade::Fusion, Drive::Zephyr));
    }

    /// §hull-ladder (docs/hull-ladder-plan.md §2): the engine-count table —
    /// engines are half of every hull, in the hull's size.
    #[test]
    fn engines_follow_the_ladder() {
        use crate::fuel::EngineSize as Sz;
        let table = [
            (ShipKind::Scout, 1, Sz::Light), (ShipKind::Raider, 1, Sz::Light), (ShipKind::Corvette, 5, Sz::Light),
            (ShipKind::Destroyer, 25, Sz::Light), (ShipKind::Cruiser, 5, Sz::Heavy), (ShipKind::Battleship, 25, Sz::Heavy),
            (ShipKind::Dreadnought, 10, Sz::Capital), (ShipKind::Titan, 100, Sz::Capital),
            (ShipKind::TinyFreighter, 2, Sz::Light), (ShipKind::CombatFreighter, 3, Sz::Light), (ShipKind::SmallFreighter, 4, Sz::Light),
            (ShipKind::Convoy, 13, Sz::Light), (ShipKind::Colony, 10, Sz::Light), (ShipKind::Transport, 15, Sz::Light),
            (ShipKind::LargeFreighter, 40, Sz::Light), (ShipKind::HeavyFreighter, 5, Sz::Heavy), (ShipKind::BulkFreighter, 15, Sz::Heavy),
            (ShipKind::MiningBarge, 3, Sz::Light), (ShipKind::Builder, 4, Sz::Light), (ShipKind::MiningBeltStation, 30, Sz::Light),
            (ShipKind::SurveyProbe, 0, Sz::Light),
        ];
        for (kind, n, size) in table {
            assert_eq!((kind.engines(), kind.engine_size()), (n, size), "{kind:?}");
        }
    }

    #[test]
    fn raiders_and_convoys_project_distinct_mobile_sensors() {
        let raider = fleet(&[(ShipKind::Raider, 1)], None);
        assert!(raider.projects_sensor());
        assert_eq!(raider.sensor_mult(), 1.0);

        let convoy = fleet(&[(ShipKind::Convoy, 2)], None);
        assert!(convoy.projects_sensor());
        assert_eq!(convoy.sensor_mult(), CONVOY_SENSOR_MULT);

        let escort = fleet(&[(ShipKind::Raider, 1), (ShipKind::Convoy, 2)], None);
        assert!(escort.projects_sensor());
        assert_eq!(escort.sensor_mult(), 1.0);

        assert!(!fleet(&[(ShipKind::Scout, 1)], None).projects_sensor());
    }

    #[test]
    fn flagship_follows_precedence_colony_convoy_corvette_raider_scout() {
        assert_eq!(
            fleet(&[(ShipKind::Convoy, 1), (ShipKind::Colony, 1)], None).flagship_kind(),
            ShipKind::Colony
        );
        assert_eq!(
            fleet(&[(ShipKind::Convoy, 1), (ShipKind::Corvette, 2)], None).flagship_kind(),
            ShipKind::Convoy
        );
        assert_eq!(
            fleet(&[(ShipKind::Raider, 5), (ShipKind::Scout, 1)], None).flagship_kind(),
            ShipKind::Raider
        );
        assert_eq!(
            fleet(&[(ShipKind::Scout, 2)], None).flagship_kind(),
            ShipKind::Scout
        );
    }

    #[test]
    fn count_class_buckets_are_deterministic_and_never_narrower_than_the_count() {
        // The exact bucket at each threshold edge.
        let cases = [
            (1, CountClass::One),
            (2, CountClass::TwoToThree),
            (3, CountClass::TwoToThree),
            (4, CountClass::FourToSeven),
            (7, CountClass::FourToSeven),
            (8, CountClass::EightToFifteen),
            (15, CountClass::EightToFifteen),
            (16, CountClass::SixteenToThirty),
            (30, CountClass::SixteenToThirty),
            (31, CountClass::ThirtyOnePlus),
            (999, CountClass::ThirtyOnePlus),
        ];
        for (n, class) in cases {
            assert_eq!(CountClass::from_count(n), class, "n={n}");
            // The bucket never rules the true count OUT (leak-safety invariant).
            let lo_hi = match class {
                CountClass::One => (1, 1),
                CountClass::TwoToThree => (2, 3),
                CountClass::FourToSeven => (4, 7),
                CountClass::EightToFifteen => (8, 15),
                CountClass::SixteenToThirty => (16, 30),
                CountClass::ThirtyOnePlus => (31, u32::MAX),
            };
            assert!(
                n >= lo_hi.0 && n <= lo_hi.1,
                "count must lie inside its own bucket"
            );
        }
    }

    #[test]
    fn stealth_transit_halves_speed_and_quiets_the_signature() {
        let mut f = fleet(&[(ShipKind::Raider, 1)], None);
        // Full speed → the detection anchor (signature 1.0).
        f.vel = Vec2::new(f.max_speed(), 0.0);
        let full_sig = f.signature();
        assert!(
            (full_sig - 1.0).abs() < 1e-9,
            "a lone raider at full speed is the 1.0 anchor"
        );
        assert_eq!(
            f.transit_speed(),
            f.max_speed(),
            "Full transit = formation speed"
        );
        // Stealth halves the move speed and, at that speed, quiets the signature.
        f.transit = TransitMode::Stealth;
        assert!(
            (f.transit_speed() - f.max_speed() * 0.5).abs() < 1e-9,
            "Stealth creeps at STEALTH_FRACTION"
        );
        f.vel = Vec2::new(f.transit_speed(), 0.0);
        assert!(
            f.signature() < full_sig,
            "creeping is quieter than flank speed"
        );
    }

    #[test]
    fn combat_power_sums_over_composition() {
        let f = fleet(&[(ShipKind::Raider, 2), (ShipKind::Corvette, 1)], None);
        assert_eq!(f.attack_power(), 2.0 * 3.0 + 1.0); // raiders(3) + corvette(1)
        assert_eq!(f.defense_power(), 2.0 * 2.0 + 4.0); // raiders(2) + corvette(4)
        assert!(f.is_combatant());
        assert!(!fleet(&[(ShipKind::Convoy, 4)], None).is_combatant());
    }

    // --- §modules Part B: the loadout partition (invariant Σ ≤ composition) ---
    use crate::module::{Loadout, ModuleKind};

    fn total_fitted(f: &Fleet, k: ShipKind) -> u32 {
        f.loadouts.get(&k).map(|m| m.values().sum()).unwrap_or(0)
    }
    fn invariant_holds(f: &Fleet) -> bool {
        f.loadouts
            .iter()
            .all(|(k, m)| m.values().sum::<u32>() <= f.count(*k))
    }

    #[test]
    fn loadout_partition_holds_through_add_remove_detach_fold() {
        let md = Loadout::new(vec![ModuleKind::MassDriver]);
        let mut f = fleet(&[(ShipKind::Raider, 5)], None);
        f.set_fitted(ShipKind::Raider, &md, 3); // 3 fitted, 2 unfitted
        assert!(invariant_holds(&f) && total_fitted(&f, ShipKind::Raider) == 3);

        // Removing an UNFITTED ship leaves the fits intact.
        assert_eq!(f.remove_stack(ShipKind::Raider, &Loadout::default(), 1), 1);
        assert_eq!(f.count(ShipKind::Raider), 4);
        assert_eq!(total_fitted(&f, ShipKind::Raider), 3);
        // Removing a FITTED ship drops a fit too.
        assert_eq!(f.remove_stack(ShipKind::Raider, &md, 1), 1);
        assert_eq!(f.count(ShipKind::Raider), 3);
        assert_eq!(total_fitted(&f, ShipKind::Raider), 2);
        assert!(invariant_holds(&f));

        // §roster: detach 2 (fitted-first) — the actual HULLS leave, carrying
        // their fits with them, and the source is left with only unfitted ships.
        let taken = f.detach_ships(ShipKind::Raider, 2);
        assert_eq!(taken.len(), 2);
        assert_eq!(
            taken.iter().filter(|s| !s.loadout.is_empty()).count(),
            2,
            "escorts keep their fits"
        );
        assert_eq!(f.count(ShipKind::Raider), 1);
        assert_eq!(total_fitted(&f, ShipKind::Raider), 0);
        assert!(invariant_holds(&f));

        // Absorb another fleet's hulls (merge/relief): their fits come along.
        let mut g = fleet(&[(ShipKind::Raider, 2)], None);
        g.set_fitted(
            ShipKind::Raider,
            &Loadout::new(vec![ModuleKind::WhippleArmor]),
            2,
        );
        f.absorb_ships(std::mem::take(&mut g.ships));
        assert_eq!(f.count(ShipKind::Raider), 3);
        assert_eq!(total_fitted(&f, ShipKind::Raider), 2);
        assert!(invariant_holds(&f));
    }

    /// §roster THE LOAD-BEARING INVARIANT: `ships` is truth and
    /// `composition`/`loadouts` are its projection, so after ANY mutation the
    /// two must agree exactly. Storing it twice is only safe because this holds.
    #[test]
    fn roster_and_cache_always_agree() {
        let md = Loadout::new(vec![ModuleKind::MassDriver]);
        let wa = Loadout::new(vec![ModuleKind::WhippleArmor]);
        let agree = |f: &Fleet| {
            let mut comp: BTreeMap<ShipKind, u32> = BTreeMap::new();
            let mut lo: crate::combat::LoadoutMap = BTreeMap::new();
            for s in &f.ships {
                *comp.entry(s.kind).or_insert(0) += 1;
                if !s.loadout.is_empty() {
                    *lo.entry(s.kind)
                        .or_default()
                        .entry(s.stack_key())
                        .or_insert(0) += 1;
                }
            }
            comp == f.composition && lo == f.loadouts
                // …and ids are unique + sorted (deterministic iteration).
                && f.ships.windows(2).all(|w| w[0].id < w[1].id)
        };
        let mut f = fleet(&[(ShipKind::Raider, 4), (ShipKind::Corvette, 2)], None);
        assert!(agree(&f), "fresh");
        f.set_fitted(ShipKind::Raider, &md, 2);
        assert!(agree(&f), "after set_fitted");
        f.add_fitted(ShipKind::Corvette, &wa, 3);
        assert!(agree(&f), "after add_fitted");
        f.remove(ShipKind::Raider, 1);
        assert!(agree(&f), "after remove");
        f.remove_stack(ShipKind::Raider, &md, 1);
        assert!(agree(&f), "after remove_stack");
        let taken = f.detach_ships(ShipKind::Corvette, 2);
        assert!(agree(&f), "after detach");
        f.absorb_ships(taken);
        assert!(agree(&f), "after absorb");
        f.reset_to(ShipKind::Scout, 5);
        assert!(agree(&f), "after reset_to");
        // A serde round trip preserves the roster and re-derives an equal cache.
        let json = serde_json::to_string(&f).unwrap();
        let back: Fleet = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ships, f.ships);
        assert!(agree(&back), "after a snapshot round-trip");
    }

    /// §roster: damage rides the HULL. A wounded ship keeps its deficit through
    /// a split and a merge — no pooling, no pro-rata, no laundering by reorg.
    #[test]
    fn damage_travels_with_the_hull_through_split_and_merge() {
        let mut f = fleet(&[(ShipKind::Raider, 3)], None);
        f.ships[1].hp = 50.0; // one wounded raider
        let full = ShipKind::Raider.hull_mass();
        assert!((f.damage_fraction() - (full - 50.0) / (3.0 * full)).abs() < 1e-9);
        // Detaching the wounded hull moves the damage with it.
        let wounded_id = f.ships[1].id;
        let taken = f.detach_ships(ShipKind::Raider, 3);
        assert_eq!(taken.iter().find(|s| s.id == wounded_id).unwrap().hp, 50.0);
        // …and absorbing it back preserves the hp under a re-issued id.
        let mut g = fleet(&[(ShipKind::Raider, 1)], None);
        g.absorb_ships(taken);
        assert_eq!(
            g.ships.iter().filter(|s| s.hp == 50.0).count(),
            1,
            "exactly one hull is still hurt"
        );
        assert_eq!(g.count(ShipKind::Raider), 4);
    }

    #[test]
    fn normalize_clamps_an_over_fit_partition() {
        let mut f = fleet(&[(ShipKind::Raider, 2)], None);
        // §roster: hand-corrupt the CACHE (4 fits over 2 hulls). Because the
        // roster is the source of truth, rebuilding restores the invariant
        // exactly — a corrupted projection can't survive a rebuild.
        let m = f.loadouts.entry(ShipKind::Raider).or_default();
        m.insert(Loadout::new(vec![ModuleKind::MassDriver]).key(), 3);
        m.insert(Loadout::new(vec![ModuleKind::TorpedoRack]).key(), 1);
        assert!(!invariant_holds(&f));
        f.normalize_loadouts();
        assert!(invariant_holds(&f), "normalize restores Σ ≤ composition");
        assert_eq!(
            total_fitted(&f, ShipKind::Raider),
            0,
            "the hulls were never actually fitted"
        );
    }

    #[test]
    fn old_snapshot_without_loadouts_loads_all_unfitted() {
        let f = fleet(&[(ShipKind::Raider, 3)], None);
        // Strip the loadouts key → a pre-module snapshot; serde default fills it.
        let mut v: serde_json::Value = serde_json::to_value(&f).unwrap();
        v.as_object_mut().unwrap().remove("loadouts");
        let f2: Fleet = serde_json::from_value(v).unwrap();
        assert!(
            f2.loadouts.is_empty(),
            "no field → all-unfitted, zero migration"
        );
        assert_eq!(f2.count(ShipKind::Raider), 3);
    }
}

#[cfg(test)]
mod drive_wire {
    use super::*;

    /// The client hand-parses this shape, so pin it: a rename of a variant that
    /// only broke the panel would otherwise ship silently.
    #[test]
    fn drive_state_serialises_the_shape_the_panel_reads() {
        let j = |d: DriveState| serde_json::to_string(&d).unwrap();
        assert_eq!(j(DriveState::Thrusters), "\"thrusters\"");
        assert_eq!(
            j(DriveState::Cruising(crate::transit::Regime::Warp)),
            "{\"cruising\":\"warp\"}"
        );
        assert_eq!(
            j(DriveState::Spooling {
                to: crate::transit::Regime::Warp,
                left: 1.5
            }),
            "{\"spooling\":{\"to\":\"warp\",\"left\":1.5}}"
        );
        assert_eq!(
            j(DriveState::Dropping {
                from: crate::transit::Regime::Warp,
                left: 1.0
            }),
            "{\"dropping\":{\"from\":\"warp\",\"left\":1.0}}"
        );
    }
}

#[cfg(test)]
mod cargo_stack_match_tests {
    use super::*;
    use crate::cargo::Commodity;

    fn fleet_with(cargo: Option<Cargo>, extra: &[(Commodity, u32)]) -> Fleet {
        let mut f = Fleet::single(
            EntityId(1),
            PlayerId(1),
            ShipKind::Freighter,
            Vec2::ZERO,
            FleetOrder::Idle,
            None,
        );
        f.cargo = cargo;
        f.cargo_extra = extra.iter().copied().collect();
        f
    }

    #[test]
    fn matches_exactly_the_manifest_cargo_stacks_builds() {
        let mut kinds = Commodity::ALL.to_vec();
        kinds.sort();
        let (lo, mid, hi) = (kinds[0], kinds[kinds.len() / 2], kinds[kinds.len() - 1]);
        let cases = [
            fleet_with(None, &[]),
            fleet_with(Some(Cargo { commodity: mid, units: 5 }), &[]),
            fleet_with(Some(Cargo { commodity: mid, units: 0 }), &[(lo, 3)]),
            fleet_with(Some(Cargo { commodity: mid, units: 5 }), &[(lo, 3), (hi, 7)]),
            fleet_with(Some(Cargo { commodity: mid, units: 5 }), &[(mid, 4)]),
            fleet_with(Some(Cargo { commodity: lo, units: 2 }), &[(mid, 0), (hi, 1)]),
            fleet_with(Some(Cargo { commodity: hi, units: 2 }), &[(lo, 1), (mid, 0)]),
            fleet_with(None, &[(lo, 0), (mid, 9)]),
        ];
        for fleet in &cases {
            let stacks = fleet.cargo_stacks();
            assert!(fleet.cargo_stacks_matches(&stacks), "{stacks:?}");
            let mut shorter = stacks.clone();
            if shorter.pop().is_some() {
                assert!(!fleet.cargo_stacks_matches(&shorter), "{stacks:?}");
            }
            let mut longer = stacks.clone();
            longer.push(Cargo { commodity: hi, units: 1 });
            assert!(!fleet.cargo_stacks_matches(&longer), "{stacks:?}");
            if let Some(first) = stacks.first() {
                let mut bumped = stacks.clone();
                bumped[0] = Cargo { units: first.units + 1, ..*first };
                assert!(!fleet.cargo_stacks_matches(&bumped), "{stacks:?}");
            }
        }
    }
}

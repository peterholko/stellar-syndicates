//! Ships: the mobile entities of the galaxy.
//!
//! Two types embody the §7 convoy-vs-raider dial — not a special rule, just
//! different acceleration / top-speed. Convoys are slow and heavy; raiders are
//! fast and light and can run a convoy down. (The lane mass-reduction effect of
//! §7/§10 lands in a later milestone.)
//!
//! Every ship moves under flip-and-burn and acts on a standing **order**; the
//! world advances each ship once per tick. There is no real-time piloting — the
//! async-native, lightspeed-bound design demands standing orders, not micro.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cargo::Cargo;
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;
use crate::movement::advance_toward;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShipKind {
    /// Slow, heavy hauler — the largest ship in the game (§7). Carries trade.
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
    /// The SETTLEMENT ship (§ships part 3): the HEAVIEST hull flying — slow,
    /// expensive to fuel — with no cargo bay (it IS the cargo: colonists +
    /// infrastructure). Claiming is now PHYSICAL: send it to an unclaimed
    /// system; on arrival ownership transfers and the ship is CONSUMED (it
    /// becomes the colony). BROADCASTS under the Convention (a declared
    /// civilian settlement vessel — and your expansion is telegraphed,
    /// raidable, escortable). Destroyed in transit = colonists lost.
    Colony,
    /// The ACTIVE-INTEL ship (§scout): the lightest hull in the game — fastest
    /// to accelerate, cheapest to fuel — with NO cargo capacity and negligible
    /// combat strength (in any engagement it is simply destroyed; its defense is
    /// speed and darkness, not armor). Runs DARK like a raider, projects an
    /// oversized sensor bubble (`sensor_mult`), and near rival systems captures
    /// timestamped intel snapshots of their fortifications.
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
    /// hyperspace buoys and deep space sensors where they stand. Emplacements
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
}

/// Mass added per unit of cargo carried. A fully-loaded convoy is meaningfully
/// heavier than an empty one, so it accelerates noticeably worse (a = F/m) — your
/// richest shipments are also the most sluggish. Tunable.
pub const CARGO_MASS_PER_UNIT: f64 = 28.0;

/// §TCA Part 5: whole cargo UNITS one Convoy hull can lift. A fleet's capacity is
/// this × the convoys aboard, so the "capacity scales with the number of convoys"
/// rule finally has a number. Tunable, playtest placeholder.
///
/// This also bounds standing logistics: automation selects a real idle cargo
/// fleet at the source and caps its lot to the fleet's aggregate hold.
pub const CARGO_UNITS_PER_CONVOY: u32 = 250;

impl ShipKind {
    /// Hull (empty) MASS, m₀. Trade convoys are ORDERS OF MAGNITUDE more massive
    /// than raiders (here ~22×), which is what makes them ponderous — the
    /// acceleration asymmetry emerges from this, not from hand-set accel consts.
    /// The scout is the LIGHTEST hull (mass drives both acceleration and the
    /// fuel-∝-mass trip cost, so light = fast AND cheap to run).
    pub fn hull_mass(self) -> f64 {
        match self {
            ShipKind::Convoy => 4500.0,
            ShipKind::Builder => 2500.0, // a crane and its shops — working mass, no bulk hold
            ShipKind::Raider => 200.0,
            ShipKind::Corvette => 800.0,
            ShipKind::Colony => 6000.0, // heaviest CIVILIAN hull — fuel-∝-mass bites
            ShipKind::Scout => 80.0,
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

    /// CONSTANT cruise speed (sim units / s), GDD §14.1 — there is no
    /// acceleration; a ship travels at exactly this speed (the fleet moves at its
    /// slowest member's, [`Fleet::max_speed`]). All stay well below `c` (= 300)
    /// so relativity is respected — nothing outruns its own light. Ordering
    /// preserves the old relative feel (scout > raider > corvette > convoy >
    /// colony).
    ///
    /// CALIBRATION (migration-gentle): magnitudes are set so a representative
    /// galaxy-crossing trip (~8000 su, the 4-player galaxy radius) takes about as
    /// long as the old flip-and-burn did — whose accel ramp meant its AVERAGE
    /// speed was well under the max cap. Convoy anchors it: old convoy (a=1.5,
    /// cap 48) crossed 8000 su in ≈199 s; constant 40 gives 8000/40 = 200 s. The
    /// other kinds keep the old max-speed RATIOS off that anchor, so raider/
    /// convoy chase dynamics and pacing hold (raider 8000 su: old ≈78 s, new 80 s;
    /// colony old ≈233 s, new 242 s). Tunable.
    pub fn max_speed(self) -> f64 {
        match self {
            ShipKind::Convoy => 40.0,
            ShipKind::Builder => 35.0, // ponderous — it is a worksite, not a courier
            ShipKind::Raider => 100.0,
            ShipKind::Corvette => 65.0, // keeps station with convoys, can't chase raiders
            ShipKind::Colony => 33.0, // slowest civilian — the long, visible voyage
            ShipKind::Scout => 115.0, // the fastest thing flying — still < c/2
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
    /// light-delayed). Convoys do; raiders and scouts run DARK — visible only
    /// inside a rival's sensor coverage. One source of truth for the View's
    /// gating (a broadcasting spy would be useless).
    pub fn broadcasts(self) -> bool {
        // Convoys (trade), corvettes (a DECLARED escort deters), and colony
        // ships (a declared civilian settlement vessel — expansion is
        // telegraphed) broadcast; raiders and scouts run dark. §ladder: every
        // CAPITAL broadcasts — a ship of the line is a declared presence
        // (deterrence is its job; sneaking is the Raider's).
        matches!(
            self,
            ShipKind::Convoy
                | ShipKind::Corvette
                | ShipKind::Colony
                | ShipKind::Destroyer
                | ShipKind::Cruiser
                | ShipKind::Battleship
                | ShipKind::Dreadnought
                | ShipKind::Titan
                // §ground: a troop convoy is a declared civilian-escort formation
                // — an invasion cannot be a surprise.
                | ShipKind::Transport
                | ShipKind::Freighter
        )
    }

    /// Multiplier on `config.sensor_range` for the sensor bubble THIS ship
    /// projects into its owner's coverage union. The scout's whole point:
    /// `SCOUT_SENSOR_MULT` × the standard bubble — mobile vision that out-sees
    /// any other ship. Tunable.
    pub fn sensor_mult(self) -> f64 {
        match self {
            ShipKind::Scout => SCOUT_SENSOR_MULT,
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

    /// Offensive weight when this kind is the AGGRESSOR in an engagement.
    pub fn attack_weight(self) -> f64 {
        match self {
            ShipKind::Builder => 0.0,  // cranes do not shoot
            ShipKind::Raider => 3.0,   // the hunter
            ShipKind::Corvette => 1.0, // guards; barely bites back
            ShipKind::Convoy => 0.0,   // civilians don't attack
            ShipKind::Colony => 0.0,   // colonists, not soldiers
            ShipKind::Scout => 0.0,    // dies if engaged — speed is its armor
            // §ladder: raw broadside climbs the ladder — but per Armaments
            // spent it DECLINES past the Cruiser (the efficiency invariant).
            ShipKind::Destroyer => 2.4,
            ShipKind::Cruiser => 4.5,
            ShipKind::Battleship => 8.0,
            ShipKind::Dreadnought => 12.0,
            ShipKind::Titan => 24.0,
            ShipKind::Transport => 0.0, // troopers do not fight ships
            ShipKind::Freighter => 0.0, // an unarmed common carrier
        }
    }

    /// Defensive weight when this kind is ATTACKED (or screening a defender).
    pub fn defense_weight(self) -> f64 {
        match self {
            ShipKind::Builder => 1.0,  // a soft worksite that needs escorting
            ShipKind::Raider => 2.0,
            ShipKind::Corvette => 4.0, // the armored screen — built to be attacked
            ShipKind::Convoy => 1.0,
            ShipKind::Colony => 1.0, // a fat civilian hull — escort it
            ShipKind::Scout => 0.0, // no armor at all
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
    /// modules; logistics hulls (Convoy/Colony) carry none. §ladder: capitals
    /// are where COMBINATIONS live — slots and points both climb. Tunable.
    pub fn module_slots(self) -> u32 {
        match self {
            ShipKind::Builder => 0, // no hardpoints — its fittings ARE the crane
            ShipKind::Corvette => 2,
            ShipKind::Raider => 2,
            ShipKind::Scout => 1,
            // §TCA: the Authority's carrier fits no modules — it is not a warship.
            ShipKind::Convoy | ShipKind::Colony | ShipKind::Transport | ShipKind::Freighter => 0,
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
        ShipKind::Scout => 2,
        ShipKind::Convoy => 2,
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
        (ShipKind::Raider, Family::Torpedo) => 1.25,      // the torpedo boat
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
/// it can be built? The five ladder hulls do; the original five never do.
pub fn requires_hull_unlock(kind: ShipKind) -> bool {
    matches!(
        kind,
        ShipKind::Destroyer
            | ShipKind::Cruiser
            | ShipKind::Battleship
            | ShipKind::Dreadnought
            | ShipKind::Titan
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
pub const FLAGSHIP_PRECEDENCE: [ShipKind; 13] = [
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
    ShipKind::Convoy,
    // A freighter fleet is pure freighters (never mixed with player ships), so its
    // rank here only names how a lone Authority hull is drawn — a hauler.
    ShipKind::Freighter,
    ShipKind::Corvette,
    ShipKind::Raider,
    // The working hull: shown only when nothing above it rides along.
    ShipKind::Builder,
    ShipKind::Scout,
];

/// All ship kinds, in a fixed deterministic order (composition iteration,
/// damage-pool distribution, report ordering). Kept in sync with [`ShipKind`].
pub const ALL_SHIP_KINDS: [ShipKind; 13] = [
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
];

/// The fastest flying speed across every ship kind — the single number the
/// light-game invariant ([`crate::config::SimConfig::light_ratio`]) is measured
/// against: `c` must comfortably outrun even the quickest hull so information
/// and orders can, in principle, overtake any raider. Recomputed from the speed
/// table so a future speed edit can't silently outrun light unnoticed.
pub fn fastest_ship_speed() -> f64 {
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

// --- STANDING UPKEEP (§upkeep) -------------------------------------------------
// Fleets cost PROVISIONS every second they exist, fed or not, anywhere. This is
// the ceiling on force: before it, a hull was a one-off purchase and an idle
// navy was free forever, so hoarding was strictly dominant and the economy never
// pushed back. The charge is on CREW, not tonnage — a Titan is a city under arms
// while a convoy is mostly empty hold — so the sink lands on warfleets and
// leaves logistics cheap.
//
// Scale check against a fresh home (one staffed Agroplex ≈ 1.2 Provisions/s, its
// 2.0M population eating 0.12/s): the ~1.08/s spare feeds roughly 18 raiders, or
// 10 corvettes, and not one Titan. A home alone supports a raiding wing; a line
// of battle needs colonies behind it. Every value Tunable — this table is the
// single dial on how big a navy the map can carry.

/// PROVISIONS per second, per hull. `Freighter` is 0: the Authority's own hull is
/// never a corporation's cost.
pub fn upkeep_per_sec(kind: ShipKind) -> f64 {
    match kind {
        ShipKind::Builder => 0.04,   // a full work crew rides along
        ShipKind::Scout => 0.01,     // a couple of crew and a very good sensor
        ShipKind::Convoy => 0.03,    // civilian hauler: big hull, small crew
        ShipKind::Colony => 0.06,    // the colonists aboard eat too
        // §ground: a trooper is a barracks under way — marines eat well.
        ShipKind::Transport => 0.35,
        ShipKind::Raider => 0.06,
        ShipKind::Corvette => 0.10,
        ShipKind::Destroyer => 0.25,
        ShipKind::Cruiser => 0.45,
        ShipKind::Battleship => 0.80,
        ShipKind::Dreadnought => 1.40,
        ShipKind::Titan => 2.50, // a flagship is a standing commitment, not a purchase
        ShipKind::Freighter => 0.0,
    }
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
    /// and the one place every corporation's traffic converges.
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
/// a player who parked "at" a system is docked, which matters more now that
/// docking is a state they can see. Nothing under way can trip it — the
/// predicate requires an Idle fleet — so a ship merely passing near a world is
/// never captured by it. Tunable.
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

/// A fleet's TRANSIT THROTTLE (§Part 4): the stealth-vs-speed choice. `Full`
/// (default, behavior-preserving) runs at the formation speed and lights up the
/// fleet's signature at flank; `Stealth` creeps at `STEALTH_FRACTION` of it (~2×
/// trip time) to stay quiet. Applies to MoveTo/Patrol; pursuit is always Full
/// (v1). serde default = Full keeps old snapshots loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitMode {
    #[default]
    Full,
    Stealth,
}

impl TransitMode {
    /// The fraction of formation speed this mode travels at.
    pub fn fraction(self) -> f64 {
        match self {
            TransitMode::Full => 1.0,
            TransitMode::Stealth => crate::detection::STEALTH_FRACTION,
        }
    }
}

/// A fleet's standing order — what it does without further input. Orders are
/// FLEET-LEVEL (GDD §13.1): the whole formation moves, intercepts, and holds as
/// one entity. A fleet-of-one behaves exactly as the old single ship did.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FleetOrder {
    /// At rest, no goal.
    Idle,
    /// Flip-and-burn to a fixed point, then go [`FleetOrder::Idle`].
    MoveTo { dest: Vec2 },
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
    Intercept { target: EntityId },
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
    Blockade { system: EntityId, station: Vec2 },
    /// ATTACK a rival fleet to DESTROY it (§offensive-orders Part 1): the targeted
    /// destroy verb. Pursues exactly like [`FleetOrder::Intercept`], but on contact
    /// it opens a FULL-DURATION engagement (`raid = false`) regardless of the
    /// target's kind — so a convoy is destroyed (its cargo lost with it), not
    /// raided. RAID (`Intercept` on a convoy) steals; ATTACK destroys.
    Attack { target: EntityId },
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
    DeliverToSystem { system: EntityId },
    /// §TCA Part 5: HAUL TO THE MARKET HUB — a PLAYER-owned convoy carrying its
    /// own cargo to the hub, where it deposits into the owner's warehouse and
    /// (optionally) sells the lot at that tick's quantity-aware execution price. Unlike the legacy
    /// `SellAtHub`, the fleet SURVIVES: it is the player's hull, so it goes Idle at
    /// the Market Hub ready for its next job. This is the player-owned half of
    /// the two logistics channels; TCA freight is the other.
    DeliverToWarehouse { sell_on_arrival: bool },
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
}

impl Ship {
    /// A fresh, undamaged hull.
    pub fn new(id: u32, kind: ShipKind, loadout: crate::module::Loadout) -> Self {
        Ship { id, kind, loadout, hp: kind.hull_mass() }
    }
    /// Full hull — the tactical engine's `max_hp` for this kind.
    pub fn max_hp(&self) -> f64 {
        self.kind.hull_mass()
    }
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
        to: crate::lane::Regime,
        /// Seconds left.
        left: f64,
    },
    /// Under way at `Regime` speed, locked on its current course.
    Cruising(crate::lane::Regime),
    /// Shutting down to thrusters. Still moving, still unable to steer.
    /// `from` is the drive being shut down — a HYPERSPACE drive winding down is
    /// still wound into the lane medium, so its shutdown is the last thing the
    /// lane transmits for it (§coupled); a warp drive's drop stirs nothing.
    /// serde default = Warp so pre-`from` snapshots load harmlessly.
    Dropping {
        #[serde(default = "crate::lane::Regime::warp")]
        from: crate::lane::Regime,
        left: f64,
    },
}

impl DriveState {
    /// Can the fleet change heading right now? Only on thrusters — which is the
    /// whole mechanic.
    pub fn can_steer(self) -> bool {
        matches!(self, DriveState::Thrusters)
    }

    /// §coupled: is this drive STIRRING THE LANE MEDIUM — i.e. is the hull IN
    /// THE LANE? It is from the moment its hyperspace drive CATCHES until that
    /// drive is fully out: Cruising, and the wind-down that follows.
    ///
    /// SPIN-UP IS THE APPROACH, NOT THE RIDE (design ruling). A spooling hull
    /// has reached the lane and is lighting its drive, but it has not joined
    /// the flow — the drive has not bitten, which is exactly why `regime()`
    /// still has it crawling at thruster speed. The wind-down is the mirror
    /// case and NOT symmetric on purpose: there the drive HAS bitten and is
    /// still wound in, so the lane carries the shutdown as the last word about
    /// a departing hull.
    ///
    /// So the lane's first word about a hull is the moment it joins the flow,
    /// and its last is that hull's drive going dark. Read identically for the
    /// owner's own reports and for a tripwire's read of a rival's wake. A warp
    /// drive touches no lane in any state.
    pub fn stirs_the_lane(self) -> bool {
        matches!(
            self,
            DriveState::Cruising(crate::lane::Regime::Hyperspace)
                | DriveState::Dropping { from: crate::lane::Regime::Hyperspace, .. }
        )
    }

    /// The regime this state moves at. Transitions run at thruster speed: a
    /// drive that has not caught yet is not carrying you, and one that is
    /// shutting down has already let go.
    pub fn regime(self) -> crate::lane::Regime {
        match self {
            DriveState::Cruising(r) => r,
            _ => crate::lane::Regime::Thrusters,
        }
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
    /// Cargo carried (convoys only; raiders carry none). Broadcast withholds
    /// this — it is revealed by sensor range, not by the Convention. Capacity
    /// scales with the number of convoys aboard; existing single-convoy rules
    /// are the N=1 case, unchanged.
    pub cargo: Option<Cargo>,
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
    /// §hyperspace: is the WARP DRIVE lit?
    ///
    /// Warp is the middle of the three drives — thrusters, warp, hyperspace —
    /// and the only one that works anywhere: it is an overlay on the same
    /// coordinates, so this is a state a fleet is in rather than a place it went.
    /// The HYPERSPACE drive is not a flag here because it is not a free choice:
    /// it only bites near a lane, so whether it is engaged is a fact about where
    /// the fleet is, read back through `Regime`.
    ///
    /// Warp off is the pre-hyperspace game exactly: slow, and quiet — since
    /// signature already scales with speed (§6.3), going dark and going slow are
    /// the same act. serde default = false, so pre-feature fleets load on
    /// thrusters.
    #[serde(default)]
    pub warp: bool,
    /// §hyperspace: the ROUTE this fleet is flying, as remaining waypoints.
    ///
    /// A lane is a curve, so riding one means following its centerline rather
    /// than flying a straight line and hoping to clip the ribbon at a useful
    /// angle. Planned once at order time from the lane network, then consumed
    /// point by point — so the path a fleet flies is the path the player was
    /// shown, and it does not silently re-plan mid-flight on information the
    /// player never had.
    ///
    /// Each step carries the ROAD it belongs to, so speed is a property of what
    /// the fleet is doing rather than of where it happens to be standing.
    ///
    /// Empty = fly straight to the order's destination, which is both the
    /// fallback when no lane saves time and how every pre-feature fleet loads.
    #[serde(default)]
    pub route: Vec<crate::lane::Leg>,
    /// §hyperspace: FUEL IN THE TANKS. Carried, not drawn.
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
    #[serde(default)]
    pub fuel: f64,
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
    /// §hyperspace: which layer this fleet is moving through, as of the last
    /// tick it moved. Recorded rather than re-derived so the badge the map shows
    /// is the regime the fleet was actually flown at.
    #[serde(default)]
    pub regime: crate::lane::Regime,
    /// §hyperspace: this fleet has run dry and is holding. A latch, so the
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
    /// TRANSIT THROTTLE (§Part 4): Full (default) or Stealth. Governs move speed
    /// and, via the retarded velocity, the fleet's detection signature.
    #[serde(default)]
    pub transit: TransitMode,
    /// §economy Part 4: SPECIALIST PASSENGERS aboard (kind → headcount) —
    /// people, not cargo, but they ride the SAME two-tier fog rule: the
    /// broadcast never includes them, a sensor-revealed manifest does. Berths
    /// come from the logistics hulls (`specialist::passenger_capacity`). Lost
    /// with the fleet (the one specialist loss rule); a merging fleet folds
    /// them in; a consumed colony ship DISEMBARKS them into the new colony.
    /// serde default keeps every old snapshot loading (nobody aboard).
    #[serde(default)]
    pub passengers: BTreeMap<crate::specialist::SpecialistKind, u32>,
    /// §modules Part B3: MODULE CRATES aboard (kind → count) — cargo, not fits.
    /// A `TransferModules` convoy hauls them between systems under the SAME
    /// two-tier fog rule as `passengers`/`cargo` (broadcast hides them, sensor
    /// coverage reveals the manifest). Deposited into the destination ledger on
    /// arrival; lost with the fleet; folded in on merge. serde default = none
    /// aboard, so every old snapshot loads unchanged.
    #[serde(default)]
    pub modules: BTreeMap<crate::module::ModuleKind, u32>,
    /// ENGAGEMENT POSTURE (§offensive-orders Part 2): standing per-fleet aggression
    /// — Passive (default), Defensive, or WeaponsFree. serde default = Passive so
    /// every old snapshot loads with today's behaviour (byte-preserving).
    #[serde(default)]
    pub posture: crate::doctrine::EngagementPosture,
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
}

/// serde default for `Fleet::garrison_fed` (old snapshots load fed).
fn default_true() -> bool {
    true
}

impl Fleet {
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
        // A hull LEAVES THE SLIP FUELLED. The build already charged the yard for
        // the ship; billing the first tank separately would only add a step that
        // is never interesting to fail, and a fleet that spawned empty could not
        // move off its own pad. Every tank after this one is bought at a dock.
        let hull_mass = kind.hull_mass();
        Fleet {
            id,
            owner,
            // §roster: the fleet-of-one primitive is one HULL at full health.
            ships: vec![Ship::new(0, kind, crate::module::Loadout::default())],
            next_ship_id: 1,
            composition,
            pos,
            vel: Vec2::ZERO,
            order,
            cargo,
            mission: None,
            defense: None,
            notified_held: false,
            marines_spent: 0,
            warp: false,
            fuel: hull_mass * crate::fuel::FUEL_PER_HULL_MASS,
            regime: crate::lane::Regime::Thrusters,
            drive_state: DriveState::default(),
            route: Vec::new(),
            stalled: false,
            damage: BTreeMap::new(),
            transit: TransitMode::Full,
            passengers: BTreeMap::new(),
            modules: BTreeMap::new(),
            posture: crate::doctrine::EngagementPosture::Passive,
            garrison_fed: true,
            supplied: true,
            plunder_frac: 0.0,
            fought: false,
            loadouts: BTreeMap::new(),
            engage_freight: false,
            disposable: false,
        }
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
                *self.loadouts.entry(s.kind).or_default().entry(s.stack_key()).or_insert(0) += 1;
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
        let mut idx: Vec<usize> = (0..self.ships.len()).filter(|i| self.ships[*i].kind == kind).collect();
        // Fitted first, then by id — deterministic.
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            x.loadout.is_empty().cmp(&y.loadout.is_empty()).then(x.id.cmp(&y.id))
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
        let mut idx: Vec<usize> = (0..self.ships.len()).filter(|i| self.ships[*i].kind == kind).collect();
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            y.loadout.is_empty().cmp(&x.loadout.is_empty()).then(x.id.cmp(&y.id))
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

    /// §upkeep: Provisions per second this whole formation draws — the sum over
    /// its hulls. A fleet's standing cost, paid wherever it happens to be.
    pub fn upkeep_per_sec(&self) -> f64 {
        self.ships.iter().map(|s| upkeep_per_sec(s.kind)).sum()
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
        self.loadouts.get(&kind).map(|m| m.values().sum()).unwrap_or(0)
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
    pub fn take_stack(&mut self, kind: ShipKind, loadout: &crate::module::Loadout, n: u32) -> Vec<Ship> {
        let key = loadout.key();
        let mut idx: Vec<usize> = (0..self.ships.len())
            .filter(|i| self.ships[*i].kind == kind && self.ships[*i].stack_key() == key)
            .collect();
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            y.deficit().partial_cmp(&x.deficit()).unwrap_or(std::cmp::Ordering::Equal).then(x.id.cmp(&y.id))
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
    pub fn remove_stack(&mut self, kind: ShipKind, loadout: &crate::module::Loadout, n: u32) -> u32 {
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

    /// Add `n` UNFITTED ships of `kind` at full health (§roster).
    pub fn add(&mut self, kind: ShipKind, n: u32) {
        self.add_fitted(kind, &crate::module::Loadout::default(), n);
    }

    /// Remove up to `n` ships of `kind`. Sheds UNFITTED hulls first (the
    /// pre-roster behaviour, where `normalize_loadouts` trimmed the fitted
    /// partition last), then by id — deterministic. Returns how many went.
    pub fn remove(&mut self, kind: ShipKind, n: u32) -> u32 {
        let mut idx: Vec<usize> = (0..self.ships.len()).filter(|i| self.ships[*i].kind == kind).collect();
        idx.sort_by(|a, b| {
            let (x, y) = (&self.ships[*a], &self.ships[*b]);
            y.loadout.is_empty().cmp(&x.loadout.is_empty()).then(x.id.cmp(&y.id))
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
    /// broadcasts — you cannot hide a freighter by parking a raider beside it.
    /// A fleet of only raiders and/or scouts runs DARK.
    pub fn broadcasts(&self) -> bool {
        self.composition.keys().any(|k| k.broadcasts())
    }

    /// §explore: is this fleet ACTIVELY SURVEYING (in the dwell window)? Drives
    /// the loudness multiplier through the one shared signature path — true only
    /// while the dwell clock runs, never during the approach.
    pub fn surveying(&self) -> bool {
        matches!(self.order, FleetOrder::Survey { dwell_since: Some(_), .. })
    }

    /// The best sensor bubble this fleet projects into its owner's coverage —
    /// the MAX `sensor_mult` among its members (a scout aboard extends vision).
    pub fn sensor_mult(&self) -> f64 {
        self.composition
            .keys()
            .map(|k| k.sensor_mult())
            .fold(1.0_f64, f64::max)
    }

    /// Total EMPTY-HULL mass = Σ hull_mass(kind) × count.
    pub fn hull_mass(&self) -> f64 {
        self.composition
            .iter()
            .map(|(k, n)| k.hull_mass() * *n as f64)
            .sum()
    }

    /// §TCA Part 5: whole cargo units this fleet can lift = `CARGO_UNITS_PER_CONVOY`
    /// per Convoy aboard. Zero for a fleet with no cargo hull (raiders, corvettes,
    /// scouts, colony ships) — those soft-reject a load outright.
    pub fn cargo_capacity(&self) -> u32 {
        self.count(ShipKind::Convoy) * CARGO_UNITS_PER_CONVOY
    }

    /// Cargo mass carried by the fleet (§7).
    pub fn cargo_mass(&self) -> f64 {
        self.cargo.map(|c| c.units as f64 * CARGO_MASS_PER_UNIT).unwrap_or(0.0)
    }

    /// Total mass = Σ hull + cargo (§7). Drives fuel-∝-distance×mass exactly as
    /// before; a fleet-of-one convoy with cargo reduces to the old `Ship::mass`.
    pub fn mass(&self) -> f64 {
        self.hull_mass() + self.cargo_mass()
    }

    /// How much fuel this formation can carry.
    ///
    /// Proportional to hull mass, because the per-tick burn is too — which makes
    /// RANGE roughly independent of fleet size. A capital group is expensive to
    /// move but not short-legged, so mass buys you cost, not a shorter leash.
    /// Cargo is excluded: a hold full of ore is not a fuel bunker.
    pub fn fuel_capacity(&self) -> f64 {
        self.hull_mass() * crate::fuel::FUEL_PER_HULL_MASS
    }

    /// Fill from a supply, returning what was actually taken.
    pub fn refuel(&mut self, available: f64) -> f64 {
        let room = (self.fuel_capacity() - self.fuel).max(0.0);
        let taken = room.min(available.max(0.0));
        self.fuel += taken;
        taken
    }

    /// How far this fleet can still go at `factor` (1 normal, H open, H×LANE on
    /// an aligned lane). Distance, not time — what a range ring is drawn from.
    pub fn range_at(&self, factor: f64) -> f64 {
        let per_second = crate::fuel::fuel_tick(self.mass(), self.transit_speed(), 1.0);
        if per_second <= 1e-12 {
            return f64::INFINITY;
        }
        (self.fuel / per_second) * self.transit_speed() * factor
    }

    /// The fleet's TRANSIT speed = formation speed × the throttle fraction
    /// (§Part 4). Full = formation speed (behavior-preserving); Stealth creeps.
    pub fn transit_speed(&self) -> f64 {
        self.max_speed() * self.transit.fraction()
    }

    /// The fleet's detection SIGNATURE (§Part 4) at its CURRENT velocity — the
    /// authoritative-side value (the View recomputes the same from retarded
    /// samples). Dark fleets only; a broadcaster's is unused.
    pub fn signature(&self) -> f64 {
        crate::detection::signature(&self.composition, self.vel.length(), self.max_speed())
    }

    /// FORMATION speed (GDD §14.2): the SLOWEST member sets the pace — the
    /// minimum constant `speed(kind)` among present kinds. For a fleet-of-one this
    /// is that ship's own speed; a hammer carrying a colony ship crawls at the
    /// colony's pace, telegraphing itself by physics. Cargo does NOT slow a fleet
    /// (constant-speed model, §14.1) — it costs FUEL (mass), not time.
    pub fn max_speed(&self) -> f64 {
        self.composition
            .keys()
            .map(|k| k.max_speed())
            .fold(f64::INFINITY, f64::min)
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
    /// transit throttle (§Part 4 — Full/Stealth).
    pub fn advance(&mut self, time: f64, dt: f64, env: &crate::lane::TransitEnv<'_>) {
        // §hyperspace: the regime is resolved per tick from WHERE the fleet is
        // and what its drive is doing — normal, warp, or an aligned
        // lane. Heading comes from current velocity, so a fleet already running
        // down a lane keeps its benefit; one at rest has no alignment and starts
        // at warp speed until it is under way.
        let heading = if self.vel.length_sq() > 1e-9 { self.vel } else { Vec2::ZERO };
        // §hyperspace: the WARP DRIVE lights automatically for a fleet under way
        // and clear of a gravity well. Nobody toggles a drive per trip — the
        // default is that you use it, and shutting it OFF is the tactical act
        // (slow and quiet, since signature already scales with speed). Without
        // this a fleet would fly a route planned at warp speeds on thrusters,
        // taking the long way round a lane's bow for nothing.
        //
        // The HYPERSPACE drive needs no flag: lane-tagged route legs engage it,
        // and an in-ribbon arrival keeps that existing coupling while idle.
        let under_way = !matches!(self.order, FleetOrder::Idle);
        // A fleet that ARRIVES inside a ribbon with its hyperspace drive already
        // engaged holds that coupling while idle. Dropping it merely because the
        // current order finished made the ship's command latency jump while the
        // player was choosing its next order. The world's standing navigation
        // doctrine makes the one required exception after this step: an idle
        // player fleet beyond every comm bubble drops automatically. This does
        // not light a drive for an idle ship that was already on thrusters, and
        // gravity wells still force every drive down.
        let holding_lane = matches!(
            self.drive_state,
            DriveState::Cruising(crate::lane::Regime::Hyperspace)
        ) && !under_way
            && !env.in_well(self.pos)
            && env.lanes.on_lane(self.pos);
        let drive = self.warp || ((under_way || holding_lane) && !env.in_well(self.pos));
        // §course-change: DRIVE THE STATE MACHINE, then read speed off it.
        //
        // A fleet cannot steer above thrusters. To change course it drops all
        // the way out, turns, and re-enters — and each transition takes time.
        // That is what makes a reversal cost something, and it replaces the old
        // bounded-turn-rate model, which decided the same question by geometry
        // and lost: past the alignment gate a hull's speed fell tenfold and its
        // turning circle with it, so a Titan could come about inside a ribbon it
        // was supposedly far too big to turn in.
        let want = self.route.first().and_then(|l| l.lane).filter(|_| drive).map_or(
            if holding_lane {
                crate::lane::Regime::Hyperspace
            } else if drive && !env.in_well(self.pos) {
                crate::lane::Regime::Warp
            } else {
                crate::lane::Regime::Thrusters
            },
            |_| crate::lane::Regime::Hyperspace,
        );
        // How far the course it WANTS is from the course it is ON.
        let aim = self.route.first().map_or_else(
            || match self.order {
                FleetOrder::MoveTo { dest } => dest - self.pos,
                FleetOrder::Construct { site, .. } | FleetOrder::Demolish { site, .. } => site - self.pos,
                _ => Vec2::ZERO,
            },
            |l| l.to - self.pos,
        );
        let off_course = if heading.length_sq() > 1e-9 && aim.length_sq() > 1e-9 {
            heading.normalized().dot(aim.normalized()).clamp(-1.0, 1.0).acos()
        } else {
            0.0
        };
        self.drive_state = match self.drive_state {
            // At rest on thrusters: start spinning up if there is anything to
            // spin up into.
            DriveState::Thrusters if want != crate::lane::Regime::Thrusters => {
                DriveState::Spooling { to: want, left: crate::lane::spool_seconds(want) }
            }
            DriveState::Thrusters => DriveState::Thrusters,
            // Mid spin-up: finish, or abandon it if the fleet no longer wants it.
            // Changed its mind mid spin-up: start the new one from scratch, or
            // fall back to thrusters if it no longer wants a drive at all.
            DriveState::Spooling { to, .. } if to != want => {
                if want == crate::lane::Regime::Thrusters {
                    DriveState::Thrusters
                } else {
                    DriveState::Spooling { to: want, left: crate::lane::spool_seconds(want) }
                }
            }
            DriveState::Spooling { to, left } => {
                let left = left - dt;
                if left <= 0.0 { DriveState::Cruising(to) } else { DriveState::Spooling { to, left } }
            }
            // Cruising: any change of regime means SHUTTING DOWN first.
            DriveState::Cruising(r) if r != want => {
                DriveState::Dropping { from: r, left: crate::lane::drop_seconds(r) }
            }
            // ...and so does any change of COURSE. In warp nothing is steering,
            // so wanting to go somewhere other than where you are pointed is not
            // something you can act on — you drop out, come about on thrusters,
            // and light it again. A fleet on a LANE is exempt: there the road is
            // steering, and a road bending is not the ship changing its mind.
            DriveState::Cruising(crate::lane::Regime::Warp)
                if off_course > crate::lane::COURSE_LOCK_RAD =>
            {
                DriveState::Dropping { from: crate::lane::Regime::Warp, left: crate::lane::drop_seconds(crate::lane::Regime::Warp) }
            }
            DriveState::Cruising(r) => DriveState::Cruising(r),
            DriveState::Dropping { from, left } => {
                let left = left - dt;
                if left <= 0.0 { DriveState::Thrusters } else { DriveState::Dropping { from, left } }
            }
        };
        self.regime = self.drive_state.regime();
        let factor = match self.regime {
            crate::lane::Regime::Hyperspace => crate::lane::WARP_FACTOR * crate::lane::LANE_MULT,
            crate::lane::Regime::Warp => crate::lane::WARP_FACTOR,
            crate::lane::Regime::Thrusters => 1.0,
        };
        let speed = self.transit_speed() * factor;
        // WHO IS STEERING. On thrusters, the fleet. On a lane, the LANE — a road
        // bending is not the ship changing course, it is the ship being carried
        // along one, so a fleet riding a ribbon follows it freely. Everywhere
        // else the course is locked: in warp there is nothing to follow and no
        // authority to turn with, so the fleet flies the line it committed to
        // when it lit the drive.
        let steered = self.drive_state.can_steer()
            || matches!(self.drive_state, DriveState::Cruising(crate::lane::Regime::Hyperspace));
        let radius = if steered { 0.0 } else { f64::INFINITY };

        match &mut self.order {
            FleetOrder::Idle => {
                // Holds station. (Already at rest.)
                self.vel = Vec2::ZERO;
            }
            FleetOrder::MoveTo { dest } => {
                // Steer to the next waypoint of the planned route, or straight at
                // the destination when there is none.
                let target = self.route.first().map_or(*dest, |l| l.to);
                let carried = self.vel;
                let step = crate::movement::advance_turning(self.pos, self.vel, target, speed, dt, radius);
                self.pos = step.pos;
                self.vel = step.vel;
                if step.arrived {
                    if self.route.is_empty() {
                        self.order = FleetOrder::Idle;
                    } else {
                        self.route.remove(0);
                        // Retiring the LAST waypoint is arriving: the route always
                        // ends at the destination, so there is nothing further to
                        // steer to and the fleet is where it was sent.
                        if self.route.is_empty() {
                            self.order = FleetOrder::Idle;
                        } else if carried.length_sq() > 1e-9 {
                            // CARRY THE HEADING THROUGH an intermediate waypoint.
                            //
                            // Arrival zeroes velocity, which is right at a
                            // destination and wrong at a corner: heading is what
                            // `factor` reads to decide a fleet is riding a lane,
                            // so a fleet that reaches a waypoint at rest is a
                            // fleet that has just left the lane. It then re-aligns
                            // over the next leg, reaches the following waypoint,
                            // and drops out again — paying warp speed
                            // for most of a route it planned to fly at ten times
                            // that. Routed trips came out SLOWER than the straight
                            // line they were meant to beat.
                            //
                            // It is also what let a fleet reverse inside a ribbon:
                            // a waypoint handed it a blank heading, and a blank
                            // heading may set off in any direction at all.
                            self.vel = carried.normalized() * speed;
                        }
                    }
                }
            }
            FleetOrder::Construct { site, .. } | FleetOrder::Demolish { site, .. } => {
                // Fly to the worksite and HOLD there — the construction /
                // demolition resolver runs the clock; going Idle would abandon
                // the job.
                let site = *site;
                let step = crate::movement::advance_turning(self.pos, self.vel, site, speed, dt, radius);
                self.pos = step.pos;
                self.vel = step.vel;
            }
            FleetOrder::Blockade { station, .. } => {
                // Fly to station, then HOLD there (keep the Blockade order — the
                // world reads on-station presence as an active blockade; going
                // Idle would drop it). Once arrived, advance_toward returns the
                // station point at zero velocity, so it simply holds each tick.
                let step = crate::movement::advance_turning(self.pos, self.vel, *station, speed, dt, radius);
                self.pos = step.pos;
                self.vel = step.vel;
            }
            FleetOrder::Survey { station, .. } => {
                // §explore: fly to the star and HOLD (the world's survey resolver
                // runs the dwell clock + completion; going Idle would drop it).
                let step = crate::movement::advance_turning(self.pos, self.vel, *station, speed, dt, radius);
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
            FleetOrder::Intercept { .. } | FleetOrder::Attack { .. } => {
                if let Some(target) = self.route.first().map(|l| l.to) {
                    let step = crate::movement::advance_turning(self.pos, self.vel, target, speed, dt, radius);
                    self.pos = step.pos;
                    self.vel = step.vel;
                    if step.arrived {
                        self.route.remove(0);
                    }
                } else {
                    self.vel = Vec2::ZERO;
                }
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
        let mut f = Fleet::single(EntityId(1), PlayerId(1), ShipKind::Scout, Vec2::ZERO, FleetOrder::Idle, cargo);
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
    fn fleet_of_one_matches_the_old_single_ship_exactly() {
        // A convoy fleet-of-one moves at the convoy's constant speed (§14.1) —
        // cargo affects fuel (mass), not speed.
        let cargo = Some(Cargo { commodity: Commodity::MetallicOre, units: 100 });
        let f = fleet(&[(ShipKind::Convoy, 1)], cargo);
        assert_eq!(f.max_speed(), ShipKind::Convoy.max_speed(), "fleet-of-one speed == its kind's speed");
        assert_eq!(f.flagship_kind(), ShipKind::Convoy);
        assert_eq!(f.total_count(), 1);
    }

    #[test]
    fn formation_speed_is_set_by_the_slowest_member() {
        // A hammer (raider) carrying a colony ship lumbers at the COLONY's pace.
        let f = fleet(&[(ShipKind::Raider, 3), (ShipKind::Colony, 1)], None);
        assert_eq!(f.max_speed(), ShipKind::Colony.max_speed(), "slowest member sets the formation speed");
        // Raider alone is far faster — proving the formation penalty.
        let raider = fleet(&[(ShipKind::Raider, 1)], None);
        assert!(raider.max_speed() > f.max_speed(), "an unencumbered raider is faster");
    }

    #[test]
    fn mass_and_fuel_sum_over_the_whole_convoy_count() {
        let cargo = Some(Cargo { commodity: Commodity::MetallicOre, units: 50 });
        let f = fleet(&[(ShipKind::Convoy, 3)], cargo);
        let expected = 3.0 * ShipKind::Convoy.hull_mass() + 50.0 * CARGO_MASS_PER_UNIT;
        assert!((f.mass() - expected).abs() < 1e-9, "mass = Σ hull×count + cargo");
        // Fuel ∝ distance × total mass, so a 3-convoy fleet burns 3× a 1-convoy
        // fleet's hull share over the same leg (cargo held equal).
        let one = fleet(&[(ShipKind::Convoy, 1)], None);
        let three = fleet(&[(ShipKind::Convoy, 3)], None);
        let d = 1000.0;
        assert!((crate::fuel::fuel_cost(d, three.mass()) - 3.0 * crate::fuel::fuel_cost(d, one.mass())).abs() < 1e-6);
    }

    #[test]
    fn broadcasts_if_any_member_broadcasts() {
        // You cannot hide a freighter by parking a raider beside it.
        assert!(fleet(&[(ShipKind::Raider, 2), (ShipKind::Convoy, 1)], None).broadcasts());
        assert!(fleet(&[(ShipKind::Corvette, 1)], None).broadcasts());
        // Raiders and/or scouts only → dark.
        assert!(!fleet(&[(ShipKind::Raider, 3)], None).broadcasts());
        assert!(!fleet(&[(ShipKind::Raider, 2), (ShipKind::Scout, 1)], None).broadcasts());
    }

    #[test]
    fn flagship_follows_precedence_colony_convoy_corvette_raider_scout() {
        assert_eq!(fleet(&[(ShipKind::Convoy, 1), (ShipKind::Colony, 1)], None).flagship_kind(), ShipKind::Colony);
        assert_eq!(fleet(&[(ShipKind::Convoy, 1), (ShipKind::Corvette, 2)], None).flagship_kind(), ShipKind::Convoy);
        assert_eq!(fleet(&[(ShipKind::Raider, 5), (ShipKind::Scout, 1)], None).flagship_kind(), ShipKind::Raider);
        assert_eq!(fleet(&[(ShipKind::Scout, 2)], None).flagship_kind(), ShipKind::Scout);
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
            assert!(n >= lo_hi.0 && n <= lo_hi.1, "count must lie inside its own bucket");
        }
    }

    #[test]
    fn stealth_transit_halves_speed_and_quiets_the_signature() {
        let mut f = fleet(&[(ShipKind::Raider, 1)], None);
        // Full speed → the detection anchor (signature 1.0).
        f.vel = Vec2::new(f.max_speed(), 0.0);
        let full_sig = f.signature();
        assert!((full_sig - 1.0).abs() < 1e-9, "a lone raider at full speed is the 1.0 anchor");
        assert_eq!(f.transit_speed(), f.max_speed(), "Full transit = formation speed");
        // Stealth halves the move speed and, at that speed, quiets the signature.
        f.transit = TransitMode::Stealth;
        assert!((f.transit_speed() - f.max_speed() * 0.5).abs() < 1e-9, "Stealth creeps at STEALTH_FRACTION");
        f.vel = Vec2::new(f.transit_speed(), 0.0);
        assert!(f.signature() < full_sig, "creeping is quieter than flank speed");
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
        f.loadouts.iter().all(|(k, m)| m.values().sum::<u32>() <= f.count(*k))
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
        assert_eq!(taken.iter().filter(|s| !s.loadout.is_empty()).count(), 2, "escorts keep their fits");
        assert_eq!(f.count(ShipKind::Raider), 1);
        assert_eq!(total_fitted(&f, ShipKind::Raider), 0);
        assert!(invariant_holds(&f));

        // Absorb another fleet's hulls (merge/relief): their fits come along.
        let mut g = fleet(&[(ShipKind::Raider, 2)], None);
        g.set_fitted(ShipKind::Raider, &Loadout::new(vec![ModuleKind::WhippleArmor]), 2);
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
                    *lo.entry(s.kind).or_default().entry(s.stack_key()).or_insert(0) += 1;
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
        assert_eq!(g.ships.iter().filter(|s| s.hp == 50.0).count(), 1, "exactly one hull is still hurt");
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
        assert_eq!(total_fitted(&f, ShipKind::Raider), 0, "the hulls were never actually fitted");
    }

    #[test]
    fn old_snapshot_without_loadouts_loads_all_unfitted() {
        let f = fleet(&[(ShipKind::Raider, 3)], None);
        // Strip the loadouts key → a pre-module snapshot; serde default fills it.
        let mut v: serde_json::Value = serde_json::to_value(&f).unwrap();
        v.as_object_mut().unwrap().remove("loadouts");
        let f2: Fleet = serde_json::from_value(v).unwrap();
        assert!(f2.loadouts.is_empty(), "no field → all-unfitted, zero migration");
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
            j(DriveState::Cruising(crate::lane::Regime::Hyperspace)),
            "{\"cruising\":\"hyperspace\"}"
        );
        assert_eq!(
            j(DriveState::Spooling { to: crate::lane::Regime::Warp, left: 1.5 }),
            "{\"spooling\":{\"to\":\"warp\",\"left\":1.5}}"
        );
        assert_eq!(
            j(DriveState::Dropping { from: crate::lane::Regime::Hyperspace, left: 3.0 }),
            "{\"dropping\":{\"from\":\"hyperspace\",\"left\":3.0}}"
        );
    }
}

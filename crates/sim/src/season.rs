//! §season (docs/endgame-keystone.md): the season arc's clock and the
//! Authority's public voice.
//!
//! Galaxy MATURITY (0–1) is read from what the active corporations have built,
//! their colonies and their research, with a slow time floor so a quiet galaxy
//! still moves. It never falls. The Authority's GRIP follows it through four
//! stages, each announced by a decree. The GALACTIC NEWS NETWORK is the
//! Authority's public feed: every item is broadcast from the Market Hub and
//! reaches a corporation when that light reaches its command center.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::cargo::Commodity;
use crate::ids::{EntityId, PlayerId};
use crate::math::Vec2;

/// How often the Authority takes its census (sim seconds).
pub const CENSUS_EVERY_S: f64 = 60.0;
/// Colonies (owned systems past home) that count as fully grown.
pub const COLONIES_FULL: f64 = 8.0;
/// Completed research programmes that count as fully grown.
pub const RESEARCH_FULL: f64 = 50.0;
/// The time floor: maturity reaches 100% by this long after the season starts
/// (ten weeks), whatever the galaxy builds.
pub const MATURITY_FLOOR_S: f64 = 70.0 * 86_400.0;
/// A census reading is kept when maturity moved at least this much.
pub const CENSUS_STEP: f64 = 0.005;
/// GNN items kept (the oldest drop off).
pub const NEWS_CAP: usize = 200;
/// A battle is news with at least this many hulls on the field…
pub const BATTLE_NEWS_HULLS: u32 = 8;
/// …or this many lost.
pub const BATTLE_NEWS_LOSSES: u32 = 4;
/// One wanted notice per culprit in this long.
pub const WANTED_EVERY_S: f64 = 6.0 * 3_600.0;

// ── The squeeze (grip II on): levies, embargoes, requisitions ──

/// The levy on every Hub sale and purchase, by grip stage.
pub const LEVY_BY_GRIP: [f64; 4] = [0.0, 0.05, 0.07, 0.10];
/// A Hub trade this large (credits) draws a levy rise.
pub const LEVY_RISE_TRADE: f64 = 20_000.0;
/// What one rise adds to a corporation's own levy.
pub const LEVY_RISE_POINTS: f64 = 0.02;
/// A corporation's own levy never climbs past this above the base.
pub const LEVY_EXTRA_MAX: f64 = 0.15;
/// At most one rise demanded a day.
pub const LEVY_RISE_EVERY_S: f64 = 86_400.0;
/// A demand unanswered this long after it is heard: a rise is accepted, a
/// requisition refused.
pub const ANSWER_WINDOW_S: f64 = 86_400.0;
/// A refused rise's arrears: this share of the trade that drew it.
pub const ARREARS_FRAC: f64 = 0.25;
/// A requisition every two days.
pub const REQUISITION_EVERY_S: f64 = 2.0 * 86_400.0;
/// What a requisition asks: this share of the corporation's valuation…
pub const REQUISITION_VALUE_FRAC: f64 = 0.04;
/// …and at least this much, scaled up by the grip.
pub const REQUISITION_MIN: f64 = 2_000.0;
/// A refused requisition costs this much standing…
pub const REFUSAL_STANDING: f64 = 8.0;
/// …and this many refusals bring an enforcement squadron…
pub const REFUSALS_ENFORCED: u32 = 3;
/// …ordered out for this long.
pub const ENFORCEMENT_ORDER_S: f64 = 12.0 * 3_600.0;

// ── Resolve, loyalists and the Authority's crackdowns (phase 3) ──

/// Resolve each 1,000 Cr of levy paid builds.
pub const RESOLVE_PER_LEVY_KCR: f64 = 0.002;
/// Resolve from a levy rise accepted / refused.
pub const RESOLVE_RISE_ACCEPTED: f64 = 0.03;
pub const RESOLVE_RISE_REFUSED: f64 = 0.05;
/// Resolve from a requisition paid / refused.
pub const RESOLVE_REQUISITION_PAID: f64 = 0.02;
pub const RESOLVE_REQUISITION_REFUSED: f64 = 0.04;
/// Resolve a crackdown on a corporation builds.
pub const RESOLVE_CRACKDOWN: f64 = 0.05;
/// Resolve every corporation gains when a false flag is exposed.
pub const RESOLVE_EXPOSED: f64 = 0.10;
/// Resolve fades this much a day without fresh grievance.
pub const RESOLVE_DECAY_PER_DAY: f64 = 0.03;
/// Each Assembly Hall level rallies this much Resolve an hour…
pub const RESOLVE_PER_HALL_HOUR: f64 = 0.01;
/// …up to this much from all of a corporation's halls.
pub const RESOLVE_HALLS_MAX_HOUR: f64 = 0.03;
/// Below this Resolve a corporation is loyal: its levy is lighter…
pub const LOYAL_BELOW: f64 = 0.20;
/// …by this much.
pub const LOYAL_LEVY_DISCOUNT: f64 = 0.02;
/// Resolve that seeks the Keystone: attuning a relic needs it.
pub const RESOLVE_ATTUNE: f64 = 0.50;
/// Production at Resolve 50% and 100%.
pub const RESOLVE_PRODUCTION: [(f64, f64); 2] = [(0.5, 1.05), (1.0, 1.10)];
/// The galaxy's Resolve that brings the emergency decree.
pub const EMERGENCY_RESOLVE: f64 = 0.50;
/// The emergency decree's levy on top of the grip's.
pub const EMERGENCY_LEVY: f64 = 0.02;
/// A crackdown runs this long.
pub const CRACKDOWN_S: f64 = 12.0 * 3_600.0;
/// Evidence at a scene can be found this long.
pub const EVIDENCE_S: f64 = 24.0 * 3_600.0;
/// A Scout this near a scene finds its evidence.
pub const EVIDENCE_RADIUS: f64 = 3_000.0;
/// At most one sabotage, staged massacre or quarantine in this long.
pub const CRACKDOWN_EVERY_S: f64 = 3.0 * 86_400.0;
/// The share of sabotage and massacres the Authority stages.
pub const STAGED_SHARE: f64 = 0.5;
/// A loyalist's standing lost when a false flag is exposed.
pub const EXPOSED_LOYAL_STANDING: f64 = 10.0;
/// The Authority's war fleet: hulls at the start, by maturity, and by the
/// galaxy's Resolve.
pub const WAR_FLEET_BASE: f64 = 6.0;
pub const WAR_FLEET_PER_MATURITY: f64 = 24.0;
pub const WAR_FLEET_PER_RESOLVE: f64 = 20.0;

/// What the Authority does when it cracks down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrackdownKind {
    /// Sabotage at the Hub: 12h of inspections and doubled freight fees.
    Sabotage,
    /// An Authority convoy destroyed: a named culprit, bounties, enforcement.
    Massacre,
    /// Disease on a busy route: Authority freight shuns the system for 12h.
    Quarantine,
    /// Waking relics: holders hand them over or lose standing.
    RelicSeizure,
    /// A Keystone at level 3, 6 or 9: enforcement at its builders.
    Keystone,
}

impl CrackdownKind {
    pub fn title(self) -> &'static str {
        match self {
            CrackdownKind::Sabotage => "Sabotage at the Hub",
            CrackdownKind::Massacre => "Convoy massacre",
            CrackdownKind::Quarantine => "Quarantine",
            CrackdownKind::RelicSeizure => "Relic seizure",
            CrackdownKind::Keystone => "Keystone crackdown",
        }
    }
}

/// One crackdown and the evidence at its scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Crackdown {
    pub id: u64,
    pub kind: CrackdownKind,
    pub at: f64,
    /// It lifts then (at once if exposed).
    pub until: f64,
    /// The scene: where its light and its evidence are.
    pub pos: Vec2,
    #[serde(default)]
    pub system: Option<EntityId>,
    /// The corporation it names or strikes.
    #[serde(default)]
    pub target: Option<PlayerId>,
    /// Staged by the Authority. Never served unless exposed.
    #[serde(default)]
    pub staged: bool,
    /// Its evidence can be found until then (0: none to find).
    #[serde(default)]
    pub evidence_until: f64,
    #[serde(default)]
    pub exposed: bool,
    /// Corporations whose Scouts read the evidence.
    #[serde(default)]
    pub investigated: Vec<PlayerId>,
}

// ── The relic journey, the Precursor worlds and the spy (phase 4) ──

/// Maturity at which the first wave of relics wakes…
pub const RELIC_FIRST_WAVE: f64 = 0.30;
/// …and by which every relic is awake.
pub const RELIC_LAST_WAVE: f64 = 0.60;
/// The share of relics in the first wave.
pub const RELIC_FIRST_SHARE: f64 = 1.0 / 3.0;
/// Attuning a relic takes this long in a young galaxy…
pub const ATTUNE_YOUNG_S: f64 = 24.0 * 3_600.0;
/// …and this long in a mature one.
pub const ATTUNE_MATURE_S: f64 = 4.0 * 3_600.0;
/// A bearing's half-width (radians): the wedge a world lies in.
pub const BEARING_SPREAD: f64 = 0.10;
/// Precursor worlds per twenty home slots.
pub const PRECURSORS_PER_20: f64 = 3.0;
/// The spy's leak: the Authority knows this long after a discovery.
pub const LEAK_S: f64 = 2.0 * 3_600.0;
/// Standing a relic's holder loses when the Authority seizes relics.
pub const RELIC_SEIZURE_STANDING: f64 = 15.0;
/// Bounty the Authority pays for each rebel hull sunk.
pub const BOUNTY_PER_HULL: f64 = 1_500.0;

/// A relic: an exotic node system's resonator, waking with the galaxy.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Relic {
    /// The maturity at which it wakes.
    pub wake_at: f64,
    /// When it woke.
    #[serde(default)]
    pub awake_at: Option<f64>,
    /// Who is attuning it, and how far (0–1).
    #[serde(default)]
    pub attuner: Option<PlayerId>,
    #[serde(default)]
    pub resonance: f64,
    /// Corporations that drew its bearing.
    #[serde(default)]
    pub bearings_to: Vec<PlayerId>,
}

/// A bearing: from a relic toward the nearest sealed world, within
/// [`BEARING_SPREAD`] either side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bearing {
    pub corp: PlayerId,
    pub relic: EntityId,
    pub from: Vec2,
    /// Radians, atan2(dy, dx).
    pub angle: f64,
    pub at: f64,
    /// The sealed world it points at (never served).
    pub target: EntityId,
}

impl Bearing {
    /// Does this bearing's wedge hold `pos`?
    pub fn holds(&self, pos: Vec2) -> bool {
        let d = pos - self.from;
        if d.length() < 1.0 {
            return true;
        }
        let a = crate::dmath::atan2(d.y, d.x);
        let mut off = (a - self.angle).abs() % std::f64::consts::TAU;
        if off > std::f64::consts::PI {
            off = std::f64::consts::TAU - off;
        }
        off <= BEARING_SPREAD
    }
}

/// A sealed Precursor world: its Keystone can close every gate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Precursor {
    /// The world in the system.
    pub body: u32,
    /// Who found it, and when.
    #[serde(default)]
    pub found: Vec<(PlayerId, f64)>,
    /// The first landing (public from then on).
    #[serde(default)]
    pub landed: Option<(PlayerId, f64)>,
    /// The guardians that woke when it was found.
    #[serde(default)]
    pub guard: Option<EntityId>,
    /// §season phase 5: its Keystone, once fed.
    #[serde(default)]
    pub keystone: Option<Keystone>,
}

/// The spy's leak, counting down to the Authority's war.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Leak {
    /// The finder; its syndicate goes to war with it.
    pub corp: PlayerId,
    pub world: EntityId,
    pub at: f64,
}

// ── The Keystone, its powers, the Seal and the endings (phase 5) ──

/// The Keystone's levels.
pub const KEYSTONE_LEVELS: u32 = 10;
/// The Authority cracks down at these levels.
pub const KEYSTONE_CRACKDOWNS: [u32; 3] = [3, 6, 9];
/// The Seal charges this long at level 10.
pub const CHARGE_S: f64 = 48.0 * 3_600.0;
/// The race runs this long from the first discovery…
pub const RACE_S: f64 = 10.0 * 86_400.0;
/// …and if nothing is found by this long into the season, the Authority
/// reseals the worlds.
pub const BACKSTOP_S: f64 = 12.0 * 7.0 * 86_400.0;
/// A held siege knocks a level off this often.
pub const KEYSTONE_SIEGE_TICK_S: f64 = 2.0 * 3_600.0;
/// What a stopper scores: a level knocked off, a rebel hull sunk, a
/// Keystone world taken.
pub const STOP_LEVEL: f64 = 10.0;
pub const STOP_HULL: f64 = 1.0;
pub const STOP_CAPTURE: f64 = 30.0;

/// The goods level `level` takes, scaled by the galaxy's size (`scale` = home
/// slots / 20).
pub fn keystone_basket(level: u32, scale: f64) -> Vec<(Commodity, f64)> {
    let l = level as f64;
    let mut basket = vec![
        (Commodity::Alloys, 300.0 * l),
        (Commodity::Machinery, 120.0 * l),
        (Commodity::Electronics, 80.0 * l),
    ];
    if level >= 4 {
        basket.push((Commodity::Composites, 50.0 * l));
    }
    if level >= 7 {
        basket.push((Commodity::PrecisionComponents, 30.0 * l));
    }
    basket.into_iter().map(|(c, n)| (c, (n * scale.max(0.25)).round())).collect()
}

/// Relics the builders must hold for level `level`: none to 5, then one more
/// a level.
pub fn keystone_relics(level: u32) -> usize {
    level.saturating_sub(5) as usize
}

/// A Keystone rising on a sealed world.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Keystone {
    pub level: u32,
    /// Goods paid toward the next level.
    #[serde(default)]
    pub paid: BTreeMap<Commodity, f64>,
    /// The Seal's charge began then (at level 10).
    #[serde(default)]
    pub charge_since: Option<f64>,
    /// When the siege last knocked a level off (or began to).
    #[serde(default)]
    pub siege_tick_at: Option<f64>,
    /// The Precursor Fleet it raised (once).
    #[serde(default)]
    pub fleet: Option<EntityId>,
}

/// How the season ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Ending {
    /// A Seal fired: every gate closed; the holder rules.
    Sovereign { holder: PlayerId, system: EntityId, at: f64 },
    /// No Seal: the Authority keeps the galaxy; loyalists and the
    /// Keystone-stoppers win.
    CharterHolds { at: f64, winners: Vec<PlayerId> },
}

/// A rise the Authority has demanded after a big Hub trade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevyRise {
    pub points: f64,
    /// The good of the trade that drew it: refused, it is embargoed.
    pub commodity: Commodity,
    /// The trade's value, for the arrears.
    pub trade: f64,
    /// When the Hub issued it.
    pub at: f64,
    /// Accepted by default after this.
    pub due: f64,
}

/// A good this corporation may not trade or ship through the Authority until
/// the arrears are paid (Colonization's boycott).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embargo {
    pub arrears: f64,
    pub since: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Demand {
    /// Goods from the corporation's Hub warehouse.
    Goods { commodity: Commodity, units: u32 },
    Credits { amount: f64 },
}

/// A periodic demand: deliver by the deadline, or refuse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requisition {
    pub id: u64,
    pub demand: Demand,
    pub at: f64,
    /// Refused by default after this.
    pub due: f64,
}

/// The Authority's squeeze on one corporation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Squeeze {
    /// This corporation's own levy above the base (a fraction).
    #[serde(default)]
    pub levy_extra: f64,
    #[serde(default)]
    pub rise: Option<LevyRise>,
    #[serde(default)]
    pub last_rise_at: Option<f64>,
    #[serde(default)]
    pub embargoes: BTreeMap<Commodity, Embargo>,
    #[serde(default)]
    pub requisitions: Vec<Requisition>,
    #[serde(default)]
    pub next_requisition_at: Option<f64>,
    /// Requisitions refused this season.
    #[serde(default)]
    pub refusals: u32,
    /// Levy paid this season (credits).
    #[serde(default)]
    pub levy_paid: f64,
    /// An enforcement squadron is due until then.
    #[serde(default)]
    pub enforced_until: f64,
    /// §season phase 3: the will to break free, 0–1.
    #[serde(default)]
    pub resolve: f64,
}

/// The Authority's grip on the charters, after Colonization's tightening
/// crown: each stage adds a squeeze and none is ever lifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grip {
    /// The freight fee and the standing ladder, as at the founding.
    #[default]
    Charter,
    /// A levy on every Hub sale and purchase; requisitions begin.
    Levies,
    /// Colony licence fees, cargo inspections, a registry tax on warships.
    Inspections,
    /// Garrisons in the Hub district, syndicate dues, enforced requisitions.
    Martial,
}

impl Grip {
    pub const ALL: [Grip; 4] = [Grip::Charter, Grip::Levies, Grip::Inspections, Grip::Martial];

    /// The maturity at which this stage is decreed.
    pub fn threshold(self) -> f64 {
        match self {
            Grip::Charter => 0.0,
            Grip::Levies => 0.25,
            Grip::Inspections => 0.45,
            Grip::Martial => 0.65,
        }
    }

    /// The stage a galaxy this mature has earned.
    pub fn for_maturity(maturity: f64) -> Grip {
        Grip::ALL.into_iter().rev().find(|g| maturity + 1e-9 >= g.threshold()).unwrap_or(Grip::Charter)
    }

    pub fn next(self) -> Option<Grip> {
        Grip::ALL.into_iter().find(|g| *g > self)
    }

    pub fn numeral(self) -> &'static str {
        ["I", "II", "III", "IV"][self as usize]
    }

    pub fn title(self) -> &'static str {
        match self {
            Grip::Charter => "Charter",
            Grip::Levies => "Levies",
            Grip::Inspections => "Inspections",
            Grip::Martial => "Martial Charter",
        }
    }

    /// The decree's headline.
    pub fn decree(self) -> &'static str {
        match self {
            Grip::Charter => "The Authority renews the charters",
            Grip::Levies => "Authority decree: a levy on every Hub trade",
            Grip::Inspections => "Authority decree: colony licences, cargo inspections, a warship registry",
            Grip::Martial => "Authority decree: the Martial Charter",
        }
    }
}

/// Who is speaking: the Authority's network, or the rebels' broadcasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewsSource {
    #[default]
    Gnn,
    FreeSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewsKind {
    Decree,
    Crackdown,
    Wanted,
    War,
    Keystone,
    Battle,
    Capture,
    Relic,
    Market,
    Medal,
    Corporation,
    Retraction,
    Resolve,
}

/// One GNN (or Free Signal) item. Frozen when printed: names are as they
/// stood then.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: u64,
    /// When the Hub broadcast it. Its light leaves the Hub then; a story the
    /// Authority had to hear of first is broadcast when that light got there.
    pub at: f64,
    #[serde(default)]
    pub source: NewsSource,
    pub kind: NewsKind,
    pub headline: String,
    /// The corporations it is about (each sees it highlighted).
    #[serde(default)]
    pub corps: Vec<PlayerId>,
    #[serde(default)]
    pub system: Option<EntityId>,
    #[serde(default)]
    pub fleet: Option<EntityId>,
}

/// One census the Hub published.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Census {
    pub at: f64,
    pub maturity: f64,
    pub grip: Grip,
}

/// The season's state: maturity, the grip and the news.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Season {
    /// When the season's clock started (the first census).
    #[serde(default)]
    pub started_at: Option<f64>,
    /// Galaxy maturity, 0–1. Never falls.
    #[serde(default)]
    pub maturity: f64,
    #[serde(default)]
    pub grip: Grip,
    /// The census readings the Hub published, kept when maturity moved by
    /// [`CENSUS_STEP`] or the grip changed, so a command center far away
    /// reads an older one.
    #[serde(default)]
    pub census: Vec<Census>,
    #[serde(default)]
    pub news: VecDeque<NewsItem>,
    #[serde(default)]
    pub next_news: u64,
    /// When each culprit was last on the wanted list.
    #[serde(default)]
    pub wanted_at: std::collections::BTreeMap<PlayerId, f64>,
    /// Battles that ended by this tick have been looked at for news.
    #[serde(default)]
    pub battles_reported: u64,
    /// The squeeze on each corporation.
    #[serde(default)]
    pub squeeze: BTreeMap<PlayerId, Squeeze>,
    #[serde(default)]
    pub next_requisition: u64,
    /// The galaxy's Resolve at the last census: the active corporations' mean.
    #[serde(default)]
    pub galaxy_resolve: f64,
    /// The emergency decree has been issued.
    #[serde(default)]
    pub emergency: bool,
    /// The Authority's war fleet, in hulls (public).
    #[serde(default)]
    pub war_fleet: u32,
    /// Crackdowns this season, oldest first.
    #[serde(default)]
    pub crackdowns: Vec<Crackdown>,
    #[serde(default)]
    pub next_crackdown: u64,
    /// When the last sabotage, staged massacre and quarantine fell.
    #[serde(default)]
    pub last_sabotage_at: Option<f64>,
    #[serde(default)]
    pub last_staged_at: Option<f64>,
    #[serde(default)]
    pub last_quarantine_at: Option<f64>,
    /// §season phase 4: relics by system, bearings drawn, the sealed worlds,
    /// leaks counting down, and the corporations at war with the Authority
    /// (since when).
    #[serde(default)]
    pub relics: BTreeMap<EntityId, Relic>,
    #[serde(default)]
    pub bearings: Vec<Bearing>,
    #[serde(default)]
    pub precursors: BTreeMap<EntityId, Precursor>,
    #[serde(default)]
    pub precursors_placed: bool,
    #[serde(default)]
    pub leaks: Vec<Leak>,
    #[serde(default)]
    pub rebels: BTreeMap<PlayerId, f64>,
    /// The first Precursor world was found then: registration closes and
    /// the race begins.
    #[serde(default)]
    pub first_found_at: Option<f64>,
    /// §season phase 5: how the season ended.
    #[serde(default)]
    pub ended: Option<Ending>,
    /// What each corporation did to stop the Keystones.
    #[serde(default)]
    pub stoppers: BTreeMap<PlayerId, f64>,
}

impl Season {
    /// A crackdown of `kind` is in force now.
    pub fn in_force(&self, kind: CrackdownKind, now: f64) -> bool {
        self.crackdowns.iter().any(|c| c.kind == kind && c.until > now)
    }

    /// `system` is under quarantine now.
    pub fn quarantined(&self, system: EntityId, now: f64) -> bool {
        self.crackdowns.iter().any(|c| c.kind == CrackdownKind::Quarantine && c.until > now && c.system == Some(system))
    }
}

/// How long attuning takes at `maturity`.
pub fn attune_secs(maturity: f64) -> f64 {
    let t = ((maturity - RELIC_FIRST_WAVE) / (1.0 - RELIC_FIRST_WAVE)).clamp(0.0, 1.0);
    ATTUNE_YOUNG_S + (ATTUNE_MATURE_S - ATTUNE_YOUNG_S) * t
}

/// The maturity at which the relic ranked `rank` of `n` wakes: the first
/// third at once, the rest spread to the last wave.
pub fn relic_wake(rank: usize, n: usize) -> f64 {
    let first = ((n as f64 * RELIC_FIRST_SHARE).ceil() as usize).max(1);
    if rank < first || n <= first {
        return RELIC_FIRST_WAVE;
    }
    RELIC_FIRST_WAVE + (RELIC_LAST_WAVE - RELIC_FIRST_WAVE) * (rank + 1 - first) as f64 / (n - first) as f64
}

/// What a corporation's Resolve does for its production.
pub fn resolve_production(resolve: f64) -> f64 {
    RESOLVE_PRODUCTION.iter().rev().find(|(at, _)| resolve + 1e-9 >= *at).map_or(1.0, |(_, m)| *m)
}

impl Season {
    /// Print an item, broadcast from the Hub at `at`.
    pub fn print(&mut self, at: f64, source: NewsSource, kind: NewsKind, headline: String,
        corps: Vec<PlayerId>, system: Option<EntityId>, fleet: Option<EntityId>) -> u64 {
        self.next_news += 1;
        let id = self.next_news;
        // Keep broadcast order: a story the Hub heard of late lands in place.
        let at_idx = self.news.partition_point(|n| n.at <= at);
        self.news.insert(at_idx, NewsItem { id, at, source, kind, headline, corps, system, fleet });
        while self.news.len() > NEWS_CAP {
            self.news.pop_front();
        }
        id
    }

    /// The items whose broadcast light has reached `cc` by `now`, newest
    /// first, at most `limit`.
    pub fn news_for(&self, cc: Vec2, hub: Vec2, c: f64, now: f64, limit: usize) -> Vec<&NewsItem> {
        let emission = now - crate::transit::delay(hub, cc, c);
        let end = self.news.partition_point(|n| n.at <= emission + 1e-9);
        self.news.range(..end).rev().take(limit).collect()
    }

    /// The census whose light from the Hub has reached `cc`.
    pub fn census_for(&self, cc: Vec2, hub: Vec2, c: f64, now: f64) -> Option<Census> {
        let emission = now - crate::transit::delay(hub, cc, c);
        let end = self.census.partition_point(|r| r.at <= emission + 1e-9);
        end.checked_sub(1).map(|i| self.census[i])
    }

    /// Record a census if it moved enough to publish.
    pub fn record(&mut self, at: f64) {
        let moved = self.census.last().is_none_or(|last| {
            (self.maturity - last.maturity).abs() >= CENSUS_STEP || self.grip != last.grip
        });
        if moved {
            self.census.push(Census { at, maturity: self.maturity, grip: self.grip });
        }
    }
}

/// One corporation's growth, 0–1: half colonies, half research.
pub fn development(colonies: usize, research: usize) -> f64 {
    0.5 * (colonies as f64 / COLONIES_FULL).min(1.0) + 0.5 * (research as f64 / RESEARCH_FULL).min(1.0)
}

/// The galaxy's maturity from its active corporations' growth: the mean of
/// the upper half, so a crowd of newcomers neither stalls it nor does one
/// rusher drag it along.
pub fn measure(mut growth: Vec<f64>) -> f64 {
    if growth.is_empty() {
        return 0.0;
    }
    growth.sort_by(|a, b| b.total_cmp(a));
    let top = growth.len().div_ceil(2);
    growth[..top].iter().sum::<f64>() / top as f64
}

/// The time floor at `elapsed` seconds into the season.
pub fn floor(elapsed: f64) -> f64 {
    (elapsed.max(0.0) / MATURITY_FLOOR_S).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grip_follows_maturity_thresholds() {
        assert_eq!(Grip::for_maturity(0.0), Grip::Charter);
        assert_eq!(Grip::for_maturity(0.249), Grip::Charter);
        assert_eq!(Grip::for_maturity(0.25), Grip::Levies);
        assert_eq!(Grip::for_maturity(0.5), Grip::Inspections);
        assert_eq!(Grip::for_maturity(0.65), Grip::Martial);
        assert_eq!(Grip::for_maturity(1.0), Grip::Martial);
        assert_eq!(Grip::Charter.next(), Some(Grip::Levies));
        assert_eq!(Grip::Martial.next(), None);
    }

    #[test]
    fn maturity_is_the_upper_half_of_the_galaxy() {
        assert_eq!(measure(vec![]), 0.0);
        // Two grown corporations and two newcomers: the upper half reads 1.
        assert!((measure(vec![1.0, 0.0, 1.0, 0.0]) - 1.0).abs() < 1e-9);
        // One rusher among five: the upper three average a third.
        assert!((measure(vec![1.0, 0.0, 0.0, 0.0, 0.0]) - 1.0 / 3.0).abs() < 1e-9);
        assert!((development(4, 25) - 0.5).abs() < 1e-9);
        assert!((development(20, 200) - 1.0).abs() < 1e-9);
        assert!((floor(MATURITY_FLOOR_S / 2.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn news_arrives_by_light_from_the_hub_newest_first() {
        let mut season = Season::default();
        season.print(10.0, NewsSource::Gnn, NewsKind::Decree, "a".into(), vec![], None, None);
        season.print(20.0, NewsSource::Gnn, NewsKind::Medal, "b".into(), vec![], None, None);
        // A story the Hub heard of late slots in by its broadcast time.
        season.print(15.0, NewsSource::Gnn, NewsKind::Battle, "c".into(), vec![], None, None);
        let hub = Vec2::new(0.0, 0.0);
        let cc = Vec2::new(4_000.0, 0.0);
        let delay = crate::transit::delay(hub, cc, 400.0);
        let heard = |now: f64| season.news_for(cc, hub, 400.0, now, 10).iter().map(|n| n.headline.clone()).collect::<Vec<_>>();
        assert!(heard(10.0 + delay - 0.1).is_empty());
        assert_eq!(heard(15.0 + delay), ["c", "a"]);
        assert_eq!(heard(30.0 + delay), ["b", "c", "a"]);
    }
}
